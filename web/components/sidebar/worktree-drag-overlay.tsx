"use client"

import { cn } from "@/lib/utils"
import type { SidebarWorktreeVm } from "@/lib/sidebar-view-model"
import { AgentStatusIcon } from "@/components/shared/status-indicator"

interface WorktreeDragOverlayProps {
  worktree: SidebarWorktreeVm
}

export function WorktreeDragOverlay({ worktree }: WorktreeDragOverlayProps) {
  return (
    <div
      className={cn(
        "bg-sidebar border border-border rounded-md shadow-xl",
        "ring-2 ring-primary/30",
        "px-3 py-2",
      )}
    >
      <div className="flex items-center gap-2">
        <AgentStatusIcon status={worktree.agentStatus} size="sm" />
        <div className="flex flex-col min-w-0">
          <span className="text-xs text-foreground truncate max-w-[160px]">
            {worktree.name}
          </span>
          <span className="text-[10px] text-muted-foreground/60 font-mono truncate">
            {worktree.worktreeName}
          </span>
        </div>
      </div>
    </div>
  )
}
