"use client"

import { Plus, X } from "lucide-react"

import { cn } from "@/lib/utils"
import type { InboxFilters, InboxUpdatedFilterPreset } from "@/lib/inbox-filters"
import { DEFAULT_INBOX_FILTERS, inboxFiltersEqual, normalizeInboxFilters } from "@/lib/inbox-filters"
import { taskStatusConfig } from "@/components/shared/task-status-selector"

const UPDATED_LABELS: Record<InboxUpdatedFilterPreset, string> = {
  any: "Any time",
  "1w": "Last 1 week",
  "2w": "Last 2 weeks",
  "4w": "Last 4 weeks",
  "1m": "Last 1 month",
  "2m": "Last 2 months",
  "4m": "Last 4 months",
  "8m": "Last 8 months",
  "12m": "Last 12 months",
}

function arrayEqual(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) return false
  }
  return true
}

function Chip({ testId, label, onRemove }: { testId?: string; label: string; onRemove: () => void }) {
  return (
    <div
      data-testid={testId}
      className="h-7 inline-flex items-center gap-2 rounded-md border px-2"
      style={{ borderColor: "#ebebeb", backgroundColor: "#fcfcfc", color: "#1b1b1b" }}
    >
      <span className="text-[12px] font-medium truncate max-w-[220px]">{label}</span>
      <button
        type="button"
        className="w-5 h-5 rounded flex items-center justify-center hover:bg-[#eeeeee] transition-colors"
        style={{ color: "#9b9b9b" }}
        onClick={onRemove}
        aria-label="Remove filter"
        title="Remove filter"
      >
        <X className="w-3.5 h-3.5" />
      </button>
    </div>
  )
}

export function InboxFilterBar({
  filters,
  projectsById,
  onFiltersChange,
  onSaveView,
}: {
  filters: InboxFilters
  projectsById: Map<string, string>
  onFiltersChange: (next: InboxFilters) => void
  onSaveView: () => void
}) {
  const normalized = normalizeInboxFilters(filters)
  const defaultNormalized = normalizeInboxFilters(DEFAULT_INBOX_FILTERS)
  const filtersActive = !inboxFiltersEqual(normalized, defaultNormalized)

  const projectActive = normalized.projectIds.length > 0
  const statusActive = !arrayEqual(normalized.statuses, defaultNormalized.statuses)
  const updatedActive = normalized.updated !== defaultNormalized.updated

  if (!filtersActive) return null

  const projectLabel = projectActive
    ? normalized.projectIds.length === 1
      ? `Project is ${projectsById.get(normalized.projectIds[0] ?? "") ?? "Unknown"}`
      : `Project is ${normalized.projectIds.length} selected`
    : ""

  const statusLabel = statusActive
    ? normalized.statuses.length === 0
      ? "Status is none"
      : normalized.statuses.length <= 3
        ? `Status is ${normalized.statuses.map(s => taskStatusConfig[s]?.label ?? s).join(", ")}`
        : `Status is ${normalized.statuses.length} selected`
    : ""

  const updatedLabel = updatedActive ? `Updated is ${UPDATED_LABELS[normalized.updated] ?? normalized.updated}` : ""

  return (
    <div
      data-testid="inbox-filter-bar"
      className="flex items-center gap-2 px-3 py-2 border-b"
      style={{ borderColor: "#ebebeb" }}
    >
      <div className="flex-1 min-w-0 flex flex-wrap items-center gap-2">
        {projectActive ? (
          <Chip
            testId="inbox-filter-chip-project"
            label={projectLabel}
            onRemove={() => onFiltersChange({ ...filters, projectIds: [] })}
          />
        ) : null}
        {statusActive ? (
          <Chip
            testId="inbox-filter-chip-status"
            label={statusLabel}
            onRemove={() => onFiltersChange({ ...filters, statuses: [...DEFAULT_INBOX_FILTERS.statuses] })}
          />
        ) : null}
        {updatedActive ? (
          <Chip
            testId="inbox-filter-chip-updated"
            label={updatedLabel}
            onRemove={() => onFiltersChange({ ...filters, updated: DEFAULT_INBOX_FILTERS.updated })}
          />
        ) : null}
      </div>
      <button
        type="button"
        data-testid="inbox-save-view-button"
        className={cn("h-7 px-2.5 rounded-md border transition-colors flex items-center gap-1.5", "hover:bg-[#eeeeee]")}
        style={{ borderColor: "#ebebeb", color: "#6b6b6b" }}
        onClick={onSaveView}
        title="Save view"
      >
        <Plus className="w-3.5 h-3.5" />
        <span className="text-[12px] font-medium">Save view</span>
      </button>
    </div>
  )
}
