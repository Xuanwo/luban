"use client"

import type React from "react"

import {
  ChevronDown,
  ChevronRight,
  GitBranch,
  Plus,
  Settings,
  Sparkles,
  LayoutGrid,
  Archive,
  Loader2,
  MessageCircle,
  GitPullRequest,
  CheckCircle2,
  XCircle,
  Clock,
  Circle,
  Home,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { useLuban } from "@/lib/luban-context"
import type { OperationStatus } from "@/lib/luban-api"
import { useEffect, useRef, useState } from "react"

type WorktreeStatus =
  | "idle" // Waiting for user input
  | "agent-running" // Agent is executing
  | "agent-done" // Agent returned, waiting for user to read
  | "pr-ci-running" // PR submitted, CI running
  | "pr-ci-passed-review" // CI passed, waiting for review
  | "pr-ci-passed-merge" // CI passed, review passed, waiting to merge
  | "pr-ci-failed" // CI failed

interface Worktree {
  id: string
  name: string
  isHome?: boolean
  status: WorktreeStatus
  prNumber?: number // PR number if submitted
  workspaceId: number
}

interface Project {
  id: number
  name: string
  expanded: boolean
  createWorkspaceStatus: OperationStatus
  worktrees: Worktree[]
}

function WorktreeStatusIndicator({
  status,
  prNumber,
  workspaceId,
  onOpenPullRequest,
  onOpenPullRequestFailedAction,
}: Pick<Worktree, "status" | "prNumber" | "workspaceId"> & {
  onOpenPullRequest: (workspaceId: number) => void
  onOpenPullRequestFailedAction: (workspaceId: number) => void
}) {
  const handlePrClick = (e: React.MouseEvent) => {
    e.stopPropagation()
    onOpenPullRequest(workspaceId)
  }

  const handleCiClick = (e: React.MouseEvent) => {
    e.stopPropagation()
    onOpenPullRequestFailedAction(workspaceId)
  }

  switch (status) {
    case "idle":
      return <Circle className="w-3 h-3 text-muted-foreground/50 flex-shrink-0" />

    case "agent-running":
      return <Loader2 className="w-3 h-3 text-primary animate-spin flex-shrink-0" />

    case "agent-done":
      return (
        <span className="relative flex-shrink-0">
          <MessageCircle className="w-3 h-3 text-amber-500" />
          <span className="absolute -top-0.5 -right-0.5 w-1.5 h-1.5 bg-amber-500 rounded-full" />
        </span>
      )

    case "pr-ci-running":
      return (
        <div className="flex items-center gap-1 flex-shrink-0">
          <button
            onClick={handlePrClick}
            className="flex items-center gap-0.5 text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
            title={`Open PR #${prNumber}`}
          >
            <GitPullRequest className="w-3 h-3" />
            <span>#{prNumber}</span>
          </button>
          <Loader2 className="w-2.5 h-2.5 text-amber-500 animate-spin" />
        </div>
      )

    case "pr-ci-passed-review":
      return (
        <div className="flex items-center gap-1 flex-shrink-0">
          <button
            onClick={handlePrClick}
            className="flex items-center gap-0.5 text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
            title={`Open PR #${prNumber}`}
          >
            <GitPullRequest className="w-3 h-3" />
            <span>#{prNumber}</span>
          </button>
          <span title="Waiting for review">
            <Clock className="w-2.5 h-2.5 text-amber-500" />
          </span>
        </div>
      )

    case "pr-ci-passed-merge":
      return (
        <div className="flex items-center gap-1 flex-shrink-0">
          <button
            onClick={handlePrClick}
            className="flex items-center gap-0.5 text-[10px] text-green-400 hover:text-green-300 transition-colors"
            title={`Open PR #${prNumber} - Ready to merge`}
          >
            <GitPullRequest className="w-3 h-3" />
            <span>#{prNumber}</span>
          </button>
          <span title="Ready to merge">
            <CheckCircle2 className="w-2.5 h-2.5 text-green-500" />
          </span>
        </div>
      )

    case "pr-ci-failed":
      return (
        <div className="flex items-center gap-1 flex-shrink-0">
          <button
            onClick={handlePrClick}
            className="flex items-center gap-0.5 text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
            title={`Open PR #${prNumber}`}
          >
            <GitPullRequest className="w-3 h-3" />
            <span>#{prNumber}</span>
          </button>
          <button onClick={handleCiClick} title="CI failed - Click to view">
            <XCircle className="w-2.5 h-2.5 text-red-500 hover:text-red-400 transition-colors" />
          </button>
        </div>
      )

    default:
      return null
  }
}

function getWorktreeStatusBg(status: WorktreeStatus): string {
  switch (status) {
    case "agent-running":
      return "bg-primary/10"
    case "agent-done":
      return "bg-amber-500/10"
    case "pr-ci-failed":
      return "bg-red-500/5"
    default:
      return ""
  }
}

interface SidebarProps {
  viewMode: "workspace" | "kanban"
  onViewModeChange: (mode: "workspace" | "kanban") => void
  widthPx: number
}

export function Sidebar({ viewMode, onViewModeChange, widthPx }: SidebarProps) {
  const {
    app,
    activeWorkspaceId,
    pickProjectPath,
    createWorkspace,
    openWorkspacePullRequest,
    openWorkspacePullRequestFailedAction,
    archiveWorkspace,
    toggleProjectExpanded,
    openWorkspace,
  } = useLuban()

  const pendingCreateRef = useRef<{ projectId: number; existingWorkspaceIds: Set<number> } | null>(null)
  const [optimisticCreatingProjectId, setOptimisticCreatingProjectId] = useState<number | null>(null)
  const [newlyCreatedWorkspaceId, setNewlyCreatedWorkspaceId] = useState<number | null>(null)

  useEffect(() => {
    if (!app) return

    if (optimisticCreatingProjectId != null) {
      const confirmed = app.projects.some(
        (p) => p.id === optimisticCreatingProjectId && p.create_workspace_status === "running",
      )
      if (confirmed) {
        setOptimisticCreatingProjectId(null)
      }
    }

    const running = app.projects.find((p) => p.create_workspace_status === "running")
    if (running) {
      if (!pendingCreateRef.current || pendingCreateRef.current.projectId !== running.id) {
        pendingCreateRef.current = {
          projectId: running.id,
          existingWorkspaceIds: new Set(running.workspaces.map((w) => w.id)),
        }
      }
      return
    }

    const pending = pendingCreateRef.current
    if (!pending) return
    const proj = app.projects.find((p) => p.id === pending.projectId)
    pendingCreateRef.current = null
    setOptimisticCreatingProjectId(null)

    const created = proj?.workspaces.find((w) => !pending.existingWorkspaceIds.has(w.id))?.id
    if (created == null) return

    setNewlyCreatedWorkspaceId(created)
    const t = window.setTimeout(() => setNewlyCreatedWorkspaceId(null), 1500)
    return () => window.clearTimeout(t)
  }, [app?.rev])

  const projects: Project[] =
    app?.projects.map((p) => ({
      id: p.id,
      name: p.slug,
      expanded: p.expanded,
      createWorkspaceStatus: p.create_workspace_status,
      worktrees: p.workspaces
        .filter((w) => w.status === "active")
        .map((w) => ({
          id: w.short_id,
          name: w.branch_name,
          isHome: w.workspace_name === "main",
          status: (() => {
            if (w.agent_run_status === "running") return "agent-running" as const
            if (w.has_unread_completion) return "agent-done" as const

            const pr = w.pull_request
            if (!pr || pr.state !== "open") return "idle" as const
            if (pr.ci_state === "failure") return "pr-ci-failed" as const
            if (pr.ci_state === "pending" || pr.ci_state == null) return "pr-ci-running" as const
            if (pr.merge_ready) return "pr-ci-passed-merge" as const
            return "pr-ci-passed-review" as const
          })(),
          prNumber: w.pull_request?.state === "open" ? w.pull_request.number : undefined,
          workspaceId: w.id,
        })),
    })) ?? []

  const getActiveWorktreeCount = (worktrees: Worktree[]) => {
    return worktrees.filter((w) => w.status !== "idle").length
  }

  return (
    <aside
      className="flex-shrink-0 border-r border-border bg-sidebar flex flex-col"
      style={{ width: `${widthPx}px` }}
    >
      <div className="flex items-center justify-between h-11 px-3 border-b border-border">
        <button
          onClick={() => onViewModeChange(viewMode === "workspace" ? "kanban" : "workspace")}
          className="flex items-center gap-2 hover:bg-sidebar-accent px-1.5 py-1 rounded transition-colors"
        >
          <div className="flex items-center justify-center w-6 h-6 rounded bg-primary/15">
            {viewMode === "workspace" ? (
              <Sparkles className="w-3.5 h-3.5 text-primary" />
            ) : (
              <LayoutGrid className="w-3.5 h-3.5 text-primary" />
            )}
          </div>
          <span className="text-sm font-medium">{viewMode === "workspace" ? "Workspace" : "Kanban"}</span>
          <ChevronDown className="w-3 h-3 text-muted-foreground" />
        </button>
        <button
          className="p-1.5 text-muted-foreground hover:text-foreground hover:bg-sidebar-accent rounded transition-colors"
          onClick={pickProjectPath}
        >
          <Plus className="w-4 h-4" />
        </button>
      </div>

      {/* Project List */}
      <div className="flex-1 overflow-y-auto overscroll-contain py-1.5">
	        {projects.map((project) => {
	          const activeCount = getActiveWorktreeCount(project.worktrees)
	          const isCreating =
	            project.createWorkspaceStatus === "running" || optimisticCreatingProjectId === project.id
	          return (
            <div key={project.id} className="group/project">
              <div className="flex items-center">
                <button
                  onClick={() => toggleProjectExpanded(project.id)}
                  className="flex-1 flex items-center gap-2 px-3 py-1.5 text-left hover:bg-sidebar-accent/50 transition-colors"
                >
                  {project.expanded ? (
                    <ChevronDown className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                  ) : (
                    <ChevronRight className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                  )}
                  <span className="text-[13px] text-muted-foreground truncate flex-1" title={project.name}>
                    {project.name}
                  </span>
                  {!project.expanded && activeCount > 0 && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/20 text-primary font-medium">
                      {activeCount}
                    </span>
                  )}
                </button>
	                <button
	                  className={cn(
	                    "p-1 mr-2 text-muted-foreground hover:text-foreground transition-all",
	                    isCreating ? "opacity-100" : "opacity-0 group-hover/project:opacity-100",
	                  )}
	                  title="Add worktree"
	                  onClick={() => {
                      if (isCreating) return
	                    if (!project.expanded) {
	                      toggleProjectExpanded(project.id)
	                    }
                      const fullProject = app?.projects.find((p) => p.id === project.id) ?? null
                      const existingWorkspaceIds = new Set<number>(
                        fullProject?.workspaces.map((w) => w.id) ?? project.worktrees.map((w) => w.workspaceId),
                      )
                      pendingCreateRef.current = { projectId: project.id, existingWorkspaceIds }
                      setOptimisticCreatingProjectId(project.id)
	                    createWorkspace(project.id)
	                  }}
	                  disabled={isCreating}
	                >
	                  {isCreating ? (
	                    <Loader2 className="w-3.5 h-3.5 animate-spin text-primary" />
	                  ) : (
	                    <Plus className="w-3.5 h-3.5" />
	                  )}
	                </button>
	              </div>

	              {project.expanded && (
	                <div className="ml-4 pl-3 border-l border-border-subtle">
	                  {project.worktrees.map((worktree, idx) => (
	                    <div
	                      key={worktree.workspaceId}
	                      className={cn(
	                        "group/worktree flex items-center gap-2 px-2 py-1.5 hover:bg-sidebar-accent/30 transition-all cursor-pointer rounded-sm mx-1",
	                        getWorktreeStatusBg(worktree.status),
	                        worktree.workspaceId === activeWorkspaceId && "bg-sidebar-accent/30",
	                        newlyCreatedWorkspaceId === worktree.workspaceId &&
	                          "animate-in slide-in-from-left-2 fade-in duration-300 bg-primary/15 ring-1 ring-primary/30",
	                      )}
	                      style={{
	                        animationDelay:
	                          newlyCreatedWorkspaceId === worktree.workspaceId ? "0ms" : `${idx * 30}ms`,
	                      }}
	                      onClick={() => {
	                        void openWorkspace(worktree.workspaceId)
	                      }}
	                    >
	                      <div className="flex flex-col flex-1 min-w-0">
	                        <div className="flex items-center gap-1.5">
	                          <GitBranch className="w-3 h-3 text-muted-foreground flex-shrink-0" />
	                          <span
                              data-testid="worktree-branch-name"
                              className="text-xs text-foreground truncate"
                              title={worktree.name}
                            >
	                            {worktree.name}
	                          </span>
	                        </div>
	                        <span
                            data-testid="worktree-short-id"
                            className="text-[10px] text-muted-foreground/50 ml-4 font-mono"
                          >
                            {worktree.id}
                          </span>
	                      </div>
	                      <WorktreeStatusIndicator
	                        status={worktree.status}
	                        prNumber={worktree.prNumber}
                        workspaceId={worktree.workspaceId}
                        onOpenPullRequest={openWorkspacePullRequest}
                        onOpenPullRequestFailedAction={openWorkspacePullRequestFailedAction}
                      />
                      {/* Archive button only for non-home worktrees */}
                      {worktree.isHome ? (
                        <span className="p-0.5 text-muted-foreground/50" title="Main worktree">
                          <Home className="w-3 h-3" />
                        </span>
                      ) : (
                        <button
                          className="p-0.5 text-muted-foreground hover:text-foreground opacity-0 group-hover/worktree:opacity-100 transition-opacity"
                          title="Archive worktree"
                          onClick={(e) => {
                            e.stopPropagation()
                            archiveWorkspace(worktree.workspaceId)
                          }}
                        >
	                          <Archive className="w-3 h-3" />
	                        </button>
	                      )}
	                    </div>
	                  ))}

	                  {isCreating && (
	                    <div className="flex items-center gap-2 px-2 py-1.5 mx-1 animate-pulse">
	                      <div className="flex flex-col flex-1 gap-1">
	                        <div className="flex items-center gap-1.5">
	                          <div className="w-3 h-3 rounded bg-muted-foreground/20" />
	                          <div className="h-3 w-20 rounded bg-muted-foreground/20" />
	                        </div>
	                        <div className="h-2.5 w-8 ml-4 rounded bg-muted-foreground/15" />
	                      </div>
	                    </div>
	                  )}
	                </div>
	              )}
	            </div>
	          )
	        })}
      </div>

      {/* Bottom Actions */}
      <div className="border-t border-border p-2">
        <button className="w-full flex items-center gap-2 px-3 py-2 text-sm text-muted-foreground hover:text-foreground hover:bg-sidebar-accent rounded transition-colors">
          <Settings className="w-4 h-4" />
          Settings
        </button>
      </div>
    </aside>
  )
}
