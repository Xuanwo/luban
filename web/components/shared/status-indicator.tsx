"use client"

import type React from "react"

import { CheckCircle2, Circle, Clock, GitMerge, Loader2, MessageCircle, XCircle } from "lucide-react"

import { cn } from "@/lib/utils"
import type { AgentStatus, PRStatus } from "@/lib/task-ui"

export type { AgentStatus, PRStatus }

export const agentStatusConfig: Record<
  AgentStatus,
  {
    icon: React.ElementType
    color: string
    label: string
    animate?: boolean
  }
> = {
  idle: { icon: Circle, color: "text-status-idle", label: "Idle" },
  running: { icon: Loader2, color: "text-status-running", label: "Running", animate: true },
  pending: { icon: MessageCircle, color: "text-status-warning", label: "Awaiting read" },
}

export const prStatusConfig: Record<
  PRStatus,
  {
    icon: React.ElementType
    color: string
    label: string
    animate?: boolean
  }
> = {
  none: { icon: Circle, color: "text-transparent", label: "" },
  "ci-running": { icon: Loader2, color: "text-status-warning", label: "CI Running", animate: true },
  "ci-passed": { icon: CheckCircle2, color: "text-status-success", label: "CI Passed" },
  "review-pending": { icon: Clock, color: "text-status-info", label: "In Review" },
  "ready-to-merge": { icon: GitMerge, color: "text-status-success", label: "Ready to merge" },
  "ci-failed": { icon: XCircle, color: "text-status-error", label: "CI Failed" },
}

export function AgentStatusIcon({
  status,
  size = "sm",
  className,
}: {
  status: AgentStatus
  size?: "xs" | "sm" | "md"
  className?: string
}) {
  const config = agentStatusConfig[status]
  const Icon = config.icon
  const iconSize = size === "xs" ? "w-3 h-3" : size === "sm" ? "w-3.5 h-3.5" : "w-4 h-4"
  const spinStyle: React.CSSProperties | undefined = config.animate
    ? { transformBox: "fill-box", transformOrigin: "center" }
    : undefined

  return (
    <span className={cn("relative flex items-center justify-center flex-shrink-0", config.color, className)}>
      <Icon className={cn(iconSize, config.animate && "animate-spin")} style={spinStyle} />
      {status === "pending" && (
        <span className="absolute -top-0.5 -right-0.5 w-1.5 h-1.5 bg-status-warning rounded-full" />
      )}
    </span>
  )
}

export function PRBadge({
  status,
  prNumber,
  workspaceId,
  size = "sm",
  onOpenPullRequest,
  onOpenPullRequestFailedAction,
  titleOverride,
}: {
  status: PRStatus
  prNumber?: number
  workspaceId?: number
  size?: "sm" | "md"
  onOpenPullRequest?: (workspaceId: number) => void
  onOpenPullRequestFailedAction?: (workspaceId: number) => void
  titleOverride?: string
}) {
  if (status === "none" || prNumber == null) return null

  const config = prStatusConfig[status]
  const Icon = config.icon
  const iconSize = size === "sm" ? "w-2.5 h-2.5" : "w-3 h-3"
  const spinStyle: React.CSSProperties | undefined = config.animate
    ? { transformBox: "fill-box", transformOrigin: "center" }
    : undefined

  const canOpenPr = workspaceId != null && onOpenPullRequest != null
  const canOpenCi = workspaceId != null && onOpenPullRequestFailedAction != null

  const handlePrClick = (e: React.MouseEvent) => {
    e.stopPropagation()
    if (!canOpenPr || workspaceId == null) return
    onOpenPullRequest(workspaceId)
  }

  const handleCiClick = (e: React.MouseEvent) => {
    e.stopPropagation()
    if (!canOpenCi || workspaceId == null) return
    onOpenPullRequestFailedAction(workspaceId)
  }

  const title = titleOverride ?? config.label

  return (
    <div className="flex items-center gap-1.5 flex-shrink-0">
      <button
        onClick={handlePrClick}
        disabled={!canOpenPr}
        className={cn(
          "text-xs font-mono text-muted-foreground hover:text-foreground transition-colors",
          !canOpenPr && "opacity-70 cursor-default hover:text-muted-foreground",
        )}
        title={`Open PR #${prNumber}`}
      >
        #{prNumber}
      </button>
      {status === "ci-failed" ? (
        <button
          onClick={handleCiClick}
          disabled={!canOpenCi}
          className={cn(!canOpenCi && "opacity-70 cursor-default")}
          title={title}
        >
          <Icon className={cn(iconSize, config.color, "hover:opacity-80 transition-opacity")} />
        </button>
      ) : (
        <Icon
          className={cn(iconSize, config.color, config.animate && "animate-spin")}
          title={title}
          style={spinStyle}
        />
      )}
    </div>
  )
}
