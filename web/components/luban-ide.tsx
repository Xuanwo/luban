"use client"

import { useEffect, useMemo, useState } from "react"
import { LubanLayout } from "./luban-layout"
import { LubanSidebar, type NavView } from "./luban-sidebar"
import { TaskListView, Task } from "./task-list-view"
import { TaskDetailView } from "./task-detail-view"
import { InboxView, type InboxNotification } from "./inbox-view"
import { SettingsPanel } from "./settings-panel"
import { NewTaskModal } from "./new-task-modal"
import { NewTaskDraftsDialog } from "./new-task-drafts-dialog"
import { useLuban } from "@/lib/luban-context"
import type { TaskSummarySnapshot } from "@/lib/luban-api"
import { computeProjectDisplayNames } from "@/lib/project-display-names"
import { projectColorClass } from "@/lib/project-colors"
import type { NewTaskDraft } from "@/lib/new-task-drafts"
import { deleteNewTaskDraft, loadNewTaskDrafts, NEW_TASK_DRAFTS_CHANGED_EVENT } from "@/lib/new-task-drafts"

/**
 * Luban IDE main layout
 *
 * Structure:
 * - Left: Navigation sidebar
 * - Right: Main content panel (floating, with rounded corners)
 *   - Inbox view (notifications with split view)
 *   - Task list view (default)
 *   - Task detail view (when a task is selected)
 */
export function LubanIDE() {
  const { app, openWorkdir: openWorkspace, activateTask } = useLuban()

  const [activeView, setActiveView] = useState<NavView>("tasks")
  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [showDetail, setShowDetail] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [newTaskOpen, setNewTaskOpen] = useState(false)
  const [newTaskInitialDraft, setNewTaskInitialDraft] = useState<NewTaskDraft | null>(null)
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null)
  const [newTaskDrafts, setNewTaskDrafts] = useState<NewTaskDraft[]>([])
  const [newTaskDraftsOpen, setNewTaskDraftsOpen] = useState(false)

  useEffect(() => {
    const refresh = () => {
      void (async () => {
        try {
          setNewTaskDrafts(await loadNewTaskDrafts())
        } catch (err) {
          console.warn("loadNewTaskDrafts failed", err)
        }
      })()
    }
    refresh()
    window.addEventListener(NEW_TASK_DRAFTS_CHANGED_EVENT, refresh)
    return () => window.removeEventListener(NEW_TASK_DRAFTS_CHANGED_EVENT, refresh)
  }, [])

  const handleViewChange = (view: NavView) => {
    if (view === "settings") {
      setSettingsOpen(true)
      return
    }
    setActiveView(view)
    setSelectedTask(null)
    setShowDetail(false)
  }

  // Handle opening full view from inbox notification
  const handleOpenFullViewFromInbox = (notification: InboxNotification) => {
    void (async () => {
      await openWorkspace(notification.workdirId)
      await activateTask(notification.taskId)
      setSelectedTask({
        id: notification.id,
        workspaceId: notification.workdirId,
        taskId: notification.taskId,
        title: notification.taskTitle,
        status:
          notification.type === "completed"
            ? "done"
            : notification.type === "failed"
              ? "canceled"
              : "iterating",
        workdir: notification.workdir,
        projectName: notification.projectName,
        projectColor: notification.projectColor,
        createdAt: notification.timestamp,
      })
      setActiveView("tasks")
      setShowDetail(true)
    })()
  }

  const projectInfoById = useMemo(() => {
    if (!app) return new Map<string, { name: string; color: string }>()
    const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))
    const out = new Map<string, { name: string; color: string }>()
    for (const p of app.projects) {
      out.set(p.id, {
        name: displayNames.get(p.path) ?? p.slug,
        color: projectColorClass(p.id),
      })
    }
    return out
  }, [app])

  const taskFromSummary = (summary: TaskSummarySnapshot): Task => {
    const project = projectInfoById.get(summary.project_id) ?? { name: summary.project_id, color: "bg-violet-500" }
    return {
      id: `task-${summary.workdir_id}-${summary.task_id}`,
      workspaceId: summary.workdir_id,
      taskId: summary.task_id,
      title: summary.title,
      status: summary.task_status,
      workdir: summary.workdir_name || summary.branch_name,
      projectName: project.name,
      projectColor: project.color,
      createdAt: "",
    }
  }

  const renderContent = () => {
    if (activeView === "inbox") {
      return (
        <InboxView
          onOpenFullView={handleOpenFullViewFromInbox}
        />
      )
    }

    if (showDetail) {
      return (
        <TaskDetailView
          taskId={selectedTask?.id}
          taskTitle={selectedTask?.title}
          workdir={selectedTask?.workdir}
          projectName={selectedTask?.projectName}
          projectColor={selectedTask?.projectColor}
          onBack={() => {
            setSelectedTask(null)
            setShowDetail(false)
          }}
        />
      )
    }

    const taskListMode =
      activeView === "tasks-all" || activeView === "archive"
        ? "all"
        : activeView === "tasks-backlog"
          ? "backlog"
          : "active"
    return (
      <TaskListView
        activeProjectId={activeProjectId}
        mode={taskListMode}
        onModeChange={(mode) => {
          setActiveView(mode === "all" ? "tasks-all" : mode === "backlog" ? "tasks-backlog" : "tasks")
        }}
        onTaskClick={(task) => {
          void (async () => {
            await openWorkspace(task.workspaceId)
            await activateTask(task.taskId)
            setSelectedTask(task)
            setShowDetail(true)
          })()
        }}
      />
    )
  }

  return (
    <>
      <LubanLayout
        sidebar={
          <LubanSidebar
            activeView={activeView}
            onViewChange={handleViewChange}
            activeProjectId={activeProjectId}
            onProjectSelected={(projectId) => setActiveProjectId(projectId)}
            onNewTask={() => {
              setNewTaskInitialDraft(null)
              setNewTaskOpen(true)
            }}
            onFavoriteTaskSelected={(task) => {
              void (async () => {
                await openWorkspace(task.workdir_id)
                await activateTask(task.task_id)
                setSelectedTask(taskFromSummary(task))
                setActiveView("tasks")
                setShowDetail(true)
              })()
            }}
            newTaskDraftCount={newTaskDrafts.length}
            onOpenNewTaskDrafts={() => setNewTaskDraftsOpen(true)}
          />
        }
      >
        {renderContent()}
      </LubanLayout>
      <SettingsPanel
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
      <NewTaskDraftsDialog
        open={newTaskDraftsOpen}
        onOpenChange={setNewTaskDraftsOpen}
        drafts={newTaskDrafts}
        onOpenDraft={(draft) => {
          setNewTaskDraftsOpen(false)
          setNewTaskInitialDraft(draft)
          setNewTaskOpen(true)
        }}
        onDeleteDraft={(draftId) => deleteNewTaskDraft(draftId)}
      />
      <NewTaskModal
        open={newTaskOpen}
        activeProjectId={activeProjectId}
        initialDraft={newTaskInitialDraft}
        onOpenChange={(open) => {
          setNewTaskOpen(open)
          if (!open) setNewTaskInitialDraft(null)
          if (!open) setShowDetail(true)
        }}
      />
    </>
  )
}
