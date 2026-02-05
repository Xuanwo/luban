"use client"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Loader2,
  Plus,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { ProjectIcon, type ProjectInfo } from "./shared/task-header"
import { TaskStatusSelector } from "./shared/task-status-selector"
import { useLuban } from "@/lib/luban-context"
import { agentRunnerLabel } from "@/lib/conversation-ui"
import { computeProjectDisplayNames } from "@/lib/project-display-names"
import { projectColorClass } from "@/lib/project-colors"
import { buildSidebarProjects } from "@/lib/sidebar-view-model"
import { fetchTasks } from "@/lib/luban-http"
import type {
  AgentRunnerKind,
  OperationStatus,
  TaskStatus,
  TasksSnapshot,
  TurnResult,
  TurnStatus,
  WorkspaceId,
  WorkspaceThreadId,
} from "@/lib/luban-api"
import { UnifiedProviderLogo } from "@/components/shared/unified-provider-logo"

const AMP_MARK_URL = "https://ampcode.com/press-kit/mark-color.svg"

export interface Task {
  id: string
  workspaceId: WorkspaceId
  taskId: WorkspaceThreadId
  title: string
  status: TaskStatus
  workdir: string
  projectName: string
  projectColor: string
  createdAt: string
}

type TaskRowModel = Task & {
  updatedAtUnixSeconds: number
  agentRunStatus: OperationStatus
  turnStatus: TurnStatus
  lastTurnResult: TurnResult | null
  hasUnreadCompletion: boolean
}

interface TaskRowProps {
  task: TaskRowModel
  selected?: boolean
  onClick?: () => void
  onStatusChange?: (status: TaskStatus) => void
}

function TaskRow({
  task,
  selected,
  onClick,
  onStatusChange,
  agentRunner,
}: TaskRowProps & { agentRunner: AgentRunnerKind | null | undefined }) {
  return (
    <div
      onClick={onClick}
      className={cn(
        "group flex items-center gap-3 px-4 h-[44px] cursor-pointer transition-colors",
        selected ? "bg-[#f0f0f0]" : "hover:bg-[#f7f7f7]"
      )}
      style={{ borderBottom: '1px solid #ebebeb' }}
    >
      <div onClick={(e) => e.stopPropagation()}>
        <TaskStatusSelector
          status={task.status}
          onStatusChange={onStatusChange}
          size="sm"
          triggerTestId={`task-status-selector-${task.workspaceId}-${task.taskId}`}
        />
      </div>
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
      <TaskAgentPill
        runner={agentRunner}
        agentRunStatus={task.agentRunStatus}
        turnStatus={task.turnStatus}
        lastTurnResult={task.lastTurnResult}
        hasUnreadCompletion={task.hasUnreadCompletion}
        testId={`task-agent-pill-${task.workspaceId}-${task.taskId}`}
      />
      {task.createdAt ? (
        <span className="text-[12px] flex-shrink-0" style={{ color: "#9b9b9b" }}>
          {task.createdAt}
        </span>
      ) : null}
    </div>
  )
}

function TaskAgentPill({
  runner,
  agentRunStatus,
  turnStatus,
  lastTurnResult,
  hasUnreadCompletion,
  testId,
}: {
  runner: AgentRunnerKind | null | undefined
  agentRunStatus: OperationStatus
  turnStatus: TurnStatus
  lastTurnResult: TurnResult | null
  hasUnreadCompletion: boolean
  testId: string
}): React.ReactElement | null {
  const isRunning = agentRunStatus === "running" || turnStatus === "running"
  const isAwaitingAck =
    !isRunning && (turnStatus === "awaiting" || (hasUnreadCompletion && lastTurnResult === "completed"))
  if (!isRunning && !isAwaitingAck) return null

  const label = agentRunnerLabel(runner)
  const title = isRunning ? `${label}: running` : `${label}: awaiting_ack`

  const avatar = (() => {
    if (runner === "amp") {
      return (
        <img
          data-agent-runner-icon="amp"
          src={AMP_MARK_URL}
          alt=""
          aria-hidden="true"
          className="w-3.5 h-3.5"
        />
      )
    }
    if (runner === "claude") {
      return <UnifiedProviderLogo providerId="anthropic" className="w-3.5 h-3.5" />
    }
    return <UnifiedProviderLogo providerId="openai" className="w-3.5 h-3.5" />
  })()

  const glyph = isRunning ? (
    <Loader2 className="w-3.5 h-3.5 animate-spin" style={{ color: "#5e6ad2" }} />
  ) : (
    <span className="relative flex items-center justify-center">
      <CheckCircle2 className="w-3.5 h-3.5" style={{ color: "#5e6ad2" }} />
      <span
        className="absolute -top-0.5 -right-0.5 w-1.5 h-1.5 rounded-full"
        style={{ backgroundColor: "#5e6ad2" }}
      />
    </span>
  )

  return (
    <span
      data-testid={testId}
      className="inline-flex items-center gap-1.5 pl-1 pr-2 py-0.5 rounded-full flex-shrink-0"
      style={{ backgroundColor: "#f0f0f0", border: "1px solid #ebebeb" }}
      title={title}
    >
      <span
        className="w-4 h-4 rounded-full flex items-center justify-center flex-shrink-0"
        style={{ backgroundColor: "#fcfcfc", border: "1px solid #ebebeb" }}
      >
        {avatar}
      </span>
      <span className="text-[11px] font-medium" style={{ color: "#6b6b6b" }}>
        {label}
      </span>
      {glyph}
    </span>
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
  mode?: "active" | "archive"
  onModeChange?: (mode: "active" | "archive") => void
  onTaskClick?: (task: Task) => void
}

export function TaskListView({ activeProjectId, mode = "active", onModeChange, onTaskClick }: TaskListViewProps) {
  const { app, wsConnected, setTaskStatus, subscribeServerEvents } = useLuban()
  const [tasksSnapshot, setTasksSnapshot] = useState<TasksSnapshot | null>(null)
  const [selectedTask, setSelectedTask] = useState<string | null>(null)
  const agentRunner = app?.agent.default_runner ?? null
  const refreshInFlightRef = useRef(false)
  const prevWsConnectedRef = useRef(false)
  const [nowUnixSeconds, setNowUnixSeconds] = useState<number>(Math.floor(Date.now() / 1000))

  const formatCreatedAt = useCallback((createdAtUnixSeconds: number): string => {
    if (!createdAtUnixSeconds) return ""
    const date = new Date(createdAtUnixSeconds * 1000)
    const year = date.getFullYear()
    const month = String(date.getMonth() + 1).padStart(2, "0")
    const day = String(date.getDate()).padStart(2, "0")
    return `${year}-${month}-${day}`
  }, [])

  useEffect(() => {
    const update = () => setNowUnixSeconds(Math.floor(Date.now() / 1000))
    const id = window.setInterval(update, 60 * 60 * 1000)
    return () => window.clearInterval(id)
  }, [])

  const refreshTasks = useCallback(async () => {
    if (!activeProjectId) {
      setTasksSnapshot(null)
      return
    }
    if (refreshInFlightRef.current) return
    refreshInFlightRef.current = true
    try {
      const snap = await fetchTasks({ projectId: activeProjectId })
      setTasksSnapshot(snap)
    } catch (err) {
      console.warn("fetchTasks failed", err)
    } finally {
      refreshInFlightRef.current = false
    }
  }, [activeProjectId])

  useEffect(() => {
    if (!app || !activeProjectId) {
      setTasksSnapshot(null)
      return
    }

    let cancelled = false
    void (async () => {
      try {
        const snap = await fetchTasks({ projectId: activeProjectId })
        if (cancelled) return
        setTasksSnapshot(snap)
      } catch (err) {
        console.warn("fetchTasks failed", err)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [activeProjectId, app])

  useEffect(() => {
    const prev = prevWsConnectedRef.current
    prevWsConnectedRef.current = wsConnected
    if (prev || !wsConnected) return
    void refreshTasks()
  }, [refreshTasks, wsConnected])

  useEffect(() => {
    if (!activeProjectId) return
    return subscribeServerEvents((event) => {
      if (event.type !== "task_summaries_changed") return
      if (event.project_id !== activeProjectId) return
      setTasksSnapshot((prev) => {
        if (!prev) return prev
        const nextTasks = [
          ...prev.tasks.filter((t) => t.workdir_id !== event.workdir_id),
          ...event.tasks,
        ]
        return { ...prev, tasks: nextTasks, rev: prev.rev + 1 }
      })
    })
  }, [activeProjectId, subscribeServerEvents])

  const applyLocalTaskStatus = useCallback((args: { workspaceId: WorkspaceId; taskId: WorkspaceThreadId; status: TaskStatus }) => {
    setTasksSnapshot((prev) => {
      if (!prev) return prev
      let changed = false
      const nextTasks = prev.tasks.map((t) => {
        if (t.workdir_id !== args.workspaceId || t.task_id !== args.taskId) return t
        if (t.task_status === args.status) return t
        changed = true
        return { ...t, task_status: args.status }
      })
      return changed ? { ...prev, tasks: nextTasks, rev: prev.rev + 1 } : prev
    })
  }, [])

  const handleStatusChange = useCallback(
    (args: { workspaceId: WorkspaceId; taskId: WorkspaceThreadId; status: TaskStatus }) => {
      applyLocalTaskStatus(args)
      setTaskStatus(args.workspaceId, args.taskId, args.status)
      window.setTimeout(() => void refreshTasks(), 200)
    },
    [applyLocalTaskStatus, refreshTasks, setTaskStatus],
  )

  const tasks = useMemo(() => {
    if (!app || !tasksSnapshot) return [] as TaskRowModel[]

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

    const out: TaskRowModel[] = []

    const isArchived = (t: (typeof tasksSnapshot.tasks)[number]): boolean => {
      const isClosed = t.task_status === "done" || t.task_status === "canceled"
      if (!isClosed) return false
      const archiveAfterSeconds = 7 * 24 * 60 * 60
      return t.updated_at_unix_seconds <= nowUnixSeconds - archiveAfterSeconds
    }

    const filtered = tasksSnapshot.tasks.filter((t) => {
      const workdir = workdirById.get(t.workdir_id) ?? null
      if (!workdir) return false
      if (workdir.status !== "active") return false
      const archived = isArchived(t)
      if (mode === "archive") return archived
      return !archived
    })

    filtered.sort((a, b) => {
      const primary =
        mode === "archive"
          ? b.updated_at_unix_seconds - a.updated_at_unix_seconds
          : b.created_at_unix_seconds - a.created_at_unix_seconds
      if (primary !== 0) return primary
      const workdir = b.workdir_id - a.workdir_id
      if (workdir !== 0) return workdir
      return b.task_id - a.task_id
    })

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
        createdAt: formatCreatedAt(t.created_at_unix_seconds),
        updatedAtUnixSeconds: t.updated_at_unix_seconds,
        agentRunStatus: t.agent_run_status,
        turnStatus: t.turn_status,
        lastTurnResult: t.last_turn_result,
        hasUnreadCompletion: t.has_unread_completion,
      })
    }

    return out
  }, [app, formatCreatedAt, mode, nowUnixSeconds, tasksSnapshot])

  const headerProject: ProjectInfo = useMemo(() => {
    if (!app) return { name: "Projects", color: "bg-violet-500" }
    if (activeProjectId) {
      const sidebarVm = buildSidebarProjects(app).find((p) => p.id === activeProjectId) ?? null
      if (sidebarVm) {
        return {
          name: sidebarVm.displayName,
          color: projectColorClass(sidebarVm.id),
          avatarUrl: sidebarVm.avatarUrl,
        }
      }

      const displayNames = computeProjectDisplayNames(app.projects.map((p) => ({ path: p.path, name: p.name })))
      const p = app.projects.find((p) => p.id === activeProjectId)
      if (p) return { name: displayNames.get(p.path) ?? p.slug, color: projectColorClass(p.id) }
    }
    return { name: "Projects", color: "bg-violet-500" }
  }, [activeProjectId, app])

  const iteratingTasks = tasks.filter((t) => t.status === "iterating")
  const validatingTasks = tasks.filter((t) => t.status === "validating")
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
          <ProjectIcon
            testId="task-list-project-icon"
            name={headerProject.name}
            color={headerProject.color}
            avatarUrl={headerProject.avatarUrl}
          />
          <span className="text-[13px] font-medium" style={{ color: '#1b1b1b' }}>
            {headerProject.name}
          </span>
        </div>

        {/* View Tabs */}
        <div className="flex items-center gap-0.5 ml-3">
          <button
            className="h-6 px-2 text-[12px] font-medium rounded-[5px] flex items-center"
            style={{
              backgroundColor: mode === "active" ? "#eeeeee" : "transparent",
              color: mode === "active" ? "#1b1b1b" : "#6b6b6b",
            }}
            data-testid="task-view-tab-active"
            onClick={() => onModeChange?.("active")}
          >
            Active
          </button>
          <button
            className="h-6 px-2 text-[12px] font-medium rounded-[5px] flex items-center hover:bg-[#eeeeee] transition-colors"
            style={{
              backgroundColor: mode === "archive" ? "#eeeeee" : "transparent",
              color: mode === "archive" ? "#1b1b1b" : "#6b6b6b",
            }}
            data-testid="task-view-tab-archive"
            onClick={() => onModeChange?.("archive")}
          >
            Archive
          </button>
        </div>
      </div>

      {/* Task List */}
      <div className="flex-1 overflow-y-auto">
        {mode === "archive" ? (
          <>
            <TaskGroup title="Done" count={doneTasks.length}>
              {doneTasks.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  agentRunner={agentRunner}
                  selected={selectedTask === task.id}
                  onClick={() => {
                    setSelectedTask(task.id)
                    onTaskClick?.(task)
                  }}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>
            <TaskGroup title="Canceled" count={canceledTasks.length}>
              {canceledTasks.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  agentRunner={agentRunner}
                  selected={selectedTask === task.id}
                  onClick={() => {
                    setSelectedTask(task.id)
                    onTaskClick?.(task)
                  }}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>
          </>
        ) : (
          <>
            <TaskGroup title="Iterating" count={iteratingTasks.length}>
              {iteratingTasks.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  agentRunner={agentRunner}
                  selected={selectedTask === task.id}
                  onClick={() => {
                    setSelectedTask(task.id)
                    onTaskClick?.(task)
                  }}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>

            <TaskGroup title="Validating" count={validatingTasks.length}>
              {validatingTasks.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  agentRunner={agentRunner}
                  selected={selectedTask === task.id}
                  onClick={() => {
                    setSelectedTask(task.id)
                    onTaskClick?.(task)
                  }}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>

            <TaskGroup title="Todo" count={todoTasks.length}>
              {todoTasks.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  agentRunner={agentRunner}
                  selected={selectedTask === task.id}
                  onClick={() => {
                    setSelectedTask(task.id)
                    onTaskClick?.(task)
                  }}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>

            <TaskGroup
              title="Backlog"
              count={backlogTasks.length}
              defaultExpanded={backlogTasks.length > 0}
            >
              {backlogTasks.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  agentRunner={agentRunner}
                  selected={selectedTask === task.id}
                  onClick={() => {
                    setSelectedTask(task.id)
                    onTaskClick?.(task)
                  }}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>

            <TaskGroup title="Done" count={doneTasks.length} defaultExpanded={false}>
              {doneTasks.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  agentRunner={agentRunner}
                  selected={selectedTask === task.id}
                  onClick={() => {
                    setSelectedTask(task.id)
                    onTaskClick?.(task)
                  }}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>

            <TaskGroup title="Canceled" count={canceledTasks.length} defaultExpanded={false}>
              {canceledTasks.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  agentRunner={agentRunner}
                  selected={selectedTask === task.id}
                  onClick={() => {
                    setSelectedTask(task.id)
                    onTaskClick?.(task)
                  }}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>
          </>
        )}
      </div>
    </div>
  )
}
