"use client"

import type React from "react"

import { useState, useRef, useMemo } from "react"
import {
  ChevronDown,
  ChevronRight,
  Plus,
  Settings,
  Sparkles,
  LayoutGrid,
  Layers,
  Archive,
  Loader2,
  Home,
  Trash2,
  MessageCircleQuestion,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { NewTaskModal } from "./new-task-modal"
import { AgentStatusIcon, PRBadge, type AgentStatus } from "./shared/status-indicator"
import type { Worktree } from "./shared/worktree"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { SettingsPanel } from "./settings-panel"
import { FeedbackModal } from "./feedback-modal"

interface Project {
  name: string
  path: string // Full path to the project
  isGit: boolean
  worktrees: Worktree[]
  expanded?: boolean
  agentStatus?: AgentStatus // Agent status for non-git projects
}

/**
 * Computes minimal unique display names for projects with conflicting basenames.
 * For example, given paths:
 *   - /Users/alice/projects/luban
 *   - /Users/bob/work/luban
 * Returns:
 *   - alice/projects/luban
 *   - bob/work/luban
 */
function computeDisplayNames(projects: Project[]): Map<string, string> {
  const result = new Map<string, string>()

  // Group projects by their basename
  const byBasename = new Map<string, Project[]>()
  for (const project of projects) {
    const basename = project.path.split("/").filter(Boolean).pop() || project.name
    const group = byBasename.get(basename) || []
    group.push(project)
    byBasename.set(basename, group)
  }

  for (const [basename, group] of byBasename) {
    if (group.length === 1) {
      // No conflict, use basename
      result.set(group[0].path, basename)
    } else {
      // Conflict detected, find minimal unique suffix for each
      const pathSegments = group.map((p) => p.path.split("/").filter(Boolean).reverse())

      // Start with 1 segment (basename), expand until all are unique
      let depth = 1
      while (depth <= Math.max(...pathSegments.map((s) => s.length))) {
        const suffixes = pathSegments.map((segs) =>
          segs.slice(0, depth).reverse().join("/")
        )
        const uniqueSuffixes = new Set(suffixes)
        if (uniqueSuffixes.size === group.length) {
          // All unique at this depth
          group.forEach((p, i) => result.set(p.path, suffixes[i]))
          break
        }
        depth++
      }

      // Fallback: if still not unique, use full path
      if (!result.has(group[0].path)) {
        group.forEach((p) => result.set(p.path, p.path))
      }
    }
  }

  return result
}

const projects: Project[] = [
  // Case 1: Git project with worktrees (expanded)
  {
    name: "luban",
    path: "/Users/xuanwo/Code/xuanwo/luban",
    isGit: true,
    worktrees: [
      { id: "lb01", name: "main", isHome: true, agentStatus: "idle", prStatus: "none" },
      { id: "lb02", name: "typical-inch", agentStatus: "running", prStatus: "none" },
      {
        id: "lb03",
        name: "scroll-fix",
        agentStatus: "idle",
        prStatus: "ci-running",
        prNumber: 1234,
        prUrl: "https://github.com/user/luban/pull/1234",
      },
      {
        id: "lb04",
        name: "auth-refactor",
        agentStatus: "idle",
        prStatus: "review-pending",
        prNumber: 1201,
        prUrl: "https://github.com/user/luban/pull/1201",
      },
    ],
    expanded: true,
  },
  // Conflicting name example: another "luban" project in a different location
  {
    name: "luban",
    path: "/Users/xuanwo/Work/projects/luban",
    isGit: true,
    worktrees: [
      { id: "lb21", name: "main", isHome: true, agentStatus: "idle", prStatus: "none" },
    ],
    expanded: true,
  },
  // Case 2: Git project without worktrees (click + to create first worktree)
  {
    name: "opendal",
    path: "/Users/xuanwo/Code/apache/opendal",
    isGit: true,
    worktrees: [],
    agentStatus: "idle",
  },
  // Case 3: Non-git project (no worktree support)
  {
    name: "my-notes",
    path: "/Users/xuanwo/Documents/my-notes",
    isGit: false,
    worktrees: [],
    agentStatus: "idle",
  },
  // More examples
  {
    name: "lance-duckdb",
    path: "/Users/xuanwo/Code/lance/lance-duckdb",
    isGit: true,
    worktrees: [
      { id: "ld01", name: "main", isHome: true, agentStatus: "idle", prStatus: "none" },
      {
        id: "ld02",
        name: "perf-opt",
        agentStatus: "idle",
        prStatus: "ci-failed",
        prNumber: 89,
        prUrl: "https://github.com/user/lance-duckdb/pull/89",
        ciUrl: "https://github.com/user/lance-duckdb/actions/runs/123456",
      },
    ],
    expanded: true,
  },
  {
    name: "blog",
    path: "/Users/xuanwo/Code/xuanwo/blog",
    isGit: true,
    worktrees: [
      { id: "bg01", name: "main", isHome: true, agentStatus: "idle", prStatus: "none" },
      { id: "bg02", name: "new-post", agentStatus: "pending", prStatus: "none" },
    ],
  },
  {
    name: "opendalfs",
    path: "/Users/xuanwo/Code/apache/opendalfs",
    isGit: true,
    worktrees: [
      { id: "of01", name: "main", isHome: true, agentStatus: "idle", prStatus: "none" },
      {
        id: "of02",
        name: "mount-impl",
        agentStatus: "idle",
        prStatus: "ready-to-merge",
        prNumber: 456,
        prUrl: "https://github.com/user/opendalfs/pull/456",
      },
    ],
  },
  {
    name: "config-files",
    path: "/Users/xuanwo/.config/config-files",
    isGit: false,
    worktrees: [],
    agentStatus: "running",
  },
  {
    name: "gpui-ghostty",
    path: "/Users/xuanwo/Code/zed/gpui-ghostty",
    isGit: true,
    worktrees: [],
    agentStatus: "pending",
  },
]

const adjectives = ["swift", "quiet", "bold", "calm", "dark", "fresh", "keen", "warm", "wild", "wise"]
const nouns = ["river", "moon", "leaf", "star", "wave", "pine", "dawn", "mist", "peak", "snow"]

function generateWorktreeName() {
  const adj = adjectives[Math.floor(Math.random() * adjectives.length)]
  const noun = nouns[Math.floor(Math.random() * nouns.length)]
  return `${adj}-${noun}`
}

function generateWorktreeId(prefix: string) {
  const num = Math.floor(Math.random() * 90) + 10
  return `${prefix}${num}`
}

interface SidebarProps {
  viewMode: "workspace" | "kanban"
  onViewModeChange: (mode: "workspace" | "kanban") => void
}

export function Sidebar({ viewMode, onViewModeChange }: SidebarProps) {
  // Use path as unique identifier since name can be duplicated
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(
    new Set([
      "/Users/xuanwo/Code/xuanwo/luban",
      "/Users/xuanwo/Work/projects/luban",
      "/Users/xuanwo/Code/lance/lance-duckdb",
      "/Users/xuanwo/Code/xuanwo/blog",
      "/Users/xuanwo/Code/apache/opendalfs",
    ]),
  )

  const [projectsState, setProjectsState] = useState<Project[]>(projects)
  const [creatingInProject, setCreatingInProject] = useState<string | null>(null)
  const [newlyCreatedId, setNewlyCreatedId] = useState<string | null>(null)
  const [archivingId, setArchivingId] = useState<string | null>(null)
  const [deletingProject, setDeletingProject] = useState<string | null>(null)
  const [projectToDelete, setProjectToDelete] = useState<string | null>(null) // 待确认删除的项目
  // 当前选中的对话：可以是 project 或 worktree
  const [activeConversation, setActiveConversation] = useState<
    { type: "project"; path: string } | { type: "worktree"; projectPath: string; worktreeId: string }
  >({ type: "worktree", projectPath: "/Users/xuanwo/Code/xuanwo/luban", worktreeId: "lb02" })
  const [newTaskOpen, setNewTaskOpen] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [feedbackOpen, setFeedbackOpen] = useState(false)
  const folderInputRef = useRef<HTMLInputElement>(null)

  // Compute display names for projects with conflicting basenames
  const displayNames = useMemo(() => computeDisplayNames(projectsState), [projectsState])

  const toggleProject = (path: string) => {
    setExpandedProjects((prev) => {
      const next = new Set(prev)
      if (next.has(path)) {
        next.delete(path)
      } else {
        next.add(path)
      }
      return next
    })
  }

  const handleCreateWorktree = async (projectPath: string) => {
    setExpandedProjects((prev) => new Set([...prev, projectPath]))
    setCreatingInProject(projectPath)

    await new Promise((resolve) => setTimeout(resolve, 800))

    const project = projectsState.find((p) => p.path === projectPath)
    const prefix = (project?.name || "wt").slice(0, 2).toLowerCase()
    const isFirstWorktree = project?.worktrees.length === 0

    const newWorktree: Worktree = {
      id: generateWorktreeId(prefix),
      name: generateWorktreeName(),
      agentStatus: "idle",
      prStatus: "none",
    }

    setProjectsState((prev) =>
      prev.map((p) => {
        if (p.path !== projectPath) return p
        if (isFirstWorktree) {
          const mainWorktree: Worktree = {
            id: generateWorktreeId(prefix),
            name: "main",
            isHome: true,
            agentStatus: "idle",
            prStatus: "none",
          }
          return { ...p, worktrees: [mainWorktree, newWorktree] }
        }
        return { ...p, worktrees: [...p.worktrees, newWorktree] }
      }),
    )

    setCreatingInProject(null)
    setNewlyCreatedId(newWorktree.id)

    setTimeout(() => {
      setNewlyCreatedId(null)
    }, 1500)
  }

  const getActiveWorktreeCount = (worktrees: Worktree[]) => {
    return worktrees.filter((w) => w.agentStatus !== "idle" || w.prStatus !== "none").length
  }

  const handleFolderSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files
    if (files && files.length > 0) {
      const path = files[0].webkitRelativePath
      const folderName = path.split("/")[0]
      console.log("Selected folder:", folderName)
    }
    e.target.value = ""
  }

  const handleAddProjectClick = () => {
    folderInputRef.current?.click()
  }

  const handleArchiveWorktree = async (projectPath: string, worktreeId: string) => {
    setArchivingId(worktreeId)

    await new Promise((resolve) => setTimeout(resolve, 1500))

    // Remove worktree from state
    setProjectsState((prev) =>
      prev.map((p) =>
        p.path === projectPath ? { ...p, worktrees: p.worktrees.filter((w) => w.id !== worktreeId) } : p,
      ),
    )

    setArchivingId(null)
  }

  const handleDeleteProject = (projectPath: string) => {
    setProjectToDelete(projectPath)
  }

  const confirmDeleteProject = async () => {
    if (!projectToDelete) return

    const projectPath = projectToDelete
    setProjectToDelete(null)
    setDeletingProject(projectPath)
    await new Promise((resolve) => setTimeout(resolve, 1000))

    setProjectsState((prev) => prev.filter((p) => p.path !== projectPath))
    setExpandedProjects((prev) => {
      const next = new Set(prev)
      next.delete(projectPath)
      return next
    })
    setDeletingProject(null)
  }

  return (
    <aside className="w-60 flex-shrink-0 border-r border-border bg-sidebar flex flex-col">
      <div className="flex items-center justify-between h-11 px-3 border-b border-border">
        <button
          onClick={() => onViewModeChange(viewMode === "workspace" ? "kanban" : "workspace")}
          className="flex items-center gap-2 hover:bg-sidebar-accent px-1.5 py-1 rounded transition-colors"
        >
          <div className="flex items-center justify-center w-6 h-6 rounded bg-secondary">
            {viewMode === "workspace" ? (
              <Layers className="w-4 h-4 text-foreground" />
            ) : (
              <LayoutGrid className="w-4 h-4 text-foreground" />
            )}
          </div>
          <span className="text-sm font-medium">{viewMode === "workspace" ? "Workspace" : "Kanban"}</span>
          <ChevronDown className="w-3 h-3 text-muted-foreground" />
        </button>
      </div>

      {/* Project List */}
      <div className="flex-1 overflow-y-auto py-1.5">
        {projectsState.map((project) => {
          const activeCount = getActiveWorktreeCount(project.worktrees)
          const isCreating = creatingInProject === project.path
          const isDeleting = deletingProject === project.path
          const hasWorktrees = project.worktrees.length > 0
          const canExpand = project.isGit && hasWorktrees
          const displayName = displayNames.get(project.path) || project.name

          // 判断当前项目是否被选中（仅当选中 project 类型且路径匹配时）
          const isProjectActive =
            activeConversation.type === "project" && activeConversation.path === project.path

          return (
            <div
              key={project.path}
              className={cn(
                "group/project",
                isDeleting && "animate-pulse opacity-50 pointer-events-none",
              )}
            >
              <div
                className={cn(
                  "relative flex items-center transition-colors",
                  isProjectActive ? "bg-primary/6" : "hover:bg-sidebar-accent/50",
                )}
              >
                <button
                  onClick={() => {
                    // 对于没有 worktree 的 git 项目或 non-git 项目，点击选中该项目
                    if (!canExpand) {
                      setActiveConversation({ type: "project", path: project.path })
                    }
                    // 有 worktree 时仅展开/收起
                    if (canExpand) toggleProject(project.path)
                  }}
                  className={cn(
                    "flex-1 flex items-center gap-2 px-3 py-1.5 text-left",
                    canExpand ? "cursor-pointer" : project.agentStatus ? "cursor-pointer" : "cursor-default",
                  )}
                >
                  {canExpand ? (
                    expandedProjects.has(project.path) ? (
                      <ChevronDown className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                    ) : (
                      <ChevronRight className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                    )
                  ) : project.agentStatus ? (
                    <AgentStatusIcon status={project.agentStatus} size="xs" />
                  ) : (
                    <span className="w-3 h-3 flex-shrink-0" />
                  )}
                  <span
                    className={cn(
                      "text-sm truncate flex-1 transition-colors",
                      isProjectActive ? "text-foreground" : "text-muted-foreground",
                    )}
                    title={project.path}
                  >
                    {displayName}
                  </span>
                  {!expandedProjects.has(project.path) && activeCount > 0 && (
                    <span className="text-xs px-1.5 py-0.5 rounded-full bg-primary/20 text-primary font-medium">
                      {activeCount}
                    </span>
                  )}
                </button>
                {/* Bottom underline for active project (tab-style) */}
                {isProjectActive && (
                  <div className="absolute bottom-0 left-3 right-3 h-0.5 bg-primary rounded-full" />
                )}
                <div className="flex items-center gap-0.5 pr-2 opacity-0 group-hover/project:opacity-100 transition-opacity">
                  {project.isGit && (
                    <button
                      onClick={() => handleCreateWorktree(project.path)}
                      disabled={isCreating}
                      className="p-1 text-muted-foreground hover:text-foreground transition-colors"
                      title="Add worktree"
                    >
                      {isCreating ? (
                        <Loader2 className="w-4 h-4 animate-spin text-primary" />
                      ) : (
                        <Plus className="w-4 h-4" />
                      )}
                    </button>
                  )}
                  <button
                    onClick={() => handleDeleteProject(project.path)}
                    disabled={isDeleting}
                    className="p-1 text-muted-foreground hover:text-destructive transition-colors"
                    title="Delete project"
                  >
                    {isDeleting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Trash2 className="w-4 h-4" />}
                  </button>
                </div>
              </div>

              {canExpand && expandedProjects.has(project.path) && (
                <div className="ml-4 pl-3 border-l border-border-subtle">
                  {project.worktrees.map((worktree, idx) => {
                    const isWorktreeActive =
                      activeConversation.type === "worktree" &&
                      activeConversation.projectPath === project.path &&
                      activeConversation.worktreeId === worktree.id

                    return (
                      <div
                        key={worktree.id}
                        onClick={() =>
                          setActiveConversation({
                            type: "worktree",
                            projectPath: project.path,
                            worktreeId: worktree.id,
                          })
                        }
                        className={cn(
                          "group/worktree relative flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all duration-200 cursor-pointer",
                          isWorktreeActive
                            ? "bg-glass-surface shadow-[var(--shadow-glass)]"
                            : "hover:bg-glass-surface-muted/50",
                          newlyCreatedId === worktree.id &&
                            "animate-in slide-in-from-left-2 fade-in duration-300 ring-1 ring-primary/30",
                          archivingId === worktree.id && "animate-pulse opacity-50 pointer-events-none",
                        )}
                        style={{
                          animationDelay: newlyCreatedId === worktree.id ? "0ms" : `${idx * 30}ms`,
                        }}
                      >
                        {archivingId === worktree.id ? (
                          <Loader2 className="w-3.5 h-3.5 animate-spin text-muted-foreground" />
                        ) : (
                          <AgentStatusIcon status={worktree.agentStatus} size="sm" className="relative" />
                        )}

                        {/* Branch name and ID */}
                        <div className="flex flex-col flex-1 min-w-0">
                          <span className="text-xs text-foreground truncate" title={worktree.name}>
                            {worktree.name}
                          </span>
                          <span className="text-xs text-muted-foreground/50 font-mono">{worktree.id}</span>
                        </div>

                        <PRBadge
                          status={worktree.prStatus}
                          prNumber={worktree.prNumber}
                          prUrl={worktree.prUrl}
                          ciUrl={worktree.ciUrl}
                        />

                        {worktree.isHome ? (
                          <span
                            className="p-0.5 text-muted-foreground/50 opacity-0 group-hover/worktree:opacity-100 transition-opacity"
                            title="Main worktree"
                          >
                            <Home className="w-3 h-3" />
                          </span>
                        ) : (
                          <button
                            onClick={(e) => {
                              e.stopPropagation()
                              handleArchiveWorktree(project.path, worktree.id)
                            }}
                            disabled={archivingId === worktree.id}
                            className={cn(
                              "p-0.5 text-muted-foreground hover:text-foreground transition-opacity",
                              archivingId === worktree.id ? "opacity-50" : "opacity-0 group-hover/worktree:opacity-100",
                            )}
                            title="Archive worktree"
                          >
                            <Archive className="w-3 h-3" />
                          </button>
                        )}


                      </div>
                    )
                  })}

                  {isCreating && (
                    <div className="flex items-center gap-2 px-2 py-1.5 mx-1 animate-pulse">
                      <div className="w-3.5 h-3.5 rounded-full bg-muted-foreground/20" />
                      <div className="flex flex-col flex-1 gap-1">
                        <div className="h-3 w-20 rounded bg-muted-foreground/20" />
                        <div className="h-2.5 w-8 rounded bg-muted-foreground/15" />
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          )
        })}

        {/* Add Project Entry */}
        <button
          onClick={handleAddProjectClick}
          className="w-full flex items-center gap-2 px-3 py-1.5 text-left text-muted-foreground/60 hover:text-muted-foreground hover:bg-sidebar-accent/50 transition-colors"
        >
          <Plus className="w-3 h-3 flex-shrink-0" />
          <span className="text-sm">Add project</span>
        </button>
        <input
          ref={folderInputRef}
          type="file"
          className="hidden"
          onChange={handleFolderSelect}
          {...({ webkitdirectory: "", directory: "" } as React.InputHTMLAttributes<HTMLInputElement>)}
        />
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
          onClick={() => setFeedbackOpen(true)}
          className="p-2 text-muted-foreground hover:text-foreground hover:bg-sidebar-accent rounded transition-colors"
          title="Send Feedback"
        >
          <MessageCircleQuestion className="w-4 h-4" />
        </button>
        <button
          onClick={() => setSettingsOpen(true)}
          className="p-2 text-muted-foreground hover:text-foreground hover:bg-sidebar-accent rounded transition-colors"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>

      <NewTaskModal open={newTaskOpen} onOpenChange={setNewTaskOpen} />

      <SettingsPanel open={settingsOpen} onOpenChange={setSettingsOpen} />

      <FeedbackModal open={feedbackOpen} onOpenChange={setFeedbackOpen} />

      {/* Delete Project Confirmation Dialog */}
      <Dialog open={projectToDelete !== null} onOpenChange={(open) => !open && setProjectToDelete(null)}>
        <DialogContent showCloseButton={false} className="sm:max-w-[400px] p-0 gap-0 bg-background border-border overflow-hidden rounded-lg">
          <div className="px-5 py-4 border-b border-border">
            <h2 className="text-base font-medium flex items-center gap-2">
              <Trash2 className="w-4 h-4 text-destructive" />
              Delete Project
            </h2>
          </div>

          <div className="p-5">
            <p className="text-sm text-muted-foreground">
              Are you sure you want to delete{" "}
              <span className="font-medium text-foreground">{`"${projectToDelete ? (displayNames.get(projectToDelete) || projectToDelete) : ""}"`}</span>?
              This action cannot be undone.
            </p>
            <p className="text-[11px] text-muted-foreground/70 mt-2">
              Your local files will not be affected. Only files managed by Luban will be removed.
            </p>
          </div>

          <div className="px-5 py-4 border-t border-border bg-secondary/30 flex items-center justify-end gap-2">
            <Button variant="outline" size="sm" onClick={() => setProjectToDelete(null)}>
              Cancel
            </Button>
            <Button variant="destructive" size="sm" onClick={confirmDeleteProject}>
              Delete
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </aside>
  )
}
