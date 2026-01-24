"use client"

import { createContext, useContext, useEffect, useMemo, useCallback } from "react"
import { useTheme } from "next-themes"
import type { ColorScheme, ColorPalette, ColorSchemeId } from "./types"
import { getSchemeOrDefault, builtinSchemes } from "./presets"

type ColorSchemeContextValue = {
  lightSchemeId: ColorSchemeId
  darkSchemeId: ColorSchemeId
  activeScheme: ColorScheme
  allSchemes: ColorScheme[]
  setLightScheme: (id: ColorSchemeId) => void
  setDarkScheme: (id: ColorSchemeId) => void
}

const ColorSchemeContext = createContext<ColorSchemeContextValue | null>(null)

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

export function applyPalette(palette: ColorPalette) {
  const css = paletteToCSS(palette)
  const root = document.documentElement
  for (const [key, value] of Object.entries(css)) {
    root.style.setProperty(key, value)
  }
}

const ALL_COLOR_VARS = [
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

export function clearPalette() {
  const root = document.documentElement
  for (const v of ALL_COLOR_VARS) {
    root.style.removeProperty(v)
  }
}

// Color role to CSS variable mapping for custom color overrides
const COLOR_ROLE_CSS_MAP: Record<string, string[]> = {
  primary: ["--primary", "--ring", "--sidebar-primary", "--sidebar-ring"],
  success: ["--chart-2"], // Green/success color
  error: ["--destructive"],
  accent: ["--accent", "--sidebar-accent"],
}

/**
 * Apply custom color overrides to the DOM.
 * Call this when the user changes colors in the picker for instant preview.
 */
export function applyCustomColors(overrides: Record<string, string> | null) {
  const root = document.documentElement
  if (!overrides) {
    // Clear custom overrides - the base palette will still be applied
    return
  }
  for (const [role, color] of Object.entries(overrides)) {
    const cssVars = COLOR_ROLE_CSS_MAP[role]
    if (cssVars) {
      for (const cssVar of cssVars) {
        root.style.setProperty(cssVar, color)
      }
    }
  }
}

interface ColorSchemeProviderProps {
  children: React.ReactNode
  lightSchemeId: ColorSchemeId
  darkSchemeId: ColorSchemeId
  onLightSchemeChange: (id: ColorSchemeId) => void
  onDarkSchemeChange: (id: ColorSchemeId) => void
}

export function ColorSchemeProvider({
  children,
  lightSchemeId,
  darkSchemeId,
  onLightSchemeChange,
  onDarkSchemeChange,
}: ColorSchemeProviderProps) {
  const { resolvedTheme } = useTheme()

  const activeMode = resolvedTheme === "dark" ? "dark" : "light"
  const activeSchemeId = activeMode === "dark" ? darkSchemeId : lightSchemeId

  const activeScheme = useMemo(() => {
    return getSchemeOrDefault(activeSchemeId, activeMode)
  }, [activeSchemeId, activeMode])

  useEffect(() => {
    if (activeSchemeId === "default") {
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
      return
    }

    applyPalette(activeScheme.palette)
  }, [activeScheme, activeSchemeId])

  const setLightScheme = useCallback(
    (id: ColorSchemeId) => {
      onLightSchemeChange(id)
    },
    [onLightSchemeChange],
  )

  const setDarkScheme = useCallback(
    (id: ColorSchemeId) => {
      onDarkSchemeChange(id)
    },
    [onDarkSchemeChange],
  )

  const value: ColorSchemeContextValue = useMemo(
    () => ({
      lightSchemeId,
      darkSchemeId,
      activeScheme,
      allSchemes: builtinSchemes,
      setLightScheme,
      setDarkScheme,
    }),
    [lightSchemeId, darkSchemeId, activeScheme, setLightScheme, setDarkScheme],
  )

  return <ColorSchemeContext.Provider value={value}>{children}</ColorSchemeContext.Provider>
}

export function useColorScheme(): ColorSchemeContextValue {
  const context = useContext(ColorSchemeContext)
  if (!context) {
    throw new Error("useColorScheme must be used within a ColorSchemeProvider")
  }
  return context
}
