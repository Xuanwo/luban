"use client"

import { useState } from "react"
import { LubanLayout } from "./luban-layout"
import { LubanSidebar, type NavView } from "./luban-sidebar"
import { TaskListView, Task } from "./task-list-view"
import { TaskDetailView } from "./task-detail-view"
import { InboxView, type InboxNotification } from "./inbox-view"
import { SettingsPanel } from "./settings-panel"
import { NewTaskModal } from "./new-task-modal"
import { useLuban } from "@/lib/luban-context"

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
  const { openWorkspace } = useLuban()

  const [activeView, setActiveView] = useState<NavView>("tasks")
  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [showDetail, setShowDetail] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [newTaskOpen, setNewTaskOpen] = useState(false)
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null)

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
      await openWorkspace(notification.workspaceId)
      setSelectedTask({
        id: notification.id,
        workspaceId: notification.workspaceId,
        title: notification.taskTitle,
        status:
          notification.type === "completed"
            ? "done"
            : notification.type === "failed"
              ? "cancelled"
              : "in-progress",
        worktree: notification.worktree,
        projectName: notification.projectName,
        projectColor: notification.projectColor,
        createdAt: notification.timestamp,
      })
      setActiveView("tasks")
      setShowDetail(true)
    })()
  }

  const renderContent = () => {
    if (activeView === "inbox") {
      return <InboxView onOpenFullView={handleOpenFullViewFromInbox} />
    }

    if (showDetail) {
      return (
        <TaskDetailView
          taskId={selectedTask?.id}
          taskTitle={selectedTask?.title}
          worktree={selectedTask?.worktree}
          projectName={selectedTask?.projectName}
          projectColor={selectedTask?.projectColor}
          onBack={() => {
            setSelectedTask(null)
            setShowDetail(false)
          }}
        />
      )
    }

    return (
      <TaskListView
        activeProjectId={activeProjectId}
        onTaskClick={(task) => {
          void (async () => {
            await openWorkspace(task.workspaceId)
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
            onWorkspaceOpened={() => {
              setSelectedTask(null)
              setShowDetail(true)
            }}
            onNewTask={() => setNewTaskOpen(true)}
          />
        }
      >
        {renderContent()}
      </LubanLayout>
      <SettingsPanel
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
      <NewTaskModal
        open={newTaskOpen}
        onOpenChange={(open) => {
          setNewTaskOpen(open)
          if (!open) setShowDetail(true)
        }}
      />
    </>
  )
}
