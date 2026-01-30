"use client"

import { useMemo, useState } from "react"
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

type NotificationType = "completed" | "failed" | "needs-review"

export interface InboxNotification {
  id: string
  workspaceId: number
  taskTitle: string
  workdir: string
  projectName: string
  projectColor: string
  type: NotificationType
  description: string
  timestamp: string
  read: boolean
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
  selected?: boolean
  onClick?: () => void
  onDoubleClick?: () => void
}

function NotificationRow({ notification, selected, onClick, onDoubleClick }: NotificationRowProps) {
  return (
    <div
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
  const { app, openWorkdir } = useLuban()
  const [selectedNotification, setSelectedNotification] = useState<InboxNotification | null>(null)
  const [pendingDiffFile, setPendingDiffFile] = useState<ChangedFile | null>(null)
  const [readIds, setReadIds] = useState<Set<string>>(() => new Set())

  const normalizePathLike = (raw: string) => raw.trim().replace(/\/+$/, "")
  const isImplicitProjectRootWorkdir = (projectPath: string, args: { workdirName: string; workdirPath: string }) =>
    args.workdirName === "main" && normalizePathLike(args.workdirPath) === normalizePathLike(projectPath)

  const notifications = useMemo(() => {
    if (!app) return [] as InboxNotification[]

    const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))

    const out: InboxNotification[] = []
    for (const p of app.projects) {
      const projectName = displayNames.get(p.path) ?? p.slug
      const projectColor = projectColorClass(p.id)
      for (const w of p.workdirs) {
        if (w.status !== "active") continue
        if (!w.has_unread_completion) continue
        if (isImplicitProjectRootWorkdir(p.path, { workdirName: w.workdir_name, workdirPath: w.workdir_path })) {
          continue
        }
        const id = `workspace-${w.id}`
        out.push({
          id,
          workspaceId: w.id,
          taskTitle: w.workdir_name || w.branch_name,
          workdir: w.branch_name || w.workdir_name,
          projectName,
          projectColor,
          type: "completed",
          description: "Task has unread completion",
          timestamp: "",
          read: readIds.has(id),
        })
      }
    }
    return out
  }, [app, readIds])

  const unreadCount = notifications.filter((n) => !n.read).length

  return (
    <div className="h-full flex">
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
          {notifications.map((notification) => (
            <NotificationRow
              key={notification.id}
              notification={notification}
              selected={selectedNotification?.id === notification.id}
              onClick={() => {
                setSelectedNotification(notification)
                setReadIds((prev) => {
                  const next = new Set(prev)
                  next.add(notification.id)
                  return next
                })
                void openWorkdir(notification.workspaceId)
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
