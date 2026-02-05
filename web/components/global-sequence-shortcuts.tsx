"use client"

import { useEffect, useRef } from "react"

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  const tag = target.tagName.toLowerCase()
  if (tag === "input" || tag === "textarea" || tag === "select") return true
  return target.isContentEditable
}

function isSuppressedTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.closest(".luban-terminal")) return true
  if (target.closest('[data-shortcuts-disabled="true"]')) return true
  return false
}

function normalizeSequenceKey(raw: string): string | null {
  if (raw.length !== 1) return null
  const upper = raw.toUpperCase()
  if (upper >= "A" && upper <= "Z") return upper
  if (raw >= "0" && raw <= "9") return raw
  return null
}

type TaskListMode = "all" | "active" | "backlog"

export function GlobalSequenceShortcuts({
  enabled,
  canGoProjectModes,
  canOpenStatusPicker,
  onNewTask,
  onGoInbox,
  onSetTaskListMode,
  onOpenStatusPicker,
}: {
  enabled: boolean
  canGoProjectModes: boolean
  canOpenStatusPicker: boolean
  onNewTask: () => void
  onGoInbox: () => void
  onSetTaskListMode: (mode: TaskListMode) => void
  onOpenStatusPicker: () => void
}) {
  const goPrefixTimeoutRef = useRef<number | null>(null)
  const pendingGoRef = useRef(false)

  useEffect(() => {
    const clearGoPrefix = () => {
      pendingGoRef.current = false
      if (goPrefixTimeoutRef.current != null) {
        window.clearTimeout(goPrefixTimeoutRef.current)
        goPrefixTimeoutRef.current = null
      }
    }

    const armGoPrefix = () => {
      pendingGoRef.current = true
      if (goPrefixTimeoutRef.current != null) window.clearTimeout(goPrefixTimeoutRef.current)
      goPrefixTimeoutRef.current = window.setTimeout(() => clearGoPrefix(), 900)
    }

    const handler = (e: KeyboardEvent) => {
      if (!enabled) return
      if (e.defaultPrevented) return
      if (e.ctrlKey || e.metaKey || e.altKey) return
      if (e.repeat) return
      if (isEditableTarget(e.target)) return
      if (isSuppressedTarget(e.target)) return

      const key = normalizeSequenceKey(e.key)

      if (!key) {
        if (pendingGoRef.current) clearGoPrefix()
        return
      }

      if (pendingGoRef.current) {
        if (key === "I") {
          e.preventDefault()
          e.stopPropagation()
          clearGoPrefix()
          onGoInbox()
          return
        }
        if (canGoProjectModes) {
          if (key === "E") {
            e.preventDefault()
            e.stopPropagation()
            clearGoPrefix()
            onSetTaskListMode("all")
            return
          }
          if (key === "A") {
            e.preventDefault()
            e.stopPropagation()
            clearGoPrefix()
            onSetTaskListMode("active")
            return
          }
          if (key === "B") {
            e.preventDefault()
            e.stopPropagation()
            clearGoPrefix()
            onSetTaskListMode("backlog")
            return
          }
        }
        clearGoPrefix()
        return
      }

      if (key === "G") {
        e.preventDefault()
        e.stopPropagation()
        armGoPrefix()
        return
      }

      if (key === "C") {
        e.preventDefault()
        e.stopPropagation()
        onNewTask()
        return
      }

      if (key === "S" && canOpenStatusPicker) {
        e.preventDefault()
        e.stopPropagation()
        onOpenStatusPicker()
        return
      }
    }

    window.addEventListener("keydown", handler, { capture: true })
    return () => {
      window.removeEventListener("keydown", handler, { capture: true } as AddEventListenerOptions)
      clearGoPrefix()
    }
  }, [canGoProjectModes, canOpenStatusPicker, enabled, onGoInbox, onNewTask, onOpenStatusPicker, onSetTaskListMode])

  return null
}

