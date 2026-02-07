"use client"

import type {
  AttachmentRef,
  AgentEvent,
  AgentRunnerKind,
  ConversationEntry,
  ConversationSnapshot,
  TaskStatus,
  ThinkingEffort,
} from "./luban-api"
import { AGENT_MODELS, DROID_MODELS } from "./agent-settings"
import { formatDurationMs } from "./duration-format"

export { formatDurationMs } from "./duration-format"

export interface ActivityEvent {
  id: string
  type: "thinking" | "tool_call" | "file_edit" | "bash" | "search" | "complete" | "assistant_message"
  title: string
  detail?: string
  status: "running" | "done"
  duration?: string
  badge?: string
  timing?: { startedAtUnixMs: number | null; doneAtUnixMs: number | null }
  subagent?: {
    taskId: number | null
    title: string | null
    runner: AgentRunnerKind | null
    action: "delegated" | "created" | "updated" | "completed"
    status: "running" | "done" | "blocked" | "failed" | "canceled"
  }
}

type ActivityStatus = ActivityEvent["status"]

export type AgentTurnStatus = "running" | "done" | "canceled" | "error"

export interface SystemEvent {
  id: string
  type: "event"
  eventType:
    | "task_created"
    | "task_archived"
    | "task_started"
    | "task_completed"
    | "task_cancelled"
    | "status_changed"
  title: string
  timestamp?: string
}

export interface Message {
  id: string
  type: "user" | "assistant" | "event" | "agent_turn" | "terminal_command" | "task_status_suggestion"
  eventSource?: "system" | "user" | "agent"
  agentRunner?: AgentRunnerKind
  content: string
  attachments?: AttachmentRef[]
  timestamp?: string
  isStreaming?: boolean
  isCancelled?: boolean
  status?: ActivityStatus
  activities?: ActivityEvent[]
  turnStatus?: AgentTurnStatus
  metadata?: {
    toolCalls?: number
    thinkingSteps?: number
    duration?: string
  }
  codeReferences?: { file: string; line: number }[]
  eventType?:
    | "task_created"
    | "task_archived"
    | "task_started"
    | "task_completed"
    | "task_cancelled"
    | "status_changed"
  terminalCommand?: {
    id: string
    command: string
    reconnect: string
    status: ActivityStatus
    outputBase64?: string
    outputByteLen?: number
  }
  taskStatusSuggestion?: {
    from: TaskStatus
    to: TaskStatus
    title: string
    explanationMarkdown: string
  }
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

export function agentModelLabel(modelId: string | null | undefined): string {
  if (!modelId) return "Model"
  return (
    AGENT_MODELS.find((m) => m.id === modelId)?.label ??
    DROID_MODELS.find((m) => m.id === modelId)?.label ??
    modelId
  )
}

export function agentRunnerLabel(runner: AgentRunnerKind | null | undefined): string {
  if (!runner) return "Agent"
  if (runner === "codex") return "Codex"
  if (runner === "claude") return "Claude"
  if (runner === "amp") return "Amp"
  if (runner === "droid") return "Droid"
  return runner
}

export function thinkingEffortLabel(effort: ThinkingEffort | null | undefined): string {
  if (!effort) return "Effort"
  if (effort === "minimal") return "Minimal"
  if (effort === "low") return "Low"
  if (effort === "medium") return "Medium"
  if (effort === "high") return "High"
  if (effort === "xhigh") return "XHigh"
  return effort
}

export function activityFromAgentItemLike(args: {
  id: string
  kind: string
  payload: unknown
  forcedStatus?: "running" | "done"
}): ActivityEvent {
  const payload = args.payload as any
  const kind = args.kind
  const forcedStatus = args.forcedStatus

  const firstSentence = (text: string): string => {
    const trimmed = text.trim()
    if (!trimmed) return ""

    const stripSummaryMarkdown = (value: string): string => {
      // Strip simple markdown emphasis markers from short summaries so the UI
      // displays plain text (e.g. "**Plan**" -> "Plan").
      let out = value
      out = out.replaceAll(/\*\*([^*]+?)\*\*/g, "$1")
      out = out.replaceAll(/__([^_]+?)__/g, "$1")
      out = out.replaceAll(/\*\*/g, "")
      out = out.replaceAll(/__/g, "")
      return out.trim()
    }

    const firstLine = trimmed.split(/\r?\n/)[0] ?? trimmed
    const match = firstLine.match(/^(.+?[.!?])(\s|$)/)
    const sentence = stripSummaryMarkdown((match?.[1] ?? firstLine).trim())
    const fallback = stripSummaryMarkdown(firstLine.trim())
    return sentence.length > 0 ? sentence : fallback
  }

  const normalizeShellCommand = (
    rawCommand: string,
  ): { displayCommand: string; badge?: "zsh" | "bash" } => {
    const trimmed = rawCommand.trim()
    const match = trimmed.match(/^(?:\/bin\/)?(zsh|bash)\s+-lc\s+(.+)$/)
    if (!match) return { displayCommand: trimmed }

    const shell = (match[1] ?? "").toLowerCase() as "zsh" | "bash"
    let inner = (match[2] ?? "").trim()
    if (
      (inner.startsWith('"') && inner.endsWith('"')) ||
      (inner.startsWith("'") && inner.endsWith("'"))
    ) {
      inner = inner.slice(1, -1).trim()
    }
    return { displayCommand: inner.length > 0 ? inner : trimmed, badge: shell }
  }

  if (kind === "command_execution") {
    const status = forcedStatus ?? (payload?.status === "in_progress" ? "running" : "done")
    const normalized = normalizeShellCommand(payload?.command ?? "Command")
    return {
      id: args.id,
      type: "bash",
      title: normalized.displayCommand,
      detail: payload?.aggregated_output ?? "",
      status,
      badge: normalized.badge,
    }
  }

  if (kind === "file_change") {
    const status = forcedStatus ?? "done"
    const changes = Array.isArray(payload?.changes) ? payload.changes : []

    const normalizePathForSummary = (raw: unknown): string => {
      const value = String(raw ?? "").trim()
      if (!value) return ""
      return value.replace(/^(\.\/|\.\\)+/, "")
    }

    const paths = (() => {
      const out: string[] = []
      for (const change of changes) {
        const path = normalizePathForSummary(change?.path)
        if (!path) continue
        if (out.includes(path)) continue
        out.push(path)
      }
      return out
    })()

    const title = (() => {
      if (paths.length === 0) return `File changes (${changes.length})`
      const limit = 3
      const shown = paths.slice(0, limit)
      const remaining = paths.length - shown.length
      const suffix = remaining > 0 ? `, +${remaining}` : ""
      return `File changes: ${shown.join(", ")}${suffix}`
    })()

    const detail = changes.map((c: any) => `${c.kind ?? "update"} ${normalizePathForSummary(c.path)}`).join("\n")
    return {
      id: args.id,
      type: "file_edit",
      title,
      detail,
      status,
    }
  }

  if (kind === "mcp_tool_call") {
    const status = forcedStatus ?? (payload?.status === "in_progress" ? "running" : "done")
    const server = String(payload?.server ?? "mcp")
    const tool = String(payload?.tool ?? "tool")
    const lowerServer = server.toLowerCase()
    const lowerTool = tool.toLowerCase()

    const parseSubagentEvent = (): ActivityEvent["subagent"] | null => {
      if (lowerServer !== "luban") return null
      if (!/(subtask|delegate)/.test(lowerTool)) return null

      const argsObj = payload?.arguments as Record<string, unknown> | null | undefined
      const resultObj = payload?.result as Record<string, unknown> | null | undefined

      const rawTaskId = resultObj?.task_id ?? resultObj?.subtask_id ?? argsObj?.task_id ?? argsObj?.subtask_id ?? null
      const parsedTaskId = Number(rawTaskId)
      const taskId = Number.isFinite(parsedTaskId) ? parsedTaskId : null

      const rawTitle = resultObj?.title ?? argsObj?.title ?? null
      const subtaskTitle = typeof rawTitle === "string" && rawTitle.trim().length > 0 ? rawTitle.trim() : null

      const rawRunner = resultObj?.runner ?? argsObj?.runner ?? null
      const runner = (() => {
        if (rawRunner === "codex" || rawRunner === "amp" || rawRunner === "claude") return rawRunner
        return null
      })()

      const action = (() => {
        if (/create|new/.test(lowerTool)) return "created" as const
        if (/complete|finish|done/.test(lowerTool)) return "completed" as const
        if (/progress|update/.test(lowerTool)) return "updated" as const
        return "delegated" as const
      })()

      const rawStatus = String(resultObj?.status ?? argsObj?.status ?? payload?.status ?? "").toLowerCase()
      const normalizedStatus = (() => {
        if (rawStatus.includes("fail") || rawStatus.includes("error")) return "failed" as const
        if (rawStatus.includes("cancel")) return "canceled" as const
        if (rawStatus.includes("block") || rawStatus.includes("pending")) return "blocked" as const
        if (rawStatus.includes("run") || rawStatus.includes("progress")) return "running" as const
        if (rawStatus.includes("done") || rawStatus.includes("success") || rawStatus.includes("complete")) return "done" as const
        return status === "running" ? "running" : "done"
      })()

      return {
        taskId,
        title: subtaskTitle,
        runner,
        action,
        status: normalizedStatus,
      }
    }

    const subagent = parseSubagentEvent()
    const title = (() => {
      if (!subagent) return `${server}.${tool}`
      const actionText = (() => {
        if (subagent.action === "created") return "Created subtask"
        if (subagent.action === "updated") return "Updated subtask"
        if (subagent.action === "completed") return "Completed subtask"
        return "Delegated subtask"
      })()
      const idText = subagent.taskId != null ? ` #${subagent.taskId}` : ""
      const titleText = subagent.title ? ` ${subagent.title}` : ""
      return `${actionText}${idText}${titleText}`
    })()
    const detail = safeStringify({
      arguments: payload?.arguments ?? null,
      result: payload?.result ?? null,
      error: payload?.error ?? null,
      status: payload?.status ?? null,
    })
    return { id: args.id, type: "tool_call", title, detail, status, subagent: subagent ?? undefined }
  }

  if (kind === "web_search") {
    return {
      id: args.id,
      type: "search",
      title: payload?.query ?? "Web search",
      detail: safeStringify(payload ?? null),
      status: forcedStatus ?? "done",
    }
  }

  if (kind === "todo_list") {
    const items = Array.isArray(payload?.items) ? payload.items : []
    const detail = items.map((i: any) => `${i.completed ? "[x]" : "[ ]"} ${i.text ?? ""}`).join("\n")
    return { id: args.id, type: "tool_call", title: "Todo list", detail, status: forcedStatus ?? "done" }
  }

  if (kind === "reasoning") {
    const full = payload?.text ?? ""
    const summary = firstSentence(full)
    return {
      id: args.id,
      type: "thinking",
      title: summary.length > 0 ? summary : "Think",
      detail: full,
      status: forcedStatus ?? "done",
    }
  }

  if (kind === "error") {
    return {
      id: args.id,
      type: "tool_call",
      title: "Error",
      detail: payload?.message ?? "",
      status: forcedStatus ?? "done",
    }
  }

  return {
    id: args.id,
    type: "complete",
    title: kind,
    detail: safeStringify(args.payload),
    status: forcedStatus ?? "done",
  }
}

export function activityFromAgentItem(entry: Extract<AgentEvent, { type: "item" }>): ActivityEvent {
  return activityFromAgentItemLike({ id: entry.id, kind: entry.kind, payload: entry.payload })
}

function inferActivityStatusFromPayload(payload: unknown): ActivityStatus {
  const status = (payload as any)?.status
  if (status === "in_progress") return "running"
  return "done"
}

function activityDisplayPriority(event: ActivityEvent): number {
  if (event.type === "assistant_message") return 100
  return 10
}

export function pickStreamingSummaryActivity(activities: ActivityEvent[]): ActivityEvent | undefined {
  for (let idx = activities.length - 1; idx >= 0; idx -= 1) {
    const event = activities[idx]
    if (event?.type === "assistant_message") return event
  }
  return activities[activities.length - 1]
}

export function buildAgentActivities(conversation: ConversationSnapshot | null): ActivityEvent[] {
  if (!conversation) return []

  const order: string[] = []
  const latestById = new Map<string, ActivityEvent>()
  const timingById = new Map<string, { startedAtUnixMs: number | null; doneAtUnixMs: number | null }>()

  const normalizeUnixMs = (unixMs: unknown): number | null => {
    if (typeof unixMs !== "number" || !Number.isFinite(unixMs) || unixMs <= 0) return null
    return unixMs
  }

  const upsert = (event: ActivityEvent) => {
    if (!latestById.has(event.id)) order.push(event.id)
    latestById.set(event.id, event)
  }

  const lastUserIndex = (() => {
    for (let i = conversation.entries.length - 1; i >= 0; i -= 1) {
      const entry = conversation.entries[i]
      if (entry?.type === "user_event" && entry.event.type === "message") return i
    }
    return -1
  })()

  for (const entry of conversation.entries.slice(lastUserIndex + 1)) {
    if (entry.type !== "agent_event") continue
    const entryCreatedAtUnixMs = normalizeUnixMs(entry.created_at_unix_ms)
    const ev = entry.event
    if (ev.type === "item") {
      const base = activityFromAgentItem(ev)
      const status = inferActivityStatusFromPayload(ev.payload)
      const timing = timingById.get(base.id) ?? { startedAtUnixMs: null, doneAtUnixMs: null }
      if (timing.startedAtUnixMs == null && entryCreatedAtUnixMs != null) timing.startedAtUnixMs = entryCreatedAtUnixMs
      if (status === "done" && entryCreatedAtUnixMs != null) {
        if (timing.doneAtUnixMs == null || entryCreatedAtUnixMs >= timing.doneAtUnixMs) {
          timing.doneAtUnixMs = entryCreatedAtUnixMs
        }
      }
      timingById.set(base.id, timing)
      upsert({ ...base, status, timing })
      continue
    }
    if (ev.type === "turn_duration") {
      upsert({
        id: `turn_duration_${ev.duration_ms}`,
        type: "complete",
        title: `Turn duration: ${formatDurationMs(ev.duration_ms)}`,
        status: "done",
        timing: { startedAtUnixMs: entryCreatedAtUnixMs, doneAtUnixMs: entryCreatedAtUnixMs },
      })
      continue
    }
    if (ev.type === "turn_usage") {
      upsert({
        id: "turn_usage",
        type: "tool_call",
        title: "Turn usage",
        detail: safeStringify(ev.usage_json ?? null),
        status: "done",
        timing: { startedAtUnixMs: entryCreatedAtUnixMs, doneAtUnixMs: entryCreatedAtUnixMs },
      })
      continue
    }
    if (ev.type === "turn_error") {
      upsert({
        id: "turn_error",
        type: "tool_call",
        title: "Turn error",
        detail: ev.message,
        status: "done",
        timing: { startedAtUnixMs: entryCreatedAtUnixMs, doneAtUnixMs: entryCreatedAtUnixMs },
      })
      continue
    }
    if (ev.type === "turn_canceled") {
      upsert({
        id: "turn_canceled",
        type: "tool_call",
        title: "Turn canceled",
        status: "done",
        timing: { startedAtUnixMs: entryCreatedAtUnixMs, doneAtUnixMs: entryCreatedAtUnixMs },
      })
      continue
    }
  }

  return order.map((id) => latestById.get(id)!).filter(Boolean)
}

export function buildMessages(
  conversation: ConversationSnapshot | null,
  opts?: { agentTurns?: "flat" | "grouped" },
): Message[] {
  if (!conversation) return []
  const mode = opts?.agentTurns ?? "flat"
  return mode === "grouped" ? buildMessagesGroupedTurns(conversation) : buildMessagesFlatEvents(conversation)
}

function buildMessagesGroupedTurns(conversation: ConversationSnapshot): Message[] {
  const out: Message[] = []

  const normalizeUnixMs = (unixMs: unknown): number | null => {
    if (typeof unixMs !== "number" || !Number.isFinite(unixMs) || unixMs <= 0) return null
    return unixMs
  }

  const unixMsToIso = (unixMs: unknown): string | undefined => {
    const normalized = normalizeUnixMs(unixMs)
    if (normalized == null) return undefined
    return new Date(normalized).toISOString()
  }

  const taskStatusLabel = (status: string): string => {
    switch (status) {
      case "backlog":
        return "Backlog"
      case "todo":
        return "Todo"
      case "iterating":
      case "in_progress":
        return "Iterating"
      case "validating":
      case "in_review":
        return "Validating"
      case "done":
        return "Done"
      case "canceled":
        return "Canceled"
      default:
        return status
    }
  }

  const turnMessageById = new Map<string, Message>()
  const turnActivityById = new Map<
    string,
    {
      order: string[]
      latestByRowId: Map<string, ActivityEvent>
      latestByKey: Map<string, string>
      timingByKey: Map<string, { startedAtUnixMs: number | null; doneAtUnixMs: number | null }>
      rowMetaByRowId: Map<string, { createdAtUnixMs: number | null; seq: number; priority: number }>
      nextSeq: number
    }
  >()
  let lastUserEntryId: string | null = null
  let lastUserOutIndex: number | null = null

  const ensureTurnMessage = (turnId: string): Message => {
    const existing = turnMessageById.get(turnId) ?? null
    if (existing) return existing

    const msg: Message = {
      id: turnId,
      type: "agent_turn",
      eventSource: "agent",
      agentRunner: conversation.agent_runner,
      content: "",
      activities: [],
      turnStatus: "done",
    }
    if (lastUserOutIndex != null) {
      out.splice(lastUserOutIndex + 1, 0, msg)
      lastUserOutIndex += 1
    } else {
      out.push(msg)
    }
    turnMessageById.set(turnId, msg)
    turnActivityById.set(turnId, {
      order: [],
      latestByRowId: new Map(),
      latestByKey: new Map(),
      timingByKey: new Map(),
      rowMetaByRowId: new Map(),
      nextSeq: 0,
    })
    return msg
  }

  const ensureTurnActivityTiming = (
    turnId: string,
    key: string,
    status: ActivityStatus,
    createdAtUnixMs: number | null,
  ): { startedAtUnixMs: number | null; doneAtUnixMs: number | null } => {
    const state = turnActivityById.get(turnId) ?? null
    if (!state) return { startedAtUnixMs: null, doneAtUnixMs: null }

    const timing = state.timingByKey.get(key) ?? { startedAtUnixMs: null, doneAtUnixMs: null }
    if (timing.startedAtUnixMs == null && createdAtUnixMs != null) timing.startedAtUnixMs = createdAtUnixMs
    if (status === "done" && createdAtUnixMs != null) {
      if (timing.doneAtUnixMs == null || createdAtUnixMs >= timing.doneAtUnixMs) {
        timing.doneAtUnixMs = createdAtUnixMs
      }
    }
    state.timingByKey.set(key, timing)
    return timing
  }

  const activityTitleFromMessageText = (text: string): string => {
    const trimmed = text.trim()
    if (!trimmed) return "Assistant update"
    const firstLine = (trimmed.split(/\r?\n/)[0] ?? "").trim()
    if (!firstLine) return "Assistant update"
    const maxLen = 96
    if (firstLine.length <= maxLen) return firstLine
    return `${firstLine.slice(0, maxLen - 1)}...`
  }

  const compareActivityRows = (
    state: {
      rowMetaByRowId: Map<string, { createdAtUnixMs: number | null; seq: number; priority: number }>
    },
    leftRowId: string,
    rightRowId: string,
  ): number => {
    const left = state.rowMetaByRowId.get(leftRowId) ?? {
      createdAtUnixMs: null,
      seq: Number.MAX_SAFE_INTEGER,
      priority: 0,
    }
    const right = state.rowMetaByRowId.get(rightRowId) ?? {
      createdAtUnixMs: null,
      seq: Number.MAX_SAFE_INTEGER,
      priority: 0,
    }

    const leftTime = left.createdAtUnixMs ?? Number.MIN_SAFE_INTEGER
    const rightTime = right.createdAtUnixMs ?? Number.MIN_SAFE_INTEGER
    if (leftTime !== rightTime) return leftTime - rightTime
    if (left.priority !== right.priority) return left.priority - right.priority
    return left.seq - right.seq
  }

  const appendTurnActivity = (
    turnId: string,
    args: {
      key: string
      rowId: string
      event: ActivityEvent
      timestamp?: string
      createdAtUnixMs?: number | null
      priority?: number
    },
  ) => {
    const state = turnActivityById.get(turnId)
    if (!state) return

    const existingRowId = state.latestByKey.get(args.key) ?? null
    if (existingRowId == null || existingRowId !== args.rowId) {
      if (existingRowId != null) {
        state.latestByRowId.delete(existingRowId)
        state.order = state.order.filter((id) => id !== existingRowId)
        state.rowMetaByRowId.delete(existingRowId)
      }
      if (!state.latestByRowId.has(args.rowId)) {
        state.order.push(args.rowId)
      }
      state.latestByKey.set(args.key, args.rowId)
    }

    state.latestByRowId.set(args.rowId, args.event)
    const previousMeta = state.rowMetaByRowId.get(args.rowId) ?? null
    const seq = previousMeta?.seq ?? state.nextSeq
    if (previousMeta == null) state.nextSeq += 1
    state.rowMetaByRowId.set(args.rowId, {
      createdAtUnixMs: args.createdAtUnixMs ?? null,
      seq,
      priority: args.priority ?? activityDisplayPriority(args.event),
    })

    const msg = turnMessageById.get(turnId) ?? null
    if (!msg) return
    if (!msg.timestamp && args.timestamp) msg.timestamp = args.timestamp
    const sortedRowIds = [...state.order].sort((left, right) => compareActivityRows(state, left, right))
    msg.activities = sortedRowIds.map((id) => state.latestByRowId.get(id)!).filter(Boolean)
  }

  const applyTurnStatus = (turnId: string, status: AgentTurnStatus) => {
    const msg = turnMessageById.get(turnId) ?? null
    if (!msg || msg.type !== "agent_turn") return
    msg.turnStatus = status
  }

  const terminalCommandIndexById = new Map<string, number>()

  for (let i = 0; i < conversation.entries.length; i += 1) {
    const entry = conversation.entries[i]!
    if (entry.type === "system_event") {
      const ev = entry.event as any
      if (ev?.event_type === "task_status_suggestion") {
        const from = String(ev.from ?? "") as TaskStatus
        const to = String(ev.to ?? "") as TaskStatus
        const title = String(ev.title ?? "").trim()
        const explanationMarkdown = String(ev.explanation_markdown ?? "")
        const fromLabel = taskStatusLabel(from)
        const toLabel = taskStatusLabel(to)
        const content = title.length > 0 ? title : `Suggested moving from ${fromLabel} to ${toLabel}`

        out.push({
          id: `s_${entry.entry_id}`,
          type: "task_status_suggestion",
          eventSource: "system",
          status: "done",
          content,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
          taskStatusSuggestion: { from, to, title, explanationMarkdown },
        })
        continue
      }

      const eventType = (() => {
        if (ev?.event_type === "task_created") return "task_created" as const
        if (ev?.event_type === "task_archived") return "task_archived" as const
        if (ev?.event_type === "task_status_changed") return "status_changed" as const
        return "status_changed" as const
      })()
      const content = (() => {
        if (ev?.event_type === "task_created") return "created the task"
        if (ev?.event_type === "task_archived") return "archived the task"
        if (ev?.event_type === "task_status_changed") {
          const from = taskStatusLabel(String(ev.from ?? ""))
          const to = taskStatusLabel(String(ev.to ?? ""))
          if (from && to) return `moved from ${from} to ${to}`
          if (to) return `changed status to ${to}`
          return "changed task status"
        }
        return "updated the task"
      })()

      out.push({
        id: `e_${entry.entry_id}`,
        type: "event",
        eventSource: "system",
        status: "done",
        eventType,
        content,
        timestamp: unixMsToIso(entry.created_at_unix_ms),
      })
      continue
    }

    if (entry.type === "user_event") {
      const ev = entry.event
      if (ev.type === "message") {
        lastUserEntryId = entry.entry_id || null
        out.push({
          id: `u_${entry.entry_id}`,
          type: "user",
          eventSource: "user",
          content: ev.text,
          attachments: ev.attachments,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        lastUserOutIndex = out.length - 1
        continue
      }

      if (ev.type === "terminal_command_started") {
        out.push({
          id: `tc_${ev.id}`,
          type: "terminal_command",
          eventSource: "user",
          content: ev.command,
          status: "running",
          timestamp: unixMsToIso(entry.created_at_unix_ms),
          terminalCommand: {
            id: ev.id,
            command: ev.command,
            reconnect: ev.reconnect,
            status: "running",
          },
        })
        terminalCommandIndexById.set(ev.id, out.length - 1)
        continue
      }

      if (ev.type === "terminal_command_finished") {
        const status: ActivityStatus = "done"
        const existingIndex = terminalCommandIndexById.get(ev.id)
        const hasOutput = (ev.output_byte_len ?? 0) > 0
        const timestamp = unixMsToIso(entry.created_at_unix_ms)

        const asSimpleEvent: Message = {
          id: `tc_ev_${ev.id}`,
          type: "event",
          eventSource: "user",
          status,
          content: `ran ${ev.command}`,
          timestamp,
        }

        if (typeof existingIndex === "number") {
          if (!hasOutput) {
            out[existingIndex] = asSimpleEvent
            continue
          }
          const prev = out[existingIndex]
          if (prev) {
            prev.type = "terminal_command"
            prev.eventSource = "user"
            prev.content = ev.command
            prev.status = status
            prev.timestamp = timestamp
            prev.terminalCommand = {
              id: ev.id,
              command: ev.command,
              reconnect: ev.reconnect,
              status,
              outputBase64: ev.output_base64,
              outputByteLen: ev.output_byte_len,
            }
          } else {
            out.push({
              id: `tc_${ev.id}`,
              type: "terminal_command",
              eventSource: "user",
              content: ev.command,
              status,
              timestamp,
              terminalCommand: {
                id: ev.id,
                command: ev.command,
                reconnect: ev.reconnect,
                status,
                outputBase64: ev.output_base64,
                outputByteLen: ev.output_byte_len,
              },
            })
            terminalCommandIndexById.set(ev.id, out.length - 1)
          }
          continue
        }

        if (!hasOutput) {
          out.push(asSimpleEvent)
          continue
        }

        out.push({
          id: `tc_${ev.id}`,
          type: "terminal_command",
          eventSource: "user",
          content: ev.command,
          status,
          timestamp,
          terminalCommand: {
            id: ev.id,
            command: ev.command,
            reconnect: ev.reconnect,
            status,
            outputBase64: ev.output_base64,
            outputByteLen: ev.output_byte_len,
          },
        })
        terminalCommandIndexById.set(ev.id, out.length - 1)
        continue
      }
      continue
    }

    if (entry.type === "agent_event") {
      const ev = entry.event
      // Reason: use per-entry runner so each message retains the icon
      // of the model that generated it, falling back to conversation-level.
      const entryRunner = entry.runner ?? conversation.agent_runner
      if (ev.type === "message") {
        const turnId = lastUserEntryId ? `t_${lastUserEntryId}` : `t_orphan_${ev.id}`
        const msg = ensureTurnMessage(turnId)
        const createdAtUnixMs = normalizeUnixMs(entry.created_at_unix_ms)
        const messageText = ev.text.trim()
        msg.agentRunner = entryRunner
        msg.content = messageText
        msg.timestamp = unixMsToIso(entry.created_at_unix_ms) ?? msg.timestamp

        const activityKey = `assistant_message_${entry.entry_id || out.length}`
        const rowId = activityKey
        appendTurnActivity(turnId, {
          key: activityKey,
          rowId,
          event: {
            id: rowId,
            type: "assistant_message",
            title: activityTitleFromMessageText(messageText),
            detail: messageText,
            status: "done",
            timing: { startedAtUnixMs: createdAtUnixMs, doneAtUnixMs: createdAtUnixMs },
          },
          createdAtUnixMs,
          priority: 100,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "item") {
        if (!lastUserEntryId) continue
        const turnId = `t_${lastUserEntryId}`
        const turnMsg = ensureTurnMessage(turnId)
        turnMsg.agentRunner = entryRunner
        const status = inferActivityStatusFromPayload(ev.payload)
        const activityKey = `item_${ev.id}`
        const rowId = activityKey
        const timing = ensureTurnActivityTiming(turnId, activityKey, status, normalizeUnixMs(entry.created_at_unix_ms))
        const activity = activityFromAgentItemLike({
          id: rowId,
          kind: String(ev.kind ?? "item"),
          payload: ev.payload,
          forcedStatus: status,
        })
        appendTurnActivity(turnId, {
          key: activityKey,
          rowId,
          event: { ...activity, timing },
          createdAtUnixMs: normalizeUnixMs(entry.created_at_unix_ms),
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "turn_duration") {
        if (!lastUserEntryId) continue
        const turnId = `t_${lastUserEntryId}`
        const turnMsg = ensureTurnMessage(turnId)
        turnMsg.agentRunner = entryRunner
        const activityKey = `turn_duration_${ev.duration_ms}`
        const rowId = `${activityKey}_${entry.entry_id || out.length}`
        const createdAtUnixMs = normalizeUnixMs(entry.created_at_unix_ms)
        appendTurnActivity(turnId, {
          key: activityKey,
          rowId,
          event: {
            id: rowId,
            type: "complete",
            title: `Turn duration: ${formatDurationMs(ev.duration_ms)}`,
            status: "done",
            timing: { startedAtUnixMs: createdAtUnixMs, doneAtUnixMs: createdAtUnixMs },
          },
          createdAtUnixMs,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "turn_usage") {
        if (!lastUserEntryId) continue
        const turnId = `t_${lastUserEntryId}`
        const turnMsg = ensureTurnMessage(turnId)
        turnMsg.agentRunner = entryRunner
        const activityKey = "turn_usage"
        const rowId = `${activityKey}_${entry.entry_id || out.length}`
        const createdAtUnixMs = normalizeUnixMs(entry.created_at_unix_ms)
        appendTurnActivity(turnId, {
          key: activityKey,
          rowId,
          event: {
            id: rowId,
            type: "tool_call",
            title: "Turn usage",
            detail: safeStringify(ev.usage_json ?? null),
            status: "done",
            timing: { startedAtUnixMs: createdAtUnixMs, doneAtUnixMs: createdAtUnixMs },
          },
          createdAtUnixMs,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "turn_error") {
        if (!lastUserEntryId) continue
        const turnId = `t_${lastUserEntryId}`
        const turnMsg = ensureTurnMessage(turnId)
        turnMsg.agentRunner = entryRunner
        applyTurnStatus(turnId, "error")
        const activityKey = "turn_error"
        const rowId = `${activityKey}_${entry.entry_id || out.length}`
        const createdAtUnixMs = normalizeUnixMs(entry.created_at_unix_ms)
        appendTurnActivity(turnId, {
          key: activityKey,
          rowId,
          event: {
            id: rowId,
            type: "tool_call",
            title: "Turn error",
            detail: ev.message,
            status: "done",
            timing: { startedAtUnixMs: createdAtUnixMs, doneAtUnixMs: createdAtUnixMs },
          },
          createdAtUnixMs,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "turn_canceled") {
        if (!lastUserEntryId) continue
        const turnId = `t_${lastUserEntryId}`
        const turnMsg = ensureTurnMessage(turnId)
        turnMsg.agentRunner = entryRunner
        applyTurnStatus(turnId, "canceled")
        const activityKey = "turn_canceled"
        const rowId = `${activityKey}_${entry.entry_id || out.length}`
        const createdAtUnixMs = normalizeUnixMs(entry.created_at_unix_ms)
        appendTurnActivity(turnId, {
          key: activityKey,
          rowId,
          event: {
            id: rowId,
            type: "tool_call",
            title: "Turn canceled",
            status: "done",
            timing: { startedAtUnixMs: createdAtUnixMs, doneAtUnixMs: createdAtUnixMs },
          },
          createdAtUnixMs,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }
    }
  }

  if (conversation.run_status === "running" && lastUserEntryId) {
    const turnId = `t_${lastUserEntryId}`
    ensureTurnMessage(turnId)
    applyTurnStatus(turnId, "running")
    const turn = turnMessageById.get(turnId) ?? null
    if (turn) turn.isStreaming = true
  } else {
    for (const turn of turnMessageById.values()) {
      if (turn.isStreaming) turn.isStreaming = false
    }
  }

  return out
}

function buildMessagesFlatEvents(conversation: ConversationSnapshot): Message[] {
  const out: Message[] = []

  const normalizeUnixMs = (unixMs: unknown): number | null => {
    if (typeof unixMs !== "number" || !Number.isFinite(unixMs) || unixMs <= 0) return null
    return unixMs
  }

  const unixMsToIso = (unixMs: unknown): string | undefined => {
    const normalized = normalizeUnixMs(unixMs)
    if (normalized == null) return undefined
    return new Date(normalized).toISOString()
  }

  const taskStatusLabel = (status: string): string => {
    switch (status) {
      case "backlog":
        return "Backlog"
      case "todo":
        return "Todo"
      case "iterating":
      case "in_progress":
        return "Iterating"
      case "validating":
      case "in_review":
        return "Validating"
      case "done":
        return "Done"
      case "canceled":
        return "Canceled"
      default:
        return status
    }
  }

  const agentEventIndexById = new Map<string, number>()
  const terminalCommandIndexById = new Map<string, number>()

  for (let i = 0; i < conversation.entries.length; i += 1) {
    const entry = conversation.entries[i]!
    if (entry.type === "system_event") {
      const ev = entry.event as any
      if (ev?.event_type === "task_status_suggestion") {
        const from = taskStatusLabel(String(ev.from ?? ""))
        const to = taskStatusLabel(String(ev.to ?? ""))
        out.push({
          id: `e_${entry.entry_id}`,
          type: "event",
          eventSource: "system",
          status: "done",
          content: `suggested moving from ${from} to ${to}`,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      const eventType = (() => {
        if (ev?.event_type === "task_created") return "task_created" as const
        if (ev?.event_type === "task_archived") return "task_archived" as const
        if (ev?.event_type === "task_status_changed") return "status_changed" as const
        return "status_changed" as const
      })()
      const content = (() => {
        if (ev?.event_type === "task_created") return "created the task"
        if (ev?.event_type === "task_archived") return "archived the task"
        if (ev?.event_type === "task_status_changed") {
          const from = taskStatusLabel(String(ev.from ?? ""))
          const to = taskStatusLabel(String(ev.to ?? ""))
          if (from && to) return `moved from ${from} to ${to}`
          if (to) return `changed status to ${to}`
          return "changed task status"
        }
        return "updated the task"
      })()

      out.push({
        id: `e_${entry.entry_id}`,
        type: "event",
        eventSource: "system",
        status: "done",
        eventType,
        content,
        timestamp: unixMsToIso(entry.created_at_unix_ms),
      })
      continue
    }

    if (entry.type === "user_event") {
      const ev = entry.event
      if (ev.type === "message") {
        out.push({
          id: `u_${entry.entry_id}`,
          type: "user",
          eventSource: "user",
          content: ev.text,
          attachments: ev.attachments,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "terminal_command_started") {
        out.push({
          id: `tc_${ev.id}`,
          type: "terminal_command",
          eventSource: "user",
          content: ev.command,
          status: "running",
          timestamp: unixMsToIso(entry.created_at_unix_ms),
          terminalCommand: {
            id: ev.id,
            command: ev.command,
            reconnect: ev.reconnect,
            status: "running",
          },
        })
        terminalCommandIndexById.set(ev.id, out.length - 1)
        continue
      }

      if (ev.type === "terminal_command_finished") {
        const status: ActivityStatus = "done"
        const existingIndex = terminalCommandIndexById.get(ev.id)
        const hasOutput = (ev.output_byte_len ?? 0) > 0
        const timestamp = unixMsToIso(entry.created_at_unix_ms)

        const asSimpleEvent: Message = {
          id: `tc_ev_${ev.id}`,
          type: "event",
          eventSource: "user",
          status,
          content: `ran ${ev.command}`,
          timestamp,
        }

        if (typeof existingIndex === "number") {
          if (!hasOutput) {
            out[existingIndex] = asSimpleEvent
            continue
          }
          const prev = out[existingIndex]
          if (prev) {
            prev.type = "terminal_command"
            prev.eventSource = "user"
            prev.content = ev.command
            prev.status = status
            prev.timestamp = timestamp
            prev.terminalCommand = {
              id: ev.id,
              command: ev.command,
              reconnect: ev.reconnect,
              status,
              outputBase64: ev.output_base64,
              outputByteLen: ev.output_byte_len,
            }
          } else {
            out.push({
              id: `tc_${ev.id}`,
              type: "terminal_command",
              eventSource: "user",
              content: ev.command,
              status,
              timestamp,
              terminalCommand: {
                id: ev.id,
                command: ev.command,
                reconnect: ev.reconnect,
                status,
                outputBase64: ev.output_base64,
                outputByteLen: ev.output_byte_len,
              },
            })
            terminalCommandIndexById.set(ev.id, out.length - 1)
          }
          continue
        }

        if (!hasOutput) {
          out.push(asSimpleEvent)
          continue
        }

        out.push({
          id: `tc_${ev.id}`,
          type: "terminal_command",
          eventSource: "user",
          content: ev.command,
          status,
          timestamp,
          terminalCommand: {
            id: ev.id,
            command: ev.command,
            reconnect: ev.reconnect,
            status,
            outputBase64: ev.output_base64,
            outputByteLen: ev.output_byte_len,
          },
        })
        terminalCommandIndexById.set(ev.id, out.length - 1)
        continue
      }
      continue
    }

    if (entry.type === "agent_event") {
      const ev = entry.event
      const entryRunner = entry.runner ?? conversation.agent_runner
      if (ev.type === "message") {
        const entryPart = entry.entry_id ? `_${entry.entry_id}` : `_${out.length}`
        out.push({
          id: `a_${ev.id}${entryPart}`,
          type: "assistant",
          eventSource: "agent",
          agentRunner: entryRunner,
          content: ev.text.trim(),
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "item") {
        const activity = activityFromAgentItem(ev)
        const status = inferActivityStatusFromPayload(ev.payload)
        const baseId = `ae_${ev.id}`
        const existing = agentEventIndexById.get(baseId)
        if (typeof existing === "number") {
          const prev = out[existing]
          if (prev && prev.type === "event") {
            prev.status = status
            prev.content = activity.title
          }
        } else {
          out.push({
            id: baseId,
            type: "event",
            eventSource: "agent",
            agentRunner: entryRunner,
            status,
            content: activity.title,
            timestamp: unixMsToIso(entry.created_at_unix_ms),
          })
          agentEventIndexById.set(baseId, out.length - 1)
        }
        continue
      }

      if (ev.type === "turn_duration") {
        out.push({
          id: `ae_turn_duration_${out.length}_${ev.duration_ms}`,
          type: "event",
          eventSource: "agent",
          agentRunner: entryRunner,
          status: "done",
          content: `Turn duration: ${formatDurationMs(ev.duration_ms)}`,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "turn_usage") {
        out.push({
          id: `ae_turn_usage_${out.length}`,
          type: "event",
          eventSource: "agent",
          agentRunner: entryRunner,
          status: "done",
          content: "Turn usage",
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "turn_error") {
        out.push({
          id: `ae_turn_error_${out.length}`,
          type: "event",
          eventSource: "agent",
          agentRunner: entryRunner,
          status: "done",
          content: `Turn error: ${ev.message}`,
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }

      if (ev.type === "turn_canceled") {
        out.push({
          id: `ae_turn_canceled_${out.length}`,
          type: "event",
          eventSource: "agent",
          agentRunner: entryRunner,
          status: "done",
          content: "Turn canceled",
          timestamp: unixMsToIso(entry.created_at_unix_ms),
        })
        continue
      }
    }
  }

  if (conversation.run_status === "running") {
    for (let i = out.length - 1; i >= 0; i -= 1) {
      const msg = out[i]
      if (msg && msg.type === "assistant") {
        msg.isStreaming = true
        break
      }
    }
  }

  return out
}
