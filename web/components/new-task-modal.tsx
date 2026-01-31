"use client"

import type React from "react"
import { useEffect, useMemo, useRef, useState } from "react"
import { toast } from "sonner"
import {
  X,
  Maximize2,
  GitBranch,
  Paperclip,
  Check,
} from "lucide-react"

import { useLuban } from "@/lib/luban-context"
import type { TaskExecuteMode } from "@/lib/luban-api"
import { draftKey } from "@/lib/ui-prefs"
import { focusChatInput } from "@/lib/focus-chat-input"
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@/components/ui/dropdown-menu"
import type { AppSnapshot } from "@/lib/luban-api"

interface NewTaskModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function NewTaskModal({ open, onOpenChange }: NewTaskModalProps) {
  const { app, executeTask, openWorkdir, activateTask, activeWorkdirId, createWorkdir, ensureMainWorkdir } = useLuban()

  const [input, setInput] = useState("")
  const [executingMode, setExecutingMode] = useState<TaskExecuteMode | null>(null)
  const [selectedProjectId, setSelectedProjectId] = useState<string>("")
  const [selectedWorkdirId, setSelectedWorkdirId] = useState<number | null>(null)
  const [projectSearch, setProjectSearch] = useState("")
  const [workdirSearch, setWorkdirSearch] = useState("")
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const appRef = useRef<AppSnapshot | null>(null)

  useEffect(() => {
    appRef.current = app
  }, [app])

  const normalizePathLike = (raw: string) => raw.trim().replace(/\/+$/, "")

  const projectOptions = useMemo(() => {
    return (app?.projects ?? []).map((p) => ({
      id: p.id,
      name: p.name,
      path: p.path,
      slug: p.slug,
      isGit: p.is_git,
      workdirs: p.workdirs.filter((w) => w.status === "active"),
    }))
  }, [app])

  const defaultProjectId = useMemo(() => {
    if (activeWorkdirId != null) {
      const opt = projectOptions.find((p) => p.workdirs.some((w) => w.id === activeWorkdirId)) ?? null
      if (opt) return opt.id
    }
    if (projectOptions.length === 1) return projectOptions[0]?.id ?? ""
    return ""
  }, [activeWorkdirId, projectOptions])

  useEffect(() => {
    if (!open) return
    if (selectedProjectId) return
    if (!defaultProjectId) return
    setSelectedProjectId(defaultProjectId)
  }, [defaultProjectId, open, selectedProjectId])

  const selectedProject = useMemo(() => {
    if (!selectedProjectId) return null
    return projectOptions.find((p) => p.id === selectedProjectId) ?? null
  }, [projectOptions, selectedProjectId])

  const workdirOptions = useMemo(() => selectedProject?.workdirs ?? [], [selectedProject?.workdirs])

  const filteredProjects = useMemo(() => {
    if (!projectSearch.trim()) return projectOptions
    const search = projectSearch.toLowerCase()
    return projectOptions.filter(
      (p) =>
        (p.name?.toLowerCase().includes(search)) ||
        (p.slug?.toLowerCase().includes(search)) ||
        (p.path?.toLowerCase().includes(search))
    )
  }, [projectOptions, projectSearch])

  const filteredWorkdirs = useMemo(() => {
    if (!workdirSearch.trim()) return workdirOptions
    const search = workdirSearch.toLowerCase()
    return workdirOptions.filter(
      (w) =>
        (w.workdir_name?.toLowerCase().includes(search)) ||
        (w.branch_name?.toLowerCase().includes(search)) ||
        (w.workdir_path?.toLowerCase().includes(search))
    )
  }, [workdirOptions, workdirSearch])

  // Check if selected project is a git project
  const isGitProject = selectedProject != null && selectedProject.isGit

  useEffect(() => {
    if (!open) return
    if (!selectedProjectId) {
      setSelectedWorkdirId(null)
      return
    }

    if (!isGitProject) {
      setSelectedWorkdirId(null)
      return
    }

    if (workdirOptions.length === 0) {
      setSelectedWorkdirId(null)
      return
    }

    // If workdir already selected and valid, keep it
    if (selectedWorkdirId === -1) return // "Create new" is always valid
    if (selectedWorkdirId != null && workdirOptions.some((w) => w.id === selectedWorkdirId)) return

    if (activeWorkdirId != null) {
      const activeOpt = workdirOptions.find((w) => w.id === activeWorkdirId) ?? null
      if (activeOpt) {
        setSelectedWorkdirId(activeOpt.id)
        return
      }
    }

    const mainOpt =
      selectedProject == null
        ? null
        : (workdirOptions.find(
            (w) =>
              w.workdir_name === "main" && normalizePathLike(w.workdir_path) === normalizePathLike(selectedProject.path),
          ) ?? null)
    if (mainOpt) {
      setSelectedWorkdirId(mainOpt.id)
      return
    }

    const first = workdirOptions[0] ?? null
    setSelectedWorkdirId(first ? first.id : null)
  }, [activeWorkdirId, isGitProject, open, selectedProjectId, selectedProject?.path, selectedWorkdirId, workdirOptions])

  // Check if we can execute the task
  const canExecute = useMemo(() => {
    if (!input.trim()) return false
    if (selectedProject == null) return false
    // Non-git project does not require explicit workdir selection.
    if (!isGitProject) return true
    // Git project - need workdir selected (either existing or -1 for create new)
    return selectedWorkdirId != null
  }, [input, isGitProject, selectedProject, selectedWorkdirId])

  // Focus input when modal opens
  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 100)
    }
  }, [open])

  const waitForWorkdir = async (args: {
    projectId: string
    predicate: (w: { id: number; workdir_name: string; workdir_path: string; status: string }) => boolean
    timeoutMs?: number
  }): Promise<number> => {
    const deadline = Date.now() + (args.timeoutMs ?? 15_000)
    while (Date.now() < deadline) {
      const snap = appRef.current
      const project = snap?.projects.find((p) => p.id === args.projectId) ?? null
      const workdir = project?.workdirs.find((w) => args.predicate(w)) ?? null
      if (workdir) return workdir.id
      await new Promise((r) => window.setTimeout(r, 200))
    }
    throw new Error("Timed out waiting for workdir to be ready")
  }

  const ensureMainWorkdirId = async (projectId: string, projectPath: string): Promise<number> => {
    ensureMainWorkdir(projectId)
    return waitForWorkdir({
      projectId,
      predicate: (w) =>
        w.status === "active" &&
        w.workdir_name === "main" &&
        normalizePathLike(w.workdir_path) === normalizePathLike(projectPath),
    })
  }

  const createNewWorkdirId = async (projectId: string, existingIds: Set<number>): Promise<number> => {
    createWorkdir(projectId)
    const deadline = Date.now() + 20_000
    while (Date.now() < deadline) {
      const snap = appRef.current
      const project = snap?.projects.find((p) => p.id === projectId) ?? null
      const next = project?.workdirs.find((w) => w.status === "active" && !existingIds.has(w.id)) ?? null
      if (next) return next.id
      await new Promise((r) => window.setTimeout(r, 200))
    }
    throw new Error("Timed out waiting for new workdir to be created")
  }

  const handleSubmit = async (mode: TaskExecuteMode) => {
    if (!input.trim()) return
    if (!selectedProject) return
    setExecutingMode(mode)
    try {
      const trimmed = input.trim()

      const workdirId = await (async (): Promise<number> => {
        if (!isGitProject) {
          const main = selectedProject.workdirs.find(
            (w) =>
              w.workdir_name === "main" && normalizePathLike(w.workdir_path) === normalizePathLike(selectedProject.path),
          )
          if (main) return main.id
          const first = selectedProject.workdirs[0]
          if (first) return first.id
          return ensureMainWorkdirId(selectedProject.id, selectedProject.path)
        }

        if (selectedWorkdirId === -1) {
          const existing = new Set(selectedProject.workdirs.map((w) => w.id))
          return createNewWorkdirId(selectedProject.id, existing)
        }
        if (selectedWorkdirId != null) return selectedWorkdirId
        return ensureMainWorkdirId(selectedProject.id, selectedProject.path)
      })()

      const result = await executeTask(trimmed, mode, workdirId)
      if (mode === "create") {
        localStorage.setItem(draftKey(result.workdir_id, result.task_id), JSON.stringify({ text: result.prompt }))
      }

      await openWorkdir(result.workdir_id)
      await activateTask(result.task_id)
      focusChatInput()

      toast(mode === "create" ? "Draft created" : "Task started")

      setInput("")
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    } finally {
      setExecutingMode(null)
    }
  }

  const handleClose = () => {
    setInput("")
    setSelectedProjectId("")
    setSelectedWorkdirId(null)
    onOpenChange(false)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      handleClose()
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      if (canExecute && !executingMode) {
        void handleSubmit("start")
      }
    }
  }

  const selectedWorkdirLabel = useMemo(() => {
    if (selectedWorkdirId === -1) return "Create new..."
    if (selectedWorkdirId == null) return "Workdir"
    const w = workdirOptions.find((w) => w.id === selectedWorkdirId)
    if (!w) return "Workdir"
    return w.workdir_name || w.branch_name || "Workdir"
  }, [selectedWorkdirId, workdirOptions])

  if (!open) return null

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[10vh]"
      style={{ backgroundColor: "rgba(0, 0, 0, 0.5)" }}
      onClick={handleClose}
      onKeyDown={handleKeyDown}
    >
      <div
        data-testid="new-task-modal"
        className="w-full max-w-[740px] flex flex-col"
        style={{
          backgroundColor: "#ffffff",
          boxShadow: "0 25px 50px -12px rgba(0, 0, 0, 0.25)",
          borderRadius: "12px",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 pt-3 pb-1">
          {/* Left: Project Selector + Template */}
          <div className="flex items-center gap-1.5">
            {/* Project Selector */}
	            <DropdownMenu>
	              <DropdownMenuTrigger asChild>
	                <button
	                  data-testid="new-task-project-selector"
	                  data-selected-project-id={selectedProject?.id ?? ""}
	                  className="h-6 pl-6 pr-2 text-[12px] flex items-center gap-1 hover:bg-[#f7f7f7] transition-colors relative"
	                  style={{
	                    backgroundColor: "#fff",
	                    color: "#2d2d2d",
                    border: "1px solid #e0e0e0",
                    borderRadius: "5px",
                    fontWeight: 500,
                  }}
                  disabled={executingMode != null}
                >
                  {/* Colored dot indicator */}
	                  <span
	                    className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 rounded-full"
	                    style={{ backgroundColor: "#26b5ce" }}
	                  />
	                  <span>
	                    {selectedProject?.name || selectedProject?.slug || "Project"}
	                  </span>
	                </button>
	              </DropdownMenuTrigger>
              <DropdownMenuContent
                align="start"
                sideOffset={4}
                className="w-[240px] rounded-lg bg-white p-0 overflow-hidden"
                style={{
                  border: "1px solid #e5e5e5",
                  boxShadow: "0 4px 12px rgba(0, 0, 0, 0.15)",
                }}
                onCloseAutoFocus={(e) => {
                  e.preventDefault()
                  setProjectSearch("")
                }}
              >
                {/* Search input */}
                <div className="px-2 py-2" style={{ borderBottom: "1px solid #eee" }}>
                  <input
                    type="text"
                    placeholder="Set project..."
                    value={projectSearch}
                    onChange={(e) => setProjectSearch(e.target.value)}
                    className="w-full h-7 px-2 text-[13px] focus:outline-none"
                    style={{ backgroundColor: "transparent", color: "#2d2d2d" }}
                    autoFocus
                  />
                </div>

	                <div className="p-1 max-h-[300px] overflow-y-auto">
	                  {/* Project options */}
		                  {filteredProjects.map((p, idx) => (
		                    <DropdownMenuItem
		                      key={p.id}
                      onClick={() => {
                        setSelectedProjectId(p.id)
                        setProjectSearch("")
                      }}
                      className="flex items-center justify-between h-8 px-2 rounded cursor-pointer hover:bg-[#f5f5f5] focus:bg-[#f5f5f5] outline-none"
                    >
	                      <div className="flex items-center gap-2">
	                        <span
	                          className="w-4 h-4 rounded flex items-center justify-center text-[10px] text-white font-medium"
	                          style={{ backgroundColor: ["#26b5ce", "#f2994a", "#eb5757", "#5e6ad2", "#27ae60"][idx % 5] }}
	                        >
	                          {(p.name || p.slug || "P").charAt(0).toUpperCase()}
	                        </span>
	                        <span className="text-[13px]" style={{ color: "#2d2d2d" }}>{p.name || p.slug}</span>
		                      </div>
		                      {selectedProjectId === p.id && (
		                        <Check className="w-4 h-4" style={{ color: "#2d2d2d" }} />
		                      )}
		                    </DropdownMenuItem>
		                  ))}

                  {filteredProjects.length === 0 && (
                    <div className="px-2 py-3 text-[13px] text-center" style={{ color: "#888" }}>
                      No projects found
                    </div>
                  )}
                </div>
              </DropdownMenuContent>
            </DropdownMenu>

	            {/* Separator and Worktree selector - only show for git projects */}
	            {isGitProject && (
	              <>
                {/* Separator */}
                <span className="text-[12px]" style={{ color: "#ccc" }}>›</span>

                {/* Worktree Selector */}
	                <DropdownMenu>
	                  <DropdownMenuTrigger asChild>
	                    <button
	                      data-testid="new-task-workdir-selector"
	                      data-selected-workdir-id={selectedWorkdirId == null ? "" : String(selectedWorkdirId)}
	                      className="h-6 pl-6 pr-2 text-[12px] flex items-center gap-1 hover:bg-[#f7f7f7] transition-colors relative"
	                      style={{
	                        backgroundColor: "#fff",
	                        color: "#2d2d2d",
                        border: "1px solid #e0e0e0",
                        borderRadius: "5px",
                        fontWeight: 500,
                      }}
                      disabled={executingMode != null}
                    >
                      <GitBranch
                        className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3"
                        style={{ color: "#666" }}
                      />
                      <span>
                        {selectedWorkdirId === -1
                          ? "Create new..."
                          : selectedWorkdirLabel}
                      </span>
                    </button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent
                    align="start"
                    sideOffset={4}
                    className="w-[240px] rounded-lg bg-white p-0 overflow-hidden"
                    style={{
                      border: "1px solid #e5e5e5",
                      boxShadow: "0 4px 12px rgba(0, 0, 0, 0.15)",
                    }}
                    onCloseAutoFocus={(e) => {
                      e.preventDefault()
                      setWorkdirSearch("")
                    }}
                  >
                    {/* Search input */}
                    <div className="px-2 py-2" style={{ borderBottom: "1px solid #eee" }}>
                      <input
                        type="text"
                        placeholder="Set workdir..."
                        value={workdirSearch}
                        onChange={(e) => setWorkdirSearch(e.target.value)}
                        className="w-full h-7 px-2 text-[13px] focus:outline-none"
                        style={{ backgroundColor: "transparent", color: "#2d2d2d" }}
                        autoFocus
                      />
                    </div>

                    <div className="p-1 max-h-[300px] overflow-y-auto">
                      {/* Create new option - only show when not searching */}
                      {!workdirSearch.trim() && (
                        <>
                          <DropdownMenuItem
                            onClick={() => {
                              setSelectedWorkdirId(-1)
                              setWorkdirSearch("")
                            }}
                            className="flex items-center justify-between h-8 px-2 rounded cursor-pointer hover:bg-[#f5f5f5] focus:bg-[#f5f5f5] outline-none"
                          >
                            <span className="text-[13px]" style={{ color: "#2d2d2d" }}>Create new...</span>
                            {selectedWorkdirId === -1 && (
                              <Check className="w-4 h-4" style={{ color: "#2d2d2d" }} />
                            )}
                          </DropdownMenuItem>

                          {/* Divider */}
                          <div className="my-1 mx-2" style={{ borderTop: "1px solid #eee" }} />
                        </>
                      )}

                      {/* Workdir options */}
                      {filteredWorkdirs.map((w) => (
                        <DropdownMenuItem
                          key={w.id}
                          onClick={() => {
                            setSelectedWorkdirId(w.id)
                            setWorkdirSearch("")
                          }}
                          className="flex items-center justify-between h-8 px-2 rounded cursor-pointer hover:bg-[#f5f5f5] focus:bg-[#f5f5f5] outline-none"
                        >
                          <span className="text-[13px]" style={{ color: "#2d2d2d" }}>
                            {w.workdir_name || w.branch_name || w.workdir_path}
                          </span>
                          {selectedWorkdirId === w.id && (
                            <Check className="w-4 h-4" style={{ color: "#2d2d2d" }} />
                          )}
                        </DropdownMenuItem>
                      ))}

                      {filteredWorkdirs.length === 0 && workdirSearch.trim() && (
                        <div className="px-2 py-3 text-[13px] text-center" style={{ color: "#888" }}>
                          No workdirs found
                        </div>
                      )}
                    </div>
                  </DropdownMenuContent>
                </DropdownMenu>
              </>
            )}
          </div>

          {/* Right: Expand + Close */}
          <div className="flex items-center">
            <button
              className="w-7 h-7 flex items-center justify-center hover:bg-[#f5f5f5] transition-colors"
              style={{ color: "#666", borderRadius: "5px" }}
              title="Expand"
            >
              <Maximize2 className="w-4 h-4" />
            </button>
            <button
              onClick={handleClose}
              className="w-7 h-7 flex items-center justify-center hover:bg-[#f5f5f5] transition-colors"
              style={{ color: "#666", borderRadius: "5px" }}
              title="Close"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Content Area */}
        <div className="px-4 pt-2 pb-4">
          {/* Description */}
          <textarea
            ref={inputRef}
            data-testid="new-task-input"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Add task description..."
            className="w-full text-[15px] resize-none focus:outline-none"
            style={{
              minHeight: "80px",
              color: "#191919",
              backgroundColor: "transparent",
              fontWeight: 450,
            }}
            disabled={executingMode != null}
          />
        </div>

        {/* Footer */}
        <div
          className="flex items-center justify-between px-4 py-3"
          style={{ borderTop: "1px solid #eee" }}
        >
          {/* Left: Attachment */}
          <button
            className="w-7 h-7 flex items-center justify-center hover:bg-[#f5f5f5] transition-colors"
            style={{ color: "#666", borderRadius: "5px" }}
            title="Attach file"
          >
            <Paperclip className="w-4 h-4" />
          </button>

          {/* Right: Create button */}
          <div className="flex items-center">
            <button
              data-testid="new-task-submit-button"
              onClick={() => void handleSubmit("start")}
              disabled={!canExecute || executingMode != null}
              className="h-7 px-4 text-[12px] transition-colors disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
              style={{
                backgroundColor: "#5e6ad2",
                color: "#ffffff",
                borderRadius: "5px",
                fontWeight: 500,
              }}
            >
              {executingMode === "start" ? "Creating..." : "Create task"}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
