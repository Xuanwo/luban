const GENERIC_FONT_FAMILIES = new Set([
  "serif",
  "sans-serif",
  "monospace",
  "cursive",
  "fantasy",
  "system-ui",
  "ui-serif",
  "ui-sans-serif",
  "ui-monospace",
  "ui-rounded",
  "emoji",
  "math",
  "fangsong",
])

function splitFontFamilyList(input: string): string[] {
  const out: string[] = []
  let current = ""
  let quote: '"' | "'" | null = null
  let escaped = false

  for (const ch of input) {
    if (escaped) {
      current += ch
      escaped = false
      continue
    }

    if (quote) {
      current += ch
      if (ch === "\\") {
        escaped = true
      } else if (ch === quote) {
        quote = null
      }
      continue
    }

    if (ch === '"' || ch === "'") {
      quote = ch
      current += ch
      continue
    }

    if (ch === ",") {
      out.push(current)
      current = ""
      continue
    }

    current += ch
  }

  out.push(current)
  return out
}

function stripWrappingQuotes(input: string): string {
  const trimmed = input.trim()
  if (trimmed.length < 2) return trimmed
  const first = trimmed[0]
  const last = trimmed[trimmed.length - 1]
  if ((first === '"' && last === '"') || (first === "'" && last === "'")) {
    return trimmed.slice(1, -1).trim()
  }
  return trimmed
}

function normalizeFamilyName(input: string): string | null {
  const stripped = stripWrappingQuotes(input)
  if (!stripped) return null

  const lowered = stripped.toLowerCase()
  if (GENERIC_FONT_FAMILIES.has(lowered)) return lowered

  const escaped = stripped.replaceAll("\\", "\\\\").replaceAll('"', '\\"')
  return `"${escaped}"`
}

export function buildFontFamilyList(
  preferred: string,
  fallbacks: readonly string[] = [],
): string {
  const seen = new Set<string>()
  const normalized: string[] = []

  const push = (raw: string) => {
    const item = normalizeFamilyName(raw)
    if (!item) return
    const key = item.toLowerCase()
    if (seen.has(key)) return
    seen.add(key)
    normalized.push(item)
  }

  for (const raw of splitFontFamilyList(preferred)) push(raw)
  for (const raw of fallbacks) push(raw)

  return normalized.join(", ")
}
