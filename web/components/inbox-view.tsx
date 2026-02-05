"use client"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  Loader2,
  MoreHorizontal,
  Filter,
  SlidersHorizontal,
  Inbox as InboxIcon,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { TaskActivityPanel } from "./task-activity-panel"
import { TaskHeader } from "./shared/task-header"
import { TaskStatusIcon, TaskStatusSelector, taskStatusConfig } from "./shared/task-status-selector"
import { useLuban } from "@/lib/luban-context"
import { computeProjectDisplayNames } from "@/lib/project-display-names"
import { projectColorClass } from "@/lib/project-colors"
import { fetchConversation, fetchTasks } from "@/lib/luban-http"
import type { ConversationSnapshot, OperationStatus, TaskStatus, TasksSnapshot, TurnResult, TurnStatus } from "@/lib/luban-api"
import { isMockMode } from "@/lib/luban-mode"

export interface InboxNotification {
  id: string
  workdirId: number
  taskId: number
  taskTitle: string
  workdir: string
  projectName: string
  projectAvatarUrl: string
  projectFallbackAvatarUrl: string
  projectColor: string
  type: TurnResult | null
  taskLifecycleStatus: TaskStatus
  taskStatus: {
    agentRunStatus: OperationStatus
    turnStatus: TurnStatus
    lastTurnResult: TurnResult | null
    hasUnreadCompletion: boolean
  }
  timestamp: string
  read: boolean
  isStarred: boolean
}

interface InboxViewProps {
  onOpenFullView?: (notification: InboxNotification) => void
}

function escapeXmlText(raw: string): string {
  return raw
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;")
}

function buildFallbackAvatarUrl(displayName: string, size: number): string {
  const letter = displayName.trim().slice(0, 1).toUpperCase() || "?"
  const safeLetter = escapeXmlText(letter)
  const svg = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">`,
    `<rect width="${size}" height="${size}" rx="3" fill="#e8e8e8" />`,
    `<text x="${size / 2}" y="${Math.floor(size * 0.7)}" text-anchor="middle" font-size="${Math.floor(size * 0.62)}" font-family="system-ui, -apple-system, sans-serif" fill="#6b6b6b">${safeLetter}</text>`,
    `</svg>`,
  ].join("")
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

function extractLatestAgentResponsePreviewLine(conversation: ConversationSnapshot): string | null {
  for (let i = conversation.entries.length - 1; i >= 0; i -= 1) {
    const entry = conversation.entries[i]
    if (!entry) continue
    if (entry.type !== "agent_event") continue
    if (entry.event.type !== "message") continue
    const line = firstNonEmptyLine(entry.event.text)
    if (line) return line
  }
  return null
}

function extractLatestUserMessagePreviewLine(conversation: ConversationSnapshot): string | null {
  for (let i = conversation.entries.length - 1; i >= 0; i -= 1) {
    const entry = conversation.entries[i]
    if (!entry) continue
    if (entry.type !== "user_event") continue
    if (entry.event.type !== "message") continue
    const line = firstNonEmptyLine(entry.event.text)
    if (line) return line
  }
  return null
}

function firstNonEmptyLine(text: string): string | null {
  for (const raw of text.split(/\r?\n/)) {
    const trimmed = raw.trim()
    if (!trimmed) continue
    return trimmed
  }
  return null
}

function InboxTaskStatusIndicator({
  lifecycleStatus,
  isRunning,
  hasUnreadCompletion,
}: {
  lifecycleStatus: TaskStatus
  isRunning: boolean
  hasUnreadCompletion: boolean
}) {
  const label = taskStatusConfig[lifecycleStatus]?.label ?? lifecycleStatus
  return (
    <div
      className="flex items-center justify-end gap-1"
      title={isRunning ? `Task status: ${label} (runner running)` : `Task status: ${label}`}
      data-testid="inbox-notification-status-indicator"
    >
      <TaskStatusIcon status={lifecycleStatus} size="xs" className="w-[14px] h-[14px]" />
      {isRunning ? (
        <Loader2
          className="w-[12px] h-[12px] animate-spin"
          style={{ color: "#5e6ad2" }}
          data-testid="inbox-notification-runner-spinner"
        />
      ) : hasUnreadCompletion ? (
        <span
          className="w-[6px] h-[6px] rounded-full"
          style={{ backgroundColor: "#5e6ad2" }}
          data-testid="inbox-notification-unread-indicator"
          aria-label="Unread completion"
          title="Unread completion"
        />
      ) : null}
    </div>
  )
}

interface NotificationRowProps {
  notification: InboxNotification
  previewText: string
  timestampText: string
  testId?: string
  selected?: boolean
  onClick?: () => void
  onDoubleClick?: () => void
}

function NotificationRow({ notification, previewText, timestampText, testId, selected, onClick, onDoubleClick }: NotificationRowProps) {
  const isRunning =
    notification.taskStatus.agentRunStatus === "running" || notification.taskStatus.turnStatus === "running"

  return (
    <div
      data-testid={testId}
      data-read={notification.read ? "true" : "false"}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      className={cn(
        "group flex items-start gap-2 px-3 py-2.5 cursor-pointer transition-colors",
        selected ? "bg-[#f0f0f0]" : "hover:bg-[#f7f7f7]",
      )}
      style={{ borderBottom: '1px solid #ebebeb' }}
    >
      {/* Content */}
      <div className="flex-1 min-w-0">
        {/* Project + Title row */}
        <div className="flex items-center gap-1.5">
          <img
            data-testid="inbox-notification-project-avatar"
            src={notification.projectAvatarUrl}
            alt={`${notification.projectName} project avatar`}
            className="w-[14px] h-[14px] rounded-[3px] flex-shrink-0"
            loading="lazy"
            decoding="async"
            onError={(e) => {
              const img = e.currentTarget
              if (img.src !== notification.projectFallbackAvatarUrl) {
                img.src = notification.projectFallbackAvatarUrl
              }
            }}
          />
          <span className="text-[12px]" style={{ color: '#6b6b6b' }}>
            {notification.projectName}
          </span>
          <span className="text-[12px]" style={{ color: '#9b9b9b' }}>›</span>
          <span
            className={cn(
              "text-[13px] truncate",
              !notification.read ? "font-medium" : "font-normal"
            )}
            style={{ color: '#1b1b1b' }}
            data-testid="inbox-notification-task-title"
          >
            {notification.taskTitle}
          </span>
        </div>
        <div
          data-testid="inbox-notification-preview"
          className="text-[12px] mt-0.5 truncate"
          style={{ color: '#6b6b6b' }}
        >
          {previewText}
        </div>
      </div>

      {/* Status + Timestamp (vertical stack) */}
      <div className="flex flex-col items-end gap-0.5 flex-shrink-0">
        <span data-testid="inbox-notification-task-status-icon">
          <InboxTaskStatusIndicator
            lifecycleStatus={notification.taskLifecycleStatus}
            isRunning={isRunning}
            hasUnreadCompletion={notification.taskStatus.hasUnreadCompletion}
          />
        </span>
        <span
          data-testid="inbox-notification-timestamp"
          className="text-[11px]"
          style={{ color: '#9b9b9b' }}
        >
          {timestampText}
        </span>
      </div>
    </div>
  )
}

// Empty state component
function EmptyState({ unreadCount }: { unreadCount: number }) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center" style={{ color: '#9b9b9b' }}>
      <InboxIcon className="w-16 h-16 mb-4" strokeWidth={1} />
      <span className="text-[14px]">
        {unreadCount > 0 ? `${unreadCount} unread notifications` : 'No notifications'}
      </span>
    </div>
  )
}

export function InboxView({ onOpenFullView }: InboxViewProps) {
  const { app, wsConnected, subscribeServerEvents, openWorkdir, activateTask, setTaskStarred, setTaskStatus } = useLuban()
  const [tasksSnapshot, setTasksSnapshot] = useState<TasksSnapshot | null>(null)
  const [selectedNotificationId, setSelectedNotificationId] = useState<string | null>(null)
  const [nowMs, setNowMs] = useState<number | null>(null)
  const [previewByNotificationId, setPreviewByNotificationId] = useState<
    Record<string, { userLine: string | null; agentLine: string | null; runStartedAtUnixMs: number | null } | null>
  >({})
  const previewByNotificationIdRef = useRef(previewByNotificationId)
  const previewInFlightRef = useRef<Set<string>>(new Set())
  const prevWsConnectedRef = useRef(false)
  const stableUpdatedAtByTaskRef = useRef<Map<string, number>>(new Map())

  useEffect(() => {
    previewByNotificationIdRef.current = previewByNotificationId
  }, [previewByNotificationId])

  useEffect(() => {
    const hasApp = app != null
    if (!hasApp) {
      setTasksSnapshot(null)
      return
    }

    let cancelled = false
    void (async () => {
      try {
        const snap = await fetchTasks()
        if (cancelled) return
        const stable = stableUpdatedAtByTaskRef.current
        for (const t of snap.tasks ?? []) {
          const key = `${t.workdir_id}:${t.task_id}`
          if (!stable.has(key)) stable.set(key, t.updated_at_unix_seconds)
        }
        setTasksSnapshot(snap)
      } catch (err) {
        console.warn("fetchTasks failed", err)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [app != null])

  const refreshTasks = useCallback(async () => {
    if (!app) return
    try {
      const snap = await fetchTasks()
      const stable = stableUpdatedAtByTaskRef.current
      for (const t of snap.tasks ?? []) {
        const key = `${t.workdir_id}:${t.task_id}`
        if (!stable.has(key)) stable.set(key, t.updated_at_unix_seconds)
      }
      setTasksSnapshot(snap)
    } catch (err) {
      console.warn("fetchTasks failed", err)
    }
  }, [app])

  useEffect(() => {
    const prev = prevWsConnectedRef.current
    prevWsConnectedRef.current = wsConnected
    if (prev || !wsConnected) return
    void refreshTasks()
  }, [refreshTasks, wsConnected])

  useEffect(() => {
    return subscribeServerEvents((event) => {
      if (event.type !== "task_summaries_changed") return
      const stable = stableUpdatedAtByTaskRef.current
      for (const t of event.tasks ?? []) {
        const key = `${t.workdir_id}:${t.task_id}`
        if (!stable.has(key)) stable.set(key, t.updated_at_unix_seconds)
      }
      setTasksSnapshot((prev) => {
        if (!prev) return prev
        const nextTasks = [
          ...prev.tasks.filter((t) => t.workdir_id !== event.workdir_id),
          ...event.tasks,
        ]
        return { ...prev, tasks: nextTasks, rev: prev.rev + 1 }
      })
    })
  }, [subscribeServerEvents])

  const applyLocalTaskLifecycleStatus = useCallback((args: { workdirId: number; taskId: number; status: TaskStatus }) => {
    setTasksSnapshot((prev) => {
      if (!prev) return prev
      let changed = false
      const nextTasks = prev.tasks.map((t) => {
        if (t.workdir_id !== args.workdirId || t.task_id !== args.taskId) return t
        if (t.task_status === args.status) return t
        changed = true
        return { ...t, task_status: args.status }
      })
      return changed ? { ...prev, tasks: nextTasks, rev: prev.rev + 1 } : prev
    })
  }, [])

  const handleTaskLifecycleStatusChange = useCallback(
    (args: { workdirId: number; taskId: number; status: TaskStatus }) => {
      applyLocalTaskLifecycleStatus(args)
      setTaskStatus(args.workdirId, args.taskId, args.status)
      window.setTimeout(() => void refreshTasks(), 200)
    },
    [applyLocalTaskLifecycleStatus, refreshTasks, setTaskStatus],
  )

  useEffect(() => {
    const update = () => setNowMs(Date.now())
    update()
    const id = window.setInterval(update, 60_000)
    return () => window.clearInterval(id)
  }, [])

  const formatTimestamp = useCallback((updatedAtUnixSeconds: number): string => {
    const date = new Date(updatedAtUnixSeconds * 1000)
    const now = nowMs ?? date.getTime()
    const diffMs = Math.max(0, now - date.getTime())
    const diffMinutes = Math.floor(diffMs / 60_000)
    if (diffMinutes < 60) return `${diffMinutes}m`
    const diffHours = Math.floor(diffMinutes / 60)
    if (diffHours < 24) return `${diffHours}h`
    const year = date.getFullYear()
    const month = String(date.getMonth() + 1).padStart(2, "0")
    const day = String(date.getDate()).padStart(2, "0")
    return `${year}-${month}-${day}`
  }, [nowMs])

  const notifications = useMemo(() => {
    if (!app || !tasksSnapshot) return [] as InboxNotification[]

    const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))
    const workdirById = new Map<number, { projectPath: string; workdirName: string; workdirPath: string; status: string }>()
    for (const p of app.projects) {
      for (const w of p.workdirs) {
        workdirById.set(w.id, {
          projectPath: p.path,
          workdirName: w.workdir_name,
          workdirPath: w.workdir_path,
          status: w.status,
        })
      }
    }

    const projectInfoById = new Map<string, { name: string; color: string; avatarUrl: string; fallbackAvatarUrl: string }>()
    for (const p of app.projects) {
      const name = displayNames.get(p.path) ?? p.slug
      const fallbackAvatarUrl = buildFallbackAvatarUrl(name, 14)
      const avatarUrl = p.is_git
        ? isMockMode()
          ? fallbackAvatarUrl
          : `/api/projects/avatar?project_id=${encodeURIComponent(p.id)}`
        : fallbackAvatarUrl
      projectInfoById.set(p.id, {
        name,
        color: projectColorClass(p.id),
        avatarUrl,
        fallbackAvatarUrl,
      })
    }

    const out: InboxNotification[] = []
    const filtered = tasksSnapshot.tasks.filter((t) => {
      const workdir = workdirById.get(t.workdir_id) ?? null
      if (!workdir) return false
      if (workdir.status !== "active") return false
      return true
    })

    const stable = stableUpdatedAtByTaskRef.current
    filtered.sort((a, b) => {
      const aStable = stable.get(`${a.workdir_id}:${a.task_id}`) ?? a.updated_at_unix_seconds
      const bStable = stable.get(`${b.workdir_id}:${b.task_id}`) ?? b.updated_at_unix_seconds
      const primary = bStable - aStable
      if (primary !== 0) return primary
      const workdir = b.workdir_id - a.workdir_id
      if (workdir !== 0) return workdir
      return b.task_id - a.task_id
    })

    for (const t of filtered) {
      const projectInfo = projectInfoById.get(t.project_id) ?? {
        name: t.project_id,
        color: "bg-violet-500",
        avatarUrl: buildFallbackAvatarUrl(t.project_id, 14),
        fallbackAvatarUrl: buildFallbackAvatarUrl(t.project_id, 14),
      }
      const id = `task-${t.workdir_id}-${t.task_id}`
      out.push({
        id,
        workdirId: t.workdir_id,
        taskId: t.task_id,
        taskTitle: t.title,
        workdir: t.workdir_name || t.branch_name,
        projectName: projectInfo.name,
        projectAvatarUrl: projectInfo.avatarUrl,
        projectFallbackAvatarUrl: projectInfo.fallbackAvatarUrl,
        projectColor: projectInfo.color,
        type: t.last_turn_result,
        taskLifecycleStatus: t.task_status,
        taskStatus: {
          agentRunStatus: t.agent_run_status,
          turnStatus: t.turn_status,
          lastTurnResult: t.last_turn_result,
          hasUnreadCompletion: t.has_unread_completion,
        },
        timestamp: formatTimestamp(t.updated_at_unix_seconds),
        read: !t.has_unread_completion,
        isStarred: t.is_starred,
      })
    }
    return out
  }, [app, formatTimestamp, tasksSnapshot])

  useEffect(() => {
    if (notifications.length === 0) return

    const concurrency = 4
    const queue = notifications
      .map((n) => ({ id: n.id, workdirId: n.workdirId, taskId: n.taskId }))
      .filter((n) => previewByNotificationIdRef.current[n.id] === undefined && !previewInFlightRef.current.has(n.id))

    if (queue.length === 0) return

    let cancelled = false
    let nextIdx = 0

    const worker = async () => {
      while (!cancelled) {
        const idx = nextIdx
        nextIdx += 1
        const item = queue[idx]
        if (!item) return

        previewInFlightRef.current.add(item.id)
        try {
          const convo = await fetchConversation(item.workdirId, item.taskId, { limit: 200 })
          if (cancelled) return
          const agentLine = extractLatestAgentResponsePreviewLine(convo)
          const userLine = extractLatestUserMessagePreviewLine(convo)
          const runStartedAtUnixMs = convo.run_started_at_unix_ms ?? null
          setPreviewByNotificationId((prev) => {
            if (prev[item.id] !== undefined) return prev
            return { ...prev, [item.id]: { agentLine, userLine, runStartedAtUnixMs } }
          })
        } catch (err) {
          if (cancelled) return
          setPreviewByNotificationId((prev) => {
            if (prev[item.id] !== undefined) return prev
            return { ...prev, [item.id]: null }
          })
        } finally {
          previewInFlightRef.current.delete(item.id)
        }
      }
    }

    for (let i = 0; i < Math.min(concurrency, queue.length); i += 1) {
      void worker()
    }

    return () => {
      cancelled = true
    }
  }, [notifications])

  const selectedNotification = useMemo(() => {
    if (!selectedNotificationId) return null
    return notifications.find((n) => n.id === selectedNotificationId) ?? null
  }, [notifications, selectedNotificationId])

  const unreadCount = notifications.filter((n) => !n.read).length

  return (
    <div className="h-full flex" data-testid="inbox-view">
      {/* Left: Notification List */}
      <div
        className="flex flex-col border-r"
        style={{ width: '400px', borderColor: '#ebebeb' }}
      >
        {/* List Header */}
        <div
          className="flex items-center justify-between h-[39px] flex-shrink-0 px-3"
          style={{ borderBottom: '1px solid #ebebeb' }}
        >
          <div className="flex items-center gap-1">
            <span className="text-[13px] font-medium" style={{ color: '#1b1b1b' }}>
              Inbox
            </span>
            <button
              className="w-5 h-5 flex items-center justify-center rounded hover:bg-[#eeeeee] transition-colors"
              style={{ color: '#9b9b9b' }}
            >
              <MoreHorizontal className="w-3.5 h-3.5" />
            </button>
          </div>
          <div className="flex items-center gap-0.5">
            <button
              className="w-6 h-6 flex items-center justify-center rounded-[5px] hover:bg-[#eeeeee] transition-colors"
              style={{ color: '#9b9b9b' }}
              title="Filter"
            >
              <Filter className="w-4 h-4" />
            </button>
            <button
              className="w-6 h-6 flex items-center justify-center rounded-[5px] hover:bg-[#eeeeee] transition-colors"
              style={{ color: '#9b9b9b' }}
              title="Display options"
            >
              <SlidersHorizontal className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Notification List */}
        <div className="flex-1 overflow-y-auto">
          {notifications.map((notification, idx) => {
            const preview = previewByNotificationId[notification.id]
            const isRunning =
              notification.taskStatus.agentRunStatus === "running" || notification.taskStatus.turnStatus === "running"

            let previewText = "Loading response..."
            if (preview !== undefined) {
              if (preview == null) {
                previewText = "No agent response yet."
              } else {
                const selected = isRunning ? preview.userLine ?? preview.agentLine : preview.agentLine ?? preview.userLine
                previewText = selected ?? "No agent response yet."
              }
            }

            const timestampText =
              isRunning && preview && preview.runStartedAtUnixMs != null
                ? formatTimestamp(Math.floor(preview.runStartedAtUnixMs / 1000))
                : notification.timestamp

            return (
              <NotificationRow
                key={notification.id}
                notification={notification}
                previewText={previewText}
                timestampText={timestampText}
                testId={`inbox-notification-row-${idx}`}
                selected={selectedNotification?.id === notification.id}
                onClick={() => {
                  setSelectedNotificationId(notification.id)
                  setTasksSnapshot((prev) => {
                    if (!prev) return prev
                    return {
                      ...prev,
                      tasks: prev.tasks.map((t) =>
                        t.workdir_id === notification.workdirId && t.task_id === notification.taskId
                          ? { ...t, has_unread_completion: false }
                          : t,
                      ),
                    }
                  })
                  void (async () => {
                    await openWorkdir(notification.workdirId)
                    await activateTask(notification.taskId)
                  })()
                }}
                onDoubleClick={() => onOpenFullView?.(notification)}
              />
            )
          })}
        </div>
      </div>

      {/* Right: Preview Panel */}
      <div className="flex-1 flex flex-col min-w-0">
        {selectedNotification ? (
          <>
            {/* Preview Header - using shared TaskHeader */}
            <TaskHeader
              title={selectedNotification.taskTitle}
              workdir={selectedNotification.workdir}
              project={{
                name: selectedNotification.projectName,
                color: selectedNotification.projectColor,
                avatarUrl: selectedNotification.projectAvatarUrl,
              }}
              actionBar={
                <div className="flex items-center gap-2 min-w-0">
                  <TaskStatusSelector
                    status={selectedNotification.taskLifecycleStatus}
                    onStatusChange={(status) =>
                      handleTaskLifecycleStatusChange({
                        workdirId: selectedNotification.workdirId,
                        taskId: selectedNotification.taskId,
                        status,
                      })
                    }
                    variant="pill"
                    size="sm"
                    triggerTestId="inbox-task-status-trigger"
                  />
                </div>
              }
              showFullActions
              isStarred={selectedNotification.isStarred}
              onToggleStar={(nextStarred) => {
                setTaskStarred(selectedNotification.workdirId, selectedNotification.taskId, nextStarred)
                setTasksSnapshot((prev) => {
                  if (!prev) return prev
                  return {
                    ...prev,
                    tasks: prev.tasks.map((t) =>
                      t.workdir_id === selectedNotification.workdirId && t.task_id === selectedNotification.taskId
                        ? { ...t, is_starred: nextStarred }
                        : t,
                    ),
                  }
                })
              }}
            />

            {/* Chat Preview */}
            <div className="flex-1 min-h-0 flex">
              <TaskActivityPanel />
            </div>
          </>
        ) : (
          <EmptyState unreadCount={unreadCount} />
        )}
      </div>
    </div>
  )
}
