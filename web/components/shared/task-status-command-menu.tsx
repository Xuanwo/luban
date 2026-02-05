"use client"

import { useEffect, useMemo, useRef } from "react"
import { createPortal } from "react-dom"
import { Check } from "lucide-react"

import type { TaskStatus } from "@/lib/luban-api"
import { cn } from "@/lib/utils"
import { taskStatuses } from "@/components/shared/task-status-selector"

export type AnchorRect = {
  top: number
  left: number
  width: number
  height: number
}

function clampPosition(args: { top: number; left: number; width: number; height: number }): { top: number; left: number } {
  const margin = 8
  const maxLeft = Math.max(margin, window.innerWidth - args.width - margin)
  const maxTop = Math.max(margin, window.innerHeight - args.height - margin)
  return {
    left: Math.min(Math.max(margin, args.left), maxLeft),
    top: Math.min(Math.max(margin, args.top), maxTop),
  }
}

export function TaskStatusCommandMenu({
  open,
  anchorRect,
  status,
  onSelect,
  onClose,
  testId = "task-status-command-menu",
}: {
  open: boolean
  anchorRect: AnchorRect | null
  status: TaskStatus
  onSelect: (next: TaskStatus) => void
  onClose: () => void
  testId?: string
}) {
  const containerRef = useRef<HTMLDivElement | null>(null)

  const numbersByStatus = useMemo(() => {
    const out = new Map<TaskStatus, string>()
    for (let i = 0; i < taskStatuses.length; i += 1) {
      const s = taskStatuses[i]
      if (!s) continue
      out.set(s.id, String(i + 1))
    }
    return out
  }, [])

  const position = useMemo(() => {
    if (!open || !anchorRect) return null
    const width = 320
    const height = 320

    const desiredTop = anchorRect.top + anchorRect.height + 8
    const desiredLeft = anchorRect.left

    const clamped = clampPosition({ top: desiredTop, left: desiredLeft, width, height })
    return { ...clamped, width, height }
  }, [anchorRect, open])

  useEffect(() => {
    if (!open) return
    const el = containerRef.current
    if (!el) return
    el.focus()
  }, [open])

  useEffect(() => {
    if (!open) return

    const onPointerDown = (e: PointerEvent) => {
      const el = containerRef.current
      if (!el) return
      if (e.target instanceof Node && el.contains(e.target)) return
      onClose()
    }

    const onScroll = () => onClose()
    const onResize = () => onClose()

    window.addEventListener("pointerdown", onPointerDown, { capture: true })
    window.addEventListener("scroll", onScroll, { capture: true })
    window.addEventListener("resize", onResize)
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, { capture: true } as AddEventListenerOptions)
      window.removeEventListener("scroll", onScroll, { capture: true } as AddEventListenerOptions)
      window.removeEventListener("resize", onResize)
    }
  }, [onClose, open])

  if (!open || !anchorRect || !position) return null

  const content = (
    <div
      ref={containerRef}
      data-testid={testId}
      data-shortcuts-disabled="true"
      role="menu"
      tabIndex={-1}
      className="fixed z-50 rounded-lg border bg-white shadow-[0_4px_16px_rgba(0,0,0,0.12)] p-1.5 outline-none"
      style={{
        top: position.top,
        left: position.left,
        width: position.width,
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault()
          e.stopPropagation()
          onClose()
          return
        }

        if (e.key.length === 1 && e.key >= "1" && e.key <= "9") {
          const idx = Number.parseInt(e.key, 10) - 1
          const s = taskStatuses[idx]
          if (!s) return
          e.preventDefault()
          e.stopPropagation()
          if (s.id !== status) onSelect(s.id)
          onClose()
        }
      }}
    >
      <div className="px-2 py-1.5 text-xs font-medium" style={{ color: "#6b6b6b" }}>
        Change status...
      </div>
      {taskStatuses.map((s) => {
        const isSelected = s.id === status
        const number = numbersByStatus.get(s.id) ?? ""
        return (
          <button
            key={s.id}
            type="button"
            role="menuitem"
            data-testid={`task-status-command-option-${s.id}`}
            className={cn(
              "w-full flex items-center gap-2 px-2 py-1.5 rounded-md transition-colors",
              "hover:bg-[#f5f5f5]",
              isSelected && "bg-[#f0f0f0]",
            )}
            onClick={() => {
              if (s.id !== status) onSelect(s.id)
              onClose()
            }}
          >
            <span className={cn(s.iconClass, "w-[14px] h-[14px] flex-shrink-0")} style={{ color: s.color }} />
            <span className="flex-1 text-[13px]" style={{ color: "#1b1b1b" }}>
              {s.label}
            </span>
            <span className="flex items-center gap-2">
              {number ? (
                <span
                  className="inline-flex items-center justify-center rounded-[6px] border px-1.5 py-0.5 text-[11px] font-medium"
                  style={{ backgroundColor: "#fcfcfc", borderColor: "#ebebeb", color: "#6b6b6b" }}
                  aria-hidden="true"
                >
                  {number}
                </span>
              ) : null}
              {isSelected ? <Check className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "#9b9b9b" }} /> : null}
            </span>
          </button>
        )
      })}
    </div>
  )

  return createPortal(content, document.body)
}

