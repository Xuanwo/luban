"use client"

import { useEffect, useState } from "react"
import { MoreHorizontal, Star, Trash2 } from "lucide-react"
import { OpenButton } from "./open-button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

export interface ProjectInfo {
  /** Project name */
  name: string
  /** Tailwind background color class (e.g., "bg-violet-500") */
  color: string
  /** Optional project avatar URL (e.g., git provider avatar). */
  avatarUrl?: string
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
  /** Whether the task is starred */
  isStarred?: boolean
  /** Called with the next starred state when toggled */
  onToggleStar?: (nextStarred: boolean) => void
  /** Called when the user confirms deletion from the "..." menu */
  onDelete?: () => void
  /** Custom actions to render on the right side */
  customActions?: React.ReactNode
  /** Optional action bar rendered below the header row */
  actionBar?: React.ReactNode
}

/**
 * Project icon component - displays an avatar image when available, otherwise a colored square with the first letter.
 * Exported for use in other components (e.g., TaskListView header)
 */
export function ProjectIcon({ name, color, avatarUrl, testId }: ProjectInfo & { testId?: string }) {
  const [avatarLoadFailed, setAvatarLoadFailed] = useState(false)

  useEffect(() => {
    setAvatarLoadFailed(false)
  }, [avatarUrl])

  if (avatarUrl && !avatarLoadFailed) {
    return (
      // eslint-disable-next-line @next/next/no-img-element
      <img
        data-testid={testId}
        src={avatarUrl}
        alt=""
        aria-hidden="true"
        width={14}
        height={14}
        className="w-[14px] h-[14px] rounded-[3px] overflow-hidden flex-shrink-0"
        onError={() => setAvatarLoadFailed(true)}
      />
    )
  }

  return (
    <span
      data-testid={testId}
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
  isStarred = false,
  onToggleStar,
  onDelete,
  customActions,
  actionBar,
}: TaskHeaderProps) {
  return (
    <div className="flex flex-col flex-shrink-0" style={{ borderBottom: "1px solid #ebebeb" }}>
      <div className="flex items-center justify-between h-[39px]" style={{ padding: "0 20px" }}>
        {/* Left: Breadcrumb + Title + Badge + Star + Settings */}
        <div className="flex items-center gap-1.5 min-w-0 flex-1">
          {/* Project breadcrumb (optional) */}
          {project && (
            <>
              <button
                onClick={onProjectClick}
                className="flex items-center gap-1 hover:opacity-70 transition-opacity flex-shrink-0"
              >
                <ProjectIcon name={project.name} color={project.color} avatarUrl={project.avatarUrl} />
                <span className="text-[13px] font-medium" style={{ color: "#1b1b1b" }}>
                  {project.name}
                </span>
              </button>
              <span className="text-[13px] flex-shrink-0" style={{ color: "#9b9b9b" }}>
                ›
              </span>
            </>
          )}

          {/* Task title */}
          <span
            data-testid="task-header-title"
            className="text-[13px] font-medium truncate"
            style={{ color: "#1b1b1b" }}
          >
            {title || "Untitled Task"}
          </span>

          {/* Workdir badge */}
          {workdir && (
            <span
              className="text-[11px] px-1.5 py-0.5 rounded flex-shrink-0 ml-1"
              style={{ backgroundColor: "#f0f0f0", color: "#6b6b6b" }}
            >
              {workdir}
            </span>
          )}

          {/* Star and Settings buttons - next to title */}
          {showFullActions && (
            <div className="flex items-center gap-0.5 ml-1 flex-shrink-0">
              <button
                data-testid="task-star-button"
                className="w-6 h-6 flex items-center justify-center rounded-[5px] hover:bg-[#eeeeee] transition-colors"
                style={{ color: isStarred ? "#f2c94c" : "#9b9b9b" }}
                title={isStarred ? "Unstar" : "Star"}
                aria-pressed={isStarred}
                onClick={() => onToggleStar?.(!isStarred)}
              >
                <Star className="w-4 h-4" fill={isStarred ? "#f2c94c" : "none"} />
              </button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button
                    className="w-6 h-6 flex items-center justify-center rounded-[5px] hover:bg-[#eeeeee] transition-colors"
                    style={{ color: "#6b6b6b" }}
                  >
                    <MoreHorizontal className="w-4 h-4" />
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start">
                  <DropdownMenuItem
                    className="text-red-600 focus:text-red-600 focus:bg-red-50"
                    onClick={onDelete}
                  >
                    <Trash2 className="w-3.5 h-3.5 mr-1.5" />
                    Delete
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          )}
        </div>

        {/* Right: Open button */}
        <div className="flex items-center gap-0.5 flex-shrink-0">
          {customActions}
          {showFullActions && <OpenButton />}
        </div>
      </div>

      {actionBar ? (
        <div
          data-testid="task-header-action-bar"
          className="flex items-center min-w-0 h-[34px]"
          style={{ padding: "0 20px" }}
        >
          {actionBar}
        </div>
      ) : null}
    </div>
  )
}
