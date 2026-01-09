"use client"

import type React from "react"

import { useEffect, useMemo, useRef, useState } from "react"
import {
  Send,
  ChevronDown,
  ChevronRight,
  Copy,
  Clock,
  Wrench,
  Brain,
  FileCode,
  ArrowDown,
  Settings2,
  MessageSquare,
  Plus,
  X,
  ExternalLink,
  GitBranch,
  RotateCcw,
  Terminal,
  Eye,
  Pencil,
  CheckCircle2,
  Loader2,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { useLuban } from "@/lib/luban-context"
import type { ConversationEntry, ConversationSnapshot, ThinkingEffort } from "@/lib/luban-api"
import { Markdown } from "@/components/markdown"

function loadJson<T>(key: string): T | null {
  const raw = localStorage.getItem(key)
  if (!raw) return null
  try {
    return JSON.parse(raw) as T
  } catch {
    return null
  }
}

function saveJson(key: string, value: unknown) {
  localStorage.setItem(key, JSON.stringify(value))
}

function draftKeyForThread(workspaceId: number, threadId: number) {
  return `luban:draft:${workspaceId}:${threadId}`
}

function followTailKeyForThread(workspaceId: number, threadId: number) {
  return `luban:follow_tail:${workspaceId}:${threadId}`
}

function threadOrderKeyForWorkspace(workspaceId: number) {
  return `luban:ui:thread_order:${workspaceId}`
}

function arraysEqual<T>(a: T[], b: T[]): boolean {
  if (a === b) return true
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false
  }
  return true
}

interface ChatTab {
  id: string
  title: string
  isActive: boolean
}

interface ClosedTab {
  id: string
  title: string
  closedAt: Date
}

interface ActivityEvent {
  id: string
  type: "thinking" | "tool_call" | "file_edit" | "bash" | "search" | "complete"
  title: string
  detail?: string
  status: "running" | "done"
  duration?: string
}

interface Message {
  id: string
  type: "user" | "assistant"
  content: string
  timestamp?: string
  isStreaming?: boolean
  activities?: ActivityEvent[]
  metadata?: {
    toolCalls?: number
    thinkingSteps?: number
    duration?: string
  }
  codeReferences?: { file: string; line: number }[]
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function activityFromAgentItemLike(args: {
  id: string
  kind: string
  payload: unknown
  forcedStatus?: "running" | "done"
}): ActivityEvent {
  const payload = args.payload as any
  const kind = args.kind
  const forcedStatus = args.forcedStatus

  if (kind === "command_execution") {
    const status = forcedStatus ?? (payload?.status === "in_progress" ? "running" : "done")
    return {
      id: args.id,
      type: "bash",
      title: payload?.command ?? "Command",
      detail: payload?.aggregated_output ?? "",
      status,
    }
  }

  if (kind === "file_change") {
    const status = forcedStatus ?? "done"
    const changes = Array.isArray(payload?.changes) ? payload.changes : []
    const detail = changes.map((c: any) => `${c.kind ?? "update"} ${c.path ?? ""}`).join("\n")
    return {
      id: args.id,
      type: "file_edit",
      title: `File changes (${changes.length})`,
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
    return {
      id: args.id,
      type: "thinking",
      title: "Reasoning",
      detail: payload?.text ?? "",
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

function activityFromAgentItem(entry: Extract<ConversationEntry, { type: "agent_item" }>): ActivityEvent {
  return activityFromAgentItemLike({ id: entry.id, kind: entry.kind, payload: entry.payload })
}

function buildMessages(conversation: ConversationSnapshot | null): Message[] {
  if (!conversation) return []

  const out: Message[] = []
  let assistantContent = ""
  let assistantActivities: ActivityEvent[] = []
  let assistantToolCalls = 0
  let assistantThinkingSteps = 0
  let assistantDurationMs: number | null = null

  function flushAssistant() {
    if (assistantContent.trim().length === 0 && assistantActivities.length === 0) return
    const metadata =
      assistantToolCalls > 0 || assistantThinkingSteps > 0 || assistantDurationMs != null
        ? {
            toolCalls: assistantToolCalls > 0 ? assistantToolCalls : undefined,
            thinkingSteps: assistantThinkingSteps > 0 ? assistantThinkingSteps : undefined,
            duration:
              assistantDurationMs != null ? formatDurationMs(assistantDurationMs) : undefined,
          }
        : undefined
    out.push({
      id: `a_${out.length}`,
      type: "assistant",
      content: assistantContent.trim(),
      activities: assistantActivities.length > 0 ? assistantActivities : undefined,
      metadata,
    })
    assistantContent = ""
    assistantActivities = []
    assistantToolCalls = 0
    assistantThinkingSteps = 0
    assistantDurationMs = null
  }

  for (const entry of conversation.entries) {
    if (entry.type === "user_message") {
      flushAssistant()
      out.push({
        id: `u_${out.length}`,
        type: "user",
        content: entry.text,
      })
      continue
    }

    if (entry.type === "agent_item") {
      if (entry.kind === "agent_message") {
        const payload = entry.payload as any
        const text = typeof payload?.text === "string" ? payload.text : ""
        assistantContent = assistantContent.length === 0 ? text : `${assistantContent}\n\n${text}`
      } else {
        assistantActivities.push(activityFromAgentItem(entry))
        if (entry.kind === "reasoning") {
          assistantThinkingSteps += 1
        } else if (entry.kind !== "error") {
          assistantToolCalls += 1
        }
      }
      continue
    }

    if (entry.type === "turn_duration") {
      assistantDurationMs = entry.duration_ms
      continue
    }

    if (entry.type === "turn_usage") {
      continue
    }

    if (entry.type === "turn_error") {
      assistantActivities.push({
        id: `turn_error_${out.length}`,
        type: "tool_call",
        title: "Turn error",
        detail: entry.message,
        status: "done",
      })
      continue
    }

    if (entry.type === "turn_canceled") {
      assistantActivities.push({
        id: `turn_canceled_${out.length}`,
        type: "tool_call",
        title: "Turn canceled",
        status: "done",
      })
      continue
    }
  }

  if (conversation.run_status === "running" && conversation.in_progress_items.length > 0) {
    for (const item of conversation.in_progress_items) {
      assistantActivities.push(
        activityFromAgentItemLike({
          id: item.id,
          kind: item.kind,
          payload: item.payload,
          forcedStatus: "running",
        }),
      )
    }
  }

  if (conversation.run_status === "running" && !assistantActivities.some((a) => a.status === "running")) {
    assistantActivities.push({
      id: "synthetic_running",
      type: "thinking",
      title: "Running...",
      status: "running",
    })
  }

  flushAssistant()

  if (conversation.run_status === "running") {
    const last = out[out.length - 1]
    if (last && last.type === "assistant") {
      last.isStreaming = true
    }
  }
  return out
}

function formatDurationMs(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000))
  const s = seconds % 60
  const minutes = Math.floor(seconds / 60)
  const m = minutes % 60
  const hours = Math.floor(minutes / 60)
  if (hours > 0) return `${hours}h${m}m${s}s`
  if (minutes > 0) return `${minutes}m${s}s`
  return `${s}s`
}

function agentModelLabel(modelId: string | null | undefined): string {
  if (!modelId) return "Model"
  if (modelId === "gpt-5.2") return "GPT-5.2"
  if (modelId === "gpt-5.2-codex") return "GPT-5.2-Codex"
  if (modelId === "gpt-5.1-codex-max") return "GPT-5.1-Codex-Max"
  return modelId
}

function thinkingEffortLabel(effort: ThinkingEffort | null | undefined): string {
  if (!effort) return "Effort"
  if (effort === "low") return "Low"
  if (effort === "medium") return "Medium"
  if (effort === "high") return "High"
  if (effort === "xhigh") return "XHigh"
  return effort
}

async function copyToClipboard(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    const el = document.createElement("textarea")
    el.value = text
    el.style.position = "fixed"
    el.style.opacity = "0"
    document.body.appendChild(el)
    el.focus()
    el.select()
    document.execCommand("copy")
    document.body.removeChild(el)
  }
}

function ActivityEventItem({
  event,
  isExpanded,
  onToggle,
}: { event: ActivityEvent; isExpanded: boolean; onToggle: () => void }) {
  const getEventIcon = () => {
    switch (event.type) {
      case "thinking":
        return <Brain className="w-3.5 h-3.5" />
      case "tool_call":
        return <Wrench className="w-3.5 h-3.5" />
      case "file_edit":
        return <Pencil className="w-3.5 h-3.5" />
      case "bash":
        return <Terminal className="w-3.5 h-3.5" />
      case "search":
        return <Eye className="w-3.5 h-3.5" />
      case "complete":
        return <CheckCircle2 className="w-3.5 h-3.5" />
      default:
        return <Wrench className="w-3.5 h-3.5" />
    }
  }

  return (
    <div className="group">
      <button
        onClick={onToggle}
        className={cn(
          "w-full flex items-center gap-2 py-1 px-2 -mx-2 rounded text-xs transition-colors",
          "hover:bg-muted/50",
          event.status === "running" ? "text-primary" : "text-muted-foreground",
        )}
      >
        {event.status === "running" ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : getEventIcon()}
        <span className="flex-1 text-left truncate">{event.title}</span>
        {event.duration && <span className="text-[10px] text-muted-foreground/70">{event.duration}</span>}
        {event.detail && (
          <ChevronRight
            className={cn("w-3 h-3 text-muted-foreground/50 transition-transform", isExpanded && "rotate-90")}
          />
        )}
      </button>
      {isExpanded && event.detail && (
        <div className="ml-5 pl-2 border-l border-border text-[11px] text-muted-foreground py-1 mb-1">
          {event.detail}
        </div>
      )}
    </div>
  )
}

function ActivityStream({ activities, isStreaming }: { activities: ActivityEvent[]; isStreaming?: boolean }) {
  const [isExpanded, setIsExpanded] = useState(false)
  const [expandedEvents, setExpandedEvents] = useState<Set<string>>(new Set())

  const latestActivity = activities[activities.length - 1]
  const completedCount = activities.filter((a) => a.status === "done").length

  const toggleEvent = (eventId: string) => {
    const newExpanded = new Set(expandedEvents)
    if (newExpanded.has(eventId)) {
      newExpanded.delete(eventId)
    } else {
      newExpanded.add(eventId)
    }
    setExpandedEvents(newExpanded)
  }

  if (!activities.length) return null

  return (
    <div className="my-3">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={cn(
          "flex items-center gap-2 py-1.5 px-2 -mx-2 rounded text-xs transition-colors w-full",
          "hover:bg-muted/50",
          isStreaming ? "text-primary" : "text-muted-foreground",
        )}
      >
        {isStreaming && latestActivity?.status === "running" ? (
          <Loader2 className="w-3.5 h-3.5 animate-spin flex-shrink-0" />
        ) : (
          <CheckCircle2 className="w-3.5 h-3.5 flex-shrink-0 text-green-500" />
        )}
        <span className="flex-1 text-left truncate">
          {isStreaming ? latestActivity?.title : `Completed ${completedCount} steps`}
        </span>
        <ChevronDown
          className={cn("w-3.5 h-3.5 text-muted-foreground/50 transition-transform", isExpanded && "rotate-180")}
        />
      </button>

      {isExpanded && (
        <div className="mt-1 ml-1 pl-3 border-l-2 border-border/50 space-y-0.5">
          {activities.map((event) => (
            <ActivityEventItem
              key={event.id}
              event={event}
              isExpanded={expandedEvents.has(event.id)}
              onToggle={() => toggleEvent(event.id)}
            />
          ))}
        </div>
      )}
    </div>
  )
}

export function ChatPanel() {
  const [showTabDropdown, setShowTabDropdown] = useState(false)

  const scrollContainerRef = useRef<HTMLDivElement | null>(null)

  const {
    app,
    activeWorkspaceId,
    activeThreadId,
    threads,
    conversation,
    selectThread,
    createThread,
    sendAgentMessage,
  } = useLuban()

  const [draftText, setDraftText] = useState("")
  const [hiddenThreadIds, setHiddenThreadIds] = useState<number[]>([])
  const [threadOrderIds, setThreadOrderIds] = useState<number[]>([])
  const [closedAtByThreadId, setClosedAtByThreadId] = useState<Record<number, number>>({})
  const [followTail, setFollowTail] = useState(true)
  const programmaticScrollRef = useRef(false)

  const messages = useMemo(() => buildMessages(conversation), [conversation])
  const modelLabel = useMemo(() => agentModelLabel(conversation?.agent_model_id), [conversation?.agent_model_id])
  const effortLabel = useMemo(
    () => thinkingEffortLabel(conversation?.thinking_effort),
    [conversation?.thinking_effort],
  )

  const projectInfo = useMemo(() => {
    if (app == null || activeWorkspaceId == null) return { name: "Luban", branch: "" }
    for (const p of app.projects) {
      for (const w of p.workspaces) {
        if (w.id !== activeWorkspaceId) continue
        return { name: p.slug, branch: w.branch_name }
      }
    }
    return { name: "Luban", branch: "" }
  }, [app, activeWorkspaceId])

  const threadsById = useMemo(() => {
    const out = new Map<number, (typeof threads)[number]>()
    for (const t of threads) out.set(t.thread_id, t)
    return out
  }, [threads])

  const hiddenThreadIdSet = useMemo(() => new Set(hiddenThreadIds), [hiddenThreadIds])

  const openThreadIds = useMemo(() => {
    if (threads.length === 0) return []
    if (threadOrderIds.length === 0) {
      return threads.map((t) => t.thread_id).filter((id) => !hiddenThreadIdSet.has(id))
    }
    return threadOrderIds.filter((id) => !hiddenThreadIdSet.has(id))
  }, [threads, threadOrderIds, hiddenThreadIdSet])

  const openThreads = useMemo(() => {
    const out: (typeof threads)[number][] = []
    for (const id of openThreadIds) {
      const t = threadsById.get(id)
      if (t) out.push(t)
    }
    return out
  }, [openThreadIds, threadsById])

  const closedThreads = useMemo(
    () =>
      threads
        .filter((t) => hiddenThreadIdSet.has(t.thread_id))
        .sort((a, b) => (closedAtByThreadId[b.thread_id] ?? 0) - (closedAtByThreadId[a.thread_id] ?? 0)),
    [threads, hiddenThreadIdSet, closedAtByThreadId],
  )

  const tabs: ChatTab[] = useMemo(
    () =>
      openThreads.map((t) => ({
        id: String(t.thread_id),
        title: t.title,
        isActive: t.thread_id === activeThreadId,
      })),
    [openThreads, activeThreadId],
  )

  const activeTabId = activeThreadId != null ? String(activeThreadId) : ""

  const closedTabs: ClosedTab[] = useMemo(
    () =>
      closedThreads.map((t) => ({
        id: String(t.thread_id),
        title: t.title,
        closedAt: new Date(closedAtByThreadId[t.thread_id] ?? Date.now()),
      })),
    [closedThreads, closedAtByThreadId],
  )

  useEffect(() => {
    if (activeWorkspaceId == null) {
      setHiddenThreadIds([])
      setClosedAtByThreadId({})
      setThreadOrderIds([])
      return
    }
    const hidden = loadJson<number[]>(`luban:ui:hidden_threads:${activeWorkspaceId}`) ?? []
    setHiddenThreadIds(Array.isArray(hidden) ? hidden.filter((x) => Number.isFinite(x)) : [])
    const closedAt = loadJson<Record<string, number>>(`luban:ui:closed_at:${activeWorkspaceId}`) ?? {}
    const parsed: Record<number, number> = {}
    for (const [k, v] of Object.entries(closedAt)) {
      const id = Number(k)
      if (!Number.isFinite(id) || !Number.isFinite(v)) continue
      parsed[id] = v
    }
    setClosedAtByThreadId(parsed)

    const storedOrder = loadJson<number[]>(threadOrderKeyForWorkspace(activeWorkspaceId)) ?? []
    const nextOrder: number[] = []
    const seen = new Set<number>()
    for (const raw of storedOrder) {
      if (!Number.isFinite(raw)) continue
      const id = Number(raw)
      if (!Number.isFinite(id) || seen.has(id)) continue
      nextOrder.push(id)
      seen.add(id)
    }
    setThreadOrderIds(nextOrder)
  }, [activeWorkspaceId])

  useEffect(() => {
    if (activeWorkspaceId == null) return
    if (threads.length === 0) return

    const hidden = new Set(hiddenThreadIds)

    const presentIds = threads.map((t) => t.thread_id)
    const presentSet = new Set(presentIds)

    let next = threadOrderIds.filter((id) => presentSet.has(id) && !hidden.has(id))
    if (next.length === 0) {
      next = presentIds.filter((id) => !hidden.has(id))
    } else {
      const seen = new Set(next)
      for (const id of presentIds) {
        if (hidden.has(id)) continue
        if (seen.has(id)) continue
        next.push(id)
        seen.add(id)
      }
    }

    if (arraysEqual(next, threadOrderIds)) return

    setThreadOrderIds(next)
    saveJson(threadOrderKeyForWorkspace(activeWorkspaceId), next)
  }, [activeWorkspaceId, threads, hiddenThreadIds, threadOrderIds])

  useEffect(() => {
    if (activeWorkspaceId == null || activeThreadId == null) {
      setDraftText("")
      return
    }

    setFollowTail(true)
    localStorage.setItem(followTailKeyForThread(activeWorkspaceId, activeThreadId), "true")

    const saved = loadJson<{ text: string }>(draftKeyForThread(activeWorkspaceId, activeThreadId))
    setDraftText(saved?.text ?? "")
  }, [activeWorkspaceId, activeThreadId])

  function scheduleScrollToBottom() {
    const el = scrollContainerRef.current
    if (!el) return

    programmaticScrollRef.current = true
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        el.scrollTop = el.scrollHeight
        programmaticScrollRef.current = false
      })
    })
  }

  useEffect(() => {
    if (!followTail) return
    if (messages.length === 0) return
    scheduleScrollToBottom()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [messages.length, followTail, activeWorkspaceId, activeThreadId])

  function persistDraft(nextText: string) {
    if (activeWorkspaceId == null || activeThreadId == null) return
    saveJson(draftKeyForThread(activeWorkspaceId, activeThreadId), {
      text: nextText,
    })
  }

  const handleTabClick = (tabId: string) => {
    const id = Number(tabId)
    if (!Number.isFinite(id)) return
    void selectThread(id)
  }

  function closeThread(threadId: number) {
    if (activeWorkspaceId == null) return
    const openIds = openThreadIds
    if (openIds.length <= 1) return

    const nextHidden = Array.from(new Set([...hiddenThreadIds, threadId]))
    const closedAt = { ...closedAtByThreadId, [threadId]: Date.now() }
    setHiddenThreadIds(nextHidden)
    setClosedAtByThreadId(closedAt)
    saveJson(`luban:ui:hidden_threads:${activeWorkspaceId}`, nextHidden)
    saveJson(`luban:ui:closed_at:${activeWorkspaceId}`, Object.fromEntries(Object.entries(closedAt)))

    const nextOrder = threadOrderIds.filter((id) => id !== threadId)
    if (!arraysEqual(nextOrder, threadOrderIds)) {
      setThreadOrderIds(nextOrder)
      saveJson(threadOrderKeyForWorkspace(activeWorkspaceId), nextOrder)
    }

    if (activeThreadId === threadId) {
      const next = nextOrder[0] ?? openIds.find((id) => id !== threadId) ?? null
      if (next != null) void selectThread(next)
    }
  }

  const handleCloseTab = (tabId: string, e: React.MouseEvent) => {
    e.stopPropagation()
    const id = Number(tabId)
    if (!Number.isFinite(id)) return
    closeThread(id)
  }

  const handleAddTab = () => {
    if (activeWorkspaceId == null) return
    createThread()
  }

  const handleRestoreTab = (closedTab: ClosedTab) => {
    if (activeWorkspaceId == null) return
    const id = Number(closedTab.id)
    if (!Number.isFinite(id)) return
    const nextHidden = hiddenThreadIds.filter((x) => x !== id)
    setHiddenThreadIds(nextHidden)
    saveJson(`luban:ui:hidden_threads:${activeWorkspaceId}`, nextHidden)

    const nextOrder = [...threadOrderIds.filter((x) => x !== id), id]
    setThreadOrderIds(nextOrder)
    saveJson(threadOrderKeyForWorkspace(activeWorkspaceId), nextOrder)

    setShowTabDropdown(false)
    void selectThread(id)
  }

  const handleSend = () => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    const text = draftText.trim()
    if (text.length === 0) return
    sendAgentMessage(text)
    setDraftText("")
    persistDraft("")
    setFollowTail(true)
    localStorage.setItem(followTailKeyForThread(activeWorkspaceId, activeThreadId), "true")
    scheduleScrollToBottom()
  }

  return (
    <div className="flex-1 flex flex-col min-w-0 bg-background">
      <div className="flex items-center h-11 border-b border-border bg-card px-4">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-sm font-medium text-foreground truncate">{projectInfo.name}</span>
          <div className="flex items-center gap-1 text-muted-foreground">
            <GitBranch className="w-3.5 h-3.5" />
            <span className="text-xs">{projectInfo.branch}</span>
          </div>
          <button
            className="p-1.5 text-muted-foreground hover:text-foreground hover:bg-muted rounded transition-colors"
            title="Open in editor"
          >
            <ExternalLink className="w-4 h-4" />
          </button>
        </div>
      </div>

      <div className="flex items-center h-10 border-b border-border bg-card/50">
        <div className="flex-1 flex items-center min-w-0 overflow-x-auto scrollbar-none">
          {tabs.map((tab) => (
            <div
              key={tab.id}
              onClick={() => handleTabClick(tab.id)}
              className={cn(
                "group flex items-center gap-2 h-10 px-3 border-r border-border cursor-pointer transition-colors min-w-0 max-w-[180px]",
                tab.id === activeTabId
                  ? "bg-background text-foreground"
                  : "text-muted-foreground hover:text-foreground hover:bg-muted/50",
              )}
            >
              <MessageSquare className="w-3.5 h-3.5 flex-shrink-0" />
              <span data-testid="thread-tab-title" className="text-xs truncate flex-1">
                {tab.title}
              </span>
              {tabs.length > 1 && (
                <button
                  onClick={(e) => handleCloseTab(tab.id, e)}
                  className="p-0.5 opacity-0 group-hover:opacity-100 hover:bg-muted rounded transition-all"
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>
          ))}
        </div>

        <div className="flex items-center border-l border-border">
          <button
            onClick={handleAddTab}
            className="flex items-center justify-center w-9 h-10 text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
            title="New tab"
          >
            <Plus className="w-4 h-4" />
          </button>

          <div className="relative">
            <button
              onClick={() => setShowTabDropdown(!showTabDropdown)}
              className={cn(
                "flex items-center justify-center w-9 h-10 text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors",
                showTabDropdown && "bg-muted text-foreground",
              )}
              title="All tabs"
            >
              <ChevronDown className="w-4 h-4" />
            </button>

            {showTabDropdown && (
              <>
                <div className="fixed inset-0 z-40" onClick={() => setShowTabDropdown(false)} />
                <div className="absolute right-0 top-full mt-1 w-64 bg-card border border-border rounded-lg shadow-xl z-50 overflow-hidden">
                  <div className="p-2 border-b border-border">
                    <span className="text-[10px] uppercase tracking-wider text-muted-foreground font-medium px-2">
                      Open Tabs
                    </span>
                  </div>
                  <div className="max-h-40 overflow-y-auto">
                    {tabs.map((tab) => (
                      <button
                        key={tab.id}
                        onClick={() => {
                          handleTabClick(tab.id)
                          setShowTabDropdown(false)
                        }}
                        className={cn(
                          "w-full flex items-center gap-2 px-3 py-2 text-left text-xs hover:bg-muted transition-colors",
                          tab.id === activeTabId && "bg-primary/10 text-primary",
                        )}
                      >
                        <MessageSquare className="w-3.5 h-3.5 flex-shrink-0" />
                        <span className="truncate">{tab.title}</span>
                      </button>
                    ))}
                  </div>

                  {closedTabs.length > 0 && (
                    <>
                      <div className="p-2 border-t border-border">
                        <span className="text-[10px] uppercase tracking-wider text-muted-foreground font-medium px-2">
                          Recently Closed
                        </span>
                      </div>
                      <div className="max-h-32 overflow-y-auto">
                        {closedTabs.map((tab) => (
                          <button
                            key={tab.id}
                            onClick={() => handleRestoreTab(tab)}
                            className="w-full flex items-center gap-2 px-3 py-2 text-left text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                          >
                            <RotateCcw className="w-3.5 h-3.5 flex-shrink-0" />
                            <span className="truncate flex-1">{tab.title}</span>
                          </button>
                        ))}
                      </div>
                    </>
                  )}
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      <div
        data-testid="chat-scroll-container"
        className="flex-1 overflow-y-auto overscroll-contain"
        ref={scrollContainerRef}
        onScroll={(e) => {
          if (activeWorkspaceId == null || activeThreadId == null) return
          const el = e.target as HTMLDivElement
          const distanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight
          const isNearBottom = distanceToBottom < 24
          if (!programmaticScrollRef.current) {
            setFollowTail(isNearBottom)
            localStorage.setItem(
              followTailKeyForThread(activeWorkspaceId, activeThreadId),
              isNearBottom ? "true" : "false",
            )
          }
        }}
      >
        <div className="max-w-3xl mx-auto py-4 px-4 space-y-4">
          {messages.length > 0 ? (
            messages.map((message) => (
            <div key={message.id} className="group">
              {message.type === "assistant" ? (
                <div className="space-y-1">
                  {message.activities && (
                    <ActivityStream activities={message.activities} isStreaming={message.isStreaming} />
                  )}

                  {message.content && message.content.length > 0 && (
                    <Markdown content={message.content} />
                  )}

                  {message.codeReferences && message.codeReferences.length > 0 && (
                    <div className="mt-3 flex flex-wrap gap-1.5">
                      {message.codeReferences.map((ref, idx) => (
                        <button
                          key={idx}
                          className="inline-flex items-center gap-1.5 px-2 py-1 bg-muted/50 hover:bg-primary/10 hover:text-primary rounded text-xs font-mono text-muted-foreground transition-all"
                        >
                          <FileCode className="w-3 h-3" />
                          {ref.file}:{ref.line}
                        </button>
                      ))}
                    </div>
                  )}

	                  {message.metadata && !message.isStreaming && (
	                    <div className="flex items-center gap-3 pt-2 text-[11px] text-muted-foreground/70">
	                      {message.metadata.toolCalls && (
	                        <span className="flex items-center gap-1">
	                          <Wrench className="w-3 h-3" />
	                          {message.metadata.toolCalls}
	                        </span>
	                      )}
	                      {message.metadata.thinkingSteps && (
	                        <span className="flex items-center gap-1">
	                          <Brain className="w-3 h-3" />
	                          {message.metadata.thinkingSteps}
	                        </span>
	                      )}
	                      {message.metadata.duration && (
	                        <span className="flex items-center gap-1">
	                          <Clock className="w-3 h-3" />
	                          {message.metadata.duration}
	                        </span>
	                      )}
	                      <button
	                        className="ml-auto opacity-0 group-hover:opacity-100 transition-opacity hover:text-foreground p-1 -m-1"
	                        onClick={() => void copyToClipboard(message.content)}
	                      >
	                        <Copy className="w-3 h-3" />
	                      </button>
	                    </div>
	                  )}
	                </div>
	              ) : (
                <div className="flex justify-end">
	                  <div
                      data-testid="user-message-bubble"
                      className="max-w-[85%] border border-border rounded-lg px-3 py-2.5 bg-muted/30"
                    >
	                    <div className="text-[13px] text-foreground space-y-1 break-words overflow-hidden">
	                      {message.content.split("\n").map((line, idx) => (
	                        <p key={idx} className="flex items-start gap-2 min-w-0">
	                          {line.startsWith("•") && (
	                            <>
	                              <span className="text-muted-foreground mt-0.5 flex-shrink-0">•</span>
	                              <span className="flex-1 min-w-0 break-words">{line.slice(2)}</span>
	                            </>
	                          )}
	                          {!line.startsWith("•") && (
	                            <span className="flex-1 min-w-0 break-words">{line}</span>
	                          )}
	                        </p>
	                      ))}
	                    </div>
	                  </div>
	                </div>
	              )}
	            </div>
          ))
          ) : (
            <div className="text-sm text-muted-foreground">
              {activeWorkspaceId == null ? "Select a workspace to start." : "Select a thread to load conversation."}
            </div>
          )}
        </div>
      </div>

      {!followTail && messages.length > 0 ? (
        <div className="flex justify-center -mt-10 mb-2 relative z-10">
          <button
            className="flex items-center gap-1.5 px-3 py-1.5 bg-card border border-border rounded-full text-xs text-muted-foreground hover:text-foreground hover:border-primary/50 transition-all shadow-lg backdrop-blur-sm"
            onClick={() => {
              if (activeWorkspaceId == null || activeThreadId == null) return
              setFollowTail(true)
              localStorage.setItem(followTailKeyForThread(activeWorkspaceId, activeThreadId), "true")
              scheduleScrollToBottom()
            }}
          >
            <ArrowDown className="w-3 h-3" />
            Scroll to bottom
          </button>
        </div>
      ) : null}

      <div className="border-t border-border bg-card/50 px-4 py-3">
        <div className="max-w-3xl mx-auto space-y-2">
          <div className="flex items-center gap-1">
            <button className="inline-flex items-center gap-1.5 px-2 py-1 hover:bg-muted rounded text-xs text-muted-foreground hover:text-foreground transition-colors">
              <Brain className="w-3.5 h-3.5" />
              {modelLabel}
              <ChevronDown className="w-3 h-3 opacity-50" />
            </button>
            <div className="w-px h-4 bg-border" />
            <button className="inline-flex items-center gap-1.5 px-2 py-1 hover:bg-muted rounded text-xs text-muted-foreground hover:text-foreground transition-colors">
              <Settings2 className="w-3.5 h-3.5" />
              {effortLabel}
              <ChevronDown className="w-3 h-3 opacity-50" />
            </button>
          </div>

          <div className="relative">
            <textarea
              data-testid="chat-input"
              value={draftText}
              onChange={(e) => {
                setDraftText(e.target.value)
                persistDraft(e.target.value)
              }}
              placeholder="Message... (⌘↵ to send)"
              className="w-full min-h-[60px] bg-background border border-border rounded-lg px-3 py-2.5 pr-11 text-sm resize-none focus:outline-none focus:ring-1 focus:ring-primary/40 focus:border-primary/60 placeholder:text-muted-foreground transition-all"
              rows={2}
              disabled={activeWorkspaceId == null || activeThreadId == null}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                  e.preventDefault()
                  handleSend()
                }
              }}
            />
            <button
              data-testid="chat-send"
              aria-label="Send message"
              className="absolute right-2 bottom-2 p-1.5 bg-primary text-primary-foreground rounded-md hover:bg-primary/90 transition-colors disabled:opacity-50"
              onClick={handleSend}
              disabled={
                draftText.trim().length === 0 ||
                activeWorkspaceId == null ||
                activeThreadId == null
              }
            >
              <Send className="w-4 h-4" />
            </button>
          </div>

        </div>
      </div>
    </div>
  )
}
