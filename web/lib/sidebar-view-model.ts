"use client"

import type { AppSnapshot, OperationStatus, ProjectId } from "./luban-api"
import { agentStatusFromWorkspace, prStatusFromWorkspace, type AgentStatus, type PRStatus } from "./worktree-ui"
import { computeProjectDisplayNames } from "./project-display-names"

export type SidebarWorktreeVm = {
  id: string
  name: string
  worktreeName: string
  isHome: boolean
  isArchiving: boolean
  agentStatus: AgentStatus
  prStatus: PRStatus
  prNumber?: number
  prTitle?: string
  workspaceId: number
  pinned: boolean
}

export type SidebarProjectVm = {
  id: ProjectId
  displayName: string
  path: string
  isGit: boolean
  expanded: boolean
  createWorkspaceStatus: OperationStatus
  worktrees: SidebarWorktreeVm[]
  pinned: boolean
}

export function buildSidebarProjects(
  app: AppSnapshot | null,
  args?: {
    optimisticArchivingWorkspaceIds?: Set<number>
    pinnedProjectIds?: Set<ProjectId>
    pinnedWorktreeIds?: Set<number>
  },
): SidebarProjectVm[] {
  if (!app) return []
  const optimisticArchiving = args?.optimisticArchivingWorkspaceIds ?? null
  const pinnedProjectIds = args?.pinnedProjectIds ?? new Set<ProjectId>()
  const pinnedWorktreeIds = args?.pinnedWorktreeIds ?? new Set<number>()

  const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))

  const projects = app.projects.map((p, projectIndex) => {
    const worktrees = p.workspaces
      .filter((w) => w.status === "active")
      .map((w, worktreeIndex) => {
        const agentStatus = agentStatusFromWorkspace(w)
        const pr = prStatusFromWorkspace(w)
        const vm: SidebarWorktreeVm = {
          id: w.short_id,
          name: w.branch_name,
          worktreeName: w.workspace_name,
          isHome: w.workspace_name === "main",
          isArchiving: w.archive_status === "running" || optimisticArchiving?.has(w.id) === true,
          agentStatus,
          prStatus: pr.status,
          prNumber: pr.prNumber,
          prTitle: pr.prState === "merged" ? "Merged" : undefined,
          workspaceId: w.id,
          pinned: pinnedWorktreeIds.has(w.id),
        }
        return { vm, index: worktreeIndex }
      })
      // Sort worktrees: pinned first, then preserve original order.
      .sort((a, b) => {
        if (a.vm.pinned && !b.vm.pinned) return -1
        if (!a.vm.pinned && b.vm.pinned) return 1
        return a.index - b.index
      })
      .map((x) => x.vm)

    const vm: SidebarProjectVm = {
      id: p.id,
      displayName: displayNames.get(p.path) ?? p.slug,
      path: p.path,
      isGit: p.is_git,
      expanded: p.expanded,
      createWorkspaceStatus: p.create_workspace_status,
      pinned: pinnedProjectIds.has(p.id),
      worktrees,
    }
    return { vm, index: projectIndex }
  })

  // Sort projects: pinned first, then preserve original order.
  return projects
    .sort((a, b) => {
      if (a.vm.pinned && !b.vm.pinned) return -1
      if (!a.vm.pinned && b.vm.pinned) return 1
      return a.index - b.index
    })
    .map((x) => x.vm)
}
