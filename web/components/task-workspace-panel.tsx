"use client"

import { useCallback, useEffect, useMemo, useRef, useState, type ComponentType } from "react"
import { FileCode2, FilePlus2, FileMinus2, FileEdit, FileSymlink, GitBranch, MonitorPlay, TerminalSquare } from "lucide-react"

import type { ChangedFileSnapshot, FileChangeStatus, WorkspaceDiffFileSnapshot } from "@/lib/luban-api"
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

type SelectedFileDiff = {
  fileId: string
  loading: boolean
  error: string | null
  data: DiffFileData | null
}

const STATUS_CONFIG: Record<FileChangeStatus, { label: string; color: string; icon: ComponentType<{ className?: string }> }> = {
  added:    { label: "A", color: "#27ae60", icon: FilePlus2 },
  modified: { label: "M", color: "#f2994a", icon: FileEdit },
  deleted:  { label: "D", color: "#eb5757", icon: FileMinus2 },
  renamed:  { label: "R", color: "#5e6ad2", icon: FileSymlink },
}

const GROUP_LABELS: Record<string, string> = {
  committed: "Committed",
  staged: "Staged",
  unstaged: "Unstaged",
}

const GROUP_ORDER = ["staged", "unstaged", "committed"] as const

const TABS: Array<{ key: WorkspaceTab; label: string; icon: ComponentType<{ className?: string }> }> = [
  { key: "agents", label: "Agents", icon: MonitorPlay },
  { key: "changes", label: "Changes", icon: GitBranch },
  { key: "preview", label: "Preview", icon: FileCode2 },
  { key: "terminal", label: "Terminal", icon: TerminalSquare },
]

function toDiffFileData(df: WorkspaceDiffFileSnapshot): DiffFileData {
  return {
    file: df.file,
    oldFile: { name: df.old_file.name, contents: df.old_file.contents },
    newFile: { name: df.new_file.name, contents: df.new_file.contents },
  }
}

function ChangesFileList({
  files,
  selectedFileId,
  onSelectFile,
}: {
  files: ChangedFileSnapshot[]
  selectedFileId: string | null
  onSelectFile: (file: ChangedFileSnapshot) => void
}) {
  const grouped = useMemo(() => {
    const map = new Map<string, ChangedFileSnapshot[]>()
    for (const file of files) {
      const group = map.get(file.group) ?? []
      group.push(file)
      map.set(file.group, group)
    }
    return map
  }, [files])

  const sortedGroups = useMemo(
    () => GROUP_ORDER.filter((g) => grouped.has(g)),
    [grouped],
  )

  return (
    <div className="h-full overflow-auto" data-testid="task-workspace-changes-list">
      {sortedGroups.map((group) => {
        const groupFiles = grouped.get(group) ?? []
        return (
          <div key={group}>
            <div
              className="flex items-center gap-2 sticky top-0"
              style={{
                padding: '8px 20px 6px',
                fontSize: '11px',
                fontWeight: 500,
                color: '#9b9b9b',
                letterSpacing: '0.3px',
                textTransform: 'uppercase',
                backgroundColor: '#fcfcfc',
              }}
            >
              <span>{GROUP_LABELS[group] ?? group}</span>
              <span style={{ color: '#c8c8c8' }}>{groupFiles.length}</span>
            </div>
            {groupFiles.map((file) => {
              const sc = STATUS_CONFIG[file.status]
              const StatusIcon = sc.icon
              const dir = file.path.includes("/") ? file.path.substring(0, file.path.lastIndexOf("/") + 1) : ""
              const selected = file.id === selectedFileId
              return (
                <div
                  key={file.id}
                  role="button"
                  tabIndex={0}
                  onClick={() => onSelectFile(file)}
                  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onSelectFile(file) }}
                  className="flex items-center gap-2 cursor-pointer transition-colors"
                  style={{
                    padding: '3px 20px',
                    fontSize: '12px',
                    backgroundColor: selected ? '#eef0fb' : 'transparent',
                  }}
                  onMouseEnter={(e) => { if (!selected) e.currentTarget.style.backgroundColor = '#f7f7f7' }}
                  onMouseLeave={(e) => { if (!selected) e.currentTarget.style.backgroundColor = 'transparent' }}
                >
                  <StatusIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: sc.color }} />
                  <span className="truncate min-w-0 flex-1" style={{ color: '#1b1b1b' }}>
                    {dir && <span style={{ color: '#9b9b9b' }}>{dir}</span>}
                    {file.name}
                  </span>
                  {(file.additions != null || file.deletions != null) && (
                    <span className="flex-shrink-0 flex items-center gap-1" style={{ fontSize: '11px', fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace' }}>
                      {file.additions != null && file.additions > 0 && (
                        <span style={{ color: '#27ae60' }}>+{file.additions}</span>
                      )}
                      {file.deletions != null && file.deletions > 0 && (
                        <span style={{ color: '#eb5757' }}>−{file.deletions}</span>
                      )}
                    </span>
                  )}
                  <span
                    className="flex-shrink-0"
                    style={{ fontSize: '10px', fontWeight: 600, color: sc.color, width: '12px', textAlign: 'center' }}
                  >
                    {sc.label}
                  </span>
                </div>
              )
            })}
          </div>
        )
      })}
    </div>
  )
}

export function TaskWorkspacePanel() {
  const { activeWorkdirId, activeTaskId } = useLuban()
  const [activeTab, setActiveTab] = useState<WorkspaceTab>("agents")
  const [changes, setChanges] = useState<ChangesState>({
    loading: false,
    error: null,
    snapshot: null,
  })
  const [selectedDiff, setSelectedDiff] = useState<SelectedFileDiff | null>(null)
  const [diffStyle, setDiffStyle] = useState<DiffStyle>("unified")
  const [topHeightPercent, setTopHeightPercent] = useState(35)
  const [isDragging, setIsDragging] = useState(false)
  const splitContainerRef = useRef<HTMLDivElement>(null)
  const diffCacheRef = useRef<Map<string, DiffFileData>>(new Map())

  const scope = `${activeWorkdirId ?? "none"}:${activeTaskId ?? "none"}`

  useEffect(() => {
    setActiveTab("agents")
  }, [scope])

  useEffect(() => {
    setSelectedDiff(null)
    diffCacheRef.current.clear()
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

  const handleSelectFile = useCallback(
    (file: ChangedFileSnapshot) => {
      if (activeWorkdirId == null) return

      const cached = diffCacheRef.current.get(file.id)
      if (cached) {
        setSelectedDiff({ fileId: file.id, loading: false, error: null, data: cached })
        return
      }

      setSelectedDiff({ fileId: file.id, loading: true, error: null, data: null })

      void (async () => {
        try {
          const snapshot = await fetchWorkspaceDiff(activeWorkdirId)
          const match = snapshot.files.find((f) => f.file.id === file.id)
          if (match) {
            const data = toDiffFileData(match)
            diffCacheRef.current.set(file.id, data)
            setSelectedDiff((prev) =>
              prev?.fileId === file.id ? { fileId: file.id, loading: false, error: null, data } : prev,
            )
          } else {
            setSelectedDiff((prev) =>
              prev?.fileId === file.id
                ? { fileId: file.id, loading: false, error: "File not found in diff.", data: null }
                : prev,
            )
          }
          for (const df of snapshot.files) {
            if (!diffCacheRef.current.has(df.file.id)) {
              diffCacheRef.current.set(df.file.id, toDiffFileData(df))
            }
          }
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err)
          setSelectedDiff((prev) =>
            prev?.fileId === file.id ? { fileId: file.id, loading: false, error: msg, data: null } : prev,
          )
        }
      })()
    },
    [activeWorkdirId],
  )

  const handleResizePointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    setIsDragging(true)
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  }, [])

  const handleResizePointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!isDragging || !splitContainerRef.current) return
      const rect = splitContainerRef.current.getBoundingClientRect()
      const y = e.clientY - rect.top
      const pct = Math.min(70, Math.max(20, (y / rect.height) * 100))
      setTopHeightPercent(pct)
    },
    [isDragging],
  )

  const handleResizePointerUp = useCallback(() => {
    setIsDragging(false)
  }, [])

  const hasActiveTask = activeWorkdirId != null && activeTaskId != null

  const files = changes.snapshot?.files ?? []

  const changesContent = useMemo(() => {
    if (activeWorkdirId == null) {
      return (
        <div className="h-full flex items-center justify-center" style={{ fontSize: '13px', color: '#9b9b9b' }}>
          Select a task first.
        </div>
      )
    }
    if (changes.loading) {
      return (
        <div className="h-full flex items-center justify-center" style={{ fontSize: '13px', color: '#9b9b9b' }}>
          Loading changes...
        </div>
      )
    }
    if (changes.error) {
      return (
        <div className="h-full flex items-center justify-center" style={{ fontSize: '13px', color: '#eb5757' }}>
          {changes.error}
        </div>
      )
    }

    if (files.length === 0) {
      return (
        <div className="h-full flex flex-col items-center justify-center gap-2" style={{ color: '#9b9b9b' }}>
          <GitBranch className="w-8 h-8" style={{ color: '#d4d4d4' }} />
          <span style={{ fontSize: '13px', fontWeight: 500 }}>No changes</span>
          <span style={{ fontSize: '12px' }}>Working tree is clean.</span>
        </div>
      )
    }

    return (
      <div ref={splitContainerRef} className="h-full flex flex-col min-h-0">
        {/* File list */}
        <div
          className="min-h-0 shrink-0 overflow-hidden"
          style={{ height: selectedDiff ? `${topHeightPercent}%` : '100%' }}
        >
          <ChangesFileList
            files={files}
            selectedFileId={selectedDiff?.fileId ?? null}
            onSelectFile={handleSelectFile}
          />
        </div>

        {/* Resizer + Diff panel */}
        {selectedDiff && (
          <>
            <div
              className="shrink-0 flex items-center justify-center"
              style={{
                height: '6px',
                cursor: 'row-resize',
                userSelect: 'none',
                borderTop: '1px solid #ebebeb',
              }}
              onPointerDown={handleResizePointerDown}
              onPointerMove={handleResizePointerMove}
              onPointerUp={handleResizePointerUp}
            >
              <div
                style={{
                  width: '32px',
                  height: isDragging ? '2px' : '1px',
                  borderRadius: '1px',
                  backgroundColor: isDragging ? '#5e6ad2' : '#d4d4d4',
                  transition: isDragging ? 'none' : 'background-color 0.15s',
                }}
                onMouseEnter={(e) => {
                  if (!isDragging) e.currentTarget.style.backgroundColor = '#5e6ad2'
                }}
                onMouseLeave={(e) => {
                  if (!isDragging) e.currentTarget.style.backgroundColor = '#d4d4d4'
                }}
              />
            </div>

            <div
              className="flex-1 min-h-0 overflow-hidden"
              style={isDragging ? { pointerEvents: 'none' } : undefined}
            >
              {selectedDiff.loading ? (
                <div className="h-full flex items-center justify-center" style={{ fontSize: '12px', color: '#9b9b9b' }}>
                  Loading diff...
                </div>
              ) : selectedDiff.error ? (
                <div className="h-full flex items-center justify-center" style={{ fontSize: '12px', color: '#eb5757' }}>
                  {selectedDiff.error}
                </div>
              ) : selectedDiff.data ? (
                <DiffTabPanel
                  isLoading={false}
                  error={null}
                  files={[selectedDiff.data]}
                  activeFileId={selectedDiff.fileId}
                  diffStyle={diffStyle}
                  onStyleChange={setDiffStyle}
                />
              ) : null}
            </div>
          </>
        )}
      </div>
    )
  }, [activeWorkdirId, changes.error, changes.loading, files, selectedDiff, topHeightPercent, isDragging, diffStyle, handleSelectFile, handleResizePointerDown, handleResizePointerMove, handleResizePointerUp])

  return (
    <div className="h-full min-h-0 flex flex-col border-l border-border bg-background" data-testid="task-workspace-panel">
      <div
        className="flex items-center gap-4 px-5 overflow-x-auto flex-shrink-0"
        style={{ height: '39px', borderBottom: '1px solid #ebebeb' }}
      >
        {TABS.map((tab) => {
          const Icon = tab.icon
          const active = activeTab === tab.key
          return (
            <button
              key={tab.key}
              type="button"
              data-testid={`task-workspace-tab-${tab.key}`}
              onClick={() => setActiveTab(tab.key)}
              className="shrink-0 inline-flex items-center gap-1.5 h-full transition-colors"
              style={{
                fontSize: '12px',
                fontWeight: active ? 500 : 400,
                color: active ? '#1b1b1b' : '#6b6b6b',
                borderBottom: active ? '2px solid #5e6ad2' : '2px solid transparent',
                marginBottom: '-1px',
              }}
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
          <div className="h-full min-h-0 overflow-hidden flex flex-col">
            <TaskActivityPanel
              showInput={false}
              showTaskHeader={false}
              showActivityHeader={false}
              compact
            />
          </div>
        ) : activeTab === "changes" ? (
          changesContent
        ) : activeTab === "preview" ? (
          <div className="h-full flex flex-col items-center justify-center gap-2" style={{ color: '#9b9b9b' }}>
            <FileCode2 className="w-8 h-8" style={{ color: '#d4d4d4' }} />
            <span style={{ fontSize: '13px', fontWeight: 500 }}>Preview</span>
            <span style={{ fontSize: '12px' }}>This feature is under development.</span>
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
