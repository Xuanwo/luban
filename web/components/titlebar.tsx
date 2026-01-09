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
    "h-10 flex items-stretch select-none",
    "bg-background text-foreground border-b border-border",
  )

  const dragRegionClass = cn(
    "flex-1 flex items-center min-w-0 px-3",
    "text-sm font-medium",
  )

  return (
    <div className={containerClass}>
      {platform === "macos" ? (
        <div className="flex items-center gap-2 px-3">
          <button
            aria-label="Close window"
            className="w-3.5 h-3.5 rounded-full bg-red-500 hover:bg-red-400 transition-colors"
            onClick={() => void close()}
            title="Close"
          />
          <button
            aria-label="Minimize window"
            className="w-3.5 h-3.5 rounded-full bg-amber-500 hover:bg-amber-400 transition-colors"
            onClick={() => void minimize()}
            title="Minimize"
          />
          <button
            aria-label={isMaximized ? "Restore window" : "Maximize window"}
            className="w-3.5 h-3.5 rounded-full bg-green-500 hover:bg-green-400 transition-colors"
            onClick={() => void toggleMaximize()}
            title={isMaximized ? "Restore" : "Maximize"}
          />
        </div>
      ) : null}

      <div
        className={dragRegionClass}
        data-tauri-drag-region
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
  )
}
