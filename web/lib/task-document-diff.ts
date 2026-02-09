import type { TaskDocumentKind } from "@/lib/luban-api"

type DiffOp =
  | { type: "equal"; line: string }
  | { type: "add"; line: string }
  | { type: "remove"; line: string }

type NumberedOp = DiffOp & {
  oldNo: number | null
  newNo: number | null
}

const DEFAULT_CONTEXT = 2
const DEFAULT_MAX_HUNKS = 8
const DEFAULT_MAX_DIFF_LINES = 180
const LCS_MATRIX_CELL_LIMIT = 200_000

function splitLines(text: string): string[] {
  return text.split(/\r?\n/)
}

function removeAllWhitespace(text: string): string {
  return text.replace(/\s+/g, "")
}

export function isWhitespaceOnlyChange(before: string, after: string): boolean {
  if (before === after) return false
  return removeAllWhitespace(before) === removeAllWhitespace(after)
}

function findCommonPrefixLines(a: string[], b: string[]): number {
  let i = 0
  while (i < a.length && i < b.length && a[i] === b[i]) i += 1
  return i
}

function findCommonSuffixLines(a: string[], b: string[], prefix: number): number {
  let suffix = 0
  while (
    suffix + prefix < a.length &&
    suffix + prefix < b.length &&
    a[a.length - 1 - suffix] === b[b.length - 1 - suffix]
  ) {
    suffix += 1
  }
  return suffix
}

function fallbackOps(oldLines: string[], newLines: string[]): DiffOp[] {
  const prefix = findCommonPrefixLines(oldLines, newLines)
  const suffix = findCommonSuffixLines(oldLines, newLines, prefix)
  const out: DiffOp[] = []
  for (let i = 0; i < prefix; i += 1) out.push({ type: "equal", line: oldLines[i] })
  for (let i = prefix; i < oldLines.length - suffix; i += 1) {
    out.push({ type: "remove", line: oldLines[i] })
  }
  for (let i = prefix; i < newLines.length - suffix; i += 1) {
    out.push({ type: "add", line: newLines[i] })
  }
  for (let i = suffix - 1; i >= 0; i -= 1) {
    out.push({ type: "equal", line: oldLines[oldLines.length - 1 - i] })
  }
  return out
}

function lcsOps(oldLines: string[], newLines: string[]): DiffOp[] {
  const n = oldLines.length
  const m = newLines.length
  if ((n + 1) * (m + 1) > LCS_MATRIX_CELL_LIMIT) {
    return fallbackOps(oldLines, newLines)
  }

  const dp: number[][] = Array.from({ length: n + 1 }, () => Array<number>(m + 1).fill(0))
  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      if (oldLines[i] === newLines[j]) {
        dp[i][j] = dp[i + 1][j + 1] + 1
      } else {
        dp[i][j] = Math.max(dp[i + 1][j], dp[i][j + 1])
      }
    }
  }

  const ops: DiffOp[] = []
  let i = 0
  let j = 0
  while (i < n && j < m) {
    if (oldLines[i] === newLines[j]) {
      ops.push({ type: "equal", line: oldLines[i] })
      i += 1
      j += 1
      continue
    }
    if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ type: "remove", line: oldLines[i] })
      i += 1
    } else {
      ops.push({ type: "add", line: newLines[j] })
      j += 1
    }
  }
  while (i < n) {
    ops.push({ type: "remove", line: oldLines[i] })
    i += 1
  }
  while (j < m) {
    ops.push({ type: "add", line: newLines[j] })
    j += 1
  }
  return ops
}

function numberOps(ops: DiffOp[]): NumberedOp[] {
  const out: NumberedOp[] = []
  let oldNo = 1
  let newNo = 1
  for (const op of ops) {
    if (op.type === "equal") {
      out.push({ ...op, oldNo, newNo })
      oldNo += 1
      newNo += 1
      continue
    }
    if (op.type === "remove") {
      out.push({ ...op, oldNo, newNo: null })
      oldNo += 1
      continue
    }
    out.push({ ...op, oldNo: null, newNo })
    newNo += 1
  }
  return out
}

function nearestOldNo(entries: NumberedOp[], index: number): number {
  for (let i = index; i >= 0; i -= 1) {
    const oldNo = entries[i].oldNo
    if (oldNo != null) return oldNo
  }
  return 1
}

function nearestNewNo(entries: NumberedOp[], index: number): number {
  for (let i = index; i >= 0; i -= 1) {
    const newNo = entries[i].newNo
    if (newNo != null) return newNo
  }
  return 1
}

function renderUnifiedDiff(before: string, after: string): string {
  const ops = numberOps(lcsOps(splitLines(before), splitLines(after)))
  if (ops.length === 0) return ""

  const changedIndexes: number[] = []
  for (let i = 0; i < ops.length; i += 1) {
    if (ops[i].type !== "equal") changedIndexes.push(i)
  }
  if (changedIndexes.length === 0) return ""

  const ranges: Array<{ start: number; end: number }> = []
  for (const idx of changedIndexes) {
    const start = Math.max(0, idx - DEFAULT_CONTEXT)
    const end = Math.min(ops.length - 1, idx + DEFAULT_CONTEXT)
    const prev = ranges[ranges.length - 1]
    if (!prev || start > prev.end + 1) {
      ranges.push({ start, end })
    } else {
      prev.end = Math.max(prev.end, end)
    }
  }

  const chunks: string[] = []
  for (const range of ranges.slice(0, DEFAULT_MAX_HUNKS)) {
    const span = ops.slice(range.start, range.end + 1)
    const oldCount = span.filter((v) => v.type !== "add").length
    const newCount = span.filter((v) => v.type !== "remove").length
    const oldStart = span.find((v) => v.oldNo != null)?.oldNo ?? nearestOldNo(ops, range.start - 1)
    const newStart = span.find((v) => v.newNo != null)?.newNo ?? nearestNewNo(ops, range.start - 1)

    chunks.push(`@@ -${oldStart},${oldCount} +${newStart},${newCount} @@`)
    for (const row of span) {
      if (row.type === "equal") chunks.push(` ${row.line}`)
      if (row.type === "remove") chunks.push(`-${row.line}`)
      if (row.type === "add") chunks.push(`+${row.line}`)
    }
  }

  const limited = chunks.slice(0, DEFAULT_MAX_DIFF_LINES)
  if (chunks.length > limited.length) {
    limited.push("... (diff truncated)")
  }
  return limited.join("\n")
}

function kindLabel(kind: TaskDocumentKind): string {
  if (kind === "task") return "TASK.md"
  if (kind === "plan") return "PLAN.md"
  return "MEMORY.md"
}

export function buildTaskDocumentChangePrompt(args: {
  kind: TaskDocumentKind
  path: string
  before: string
  after: string
}): string | null {
  if (args.before === args.after) return null
  if (isWhitespaceOnlyChange(args.before, args.after)) return null

  const diff = renderUnifiedDiff(args.before, args.after)
  if (!diff.trim()) return null

  return [
    "Task document updated by user edits.",
    "",
    `Document: ${kindLabel(args.kind)}`,
    `Path: ${args.path}`,
    "Apply this update and continue the task based on the latest content.",
    "",
    "Change patch:",
    "```diff",
    diff,
    "```",
  ].join("\n")
}
