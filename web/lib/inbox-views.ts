"use client"

import type { InboxFilters } from "./inbox-filters"
import { normalizeInboxFilters } from "./inbox-filters"
import { INBOX_VIEWS_KEY, loadJson, saveJson } from "./ui-prefs"

export type InboxSavedView = {
  id: string
  name: string
  filters: InboxFilters
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}

function isString(value: unknown): value is string {
  return typeof value === "string"
}

function coerceInboxFilters(value: unknown): InboxFilters | null {
  if (!isRecord(value)) return null
  const projectIds = Array.isArray(value.projectIds) ? value.projectIds.filter(isString) : []
  const statuses = Array.isArray(value.statuses) ? value.statuses.filter(isString) : []
  const updated = isString(value.updated) ? value.updated : "any"
  return normalizeInboxFilters({
    projectIds,
    statuses: statuses as InboxFilters["statuses"],
    updated: updated as InboxFilters["updated"],
  })
}

export function loadInboxViews(): InboxSavedView[] {
  const raw = loadJson<unknown>(INBOX_VIEWS_KEY)
  if (!Array.isArray(raw)) return []
  const out: InboxSavedView[] = []
  for (const item of raw) {
    if (!isRecord(item)) continue
    if (!isString(item.id) || !isString(item.name)) continue
    const filters = coerceInboxFilters(item.filters)
    if (!filters) continue
    out.push({ id: item.id, name: item.name, filters })
  }
  return out
}

export function saveInboxViews(views: InboxSavedView[]): void {
  saveJson(
    INBOX_VIEWS_KEY,
    views.map(v => ({ ...v, filters: normalizeInboxFilters(v.filters) }))
  )
}

export function nextDefaultInboxViewName(existing: InboxSavedView[]): string {
  let next = 1
  for (const view of existing) {
    const m = /^View\s+(\d+)$/.exec(view.name.trim())
    if (!m) continue
    const n = Number(m[1])
    if (Number.isFinite(n) && n >= next) next = n + 1
  }
  return `View ${next}`
}
