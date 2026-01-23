"use client"

import { useEffect, useRef } from "react"
import { useTheme } from "next-themes"

import { useAppearance } from "@/components/appearance-provider"
import { useLuban } from "@/lib/luban-context"
import { getSchemeOrDefault } from "@/components/color-scheme/presets"
import type { ColorPalette } from "@/components/color-scheme/types"

function normalizeFont(raw: string, fallback: string): string {
  const trimmed = raw.trim()
  if (!trimmed) return fallback
  if (trimmed.length > 128) return fallback
  return trimmed
}

function paletteToCSS(palette: ColorPalette): Record<string, string> {
  return {
    "--background": palette.background,
    "--foreground": palette.foreground,
    "--card": palette.card,
    "--card-foreground": palette.cardForeground,
    "--popover": palette.popover,
    "--popover-foreground": palette.popoverForeground,
    "--primary": palette.primary,
    "--primary-foreground": palette.primaryForeground,
    "--secondary": palette.secondary,
    "--secondary-foreground": palette.secondaryForeground,
    "--muted": palette.muted,
    "--muted-foreground": palette.mutedForeground,
    "--accent": palette.accent,
    "--accent-foreground": palette.accentForeground,
    "--destructive": palette.destructive,
    "--destructive-foreground": palette.destructiveForeground,
    "--border": palette.border,
    "--input": palette.input,
    "--ring": palette.ring,
    "--sidebar": palette.sidebar,
    "--sidebar-foreground": palette.sidebarForeground,
    "--sidebar-primary": palette.sidebarPrimary,
    "--sidebar-primary-foreground": palette.sidebarPrimaryForeground,
    "--sidebar-accent": palette.sidebarAccent,
    "--sidebar-accent-foreground": palette.sidebarAccentForeground,
    "--sidebar-border": palette.sidebarBorder,
    "--sidebar-ring": palette.sidebarRing,
    "--chart-1": palette.chart1,
    "--chart-2": palette.chart2,
    "--chart-3": palette.chart3,
    "--chart-4": palette.chart4,
    "--chart-5": palette.chart5,
  }
}

function applyPalette(palette: ColorPalette) {
  const css = paletteToCSS(palette)
  const root = document.documentElement
  for (const [key, value] of Object.entries(css)) {
    root.style.setProperty(key, value)
  }
}

function clearPalette() {
  const root = document.documentElement
  const cssVars = [
    "--background",
    "--foreground",
    "--card",
    "--card-foreground",
    "--popover",
    "--popover-foreground",
    "--primary",
    "--primary-foreground",
    "--secondary",
    "--secondary-foreground",
    "--muted",
    "--muted-foreground",
    "--accent",
    "--accent-foreground",
    "--destructive",
    "--destructive-foreground",
    "--border",
    "--input",
    "--ring",
    "--sidebar",
    "--sidebar-foreground",
    "--sidebar-primary",
    "--sidebar-primary-foreground",
    "--sidebar-accent",
    "--sidebar-accent-foreground",
    "--sidebar-border",
    "--sidebar-ring",
    "--chart-1",
    "--chart-2",
    "--chart-3",
    "--chart-4",
    "--chart-5",
  ]
  for (const key of cssVars) {
    root.style.removeProperty(key)
  }
}

export function AppearanceSync() {
  const { app } = useLuban()
  const { setTheme, resolvedTheme } = useTheme()
  const { fonts, setFonts } = useAppearance()
  const lastThemeRef = useRef<string | null>(null)
  const lastFontsRef = useRef<string | null>(null)
  const lastColorSchemeRef = useRef<string | null>(null)

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
    if (lastFontsRef.current !== digest) {
      lastFontsRef.current = digest
      setFonts({ uiFont, chatFont, codeFont, terminalFont })
    }
  }, [app?.rev, fonts.uiFont, fonts.chatFont, fonts.codeFont, fonts.terminalFont, setTheme, setFonts])

  useEffect(() => {
    if (!app) return

    const colorScheme = app.appearance.color_scheme
    const lightSchemeId = colorScheme?.light_scheme_id ?? "default"
    const darkSchemeId = colorScheme?.dark_scheme_id ?? "default"

    const activeMode = resolvedTheme === "dark" ? "dark" : "light"
    const activeSchemeId = activeMode === "dark" ? darkSchemeId : lightSchemeId

    const schemeDigest = JSON.stringify({ activeSchemeId, activeMode })
    if (lastColorSchemeRef.current === schemeDigest) return
    lastColorSchemeRef.current = schemeDigest

    if (activeSchemeId === "default") {
      clearPalette()
      return
    }

    const scheme = getSchemeOrDefault(activeSchemeId, activeMode)
    applyPalette(scheme.palette)
  }, [app?.rev, app?.appearance?.color_scheme, resolvedTheme])

  return null
}
