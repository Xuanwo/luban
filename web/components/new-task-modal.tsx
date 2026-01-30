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

interface NewTaskModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function NewTaskModal({ open, onOpenChange }: NewTaskModalProps) {
  const { app, executeTask, openWorkdir, activateTask, activeWorkdirId } = useLuban()

  const [input, setInput] = useState("")
  const [executingMode, setExecutingMode] = useState<TaskExecuteMode | null>(null)
  const [selectedProjectId, setSelectedProjectId] = useState<string>("")
  const [selectedWorkdirId, setSelectedWorkdirId] = useState<number | null>(null)
  const [projectSearch, setProjectSearch] = useState("")
  const [workdirSearch, setWorkdirSearch] = useState("")
  const inputRef = useRef<HTMLTextAreaElement>(null)

  const normalizePathLike = (raw: string) => raw.trim().replace(/\/+$/, "")

  const projectOptions = useMemo(() => {
    return (app?.projects ?? []).map((p) => ({
      id: p.id,
      name: p.name,
      path: p.path,
      slug: p.slug,
      workdirs: p.workdirs.filter((w) => w.status === "active"),
    }))
  }, [app])

  useEffect(() => {
    if (!open) return
    if (selectedProjectId) return
    // Default to "auto" when modal opens
    setSelectedProjectId("auto")
  }, [open, selectedProjectId])

  const selectedProject = useMemo(() => {
    if (selectedProjectId === "auto") {
      // Fall back to first project if only one exists
      if (projectOptions.length === 1) {
        return projectOptions[0]
      }
      return null
    }
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

  // Check if selected project is a git project (has workdirs)
  const isGitProject = selectedProject != null && workdirOptions.length > 0

  useEffect(() => {
    if (!open) return
    if (!selectedProjectId || selectedProjectId === "auto") {
      setSelectedWorkdirId(null)
      return
    }

    // If workdir already selected and valid, keep it
    if (selectedWorkdirId === -1) return // "Create new" is always valid
    if (selectedWorkdirId != null && workdirOptions.some((w) => w.id === selectedWorkdirId)) return

    // Default to "Create new..." (-1) for git projects
    if (workdirOptions.length > 0) {
      setSelectedWorkdirId(-1)
    } else {
      setSelectedWorkdirId(null)
    }
  }, [activeWorkdirId, open, selectedProjectId, selectedWorkdirId, workdirOptions])

  // Check if we can execute the task
  const canExecute = useMemo(() => {
    if (!input.trim()) return false
    if (selectedProjectId === "auto") return true
    if (selectedProject == null) return false
    // Non-git project (no workdirs)
    if (workdirOptions.length === 0) return true
    // Git project - need workdir selected (either existing or -1 for create new)
    return selectedWorkdirId != null
  }, [input, selectedProjectId, selectedProject, workdirOptions.length, selectedWorkdirId])

  // Focus input when modal opens
  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 100)
    }
  }, [open])

  const handleSubmit = async (mode: TaskExecuteMode) => {
    if (!input.trim()) return
    if (!selectedProject) return
    if (selectedWorkdirId == null) return
    setExecutingMode(mode)
    try {
      const result = await executeTask(
        {
          raw_input: input.trim(),
          intent_kind: "other",
          title: input.trim().slice(0, 100),
          project: { type: "local_path", path: selectedProject.path },
        },
        mode,
        selectedWorkdirId
      )
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
                    {selectedProjectId === "auto"
                      ? "Auto"
                      : selectedProject?.name || selectedProject?.slug || "Project"}
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
                  {/* Auto option - only show when not searching */}
                  {!projectSearch.trim() && (
                    <>
                      <DropdownMenuItem
                        onClick={() => {
                          setSelectedProjectId("auto")
                          setProjectSearch("")
                        }}
                        className="flex items-center justify-between h-8 px-2 rounded cursor-pointer hover:bg-[#f5f5f5] focus:bg-[#f5f5f5] outline-none"
                      >
                        <div className="flex items-center gap-2">
                          <span
                            className="w-4 h-4 rounded flex items-center justify-center"
                            style={{ backgroundColor: "#26b5ce" }}
                          >
                            <span className="text-[10px] text-white font-medium">A</span>
                          </span>
                          <span className="text-[13px]" style={{ color: "#2d2d2d" }}>Auto</span>
                        </div>
                        {selectedProjectId === "auto" && (
                          <Check className="w-4 h-4" style={{ color: "#2d2d2d" }} />
                        )}
                      </DropdownMenuItem>

                      {/* Divider */}
                      <div className="my-1 mx-2" style={{ borderTop: "1px solid #eee" }} />
                    </>
                  )}

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
            {isGitProject && selectedProjectId !== "auto" && (
              <>
                {/* Separator */}
                <span className="text-[12px]" style={{ color: "#ccc" }}>›</span>

                {/* Worktree Selector */}
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <button
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
                        placeholder="Set worktree..."
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
                          No worktrees found
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
