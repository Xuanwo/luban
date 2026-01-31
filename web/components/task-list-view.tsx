"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import {
  ChevronDown,
  ChevronRight,
  Circle,
  CircleDot,
  CheckCircle2,
  Plus,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { ProjectIcon, type ProjectInfo } from "./shared/task-header"
import { useLuban } from "@/lib/luban-context"
import { computeProjectDisplayNames } from "@/lib/project-display-names"
import { projectColorClass } from "@/lib/project-colors"
import { fetchTasks } from "@/lib/luban-http"
import type { TaskStatus, TasksSnapshot } from "@/lib/luban-api"

export interface Task {
  id: string
  workspaceId: number
  taskId: number
  title: string
  status: TaskStatus
  workdir: string
  projectName: string
  projectColor: string
  createdAt: string
}

const StatusIcon = ({ status }: { status: TaskStatus }) => {
  switch (status) {
    case "backlog":
      return <Circle className="w-[14px] h-[14px]" style={{ color: '#d4d4d4' }} />
    case "todo":
      return <Circle className="w-[14px] h-[14px]" style={{ color: '#9b9b9b' }} />
    case "in_progress":
      return <CircleDot className="w-[14px] h-[14px]" style={{ color: '#f2994a' }} />
    case "in_review":
      return <CircleDot className="w-[14px] h-[14px]" style={{ color: '#5e6ad2' }} />
    case "done":
      return <CheckCircle2 className="w-[14px] h-[14px]" style={{ color: '#5e6ad2' }} />
    case "canceled":
      return <Circle className="w-[14px] h-[14px]" style={{ color: '#d4d4d4' }} />
  }
}

interface TaskRowProps {
  task: Task
  selected?: boolean
  onClick?: () => void
}

function TaskRow({ task, selected, onClick }: TaskRowProps) {
  return (
    <div
      onClick={onClick}
      className={cn(
        "group flex items-center gap-3 px-4 h-[44px] cursor-pointer transition-colors",
        selected ? "bg-[#f0f0f0]" : "hover:bg-[#f7f7f7]"
      )}
      style={{ borderBottom: '1px solid #ebebeb' }}
    >
      <StatusIcon status={task.status} />
      <span
        className="text-[13px] truncate"
        style={{ color: '#1b1b1b' }}
      >
        {task.title}
      </span>
      <span
        className="text-[11px] px-1.5 py-0.5 rounded flex-shrink-0"
        style={{ backgroundColor: '#f0f0f0', color: '#6b6b6b' }}
      >
        {task.workdir}
      </span>
      <span className="flex-1" />
      {task.createdAt ? (
        <span className="text-[12px] flex-shrink-0" style={{ color: "#9b9b9b" }}>
          {task.createdAt}
        </span>
      ) : null}
    </div>
  )
}

interface TaskGroupProps {
  title: string
  count: number
  defaultExpanded?: boolean
  children: React.ReactNode
}

function TaskGroup({ title, count, defaultExpanded = true, children }: TaskGroupProps) {
  const [expanded, setExpanded] = useState(defaultExpanded)
  const userToggledRef = useRef(false)

  useEffect(() => {
    if (userToggledRef.current) return
    if (!defaultExpanded) return
    setExpanded(true)
  }, [defaultExpanded])

  return (
    <div>
      <button
        onClick={() => {
          userToggledRef.current = true
          setExpanded(!expanded)
        }}
        className="group w-full flex items-center gap-2 px-4 h-[36px] text-[13px] font-medium hover:bg-[#f7f7f7] transition-colors"
        style={{ color: '#1b1b1b' }}
      >
        <span style={{ color: '#9b9b9b' }}>
          {expanded ? (
            <ChevronDown className="w-[14px] h-[14px]" />
          ) : (
            <ChevronRight className="w-[14px] h-[14px]" />
          )}
        </span>
        <span>{title}</span>
        <span style={{ color: '#9b9b9b' }} className="font-normal">{count}</span>
        <button
          onClick={(e) => {
            e.stopPropagation()
          }}
          className="ml-auto p-1 rounded hover:bg-[#e8e8e8] transition-colors opacity-0 group-hover:opacity-100"
          style={{ color: '#9b9b9b' }}
        >
          <Plus className="w-[14px] h-[14px]" />
        </button>
      </button>
      {expanded && <div>{children}</div>}
    </div>
  )
}

interface TaskListViewProps {
  activeProjectId?: string | null
  onTaskClick?: (task: Task) => void
}

export function TaskListView({ activeProjectId, onTaskClick }: TaskListViewProps) {
  const { app } = useLuban()
  const [tasksSnapshot, setTasksSnapshot] = useState<TasksSnapshot | null>(null)
  const [selectedTask, setSelectedTask] = useState<string | null>(null)

  useEffect(() => {
    if (!app) {
      setTasksSnapshot(null)
      return
    }

    let cancelled = false
    void (async () => {
      try {
        const snap = await fetchTasks(activeProjectId ? { projectId: activeProjectId } : {})
        if (cancelled) return
        setTasksSnapshot(snap)
      } catch (err) {
        console.warn("fetchTasks failed", err)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [activeProjectId, app?.rev])

  const tasks = useMemo(() => {
    if (!app || !tasksSnapshot) return [] as Task[]

    const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))
    const projectInfoById = new Map<string, { name: string; color: string }>()
    for (const p of app.projects) {
      projectInfoById.set(p.path, {
        name: displayNames.get(p.path) ?? p.slug,
        color: projectColorClass(p.id),
      })
    }

    const workdirById = new Map<number, { projectPath: string; workdirName: string; workdirPath: string; status: string }>()
    for (const p of app.projects) {
      for (const w of p.workdirs) {
        workdirById.set(w.id, {
          projectPath: p.path,
          workdirName: w.workdir_name,
          workdirPath: w.workdir_path,
          status: w.status,
        })
      }
    }

    const out: Task[] = []

    const filtered = tasksSnapshot.tasks.filter((t) => {
      const workdir = workdirById.get(t.workdir_id) ?? null
      if (!workdir) return false
      if (workdir.status !== "active") return false
      return true
    })

    filtered.sort((a, b) => b.updated_at_unix_seconds - a.updated_at_unix_seconds)

    for (const t of filtered) {
      const project = projectInfoById.get(t.project_id) ?? { name: t.project_id, color: "bg-violet-500" }
      out.push({
        id: `task-${t.workdir_id}-${t.task_id}`,
        workspaceId: t.workdir_id,
        taskId: t.task_id,
        title: t.title,
        status: t.task_status,
        workdir: t.workdir_name || t.branch_name,
        projectName: project.name,
        projectColor: project.color,
        createdAt: "",
      })
    }

    return out
  }, [activeProjectId, app, tasksSnapshot])

  const headerProject: ProjectInfo = useMemo(() => {
    if (!app) return { name: "Projects", color: "bg-violet-500" }
    const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))
    if (activeProjectId) {
      const p = app.projects.find((p) => p.id === activeProjectId)
      if (p) return { name: displayNames.get(p.path) ?? p.slug, color: projectColorClass(p.id) }
    }
    return { name: "Projects", color: "bg-violet-500" }
  }, [activeProjectId, app])

  const inProgressTasks = tasks.filter((t) => t.status === "in_progress")
  const inReviewTasks = tasks.filter((t) => t.status === "in_review")
  const todoTasks = tasks.filter((t) => t.status === "todo")
  const backlogTasks = tasks.filter((t) => t.status === "backlog")
  const doneTasks = tasks.filter((t) => t.status === "done")
  const canceledTasks = tasks.filter((t) => t.status === "canceled")

  return (
    <div className="h-full flex flex-col" data-testid="task-list-view">
      {/* Header */}
      <div
        className="flex items-center h-[39px] flex-shrink-0"
        style={{ padding: '0 24px 0 20px', borderBottom: '1px solid #ebebeb' }}
      >
        {/* Project Indicator */}
        <div className="flex items-center gap-1">
          <ProjectIcon name={headerProject.name} color={headerProject.color} />
          <span className="text-[13px] font-medium" style={{ color: '#1b1b1b' }}>
            {headerProject.name}
          </span>
        </div>

        {/* View Tabs */}
        <div className="flex items-center gap-0.5 ml-3">
          <button
            className="h-6 px-2 text-[12px] font-medium rounded-[5px] flex items-center"
            style={{ backgroundColor: '#eeeeee', color: '#1b1b1b' }}
          >
            All Issues
          </button>
          <button
            className="h-6 px-2 text-[12px] font-medium rounded-[5px] flex items-center hover:bg-[#eeeeee] transition-colors"
            style={{ color: '#6b6b6b' }}
          >
            Active
          </button>
          <button
            className="h-6 px-2 text-[12px] font-medium rounded-[5px] flex items-center hover:bg-[#eeeeee] transition-colors"
            style={{ color: '#6b6b6b' }}
          >
            Backlog
          </button>
        </div>
      </div>

      {/* Task List */}
      <div className="flex-1 overflow-y-auto">
        <TaskGroup title="In Progress" count={inProgressTasks.length}>
          {inProgressTasks.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              selected={selectedTask === task.id}
              onClick={() => {
                setSelectedTask(task.id)
                onTaskClick?.(task)
              }}
            />
          ))}
        </TaskGroup>

        <TaskGroup title="In Review" count={inReviewTasks.length}>
          {inReviewTasks.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              selected={selectedTask === task.id}
              onClick={() => {
                setSelectedTask(task.id)
                onTaskClick?.(task)
              }}
            />
          ))}
        </TaskGroup>

        <TaskGroup title="Todo" count={todoTasks.length}>
          {todoTasks.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              selected={selectedTask === task.id}
              onClick={() => {
                setSelectedTask(task.id)
                onTaskClick?.(task)
              }}
            />
          ))}
        </TaskGroup>

        <TaskGroup title="Backlog" count={backlogTasks.length} defaultExpanded={backlogTasks.length > 0}>
          {backlogTasks.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              selected={selectedTask === task.id}
              onClick={() => {
                setSelectedTask(task.id)
                onTaskClick?.(task)
              }}
            />
          ))}
        </TaskGroup>

        <TaskGroup title="Done" count={doneTasks.length} defaultExpanded={false}>
          {doneTasks.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              selected={selectedTask === task.id}
              onClick={() => {
                setSelectedTask(task.id)
                onTaskClick?.(task)
              }}
            />
          ))}
        </TaskGroup>

        <TaskGroup title="Canceled" count={canceledTasks.length} defaultExpanded={false}>
          {canceledTasks.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              selected={selectedTask === task.id}
              onClick={() => {
                setSelectedTask(task.id)
                onTaskClick?.(task)
              }}
            />
          ))}
        </TaskGroup>
      </div>
    </div>
  )
}
