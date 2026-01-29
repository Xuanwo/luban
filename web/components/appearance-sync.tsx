"use client"

import { useEffect, useRef } from "react"
import { useTheme } from "next-themes"

import { useAppearance } from "@/components/appearance-provider"
import { useLuban } from "@/lib/luban-context"

function normalizeFont(raw: string, fallback: string): string {
  const trimmed = raw.trim()
  if (!trimmed) return fallback
  if (trimmed.length > 128) return fallback
  return trimmed
}

export function AppearanceSync() {
  const { app } = useLuban()
  const { setTheme } = useTheme()
  const { fonts, setFonts } = useAppearance()
  const lastThemeRef = useRef<string | null>(null)
  const lastFontsRef = useRef<string | null>(null)

  useEffect(() => {
    if (!app) return

    const theme = app.appearance.theme
    if (lastThemeRef.current !== theme) {
      lastThemeRef.current = theme
      setTheme(theme)
    }

    const next = app.appearance.fonts
    const uiFont = normalizeFont(next.ui_font, fonts.uiFont)
    const chatFont = normalizeFont(next.chat_font, fonts.chatFont)
    const codeFont = normalizeFont(next.code_font, fonts.codeFont)
    const terminalFont = normalizeFont(next.terminal_font, fonts.terminalFont)

    const digest = JSON.stringify({ uiFont, chatFont, codeFont, terminalFont })
    if (lastFontsRef.current === digest) return
    lastFontsRef.current = digest

    setFonts({ uiFont, chatFont, codeFont, terminalFont })
  }, [app, fonts.chatFont, fonts.codeFont, fonts.terminalFont, fonts.uiFont, setFonts, setTheme])

  return null
}
