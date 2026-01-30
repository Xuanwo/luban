"use client"

import { useState } from "react"
import { ChatPanel } from "./chat-panel"
import { RightSidebar } from "./right-sidebar"
import { TaskHeader } from "./shared/task-header"
import type { ChangedFile } from "./right-sidebar"

interface TaskDetailViewProps {
  taskId?: string
  taskTitle?: string
  worktree?: string
  projectName?: string
  projectColor?: string
  onBack?: () => void
}

export function TaskDetailView({ taskId, taskTitle, worktree, projectName = "Luban", projectColor = "bg-violet-500", onBack }: TaskDetailViewProps) {
  const [rightSidebarWidthPx, setRightSidebarWidthPx] = useState(320)
  const [pendingDiffFile, setPendingDiffFile] = useState<ChangedFile | null>(null)

  function clamp(n: number, min: number, max: number) {
    return Math.round(Math.max(min, Math.min(max, n)))
  }

  function startResize(args: {
    pointerDownClientX: number
    initialRightSidebarWidthPx: number
  }) {
    const originalCursor = document.body.style.cursor
    const originalUserSelect = document.body.style.userSelect
    document.body.style.cursor = "col-resize"
    document.body.style.userSelect = "none"

    const onMove = (ev: PointerEvent) => {
      const dx = ev.clientX - args.pointerDownClientX
      setRightSidebarWidthPx(clamp(args.initialRightSidebarWidthPx - dx, 260, 640))
    }

    const onUp = () => {
      window.removeEventListener("pointermove", onMove)
      window.removeEventListener("pointerup", onUp)
      document.body.style.cursor = originalCursor
      document.body.style.userSelect = originalUserSelect
    }

    window.addEventListener("pointermove", onMove)
    window.addEventListener("pointerup", onUp, { once: true })
  }

  return (
    <div className="h-full flex">
      {/* Left: Main Content Area */}
      <div className="flex-1 min-w-0 flex flex-col">
        {/* Header / Breadcrumb */}
        <TaskHeader
          title={taskTitle || "Untitled Task"}
          worktree={worktree || "main"}
          project={{ name: projectName, color: projectColor }}
          onProjectClick={onBack}
          showFullActions
        />

        {/* Chat Panel */}
        <div className="flex-1 min-h-0 flex">
          <ChatPanel
            pendingDiffFile={pendingDiffFile}
            onDiffFileOpened={() => setPendingDiffFile(null)}
          />
        </div>
      </div>

      {/* Resizer */}
      <div className="relative w-0 flex-shrink-0">
        <div
          className="absolute -left-1 top-0 h-full w-2 bg-transparent hover:bg-border/60 active:bg-border cursor-col-resize z-10"
          title="Resize terminal"
          onPointerDown={(e) => {
            if (e.button !== 0) return
            e.preventDefault()
            startResize({
              pointerDownClientX: e.clientX,
              initialRightSidebarWidthPx: rightSidebarWidthPx,
            })
          }}
        />
      </div>

      {/* Right: Sidebar - Full Height */}
      <RightSidebar
        widthPx={rightSidebarWidthPx}
        onOpenDiffTab={(file) => setPendingDiffFile(file)}
      />
    </div>
  )
}
