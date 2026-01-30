"use client"

import type React from "react"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { toast } from "sonner"
import {
  Terminal,
  Paperclip,
  ChevronDown,
  ChevronRight,
  File as FileIcon,
  FileImage,
  FileText,
  FileJson,
  FileCode,
  Trash2,
  Copy,
  FilePlus,
  GitCompareArrows,
} from "lucide-react"

import { cn } from "@/lib/utils"
import { PtyTerminal } from "./pty-terminal"
import { useLuban } from "@/lib/luban-context"
import type {
  AttachmentRef,
  ChangedFileSnapshot,
  ContextItemSnapshot,
  FileChangeGroup,
  FileChangeStatus,
} from "@/lib/luban-api"
import {
  deleteContextItem,
  fetchContext,
  fetchWorkspaceChanges,
  uploadAttachment,
} from "@/lib/luban-http"
import { emitAddChatAttachments } from "@/lib/chat-attachment-events"
import { emitContextChanged, onContextChanged } from "@/lib/context-events"
import { focusChatInput } from "@/lib/focus-chat-input"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

type RightPanelTab = "terminal" | "context" | "changes"

interface RightSidebarProps {
  widthPx: number
  onOpenDiffTab?: (file: ChangedFile) => void
}

export type ChangedFile = ChangedFileSnapshot

export function RightSidebar({ widthPx, onOpenDiffTab }: RightSidebarProps) {
  const { activeWorkspaceId } = useLuban()
  const [activeTab, setActiveTab] = useState<RightPanelTab>("terminal")
  const [isDragOver, setIsDragOver] = useState(false)
  const [droppedFiles, setDroppedFiles] = useState<File[] | null>(null)

  const canUseContext = activeWorkspaceId != null
  const canUseChanges = activeWorkspaceId != null

  const handleDragOver = (e: React.DragEvent) => {
    if (activeTab !== "context") return
    if (!canUseContext) return
    if (!e.dataTransfer.types.includes("Files")) return
    e.preventDefault()
    setIsDragOver(true)
  }

  const handleDragLeave = () => {
    setIsDragOver(false)
  }

  const handleDrop = (e: React.DragEvent) => {
    if (activeTab !== "context") return
    if (!canUseContext) return
    e.preventDefault()
    setIsDragOver(false)
    const files = Array.from(e.dataTransfer.files ?? [])
    if (files.length === 0) return
    setDroppedFiles(files)
  }

  return (
    <div
      className={cn(
        "flex flex-col transition-colors",
        isDragOver && "bg-primary/5",
      )}
      style={{
        width: `${widthPx}px`,
        borderLeft: '1px solid #ebebeb',
        backgroundColor: '#fcfcfc'
      }}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {/* Header - Properties style */}
      <div
        className="flex items-center"
        style={{ height: '39px', padding: '0 16px', borderBottom: '1px solid #ebebeb' }}
      >
        {/* Tab switches */}
        <div className="flex items-center gap-0.5">
          <button
            data-testid="right-sidebar-tab-terminal"
            onClick={() => setActiveTab("terminal")}
            className={cn(
              "w-6 h-6 flex items-center justify-center rounded-[5px] transition-colors",
              activeTab === "terminal"
                ? "bg-[#eeeeee] text-[#1b1b1b]"
                : "text-[#9b9b9b] hover:bg-[#eeeeee] hover:text-[#6b6b6b]",
            )}
            title="Terminal"
            aria-label="Terminal"
            type="button"
          >
            <Terminal className="w-4 h-4" />
          </button>

          <button
            data-testid="right-sidebar-tab-context"
            onClick={() => setActiveTab("context")}
            className={cn(
              "w-6 h-6 flex items-center justify-center rounded-[5px] transition-colors",
              activeTab === "context"
                ? "bg-[#eeeeee] text-[#1b1b1b]"
                : "text-[#9b9b9b] hover:bg-[#eeeeee] hover:text-[#6b6b6b]",
              !canUseContext && "opacity-60",
            )}
            title="Context"
            aria-label="Context"
            disabled={!canUseContext}
            type="button"
          >
            <Paperclip className="w-4 h-4" />
          </button>

          <button
            data-testid="right-sidebar-tab-changes"
            onClick={() => setActiveTab("changes")}
            className={cn(
              "w-6 h-6 flex items-center justify-center rounded-[5px] transition-colors",
              activeTab === "changes"
                ? "bg-[#eeeeee] text-[#1b1b1b]"
                : "text-[#9b9b9b] hover:bg-[#eeeeee] hover:text-[#6b6b6b]",
              !canUseChanges && "opacity-60",
            )}
            title="Changes"
            aria-label="Changes"
            disabled={!canUseChanges}
            type="button"
          >
            <GitCompareArrows className="w-4 h-4" />
          </button>
        </div>
      </div>

      <div className="flex-1 min-h-0 overflow-hidden">
        {activeTab === "terminal" ? (
          <div className="h-full min-h-0 overflow-hidden">
            <PtyTerminal />
          </div>
        ) : activeTab === "changes" ? (
          <div className="h-full overflow-auto overscroll-contain">
            <ChangesPanel workspaceId={activeWorkspaceId} onOpenDiffTab={onOpenDiffTab} />
          </div>
        ) : (
          <div className="h-full overflow-auto overscroll-contain">
            <ContextPanel
              workspaceId={activeWorkspaceId}
              isDragOver={isDragOver}
              droppedFiles={droppedFiles}
              onConsumeDroppedFiles={() => setDroppedFiles(null)}
            />
          </div>
        )}
      </div>
    </div>
  )
}

function ChangesPanel({
  workspaceId,
  onOpenDiffTab,
}: {
  workspaceId: number | null
  onOpenDiffTab?: (file: ChangedFile) => void
}) {
  const [files, setFiles] = useState<ChangedFile[]>([])
  const [error, setError] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [expandedGroups, setExpandedGroups] = useState<Set<FileChangeGroup>>(
    () => new Set(["staged", "unstaged"]),
  )

  const refresh = useCallback(async () => {
    if (workspaceId == null) return
    setIsLoading(true)
    setError(null)
    try {
      const snap = await fetchWorkspaceChanges(workspaceId)
      setFiles(snap.files ?? [])
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setIsLoading(false)
    }
  }, [workspaceId])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const toggleGroup = (group: FileChangeGroup) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev)
      if (next.has(group)) next.delete(group)
      else next.add(group)
      return next
    })
  }

  const committedFiles = files.filter((f) => f.group === "committed")
  const stagedFiles = files.filter((f) => f.group === "staged")
  const unstagedFiles = files.filter((f) => f.group === "unstaged")

  const getStatusColor = (status: FileChangeStatus) => {
    switch (status) {
      case "modified":
        return "text-status-warning"
      case "added":
        return "text-status-success"
      case "deleted":
        return "text-status-error"
      case "renamed":
        return "text-status-info"
      default:
        return "text-muted-foreground"
    }
  }

  const getStatusLabel = (status: FileChangeStatus) => {
    switch (status) {
      case "modified":
        return "M"
      case "added":
        return "A"
      case "deleted":
        return "D"
      case "renamed":
        return "R"
      default:
        return "?"
    }
  }

  const renderGroup = (title: string, list: ChangedFile[], group: FileChangeGroup) => {
    if (list.length === 0) return null
    const isExpanded = expandedGroups.has(group)

    return (
      <div key={group}>
        <button
          onClick={() => toggleGroup(group)}
          className="w-full flex items-center gap-1.5 px-2 py-1.5 hover:bg-muted/50 transition-colors"
        >
          {isExpanded ? (
            <ChevronDown className="w-3 h-3 text-muted-foreground" />
          ) : (
            <ChevronRight className="w-3 h-3 text-muted-foreground" />
          )}
          <span className="text-xs font-medium text-foreground">{title}</span>
          <span className="text-[10px] text-muted-foreground">({list.length})</span>
        </button>

        {isExpanded && (
          <div className="space-y-px">
            {list.map((file) => (
              <button
                key={file.id}
                onClick={() => onOpenDiffTab?.(file)}
                className="group w-full flex items-center gap-2 py-1 px-2 pl-6 hover:bg-muted/50 transition-colors text-left"
              >
                <span className={cn("text-[10px] font-mono font-semibold w-3", getStatusColor(file.status))}>
                  {getStatusLabel(file.status)}
                </span>
                <span className="flex-1 text-xs truncate text-muted-foreground group-hover:text-foreground">
                  {file.name}
                </span>
                {(file.additions != null || file.deletions != null) && (
                  <span className="text-[10px] text-muted-foreground/70">
                    {file.additions != null && file.additions > 0 && (
                      <span className="text-status-success">+{file.additions}</span>
                    )}
                    {file.additions != null &&
                      file.deletions != null &&
                      file.additions > 0 &&
                      file.deletions > 0 && <span className="mx-0.5">/</span>}
                    {file.deletions != null && file.deletions > 0 && (
                      <span className="text-status-error">-{file.deletions}</span>
                    )}
                  </span>
                )}
              </button>
            ))}
          </div>
        )}
      </div>
    )
  }

  if (workspaceId == null) {
    return <div className="p-3 text-xs text-muted-foreground">Select a workspace to view changes.</div>
  }

  return (
    <div className="py-1">
      {isLoading && <div className="px-3 py-2 text-xs text-muted-foreground">Loading…</div>}
      {error && <div className="px-3 py-2 text-xs text-destructive">{error}</div>}

      {renderGroup("Committed", committedFiles, "committed")}
      {renderGroup("Staged", stagedFiles, "staged")}
      {renderGroup("Unstaged", unstagedFiles, "unstaged")}

      {files.length === 0 && !isLoading && !error && (
        <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
          <GitCompareArrows className="w-8 h-8 mb-2 opacity-50" />
          <span className="text-xs">No changes</span>
        </div>
      )}
    </div>
  )
}

type ContextFileType = "image" | "document" | "data" | "code" | "text"

type ContextFileNode = {
  id: string
  name: string
  type: ContextFileType
  path: string
  contextId: number
  attachment: AttachmentRef
}

function fileTypeForAttachment(att: AttachmentRef): ContextFileType {
  if (att.kind === "image") return "image"
  const ext = att.extension.toLowerCase()
  if (ext === "json" || ext === "csv" || ext === "xml" || ext === "yaml" || ext === "yml" || ext === "toml") {
    return "data"
  }
  if (ext === "ts" || ext === "tsx" || ext === "js" || ext === "jsx" || ext === "rs" || ext === "go" || ext === "py") {
    return "code"
  }
  if (ext === "txt" || ext === "md") return "text"
  return "document"
}

function buildContextList(items: ContextItemSnapshot[]): ContextFileNode[] {
  return items.map((item) => {
    const att = item.attachment
    const type = fileTypeForAttachment(att)
    const name = att.name
    const path = `/context/${name}`
    return {
      id: `ctx-${item.context_id}`,
      name,
      type,
      path,
      contextId: item.context_id,
      attachment: att,
    }
  })
}

function attachmentKindForFile(file: File): "image" | "text" | "file" {
  const name = file.name.toLowerCase()
  if (file.type.startsWith("image/")) return "image"
  if (file.type.startsWith("text/")) return "text"
  if (name.endsWith(".md") || name.endsWith(".txt") || name.endsWith(".json") || name.endsWith(".csv") || name.endsWith(".yaml") || name.endsWith(".yml")) {
    return "text"
  }
  return "file"
}

function ContextPanel({
  workspaceId,
  isDragOver,
  droppedFiles,
  onConsumeDroppedFiles,
}: {
  workspaceId: number | null
  isDragOver: boolean
  droppedFiles: File[] | null
  onConsumeDroppedFiles: () => void
}) {
  const [items, setItems] = useState<ContextItemSnapshot[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set())
  const fileInputRef = useRef<HTMLInputElement | null>(null)

  const refresh = useCallback(async () => {
    if (workspaceId == null) return
    setIsLoading(true)
    setError(null)
    try {
      const snap = await fetchContext(workspaceId)
      setItems(snap.items ?? [])
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setIsLoading(false)
    }
  }, [workspaceId])

  const uploadFiles = useCallback(
    async (files: File[]) => {
      if (workspaceId == null || files.length === 0) return
      await Promise.all(
        files.map((file) => uploadAttachment({ workspaceId, file, kind: attachmentKindForFile(file) })),
      )
      emitContextChanged(workspaceId)
      await refresh()
    },
    [refresh, workspaceId],
  )

  useEffect(() => {
    void refresh()
  }, [refresh])

  useEffect(() => {
    if (workspaceId == null) return
    return onContextChanged((wid) => {
      if (wid !== workspaceId) return
      void refresh()
    })
  }, [refresh, workspaceId])

  useEffect(() => {
    if (!droppedFiles || droppedFiles.length === 0) return
    if (workspaceId == null) return

    const files = droppedFiles
    onConsumeDroppedFiles()

    ;(async () => {
      try {
        await uploadFiles(files)
      } catch (err) {
        toast.error(err instanceof Error ? err.message : String(err))
      }
    })()
  }, [droppedFiles, onConsumeDroppedFiles, uploadFiles, workspaceId])

  const list = useMemo(() => buildContextList(items), [items])

  const toggleSelect = (id: string, event: React.MouseEvent) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (event.metaKey || event.ctrlKey) {
        if (next.has(id)) next.delete(id)
        else next.add(id)
      } else {
        next.clear()
        next.add(id)
      }
      return next
    })
  }

  const handleAddToChat = (item: ContextFileNode) => {
    emitAddChatAttachments([item.attachment])
    focusChatInput()
  }

  const handleCopyPath = (item: ContextFileNode) => {
    void navigator.clipboard.writeText(item.path).catch(() => {})
    toast("Copied path")
  }

  const handleDelete = async (item: ContextFileNode) => {
    if (workspaceId == null) return
    try {
      await deleteContextItem(workspaceId, item.contextId)
      setItems((prev) => prev.filter((i) => i.context_id !== item.contextId))
      emitContextChanged(workspaceId)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    }
  }

  const handleAddContext = () => {
    fileInputRef.current?.click()
  }

  const handleFileSelect = (files: FileList | null) => {
    if (!files || workspaceId == null) return
    const picked = Array.from(files)
    fileInputRef.current && (fileInputRef.current.value = "")
    void uploadFiles(picked).catch((err) => {
      toast.error(err instanceof Error ? err.message : String(err))
    })
  }

  if (workspaceId == null) {
    return <div className="p-3 text-xs text-muted-foreground">Select a workspace to manage context.</div>
  }

  return (
    <div className="flex flex-col h-full relative">
      {isDragOver && (
        <div className="absolute inset-0 flex items-center justify-center bg-primary/10 border-2 border-dashed border-primary rounded-lg m-2 z-10 pointer-events-none">
          <div className="text-center">
            <Paperclip className="w-8 h-8 text-primary mx-auto mb-2" />
            <span className="text-sm font-medium text-primary">Drop files to add context</span>
          </div>
        </div>
      )}

      <div className="flex-1 overflow-auto py-1">
        {isLoading && <div className="px-3 py-2 text-xs text-muted-foreground">Loading…</div>}
        {error && <div className="px-3 py-2 text-xs text-destructive">{error}</div>}

        {list.length === 0 && !isLoading && !error ? (
          <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
            <Paperclip className="w-8 h-8 mb-2 opacity-50" />
            <span className="text-xs">No contexts</span>
          </div>
        ) : (
          list.map((file) => (
            <ContextFileRow
              key={file.id}
              file={file}
              selectedIds={selectedIds}
              onSelect={toggleSelect}
              onAddToChat={handleAddToChat}
              onDelete={handleDelete}
              onCopyPath={handleCopyPath}
            />
          ))
        )}
      </div>

      <div className="p-3">
        <input
          ref={fileInputRef}
          type="file"
          multiple
          className="hidden"
          onChange={(e) => handleFileSelect(e.target.files)}
        />
        <button
          data-testid="context-add-more"
          onClick={handleAddContext}
          className="w-full aspect-[3/1] flex flex-col items-center justify-center gap-1.5 border border-dashed border-muted-foreground/30 rounded-lg text-muted-foreground hover:text-foreground hover:border-muted-foreground/50 hover:bg-muted/30 transition-colors"
        >
          <Paperclip className="w-5 h-5" />
          <span className="text-xs">Add more context</span>
        </button>
      </div>
    </div>
  )
}

function ContextFileRow({
  file,
  selectedIds,
  onSelect,
  onAddToChat,
  onDelete,
  onCopyPath,
}: {
  file: ContextFileNode
  selectedIds: Set<string>
  onSelect: (id: string, event: React.MouseEvent) => void
  onAddToChat: (file: ContextFileNode) => void
  onDelete: (file: ContextFileNode) => void
  onCopyPath: (file: ContextFileNode) => void
}) {
  const isSelected = selectedIds.has(file.id)

  const getFileIcon = (type: ContextFileType) => {
    switch (type) {
      case "image":
        return <FileImage className="w-4 h-4 text-status-info" />
      case "document":
        return <FileText className="w-4 h-4 text-status-error" />
      case "data":
        return <FileJson className="w-4 h-4 text-status-warning" />
      case "code":
        return <FileCode className="w-4 h-4 text-status-success" />
      default:
        return <FileIcon className="w-4 h-4 text-muted-foreground" />
    }
  }

  return (
    <div
      data-testid="context-file-row"
      className={cn(
        "group flex items-center gap-1 py-1 px-2 cursor-pointer transition-colors",
        isSelected ? "bg-accent text-accent-foreground" : "hover:bg-muted/50",
      )}
      style={{ paddingLeft: "8px" }}
      onClick={(e) => onSelect(file.id, e)}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("luban-context-attachment", JSON.stringify(file.attachment))
        e.dataTransfer.setData("context-item", JSON.stringify({ path: file.path }))
      }}
    >
      <span className="w-4" />
      {getFileIcon(file.type)}

      <span className="flex-1 text-xs truncate">{file.name}</span>

      <button
        data-testid="context-add-to-chat"
        onClick={(e) => {
          e.stopPropagation()
          onAddToChat(file)
        }}
        className="p-0.5 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-primary rounded transition-all"
        title="Add to chat"
      >
        <FilePlus className="w-3.5 h-3.5" />
      </button>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            className="p-0.5 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-foreground rounded transition-all"
            onClick={(e) => e.stopPropagation()}
          >
            <ChevronDown className="w-3.5 h-3.5" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-36">
          <DropdownMenuItem onClick={() => onCopyPath(file)}>
            <Copy className="w-3.5 h-3.5 mr-2" />
            Copy path
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => onDelete(file)} className="text-destructive focus:text-destructive">
            <Trash2 className="w-3.5 h-3.5 mr-2" />
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  )
}
