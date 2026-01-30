"use client"

import { useState } from "react"
import { LubanLayout } from "./luban-layout"
import { LubanSidebar, type NavView } from "./luban-sidebar"
import { TaskListView, Task } from "./task-list-view"
import { TaskDetailView } from "./task-detail-view"
import { InboxView, type InboxNotification } from "./inbox-view"
import { SettingsPanel } from "./settings-panel"

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
  const [activeView, setActiveView] = useState<NavView>("inbox")
  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)

  const handleViewChange = (view: NavView) => {
    if (view === "settings") {
      setSettingsOpen(true)
      return
    }
    setActiveView(view)
    setSelectedTask(null) // Clear selected task when switching views
  }

  // Handle opening full view from inbox notification
  const handleOpenFullViewFromInbox = (notification: InboxNotification) => {
    // Convert notification to task format
    const task: Task = {
      id: notification.id,
      title: notification.taskTitle,
      status: notification.type === "completed" ? "done" : notification.type === "failed" ? "cancelled" : "in-progress",
      worktree: notification.worktree,
      projectName: notification.projectName,
      projectColor: notification.projectColor,
      createdAt: notification.timestamp,
    }
    setSelectedTask(task)
    setActiveView("tasks") // Switch to tasks view to show full detail
  }

  const renderContent = () => {
    if (activeView === "inbox") {
      return <InboxView onOpenFullView={handleOpenFullViewFromInbox} />
    }

    if (selectedTask) {
      return (
        <TaskDetailView
          taskId={selectedTask.id}
          taskTitle={selectedTask.title}
          worktree={selectedTask.worktree}
          projectName={selectedTask.projectName}
          projectColor={selectedTask.projectColor}
          onBack={() => setSelectedTask(null)}
        />
      )
    }

    return <TaskListView onTaskClick={(task) => setSelectedTask(task)} />
  }

  return (
    <>
      <LubanLayout
        sidebar={
          <LubanSidebar
            activeView={activeView}
            onViewChange={handleViewChange}
          />
        }
      >
        {renderContent()}
      </LubanLayout>
      <SettingsPanel
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
    </>
  )
}
