"use client"

import { useEffect, useState } from "react"
import { TaskActivityPanel } from "./task-activity-panel"
import { TaskHeader } from "./shared/task-header"
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
    <div className="h-full flex flex-col">
      <TaskHeader
        title={resolvedTitle}
        workdir={resolvedWorkdir}
        project={{ name: resolvedProjectName, color: resolvedProjectColor, avatarUrl: resolvedProjectAvatarUrl }}
        onProjectClick={onBack}
        showFullActions
        isStarred={isStarred}
        onDelete={() => {
          if (activeWorkspaceId == null || activeThreadId == null) return
          void deleteTask(activeThreadId)
          onBack?.()
        }}
        onToggleStar={(nextStarred) => {
          if (activeWorkspaceId == null || activeThreadId == null) return
          setTaskStarred(activeWorkspaceId, activeThreadId, nextStarred)
          setIsStarred(nextStarred)
        }}
      />

      <div className="flex-1 min-h-0 flex">
        <TaskActivityPanel />
      </div>
    </div>
  )
}
