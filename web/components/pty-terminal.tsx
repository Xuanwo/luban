"use client"

import { useEffect, useRef } from "react"
import { FitAddon, Terminal } from "ghostty-web"
import type { ITheme } from "ghostty-web"

import { useLuban } from "@/lib/luban-context"

function cssVar(name: string): string | null {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return raw.length > 0 ? raw : null
}

function normalizeHexColor(raw: string): string | null {
  const hex = raw.trim()
  if (!hex.startsWith("#")) return null
  const h = hex.slice(1)
  if (h.length === 3) {
    return `#${h[0]}${h[0]}${h[1]}${h[1]}${h[2]}${h[2]}`
  }
  if (h.length === 6) return `#${h}`
  return null
}

function parseRgbColor(raw: string): { r: number; g: number; b: number } | null {
  const trimmed = raw.trim()
  const m = /^rgb\\(\\s*(\\d+)\\s*,\\s*(\\d+)\\s*,\\s*(\\d+)\\s*\\)$/.exec(trimmed)
  if (!m) return null
  const r = Number.parseInt(m[1], 10)
  const g = Number.parseInt(m[2], 10)
  const b = Number.parseInt(m[3], 10)
  if (![r, g, b].every((v) => Number.isFinite(v) && v >= 0 && v <= 255)) return null
  return { r, g, b }
}

function toHexColor(raw: string): string | null {
  const hex = normalizeHexColor(raw)
  if (hex) return hex
  const rgb = parseRgbColor(raw)
  if (!rgb) return null
  const to2 = (v: number) => v.toString(16).padStart(2, "0")
  return `#${to2(rgb.r)}${to2(rgb.g)}${to2(rgb.b)}`
}

function hexToRgba(hex: string, alpha: number): string | null {
  const raw = hex.trim()
  if (!raw.startsWith("#")) return null
  const h = raw.slice(1)
  const normalized =
    h.length === 3
      ? `${h[0]}${h[0]}${h[1]}${h[1]}${h[2]}${h[2]}`
      : h.length === 6
        ? h
        : null
  if (!normalized) return null
  const r = Number.parseInt(normalized.slice(0, 2), 16)
  const g = Number.parseInt(normalized.slice(2, 4), 16)
  const b = Number.parseInt(normalized.slice(4, 6), 16)
  if (!Number.isFinite(r) || !Number.isFinite(g) || !Number.isFinite(b)) return null
  const a = Math.max(0, Math.min(1, alpha))
  return `rgba(${r}, ${g}, ${b}, ${a})`
}

function terminalThemeFromCss(): ITheme {
  const background = cssVar("--card") ?? "#ffffff"
  const foreground = cssVar("--card-foreground") ?? "#333333"
  const cursor = cssVar("--foreground") ?? foreground
  const primary = cssVar("--primary") ?? "#3b82f6"
  const mutedForeground = cssVar("--muted-foreground") ?? "#6b7280"
  const selectionBackground = hexToRgba(toHexColor(primary) ?? primary, 0.25) ?? "rgba(59, 130, 246, 0.25)"
  return {
    background,
    foreground,
    cursor,
    selectionBackground,
    black: background,
    brightBlack: mutedForeground,
    white: foreground,
    brightWhite: foreground,
  }
}

function writeOscTheme(term: Terminal) {
  const background = toHexColor(cssVar("--card") ?? "#ffffff") ?? "#ffffff"
  const foreground = toHexColor(cssVar("--card-foreground") ?? "#333333") ?? "#333333"
  const cursor = toHexColor(cssVar("--foreground") ?? foreground) ?? foreground

  // ghostty-web renders fg/bg from the emulator's resolved RGB values.
  // Emit OSC sequences to update defaults in the emulator itself.
  const osc =
    `\x1b]10;${foreground}\x07` + // default foreground
    `\x1b]11;${background}\x07` + // default background
    `\x1b]12;${cursor}\x07` // cursor
  term.write(osc)
}

function isValidTerminalSize(cols: number, rows: number): boolean {
  return Number.isFinite(cols) && Number.isFinite(rows) && cols >= 2 && rows >= 2
}

export function PtyTerminal() {
  const { activeWorkspaceId } = useLuban()
  const containerRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    if (activeWorkspaceId == null) {
      container.textContent = "Select a workspace to start a terminal."
      return
    }

    container.innerHTML = ""

    const ptyThreadId = 1

    let disposed = false
    const fitAddon = new FitAddon()
    const term = new Terminal({
      fontFamily:
        "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace",
      fontSize: 12,
      wasmPath: "/ghostty-vt.wasm",
      cursorBlink: true,
      theme: terminalThemeFromCss(),
    })

    term.loadAddon(fitAddon)

    const decoder = new TextDecoder("utf-8")
    let ws: WebSocket | null = null
    let dataDisposable: { dispose: () => void } | null = null
    let resizeDisposable: { dispose: () => void } | null = null

    function sendResizeIfReady(cols: number, rows: number) {
      if (!isValidTerminalSize(cols, rows)) return
      if (ws?.readyState !== WebSocket.OPEN) return
      ws.send(JSON.stringify({ type: "resize", cols, rows }))
    }

    function scheduleFitAndResizeSync() {
      let attempts = 0
      const maxAttempts = 20
      const tick = () => {
        if (disposed) return
        try {
          fitAddon.fit()
        } catch {
          // ignore
        }
        sendResizeIfReady(term.cols, term.rows)
        attempts += 1
        if (!isValidTerminalSize(term.cols, term.rows) && attempts < maxAttempts) {
          window.setTimeout(tick, 50)
        }
      }
      window.requestAnimationFrame(tick)
    }

    void term
      .open(container)
      .then(() => {
        if (disposed) return

        writeOscTheme(term)

        resizeDisposable = term.onResize(({ cols, rows }) => {
          sendResizeIfReady(cols, rows)
        })

        scheduleFitAndResizeSync()
        fitAddon.observeResize()

        const url = new URL(`/api/pty/${activeWorkspaceId}/${ptyThreadId}`, window.location.href)
        url.protocol = url.protocol === "https:" ? "wss:" : "ws:"
        ws = new WebSocket(url.toString())
        ws.binaryType = "arraybuffer"

        ws.onmessage = (ev) => {
          if (disposed) return
          if (typeof ev.data === "string") return
          const bytes = new Uint8Array(ev.data as ArrayBuffer)
          term.write(decoder.decode(bytes))
        }

        ws.onopen = () => {
          if (disposed) return
          scheduleFitAndResizeSync()
        }

        dataDisposable = term.onData((data: string) => {
          if (ws?.readyState !== WebSocket.OPEN) return
          ws.send(JSON.stringify({ type: "input", data }))
        })
      })
      .catch((err) => {
        if (disposed) return
        container.textContent = `Terminal init failed: ${String(err)}`
      })

    return () => {
      disposed = true
      dataDisposable?.dispose()
      resizeDisposable?.dispose()
      ws?.close()
      term.dispose()
    }
  }, [activeWorkspaceId])

  return (
    <div
      ref={containerRef}
      data-testid="pty-terminal"
      className="h-full w-full p-3 font-mono text-xs overflow-auto bg-card text-card-foreground"
    />
  )
}
