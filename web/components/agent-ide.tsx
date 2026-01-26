"use client"

import { useEffect, useState } from "react"
import { Sidebar } from "./sidebar"
import { ChatPanel } from "./chat-panel"
import { RightSidebar } from "./right-sidebar"
import { KanbanBoard } from "./kanban-board"
import type { ChangedFile } from "./right-sidebar"
import {
  RIGHT_SIDEBAR_OPEN_KEY,
  RIGHT_SIDEBAR_WIDTH_KEY,
  SIDEBAR_WIDTH_KEY,
  VIEW_MODE_KEY,
} from "@/lib/ui-prefs"

function clamp(n: number, min: number, max: number) {
  return Math.round(Math.max(min, Math.min(max, n)))
}

export function AgentIDE() {
  const [rightSidebarOpen, setRightSidebarOpen] = useState(true)
  const [viewMode, setViewMode] = useState<"workspace" | "kanban">("workspace")
  const [sidebarWidthPx, setSidebarWidthPx] = useState(240)
  const [rightSidebarWidthPx, setRightSidebarWidthPx] = useState(320)
  const [pendingDiffFile, setPendingDiffFile] = useState<ChangedFile | null>(null)

  useEffect(() => {
    const rawOpen = localStorage.getItem(RIGHT_SIDEBAR_OPEN_KEY)
    setRightSidebarOpen(rawOpen !== "false")

    const rawViewMode = localStorage.getItem(VIEW_MODE_KEY)
    if (rawViewMode === "workspace" || rawViewMode === "kanban") {
      setViewMode(rawViewMode)
    }

    const rawSidebarWidth = Number(localStorage.getItem(SIDEBAR_WIDTH_KEY) ?? "")
    if (Number.isFinite(rawSidebarWidth) && rawSidebarWidth > 0) {
      setSidebarWidthPx(clamp(rawSidebarWidth, 200, 420))
    }

    const rawRightSidebarWidth = Number(localStorage.getItem(RIGHT_SIDEBAR_WIDTH_KEY) ?? "")
    if (Number.isFinite(rawRightSidebarWidth) && rawRightSidebarWidth > 0) {
      setRightSidebarWidthPx(clamp(rawRightSidebarWidth, 260, 640))
    }
  }, [])

  useEffect(() => {
    localStorage.setItem(RIGHT_SIDEBAR_OPEN_KEY, String(rightSidebarOpen))
  }, [rightSidebarOpen])

  useEffect(() => {
    localStorage.setItem(VIEW_MODE_KEY, viewMode)
  }, [viewMode])

  useEffect(() => {
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidthPx))
  }, [sidebarWidthPx])

  useEffect(() => {
    localStorage.setItem(RIGHT_SIDEBAR_WIDTH_KEY, String(rightSidebarWidthPx))
  }, [rightSidebarWidthPx])

  function startResize(args: {
    edge: "left" | "right"
    pointerDownClientX: number
    initialSidebarWidthPx: number
    initialRightSidebarWidthPx: number
  }) {
    const originalCursor = document.body.style.cursor
    const originalUserSelect = document.body.style.userSelect
    document.body.style.cursor = "col-resize"
    document.body.style.userSelect = "none"

    const onMove = (ev: PointerEvent) => {
      const dx = ev.clientX - args.pointerDownClientX
      if (args.edge === "left") {
        setSidebarWidthPx(clamp(args.initialSidebarWidthPx + dx, 200, 420))
      } else {
        setRightSidebarWidthPx(clamp(args.initialRightSidebarWidthPx - dx, 260, 640))
      }
    }

    const onUp = () => {
      window.removeEventListener("pointermove", onMove)
      window.removeEventListener("pointerup", onUp)
      document.body.style.cursor = originalCursor
      document.body.style.userSelect = originalUserSelect
    }

    window.addEventListener("pointermove", onMove)
    window.addEventListener("pointerup", onUp, { once: true })
  }

  return (
    <div className="flex flex-col h-screen bg-background text-foreground overflow-hidden">
      <div className="relative flex flex-1 min-h-0 overflow-hidden">
        {viewMode === "workspace" ? (
          <>
            <Sidebar viewMode={viewMode} onViewModeChange={setViewMode} widthPx={sidebarWidthPx} />

            {/* Use a zero-width wrapper so borders remain continuous across panes. */}
            <div className="relative w-0 flex-shrink-0">
              <div
                data-testid="sidebar-resizer"
                className="absolute -left-1 top-0 h-full w-2 bg-transparent hover:bg-border/60 active:bg-border cursor-col-resize"
                title="Resize sidebar"
                onPointerDown={(e) => {
                  if (e.button !== 0) return
                  e.preventDefault()
                  startResize({
                    edge: "left",
                    pointerDownClientX: e.clientX,
                    initialSidebarWidthPx: sidebarWidthPx,
                    initialRightSidebarWidthPx: rightSidebarWidthPx,
                  })
                }}
              />
            </div>

            <div className="flex-1 min-h-0 min-w-0 flex">
              <ChatPanel
                pendingDiffFile={pendingDiffFile}
                onDiffFileOpened={() => setPendingDiffFile(null)}
              />
            </div>

            {rightSidebarOpen ? (
              <>
                <div className="relative w-0 flex-shrink-0">
                  <div
                    data-testid="terminal-resizer"
                    className="absolute -left-1 top-0 h-full w-2 bg-transparent hover:bg-border/60 active:bg-border cursor-col-resize"
                    title="Resize terminal"
                    onPointerDown={(e) => {
                      if (e.button !== 0) return
                      e.preventDefault()
                      startResize({
                        edge: "right",
                        pointerDownClientX: e.clientX,
                        initialSidebarWidthPx: sidebarWidthPx,
                        initialRightSidebarWidthPx: rightSidebarWidthPx,
                      })
                    }}
                  />
                </div>
                <RightSidebar
                  isOpen={rightSidebarOpen}
                  onToggle={() => setRightSidebarOpen(!rightSidebarOpen)}
                  widthPx={rightSidebarWidthPx}
                  onOpenDiffTab={(file) => setPendingDiffFile(file)}
                />
              </>
            ) : (
              <RightSidebar
                isOpen={rightSidebarOpen}
                onToggle={() => setRightSidebarOpen(!rightSidebarOpen)}
                widthPx={rightSidebarWidthPx}
                onOpenDiffTab={(file) => setPendingDiffFile(file)}
              />
            )}
          </>
        ) : (
          <KanbanBoard onViewModeChange={setViewMode} />
        )}
      </div>
    </div>
  )
}
