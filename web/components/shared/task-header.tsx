"use client"

import { MoreHorizontal, Star } from "lucide-react"
import { OpenButton } from "./open-button"

export interface ProjectInfo {
  /** Project name */
  name: string
  /** Tailwind background color class (e.g., "bg-violet-500") */
  color: string
}

interface TaskHeaderProps {
  /** Task title to display */
  title: string
  /** Workdir/branch label */
  workdir?: string
  /** Project information (name and color) */
  project?: ProjectInfo
  /** Callback when project breadcrumb is clicked */
  onProjectClick?: () => void
  /** Whether to show the full action buttons (star, open in vscode, etc.) */
  showFullActions?: boolean
  /** Custom actions to render on the right side */
  customActions?: React.ReactNode
}

/**
 * Project icon component - displays a colored square with the first letter
 * Exported for use in other components (e.g., TaskListView header)
 */
export function ProjectIcon({ name, color }: ProjectInfo) {
  return (
    <span
      className={`w-[14px] h-[14px] rounded-[3px] flex items-center justify-center text-[9px] font-semibold text-white flex-shrink-0 ${color}`}
    >
      {name.charAt(0).toUpperCase()}
    </span>
  )
}

/**
 * Shared header component for task/issue views
 * Used in TaskDetailView and InboxView preview panel
 */
export function TaskHeader({
  title,
  workdir,
  project,
  onProjectClick,
  showFullActions = false,
  customActions,
}: TaskHeaderProps) {
  return (
    <div
      className="flex items-center justify-between h-[39px] flex-shrink-0"
      style={{ padding: '0 20px', borderBottom: '1px solid #ebebeb' }}
    >
      {/* Left: Breadcrumb + Title + Badge */}
      <div className="flex items-center gap-1.5 min-w-0 flex-1">
        {/* Project breadcrumb (optional) */}
        {project && (
          <>
            <button
              onClick={onProjectClick}
              className="flex items-center gap-1 hover:opacity-70 transition-opacity flex-shrink-0"
            >
              <ProjectIcon name={project.name} color={project.color} />
              <span className="text-[13px] font-medium" style={{ color: '#1b1b1b' }}>
                {project.name}
              </span>
            </button>
            <span className="text-[13px] flex-shrink-0" style={{ color: '#9b9b9b' }}>›</span>
          </>
        )}

        {/* Task title */}
        <span className="text-[13px] font-medium truncate" style={{ color: '#1b1b1b' }}>
          {title || "Untitled Task"}
        </span>

        {/* Workdir badge */}
        {workdir && (
          <span
            className="text-[11px] px-1.5 py-0.5 rounded flex-shrink-0 ml-1"
            style={{ backgroundColor: '#f0f0f0', color: '#6b6b6b' }}
          >
            {workdir}
          </span>
        )}
      </div>

      {/* Right: Actions */}
      <div className="flex items-center gap-0.5 flex-shrink-0">
        {customActions}
        {showFullActions && (
          <>
            <OpenButton />
            <button
              className="w-6 h-6 flex items-center justify-center rounded-[5px] hover:bg-[#eeeeee] transition-colors"
              style={{ color: '#9b9b9b' }}
            >
              <Star className="w-4 h-4" />
            </button>
          </>
        )}
        <button
          className="w-6 h-6 flex items-center justify-center rounded-[5px] hover:bg-[#eeeeee] transition-colors"
          style={{ color: '#6b6b6b' }}
        >
          <MoreHorizontal className="w-4 h-4" />
        </button>
      </div>
    </div>
  )
}
