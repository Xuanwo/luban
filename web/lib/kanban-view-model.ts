"use client"

import type { AppSnapshot } from "./luban-api"
import { computeProjectDisplayNames } from "./project-display-names"
import {
  kanbanColumns,
  agentStatusFromTask,
  kanbanColumnForTask,
  prStatusFromTask,
  type KanbanColumn,
  type AgentStatus,
  type PRStatus,
} from "./task-ui"

export type KanbanTaskVm = {
  id: string
  name: string
  projectName: string
  agentStatus: AgentStatus
  prStatus: PRStatus
  prNumber?: number
  prTitle?: string
  workspaceId: number
}

export type KanbanBoardVm = {
  tasks: KanbanTaskVm[]
  tasksByColumn: Record<KanbanColumn, KanbanTaskVm[]>
}

export function buildKanbanTasks(app: AppSnapshot | null): KanbanTaskVm[] {
  if (!app) return []
  const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))
  const out: KanbanTaskVm[] = []
  for (const p of app.projects) {
    for (const w of p.workspaces) {
      if (w.status !== "active") continue
      const agentStatus = agentStatusFromTask(w)
      const pr = prStatusFromTask(w)
      out.push({
        id: w.short_id,
        name: w.branch_name,
        projectName: displayNames.get(p.path) ?? p.slug,
        agentStatus,
        prStatus: pr.status,
        prNumber: pr.prNumber,
        prTitle: pr.prState === "merged" ? "Merged" : undefined,
        workspaceId: w.id,
      })
    }
  }
  return out
}

export function groupKanbanTasksByColumn(
  tasks: KanbanTaskVm[],
): Record<KanbanColumn, KanbanTaskVm[]> {
  return kanbanColumns.reduce(
    (acc, col) => {
      acc[col.id] = tasks.filter(
        (w) => kanbanColumnForTask({ agentStatus: w.agentStatus, prStatus: w.prStatus }) === col.id,
      )
      return acc
    },
    {} as Record<KanbanColumn, KanbanTaskVm[]>,
  )
}

export function buildKanbanBoardVm(app: AppSnapshot | null): KanbanBoardVm {
  const tasks = buildKanbanTasks(app)
  return { tasks, tasksByColumn: groupKanbanTasksByColumn(tasks) }
}
