"use client"

import type React from "react"

import {
  ChevronDown,
  ChevronRight,
  Plus,
  Settings,
  LayoutGrid,
  Sparkles,
  Layers,
  Archive,
  Loader2,
  Home,
  Trash2,
  MessageCircleQuestion,
  Pin,
  PinOff,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { useLuban } from "@/lib/luban-context"
import type { SidebarProjectVm, SidebarWorktreeVm } from "@/lib/sidebar-view-model"
import { buildSidebarProjects } from "@/lib/sidebar-view-model"
import { useEffect, useMemo, useRef, useState } from "react"
import { toast } from "sonner"
import { NewTaskModal } from "./new-task-modal"
import { FeedbackModal } from "./feedback-modal"
import { AgentStatusIcon, PRBadge } from "./shared/status-indicator"
import { focusChatInput } from "@/lib/focus-chat-input"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { SettingsPanel } from "@/components/settings-panel"
import type { AgentStatus } from "@/lib/worktree-ui"
import type { OpenSettingsDetail, SettingsSectionId } from "@/lib/open-settings"
import type { ProjectId, WorkspaceId } from "@/lib/luban-api"
import {
  PINNED_PROJECTS_KEY,
  PINNED_WORKTREES_KEY,
  PROJECT_ORDER_KEY,
  WORKTREE_ORDER_KEY,
  loadJson,
  saveJson,
} from "@/lib/ui-prefs"
import {
  DndContext,
  DragOverlay,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragStartEvent,
  type DragEndEvent,
  type DragCancelEvent,
  type UniqueIdentifier,
} from "@dnd-kit/core"
import {
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
  arrayMove,
} from "@dnd-kit/sortable"

import { SortableProject, SortableWorktree, ProjectDragOverlay, WorktreeDragOverlay } from "./sidebar/index"

interface SidebarProps {
  viewMode: "workspace" | "kanban"
  onViewModeChange: (mode: "workspace" | "kanban") => void
  widthPx: number
}

function normalizePathLike(raw: string): string {
  return raw.trim().replace(/\/+$/, "")
}

export function Sidebar({ viewMode, onViewModeChange, widthPx }: SidebarProps) {
  const {
    app,
    activeWorkspaceId,
    createWorkspace,
    openWorkspacePullRequest,
    openWorkspacePullRequestFailedAction,
    archiveWorkspace,
    toggleProjectExpanded,
    openWorkspace,
    addProject,
    addProjectAndOpen,
    ensureMainWorkspace,
    pickProjectPath,
    deleteProject,
    setSidebarProjectOrder,
    setSidebarWorktreeOrder,
  } = useLuban()

  const pendingCreateRef = useRef<{ projectId: ProjectId; existingWorkspaceIds: Set<number> } | null>(null)
  const pendingAddProjectPathRef = useRef<string | null>(null)
  const pendingOpenMainProjectIdRef = useRef<ProjectId | null>(null)
  const [optimisticCreatingProjectId, setOptimisticCreatingProjectId] = useState<ProjectId | null>(null)
  const [newlyCreatedWorkspaceId, setNewlyCreatedWorkspaceId] = useState<number | null>(null)
  const [optimisticArchivingWorkspaceIds, setOptimisticArchivingWorkspaceIds] = useState<Set<number>>(
    () => new Set(),
  )
  const [deletingProjectId, setDeletingProjectId] = useState<ProjectId | null>(null)
  const [projectToDelete, setProjectToDelete] = useState<{ id: ProjectId; name: string } | null>(null)
  const [newTaskOpen, setNewTaskOpen] = useState(false)
  const [feedbackOpen, setFeedbackOpen] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [settingsInitialSectionId, setSettingsInitialSectionId] = useState<SettingsSectionId | null>(null)
  const [settingsInitialAgentId, setSettingsInitialAgentId] = useState<string | null>(null)
  const [settingsInitialAgentFilePath, setSettingsInitialAgentFilePath] = useState<string | null>(null)
  const [pinnedProjectIds, setPinnedProjectIds] = useState<Set<string>>(() => {
    if (typeof window === "undefined") return new Set()
    const saved = loadJson<string[]>(PINNED_PROJECTS_KEY)
    return new Set(saved ?? [])
  })

  const togglePinProject = (projectId: string) => {
    setPinnedProjectIds((prev) => {
      const next = new Set(prev)
      if (next.has(projectId)) {
        next.delete(projectId)
      } else {
        next.add(projectId)
      }
      saveJson(PINNED_PROJECTS_KEY, Array.from(next))
      return next
    })
  }

  const [pinnedWorktreeIds, setPinnedWorktreeIds] = useState<Set<number>>(() => {
    if (typeof window === "undefined") return new Set()
    const saved = loadJson<number[]>(PINNED_WORKTREES_KEY)
    return new Set(saved ?? [])
  })

  const togglePinWorktree = (worktreeId: number) => {
    setPinnedWorktreeIds((prev) => {
      const next = new Set(prev)
      if (next.has(worktreeId)) {
        next.delete(worktreeId)
      } else {
        next.add(worktreeId)
      }
      saveJson(PINNED_WORKTREES_KEY, Array.from(next))
      return next
    })
  }

  const projectOrder = app?.ui?.sidebar_project_order ?? []
  const worktreeOrder = useMemo(() => {
    const record = app?.ui?.sidebar_worktree_order ?? {}
    return new Map(Object.entries(record) as [ProjectId, number[]][])
  }, [app?.ui?.sidebar_worktree_order])

  const [activeProjectDragId, setActiveProjectDragId] = useState<UniqueIdentifier | null>(null)
  const [activeWorktreeDragId, setActiveWorktreeDragId] = useState<UniqueIdentifier | null>(null)
  const [draggingProjectId, setDraggingProjectId] = useState<ProjectId | null>(null)
  const migratedSidebarOrderRef = useRef(false)

  const projectSensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  )

  const handleProjectDragStart = (event: DragStartEvent) => {
    setActiveProjectDragId(event.active.id)
  }

  const handleProjectDragEnd = (event: DragEndEvent) => {
    const { active, over } = event
    setActiveProjectDragId(null)

    if (over && active.id !== over.id) {
      const currentOrder = projects.map((p) => p.id)
      const oldIndex = currentOrder.indexOf(active.id as ProjectId)
      const newIndex = currentOrder.indexOf(over.id as ProjectId)
      if (oldIndex < 0 || newIndex < 0) return
      const nextOrder = arrayMove(currentOrder, oldIndex, newIndex)
      saveJson(PROJECT_ORDER_KEY, nextOrder)
      setSidebarProjectOrder(nextOrder)
    }
  }

  const handleProjectDragCancel = (_event: DragCancelEvent) => {
    setActiveProjectDragId(null)
  }

  const handleWorktreeDragStart = (projectId: ProjectId) => (event: DragStartEvent) => {
    setActiveWorktreeDragId(event.active.id)
    setDraggingProjectId(projectId)
  }

  const handleWorktreeDragEnd = (projectId: ProjectId, worktrees: SidebarWorktreeVm[]) => (event: DragEndEvent) => {
    const { active, over } = event
    setActiveWorktreeDragId(null)
    setDraggingProjectId(null)

    if (over && active.id !== over.id) {
      const currentOrder = worktrees.map((w) => w.workspaceId)
      const oldIndex = currentOrder.indexOf(active.id as number)
      const newIndex = currentOrder.indexOf(over.id as number)
      if (oldIndex < 0 || newIndex < 0) return
      const nextOrder = arrayMove(currentOrder, oldIndex, newIndex)
      const saved = loadJson<Record<ProjectId, number[]>>(WORKTREE_ORDER_KEY) ?? {}
      saved[projectId] = nextOrder
      saveJson(WORKTREE_ORDER_KEY, saved)
      setSidebarWorktreeOrder(projectId, nextOrder)
    }
  }

  useEffect(() => {
    if (typeof window === "undefined") return
    if (!app) return
    if (migratedSidebarOrderRef.current) return
    migratedSidebarOrderRef.current = true

    const serverProjectOrder = app.ui.sidebar_project_order ?? []
    const serverWorktreeOrder = app.ui.sidebar_worktree_order ?? {}
    if (serverProjectOrder.length > 0 || Object.keys(serverWorktreeOrder).length > 0) {
      return
    }

    const localProjectOrder = loadJson<ProjectId[]>(PROJECT_ORDER_KEY) ?? []
    const localWorktreeOrder = loadJson<Record<ProjectId, number[]>>(WORKTREE_ORDER_KEY) ?? {}
    if (localProjectOrder.length === 0 && Object.keys(localWorktreeOrder).length === 0) {
      return
    }

    const projectIds = app.projects.map((p) => p.id)
    const normalizedProjectOrder = [
      ...localProjectOrder.filter((id) => projectIds.includes(id)),
      ...projectIds.filter((id) => !localProjectOrder.includes(id)),
    ]
    if (normalizedProjectOrder.length > 0) {
      setSidebarProjectOrder(normalizedProjectOrder)
    }

    for (const project of app.projects) {
      const worktreeIds = project.workspaces.filter((w) => w.status === "active").map((w) => w.id)
      if (worktreeIds.length === 0) continue
      const localOrder = localWorktreeOrder[project.id] ?? []
      const normalizedOrder = [
        ...localOrder.filter((id) => worktreeIds.includes(id)),
        ...worktreeIds.filter((id) => !localOrder.includes(id)),
      ]
      if (normalizedOrder.length > 0) {
        setSidebarWorktreeOrder(project.id, normalizedOrder)
      }
    }

  }, [app?.rev, setSidebarProjectOrder, setSidebarWorktreeOrder])

  useEffect(() => {
    if (newlyCreatedWorkspaceId == null) return
    const t = window.setTimeout(() => setNewlyCreatedWorkspaceId(null), 1500)
    return () => window.clearTimeout(t)
  }, [newlyCreatedWorkspaceId])

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<OpenSettingsDetail | null>).detail
      setSettingsInitialSectionId(detail?.sectionId ?? "agent")
      setSettingsInitialAgentId(detail?.agentId ?? null)
      setSettingsInitialAgentFilePath(detail?.agentFilePath ?? null)
      setSettingsOpen(true)
    }
    window.addEventListener("luban:open-settings", handler)
    return () => window.removeEventListener("luban:open-settings", handler)
  }, [])

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<{ path?: string } | null>).detail
      const path = detail?.path?.trim()
      if (!path) return
      setSettingsOpen(false)
      setSettingsInitialSectionId(null)
      onViewModeChange("workspace")
      void (async () => {
        try {
          const res = await addProjectAndOpen(path)
          await openWorkspace(res.workspaceId)
          focusChatInput()
        } catch (err) {
          toast.error(err instanceof Error ? err.message : String(err))
        }
      })()
    }
    window.addEventListener("luban:add-project-and-open", handler)
    return () => window.removeEventListener("luban:add-project-and-open", handler)
  }, [addProjectAndOpen, onViewModeChange, openWorkspace])

  useEffect(() => {
    if (!app) return

    setOptimisticArchivingWorkspaceIds((prev) => {
      if (prev.size === 0) return prev

      const activeById = new Map<number, "idle" | "running">()
      for (const project of app.projects) {
        for (const workspace of project.workspaces) {
          if (workspace.status !== "active") continue
          activeById.set(workspace.id, workspace.archive_status)
        }
      }

      let changed = false
      const next = new Set<number>()
      for (const id of prev) {
        const status = activeById.get(id)
        if (!status) {
          changed = true
          continue
        }
        if (status !== "running") {
          changed = true
          continue
        }
        next.add(id)
      }
      return changed ? next : prev
    })

    if (deletingProjectId != null) {
      const stillExists = app.projects.some((p) => p.id === deletingProjectId)
      if (!stillExists) {
        setDeletingProjectId(null)
      }
    }

    const pendingProjectPath = pendingAddProjectPathRef.current
    if (pendingProjectPath) {
      const match = app.projects.find(
        (p) => normalizePathLike(p.path) === normalizePathLike(pendingProjectPath),
      )
      if (match) {
        pendingAddProjectPathRef.current = null

        if (!match.expanded) {
          toggleProjectExpanded(match.id)
        }

        const activeWorkspaces = match.workspaces.filter((w) => w.status === "active")
        if (activeWorkspaces.length === 0) {
          pendingOpenMainProjectIdRef.current = match.id
          ensureMainWorkspace(match.id)
          return
        }

        const main =
          activeWorkspaces.find((w) => w.workspace_name === "main" && w.worktree_path === match.path) ??
          activeWorkspaces[0] ??
          null
        if (!main) return

        void openWorkspace(main.id as WorkspaceId).then(() => focusChatInput())
      }
    }

    const pendingOpenMainProjectId = pendingOpenMainProjectIdRef.current
    if (pendingOpenMainProjectId != null) {
      const match = app.projects.find((p) => p.id === pendingOpenMainProjectId)
      if (match) {
        const activeWorkspaces = match.workspaces.filter((w) => w.status === "active")
        const main =
          activeWorkspaces.find((w) => w.workspace_name === "main" && w.worktree_path === match.path) ??
          activeWorkspaces[0] ??
          null
        if (main) {
          pendingOpenMainProjectIdRef.current = null
          void openWorkspace(main.id as WorkspaceId).then(() => focusChatInput())
        }
      }
    }

    if (optimisticCreatingProjectId != null) {
      const confirmed = app.projects.some(
        (p) => p.id === optimisticCreatingProjectId && p.create_workspace_status === "running",
      )
      if (confirmed) {
        setOptimisticCreatingProjectId(null)
      }
    }

    const pending = pendingCreateRef.current
    if (pending) {
      const pendingProject = app.projects.find((p) => p.id === pending.projectId)
      if (pendingProject?.create_workspace_status === "running") {
        return
      }

      pendingCreateRef.current = null
      setOptimisticCreatingProjectId(null)

      const created = pendingProject?.workspaces.find((w) => !pending.existingWorkspaceIds.has(w.id))?.id
      if (created == null) return

      setNewlyCreatedWorkspaceId(created)
      void openWorkspace(created).then(() => focusChatInput())
    }

    const running = app.projects.find((p) => p.create_workspace_status === "running")
    if (!running) return
    pendingCreateRef.current = {
      projectId: running.id,
      existingWorkspaceIds: new Set(running.workspaces.map((w) => w.id)),
    }
  }, [app?.rev])

  const projects: SidebarProjectVm[] = buildSidebarProjects(app, {
    optimisticArchivingWorkspaceIds,
    pinnedProjectIds,
    pinnedWorktreeIds,
    projectOrder,
    worktreeOrder,
  })

  const getActiveWorktreeCount = (worktrees: SidebarWorktreeVm[]) => {
    return worktrees.filter((w) => w.agentStatus !== "idle" || w.prStatus !== "none").length
  }

  const deriveStandaloneProjectStatus = (args: { isCreating: boolean; isGit: boolean }): AgentStatus | null => {
    if (!args.isGit) return "idle"
    if (args.isCreating) return "running"
    return "idle"
  }

  const handleAddProjectClick = async () => {
    try {
      const picked = await pickProjectPath()
      if (!picked) return
      pendingAddProjectPathRef.current = picked
      addProject(picked)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    }
  }

  const confirmDeleteProject = () => {
    if (!projectToDelete) return
    const projectId = projectToDelete.id
    setProjectToDelete(null)
    setDeletingProjectId(projectId)
    deleteProject(projectId)

    window.setTimeout(() => {
      setDeletingProjectId((prev) => (prev === projectId ? null : prev))
    }, 30_000)
  }

  return (
    <aside
      data-testid="left-sidebar"
      className="flex-shrink-0 border-r border-border bg-sidebar flex flex-col overflow-x-hidden"
      style={{ width: `${widthPx}px` }}
    >
      <div className="flex items-center justify-between h-11 px-3 border-b border-border">
        <button
          onClick={() => onViewModeChange(viewMode === "workspace" ? "kanban" : "workspace")}
          className="flex items-center gap-2 hover:bg-sidebar-accent px-1.5 py-1 rounded transition-colors"
        >
          <div className="flex items-center justify-center w-6 h-6 rounded bg-secondary">
            {viewMode === "workspace" ? (
              <Layers className="w-3.5 h-3.5 text-foreground" />
            ) : (
              <LayoutGrid className="w-3.5 h-3.5 text-foreground" />
            )}
          </div>
          <span className="text-sm font-medium">{viewMode === "workspace" ? "Workspace" : "Kanban"}</span>
          <ChevronDown className="w-3 h-3 text-muted-foreground" />
        </button>
      </div>

      {/* Project List */}
      <div
        data-testid="left-sidebar-scroll"
        className="flex-1 overflow-y-auto overflow-x-hidden overscroll-contain py-1.5"
      >
        <DndContext
          sensors={projectSensors}
          collisionDetection={closestCenter}
          onDragStart={handleProjectDragStart}
          onDragEnd={handleProjectDragEnd}
          onDragCancel={handleProjectDragCancel}
        >
          <SortableContext
            items={projects.map((p) => p.id)}
            strategy={verticalListSortingStrategy}
          >
        {projects.map((project) => {
          const activeCount = getActiveWorktreeCount(project.worktrees)
          const isCreating =
            project.createWorkspaceStatus === "running" || optimisticCreatingProjectId === project.id
          const isDeleting = deletingProjectId === project.id
          const standaloneMainWorktree =
            project.worktrees.length === 1 && project.worktrees[0]?.isHome ? project.worktrees[0] : null
          const canExpand = project.isGit && project.worktrees.length > 1
          const isExpanded = canExpand && project.expanded
          const standaloneStatus = canExpand
            ? null
            : standaloneMainWorktree?.agentStatus ??
              deriveStandaloneProjectStatus({ isCreating, isGit: project.isGit })
          const isStandaloneMainActive =
            standaloneMainWorktree != null && standaloneMainWorktree.workspaceId === activeWorkspaceId
          return (
            <SortableProject key={project.id} id={project.id} disabled={isDeleting}>
              <div
                className={cn(
                  "transition-transform duration-300",
                  isDeleting && "animate-pulse opacity-50 pointer-events-none",
                  project.pinned && "animate-in slide-in-from-bottom-1 fade-in duration-300",
                )}
              >
                <div
                  className={cn(
                    "relative flex items-center mx-1 rounded-md transition-colors",
                    isStandaloneMainActive ? "bg-primary/6" : "hover:bg-primary/4",
                  )}
                >
                <button
                  data-testid={standaloneMainWorktree ? "project-main-only-entry" : undefined}
                  onClick={() => {
                    if (canExpand) {
                      toggleProjectExpanded(project.id)
                      return
                    }
                    if (standaloneMainWorktree) {
                      void openWorkspace(standaloneMainWorktree.workspaceId)
                      return
                    }
                    if (project.worktrees.length === 0) {
                      pendingOpenMainProjectIdRef.current = project.id
                      ensureMainWorkspace(project.id)
                      return
                    }
                  }}
                  className={cn(
                    "flex-1 min-w-0 flex items-center gap-2 px-2 py-2.5 text-left transition-[padding] duration-150",
                    "group-hover/project:pr-24",
                    canExpand || standaloneMainWorktree || project.worktrees.length === 0
                      ? "cursor-pointer"
                      : "cursor-default",
                  )}
                >
                  {canExpand ? (
                    isExpanded ? (
                      <ChevronDown className="w-3.5 h-3.5 text-muted-foreground flex-shrink-0" />
                    ) : (
                      <ChevronRight className="w-3.5 h-3.5 text-muted-foreground flex-shrink-0" />
                    )
                  ) : standaloneStatus ? (
                    <span data-testid="project-agent-status-icon">
                      <AgentStatusIcon status={standaloneStatus} size="sm" />
                    </span>
                  ) : (
                    <span className="w-3.5 h-3.5 flex-shrink-0" />
                  )}
                  <span
                    className={cn(
                      "text-sm font-semibold truncate flex-1 min-w-0 transition-colors",
                      isStandaloneMainActive ? "text-foreground" : "text-foreground/80",
                    )}
                    title={project.path}
                  >
                    {project.displayName}
                  </span>
                  {project.pinned && (
                    <Pin
                      className="w-3 h-3 text-base09 flex-shrink-0 opacity-70 group-hover/project:opacity-0 transition-opacity duration-150"
                      aria-label="Pinned"
                    />
                  )}
                  {canExpand && !isExpanded && activeCount > 0 && (
                    <span className="text-xs px-1.5 py-0.5 rounded-full bg-primary/20 text-primary font-medium">
                      {activeCount}
                    </span>
                  )}
                </button>
                {isStandaloneMainActive && (
                  <div
                    data-testid="project-active-indicator"
                    className="absolute left-0 top-0 bottom-0 w-0.5 bg-primary rounded-l-md"
                  />
                )}
                <div
                  aria-hidden="true"
                  className={cn(
                    "absolute right-0 top-0 bottom-0 w-32 opacity-0 pointer-events-none transition-opacity duration-150 z-10",
                    "bg-gradient-to-l from-sidebar via-sidebar/90 to-transparent",
                    isStandaloneMainActive && "from-[hsl(var(--primary)/0.06)] via-[hsl(var(--primary)/0.04)]",
                    !isStandaloneMainActive && "group-hover/project:from-[hsl(var(--primary)/0.04)] group-hover/project:via-[hsl(var(--primary)/0.02)]",
                    "group-hover/project:opacity-100",
                  )}
                />
                <div className="absolute right-1 top-1/2 -translate-y-1/2 z-20 flex items-center gap-0 opacity-0 pointer-events-none group-hover/project:opacity-100 group-hover/project:pointer-events-auto transition-opacity duration-150">
                  <button
                    data-testid="project-pin-button"
                    onClick={(e) => {
                      e.stopPropagation()
                      togglePinProject(project.id)
                    }}
                    className="p-1 text-muted-foreground hover:text-foreground"
                    title={project.pinned ? "Unpin project" : "Pin project"}
                  >
                    {project.pinned ? <PinOff className="w-4 h-4" /> : <Pin className="w-4 h-4" />}
                  </button>
                  {project.isGit && (
                    <button
                      className="p-1 text-muted-foreground hover:text-foreground transition-colors"
                      title="Add worktree"
                      onClick={() => {
                        if (isCreating) return
                        if (!project.expanded) {
                          toggleProjectExpanded(project.id)
                        }
                        const fullProject = app?.projects.find((p) => p.id === project.id) ?? null
                        const existingWorkspaceIds = new Set<number>(
                          fullProject?.workspaces.map((w) => w.id) ?? project.worktrees.map((w) => w.workspaceId),
                        )
                        pendingCreateRef.current = { projectId: project.id, existingWorkspaceIds }
                        setOptimisticCreatingProjectId(project.id)
                        createWorkspace(project.id)
                      }}
                      disabled={isCreating}
                    >
                      {isCreating ? (
                        <Loader2 className="w-4 h-4 animate-spin text-primary" />
                      ) : (
                        <Plus className="w-4 h-4" />
                      )}
                    </button>
                  )}
                  <button
                    data-testid="project-delete-button"
                    onClick={() => setProjectToDelete({ id: project.id, name: project.displayName })}
                    disabled={isDeleting}
                    className="p-1 text-muted-foreground hover:text-destructive transition-colors"
                    title="Delete project"
                  >
                    {isDeleting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Trash2 className="w-4 h-4" />}
                  </button>
                </div>
              </div>

              {isExpanded && (
                <DndContext
                  sensors={projectSensors}
                  collisionDetection={closestCenter}
                  onDragStart={handleWorktreeDragStart(project.id)}
                  onDragEnd={handleWorktreeDragEnd(project.id, project.worktrees)}
                >
                  <SortableContext
                    items={project.worktrees.map((w) => w.workspaceId)}
                    strategy={verticalListSortingStrategy}
                  >
                <div className="">
                  {project.worktrees.map((worktree, idx) => (
                    <SortableWorktree
                      key={worktree.workspaceId}
                      id={worktree.workspaceId}
                      disabled={worktree.isArchiving}
                    >
                    <div
                      data-testid="worktree-row"
                      className={cn(
                        "group/worktree relative flex items-center gap-2 px-2 py-1.5 mx-1 rounded-md cursor-pointer outline-none",
                        worktree.workspaceId === activeWorkspaceId ? "bg-primary/6" : "hover:bg-primary/4",
                        newlyCreatedWorkspaceId === worktree.workspaceId &&
                          "animate-in slide-in-from-left-2 fade-in duration-300 ring-1 ring-primary/30",
                        worktree.pinned && newlyCreatedWorkspaceId !== worktree.workspaceId &&
                          "animate-in slide-in-from-bottom-1 fade-in duration-300",
                        worktree.isArchiving && "animate-pulse opacity-50 pointer-events-none",
                      )}
                      style={{
                        animationDelay:
                          newlyCreatedWorkspaceId === worktree.workspaceId ? "0ms" : `${idx * 30}ms`,
                      }}
                      onClick={() => {
                        void openWorkspace(worktree.workspaceId)
                      }}
                    >
                      {worktree.isArchiving ? (
                        <Loader2
                          data-testid="worktree-archiving-spinner"
                          className="w-3.5 h-3.5 animate-spin text-muted-foreground flex-shrink-0"
                        />
                      ) : (
                        <span className="flex-shrink-0">
                          <AgentStatusIcon status={worktree.agentStatus} size="sm" />
                        </span>
                      )}

                      <div className="flex flex-col flex-1 min-w-0">
                        <div className="flex items-center gap-1">
                          <span
                            data-testid="worktree-branch-name"
                            className="text-xs text-foreground truncate"
                            title={worktree.name}
                          >
                            {worktree.name}
                          </span>
                          {worktree.pinned && (
                            <Pin
                              className="w-2.5 h-2.5 text-base09 flex-shrink-0 opacity-70"
                              aria-label="Pinned"
                            />
                          )}
                        </div>
                        <span
                          data-testid="worktree-worktree-name"
                          className="text-[10px] text-muted-foreground/50 font-mono truncate"
                          title={worktree.id}
                        >
                          {worktree.worktreeName}
                        </span>
                      </div>

                      <PRBadge
                        status={worktree.prStatus}
                        prNumber={worktree.prNumber}
                        workspaceId={worktree.workspaceId}
                        onOpenPullRequest={openWorkspacePullRequest}
                        onOpenPullRequestFailedAction={openWorkspacePullRequestFailedAction}
                        titleOverride={worktree.prTitle}
                      />

                      {/* Action buttons - fixed position, opacity only */}
                      <div className="flex items-center gap-0.5 opacity-0 group-hover/worktree:opacity-100 transition-opacity">
                        <button
                          data-testid="worktree-pin-button"
                          onClick={(e) => {
                            e.stopPropagation()
                            togglePinWorktree(worktree.workspaceId)
                          }}
                          className="p-0.5 text-muted-foreground hover:text-foreground"
                          title={worktree.pinned ? "Unpin worktree" : "Pin worktree"}
                        >
                          {worktree.pinned ? <PinOff className="w-3.5 h-3.5" /> : <Pin className="w-3.5 h-3.5" />}
                        </button>
                        {worktree.isHome ? (
                          <span
                            data-testid="worktree-home-icon"
                            className="p-0.5 text-muted-foreground/50"
                            title="Main worktree"
                          >
                            <Home className="w-3.5 h-3.5" />
                          </span>
                        ) : (
                          <button
                            className={cn(
                              "p-0.5 text-muted-foreground hover:text-foreground",
                              worktree.isArchiving && "opacity-50",
                            )}
                            title="Archive worktree"
                            onClick={(e) => {
                              e.stopPropagation()
                              setOptimisticArchivingWorkspaceIds((prev) => {
                                const next = new Set(prev)
                                next.add(worktree.workspaceId)
                                return next
                              })
                              archiveWorkspace(worktree.workspaceId)
                            }}
                            disabled={worktree.isArchiving}
                          >
                            <Archive className="w-3.5 h-3.5" />
                          </button>
                        )}
                      </div>

                      {worktree.workspaceId === activeWorkspaceId && (
                        <div
                          data-testid="worktree-active-indicator"
                          className="absolute left-0 top-0 bottom-0 w-0.5 bg-primary rounded-l-md"
                        />
                      )}
                    </div>
                    </SortableWorktree>
                  ))}

                  {isCreating && (
                    <div className="flex items-center gap-2 px-2 py-1.5 mx-1 animate-pulse">
                      <div className="flex flex-col flex-1 gap-1">
                        <div className="flex items-center gap-1.5">
                          <div className="w-3 h-3 rounded bg-muted-foreground/20" />
                          <div className="h-3 w-20 rounded bg-muted-foreground/20" />
                        </div>
                        <div className="h-2.5 w-8 ml-4 rounded bg-muted-foreground/15" />
                      </div>
                    </div>
                  )}
                </div>
                  </SortableContext>
                  <DragOverlay dropAnimation={{
                    duration: 250,
                    easing: "cubic-bezier(0.18, 0.67, 0.6, 1.22)",
                  }}>
                    {activeWorktreeDragId && draggingProjectId === project.id ? (
                      <WorktreeDragOverlay
                        worktree={project.worktrees.find((w) => w.workspaceId === activeWorktreeDragId)!}
                      />
                    ) : null}
                  </DragOverlay>
                </DndContext>
              )}
              </div>
            </SortableProject>
	          )
	        })}
          </SortableContext>
          <DragOverlay dropAnimation={{
            duration: 250,
            easing: "cubic-bezier(0.18, 0.67, 0.6, 1.22)",
          }}>
            {activeProjectDragId ? (
              <ProjectDragOverlay
                project={projects.find((p) => p.id === activeProjectDragId)!}
              />
            ) : null}
          </DragOverlay>
        </DndContext>

        <button
          onClick={() => void handleAddProjectClick()}
          className="w-full flex items-center gap-2 px-3 py-1.5 text-left text-muted-foreground/60 hover:text-muted-foreground hover:bg-sidebar-accent/50 transition-colors"
        >
          <Plus className="w-3 h-3 flex-shrink-0" />
          <span className="text-[13px]">Add project</span>
        </button>
      </div>

      {/* Bottom Actions */}
      <div className="border-t border-border p-2 flex items-center gap-2">
        <button
          onClick={() => setNewTaskOpen(true)}
          className="flex-1 flex items-center gap-2 px-3 py-2 text-sm text-muted-foreground hover:text-foreground hover:bg-sidebar-accent rounded transition-colors"
        >
          <Sparkles className="w-4 h-4 text-primary" />
          New Task
        </button>
        <button
          data-testid="sidebar-open-feedback"
          onClick={() => setFeedbackOpen(true)}
          className="p-2 text-muted-foreground hover:text-foreground hover:bg-sidebar-accent rounded transition-colors"
          title="Send Feedback"
        >
          <MessageCircleQuestion className="w-4 h-4" />
        </button>
        <button
          data-testid="sidebar-open-settings"
          onClick={() => {
            setSettingsInitialSectionId(null)
            setSettingsInitialAgentId(null)
            setSettingsInitialAgentFilePath(null)
            setSettingsOpen(true)
          }}
          className="p-2 text-muted-foreground hover:text-foreground hover:bg-sidebar-accent rounded transition-colors"
          title="Settings"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>

      <NewTaskModal open={newTaskOpen} onOpenChange={setNewTaskOpen} />
      <FeedbackModal open={feedbackOpen} onOpenChange={setFeedbackOpen} />
      <SettingsPanel
        open={settingsOpen}
        onOpenChange={(open) => {
          setSettingsOpen(open)
          if (!open) {
            setSettingsInitialSectionId(null)
            setSettingsInitialAgentId(null)
            setSettingsInitialAgentFilePath(null)
          }
        }}
        initialSectionId={settingsInitialSectionId ?? undefined}
        initialAgentId={settingsInitialAgentId ?? undefined}
        initialAgentFilePath={settingsInitialAgentFilePath ?? undefined}
      />

      <Dialog open={projectToDelete !== null} onOpenChange={(open) => !open && setProjectToDelete(null)}>
        <DialogContent
          data-testid="project-delete-dialog"
          showCloseButton={false}
          className="sm:max-w-[400px] p-0 gap-0 bg-background border-border overflow-hidden rounded-lg"
        >
          <div className="px-5 py-4 border-b border-border">
            <h2 className="text-base font-medium flex items-center gap-2">
              <Trash2 className="w-4 h-4 text-destructive" />
              Delete Project
            </h2>
          </div>

          <div className="p-5">
            <p className="text-sm text-muted-foreground">
              Are you sure you want to delete{" "}
              <span className="font-medium text-foreground">&quot;{projectToDelete?.name ?? ""}&quot;</span>? This action
              cannot be undone.
            </p>
            <p className="text-[11px] text-muted-foreground/70 mt-2">
              Your local files will not be affected. Only files managed by Luban will be removed.
            </p>
          </div>

          <div className="px-5 py-4 border-t border-border bg-secondary/30 flex items-center justify-end gap-2">
            <Button
              data-testid="project-delete-cancel"
              variant="outline"
              size="sm"
              onClick={() => setProjectToDelete(null)}
            >
              Cancel
            </Button>
            <Button
              data-testid="project-delete-confirm"
              variant="destructive"
              size="sm"
              onClick={confirmDeleteProject}
            >
              Delete
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </aside>
  )
}
