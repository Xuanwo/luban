"use client"

import type {
  AttachmentRef,
  AgentEvent,
  ConversationEntry,
  ConversationSnapshot,
  ThinkingEffort,
} from "./luban-api"
import { AGENT_MODELS } from "./agent-settings"

export interface ActivityEvent {
  id: string
  type: "thinking" | "tool_call" | "file_edit" | "bash" | "search" | "complete"
  title: string
  detail?: string
  status: "running" | "done"
  duration?: string
  badge?: string
}

type ActivityStatus = ActivityEvent["status"]

export interface SystemEvent {
  id: string
  type: "event"
  eventType: "task_created" | "task_started" | "task_completed" | "task_cancelled" | "status_changed"
  title: string
  timestamp?: string
}

export interface Message {
  id: string
  type: "user" | "assistant" | "event"
  eventSource?: "system" | "user" | "agent"
  content: string
  attachments?: AttachmentRef[]
  timestamp?: string
  isStreaming?: boolean
  isCancelled?: boolean
  status?: ActivityStatus
  activities?: ActivityEvent[]
  metadata?: {
    toolCalls?: number
    thinkingSteps?: number
    duration?: string
  }
  codeReferences?: { file: string; line: number }[]
  eventType?: "task_created" | "task_started" | "task_completed" | "task_cancelled" | "status_changed"
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

export function formatDurationMs(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000))
  const s = seconds % 60
  const minutes = Math.floor(seconds / 60)
  const m = minutes % 60
  const hours = Math.floor(minutes / 60)
  if (hours > 0) return `${hours}h${m}m${s}s`
  if (minutes > 0) return `${minutes}m${s}s`
  return `${s}s`
}

export function agentModelLabel(modelId: string | null | undefined): string {
  if (!modelId) return "Model"
  return AGENT_MODELS.find((m) => m.id === modelId)?.label ?? modelId
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
    const title = `${payload?.server ?? "mcp"}.${payload?.tool ?? "tool"}`
    const detail = safeStringify({
      arguments: payload?.arguments ?? null,
      result: payload?.result ?? null,
      error: payload?.error ?? null,
      status: payload?.status ?? null,
    })
    return { id: args.id, type: "tool_call", title, detail, status }
  }

  if (kind === "web_search") {
    return {
      id: args.id,
      type: "search",
      title: payload?.query ?? "Web search",
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

export function buildAgentActivities(conversation: ConversationSnapshot | null): ActivityEvent[] {
  if (!conversation) return []

  const order: string[] = []
  const latestById = new Map<string, ActivityEvent>()

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
    const ev = entry.event
    if (ev.type === "item") {
      const base = activityFromAgentItem(ev)
      upsert({ ...base, status: inferActivityStatusFromPayload(ev.payload) })
      continue
    }
    if (ev.type === "turn_duration") {
      upsert({
        id: `turn_duration_${ev.duration_ms}`,
        type: "complete",
        title: `Turn duration: ${formatDurationMs(ev.duration_ms)}`,
        status: "done",
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
      })
      continue
    }
    if (ev.type === "turn_canceled") {
      upsert({
        id: "turn_canceled",
        type: "tool_call",
        title: "Turn canceled",
        status: "done",
      })
      continue
    }
  }

  return order.map((id) => latestById.get(id)!).filter(Boolean)
}

export function buildMessages(conversation: ConversationSnapshot | null): Message[] {
  if (!conversation) return []

  const out: Message[] = []
  const tailStart = Math.max(0, conversation.entries.length - 3)

  const unixMsToIso = (unixMs: number | null | undefined): string | undefined => {
    if (typeof unixMs !== "number" || !Number.isFinite(unixMs)) return undefined
    return new Date(unixMs).toISOString()
  }

  const taskStatusLabel = (status: string): string => {
    switch (status) {
      case "backlog":
        return "Backlog"
      case "todo":
        return "Todo"
      case "in_progress":
        return "In Progress"
      case "in_review":
        return "In Review"
      case "done":
        return "Done"
      case "canceled":
        return "Canceled"
      default:
        return status
    }
  }

  const agentMessageIndexById = new Map<string, number>()
  const agentEventIndexById = new Map<string, number>()

  for (let i = 0; i < conversation.entries.length; i += 1) {
    const entry = conversation.entries[i]!
    const isInTail = i >= tailStart
    if (entry.type === "system_event") {
      const ev = entry.event as any
      const eventType = (() => {
        if (ev?.event_type === "task_created") return "task_created" as const
        if (ev?.event_type === "task_status_changed") return "status_changed" as const
        return "status_changed" as const
      })()
      const content = (() => {
        if (ev?.event_type === "task_created") return "created the task"
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
      if (entry.event.type === "message") {
        out.push({
          id: `u_${entry.entry_id}`,
          type: "user",
          eventSource: "user",
          content: entry.event.text,
          attachments: entry.event.attachments,
          timestamp: new Date().toISOString(),
        })
      }
      continue
    }

    if (entry.type === "agent_event") {
      const ev = entry.event
      if (ev.type === "message") {
        const baseId = `a_${ev.id}`
        if (isInTail) {
          out.push({
            id: `${baseId}_${entry.entry_id}`,
            type: "assistant",
            eventSource: "agent",
            content: ev.text.trim(),
            timestamp: new Date().toISOString(),
          })
        } else {
          const existing = agentMessageIndexById.get(baseId)
          if (typeof existing === "number") {
            const prev = out[existing]
            if (prev && prev.type === "assistant") {
              prev.content = ev.text.trim()
            }
          } else {
            out.push({
              id: baseId,
              type: "assistant",
              eventSource: "agent",
              content: ev.text.trim(),
              timestamp: new Date().toISOString(),
            })
            agentMessageIndexById.set(baseId, out.length - 1)
          }
        }
        continue
      }

      if (ev.type === "item") {
        const activity = activityFromAgentItem(ev)
        const status = inferActivityStatusFromPayload(ev.payload)
        const baseId = `ae_${ev.id}`
        if (isInTail) {
          out.push({
            id: `${baseId}_${entry.entry_id}`,
            type: "event",
            eventSource: "agent",
            status,
            content: activity.title,
            timestamp: new Date().toISOString(),
          })
        } else {
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
              status,
              content: activity.title,
              timestamp: new Date().toISOString(),
            })
            agentEventIndexById.set(baseId, out.length - 1)
          }
        }
        continue
      }

      if (ev.type === "turn_duration") {
        out.push({
          id: `ae_turn_duration_${out.length}_${ev.duration_ms}`,
          type: "event",
          eventSource: "agent",
          status: "done",
          content: `Turn duration: ${formatDurationMs(ev.duration_ms)}`,
          timestamp: new Date().toISOString(),
        })
        continue
      }

      if (ev.type === "turn_usage") {
        out.push({
          id: `ae_turn_usage_${out.length}`,
          type: "event",
          eventSource: "agent",
          status: "done",
          content: "Turn usage",
          timestamp: new Date().toISOString(),
        })
        continue
      }

      if (ev.type === "turn_error") {
        out.push({
          id: `ae_turn_error_${out.length}`,
          type: "event",
          eventSource: "agent",
          status: "done",
          content: `Turn error: ${ev.message}`,
          timestamp: new Date().toISOString(),
        })
        continue
      }

      if (ev.type === "turn_canceled") {
        out.push({
          id: `ae_turn_canceled_${out.length}`,
          type: "event",
          eventSource: "agent",
          status: "done",
          content: "Turn canceled",
          timestamp: new Date().toISOString(),
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
