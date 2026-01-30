"use client"

import { useState } from "react"
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

type TaskStatus = "todo" | "in-progress" | "done" | "cancelled"

export interface Task {
  id: string
  title: string
  status: TaskStatus
  worktree: string
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
        {task.worktree}
      </span>
      <span className="flex-1" />
      <span
        className="text-[12px] flex-shrink-0"
        style={{ color: '#9b9b9b' }}
      >
        {task.createdAt}
      </span>
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

// Mock data
const mockTasks: Task[] = [
  {
    id: "1",
    title: "Implement new layout for Luban dashboard",
    status: "in-progress",
    worktree: "miracle-main",
    projectName: "Luban",
    projectColor: "bg-violet-500",
    createdAt: "Jan 29",
  },
  {
    id: "2",
    title: "Add task filtering and sorting functionality",
    status: "in-progress",
    worktree: "feature-filter",
    projectName: "Luban",
    projectColor: "bg-violet-500",
    createdAt: "Jan 28",
  },
  {
    id: "3",
    title: "Design new sidebar navigation structure",
    status: "in-progress",
    worktree: "miracle-main",
    projectName: "Luban",
    projectColor: "bg-violet-500",
    createdAt: "Jan 28",
  },
  {
    id: "4",
    title: "Set up keyboard shortcuts for task management",
    status: "todo",
    worktree: "feature-shortcuts",
    projectName: "Luban",
    projectColor: "bg-violet-500",
    createdAt: "Jan 27",
  },
  {
    id: "5",
    title: "Create task detail panel component",
    status: "todo",
    worktree: "miracle-main",
    projectName: "Luban",
    projectColor: "bg-violet-500",
    createdAt: "Jan 27",
  },
  {
    id: "6",
    title: "Implement drag and drop for task reordering",
    status: "todo",
    worktree: "feature-dnd",
    projectName: "Luban",
    projectColor: "bg-violet-500",
    createdAt: "Jan 26",
  },
  {
    id: "7",
    title: "Add dark mode support",
    status: "done",
    worktree: "feature-dark",
    projectName: "Frontend",
    projectColor: "bg-blue-500",
    createdAt: "Jan 25",
  },
  {
    id: "8",
    title: "Initial project setup and configuration",
    status: "done",
    worktree: "main",
    projectName: "Backend",
    projectColor: "bg-emerald-500",
    createdAt: "Jan 24",
  },
]

interface TaskListViewProps {
  project?: ProjectInfo
  onTaskClick?: (task: Task) => void
}

export function TaskListView({ project = { name: "Luban", color: "bg-violet-500" }, onTaskClick }: TaskListViewProps) {
  const [selectedTask, setSelectedTask] = useState<string | null>(null)

  const inProgressTasks = mockTasks.filter((t) => t.status === "in-progress")
  const todoTasks = mockTasks.filter((t) => t.status === "todo")
  const doneTasks = mockTasks.filter((t) => t.status === "done")

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div
        className="flex items-center h-[39px] flex-shrink-0"
        style={{ padding: '0 24px 0 20px', borderBottom: '1px solid #ebebeb' }}
      >
        {/* Project Indicator */}
        <div className="flex items-center gap-1">
          <ProjectIcon name={project.name} color={project.color} />
          <span className="text-[13px] font-medium" style={{ color: '#1b1b1b' }}>
            {project.name}
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
