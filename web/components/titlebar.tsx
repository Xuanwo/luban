"use client"

import { Minus, Square, X } from "lucide-react"
import { useEffect, useMemo, useState } from "react"
import { cn } from "@/lib/utils"

type Platform = "macos" | "windows" | "linux" | "unknown"

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
}

function platformFromNavigator(): Platform {
  if (typeof navigator === "undefined") return "unknown"
  const ua = navigator.userAgent.toLowerCase()
  const plat = (navigator.platform ?? "").toLowerCase()
  if (ua.includes("mac") || plat.includes("mac")) return "macos"
  if (ua.includes("win") || plat.includes("win")) return "windows"
  if (ua.includes("linux") || plat.includes("linux")) return "linux"
  return "unknown"
}

export function Titlebar() {
  const enabled = useMemo(() => isTauriRuntime(), [])
  const [platform] = useState<Platform>(() => (enabled ? platformFromNavigator() : "unknown"))
  const [isMaximized, setIsMaximized] = useState(false)

  useEffect(() => {
    if (!enabled) return

    let unsubMax: (() => void) | null = null
    let alive = true

    void (async () => {
      try {
        const w = await import("@tauri-apps/api/window")
        const win = w.getCurrentWindow()
        const current = await win.isMaximized()
        if (!alive) return
        setIsMaximized(current)

        unsubMax = await win.onResized(async () => {
          try {
            const v = await win.isMaximized()
            if (alive) setIsMaximized(v)
          } catch {
            // ignore
          }
        })
      } catch {
        // ignore
      }
    })()

    return () => {
      alive = false
      unsubMax?.()
    }
  }, [enabled])

  if (!enabled) return null

  async function minimize() {
    const w = await import("@tauri-apps/api/window")
    await w.getCurrentWindow().minimize()
  }

  async function toggleMaximize() {
    const w = await import("@tauri-apps/api/window")
    const win = w.getCurrentWindow()
    await win.toggleMaximize()
    try {
      const v = await win.isMaximized()
      setIsMaximized(v)
    } catch {
      // ignore
    }
  }

  async function close() {
    const w = await import("@tauri-apps/api/window")
    await w.getCurrentWindow().close()
  }

  const containerClass = cn(
    "relative h-10 flex items-stretch select-none",
    "bg-background text-foreground border-b border-border",
  )

  return (
    <div className={containerClass}>
      {/* Full-surface drag region behind interactive controls */}
      <div className="absolute inset-0" data-tauri-drag-region />

      <div className="relative z-10 flex items-stretch flex-1 min-w-0">
        {/* macOS uses native traffic lights (TitleBarStyle::Overlay). Reserve space so the title doesn't overlap. */}
        {platform === "macos" ? <div className="w-[84px] flex-shrink-0" /> : null}

        <div
          className={cn("flex-1 flex items-center min-w-0 px-3", "text-sm font-medium")}
          onDoubleClick={() => {
            if (platform === "macos") return
            void toggleMaximize()
          }}
        >
          <span className="truncate">Luban</span>
        </div>

        {platform === "macos" ? null : (
          <div className="flex items-stretch">
            <button
              aria-label="Minimize window"
              className="w-11 flex items-center justify-center hover:bg-muted/60 transition-colors"
              onClick={() => void minimize()}
              title="Minimize"
            >
              <Minus className="w-4 h-4" />
            </button>
            <button
              aria-label={isMaximized ? "Restore window" : "Maximize window"}
              className="w-11 flex items-center justify-center hover:bg-muted/60 transition-colors"
              onClick={() => void toggleMaximize()}
              title={isMaximized ? "Restore" : "Maximize"}
            >
              <Square className={cn("w-4 h-4", isMaximized && "scale-90")} />
            </button>
            <button
              aria-label="Close window"
              className="w-11 flex items-center justify-center hover:bg-destructive hover:text-destructive-foreground transition-colors"
              onClick={() => void close()}
              title="Close"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
