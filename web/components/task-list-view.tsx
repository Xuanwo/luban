"use client"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  Activity,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Layers,
  Loader2,
  ListChecks,
  Plus,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { ProjectIcon, type ProjectInfo } from "./shared/task-header"
import { TaskStatusSelector } from "./shared/task-status-selector"
import { TaskStatusCommandMenu, type AnchorRect } from "./shared/task-status-command-menu"
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
import { ShortcutTooltip } from "@/components/shared/shortcut-tooltip"

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
  onMouseEnter?: () => void
  onMouseLeave?: () => void
  onStatusChange?: (status: TaskStatus) => void
}

function TaskRow({
  task,
  selected,
  onClick,
  onMouseEnter,
  onMouseLeave,
  onStatusChange,
  agentRunner,
}: TaskRowProps & { agentRunner: AgentRunnerKind | null | undefined }) {
  const isArchived = task.status === "done" || task.status === "canceled"
  return (
    <div
      onClick={isArchived ? undefined : onClick}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      data-task-row-id={task.id}
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
          disabled={isArchived}
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
        // eslint-disable-next-line @next/next/no-img-element
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
  const testId = `task-group-${title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .replace(/-+$/, "")}`

  useEffect(() => {
    if (userToggledRef.current) return
    if (!defaultExpanded) return
    setExpanded(true)
  }, [defaultExpanded])

  return (
    <div>
      <button
        data-testid={testId}
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
  mode?: "all" | "active" | "backlog"
  onModeChange?: (mode: "all" | "active" | "backlog") => void
  onTaskClick?: (task: Task) => void
  statusPickerRequestSeq?: number
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  const tag = target.tagName.toLowerCase()
  if (tag === "input" || tag === "textarea" || tag === "select") return true
  return target.isContentEditable
}

export function TaskListView({
  activeProjectId,
  mode = "active",
  onModeChange,
  onTaskClick,
  statusPickerRequestSeq = 0,
}: TaskListViewProps) {
  const { app, wsConnected, setTaskStatus, subscribeServerEvents } = useLuban()
  const [tasksSnapshot, setTasksSnapshot] = useState<TasksSnapshot | null>(null)
  const [selectedTask, setSelectedTask] = useState<string | null>(null)
  const [hoveredTaskId, setHoveredTaskId] = useState<string | null>(null)
  const [statusMenuOpen, setStatusMenuOpen] = useState(false)
  const [statusMenuAnchorRect, setStatusMenuAnchorRect] = useState<AnchorRect | null>(null)
  const [statusMenuTaskRowId, setStatusMenuTaskRowId] = useState<string | null>(null)
  const prevStatusPickerSeqRef = useRef<number>(statusPickerRequestSeq)
  const agentRunner = app?.agent.default_runner ?? null
  const refreshInFlightRef = useRef(false)
  const prevWsConnectedRef = useRef(false)

  const tasksWorkdirStatus = mode === "all" ? ("all" as const) : ("active" as const)
  const tasksTaskStatus: TaskStatus[] | undefined = useMemo(() => {
    if (mode === "backlog") return ["backlog"]
    if (mode === "active") return ["todo", "iterating", "validating"]
    return undefined
  }, [mode])

  const formatCreatedAt = useCallback((createdAtUnixSeconds: number): string => {
    if (!createdAtUnixSeconds) return ""
    const date = new Date(createdAtUnixSeconds * 1000)
    const year = date.getFullYear()
    const month = String(date.getMonth() + 1).padStart(2, "0")
    const day = String(date.getDate()).padStart(2, "0")
    return `${year}-${month}-${day}`
  }, [])

  const refreshTasks = useCallback(async () => {
    if (!activeProjectId) {
      setTasksSnapshot(null)
      return
    }
    if (refreshInFlightRef.current) return
    refreshInFlightRef.current = true
    try {
      const snap = await fetchTasks({ projectId: activeProjectId, workdirStatus: tasksWorkdirStatus, taskStatus: tasksTaskStatus })
      setTasksSnapshot(snap)
    } catch (err) {
      console.warn("fetchTasks failed", err)
    } finally {
      refreshInFlightRef.current = false
    }
  }, [activeProjectId, tasksTaskStatus, tasksWorkdirStatus])

  useEffect(() => {
    if (!app || !activeProjectId) {
      setTasksSnapshot(null)
      return
    }

    let cancelled = false
    void (async () => {
      try {
        const snap = await fetchTasks({ projectId: activeProjectId, workdirStatus: tasksWorkdirStatus, taskStatus: tasksTaskStatus })
        if (cancelled) return
        setTasksSnapshot(snap)
      } catch (err) {
        console.warn("fetchTasks failed", err)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [activeProjectId, app, tasksTaskStatus, tasksWorkdirStatus])

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

    const filtered = tasksSnapshot.tasks.filter((t) => {
      const workdir = workdirById.get(t.workdir_id) ?? null
      if (!workdir) return false
      if (mode !== "all" && workdir.status !== "active") return false
      if (mode === "all") return true
      if (mode === "backlog") return t.task_status === "backlog"
      return t.task_status === "iterating" || t.task_status === "validating" || t.task_status === "todo"
    })

    filtered.sort((a, b) => {
      const primary =
        mode === "all" ? b.updated_at_unix_seconds - a.updated_at_unix_seconds : b.created_at_unix_seconds - a.created_at_unix_seconds
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
  }, [app, formatCreatedAt, mode, tasksSnapshot])

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

  const tasksByRowId = useMemo(() => {
    const out = new Map<string, TaskRowModel>()
    for (const t of tasks) out.set(t.id, t)
    return out
  }, [tasks])

  const orderedTasks = useMemo(() => {
    if (mode === "backlog") return [...backlogTasks]
    if (mode === "active") return [...iteratingTasks, ...validatingTasks, ...todoTasks]
    return [...iteratingTasks, ...validatingTasks, ...todoTasks, ...backlogTasks, ...doneTasks, ...canceledTasks]
  }, [backlogTasks, canceledTasks, doneTasks, iteratingTasks, mode, todoTasks, validatingTasks])

  const activeTaskRowIdForStatus = hoveredTaskId ?? selectedTask

  const openStatusMenuForTaskRowId = useCallback(
    (taskRowId: string) => {
      const t = tasksByRowId.get(taskRowId)
      if (!t) return

      const trigger = document.querySelector(
        `[data-testid="task-status-selector-${t.workspaceId}-${t.taskId}"]`,
      ) as HTMLElement | null
      const rowEl = document.querySelector(`[data-task-row-id="${taskRowId}"]`) as HTMLElement | null
      const rect = trigger?.getBoundingClientRect() ?? rowEl?.getBoundingClientRect() ?? null
      if (!rect) return

      setStatusMenuTaskRowId(taskRowId)
      setStatusMenuAnchorRect({ top: rect.top, left: rect.left, width: rect.width, height: rect.height })
      setStatusMenuOpen(true)
    },
    [tasksByRowId],
  )

  useEffect(() => {
    if (prevStatusPickerSeqRef.current === statusPickerRequestSeq) return
    prevStatusPickerSeqRef.current = statusPickerRequestSeq
    if (!activeTaskRowIdForStatus) return
    openStatusMenuForTaskRowId(activeTaskRowIdForStatus)
  }, [activeTaskRowIdForStatus, openStatusMenuForTaskRowId, statusPickerRequestSeq])

  const statusMenuTask = statusMenuTaskRowId ? tasksByRowId.get(statusMenuTaskRowId) ?? null : null

  useEffect(() => {
    if (statusMenuOpen && !statusMenuTask) {
      setStatusMenuOpen(false)
      setStatusMenuTaskRowId(null)
      setStatusMenuAnchorRect(null)
    }
  }, [statusMenuOpen, statusMenuTask])

  return (
    <div
      className="h-full flex flex-col outline-none"
      data-testid="task-list-view"
      tabIndex={0}
      onKeyDown={(e) => {
        if (statusMenuOpen) return
        if (e.ctrlKey || e.metaKey || e.altKey) return
        if (isEditableTarget(e.target)) return

        if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return
        if (orderedTasks.length === 0) return

        e.preventDefault()
        e.stopPropagation()

        const currentIndex = selectedTask ? orderedTasks.findIndex((t) => t.id === selectedTask) : -1
        const delta = e.key === "ArrowDown" ? 1 : -1
        const nextIndex = Math.min(Math.max(0, currentIndex + delta), orderedTasks.length - 1)
        const next = orderedTasks[nextIndex]
        if (!next) return

        setSelectedTask(next.id)
        window.requestAnimationFrame(() => {
          const el = document.querySelector(`[data-task-row-id="${next.id}"]`) as HTMLElement | null
          el?.scrollIntoView({ block: "nearest" })
        })
      }}
    >
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
          <ShortcutTooltip label="Go to all tasks" keys={["G", "E"]} side="bottom" align="start">
            <button
              className="h-6 px-2 text-[12px] font-medium rounded-[5px] flex items-center gap-1.5 hover:bg-[#eeeeee] transition-colors"
              style={{
                backgroundColor: mode === "all" ? "#eeeeee" : "transparent",
                color: mode === "all" ? "#1b1b1b" : "#6b6b6b",
              }}
              data-testid="task-view-tab-all"
              onClick={() => onModeChange?.("all")}
            >
              <ListChecks className="w-3.5 h-3.5" />
              All tasks
            </button>
          </ShortcutTooltip>
          <ShortcutTooltip label="Go to active" keys={["G", "A"]} side="bottom" align="start">
            <button
              className="h-6 px-2 text-[12px] font-medium rounded-[5px] flex items-center gap-1.5 hover:bg-[#eeeeee] transition-colors"
              style={{
                backgroundColor: mode === "active" ? "#eeeeee" : "transparent",
                color: mode === "active" ? "#1b1b1b" : "#6b6b6b",
              }}
              data-testid="task-view-tab-active"
              onClick={() => onModeChange?.("active")}
            >
              <Activity className="w-3.5 h-3.5" />
              Active
            </button>
          </ShortcutTooltip>
          <ShortcutTooltip label="Go to backlog" keys={["G", "B"]} side="bottom" align="start">
            <button
              className="h-6 px-2 text-[12px] font-medium rounded-[5px] flex items-center gap-1.5 hover:bg-[#eeeeee] transition-colors"
              style={{
                backgroundColor: mode === "backlog" ? "#eeeeee" : "transparent",
                color: mode === "backlog" ? "#1b1b1b" : "#6b6b6b",
              }}
              data-testid="task-view-tab-backlog"
              onClick={() => onModeChange?.("backlog")}
            >
              <Layers className="w-3.5 h-3.5" />
              Backlog
            </button>
          </ShortcutTooltip>
        </div>
      </div>

      {/* Task List */}
      <div className="flex-1 overflow-y-auto">
        {mode === "backlog" ? (
          <>
            <TaskGroup title="Backlog" count={backlogTasks.length} defaultExpanded={true}>
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>
          </>
        ) : mode === "active" ? (
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>

            <TaskGroup title="Backlog" count={backlogTasks.length} defaultExpanded={backlogTasks.length > 0}>
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
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
                  onMouseEnter={() => setHoveredTaskId(task.id)}
                  onMouseLeave={() => setHoveredTaskId((prev) => (prev === task.id ? null : prev))}
                  onStatusChange={(newStatus) =>
                    handleStatusChange({ workspaceId: task.workspaceId, taskId: task.taskId, status: newStatus })
                  }
                />
              ))}
            </TaskGroup>
          </>
        )}
      </div>
      {statusMenuTask ? (
        <TaskStatusCommandMenu
          open={statusMenuOpen}
          anchorRect={statusMenuAnchorRect}
          status={statusMenuTask.status}
          onSelect={(next) =>
            handleStatusChange({ workspaceId: statusMenuTask.workspaceId, taskId: statusMenuTask.taskId, status: next })
          }
          onClose={() => {
            setStatusMenuOpen(false)
            setStatusMenuTaskRowId(null)
            setStatusMenuAnchorRect(null)
          }}
        />
      ) : null}
    </div>
  )
}
