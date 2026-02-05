"use client"

export const ACTIVE_WORKSPACE_KEY = "luban:active_workspace_id"

export const VIEW_MODE_KEY = "luban:ui:view_mode"
export const SIDEBAR_WIDTH_KEY = "luban:ui:sidebar_width_px"
export const GLOBAL_ZOOM_KEY = "luban:ui:global_zoom"
export const PROJECT_ORDER_KEY = "luban:ui:project_order"

export const INBOX_FILTERS_KEY = "luban:ui:inbox_filters"
export const INBOX_VIEWS_KEY = "luban:ui:inbox_views"
export const INBOX_ACTIVE_VIEW_KEY = "luban:ui:inbox_active_view"

export function activeThreadKey(workspaceId: number): string {
  return `luban:active_thread_id:${workspaceId}`
}

export function draftKey(workspaceId: number, threadId: number): string {
  return `luban:draft:${workspaceId}:${threadId}`
}

export function followTailKey(workspaceId: number, threadId: number): string {
  return `luban:follow_tail:${workspaceId}:${threadId}`
}

export function loadJson<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key)
    if (!raw) return null
    return JSON.parse(raw) as T
  } catch {
    return null
  }
}

export function saveJson(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // Ignore storage errors (private mode, blocked, etc.).
  }
}
