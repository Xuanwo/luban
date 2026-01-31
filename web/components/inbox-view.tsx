"use client"

import { useEffect, useMemo, useState } from "react"
import {
  CheckCircle2,
  AlertCircle,
  MessageSquare,
  MoreHorizontal,
  Filter,
  SlidersHorizontal,
  Inbox as InboxIcon,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { ChatPanel } from "./chat-panel"
import { TaskHeader, ProjectIcon } from "./shared/task-header"
import type { ChangedFile } from "./right-sidebar"
import { useLuban } from "@/lib/luban-context"
import { computeProjectDisplayNames } from "@/lib/project-display-names"
import { projectColorClass } from "@/lib/project-colors"
import { fetchTasks } from "@/lib/luban-http"
import type { TasksSnapshot, TaskSummarySnapshot } from "@/lib/luban-api"

type NotificationType = "completed" | "failed" | "needs-review"

export interface InboxNotification {
  id: string
  workdirId: number
  taskId: number
  taskTitle: string
  workdir: string
  projectName: string
  projectColor: string
  type: NotificationType
  description: string
  timestamp: string
  read: boolean
  isStarred: boolean
}

interface InboxViewProps {
  onOpenFullView?: (notification: InboxNotification) => void
}

const NotificationIcon = ({ type }: { type: NotificationType }) => {
  switch (type) {
    case "completed":
      return <CheckCircle2 className="w-[14px] h-[14px]" style={{ color: '#5e6ad2' }} />
    case "failed":
      return <AlertCircle className="w-[14px] h-[14px]" style={{ color: '#eb5757' }} />
    case "needs-review":
      return <MessageSquare className="w-[14px] h-[14px]" style={{ color: '#f2994a' }} />
  }
}

interface NotificationRowProps {
  notification: InboxNotification
  testId?: string
  selected?: boolean
  onClick?: () => void
  onDoubleClick?: () => void
}

function NotificationRow({ notification, testId, selected, onClick, onDoubleClick }: NotificationRowProps) {
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
          <ProjectIcon name={notification.projectName} color={notification.projectColor} />
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
          >
            {notification.taskTitle}
          </span>
        </div>
        <div
          className="text-[12px] mt-0.5 truncate"
          style={{ color: '#6b6b6b' }}
        >
          {notification.description}
        </div>
      </div>

      {/* Status + Timestamp (vertical stack) */}
      <div className="flex flex-col items-end gap-0.5 flex-shrink-0">
        <NotificationIcon type={notification.type} />
        <span
          className="text-[11px]"
          style={{ color: '#9b9b9b' }}
        >
          {notification.timestamp}
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
  const { app, openWorkdir, activateTask, setTaskStarred } = useLuban()
  const [tasksSnapshot, setTasksSnapshot] = useState<TasksSnapshot | null>(null)
  const [selectedNotificationId, setSelectedNotificationId] = useState<string | null>(null)
  const [pendingDiffFile, setPendingDiffFile] = useState<ChangedFile | null>(null)

  useEffect(() => {
    if (!app) {
      setTasksSnapshot(null)
      return
    }

    let cancelled = false
    void (async () => {
      try {
        const snap = await fetchTasks()
        if (cancelled) return
        setTasksSnapshot(snap)
      } catch (err) {
        console.warn("fetchTasks failed", err)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [app?.rev])

  const formatTimestamp = (updatedAtUnixSeconds: number): string => {
    const date = new Date(updatedAtUnixSeconds * 1000)
    const now = Date.now()
    const diffMs = Math.max(0, now - date.getTime())
    const diffMinutes = Math.floor(diffMs / 60_000)
    if (diffMinutes < 60) return `${diffMinutes}m`
    const diffHours = Math.floor(diffMinutes / 60)
    if (diffHours < 24) return `${diffHours}h`
    const year = date.getFullYear()
    const month = String(date.getMonth() + 1).padStart(2, "0")
    const day = String(date.getDate()).padStart(2, "0")
    return `${year}-${month}-${day}`
  }

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

    const projectNameById = new Map<string, { name: string; color: string }>()
    for (const p of app.projects) {
      projectNameById.set(p.path, {
        name: displayNames.get(p.path) ?? p.slug,
        color: projectColorClass(p.id),
      })
    }

    const out: InboxNotification[] = []
    const filtered = tasksSnapshot.tasks.filter((t) => {
      const workdir = workdirById.get(t.workdir_id) ?? null
      if (!workdir) return false
      if (workdir.status !== "active") return false
      return true
    })

    filtered.sort((a, b) => b.updated_at_unix_seconds - a.updated_at_unix_seconds)

    for (const t of filtered) {
      const projectInfo = projectNameById.get(t.project_id) ?? { name: t.project_id, color: "bg-[#5e6ad2]" }
      const id = `task-${t.workdir_id}-${t.task_id}`
      out.push({
        id,
        workdirId: t.workdir_id,
        taskId: t.task_id,
        taskTitle: t.title,
        workdir: t.workdir_name || t.branch_name,
        projectName: projectInfo.name,
        projectColor: projectInfo.color,
        type: "completed",
        description: t.has_unread_completion ? "Unread completion" : "Read completion",
        timestamp: formatTimestamp(t.updated_at_unix_seconds),
        read: !t.has_unread_completion,
        isStarred: t.is_starred,
      })
    }
    return out
  }, [app, tasksSnapshot])

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
          {notifications.map((notification, idx) => (
            <NotificationRow
              key={notification.id}
              notification={notification}
              testId={`inbox-notification-row-${idx}`}
              selected={selectedNotification?.id === notification.id}
              onClick={() => {
                setSelectedNotificationId(notification.id)
                void (async () => {
                  await openWorkdir(notification.workdirId)
                  await activateTask(notification.taskId)
                })()
              }}
              onDoubleClick={() => onOpenFullView?.(notification)}
            />
          ))}
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
              project={{ name: selectedNotification.projectName, color: selectedNotification.projectColor }}
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
              <ChatPanel
                pendingDiffFile={pendingDiffFile}
                onDiffFileOpened={() => setPendingDiffFile(null)}
              />
            </div>
          </>
        ) : (
          <EmptyState unreadCount={unreadCount} />
        )}
      </div>
    </div>
  )
}
