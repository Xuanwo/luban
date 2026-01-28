"use client"

import { cn } from "@/lib/utils"
import type { SidebarProjectVm } from "@/lib/sidebar-view-model"
import { Layers } from "lucide-react"

interface ProjectDragOverlayProps {
  project: SidebarProjectVm
}

export function ProjectDragOverlay({ project }: ProjectDragOverlayProps) {
  return (
    <div
      className={cn(
        "bg-sidebar border border-border rounded-md shadow-xl",
        "ring-2 ring-primary/30",
        "px-3 py-2",
      )}
    >
      <div className="flex items-center gap-2">
        <div className="flex items-center justify-center w-5 h-5 rounded bg-secondary">
          <Layers className="w-3 h-3 text-foreground" />
        </div>
        <span className="text-sm font-medium text-foreground truncate max-w-[180px]">
          {project.displayName}
        </span>
        {project.worktrees.length > 0 && (
          <span className="text-xs text-muted-foreground">
            ({project.worktrees.length} worktree{project.worktrees.length !== 1 ? "s" : ""})
          </span>
        )}
      </div>
    </div>
  )
}
