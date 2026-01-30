"use client"

import { useState } from "react"
import { ChatPanel } from "./chat-panel"
import { RightSidebar } from "./right-sidebar"
import { TaskHeader } from "./shared/task-header"
import type { ChangedFile } from "./right-sidebar"
import { useLuban } from "@/lib/luban-context"
import { getActiveProjectInfo } from "@/lib/active-project-info"
import { projectColorClass } from "@/lib/project-colors"

interface TaskDetailViewProps {
  taskId?: string
  taskTitle?: string
  workdir?: string
  projectName?: string
  projectColor?: string
  onBack?: () => void
}

export function TaskDetailView({ taskId, taskTitle, workdir, projectName, projectColor, onBack }: TaskDetailViewProps) {
  const {
    app,
    activeWorkdirId: activeWorkspaceId,
    activeWorkdir: activeWorkspace,
    activeTaskId: activeThreadId,
    tasks: threads,
  } = useLuban()
  const [rightSidebarWidthPx, setRightSidebarWidthPx] = useState(320)
  const [pendingDiffFile, setPendingDiffFile] = useState<ChangedFile | null>(null)

  const projectInfo = getActiveProjectInfo(app, activeWorkspaceId)
  const resolvedProjectName = projectName ?? projectInfo.name
  const resolvedWorkdir = workdir ?? activeWorkspace?.branch_name ?? activeWorkspace?.workdir_name ?? "main"
  const resolvedTitle =
    taskTitle ??
    (activeThreadId != null ? threads.find((t) => t.task_id === activeThreadId)?.title : null) ??
    activeWorkspace?.workdir_name ??
    "Task"

  const resolvedProjectColor = (() => {
    if (projectColor) return projectColor
    if (!app || activeWorkspaceId == null) return "bg-violet-500"
    for (const p of app.projects) {
      if (p.workdirs.some((w) => w.id === activeWorkspaceId)) {
        return projectColorClass(p.id)
      }
    }
    return "bg-violet-500"
  })()

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
          title={resolvedTitle}
          workdir={resolvedWorkdir}
          project={{ name: resolvedProjectName, color: resolvedProjectColor }}
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
