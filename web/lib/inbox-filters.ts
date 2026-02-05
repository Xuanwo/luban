import type { TaskStatus } from "./luban-api"

export type InboxUpdatedFilterPreset = "any" | "1w" | "2w" | "4w" | "1m" | "2m" | "4m" | "8m" | "12m"

export type InboxFilters = {
  projectIds: string[]
  statuses: TaskStatus[]
  updated: InboxUpdatedFilterPreset
}

export const DEFAULT_INBOX_FILTERS: InboxFilters = {
  projectIds: [],
  statuses: ["backlog", "todo", "iterating", "validating", "canceled"],
  updated: "any",
}

export type InboxFilterableItem = {
  projectId: string
  status: TaskStatus
  updatedAtUnixSeconds: number
}

function uniqueSorted(values: readonly string[]): string[] {
  return Array.from(new Set(values)).sort()
}

function setEqual(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) return false
  }
  return true
}

export function inboxFiltersEqual(a: InboxFilters, b: InboxFilters): boolean {
  return (
    a.updated === b.updated &&
    setEqual(uniqueSorted(a.projectIds), uniqueSorted(b.projectIds)) &&
    setEqual(uniqueSorted(a.statuses), uniqueSorted(b.statuses))
  )
}

function thresholdUnixSeconds(preset: InboxUpdatedFilterPreset, nowUnixSeconds: number): number | null {
  if (preset === "any") return null

  if (preset.endsWith("w")) {
    const weeks = Number(preset.slice(0, -1))
    if (!Number.isFinite(weeks) || weeks <= 0) return null
    return nowUnixSeconds - weeks * 7 * 24 * 60 * 60
  }

  if (preset.endsWith("m")) {
    const months = Number(preset.slice(0, -1))
    if (!Number.isFinite(months) || months <= 0) return null
    const date = new Date(nowUnixSeconds * 1000)
    date.setMonth(date.getMonth() - months)
    return Math.floor(date.getTime() / 1000)
  }

  return null
}

export function applyInboxFilters<T extends InboxFilterableItem>(
  items: readonly T[],
  filters: InboxFilters,
  nowUnixSeconds: number
): T[] {
  const threshold = thresholdUnixSeconds(filters.updated, nowUnixSeconds)
  const projectAllow = new Set(filters.projectIds)
  const statusAllow = new Set(filters.statuses)

  return items.filter(item => {
    if (filters.projectIds.length > 0 && !projectAllow.has(item.projectId)) return false
    if (!statusAllow.has(item.status)) return false
    if (threshold != null && item.updatedAtUnixSeconds < threshold) return false
    return true
  })
}
