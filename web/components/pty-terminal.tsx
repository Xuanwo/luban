"use client"

import { useEffect, useRef } from "react"
import { Terminal, type ITheme } from "@xterm/xterm"
import { FitAddon } from "@xterm/addon-fit"
import { WebglAddon } from "@xterm/addon-webgl"
import { useTheme } from "next-themes"

import { useLuban } from "@/lib/luban-context"
import { useAppearance } from "@/components/appearance-provider"
import { isMockMode } from "@/lib/luban-mode"

function escapeCssFontName(value: string): string {
  return value.replaceAll('"', '\\"')
}

function encodeBinaryString(value: string): Uint8Array {
  const out = new Uint8Array(value.length)
  for (let i = 0; i < value.length; i++) out[i] = value.charCodeAt(i) & 0xff
  return out
}

function encodePtyInputText(encoder: TextEncoder, text: string): Uint8Array {
  if (text.length === 0) return new Uint8Array()

  if (!/[\u0080-\u009f]/.test(text)) {
    return encoder.encode(text)
  }

  const bytes: number[] = []
  for (const ch of text) {
    const codePoint = ch.codePointAt(0)
    if (codePoint != null && codePoint >= 0x80 && codePoint <= 0x9f) {
      bytes.push(codePoint)
      continue
    }
    const encoded = encoder.encode(ch)
    for (let i = 0; i < encoded.length; i++) bytes.push(encoded[i] ?? 0)
  }
  return Uint8Array.from(bytes)
}

function terminalFontFamily(fontName: string): string {
  const escaped = escapeCssFontName(fontName.trim() || "Geist Mono")
  return `"${escaped}", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace`
}

async function copyToClipboard(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    const el = document.createElement("textarea")
    el.value = text
    el.style.position = "fixed"
    el.style.left = "-9999px"
    el.style.top = "-9999px"
    document.body.appendChild(el)
    el.focus()
    el.select()
    el.setSelectionRange(0, text.length)
    document.execCommand("copy")
    document.body.removeChild(el)
  }
}

function cssVar(scope: Element, name: string): string | null {
  const raw = getComputedStyle(scope).getPropertyValue(name).trim()
  return raw.length > 0 ? raw : null
}

function parseHexColor(raw: string): { r: number; g: number; b: number } | null {
  const normalized = normalizeHexColor(raw)
  if (!normalized) return null
  const h = normalized.slice(1)
  const r = Number.parseInt(h.slice(0, 2), 16)
  const g = Number.parseInt(h.slice(2, 4), 16)
  const b = Number.parseInt(h.slice(4, 6), 16)
  if (![r, g, b].every((v) => Number.isFinite(v) && v >= 0 && v <= 255)) return null
  return { r, g, b }
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

function parseHslTriple(raw: string): { h: number; s: number; l: number } | null {
  const parts = raw.trim().split(/\s+/)
  if (parts.length < 3) return null
  const h = Number.parseFloat(parts[0] ?? "")
  const sStr = parts[1] ?? ""
  const lStr = parts[2] ?? ""
  if (!sStr.endsWith("%") || !lStr.endsWith("%")) return null
  const s = Number.parseFloat(sStr.slice(0, -1))
  const l = Number.parseFloat(lStr.slice(0, -1))
  if (![h, s, l].every((v) => Number.isFinite(v))) return null
  if (s < 0 || s > 100 || l < 0 || l > 100) return null
  return { h, s, l }
}

function hslToRgb(h: number, s: number, l: number): { r: number; g: number; b: number } {
  const hh = ((h % 360) + 360) % 360
  const ss = Math.max(0, Math.min(1, s / 100))
  const ll = Math.max(0, Math.min(1, l / 100))
  const c = (1 - Math.abs(2 * ll - 1)) * ss
  const x = c * (1 - Math.abs(((hh / 60) % 2) - 1))
  const m = ll - c / 2

  const [rp, gp, bp] =
    hh < 60
      ? [c, x, 0]
      : hh < 120
        ? [x, c, 0]
        : hh < 180
          ? [0, c, x]
          : hh < 240
            ? [0, x, c]
            : hh < 300
              ? [x, 0, c]
              : [c, 0, x]

  const to255 = (v: number) => Math.round((v + m) * 255)
  return { r: to255(rp), g: to255(gp), b: to255(bp) }
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

function resolveCssColor(scope: Element, name: string, fallback: { r: number; g: number; b: number }): { r: number; g: number; b: number } {
  const raw = cssVar(scope, name)
  if (!raw) return fallback

  const hex = parseHexColor(raw)
  if (hex) return hex
  const rgb = parseRgbColor(raw)
  if (rgb) return rgb

  const hsl = parseHslTriple(raw)
  if (hsl) return hslToRgb(hsl.h, hsl.s, hsl.l)

  return fallback
}

function rgbToCss(rgb: { r: number; g: number; b: number }): string {
  return `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`
}

function rgbToHex(rgb: { r: number; g: number; b: number }): string {
  const to2 = (v: number) => Math.max(0, Math.min(255, v)).toString(16).padStart(2, "0")
  return `#${to2(rgb.r)}${to2(rgb.g)}${to2(rgb.b)}`
}

function rgbToRgbaCss(rgb: { r: number; g: number; b: number }, alpha: number): string {
  const a = Math.max(0, Math.min(1, alpha))
  return `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${a})`
}

function terminalThemeFromCss(scope: Element): ITheme {
  const backgroundRgb = resolveCssColor(scope, "--card", { r: 255, g: 255, b: 255 })
  const foregroundRgb = resolveCssColor(scope, "--card-foreground", { r: 51, g: 51, b: 51 })
  const cursorRgb = resolveCssColor(scope, "--foreground", foregroundRgb)
  const primaryRgb = resolveCssColor(scope, "--primary", { r: 59, g: 130, b: 246 })
  const mutedForegroundRgb = resolveCssColor(scope, "--muted-foreground", { r: 107, g: 114, b: 128 })

  const background = rgbToHex(backgroundRgb)
  const foreground = rgbToHex(foregroundRgb)
  const cursor = rgbToHex(cursorRgb)
  const mutedForeground = rgbToHex(mutedForegroundRgb)
  const selectionBackground = rgbToRgbaCss(primaryRgb, 0.25)
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

function isValidTerminalSize(cols: number, rows: number): boolean {
  return Number.isFinite(cols) && Number.isFinite(rows) && cols >= 2 && rows >= 2
}

export function PtyTerminal() {
  const { activeWorkspaceId, activeWorkspace } = useLuban()
  const { fonts } = useAppearance()
  const { resolvedTheme } = useTheme()
  const outerRef = useRef<HTMLDivElement | null>(null)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const webglAddonRef = useRef<WebglAddon | null>(null)
  const lastThemeDigestRef = useRef<string | null>(null)

  useEffect(() => {
    const term = termRef.current
    if (!term) return
    term.options.fontFamily = terminalFontFamily(fonts.terminalFont)
    fitAddonRef.current?.fit()
  }, [fonts.terminalFont])

  useEffect(() => {
    const term = termRef.current
    if (!term) return
    const scope = outerRef.current
    if (!scope) return

    let cancelled = false
    window.requestAnimationFrame(() => {
      if (cancelled) return

      const theme = terminalThemeFromCss(scope)
      const digest = JSON.stringify(theme)
      if (lastThemeDigestRef.current === digest) return
      lastThemeDigestRef.current = digest

      term.options.theme = theme
      term.refresh(0, Math.max(0, term.rows - 1))
    })

    return () => {
      cancelled = true
    }
  }, [resolvedTheme])

  useEffect(() => {
    const outer = outerRef.current
    const container = containerRef.current
    if (!outer || !container) return

    const isMock = isMockMode()

    if (activeWorkspaceId == null) {
      container.textContent = "Select a workspace to start a terminal."
      return
    }

    container.innerHTML = ""

    const ptyThreadId = 1
    const mockWorkspaceLabel = activeWorkspace?.workspace_name ?? `workspace-${activeWorkspaceId}`
    const mockCwd = activeWorkspace?.worktree_path ?? `/mock/workspaces/${activeWorkspaceId}`

    let disposed = false
    const fitAddon = new FitAddon()
    const webglAddon = new WebglAddon()
    fitAddonRef.current = fitAddon
    webglAddonRef.current = webglAddon

    const term = new Terminal({
      fontFamily: terminalFontFamily(fonts.terminalFont),
      fontSize: 12,
      cursorBlink: true,
      allowTransparency: true,
      theme: terminalThemeFromCss(outer),
      scrollback: 5000,
    })
    termRef.current = term

    term.loadAddon(fitAddon)
    try {
      term.loadAddon(webglAddon)
    } catch {
      // Ignore WebGL initialization failures (unsupported GPU context).
    }

    const encoder = new TextEncoder()
    let ws: WebSocket | null = null
    let dataDisposable: { dispose: () => void } | null = null
    let binaryDisposable: { dispose: () => void } | null = null
    let resizeDisposable: { dispose: () => void } | null = null
    let resizeObserver: ResizeObserver | null = null
    let keydownCapture: ((ev: KeyboardEvent) => void) | null = null
    let pasteCapture: ((ev: ClipboardEvent) => void) | null = null
    let focusCapture: (() => void) | null = null
    let pendingPastePromise: Promise<string> | null = null
    let pasteHandled = false
    let pendingInput: Uint8Array[] = []
    let pendingReconnectTimer: number | null = null

    function sendInput(text: string) {
      if (isMock) {
        mockHandleInputText(text)
        return
      }
      const socket = ws
      if (!socket) return
      const bytes = encodePtyInputText(encoder, text)
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(bytes)
      } else {
        pendingInput.push(bytes)
      }
    }

    function sendBinaryInput(value: string) {
      if (isMock) {
        mockHandleInputBinary(value)
        return
      }
      const socket = ws
      if (!socket) return
      const bytes = encodeBinaryString(value)
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(bytes)
      } else {
        pendingInput.push(bytes)
      }
    }

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

    try {
      term.open(container)
    } catch (err) {
      container.textContent = `Terminal init failed: ${String(err)}`
      return
    }

    let mockLine = ""
    let mockShownBanner = false

    function writeMockPrompt() {
      term.write(`${mockWorkspaceLabel} $ `)
    }

    function writeMockBannerIfNeeded() {
      if (mockShownBanner) return
      mockShownBanner = true
      term.writeln("Mock PTY (local) - not connected to server")
      term.writeln(`Workspace: ${mockCwd}`)
      writeMockPrompt()
    }

    function mockWriteOutput(lines: string[]) {
      for (const line of lines) term.writeln(line)
    }

    function mockRunCommand(commandLine: string): void {
      const trimmed = commandLine.trim()
      if (trimmed.length === 0) {
        writeMockPrompt()
        return
      }

      const [cmd, ...rest] = trimmed.split(/\s+/)
      const arg = rest.join(" ")

      if (cmd === "help") {
        mockWriteOutput([
          "Commands:",
          "  help",
          "  pwd",
          "  ls",
          "  echo <text>",
          "  date",
          "  clear",
        ])
        writeMockPrompt()
        return
      }

      if (cmd === "pwd") {
        mockWriteOutput([mockCwd])
        writeMockPrompt()
        return
      }

      if (cmd === "ls") {
        mockWriteOutput(["crates/", "docs/", "web/"])
        writeMockPrompt()
        return
      }

      if (cmd === "echo") {
        mockWriteOutput([arg])
        writeMockPrompt()
        return
      }

      if (cmd === "date") {
        mockWriteOutput([new Date().toISOString()])
        writeMockPrompt()
        return
      }

      if (cmd === "clear") {
        term.clear()
        writeMockPrompt()
        return
      }

      mockWriteOutput([`mock: command not found: ${cmd}`])
      writeMockPrompt()
    }

    function mockHandleInputText(text: string): void {
      writeMockBannerIfNeeded()

      for (const ch of text) {
        const code = ch.charCodeAt(0)

        if (ch === "\r" || ch === "\n") {
          term.write("\r\n")
          const line = mockLine
          mockLine = ""
          mockRunCommand(line)
          continue
        }

        if (code === 0x7f) {
          if (mockLine.length > 0) {
            mockLine = mockLine.slice(0, -1)
            term.write("\b \b")
          }
          continue
        }

        if (code === 0x03) {
          term.write("^C\r\n")
          mockLine = ""
          writeMockPrompt()
          continue
        }

        if (code === 0x0c) {
          term.clear()
          mockLine = ""
          writeMockPrompt()
          continue
        }

        if (code < 0x20) continue

        mockLine += ch
        term.write(ch)
      }
    }

    function mockHandleInputBinary(value: string): void {
      mockHandleInputText(value)
    }

    // The terminal is often initialized before theme variables settle (e.g. next-themes
    // hydration). Re-apply once after mount so the renderer picks up the correct background.
    window.requestAnimationFrame(() => {
      if (disposed) return
      const theme = terminalThemeFromCss(outer)
      const digest = JSON.stringify(theme)
      lastThemeDigestRef.current = digest
      term.options.theme = theme
      term.refresh(0, Math.max(0, term.rows - 1))
    })

    focusCapture = () => {
      try {
        container.focus({ preventScroll: true })
        term.focus()
      } catch {
        // ignore
      }
    }

    keydownCapture = (ev: KeyboardEvent) => {
      if (ev.key === "Backspace" && !ev.altKey && !ev.ctrlKey && !ev.metaKey) {
        ev.preventDefault()
        ev.stopPropagation()
        ev.stopImmediatePropagation()
        sendBinaryInput("\x7f")
        return
      }

      const isShortcut = ev.ctrlKey || ev.metaKey
      if (!isShortcut) return

      const isMac = navigator.platform.toLowerCase().includes("mac")

      if (ev.key === "ArrowLeft" || ev.key === "ArrowRight") {
        const seq = (() => {
          if (isMac && ev.metaKey) {
            return ev.key === "ArrowLeft" ? "\x01" : "\x05"
          }
          if (ev.ctrlKey) {
            return ev.key === "ArrowLeft" ? "\x1bb" : "\x1bf"
          }
          return null
        })()

        if (seq) {
          ev.preventDefault()
          ev.stopPropagation()
          ev.stopImmediatePropagation()
          sendInput(seq)
          return
        }
      }

      if (ev.code === "KeyC") {
        if (!term.hasSelection()) return
        const selection = term.getSelection()
        if (selection.trim().length === 0) return

        ev.preventDefault()
        ev.stopPropagation()
        ev.stopImmediatePropagation()
        void copyToClipboard(selection)
        return
      }

      if (ev.code === "KeyV") {
        pasteHandled = false
        const promise = navigator.clipboard?.readText ? navigator.clipboard.readText() : null
        pendingPastePromise = promise
        if (!promise) return

        ev.preventDefault()
        ev.stopPropagation()
        ev.stopImmediatePropagation()

        queueMicrotask(() => {
          if (disposed) return
          if (pasteHandled) return

          void promise
            .then((text) => {
              if (disposed) return
              if (pasteHandled) return
              if (text.length === 0) return
              sendInput(text)
            })
            .catch(() => {
              // Ignore clipboard errors (permissions, etc.).
            })
            .finally(() => {
              if (pendingPastePromise === promise) pendingPastePromise = null
            })
        })
        return
      }
    }

    pasteCapture = (ev: ClipboardEvent) => {
      pasteHandled = true
      const text = ev.clipboardData?.getData("text/plain") ?? ""
      if (text.length === 0) return
      ev.preventDefault()
      ev.stopPropagation()
      ev.stopImmediatePropagation()
      sendInput(text)
    }

    outer.addEventListener("mousedown", focusCapture, true)
    outer.addEventListener("touchstart", focusCapture, true)
    outer.addEventListener("keydown", keydownCapture, true)
    outer.addEventListener("paste", pasteCapture, true)

    resizeDisposable = term.onResize(({ cols, rows }) => {
      sendResizeIfReady(cols, rows)
    })

    resizeObserver = new ResizeObserver(() => {
      scheduleFitAndResizeSync()
    })
    resizeObserver.observe(container)

    scheduleFitAndResizeSync()

    if (isMock) {
      writeMockBannerIfNeeded()
    } else {
      const connect = () => {
        const url = new URL(`/api/pty/${activeWorkspaceId}/${ptyThreadId}`, window.location.href)
        url.protocol = url.protocol === "https:" ? "wss:" : "ws:"

        const socket = new WebSocket(url.toString())
        socket.binaryType = "arraybuffer"
        ws = socket

        socket.onmessage = (ev) => {
          if (disposed) return
          if (typeof ev.data === "string") return
          const bytes = new Uint8Array(ev.data as ArrayBuffer)
          term.write(bytes)
        }

        socket.onopen = () => {
          if (disposed) return
          if (pendingInput.length > 0) {
            for (const bytes of pendingInput) socket.send(bytes)
            pendingInput = []
          }
          scheduleFitAndResizeSync()
        }

        socket.onclose = () => {
          if (disposed) return
          if (ws !== socket) return
          if (pendingReconnectTimer != null) return
          pendingReconnectTimer = window.setTimeout(() => {
            pendingReconnectTimer = null
            if (disposed) return
            connect()
          }, 150)
        }
      }

      connect()
    }

    dataDisposable = term.onData((data: string) => sendInput(data))
    binaryDisposable = term.onBinary((data: string) => sendBinaryInput(data))

    return () => {
      disposed = true
      termRef.current = null
      fitAddonRef.current = null
      webglAddonRef.current = null
      if (focusCapture) outer.removeEventListener("mousedown", focusCapture, true)
      if (focusCapture) outer.removeEventListener("touchstart", focusCapture, true)
      if (keydownCapture) outer.removeEventListener("keydown", keydownCapture, true)
      if (pasteCapture) outer.removeEventListener("paste", pasteCapture, true)
      resizeObserver?.disconnect()
      dataDisposable?.dispose()
      binaryDisposable?.dispose()
      resizeDisposable?.dispose()
      if (pendingReconnectTimer != null) window.clearTimeout(pendingReconnectTimer)
      ws?.close()
      webglAddon.dispose()
      term.dispose()
    }
  }, [activeWorkspaceId, activeWorkspace?.workspace_name, activeWorkspace?.worktree_path])

  return (
    <div
      data-testid="pty-terminal"
      ref={outerRef}
      tabIndex={0}
      className="luban-terminal h-full w-full p-0 font-mono text-xs overflow-hidden bg-card text-card-foreground focus:outline-none flex"
    >
      <div className="flex-1 min-h-0 min-w-0 overflow-hidden px-3">
        <div ref={containerRef} className="h-full w-full overflow-hidden" />
      </div>
    </div>
  )
}
