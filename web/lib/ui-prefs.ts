"use client"

export const ACTIVE_WORKSPACE_KEY = "luban:active_workspace_id"

export const RIGHT_SIDEBAR_OPEN_KEY = "luban:ui:right_sidebar_open"
export const VIEW_MODE_KEY = "luban:ui:view_mode"
export const SIDEBAR_WIDTH_KEY = "luban:ui:sidebar_width_px"
export const RIGHT_SIDEBAR_WIDTH_KEY = "luban:ui:right_sidebar_width_px"
export const GLOBAL_ZOOM_KEY = "luban:ui:global_zoom"
export const PROJECT_ORDER_KEY = "luban:ui:project_order"

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
  const raw = localStorage.getItem(key)
  if (!raw) return null
  try {
    return JSON.parse(raw) as T
  } catch {
    return null
  }
}

export function saveJson(key: string, value: unknown) {
  localStorage.setItem(key, JSON.stringify(value))
}
