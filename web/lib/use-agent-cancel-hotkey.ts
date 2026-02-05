import { useCallback, useEffect, useMemo, useRef, useState } from "react"

type KeydownTarget = {
  tagName?: string
  isContentEditable?: boolean
}

function isTextInputTarget(target: unknown): boolean {
  const el = target as KeydownTarget | null
  if (!el) return false
  const tag = String(el.tagName ?? "").toUpperCase()
  return tag === "INPUT" || tag === "TEXTAREA" || Boolean(el.isContentEditable)
}

export function useAgentCancelHotkey(args: {
  enabled: boolean
  blocked: boolean
  onCancel: () => void
  timeoutMs?: number
}): { escHintVisible: boolean; escTimeoutMs: number; clearEscHint: () => void } {
  const escTimeoutMs = args.timeoutMs ?? 2000
  const [escHintVisible, setEscHintVisible] = useState(false)
  const escHintVisibleRef = useRef(false)
  const escTimeoutRef = useRef<number | null>(null)

  useEffect(() => {
    escHintVisibleRef.current = escHintVisible
  }, [escHintVisible])

  const clearEscHint = useCallback(() => {
    setEscHintVisible(false)
    if (escTimeoutRef.current != null) {
      window.clearTimeout(escTimeoutRef.current)
      escTimeoutRef.current = null
    }
  }, [])

  const onCancel = useMemo(() => args.onCancel, [args.onCancel])

  useEffect(() => {
    if (!args.enabled) {
      clearEscHint()
      return
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return
      if (!args.enabled) return
      if (args.blocked) return
      if (e.defaultPrevented) return
      if (isTextInputTarget(e.target)) return

      if (escHintVisibleRef.current) {
        e.preventDefault()
        clearEscHint()
        onCancel()
        return
      }

      e.preventDefault()

      setEscHintVisible(true)
      if (escTimeoutRef.current != null) {
        window.clearTimeout(escTimeoutRef.current)
      }
      escTimeoutRef.current = window.setTimeout(() => {
        setEscHintVisible(false)
        escTimeoutRef.current = null
      }, escTimeoutMs)
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => {
      window.removeEventListener("keydown", handleKeyDown)
      if (escTimeoutRef.current != null) {
        window.clearTimeout(escTimeoutRef.current)
        escTimeoutRef.current = null
      }
    }
  }, [args.blocked, args.enabled, clearEscHint, escTimeoutMs, onCancel])

  return { escHintVisible, escTimeoutMs, clearEscHint }
}
