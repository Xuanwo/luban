"use client"

import type { ThreadMeta, WorkspaceId, WorkspaceTabsSnapshot } from "./luban-api"

export const DEFAULT_NEW_THREAD_TIMEOUT_MS = 5_000
export const DEFAULT_NEW_THREAD_POLL_MS = 250

export function pickCreatedThreadId(args: {
  threads: ThreadMeta[]
  existingThreadIds: Set<number>
}): number | null {
  const created = args.threads
    .map((t) => t.task_id)
    .filter((id) => !args.existingThreadIds.has(id))
    .sort((a, b) => b - a)[0]

  return created ?? null
}

export async function waitForNewThread(args: {
  workspaceId: WorkspaceId
  existingThreadIds: Set<number>
  fetchThreads: (workspaceId: WorkspaceId) => Promise<{ tasks: ThreadMeta[]; tabs: WorkspaceTabsSnapshot }>
  timeoutMs?: number
  pollMs?: number
}): Promise<{ threads: ThreadMeta[]; tabs: WorkspaceTabsSnapshot; createdThreadId: number | null }> {
  const timeoutMs = args.timeoutMs ?? DEFAULT_NEW_THREAD_TIMEOUT_MS
  const pollMs = args.pollMs ?? DEFAULT_NEW_THREAD_POLL_MS

  const startedAt = Date.now()
  while (Date.now() - startedAt < timeoutMs) {
    try {
      const snap = await args.fetchThreads(args.workspaceId)
      const createdThreadId = pickCreatedThreadId({
        threads: snap.tasks,
        existingThreadIds: args.existingThreadIds,
      })
      return { threads: snap.tasks, tabs: snap.tabs, createdThreadId }
    } catch {
      // ignore and retry
    }
    await new Promise((r) => setTimeout(r, pollMs))
  }
  return { threads: [], tabs: { open_tabs: [], archived_tabs: [], active_tab: 1 }, createdThreadId: null }
}
