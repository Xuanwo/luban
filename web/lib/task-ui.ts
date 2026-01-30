"use client"

import type { WorkspaceSnapshot } from "./luban-api"

export type AgentStatus = "idle" | "running" | "pending"
export type PRStatus =
  | "none"
  | "ci-running"
  | "ci-passed"
  | "review-pending"
  | "ready-to-merge"
  | "ci-failed"

export type KanbanColumn = "backlog" | "running" | "pending" | "reviewing" | "done"

export const kanbanColumns: { id: KanbanColumn; label: string; color: string }[] = [
  { id: "backlog", label: "Backlog", color: "text-status-idle" },
  { id: "running", label: "Running", color: "text-status-running" },
  { id: "pending", label: "Pending", color: "text-status-warning" },
  { id: "reviewing", label: "Reviewing", color: "text-status-info" },
  { id: "done", label: "Done", color: "text-status-success" },
]

export function agentStatusFromTask(task: WorkspaceSnapshot): AgentStatus {
  if (task.agent_run_status === "running") return "running"
  if (task.has_unread_completion) return "pending"
  return "idle"
}

export function prStatusFromTask(task: WorkspaceSnapshot): {
  status: PRStatus
  prNumber?: number
  prState?: "open" | "closed" | "merged"
} {
  const pr = task.pull_request
  if (!pr) return { status: "none" }

  if (pr.state === "merged") {
    return { status: "ready-to-merge", prNumber: pr.number, prState: "merged" }
  }
  if (pr.state !== "open") {
    return { status: "none", prState: pr.state }
  }

  if (pr.ci_state === "failure") return { status: "ci-failed", prNumber: pr.number, prState: pr.state }
  if (pr.ci_state === "pending" || pr.ci_state == null) {
    return { status: "ci-running", prNumber: pr.number, prState: pr.state }
  }

  if (pr.merge_ready) return { status: "ready-to-merge", prNumber: pr.number, prState: pr.state }
  return { status: "ci-passed", prNumber: pr.number, prState: pr.state }
}

export function kanbanColumnForTask(args: { agentStatus: AgentStatus; prStatus: PRStatus }): KanbanColumn {
  if (args.agentStatus === "running") return "running"
  if (args.agentStatus === "pending") return "pending"

  switch (args.prStatus) {
    case "ci-running":
    case "ci-passed":
    case "review-pending":
      return "reviewing"
    case "ready-to-merge":
      return "done"
    case "ci-failed":
      return "pending"
    default:
      return "backlog"
  }
}
