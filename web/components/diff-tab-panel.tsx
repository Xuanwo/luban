"use client"

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import { AlignJustify, ChevronDown, ChevronRight, Columns2, GitCompareArrows } from "lucide-react"

import { cn } from "@/lib/utils"
import {
  MultiFileDiff,
  WorkerPoolContextProvider,
  type FileContents,
  type WorkerInitializationRenderOptions,
  type WorkerPoolOptions,
} from "@pierre/diffs/react"
import type { ChangedFile } from "./right-sidebar"

export type DiffStyle = "split" | "unified"

export interface DiffFileData {
  file: ChangedFile
  oldFile: FileContents
  newFile: FileContents
}

export function DiffTabPanel({
  isLoading,
  error,
  files,
  activeFileId,
  diffStyle,
  onStyleChange,
}: {
  isLoading: boolean
  error: string | null
  files: DiffFileData[]
  activeFileId?: string
  diffStyle: DiffStyle
  onStyleChange: (style: DiffStyle) => void
}) {
  if (isLoading) {
    return <div className="px-4 py-3 text-xs text-muted-foreground">Loading…</div>
  }

  if (error) {
    return <div className="px-4 py-3 text-xs text-destructive">{error}</div>
  }

  if (files.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
        <GitCompareArrows className="w-8 h-8 mb-2 opacity-50" />
        <span className="text-xs">No changes</span>
      </div>
    )
  }

  return (
    <WorkerPoolContextProvider poolOptions={pierreDiffWorkerPoolOptions} highlighterOptions={pierreDiffHighlighterOptions}>
      <AllFilesDiffViewer
        files={files}
        activeFileId={activeFileId}
        diffStyle={diffStyle}
        onStyleChange={onStyleChange}
      />
    </WorkerPoolContextProvider>
  )
}

const pierreDiffWorkerPoolOptions: WorkerPoolOptions = {
  workerFactory: () => new Worker(new URL("../node_modules/@pierre/diffs/dist/worker/worker-portable.js", import.meta.url)),
  poolSize: 2,
}

const pierreDiffHighlighterOptions: WorkerInitializationRenderOptions = { langs: ["text"] }

function AllFilesDiffViewer({
  files,
  activeFileId,
  diffStyle,
  onStyleChange,
}: {
  files: DiffFileData[]
  activeFileId?: string
  diffStyle: DiffStyle
  onStyleChange: (style: DiffStyle) => void
}) {
  const fileRefs = useRef<Record<string, HTMLDivElement | null>>({})
  const prevActiveFileIdRef = useRef<string | undefined>(undefined)
  const [collapsedFiles, setCollapsedFiles] = useState<Set<string>>(() => new Set())

  const toggleCollapse = (fileId: string) => {
    setCollapsedFiles((prev) => {
      const next = new Set(prev)
      if (next.has(fileId)) next.delete(fileId)
      else next.add(fileId)
      return next
    })
  }

  const scrollContainerRef = useRef<HTMLDivElement | null>(null)
  const [renderedFiles, setRenderedFiles] = useState<Set<string>>(() => new Set())
  const collapsedFilesRef = useRef<Set<string>>(new Set())
  const renderedFilesRef = useRef<Set<string>>(new Set())
  const pendingRenderIdsRef = useRef<Set<string>>(new Set())
  const flushTimerRef = useRef<number | null>(null)

  const fileIndexById = useMemo(() => {
    const out = new Map<string, number>()
    for (const [idx, item] of files.entries()) {
      out.set(item.file.id, idx)
    }
    return out
  }, [files])

  const flushPendingRenderedFiles = useCallback(() => {
    flushTimerRef.current = null
    const pending = pendingRenderIdsRef.current
    if (pending.size === 0) return
    setRenderedFiles((prev) => {
      const next = new Set(prev)
      for (const id of pending) next.add(id)
      pending.clear()
      return next
    })
  }, [])

  const renderFilesImmediately = useCallback((ids: string[]) => {
    const unique = ids.filter((id) => !renderedFilesRef.current.has(id))
    if (unique.length === 0) return
    setRenderedFiles((prev) => {
      const next = new Set(prev)
      for (const id of unique) next.add(id)
      return next
    })
  }, [])

  const scheduleRenderFiles = useCallback((ids: string[]) => {
    for (const id of ids) {
      if (renderedFilesRef.current.has(id)) continue
      pendingRenderIdsRef.current.add(id)
    }
    if (pendingRenderIdsRef.current.size === 0) return
    if (flushTimerRef.current != null) return

    type RequestIdleCallbackFn = (callback: IdleRequestCallback, options?: IdleRequestOptions) => number
    const ric = (globalThis as { requestIdleCallback?: RequestIdleCallbackFn }).requestIdleCallback
    if (typeof ric === "function") {
      flushTimerRef.current = ric(flushPendingRenderedFiles, { timeout: 200 })
      return
    }

    flushTimerRef.current = globalThis.setTimeout(flushPendingRenderedFiles, 0) as unknown as number
  }, [flushPendingRenderedFiles])

  useEffect(() => {
    collapsedFilesRef.current = collapsedFiles
  }, [collapsedFiles])

  useEffect(() => {
    renderedFilesRef.current = renderedFiles
  }, [renderedFiles])

  useEffect(() => {
    if (!activeFileId) return
    if (activeFileId === prevActiveFileIdRef.current) return

    const el = fileRefs.current[activeFileId]
    if (!el) return

    if (collapsedFilesRef.current.has(activeFileId)) {
      setCollapsedFiles((prev) => {
        const next = new Set(prev)
        next.delete(activeFileId)
        return next
      })
    }

    renderFilesImmediately([activeFileId])
    el.scrollIntoView({ behavior: "smooth", block: "start" })
    prevActiveFileIdRef.current = activeFileId
  }, [activeFileId, collapsedFiles, renderFilesImmediately])

  useEffect(() => {
    const root = scrollContainerRef.current
    if (!root) return

    const observer = new IntersectionObserver(
      (entries) => {
        const toRender: string[] = []
        for (const entry of entries) {
          if (!entry.isIntersecting) continue
          const target = entry.target as HTMLElement
          const fileId = target.dataset.diffFileId
          if (!fileId) continue
          if (collapsedFilesRef.current.has(fileId)) continue
          if (renderedFilesRef.current.has(fileId)) continue
          toRender.push(fileId)

          const idx = fileIndexById.get(fileId)
          if (idx == null) continue
          for (const delta of [-1, 1]) {
            const n = files[idx + delta]
            if (!n) continue
            if (!collapsedFilesRef.current.has(n.file.id) && !renderedFilesRef.current.has(n.file.id))
              toRender.push(n.file.id)
          }
        }
        if (toRender.length > 0) scheduleRenderFiles(toRender)
      },
      { root, rootMargin: "800px 0px 800px 0px", threshold: 0.01 },
    )

    for (const item of files) {
      const el = fileRefs.current[item.file.id]
      if (el) observer.observe(el)
    }

    return () => {
      observer.disconnect()
    }
  }, [fileIndexById, files, scheduleRenderFiles])

  useLayoutEffect(() => {
    if (files.length === 0) return
    if (renderedFilesRef.current.size > 0) return
    const initial: string[] = []
    if (activeFileId) initial.push(activeFileId)
    initial.push(files[0].file.id)
    renderFilesImmediately(initial)
  }, [activeFileId, files, renderFilesImmediately])

  const getStatusColor = (status: ChangedFile["status"]) => {
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

  const getStatusLabel = (status: ChangedFile["status"]) => {
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

  const totalAdditions = files.reduce((sum, f) => sum + (f.file.additions ?? 0), 0)
  const totalDeletions = files.reduce((sum, f) => sum + (f.file.deletions ?? 0), 0)

  return (
    <div className="flex-1 min-h-0 flex flex-col overflow-hidden bg-background" data-testid="diff-viewer">
      <div className="flex items-center gap-2 px-4 py-2 bg-muted/50 border-b border-border text-xs">
        <span className="text-foreground font-medium">{files.length} files changed</span>
        <span className="text-muted-foreground">
          {totalAdditions > 0 && <span className="text-status-success">+{totalAdditions}</span>}
          {totalAdditions > 0 && totalDeletions > 0 && <span className="mx-1">/</span>}
          {totalDeletions > 0 && <span className="text-status-error">-{totalDeletions}</span>}
        </span>
        <div className="ml-auto flex items-center gap-0.5 p-0.5 bg-muted rounded">
          <button
            onClick={() => onStyleChange("split")}
            className={cn(
              "p-1 rounded transition-colors",
              diffStyle === "split"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
            title="Split view"
          >
            <Columns2 className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={() => onStyleChange("unified")}
            className={cn(
              "p-1 rounded transition-colors",
              diffStyle === "unified"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
            title="Unified view"
          >
            <AlignJustify className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      <div
        className="flex-1 min-h-0 overflow-auto"
        data-testid="diff-scroll-container"
        ref={scrollContainerRef}
      >
        {files.map((fileData) => {
          const isCollapsed = collapsedFiles.has(fileData.file.id)
          const isRendered =
            files.length === 1 || renderedFiles.has(fileData.file.id) || (activeFileId != null && fileData.file.id === activeFileId)
          return (
            <div
              key={fileData.file.id}
              data-diff-file-id={fileData.file.id}
              ref={(el) => {
                fileRefs.current[fileData.file.id] = el
              }}
              className="border-b border-border last:border-b-0"
            >
              <button
                onClick={() => {
                  const fileId = fileData.file.id
                  const willExpand = collapsedFilesRef.current.has(fileId)
                  toggleCollapse(fileId)
                  if (willExpand) renderFilesImmediately([fileId])
                }}
                className="sticky top-0 z-[5] w-full flex items-center gap-2 px-4 py-2 bg-muted/80 backdrop-blur-sm border-b border-border/50 text-xs hover:bg-muted transition-colors text-left"
              >
                {isCollapsed ? (
                  <ChevronRight className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                ) : (
                  <ChevronDown className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                )}
                <span className={cn("font-mono font-semibold", getStatusColor(fileData.file.status))}>
                  {getStatusLabel(fileData.file.status)}
                </span>
                <span className="font-mono text-foreground">{fileData.file.path}</span>
                {fileData.file.additions != null && fileData.file.additions > 0 && (
                  <span className="text-status-success">+{fileData.file.additions}</span>
                )}
                {fileData.file.deletions != null && fileData.file.deletions > 0 && (
                  <span className="text-status-error">-{fileData.file.deletions}</span>
                )}
              </button>

              {!isCollapsed && (
                <>
                  {!isRendered ? (
                    <div className="px-4 py-3 text-xs text-muted-foreground">Rendering…</div>
                  ) : (
                    <MultiFileDiff
                      oldFile={fileData.oldFile}
                      newFile={fileData.newFile}
                      options={{
                        theme: { dark: "pierre-dark", light: "pierre-light" },
                        diffStyle: diffStyle,
                        diffIndicators: "bars",
                        hunkSeparators: "line-info",
                        lineDiffType: "word-alt",
                        enableLineSelection: true,
                      }}
                    />
                  )}
                </>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
