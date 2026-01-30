"use client"

import { useMemo, useState } from "react"
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

type TaskStatus = "todo" | "in-progress" | "done" | "cancelled"

export interface Task {
  id: string
  workspaceId: number
  title: string
  status: TaskStatus
  workdir: string
  projectName: string
  projectColor: string
  createdAt: string
}

const StatusIcon = ({ status }: { status: TaskStatus }) => {
  switch (status) {
    case "todo":
      return <Circle className="w-[14px] h-[14px]" style={{ color: '#9b9b9b' }} />
    case "in-progress":
      return <CircleDot className="w-[14px] h-[14px]" style={{ color: '#f2994a' }} />
    case "done":
      return <CheckCircle2 className="w-[14px] h-[14px]" style={{ color: '#5e6ad2' }} />
    case "cancelled":
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

  return (
    <div>
      <button
        onClick={() => setExpanded(!expanded)}
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

function taskStatusFromWorkspace(args: { agentRunStatus: string; hasUnreadCompletion: boolean }): TaskStatus {
  if (args.agentRunStatus === "running") return "in-progress"
  if (args.hasUnreadCompletion) return "done"
  return "todo"
}

export function TaskListView({ activeProjectId, onTaskClick }: TaskListViewProps) {
  const { app } = useLuban()
  const [selectedTask, setSelectedTask] = useState<string | null>(null)

  const normalizePathLike = (raw: string) => raw.trim().replace(/\/+$/, "")
  const isImplicitProjectRootWorkspace = (projectPath: string, args: { workspaceName: string; worktreePath: string }) =>
    args.workspaceName === "main" && normalizePathLike(args.worktreePath) === normalizePathLike(projectPath)

  const tasks = useMemo(() => {
    if (!app) return [] as Task[]

    const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))
    const out: Task[] = []

    for (const p of app.projects) {
      if (activeProjectId && p.id !== activeProjectId) continue
      const projectName = displayNames.get(p.path) ?? p.slug
      const projectColor = projectColorClass(p.id)
      for (const w of p.workspaces) {
        if (w.status !== "active") continue
        if (isImplicitProjectRootWorkspace(p.path, { workspaceName: w.workspace_name, worktreePath: w.worktree_path })) {
          continue
        }
        out.push({
          id: String(w.id),
          workspaceId: w.id,
          title: w.workspace_name || w.branch_name,
          status: taskStatusFromWorkspace({
            agentRunStatus: w.agent_run_status,
            hasUnreadCompletion: w.has_unread_completion,
          }),
          workdir: w.branch_name || w.workspace_name,
          projectName,
          projectColor,
          createdAt: "",
        })
      }
    }

    return out
  }, [activeProjectId, app])

  const headerProject: ProjectInfo = useMemo(() => {
    if (!app) return { name: "Projects", color: "bg-violet-500" }
    const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))
    if (activeProjectId) {
      const p = app.projects.find((p) => p.id === activeProjectId)
      if (p) return { name: displayNames.get(p.path) ?? p.slug, color: projectColorClass(p.id) }
    }
    return { name: "Projects", color: "bg-violet-500" }
  }, [activeProjectId, app])

  const inProgressTasks = tasks.filter((t) => t.status === "in-progress")
  const todoTasks = tasks.filter((t) => t.status === "todo")
  const doneTasks = tasks.filter((t) => t.status === "done")

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
      </div>
    </div>
  )
}
