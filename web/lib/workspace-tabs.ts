"use client"

import type { ThreadMeta, WorkspaceTabsSnapshot } from "./luban-api"

export function normalizeWorkspaceTabsSnapshot(args: {
  tabs: WorkspaceTabsSnapshot
  threads: ThreadMeta[]
}): WorkspaceTabsSnapshot {
  const openTabs = args.tabs.open_tabs ?? []
  const archivedTabs = args.tabs.archived_tabs ?? []
  const known = new Set<number>([...openTabs, ...archivedTabs])
  const missing = args.threads.map((t) => t.task_id).filter((id) => !known.has(id))
  if (missing.length === 0) return args.tabs

  missing.sort((a, b) => a - b)
  return {
    ...args.tabs,
    archived_tabs: [...missing, ...archivedTabs],
  }
}
