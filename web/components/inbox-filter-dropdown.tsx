"use client"

import type React from "react"

import { Check, ChevronRight } from "lucide-react"

import { cn } from "@/lib/utils"
import type { InboxFilters, InboxUpdatedFilterPreset } from "@/lib/inbox-filters"
import { normalizeInboxFilters } from "@/lib/inbox-filters"
import { taskStatuses, TaskStatusIcon } from "@/components/shared/task-status-selector"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

export type InboxFilterProjectOption = {
  id: string
  name: string
  avatarUrl: string
  fallbackAvatarUrl: string
}

const UPDATED_PRESETS: Array<{ id: InboxUpdatedFilterPreset; label: string }> = [
  { id: "any", label: "Any time" },
  { id: "1w", label: "Last 1 week" },
  { id: "2w", label: "Last 2 weeks" },
  { id: "4w", label: "Last 4 weeks" },
  { id: "1m", label: "Last 1 month" },
  { id: "2m", label: "Last 2 months" },
  { id: "4m", label: "Last 4 months" },
  { id: "8m", label: "Last 8 months" },
  { id: "12m", label: "Last 12 months" },
]

function SubHeader({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-2 py-1 text-[11px] font-medium" style={{ color: "#9b9b9b" }}>
      {children}
    </div>
  )
}

function MenuRow({
  selected,
  onSelect,
  testId,
  keepOpen,
  children,
}: {
  selected: boolean
  onSelect: () => void
  testId?: string
  keepOpen?: boolean
  children: React.ReactNode
}) {
  return (
    <DropdownMenuItem
      data-testid={testId}
      className="flex items-center gap-2 px-2 py-1.5 rounded-md cursor-pointer"
      style={{ color: "#1b1b1b" }}
      onSelect={e => {
        if (keepOpen) e.preventDefault()
        onSelect()
      }}
    >
      <span className="flex-1 min-w-0 flex items-center gap-2">{children}</span>
      {selected ? <Check className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "#5e6ad2" }} /> : null}
    </DropdownMenuItem>
  )
}

function SubmenuTrigger({ children, testId }: { children: React.ReactNode; testId?: string }) {
  return (
    <DropdownMenuSubTrigger
      data-testid={testId}
      className={cn(
        "flex items-center gap-2 px-2 py-1.5 rounded-md cursor-pointer text-[13px]",
        "focus:bg-[#f5f5f5] data-[state=open]:bg-[#f5f5f5]"
      )}
      style={{ color: "#1b1b1b" }}
    >
      <span className="flex-1 min-w-0 truncate">{children}</span>
      <ChevronRight className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "#9b9b9b" }} />
    </DropdownMenuSubTrigger>
  )
}

function SubmenuContent({ children }: { children: React.ReactNode }) {
  return (
    <DropdownMenuPortal>
      <DropdownMenuSubContent
        sideOffset={6}
        alignOffset={-4}
        className="z-50 min-w-[240px] rounded-lg border bg-white p-1.5 shadow-[0_4px_16px_rgba(0,0,0,0.12)]"
        style={{ borderColor: "#e5e5e5" }}
      >
        {children}
      </DropdownMenuSubContent>
    </DropdownMenuPortal>
  )
}

export function InboxFilterDropdown({
  filters,
  projects,
  onFiltersChange,
  onClear,
  children,
}: {
  filters: InboxFilters
  projects: InboxFilterProjectOption[]
  onFiltersChange: (next: InboxFilters) => void
  onClear: () => void
  children: React.ReactNode
}) {
  const selectedProjects = new Set(filters.projectIds)
  const selectedStatuses = new Set(filters.statuses)

  const applyFilters = (next: InboxFilters) => {
    onFiltersChange(normalizeInboxFilters(next))
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>{children}</DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        sideOffset={6}
        className="w-[240px] rounded-lg border-[#e5e5e5] bg-white shadow-[0_4px_16px_rgba(0,0,0,0.12)] p-1.5"
        data-testid="inbox-filter-menu"
      >
        <SubHeader>Add filter...</SubHeader>

        <DropdownMenuSub>
          <SubmenuTrigger testId="inbox-filter-sub-project">Project</SubmenuTrigger>
          <SubmenuContent>
            <SubHeader>Project</SubHeader>
            <MenuRow
              selected={filters.projectIds.length === 0}
              keepOpen
              testId="inbox-filter-project-all"
              onSelect={() => applyFilters({ ...filters, projectIds: [] })}
            >
              <span className="text-[13px]" style={{ color: "#6b6b6b" }}>
                All projects
              </span>
            </MenuRow>
            <DropdownMenuSeparator className="my-1" />
            {projects.map(p => {
              const selected = selectedProjects.has(p.id)
              return (
                <MenuRow
                  key={p.id}
                  selected={selected}
                  keepOpen
                  testId={`inbox-filter-project-${p.id}`}
                  onSelect={() => {
                    if (selected) {
                      applyFilters({ ...filters, projectIds: filters.projectIds.filter(id => id !== p.id) })
                      return
                    }
                    const nextIds = filters.projectIds.length === 0 ? [p.id] : [...filters.projectIds, p.id]
                    applyFilters({ ...filters, projectIds: nextIds })
                  }}
                >
                  <img
                    src={p.avatarUrl}
                    alt=""
                    aria-hidden="true"
                    className="w-[14px] h-[14px] rounded-[3px] flex-shrink-0"
                    loading="lazy"
                    decoding="async"
                    onError={e => {
                      const img = e.currentTarget
                      if (img.src !== p.fallbackAvatarUrl) img.src = p.fallbackAvatarUrl
                    }}
                  />
                  <span className="text-[13px] truncate">{p.name}</span>
                </MenuRow>
              )
            })}
          </SubmenuContent>
        </DropdownMenuSub>

        <DropdownMenuSub>
          <SubmenuTrigger testId="inbox-filter-sub-status">Status</SubmenuTrigger>
          <SubmenuContent>
            <SubHeader>Status</SubHeader>
            {taskStatuses.map(s => {
              const selected = selectedStatuses.has(s.id)
              return (
                <MenuRow
                  key={s.id}
                  selected={selected}
                  keepOpen
                  testId={`inbox-filter-status-${s.id}`}
                  onSelect={() => {
                    const next = selected ? filters.statuses.filter(id => id !== s.id) : [...filters.statuses, s.id]
                    applyFilters({ ...filters, statuses: next })
                  }}
                >
                  <TaskStatusIcon status={s.id} size="xs" />
                  <span className="text-[13px] truncate">{s.label}</span>
                </MenuRow>
              )
            })}
          </SubmenuContent>
        </DropdownMenuSub>

        <DropdownMenuSub>
          <SubmenuTrigger testId="inbox-filter-sub-updated">Updated</SubmenuTrigger>
          <SubmenuContent>
            <SubHeader>Updated</SubHeader>
            {UPDATED_PRESETS.map(preset => {
              const selected = filters.updated === preset.id
              return (
                <MenuRow
                  key={preset.id}
                  selected={selected}
                  testId={`inbox-filter-updated-${preset.id}`}
                  onSelect={() => applyFilters({ ...filters, updated: preset.id })}
                >
                  <span className="text-[13px] truncate">{preset.label}</span>
                </MenuRow>
              )
            })}
          </SubmenuContent>
        </DropdownMenuSub>

        <DropdownMenuSeparator className="my-1" />
        <DropdownMenuItem
          data-testid="inbox-filter-clear"
          className="flex items-center gap-2 px-2 py-1.5 rounded-md cursor-pointer"
          style={{ color: "#6b6b6b" }}
          onSelect={() => onClear()}
        >
          Clear filters
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
