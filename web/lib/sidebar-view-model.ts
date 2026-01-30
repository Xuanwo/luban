"use client"

import type { AppSnapshot, OperationStatus, ProjectId } from "./luban-api"
import { computeProjectDisplayNames } from "./project-display-names"

export type SidebarProjectVm = {
  id: ProjectId
  displayName: string
  path: string
  isGit: boolean
  createWorkspaceStatus: OperationStatus
}

export function buildSidebarProjects(
  app: AppSnapshot | null,
  args?: {
    projectOrder?: ProjectId[]
  },
): SidebarProjectVm[] {
  if (!app) return []
  const projectOrder = args?.projectOrder ?? []

  const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))

  const projects = app.projects.map((p) => {
    const vm: SidebarProjectVm = {
      id: p.id,
      displayName: displayNames.get(p.path) ?? p.slug,
      path: p.path,
      isGit: p.is_git,
      createWorkspaceStatus: p.create_workspace_status,
    }
    return vm
  })

  if (projectOrder.length === 0) return projects

  return sortWithCustomOrder(projects, (p) => p.id, projectOrder, () => 0)
}

function sortWithCustomOrder<T, K>(
  items: T[],
  getKey: (item: T) => K,
  order: K[],
  fallbackCompare: (a: T, b: T) => number
): T[] {
  const orderMap = new Map<K, number>()
  order.forEach((key, idx) => orderMap.set(key, idx))

  return [...items].sort((a, b) => {
    const fallback = fallbackCompare(a, b)
    if (fallback !== 0) return fallback

    const aOrder = orderMap.get(getKey(a))
    const bOrder = orderMap.get(getKey(b))

    if (aOrder !== undefined && bOrder !== undefined) {
      return aOrder - bOrder
    }
    if (aOrder !== undefined) return -1
    if (bOrder !== undefined) return 1
    return 0
  })
}
