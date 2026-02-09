"use client"

import { useEffect, useMemo, useState, type ComponentType } from "react"
import { FileCode2, GitBranch, MonitorPlay, TerminalSquare } from "lucide-react"

import { DiffTabPanel, type DiffFileData, type DiffStyle } from "@/components/diff-tab-panel"
import { PtyTerminal } from "@/components/pty-terminal"
import { TaskActivityPanel } from "@/components/task-activity-panel"
import type { WorkspaceChangesSnapshot } from "@/lib/luban-api"
import { useLuban } from "@/lib/luban-context"
import { fetchWorkspaceChanges, fetchWorkspaceDiff } from "@/lib/luban-http"

type WorkspaceTab = "agents" | "changes" | "preview" | "terminal"

type ChangesState = {
  loading: boolean
  error: string | null
  snapshot: WorkspaceChangesSnapshot | null
}

type DiffState = {
  loading: boolean
  error: string | null
  files: DiffFileData[]
}

const TABS: Array<{ key: WorkspaceTab; label: string; icon: ComponentType<{ className?: string }> }> = [
  { key: "agents", label: "Agents", icon: MonitorPlay },
  { key: "changes", label: "Changes", icon: GitBranch },
  { key: "preview", label: "Preview", icon: FileCode2 },
  { key: "terminal", label: "Terminal", icon: TerminalSquare },
]

export function TaskWorkspacePanel() {
  const { activeWorkdirId, activeTaskId } = useLuban()
  const [activeTab, setActiveTab] = useState<WorkspaceTab>("agents")
  const [changes, setChanges] = useState<ChangesState>({
    loading: false,
    error: null,
    snapshot: null,
  })
  const [diff, setDiff] = useState<DiffState>({
    loading: false,
    error: null,
    files: [],
  })
  const [diffStyle, setDiffStyle] = useState<DiffStyle>("split")

  const scope = `${activeWorkdirId ?? "none"}:${activeTaskId ?? "none"}`

  useEffect(() => {
    setActiveTab("agents")
  }, [scope])

  useEffect(() => {
    if (activeWorkdirId == null) {
      setChanges({ loading: false, error: null, snapshot: null })
      return
    }
    if (activeTab !== "changes") return
    if (changes.snapshot?.workdir_id === activeWorkdirId && changes.error == null) return

    let cancelled = false
    setChanges((prev) => ({ ...prev, loading: true, error: null }))
    void (async () => {
      try {
        const snapshot = await fetchWorkspaceChanges(activeWorkdirId)
        if (cancelled) return
        setChanges({ loading: false, error: null, snapshot })
      } catch (err) {
        if (cancelled) return
        setChanges({
          loading: false,
          error: err instanceof Error ? err.message : String(err),
          snapshot: null,
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [activeTab, activeWorkdirId, changes.error, changes.snapshot?.workdir_id])

  useEffect(() => {
    if (activeWorkdirId == null) {
      setDiff({ loading: false, error: null, files: [] })
      return
    }
    if (activeTab !== "preview") return
    if (diff.files.length > 0 && diff.error == null) return

    let cancelled = false
    setDiff((prev) => ({ ...prev, loading: true, error: null }))
    void (async () => {
      try {
        const snapshot = await fetchWorkspaceDiff(activeWorkdirId)
        if (cancelled) return
        const files: DiffFileData[] = (snapshot.files ?? []).map((file) => ({
          file: file.file,
          oldFile: { name: file.old_file.name, contents: file.old_file.contents },
          newFile: { name: file.new_file.name, contents: file.new_file.contents },
        }))
        setDiff({ loading: false, error: null, files })
      } catch (err) {
        if (cancelled) return
        setDiff({
          loading: false,
          error: err instanceof Error ? err.message : String(err),
          files: [],
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [activeTab, activeWorkdirId, diff.error, diff.files.length])

  const hasActiveTask = activeWorkdirId != null && activeTaskId != null

  const changesContent = useMemo(() => {
    if (activeWorkdirId == null) {
      return <div className="px-4 py-3 text-xs text-muted-foreground">Select a task first.</div>
    }
    if (changes.loading) {
      return <div className="px-4 py-3 text-xs text-muted-foreground">Loading changes...</div>
    }
    if (changes.error) {
      return <div className="px-4 py-3 text-xs text-destructive">{changes.error}</div>
    }

    const files = changes.snapshot?.files ?? []
    if (files.length === 0) {
      return <div className="px-4 py-3 text-xs text-muted-foreground">No code changes.</div>
    }

    return (
      <div className="h-full overflow-auto px-3 py-2 space-y-2" data-testid="task-workspace-changes-list">
        {files.map((file) => (
          <div key={file.id} className="rounded border border-border bg-card px-3 py-2">
            <div className="flex items-center gap-2">
              <span className="text-[11px] font-mono text-muted-foreground">{file.group}</span>
              <span className="text-xs font-medium text-foreground">{file.status.toUpperCase()}</span>
              <span className="text-xs text-muted-foreground truncate">{file.path}</span>
            </div>
          </div>
        ))}
      </div>
    )
  }, [activeWorkdirId, changes.error, changes.loading, changes.snapshot?.files])

  return (
    <div className="h-full min-h-0 flex flex-col border-l border-border bg-background" data-testid="task-workspace-panel">
      <div className="px-3 py-2 border-b border-border flex items-center gap-1 overflow-x-auto">
        {TABS.map((tab) => {
          const Icon = tab.icon
          const active = activeTab === tab.key
          return (
            <button
              key={tab.key}
              type="button"
              data-testid={`task-workspace-tab-${tab.key}`}
              onClick={() => setActiveTab(tab.key)}
              className={`shrink-0 inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs border ${
                active
                  ? "border-zinc-300 bg-zinc-100 text-zinc-900"
                  : "border-transparent bg-transparent text-zinc-600 hover:bg-zinc-100"
              }`}
            >
              <Icon className="w-3.5 h-3.5" />
              <span>{tab.label}</span>
            </button>
          )
        })}
      </div>

      <div className="flex-1 min-h-0" data-testid="task-workspace-content">
        {!hasActiveTask ? (
          <div className="h-full flex items-center justify-center text-sm text-muted-foreground">
            Select a task to view workspace details.
          </div>
        ) : activeTab === "agents" ? (
          <div className="h-full min-h-0 overflow-hidden">
            <TaskActivityPanel
              showInput={false}
              showTaskHeader={false}
              showActivityHeader={false}
            />
          </div>
        ) : activeTab === "changes" ? (
          changesContent
        ) : activeTab === "preview" ? (
          <div className="h-full min-h-0 overflow-hidden">
            <DiffTabPanel
              isLoading={diff.loading}
              error={diff.error}
              files={diff.files}
              diffStyle={diffStyle}
              onStyleChange={setDiffStyle}
            />
          </div>
        ) : (
          <div className="h-full px-3 py-3">
            <div className="h-full rounded border border-border overflow-hidden bg-card">
              <PtyTerminal autoFocus />
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
