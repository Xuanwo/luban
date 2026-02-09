"use client"

import type React from "react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { toast } from "sonner"
import {
  X,
  Maximize2,
  Minimize2,
  GitBranch,
  Paperclip,
  Check,
} from "lucide-react"

import { useLuban } from "@/lib/luban-context"
import type { AttachmentRef, TaskExecuteMode } from "@/lib/luban-api"
import { draftKey } from "@/lib/ui-prefs"
import { focusChatInput } from "@/lib/focus-chat-input"
import { uploadAttachment } from "@/lib/luban-http"
import { isMockMode } from "@/lib/luban-mode"
import type { NewTaskDraft } from "@/lib/new-task-drafts"
import {
  clearNewTaskStash,
  loadNewTaskStash,
  saveNewTaskStash,
  upsertNewTaskDraft,
} from "@/lib/new-task-drafts"
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@/components/ui/dropdown-menu"
import type { AppSnapshot } from "@/lib/luban-api"

function escapeXmlText(raw: string): string {
  return raw
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;")
}

function buildFallbackAvatarUrl(displayName: string, size: number): string {
  const letter = displayName.trim().slice(0, 1).toUpperCase() || "?"
  const safeLetter = escapeXmlText(letter)
  const svg = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">`,
    `<rect width="${size}" height="${size}" rx="4" fill="#e8e8e8" />`,
    `<text x="${size / 2}" y="${Math.floor(size * 0.67)}" text-anchor="middle" font-size="${Math.floor(size * 0.56)}" font-family="system-ui, -apple-system, sans-serif" fill="#6b6b6b">${safeLetter}</text>`,
    `</svg>`,
  ].join("")
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

interface NewTaskModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onTaskCreated?: () => void
  activeProjectId?: string | null
  initialDraft?: NewTaskDraft | null
}

type PendingAttachment = {
  id: string
  file: File
  kind: "image" | "file"
  name: string
  url?: string
}

function normalizePathLike(raw: string): string {
  return raw.trim().replace(/\/+$/, "")
}

export function NewTaskModal({ open, onOpenChange, onTaskCreated, activeProjectId, initialDraft }: NewTaskModalProps) {
  const {
    app,
    executeTask,
    openWorkdir,
    activateTask,
    activeWorkdirId,
    createWorkdir,
    ensureMainWorkdir,
  } = useLuban()

  const [input, setInput] = useState("")
  const [executingMode, setExecutingMode] = useState<TaskExecuteMode | null>(null)
  const [attachments, setAttachments] = useState<PendingAttachment[]>([])
  const [selectedProjectId, setSelectedProjectId] = useState<string>("")
  const [selectedWorkdirId, setSelectedWorkdirId] = useState<number | null>(null)
  const [editingDraftId, setEditingDraftId] = useState<string | null>(null)
  const [isExpanded, setIsExpanded] = useState(false)
  const [projectSearch, setProjectSearch] = useState("")
  const [workdirSearch, setWorkdirSearch] = useState("")
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const appRef = useRef<AppSnapshot | null>(null)
  const submitInFlightRef = useRef(false)
  const prevOpenRef = useRef(false)
  const hasUserEditedRef = useRef(false)

  useEffect(() => {
    appRef.current = app
  }, [app])

  const projectOptions = useMemo(() => {
    return (app?.projects ?? []).map((p) => {
      const displayName = p.name || p.slug || p.id
      const fallbackAvatarUrl = buildFallbackAvatarUrl(displayName, 16)
      return {
        id: p.id,
        name: p.name,
        displayName,
        path: p.path,
        slug: p.slug,
        isGit: p.is_git,
        fallbackAvatarUrl,
        avatarUrl: p.is_git
          ? isMockMode()
            ? fallbackAvatarUrl
            : `/api/projects/avatar?project_id=${encodeURIComponent(p.id)}`
          : undefined,
        workdirs: p.workdirs.filter((w) => w.status === "active"),
      }
    })
  }, [app])

  const defaultProjectId = useMemo(() => {
    if (activeProjectId) {
      const opt = projectOptions.find((p) => p.id === activeProjectId) ?? null
      if (opt) return opt.id
    }
    if (activeWorkdirId != null) {
      const opt = projectOptions.find((p) => p.workdirs.some((w) => w.id === activeWorkdirId)) ?? null
      if (opt) return opt.id
    }
    if (projectOptions.length === 1) return projectOptions[0]?.id ?? ""
    return projectOptions[0]?.id ?? ""
  }, [activeProjectId, activeWorkdirId, projectOptions])

  const effectiveProjectId = selectedProjectId || defaultProjectId || ""

  const pickDefaultWorkdirIdForProjectId = useCallback(
    (projectId: string): number | null => {
      const project = projectOptions.find((p) => p.id === projectId) ?? null
      if (!project) return null
      if (!project.isGit) return null
      if (project.workdirs.length === 0) return null

      if (activeWorkdirId != null && project.workdirs.some((w) => w.id === activeWorkdirId)) {
        return activeWorkdirId
      }

      const main =
        project.workdirs.find(
          (w) =>
            w.workdir_name === "main" &&
            normalizePathLike(w.workdir_path) === normalizePathLike(project.path),
        ) ?? null
      if (main) return main.id

      return project.workdirs[0]?.id ?? null
    },
    [activeWorkdirId, projectOptions],
  )

  const stashNow = useCallback(() => {
    const trimmed = input.trim()
    if (!trimmed) {
      void clearNewTaskStash().catch((err) => console.warn("clearNewTaskStash failed", err))
      return
    }
    void saveNewTaskStash({
      text: input,
      projectId: effectiveProjectId || null,
      // Do not persist workdir selection; always use the contextual default on next open.
      workdirId: null,
      editingDraftId,
      updatedAtUnixMs: Date.now(),
    }).catch((err) => console.warn("saveNewTaskStash failed", err))
  }, [editingDraftId, effectiveProjectId, input])

  useEffect(() => {
    const prev = prevOpenRef.current
    prevOpenRef.current = open
    if (prev || !open) return

    setIsExpanded(false)
    setProjectSearch("")
    setWorkdirSearch("")
    hasUserEditedRef.current = false
    setInput("")
    setEditingDraftId(null)
    setSelectedProjectId(defaultProjectId || "")
    setSelectedWorkdirId(defaultProjectId ? pickDefaultWorkdirIdForProjectId(defaultProjectId) : null)

    if (initialDraft) {
      void clearNewTaskStash().catch((err) => console.warn("clearNewTaskStash failed", err))
      setInput(initialDraft.text)
      setSelectedProjectId(initialDraft.projectId ?? defaultProjectId ?? "")
      setSelectedWorkdirId(initialDraft.workdirId)
      setEditingDraftId(initialDraft.id)
      return
    }

    let cancelled = false
    void (async () => {
      try {
        const stash = await loadNewTaskStash()
        if (cancelled) return
        if (hasUserEditedRef.current) return
        if (stash) {
          setInput(stash.text)
          setSelectedProjectId(stash.projectId ?? defaultProjectId ?? "")
          setSelectedWorkdirId(
            (stash.projectId ? pickDefaultWorkdirIdForProjectId(stash.projectId) : null) ??
              (defaultProjectId ? pickDefaultWorkdirIdForProjectId(defaultProjectId) : null),
          )
          setEditingDraftId(stash.editingDraftId)
          return
        }
      } catch (err) {
        console.warn("loadNewTaskStash failed", err)
      }
      if (cancelled) return
      if (hasUserEditedRef.current) return
      setInput("")
      setEditingDraftId(null)
      setSelectedWorkdirId(null)
      setSelectedProjectId(defaultProjectId || "")
    })()

    return () => {
      cancelled = true
    }
  }, [defaultProjectId, initialDraft, open, pickDefaultWorkdirIdForProjectId])

  useEffect(() => {
    if (!open) return
    if (selectedProjectId) return
    if (!defaultProjectId) return
    setSelectedProjectId(defaultProjectId)
  }, [defaultProjectId, open, selectedProjectId])

  const selectedProject = useMemo(() => {
    if (!effectiveProjectId) return null
    return projectOptions.find((p) => p.id === effectiveProjectId) ?? null
  }, [effectiveProjectId, projectOptions])

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
    if (!effectiveProjectId) {
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
  }, [activeWorkdirId, effectiveProjectId, isGitProject, open, selectedProject, selectedWorkdirId, workdirOptions])

  // Check if we can execute the task
  const canExecute = useMemo(() => {
    if (!input.trim()) return false
    if (selectedProject == null) return false
    // Non-git project does not require explicit workdir selection.
    if (!isGitProject) return true
    return true
  }, [input, isGitProject, selectedProject])

  // Focus input when modal opens
  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 100)
    }
  }, [open])

  const syncTextareaHeight = useCallback(() => {
    const el = inputRef.current
    if (!el) return

    const minHeightPx = 80
    const preferredMaxHeightPx = isExpanded ? 520 : 240
    const viewportCapPx =
      typeof window !== "undefined"
        ? Math.floor(window.innerHeight * (isExpanded ? 0.65 : 0.4))
        : preferredMaxHeightPx
    const maxHeightPx = Math.max(minHeightPx, Math.min(preferredMaxHeightPx, viewportCapPx))

    el.style.height = "auto"
    const scrollHeightPx = el.scrollHeight
    const nextHeightPx = Math.max(minHeightPx, Math.min(scrollHeightPx, maxHeightPx))
    el.style.height = `${nextHeightPx}px`
    el.style.overflowY = scrollHeightPx > maxHeightPx ? "auto" : "hidden"
  }, [isExpanded])

  useEffect(() => {
    if (!open) return
    syncTextareaHeight()
  }, [open, input, isExpanded, syncTextareaHeight])

  useEffect(() => {
    if (!open) return
    const onResize = () => syncTextareaHeight()
    window.addEventListener("resize", onResize)
    return () => window.removeEventListener("resize", onResize)
  }, [open, syncTextareaHeight])

  const revokeAttachmentUrls = (items: PendingAttachment[]) => {
    for (const item of items) {
      if (item.url) URL.revokeObjectURL(item.url)
    }
  }

  useEffect(() => {
    if (open) return
    if (attachments.length === 0) return
    revokeAttachmentUrls(attachments)
    setAttachments([])
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  const addPendingFiles = (files: Iterable<File>) => {
    const next: PendingAttachment[] = []
    for (const file of files) {
      const isImage = file.type.startsWith("image/")
      const id =
        typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
          ? crypto.randomUUID()
          : `${Date.now()}-${Math.random().toString(36).slice(2)}`
      next.push({
        id,
        file,
        kind: isImage ? "image" : "file",
        name: file.name || (isImage ? "screenshot.png" : "file"),
        url: isImage ? URL.createObjectURL(file) : undefined,
      })
    }
    if (next.length === 0) return
    setAttachments((prev) => [...prev, ...next])
  }

  const handleFileSelect = (files: FileList | null) => {
    if (!files || files.length === 0) return
    addPendingFiles(Array.from(files))
  }

  const handlePaste = (e: React.ClipboardEvent) => {
    const items = e.clipboardData?.items
    if (!items) return

    const files: File[] = []
    for (const item of Array.from(items)) {
      if (item.kind !== "file") continue
      const file = item.getAsFile()
      if (file) files.push(file)
    }

    if (files.length === 0) return
    e.preventDefault()
    addPendingFiles(files)
  }

  const removeAttachment = (id: string) => {
    setAttachments((prev) => {
      const removed = prev.find((a) => a.id === id) ?? null
      if (removed) revokeAttachmentUrls([removed])
      return prev.filter((a) => a.id !== id)
    })
  }

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
    if (submitInFlightRef.current) return
    if (!input.trim()) return
    if (!selectedProject) return
    submitInFlightRef.current = true
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
        if (activeWorkdirId != null && selectedProject.workdirs.some((w) => w.id === activeWorkdirId)) {
          return activeWorkdirId
        }
        const main = selectedProject.workdirs.find(
          (w) =>
            w.workdir_name === "main" && normalizePathLike(w.workdir_path) === normalizePathLike(selectedProject.path),
        )
        if (main) return main.id
        const first = selectedProject.workdirs[0]
        if (first) return first.id
        return ensureMainWorkdirId(selectedProject.id, selectedProject.path)
      })()

      const settled = await Promise.allSettled(
        attachments.map((att) =>
          uploadAttachment({
            workspaceId: workdirId,
            file: att.file,
            kind: att.kind === "image" ? "image" : "file",
          }),
        ),
      )

      const uploaded: AttachmentRef[] = []
      let failed = 0
      for (const entry of settled) {
        if (entry.status === "fulfilled") uploaded.push(entry.value)
        else failed += 1
      }
      if (failed > 0) {
        toast.error(`${failed} attachment(s) failed to upload; proceeding without them`)
      }

      const result = await executeTask(trimmed, mode, workdirId, uploaded)

      await openWorkdir(result.workdir_id)
      await activateTask(result.task_id)

      if (mode === "create") {
        localStorage.setItem(
          draftKey(result.workdir_id, result.task_id),
          JSON.stringify({ text: result.prompt, attachments: uploaded }),
        )
      }
      focusChatInput()

      toast(mode === "create" ? "Draft created" : "Task started")

      setInput("")
      revokeAttachmentUrls(attachments)
      setAttachments([])
      setEditingDraftId(null)
      void clearNewTaskStash().catch((err) => console.warn("clearNewTaskStash failed", err))
      onTaskCreated?.()
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    } finally {
      submitInFlightRef.current = false
      setExecutingMode(null)
    }
  }

  const handleClose = useCallback(() => {
    setInput("")
    revokeAttachmentUrls(attachments)
    setAttachments([])
    setSelectedProjectId("")
    setSelectedWorkdirId(null)
    setEditingDraftId(null)
    onOpenChange(false)
  }, [attachments, onOpenChange])

  useEffect(() => {
    if (!open) return

    const handler = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return
      e.preventDefault()
      e.stopPropagation()
      stashNow()
      onOpenChange(false)
    }

    window.addEventListener("keydown", handler, { capture: true })
    return () => window.removeEventListener("keydown", handler, { capture: true } as AddEventListenerOptions)
  }, [onOpenChange, open, stashNow])

  useEffect(() => {
    if (!open) return

    const handler = () => stashNow()
    window.addEventListener("blur", handler)
    return () => window.removeEventListener("blur", handler)
  }, [open, stashNow])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault()
      e.stopPropagation()
      stashNow()
      onOpenChange(false)
      return
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      if (e.repeat) {
        e.preventDefault()
        e.stopPropagation()
        return
      }
      e.preventDefault()
      e.stopPropagation()
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

  const canSaveDraft = input.trim().length > 0

  const handleDismiss = () => {
    stashNow()
    onOpenChange(false)
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[10vh]"
      style={{ backgroundColor: "rgba(0, 0, 0, 0.5)" }}
      onClick={handleDismiss}
      onKeyDown={handleKeyDown}
    >
      <div
        data-testid="new-task-modal"
        data-expanded={isExpanded ? "true" : "false"}
        className={`w-full flex flex-col transition-[max-width,max-height] duration-150 ${
          isExpanded ? "max-w-[980px] max-h-[85vh]" : "max-w-[740px] max-h-[70vh]"
        }`}
        style={{
          backgroundColor: "#ffffff",
          boxShadow: "0 25px 50px -12px rgba(0, 0, 0, 0.25)",
          borderRadius: "12px",
          overflow: "hidden",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 pt-3 pb-1">
          {/* Left: Project Selector + Template */}
          <div className="flex items-center gap-1.5 min-w-0 flex-1 overflow-hidden">
            {/* Project Selector */}
	            <DropdownMenu>
	              <DropdownMenuTrigger asChild>
	                <button
	                  data-testid="new-task-project-selector"
	                  data-selected-project-id={selectedProject?.id ?? ""}
	                  className="h-6 pl-6 pr-2 text-[12px] flex items-center gap-1 hover:bg-[#f7f7f7] transition-colors relative min-w-0 max-w-[280px]"
	                  style={{
	                    backgroundColor: "#fff",
	                    color: "#2d2d2d",
                    border: "1px solid #e0e0e0",
                    borderRadius: "5px",
                    fontWeight: 500,
                  }}
                  disabled={executingMode != null}
                >
		                  {/* Project indicator */}
		                  {selectedProject != null && selectedProject.isGit ? (
		                    /* eslint-disable-next-line @next/next/no-img-element */
		                    <img
		                      src={selectedProject.avatarUrl ?? selectedProject.fallbackAvatarUrl}
		                      alt=""
	                      width={12}
	                      height={12}
	                      className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 rounded overflow-hidden"
	                      onError={(e) => {
	                        e.currentTarget.onerror = null
	                        e.currentTarget.src = selectedProject.fallbackAvatarUrl
	                      }}
	                    />
	                  ) : (
	                    <span
	                      className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 rounded-full"
	                      style={{ backgroundColor: "#26b5ce" }}
	                    />
	                  )}
		                  <span className="truncate">
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
	                      data-testid={`new-task-project-option-${p.id}`}
	                      onClick={() => {
	                        setSelectedProjectId(p.id)
	                        setProjectSearch("")
	                      }}
	                      className="flex items-center justify-between h-8 px-2 rounded cursor-pointer hover:bg-[#f5f5f5] focus:bg-[#f5f5f5] outline-none"
	                    >
			                      <div className="flex items-center gap-2">
			                        {p.isGit ? (
		                          /* eslint-disable-next-line @next/next/no-img-element */
		                          <img
		                            src={p.avatarUrl ?? p.fallbackAvatarUrl}
		                            alt=""
	                            width={16}
	                            height={16}
	                            className="w-4 h-4 rounded overflow-hidden flex-shrink-0"
	                            onError={(e) => {
	                              e.currentTarget.onerror = null
	                              e.currentTarget.src = p.fallbackAvatarUrl
	                            }}
	                          />
	                        ) : (
		                          <span
		                            className="w-4 h-4 rounded flex items-center justify-center text-[10px] text-white font-medium"
		                            style={{ backgroundColor: ["#26b5ce", "#f2994a", "#eb5757", "#5e6ad2", "#27ae60"][idx % 5] }}
		                          >
		                            {(p.name || p.slug || "P").charAt(0).toUpperCase()}
		                          </span>
	                        )}
		                        <span className="text-[13px]" style={{ color: "#2d2d2d" }}>{p.name || p.slug}</span>
			                      </div>
                      {effectiveProjectId === p.id && (
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
          <div className="flex items-center gap-1 flex-shrink-0">
            {canSaveDraft && (
            <button
              type="button"
              data-testid="new-task-save-draft-button"
              className="h-7 px-2.5 text-[12px] transition-colors hover:bg-[#f5f5f5] disabled:opacity-40 disabled:cursor-not-allowed"
              style={{ color: "#666", borderRadius: "5px", fontWeight: 500 }}
              onClick={() => {
                void (async () => {
                  const trimmed = input.trim()
	                  if (!trimmed) return
	                  try {
	                    const draftWorkdirId = selectedWorkdirId === -1 ? null : selectedWorkdirId
	                    await upsertNewTaskDraft({
	                      id: editingDraftId,
	                      text: input,
	                      projectId: effectiveProjectId || null,
	                      workdirId: draftWorkdirId,
	                    })
	                    await clearNewTaskStash()
	                    setInput("")
	                    revokeAttachmentUrls(attachments)
	                    setAttachments([])
	                    setSelectedProjectId("")
	                    setSelectedWorkdirId(null)
	                    setEditingDraftId(null)
	                    onOpenChange(false)
	                    toast("Saved to Drafts")
	                    } catch (err) {
	                      toast.error(err instanceof Error ? err.message : String(err))
	                    }
	                  })()
	                }}
                disabled={executingMode != null}
                title="Save as draft"
              >
                Save as draft
              </button>
            )}
            <button
              data-testid="new-task-expand-button"
              className="w-7 h-7 flex items-center justify-center hover:bg-[#f5f5f5] transition-colors"
              style={{ color: "#666", borderRadius: "5px" }}
              title={isExpanded ? "Collapse" : "Expand"}
              aria-pressed={isExpanded}
              onClick={() => setIsExpanded((v) => !v)}
            >
              {isExpanded ? <Minimize2 className="w-4 h-4" /> : <Maximize2 className="w-4 h-4" />}
            </button>
            <button
              data-testid="new-task-close-button"
              onClick={() => {
                void clearNewTaskStash().catch((err) => console.warn("clearNewTaskStash failed", err))
                handleClose()
              }}
              className="w-7 h-7 flex items-center justify-center hover:bg-[#f5f5f5] transition-colors"
              style={{ color: "#666", borderRadius: "5px" }}
              title="Close"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Content Area */}
        <div className="px-4 pt-2 pb-4 flex-1 overflow-y-auto">
          {/* Description */}
          <textarea
            ref={inputRef}
            data-testid="new-task-input"
            value={input}
            onChange={(e) => {
              hasUserEditedRef.current = true
              setInput(e.target.value)
            }}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
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

          {attachments.length > 0 && (
            <div className="mt-3 flex flex-wrap gap-2" data-testid="new-task-attachments">
              {attachments.map((att) => (
                <div
                  key={att.id}
                  data-testid="new-task-attachment-tile"
                  className="group relative w-16 h-16 rounded-lg overflow-hidden border border-[#e5e5e5] bg-[#fafafa] flex items-center justify-center"
                >
                  {att.kind === "image" && att.url ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img src={att.url} alt={att.name} className="w-full h-full object-cover" />
                  ) : (
                    <span className="text-[10px] text-[#666] px-1 text-center break-all">{att.name}</span>
                  )}
                  <button
                    type="button"
                    aria-label="Remove attachment"
                    data-testid="new-task-attachment-remove"
                    onClick={() => removeAttachment(att.id)}
                    disabled={executingMode != null}
                    className="absolute top-1 right-1 w-5 h-5 flex items-center justify-center rounded bg-white/90 border border-[#e5e5e5] opacity-0 group-hover:opacity-100 transition-opacity disabled:opacity-40"
                  >
                    <X className="w-3 h-3" />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          className="flex items-center justify-between px-4 py-3"
          style={{ borderTop: "1px solid #eee" }}
        >
          {/* Left: Attachment */}
          <input
            ref={fileInputRef}
            data-testid="new-task-attachment-input"
            type="file"
            multiple
            accept="image/*,.pdf,.txt,.md,.json,.csv,.xml,.yaml,.yml"
            className="hidden"
            onChange={(e) => handleFileSelect(e.target.files)}
          />
          <button
            data-testid="new-task-attach-button"
            className="w-7 h-7 flex items-center justify-center hover:bg-[#f5f5f5] transition-colors"
            style={{ color: "#666", borderRadius: "5px" }}
            title="Attach file"
            onClick={() => fileInputRef.current?.click()}
            onMouseDown={(e) => e.preventDefault()}
            disabled={executingMode != null}
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
