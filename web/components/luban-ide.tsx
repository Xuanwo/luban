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
import { GlobalSequenceShortcuts } from "./global-sequence-shortcuts"
import { useLuban } from "@/lib/luban-context"
import type { TaskSummarySnapshot } from "@/lib/luban-api"
import type { InboxFilters } from "@/lib/inbox-filters"
import { DEFAULT_INBOX_FILTERS, inboxFiltersEqual, normalizeInboxFilters } from "@/lib/inbox-filters"
import type { InboxSavedView } from "@/lib/inbox-views"
import { loadInboxViews, nextDefaultInboxViewName, saveInboxViews } from "@/lib/inbox-views"
import { computeProjectDisplayNames } from "@/lib/project-display-names"
import { projectColorClass } from "@/lib/project-colors"
import type { NewTaskDraft } from "@/lib/new-task-drafts"
import { deleteNewTaskDraft, loadNewTaskDrafts, NEW_TASK_DRAFTS_CHANGED_EVENT } from "@/lib/new-task-drafts"
import { INBOX_ACTIVE_VIEW_KEY, INBOX_FILTERS_KEY, loadJson, saveJson } from "@/lib/ui-prefs"

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}

function isString(value: unknown): value is string {
  return typeof value === "string"
}

function coerceInboxFilters(value: unknown): InboxFilters | null {
  if (!isRecord(value)) return null
  const projectIds = Array.isArray(value.projectIds) ? value.projectIds.filter(isString) : []
  const statuses = Array.isArray(value.statuses) ? value.statuses.filter(isString) : []
  const updated = isString(value.updated) ? value.updated : DEFAULT_INBOX_FILTERS.updated
  return normalizeInboxFilters({
    projectIds,
    statuses: statuses as InboxFilters["statuses"],
    updated: updated as InboxFilters["updated"],
  })
}

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
  const [inboxRefreshSeq, setInboxRefreshSeq] = useState(0)
  const [inboxViews, setInboxViews] = useState<InboxSavedView[]>(() => loadInboxViews())
  const [inboxFilters, setInboxFilters] = useState<InboxFilters>(() => {
    const stored = loadJson<unknown>(INBOX_FILTERS_KEY)
    return coerceInboxFilters(stored) ?? DEFAULT_INBOX_FILTERS
  })
  const [activeInboxViewId, setActiveInboxViewId] = useState<string | null>(() => {
    const stored = loadJson<unknown>(INBOX_ACTIVE_VIEW_KEY)
    return typeof stored === "string" ? stored : null
  })
  const [renamingInboxViewId, setRenamingInboxViewId] = useState<string | null>(null)
  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [showDetail, setShowDetail] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [newTaskOpen, setNewTaskOpen] = useState(false)
  const [newTaskInitialDraft, setNewTaskInitialDraft] = useState<NewTaskDraft | null>(null)
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null)
  const [newTaskDrafts, setNewTaskDrafts] = useState<NewTaskDraft[]>([])
  const [newTaskDraftsOpen, setNewTaskDraftsOpen] = useState(false)
  const [statusPickerRequestSeq, setStatusPickerRequestSeq] = useState(0)

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

  useEffect(() => {
    saveInboxViews(inboxViews)
  }, [inboxViews])

  useEffect(() => {
    saveJson(INBOX_FILTERS_KEY, normalizeInboxFilters(inboxFilters))
  }, [inboxFilters])

  useEffect(() => {
    saveJson(INBOX_ACTIVE_VIEW_KEY, activeInboxViewId)
  }, [activeInboxViewId])

  useEffect(() => {
    if (activeInboxViewId == null) return
    if (inboxViews.some(v => v.id === activeInboxViewId)) return
    setActiveInboxViewId(null)
  }, [activeInboxViewId, inboxViews])

  const openInboxBase = () => {
    setInboxRefreshSeq(prev => prev + 1)
    setActiveView("inbox")
    setSelectedTask(null)
    setShowDetail(false)
  }

  const openPlainInbox = () => {
    openInboxBase()
    setInboxFilters(DEFAULT_INBOX_FILTERS)
    setActiveInboxViewId(null)
    setRenamingInboxViewId(null)
  }

  const handleViewChange = (view: NavView) => {
    if (view === "settings") {
      setSettingsOpen(true)
      return
    }
    if (view === "inbox") {
      openPlainInbox()
      return
    }
    setActiveView(view)
    setSelectedTask(null)
    setShowDetail(false)
  }

  const handleInboxFiltersChange = (next: InboxFilters) => {
    const normalized = normalizeInboxFilters(next)
    setInboxFilters(normalized)
    const match = inboxViews.find(v => inboxFiltersEqual(v.filters, normalized)) ?? null
    setActiveInboxViewId(match?.id ?? null)
  }

  const handleCreateInboxView = () => {
    const id =
      typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
        ? crypto.randomUUID()
        : `${Date.now()}-${Math.random().toString(16).slice(2)}`
    setInboxViews(prev => [
      ...prev,
      {
        id,
        name: nextDefaultInboxViewName(prev),
        filters: normalizeInboxFilters(inboxFilters),
      },
    ])
    setActiveInboxViewId(id)
    setRenamingInboxViewId(id)
  }

  const handleApplyInboxView = (viewId: string) => {
    const view = inboxViews.find(v => v.id === viewId) ?? null
    if (!view) return
    openInboxBase()
    setInboxFilters(normalizeInboxFilters(view.filters))
    setActiveInboxViewId(view.id)
    setRenamingInboxViewId(null)
  }

  const handleDeleteInboxView = (viewId: string) => {
    setInboxViews(prev => prev.filter(v => v.id !== viewId))
    if (activeInboxViewId === viewId) setActiveInboxViewId(null)
    if (renamingInboxViewId === viewId) setRenamingInboxViewId(null)
  }

  const handleRenameInboxView = (viewId: string, nextName: string) => {
    const trimmed = nextName.trim()
    setInboxViews(prev => prev.map(v => (v.id === viewId ? { ...v, name: trimmed.length > 0 ? trimmed : v.name } : v)))
    setRenamingInboxViewId(null)
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
        status: notification.type === "completed" ? "done" : notification.type === "failed" ? "canceled" : "iterating",
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
    const displayNames = computeProjectDisplayNames(app.projects.map(p => ({ path: p.path, name: p.name })))
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
          refreshSeq={inboxRefreshSeq}
          filters={inboxFilters}
          onFiltersChange={handleInboxFiltersChange}
          onSaveView={handleCreateInboxView}
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
        statusPickerRequestSeq={statusPickerRequestSeq}
        onModeChange={mode => {
          setActiveView(mode === "all" ? "tasks-all" : mode === "backlog" ? "tasks-backlog" : "tasks")
        }}
        onTaskClick={task => {
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
      <GlobalSequenceShortcuts
        enabled={!settingsOpen && !newTaskOpen && !newTaskDraftsOpen}
        canGoProjectModes={activeProjectId != null}
        canOpenStatusPicker={activeProjectId != null && activeView !== "inbox" && !showDetail}
        onNewTask={() => {
          setNewTaskInitialDraft(null)
          setNewTaskOpen(true)
        }}
        onGoInbox={() => handleViewChange("inbox")}
        onSetTaskListMode={mode => {
          setSelectedTask(null)
          setShowDetail(false)
          setActiveView(mode === "all" ? "tasks-all" : mode === "backlog" ? "tasks-backlog" : "tasks")
        }}
        onOpenStatusPicker={() => setStatusPickerRequestSeq(prev => prev + 1)}
      />
      <LubanLayout
        sidebar={
          <LubanSidebar
            activeView={activeView}
            onViewChange={handleViewChange}
            activeProjectId={activeProjectId}
            onProjectSelected={projectId => setActiveProjectId(projectId)}
            inboxViews={inboxViews}
            activeInboxViewId={activeInboxViewId}
            renamingInboxViewId={renamingInboxViewId}
            onInboxViewSelected={handleApplyInboxView}
            onInboxViewDeleted={handleDeleteInboxView}
            onInboxViewRenameRequested={setRenamingInboxViewId}
            onInboxViewRenameSaved={handleRenameInboxView}
            onInboxViewRenameCanceled={() => setRenamingInboxViewId(null)}
            onNewTask={() => {
              setNewTaskInitialDraft(null)
              setNewTaskOpen(true)
            }}
            onFavoriteTaskSelected={task => {
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
      <SettingsPanel open={settingsOpen} onOpenChange={setSettingsOpen} />
      <NewTaskDraftsDialog
        open={newTaskDraftsOpen}
        onOpenChange={setNewTaskDraftsOpen}
        drafts={newTaskDrafts}
        onOpenDraft={draft => {
          setNewTaskDraftsOpen(false)
          setNewTaskInitialDraft(draft)
          setNewTaskOpen(true)
        }}
        onDeleteDraft={draftId => deleteNewTaskDraft(draftId)}
      />
      <NewTaskModal
        open={newTaskOpen}
        activeProjectId={activeProjectId}
        initialDraft={newTaskInitialDraft}
        onOpenChange={open => {
          setNewTaskOpen(open)
          if (!open) setNewTaskInitialDraft(null)
          if (!open) setShowDetail(true)
        }}
      />
    </>
  )
}
