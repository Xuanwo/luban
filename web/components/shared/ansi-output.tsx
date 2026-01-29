"use client"

import * as React from "react"

import { cn } from "@/lib/utils"

type AnsiStyleState = {
  fg?: string
  bg?: string
  bold?: boolean
  dim?: boolean
  italic?: boolean
  underline?: boolean
  inverse?: boolean
}

type AnsiSegment = {
  text: string
  style: AnsiStyleState
  styleKey: string
}

const ESC = "\u001b"
const CSI = "\u009b"
const OSC = "\u009d"

const DEFAULT_STYLE_KEY = "||||||"

function normalizeNewlines(input: string): string {
  return input.replaceAll("\r\n", "\n").replaceAll("\r", "\n")
}

function styleKey(style: AnsiStyleState): string {
  return [
    style.fg ?? "",
    style.bg ?? "",
    style.bold ? "1" : "",
    style.dim ? "1" : "",
    style.italic ? "1" : "",
    style.underline ? "1" : "",
    style.inverse ? "1" : "",
  ].join("|")
}

function isCsiFinalByte(code: number): boolean {
  return code >= 0x40 && code <= 0x7e
}

function parseCsiSequence(
  source: string,
  startIndex: number,
): { paramsText: string; command: string; nextIndex: number } | null {
  let cursor = startIndex
  let paramsText = ""
  while (cursor < source.length) {
    const code = source.charCodeAt(cursor)
    const ch = source[cursor] ?? ""
    if (isCsiFinalByte(code)) {
      return { paramsText, command: ch, nextIndex: cursor + 1 }
    }
    paramsText += ch
    cursor += 1
  }
  return null
}

function parseSgrCodes(paramsText: string): number[] {
  const trimmed = paramsText.trim()
  if (trimmed.length === 0) return [0]
  const out: number[] = []
  for (const part of trimmed.split(";")) {
    if (part.length === 0) {
      out.push(0)
      continue
    }
    const parsed = Number.parseInt(part, 10)
    if (Number.isFinite(parsed)) out.push(parsed)
  }
  return out.length > 0 ? out : [0]
}

function ansi16CssColor(index: number): string | null {
  switch (index) {
    case 0:
      return "var(--terminal-ansi-black)"
    case 1:
      return "var(--terminal-ansi-red)"
    case 2:
      return "var(--terminal-ansi-green)"
    case 3:
      return "var(--terminal-ansi-yellow)"
    case 4:
      return "var(--terminal-ansi-blue)"
    case 5:
      return "var(--terminal-ansi-magenta)"
    case 6:
      return "var(--terminal-ansi-cyan)"
    case 7:
      return "var(--terminal-ansi-white)"
    case 8:
      return "var(--terminal-ansi-bright-black)"
    case 9:
      return "var(--terminal-ansi-bright-red)"
    case 10:
      return "var(--terminal-ansi-bright-green)"
    case 11:
      return "var(--terminal-ansi-bright-yellow)"
    case 12:
      return "var(--terminal-ansi-bright-blue)"
    case 13:
      return "var(--terminal-ansi-bright-magenta)"
    case 14:
      return "var(--terminal-ansi-bright-cyan)"
    case 15:
      return "var(--terminal-ansi-bright-white)"
    default:
      return null
  }
}

function rgbCss(r: number, g: number, b: number): string {
  const clamp = (value: number) => Math.max(0, Math.min(255, Math.round(value)))
  return `rgb(${clamp(r)}, ${clamp(g)}, ${clamp(b)})`
}

function ansi256CssColor(index: number): string | null {
  if (!Number.isFinite(index) || index < 0) return null
  if (index <= 15) return ansi16CssColor(index)

  if (index >= 16 && index <= 231) {
    const offset = index - 16
    const r = Math.floor(offset / 36)
    const g = Math.floor((offset % 36) / 6)
    const b = offset % 6
    const steps = [0, 95, 135, 175, 215, 255]
    return rgbCss(steps[r] ?? 0, steps[g] ?? 0, steps[b] ?? 0)
  }

  if (index >= 232 && index <= 255) {
    const shade = 8 + (index - 232) * 10
    return rgbCss(shade, shade, shade)
  }

  return null
}

function applySgrCodes(style: AnsiStyleState, codes: number[]): AnsiStyleState {
  let idx = 0
  while (idx < codes.length) {
    const code = codes[idx] ?? 0

    if (code === 0) {
      style = {}
      idx += 1
      continue
    }

    if (code === 1) {
      style.bold = true
      idx += 1
      continue
    }
    if (code === 2) {
      style.dim = true
      idx += 1
      continue
    }
    if (code === 3) {
      style.italic = true
      idx += 1
      continue
    }
    if (code === 4) {
      style.underline = true
      idx += 1
      continue
    }
    if (code === 7) {
      style.inverse = true
      idx += 1
      continue
    }

    if (code === 22) {
      style.bold = false
      style.dim = false
      idx += 1
      continue
    }
    if (code === 23) {
      style.italic = false
      idx += 1
      continue
    }
    if (code === 24) {
      style.underline = false
      idx += 1
      continue
    }
    if (code === 27) {
      style.inverse = false
      idx += 1
      continue
    }

    if (code === 39) {
      style.fg = undefined
      idx += 1
      continue
    }
    if (code === 49) {
      style.bg = undefined
      idx += 1
      continue
    }

    if (code >= 30 && code <= 37) {
      style.fg = ansi16CssColor(code - 30) ?? undefined
      idx += 1
      continue
    }
    if (code >= 40 && code <= 47) {
      style.bg = ansi16CssColor(code - 40) ?? undefined
      idx += 1
      continue
    }
    if (code >= 90 && code <= 97) {
      style.fg = ansi16CssColor(code - 90 + 8) ?? undefined
      idx += 1
      continue
    }
    if (code >= 100 && code <= 107) {
      style.bg = ansi16CssColor(code - 100 + 8) ?? undefined
      idx += 1
      continue
    }

    if (code === 38 || code === 48) {
      const target = code === 38 ? "fg" : "bg"
      const mode = codes[idx + 1]
      if (mode === 5) {
        const colorIndex = codes[idx + 2]
        const css = ansi256CssColor(colorIndex ?? -1)
        style = { ...style, [target]: css ?? undefined }
        idx += 3
        continue
      }
      if (mode === 2) {
        const r = codes[idx + 2] ?? 0
        const g = codes[idx + 3] ?? 0
        const b = codes[idx + 4] ?? 0
        style = { ...style, [target]: rgbCss(r, g, b) }
        idx += 5
        continue
      }

      idx += 1
      continue
    }

    idx += 1
  }
  return style
}

function styleToCss(style: AnsiStyleState): React.CSSProperties {
  let fg = style.fg
  let bg = style.bg
  if (style.inverse) {
    const nextFg = bg
    const nextBg = fg
    fg = nextFg
    bg = nextBg
  }

  const out: React.CSSProperties = {}
  if (fg) out.color = fg
  if (bg) out.backgroundColor = bg
  if (style.bold) out.fontWeight = 600
  if (style.dim) out.opacity = 0.75
  if (style.italic) out.fontStyle = "italic"
  if (style.underline) out.textDecoration = "underline"
  return out
}

function skipOsc(source: string, startIndex: number): number {
  let cursor = startIndex
  while (cursor < source.length) {
    const ch = source[cursor] ?? ""
    if (ch === "\u0007") return cursor + 1
    if (ch === ESC && source[cursor + 1] === "\\") return cursor + 2
    cursor += 1
  }
  return cursor
}

function parseAnsiSegments(raw: string): AnsiSegment[] {
  const input = normalizeNewlines(raw)
  const hasEscapePrefixes = input.includes(ESC) || input.includes(CSI) || input.includes(OSC)
  const usesBracketEscapes = !hasEscapePrefixes && /\[\[(?:\d|;|\?)/.test(input) && /\[\[[0-9;?]*m/.test(input)

  let segments: AnsiSegment[] = []
  let activeStyle: AnsiStyleState = {}
  let activeStyleKey = styleKey(activeStyle)
  let buffer = ""

  const flush = () => {
    if (buffer.length === 0) return
    if (segments.length > 0 && segments[segments.length - 1]?.styleKey === activeStyleKey) {
      const last = segments[segments.length - 1]
      if (last) last.text += buffer
    } else {
      segments.push({ text: buffer, style: { ...activeStyle }, styleKey: activeStyleKey })
    }
    buffer = ""
  }

  const applyStyleUpdate = (nextStyle: AnsiStyleState) => {
    activeStyle = nextStyle
    activeStyleKey = styleKey(activeStyle)
  }

  let cursor = 0
  while (cursor < input.length) {
    const ch = input[cursor] ?? ""

    if (ch === ESC) {
      flush()
      const next = input[cursor + 1] ?? ""
      if (next === "[") {
        const seq = parseCsiSequence(input, cursor + 2)
        if (seq) {
          cursor = seq.nextIndex
          if (seq.command === "m") {
            applyStyleUpdate(applySgrCodes(activeStyle, parseSgrCodes(seq.paramsText)))
          }
          continue
        }
        cursor = input.length
        continue
      }

      if (next === "]") {
        cursor = skipOsc(input, cursor + 2)
        continue
      }

      cursor += Math.min(2, input.length - cursor)
      continue
    }

    if (ch === CSI) {
      flush()
      const seq = parseCsiSequence(input, cursor + 1)
      if (seq) {
        cursor = seq.nextIndex
        if (seq.command === "m") {
          applyStyleUpdate(applySgrCodes(activeStyle, parseSgrCodes(seq.paramsText)))
        }
        continue
      }
      cursor = input.length
      continue
    }

    if (ch === OSC) {
      flush()
      cursor = skipOsc(input, cursor + 1)
      continue
    }

    if (usesBracketEscapes && ch === "[" && input[cursor + 1] === "[") {
      const paramStart = input[cursor + 2] ?? ""
      if (paramStart && /[0-9;?]/.test(paramStart)) {
        flush()
        const seq = parseCsiSequence(input, cursor + 2)
        if (seq) {
          cursor = seq.nextIndex
          if (seq.command === "m") {
            applyStyleUpdate(applySgrCodes(activeStyle, parseSgrCodes(seq.paramsText)))
          }
          continue
        }
        cursor = input.length
        continue
      }
    }

    if (ch === "\u0000") {
      cursor += 1
      continue
    }

    if (ch === "\u0008") {
      buffer = buffer.length > 0 ? buffer.slice(0, -1) : buffer
      cursor += 1
      continue
    }

    buffer += ch
    cursor += 1
  }

  flush()

  if (segments.length === 0) return []

  if (segments[0]?.styleKey !== DEFAULT_STYLE_KEY) {
    return segments
  }

  return segments
}

function ansiSegmentsToNodes(segments: AnsiSegment[]): React.ReactNode[] {
  return segments.map((segment, index) => {
    if (segment.styleKey === DEFAULT_STYLE_KEY) return segment.text
    return (
      <span key={index} style={styleToCss(segment.style)}>
        {segment.text}
      </span>
    )
  })
}

export function AnsiOutput({
  text,
  className,
  fallback = "No output.",
  "data-testid": testId,
}: {
  text: string
  className?: string
  fallback?: string
  "data-testid"?: string
}): React.ReactElement {
  const normalized = React.useMemo(() => normalizeNewlines(text), [text])
  const hasContent = normalized.trim().length > 0
  const nodes = React.useMemo(() => ansiSegmentsToNodes(parseAnsiSegments(normalized)), [normalized])

  return (
    <pre data-testid={testId} className={cn("whitespace-pre-wrap break-words font-mono", className)}>
      {hasContent ? nodes : fallback}
    </pre>
  )
}

