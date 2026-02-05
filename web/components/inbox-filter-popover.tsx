'use client'

import type React from 'react'

import { useEffect, useMemo, useState } from 'react'
import { Check, ChevronDown, ChevronRight, RotateCcw, X } from 'lucide-react'
import * as Popover from '@radix-ui/react-popover'

import { cn } from '@/lib/utils'
import type { InboxFilters, InboxUpdatedFilterPreset } from '@/lib/inbox-filters'
import { taskStatuses, TaskStatusIcon } from '@/components/shared/task-status-selector'

export type InboxFilterProjectOption = {
	id: string
	name: string
	avatarUrl: string
	fallbackAvatarUrl: string
}

const UPDATED_PRESETS: Array<{ id: InboxUpdatedFilterPreset; label: string }> = [
	{ id: 'any', label: 'Any time' },
	{ id: '1w', label: 'Last 1 week' },
	{ id: '2w', label: 'Last 2 weeks' },
	{ id: '4w', label: 'Last 4 weeks' },
	{ id: '1m', label: 'Last 1 month' },
	{ id: '2m', label: 'Last 2 months' },
	{ id: '4m', label: 'Last 4 months' },
	{ id: '8m', label: 'Last 8 months' },
	{ id: '12m', label: 'Last 12 months' },
]

function CollapsibleSection({
	sectionId,
	title,
	summary,
	open,
	onOpenChange,
	children,
}: {
	sectionId: string
	title: string
	summary: string
	open: boolean
	onOpenChange: (open: boolean) => void
	children: React.ReactNode
}) {
	return (
		<div className="flex flex-col">
			<button
				type="button"
				data-testid={`inbox-filter-section-${sectionId}-toggle`}
				aria-expanded={open}
				onClick={() => onOpenChange(!open)}
				className="w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-[12px] transition-colors hover:bg-[#f5f5f5]"
				style={{ color: '#1b1b1b' }}
			>
				<span className="flex items-center justify-center w-4 h-4 flex-shrink-0" style={{ color: '#9b9b9b' }}>
					{open ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
				</span>
				<span className="font-medium">{title}</span>
				<span className="min-w-0 truncate ml-auto text-[12px]" style={{ color: '#9b9b9b' }}>
					{summary}
				</span>
			</button>
			{open ? <div className="flex flex-col gap-0.5 mt-0.5">{children}</div> : null}
		</div>
	)
}

function OptionRow({ selected, onClick, testId, children }: { selected: boolean; onClick: () => void; testId?: string; children: React.ReactNode }) {
	return (
		<button
			type="button"
			data-testid={testId}
			className={cn('w-full flex items-center gap-1.5 rounded-md px-2 py-1.5 text-[13px] transition-colors', 'hover:bg-[#f5f5f5]')}
			style={{ color: '#1b1b1b' }}
			onClick={onClick}
		>
			<span className="min-w-0 flex-1 flex items-center gap-1.5">{children}</span>
			{selected ? <Check className="w-3.5 h-3.5 flex-shrink-0" style={{ color: '#5e6ad2' }} /> : null}
		</button>
	)
}

export function InboxFilterPopover({
	open,
	onOpenChange,
	projects,
	filters,
	onFiltersChange,
	onClear,
	children,
}: {
	open: boolean
	onOpenChange: (open: boolean) => void
	projects: InboxFilterProjectOption[]
	filters: InboxFilters
	onFiltersChange: (filters: InboxFilters) => void
	onClear: () => void
	children: React.ReactNode
}) {
	const selectedProjects = new Set(filters.projectIds)
	const allProjectsSelected = filters.projectIds.length === 0
	const selectedStatuses = new Set(filters.statuses)

	const [projectOpen, setProjectOpen] = useState(false)
	const [statusOpen, setStatusOpen] = useState(false)
	const [updatedOpen, setUpdatedOpen] = useState(false)

	useEffect(() => {
		if (!open) return
		setProjectOpen(false)
		setStatusOpen(false)
		setUpdatedOpen(false)
	}, [open])

	const projectSummary = useMemo(() => {
		if (allProjectsSelected) return 'All projects'
		if (filters.projectIds.length === 1) {
			const id = filters.projectIds[0]
			const match = id ? projects.find((p) => p.id === id) : null
			if (match) return match.name
		}
		return `${filters.projectIds.length} selected`
	}, [allProjectsSelected, filters.projectIds, projects])

	const statusSummary = useMemo(() => {
		const selectedCount = filters.statuses.length
		if (selectedCount === 0) return 'None'
		if (selectedCount === taskStatuses.length) return 'All statuses'
		const doneExcluded = !filters.statuses.includes('done') && taskStatuses.some((s) => s.id === 'done')
		if (doneExcluded && selectedCount === taskStatuses.length - 1) return 'All except Done'
		return `${selectedCount} selected`
	}, [filters.statuses])

	const updatedSummary = useMemo(() => {
		return UPDATED_PRESETS.find((p) => p.id === filters.updated)?.label ?? 'Any time'
	}, [filters.updated])

	return (
		<Popover.Root open={open} onOpenChange={onOpenChange}>
			<Popover.Trigger asChild>{children}</Popover.Trigger>
			<Popover.Portal>
				<Popover.Content
					sideOffset={6}
					align="end"
					className={cn('z-50 w-[320px] rounded-lg border bg-white p-2.5 shadow-[0_4px_16px_rgba(0,0,0,0.12)]')}
					style={{ borderColor: '#e5e5e5' }}
					data-testid="inbox-filter-popover"
				>
					<div className="flex items-center justify-between mb-2">
						<div className="text-[13px] font-medium" style={{ color: '#1b1b1b' }}>
							Filter
						</div>
						<div className="flex items-center gap-1">
							<button
								type="button"
								data-testid="inbox-filter-clear"
								className="h-7 px-2 rounded-md hover:bg-[#f5f5f5] transition-colors flex items-center gap-1.5"
								style={{ color: '#6b6b6b' }}
								onClick={onClear}
							>
								<RotateCcw className="w-3.5 h-3.5" />
								<span className="text-[12px] font-medium">Clear</span>
							</button>
							<Popover.Close asChild>
								<button
									type="button"
									data-testid="inbox-filter-close"
									className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-[#f5f5f5] transition-colors"
									style={{ color: '#9b9b9b' }}
									title="Close"
								>
									<X className="w-4 h-4" />
								</button>
							</Popover.Close>
						</div>
					</div>

					<div className="flex flex-col gap-2">
						<CollapsibleSection sectionId="project" title="Project" summary={projectSummary} open={projectOpen} onOpenChange={setProjectOpen}>
							<OptionRow
								selected={allProjectsSelected}
								onClick={() => onFiltersChange({ ...filters, projectIds: [] })}
								testId="inbox-filter-project-all"
							>
								<span className="text-[12px]" style={{ color: '#6b6b6b' }}>
									All projects
								</span>
							</OptionRow>
							{projects.map((p) => {
								const selected = selectedProjects.has(p.id)
								return (
									<OptionRow
										key={p.id}
										selected={selected}
										onClick={() => {
											if (selected) {
												const nextIds = filters.projectIds.filter((id) => id !== p.id)
												onFiltersChange({ ...filters, projectIds: nextIds })
												return
											}
											const nextIds = allProjectsSelected ? [p.id] : [...filters.projectIds, p.id]
											onFiltersChange({ ...filters, projectIds: nextIds })
										}}
										testId={`inbox-filter-project-${p.id}`}
									>
										<img
											src={p.avatarUrl}
											alt=""
											aria-hidden="true"
											className="w-[14px] h-[14px] rounded-[3px] flex-shrink-0"
											loading="lazy"
											decoding="async"
											onError={(e) => {
												const img = e.currentTarget
												if (img.src !== p.fallbackAvatarUrl) img.src = p.fallbackAvatarUrl
											}}
										/>
										<span className="truncate">{p.name}</span>
									</OptionRow>
								)
							})}
						</CollapsibleSection>

						<CollapsibleSection sectionId="status" title="Status" summary={statusSummary} open={statusOpen} onOpenChange={setStatusOpen}>
							{taskStatuses.map((s) => {
								const selected = selectedStatuses.has(s.id)
								return (
									<OptionRow
										key={s.id}
										selected={selected}
										onClick={() => {
											const next = selected ? filters.statuses.filter((id) => id !== s.id) : [...filters.statuses, s.id]
											onFiltersChange({ ...filters, statuses: next })
										}}
										testId={`inbox-filter-status-${s.id}`}
									>
										<TaskStatusIcon status={s.id} size="xs" />
										<span className="truncate">{s.label}</span>
									</OptionRow>
								)
							})}
						</CollapsibleSection>

						<CollapsibleSection sectionId="updated" title="Updated" summary={updatedSummary} open={updatedOpen} onOpenChange={setUpdatedOpen}>
							{UPDATED_PRESETS.map((preset) => {
								const selected = filters.updated === preset.id
								return (
									<OptionRow
										key={preset.id}
										selected={selected}
										onClick={() => onFiltersChange({ ...filters, updated: preset.id })}
										testId={`inbox-filter-updated-${preset.id}`}
									>
										<span className="truncate">{preset.label}</span>
									</OptionRow>
								)
							})}
						</CollapsibleSection>
					</div>
				</Popover.Content>
			</Popover.Portal>
		</Popover.Root>
	)
}
