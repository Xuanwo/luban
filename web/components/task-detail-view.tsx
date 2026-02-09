"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { MoreHorizontal, Star, Trash2 } from "lucide-react"
import { TaskDocumentPanel } from "./task-document-panel"
import { TaskWorkspacePanel } from "./task-workspace-panel"
import { TaskHeader } from "./shared/task-header"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { useLuban } from "@/lib/luban-context"
import { getActiveProjectInfo } from "@/lib/active-project-info"
import { projectColorClass } from "@/lib/project-colors"
import { buildSidebarProjects } from "@/lib/sidebar-view-model"
import { fetchTasks } from "@/lib/luban-http"

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
    setTaskStarred,
    deleteTask,
  } = useLuban()
  const [isStarred, setIsStarred] = useState(false)
  const [leftWidthPercent, setLeftWidthPercent] = useState(55)
  const [isDragging, setIsDragging] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  const handlePointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    setIsDragging(true)
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  }, [])

  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!isDragging || !containerRef.current) return
      const rect = containerRef.current.getBoundingClientRect()
      const x = e.clientX - rect.left
      const pct = Math.min(75, Math.max(30, (x / rect.width) * 100))
      setLeftWidthPercent(pct)
    },
    [isDragging],
  )

  const handlePointerUp = useCallback(() => {
    setIsDragging(false)
  }, [])

  const projectInfo = getActiveProjectInfo(app, activeWorkspaceId)
  const resolvedProjectName = projectName ?? projectInfo.name
  const resolvedWorkdir = workdir ?? activeWorkspace?.branch_name ?? activeWorkspace?.workdir_name ?? "main"
  const resolvedTitle =
    taskTitle ??
    (activeThreadId != null ? threads.find((t) => t.task_id === activeThreadId)?.title : null) ??
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

  const resolvedProjectAvatarUrl = (() => {
    if (!app || activeWorkspaceId == null) return undefined
    const projectId = (() => {
      for (const p of app.projects) {
        if (p.workdirs.some((w) => w.id === activeWorkspaceId)) return p.id
      }
      return null
    })()
    if (!projectId) return undefined
    return buildSidebarProjects(app).find((p) => p.id === projectId)?.avatarUrl
  })()

  useEffect(() => {
    if (!app || activeWorkspaceId == null || activeThreadId == null) {
      setIsStarred(false)
      return
    }

    const projectPath = (() => {
      for (const p of app.projects) {
        if (p.workdirs.some((w) => w.id === activeWorkspaceId)) return p.path
      }
      return null
    })()

    let cancelled = false
    void (async () => {
      try {
        const snap = await fetchTasks(projectPath ? { projectId: projectPath, workdirStatus: "all" } : { workdirStatus: "all" })
        if (cancelled) return
        const found =
          snap.tasks.find((t) => t.workdir_id === activeWorkspaceId && t.task_id === activeThreadId) ?? null
        setIsStarred(found?.is_starred ?? false)
      } catch (err) {
        console.warn("fetchTasks failed", err)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [app, activeThreadId, activeWorkspaceId])

  return (
    <div ref={containerRef} className="h-full flex flex-col md:flex-row">
      {/* Left column: header + documents */}
      <div
        className="h-[48%] md:h-full min-h-0 flex flex-col"
        style={{ width: `${leftWidthPercent}%` }}
      >
        <TaskHeader
          title={resolvedTitle}
          workdir={resolvedWorkdir}
          project={{ name: resolvedProjectName, color: resolvedProjectColor, avatarUrl: resolvedProjectAvatarUrl }}
          onProjectClick={onBack}
          customActions={
            <div className="flex items-center gap-0.5">
              <button
                data-testid="task-star-button"
                className="w-6 h-6 flex items-center justify-center rounded-[4px] hover:bg-[#eeeeee] transition-colors"
                style={{ color: isStarred ? '#f2c94c' : '#9b9b9b' }}
                title={isStarred ? "Unstar" : "Star"}
                aria-pressed={isStarred}
                onClick={() => {
                  if (activeWorkspaceId == null || activeThreadId == null) return
                  setTaskStarred(activeWorkspaceId, activeThreadId, !isStarred)
                  setIsStarred(!isStarred)
                }}
              >
                <Star className="w-3.5 h-3.5" fill={isStarred ? '#f2c94c' : 'none'} />
              </button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button
                    className="w-6 h-6 flex items-center justify-center rounded-[4px] hover:bg-[#eeeeee] transition-colors"
                    style={{ color: '#6b6b6b' }}
                  >
                    <MoreHorizontal className="w-3.5 h-3.5" />
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem
                    className="text-red-600 focus:text-red-600 focus:bg-red-50"
                    onClick={() => {
                      if (activeWorkspaceId == null || activeThreadId == null) return
                      void deleteTask(activeThreadId)
                      onBack?.()
                    }}
                  >
                    <Trash2 className="w-3.5 h-3.5 mr-1.5" />
                    Delete
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          }
        />
        <div className="flex-1 min-h-0">
          <TaskDocumentPanel />
        </div>
      </div>

      {/* Resizer */}
      <div
        className="hidden md:flex items-center justify-center flex-shrink-0"
        style={{
          width: '6px',
          cursor: 'col-resize',
          userSelect: 'none',
        }}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
      >
        <div
          style={{
            width: isDragging ? '2px' : '1px',
            height: '100%',
            backgroundColor: isDragging ? '#5e6ad2' : 'transparent',
            transition: isDragging ? 'none' : 'background-color 0.15s',
          }}
          onMouseEnter={(e) => {
            if (!isDragging) e.currentTarget.style.backgroundColor = '#5e6ad2'
            e.currentTarget.style.width = '2px'
          }}
          onMouseLeave={(e) => {
            if (!isDragging) {
              e.currentTarget.style.backgroundColor = 'transparent'
              e.currentTarget.style.width = '1px'
            }
          }}
        />
      </div>

      {/* Right column: workspace panel (tab bar is its own header) */}
      <div
        className="flex-1 min-w-0 min-h-0"
        style={isDragging ? { pointerEvents: 'none' } : undefined}
      >
        <TaskWorkspacePanel />
      </div>
    </div>
  )
}
