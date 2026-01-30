"use client"

import type { ServerEvent, WorkspaceId } from "./luban-api"
import type { LubanStore } from "./luban-store"
import { DEFAULT_NEW_THREAD_TIMEOUT_MS, pickCreatedThreadId } from "./luban-thread-flow"
import { normalizeWorkspaceTabsSnapshot } from "./workspace-tabs"

export function createLubanServerEventHandler(args: {
  store: LubanStore
  onToast: (message: string) => void
  onSelectThreadInWorkspace: (workspaceId: WorkspaceId, threadId: number) => void
}): (event: ServerEvent) => void {
  return (event) => {
    switch (event.type) {
      case "app_changed": {
        args.store.setApp(event.snapshot)
        return
      }
      case "workdir_tasks_changed": {
        const wid = args.store.refs.activeWorkspaceIdRef.current
        if (wid == null || wid !== event.workdir_id) return

        args.store.cacheThreads(wid, event.tasks)
        args.store.setThreads(event.tasks)
        const normalizedTabs = normalizeWorkspaceTabsSnapshot({ tabs: event.tabs, threads: event.tasks })
        args.store.cacheWorkspaceTabs(wid, normalizedTabs)
        args.store.setWorkspaceTabs(normalizedTabs)
        const current = args.store.refs.activeThreadIdRef.current
        const threadIds = new Set(event.tasks.map((t) => t.task_id))
        const openThreadIds = (normalizedTabs.open_tabs ?? []).filter((id) => threadIds.has(id))

        const pending = args.store.refs.pendingCreateThreadRef.current
        if (pending && pending.workspaceId === wid) {
          const created = pickCreatedThreadId({
            threads: event.tasks,
            existingThreadIds: pending.existingThreadIds,
          })
          if (created != null) {
            args.store.refs.pendingCreateThreadRef.current = null
            args.onSelectThreadInWorkspace(wid, created)
            return
          }

          if (Date.now() - pending.requestedAtUnixMs > DEFAULT_NEW_THREAD_TIMEOUT_MS) {
            args.store.refs.pendingCreateThreadRef.current = null
          }
        }

        const currentExists = current != null && threadIds.has(current)
        const currentIsOpen = current != null && openThreadIds.includes(current)
        if (!currentExists || !currentIsOpen) {
          const preferred = normalizedTabs.active_tab
          const next = (openThreadIds.includes(preferred) ? preferred : null) ?? openThreadIds[0] ?? event.tasks[0]?.task_id ?? null
          if (next != null) args.onSelectThreadInWorkspace(wid, next)
        }
        return
      }
      case "conversation_changed": {
        const wid = args.store.refs.activeWorkspaceIdRef.current
        const tid = args.store.refs.activeThreadIdRef.current
        args.store.cacheConversation(event.snapshot)
        if (wid == null || tid == null) return
        if (event.snapshot.workdir_id === wid && event.snapshot.task_id === tid) {
          args.store.setConversation(event.snapshot)
        }
        return
      }
      case "toast": {
        args.onToast(event.message)
        return
      }
      case "project_path_picked":
      case "task_preview_ready":
      case "task_executed":
        return
    }
  }
}
