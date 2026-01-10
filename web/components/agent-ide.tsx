"use client"

import { useEffect, useState } from "react"
import { Sidebar } from "./sidebar"
import { ChatPanel } from "./chat-panel"
import { RightSidebar } from "./right-sidebar"

const RIGHT_SIDEBAR_OPEN_KEY = "luban:ui:right_sidebar_open"
const VIEW_MODE_KEY = "luban:ui:view_mode"
const SIDEBAR_WIDTH_KEY = "luban:ui:sidebar_width_px"
const RIGHT_SIDEBAR_WIDTH_KEY = "luban:ui:right_sidebar_width_px"

function clamp(n: number, min: number, max: number) {
  return Math.max(min, Math.min(max, n))
}

export function AgentIDE() {
  const [rightSidebarOpen, setRightSidebarOpen] = useState(true)
  const [viewMode, setViewMode] = useState<"workspace" | "kanban">("workspace")
  const [sidebarWidthPx, setSidebarWidthPx] = useState(240)
  const [rightSidebarWidthPx, setRightSidebarWidthPx] = useState(320)

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
      <div className="relative flex flex-1 overflow-hidden">
        {/* Left Sidebar */}
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

        {/* Middle - Chat Panel */}
        <div className="flex-1 min-w-0 flex">
          <ChatPanel />
        </div>

        {/* Right Sidebar - collapsed state renders as absolute button */}
        {rightSidebarOpen ? (
          <>
            {/* Use a zero-width wrapper so borders remain continuous across panes. */}
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
            />
          </>
        ) : (
          <RightSidebar
            isOpen={rightSidebarOpen}
            onToggle={() => setRightSidebarOpen(!rightSidebarOpen)}
            widthPx={rightSidebarWidthPx}
          />
        )}
      </div>
    </div>
  )
}
