import { useMemo } from "react"

import type { ThreadMeta, WorkspaceTabsSnapshot, WorkspaceThreadId } from "@/lib/luban-api"

export type ChatTab = {
  id: string
  title: string
  isActive: boolean
}

export type ArchivedTab = {
  id: string
  title: string
}

export function useThreadTabs(args: {
  threads: ThreadMeta[]
  workspaceTabs: WorkspaceTabsSnapshot | null
  activeThreadId: WorkspaceThreadId | null
}): {
  tabs: ChatTab[]
  archivedTabs: ArchivedTab[]
  openThreadIds: WorkspaceThreadId[]
  closedThreadIds: WorkspaceThreadId[]
  activeTabId: string
} {
  const threadsById = useMemo(() => {
    const out = new Map<number, ThreadMeta>()
    for (const t of args.threads) out.set(t.task_id, t)
    return out
  }, [args.threads])

  const { openThreadIds, closedThreadIds } = useMemo(() => {
    const all = args.threads.map((t) => t.task_id)
    if (all.length === 0) return { openThreadIds: [] as WorkspaceThreadId[], closedThreadIds: [] as WorkspaceThreadId[] }

    const openFromTabs = (args.workspaceTabs?.open_tabs ?? []).filter((id) => threadsById.has(id))
    const archivedFromTabs = (args.workspaceTabs?.archived_tabs ?? []).filter((id) => threadsById.has(id))

    const open = [...openFromTabs]
    if (
      args.activeThreadId != null &&
      threadsById.has(args.activeThreadId) &&
      !archivedFromTabs.includes(args.activeThreadId) &&
      !open.includes(args.activeThreadId)
    ) {
      open.push(args.activeThreadId)
    }

    const known = new Set<number>([...open, ...archivedFromTabs])
    const recovered = all.filter((id) => !known.has(id))

    return {
      openThreadIds: open.length > 0 ? open : [all[0]!],
      closedThreadIds: [...recovered, ...archivedFromTabs],
    }
  }, [args.activeThreadId, args.threads, args.workspaceTabs?.archived_tabs, args.workspaceTabs?.open_tabs, threadsById])

  const tabs: ChatTab[] = useMemo(() => {
    return openThreadIds
      .map((id) => threadsById.get(id))
      .filter((t): t is ThreadMeta => Boolean(t))
      .map((t) => ({
        id: String(t.task_id),
        title: t.title,
        isActive: t.task_id === args.activeThreadId,
      }))
  }, [args.activeThreadId, openThreadIds, threadsById])

  const archivedTabs: ArchivedTab[] = useMemo(() => {
    const out: ArchivedTab[] = []
    for (const id of [...closedThreadIds].reverse()) {
      const t = threadsById.get(id)
      if (t) out.push({ id: String(id), title: t.title })
      else out.push({ id: String(id), title: `Task ${id}` })
    }
    return out
  }, [closedThreadIds, threadsById])

  const activeTabId = args.activeThreadId != null ? String(args.activeThreadId) : ""

  return { tabs, archivedTabs, openThreadIds, closedThreadIds, activeTabId }
}
