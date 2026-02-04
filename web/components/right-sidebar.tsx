"use client"

import type React from "react"

import { useCallback, useEffect, useState } from "react"
import { Terminal, ChevronDown, ChevronRight, GitCompareArrows } from "lucide-react"
import { cn } from "@/lib/utils"
import { PtyTerminal } from "./pty-terminal"
import { useLuban } from "@/lib/luban-context"
import type { ChangedFileSnapshot, FileChangeGroup, FileChangeStatus } from "@/lib/luban-api"
import { fetchWorkspaceChanges } from "@/lib/luban-http"

type RightPanelTab = "terminal" | "changes"

interface RightSidebarProps {
  widthPx: number
  onOpenDiffTab?: (file: ChangedFile) => void
}

export type ChangedFile = ChangedFileSnapshot

export function RightSidebar({ widthPx, onOpenDiffTab }: RightSidebarProps) {
  const { activeWorkdirId: activeWorkspaceId } = useLuban()
  const [activeTab, setActiveTab] = useState<RightPanelTab>("terminal")

  const canUseChanges = activeWorkspaceId != null

  return (
    <div
      className={cn("flex flex-col transition-colors")}
      style={{
        width: `${widthPx}px`,
        borderLeft: "1px solid #ebebeb",
        backgroundColor: "#fcfcfc",
      }}
    >
      <div
        className="flex items-center"
        style={{ height: "39px", padding: "0 16px", borderBottom: "1px solid #ebebeb" }}
      >
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
        {activeTab === "changes" ? (
          <div className="h-full overflow-auto overscroll-contain">
            <ChangesPanel workspaceId={activeWorkspaceId} onOpenDiffTab={onOpenDiffTab} />
          </div>
        ) : (
          <div className="h-full min-h-0 overflow-hidden">
            <PtyTerminal />
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
          type="button"
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
                type="button"
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

