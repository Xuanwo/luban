"use client"

import { useMemo, useState, useRef, useEffect, useCallback } from "react"
import { Check, ChevronDown, RotateCcw } from "lucide-react"
import { useTheme } from "next-themes"
import { lightSchemes, darkSchemes, getSchemeOrDefault } from "./presets"
import { applyCustomColors, applyPalette, clearPalette } from "./color-scheme-provider"
import { ColorPickerPopover } from "./color-picker-popover"
import type { ColorScheme, ColorSchemeId, ColorOverrides } from "./types"
import type { AppearanceTheme } from "@/lib/luban-api"
import { cn } from "@/lib/utils"

interface ColorSchemeSettingsProps {
  lightSchemeId: ColorSchemeId
  darkSchemeId: ColorSchemeId
  onLightSchemeChange: (id: ColorSchemeId) => void
  onDarkSchemeChange: (id: ColorSchemeId) => void
  onThemeChange: (theme: AppearanceTheme) => void
}

type ColorRole = "primary" | "accent" | "success" | "error"

function PresetDropdown({
  schemes,
  selectedId,
  onSelect,
  hasCustomColors,
}: {
  schemes: ColorScheme[]
  selectedId: string
  onSelect: (id: string) => void
  hasCustomColors: boolean
}) {
  const [open, setOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const handleClick = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener("mousedown", handleClick)
    return () => document.removeEventListener("mousedown", handleClick)
  }, [open])

  const selected = schemes.find(s => s.id === selectedId)
  const displayName = hasCustomColors ? `${selected?.name ?? "Default"} (Custom)` : (selected?.name ?? "Default")

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 text-[11px] font-medium opacity-70 hover:opacity-100 transition-opacity"
      >
        <span>{displayName}</span>
        <ChevronDown className={cn("w-3 h-3 transition-transform", open && "rotate-180")} />
      </button>

      {open && (
        <div className="absolute top-full left-0 mt-2 w-52 bg-popover/95 backdrop-blur-xl border border-border/50 rounded-lg shadow-2xl z-50 overflow-hidden">
          <div className="p-1 max-h-56 overflow-y-auto">
            {schemes.map((scheme) => {
              const isSelected = scheme.id === selectedId
              const colors = scheme.previewColors
              return (
                <button
                  key={scheme.id}
                  onClick={() => {
                    onSelect(scheme.id)
                    setOpen(false)
                  }}
                  className={cn(
                    "w-full flex items-center gap-2 px-2 py-1.5 text-left rounded-md transition-colors",
                    isSelected ? "bg-primary/10" : "hover:bg-accent/50",
                  )}
                >
                  <div className="flex -space-x-0.5 shrink-0">
                    {colors.slice(0, 4).map((c, i) => (
                      <div
                        key={i}
                        className="w-3 h-3 rounded-full border border-white/20"
                        style={{ backgroundColor: c }}
                      />
                    ))}
                  </div>
                  <span className={cn(
                    "text-[11px] flex-1 whitespace-nowrap",
                    isSelected ? "text-primary font-medium" : "",
                  )}>
                    {scheme.name}
                  </span>
                  {isSelected && <Check className="w-3 h-3 text-primary shrink-0" />}
                </button>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}

function LubanPreview({
  scheme,
  theme,
  onThemeChange,
  schemes,
  selectedSchemeId,
  onSchemeSelect,
  colorOverrides,
  onColorChange,
  onReset,
}: {
  scheme: ColorScheme
  theme: string | undefined
  onThemeChange: (theme: AppearanceTheme) => void
  schemes: ColorScheme[]
  selectedSchemeId: string
  onSchemeSelect: (id: string) => void
  colorOverrides: ColorOverrides
  onColorChange: (role: ColorRole, color: string) => void
  onReset: () => void
}) {
  const [hoveredRole, setHoveredRole] = useState<ColorRole | null>(null)

  // Base colors from scheme
  const baseColors = scheme.previewColors
  const bg = baseColors[0]
  const fg = baseColors[5]

  // Apply overrides
  const primary = colorOverrides.primary ?? baseColors[1]
  const green = colorOverrides.success ?? baseColors[2]
  const red = colorOverrides.error ?? baseColors[3]
  const purple = colorOverrides.accent ?? baseColors[4]

  const hasCustomColors = Object.keys(colorOverrides).length > 0

  const isDarkScheme = scheme.mode === "dark"
  const sidebarBg = isDarkScheme
    ? `color-mix(in srgb, ${bg} 75%, black)`
    : `color-mix(in srgb, ${bg} 88%, ${fg})`
  const terminalBg = isDarkScheme ? "#0c0c0c" : `color-mix(in srgb, ${bg} 85%, ${fg})`
  const bubbleBg = isDarkScheme
    ? `color-mix(in srgb, ${bg} 55%, white)`
    : `color-mix(in srgb, ${bg} 75%, ${fg})`

  const trafficLights = (
    <div className="flex items-center gap-1.5">
      <div className="w-2.5 h-2.5 rounded-full bg-[#FF5F57]" />
      <div className="w-2.5 h-2.5 rounded-full bg-[#FEBC2E]" />
      <div className="w-2.5 h-2.5 rounded-full bg-[#28C840]" />
    </div>
  )

  const handleHover = useCallback((role: ColorRole) => (highlighted: boolean) => {
    setHoveredRole(highlighted ? role : null)
  }, [])

  return (
    <div
      className="w-full rounded-xl overflow-hidden shadow-lg ring-1 ring-black/5 dark:ring-white/5 transition-all duration-500"
      style={{ backgroundColor: bg, color: fg }}
    >
      {/* macOS-style title bar */}
      <div
        className="h-9 flex items-center justify-between px-3 transition-colors duration-300"
        style={{
          backgroundColor: sidebarBg,
          borderBottom: `1px solid ${fg}10`,
        }}
      >
        {trafficLights}

        {/* Center: Preset selector */}
        <PresetDropdown
          schemes={schemes}
          selectedId={selectedSchemeId}
          onSelect={onSchemeSelect}
          hasCustomColors={hasCustomColors}
        />

        {/* Right: Mode toggle */}
        <div
          className="flex items-center p-0.5 rounded-md"
          style={{ backgroundColor: `${fg}08` }}
        >
          {(["light", "dark", "system"] as const).map((mode) => {
            const isSelected = theme === mode
            const labels = { light: "☀", dark: "☾", system: "◐" }
            return (
              <button
                key={mode}
                onClick={() => onThemeChange(mode)}
                className={cn(
                  "w-6 h-5 flex items-center justify-center rounded text-[10px] transition-all",
                  isSelected
                    ? "bg-white/90 dark:bg-white/20 shadow-sm"
                    : "hover:bg-white/40 dark:hover:bg-white/10",
                )}
                title={mode.charAt(0).toUpperCase() + mode.slice(1)}
              >
                {labels[mode]}
              </button>
            )
          })}
        </div>
      </div>

      {/* Main preview area */}
      <div className="flex h-48">
        {/* Sidebar */}
        <div
          className="w-24 flex flex-col transition-colors duration-300"
          style={{
            backgroundColor: sidebarBg,
            borderRight: `1px solid ${fg}08`,
          }}
        >
          <div className="flex-1 p-2 space-y-0.5">
            {/* Active item */}
            <div
              className={cn(
                "flex items-center gap-1.5 px-2 py-1.5 rounded-md transition-all duration-200",
                (hoveredRole === "primary" || hoveredRole === "success") && "ring-1 ring-white/30",
              )}
              style={{ backgroundColor: `${primary}20` }}
            >
              <div
                className={cn(
                  "w-1.5 h-1.5 rounded-full transition-transform duration-200",
                  hoveredRole === "success" && "scale-150",
                )}
                style={{ backgroundColor: green }}
              />
              <div
                className={cn(
                  "h-1.5 w-10 rounded-sm transition-all duration-200",
                  hoveredRole === "primary" && "ring-1 ring-white/40",
                )}
                style={{ backgroundColor: primary }}
              />
            </div>
            {/* Inactive items */}
            <div className="flex items-center gap-1.5 px-2 py-1.5 opacity-60">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: `${fg}40` }} />
              <div className="h-1.5 w-12 rounded-sm" style={{ backgroundColor: `${fg}20` }} />
            </div>
            <div
              className={cn(
                "flex items-center gap-1.5 px-2 py-1.5 opacity-60 rounded-md transition-all duration-200",
                hoveredRole === "error" && "opacity-100 ring-1 ring-white/30",
              )}
            >
              <div
                className={cn(
                  "w-1.5 h-1.5 rounded-full transition-transform duration-200",
                  hoveredRole === "error" && "scale-150",
                )}
                style={{ backgroundColor: red }}
              />
              <div className="h-1.5 w-8 rounded-sm" style={{ backgroundColor: `${fg}20` }} />
            </div>
          </div>
        </div>

        {/* Main chat area */}
        <div className="flex-1 flex flex-col min-w-0">
          {/* Tab bar */}
          <div
            className="h-7 flex items-center gap-1 px-2"
            style={{ borderBottom: `1px solid ${fg}06` }}
          >
            <div
              className="flex items-center gap-1 px-2 py-1 rounded-md"
              style={{ backgroundColor: `${fg}08` }}
            >
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: green }} />
              <div className="h-1 w-6 rounded-sm" style={{ backgroundColor: `${fg}30` }} />
            </div>
            <div className="flex items-center gap-1 px-2 py-1 opacity-50">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: `${fg}30` }} />
              <div className="h-1 w-8 rounded-sm" style={{ backgroundColor: `${fg}15` }} />
            </div>
          </div>

          {/* Chat content */}
          <div className="flex-1 p-3 space-y-2.5 overflow-hidden">
            {/* User message */}
            <div className="flex justify-end">
              <div
                className={cn(
                  "px-2.5 py-1.5 rounded-2xl rounded-br-md transition-all duration-200",
                  hoveredRole === "primary" && "scale-105 shadow-lg",
                )}
                style={{
                  backgroundColor: primary,
                  color: isDarkScheme ? bg : "#fff",
                }}
              >
                <div className="h-1.5 w-16 rounded-sm" style={{ backgroundColor: "currentColor", opacity: 0.5 }} />
              </div>
            </div>

            {/* Assistant message */}
            <div className="flex justify-start">
              <div
                className="px-2.5 py-1.5 rounded-2xl rounded-bl-md transition-colors duration-300"
                style={{ backgroundColor: bubbleBg }}
              >
                <div className="space-y-1">
                  <div className="h-1.5 w-28 rounded-sm" style={{ backgroundColor: `${fg}35` }} />
                  <div className="h-1.5 w-20 rounded-sm" style={{ backgroundColor: `${fg}25` }} />
                </div>
              </div>
            </div>

            {/* Code block */}
            <div className="flex justify-start">
              <div
                className={cn(
                  "px-2 py-1.5 rounded-lg text-[9px] font-mono leading-relaxed transition-all duration-200",
                  hoveredRole === "accent" && "ring-1 ring-white/20 shadow-lg",
                )}
                style={{ backgroundColor: terminalBg }}
              >
                <div className="flex items-center gap-1">
                  <span
                    className={cn("transition-all duration-200", hoveredRole === "accent" && "scale-110")}
                    style={{ color: purple }}
                  >fn</span>
                  <span
                    className={cn("transition-all duration-200", hoveredRole === "primary" && "scale-110")}
                    style={{ color: primary }}
                  >main</span>
                  <span style={{ color: `${fg}50` }}>()</span>
                </div>
                <div className="flex items-center gap-1 pl-2">
                  <span
                    className={cn("transition-all duration-200", hoveredRole === "success" && "scale-110")}
                    style={{ color: green }}
                  >println!</span>
                  <span style={{ color: `${fg}50` }}>(</span>
                  <span
                    className={cn("transition-all duration-200", hoveredRole === "error" && "scale-110")}
                    style={{ color: red }}
                  >&quot;Hi&quot;</span>
                  <span style={{ color: `${fg}50` }}>)</span>
                </div>
              </div>
            </div>
          </div>

          {/* Input area */}
          <div className="px-2 pb-2">
            <div
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg"
              style={{ backgroundColor: `${fg}05` }}
            >
              <div className="h-1.5 flex-1 rounded-sm" style={{ backgroundColor: `${fg}10` }} />
              <div
                className={cn(
                  "w-4 h-4 rounded-md transition-transform duration-200",
                  hoveredRole === "primary" && "scale-110",
                )}
                style={{ backgroundColor: primary, opacity: 0.7 }}
              />
            </div>
          </div>
        </div>

        {/* Terminal panel */}
        <div
          className="w-20 flex flex-col"
          style={{ borderLeft: `1px solid ${fg}06` }}
        >
          <div
            className="h-7 flex items-center px-2"
            style={{ borderBottom: `1px solid ${fg}06` }}
          >
            <div className="h-1 w-10 rounded-sm" style={{ backgroundColor: `${fg}20` }} />
          </div>
          <div
            className="flex-1 p-2 font-mono text-[8px] transition-colors duration-300"
            style={{ backgroundColor: terminalBg }}
          >
            <div className="space-y-1">
              <div className="flex items-center gap-0.5">
                <span
                  className={cn("transition-transform duration-200", hoveredRole === "success" && "scale-125")}
                  style={{ color: green }}
                >$</span>
                <div className="h-1 w-8 rounded-sm" style={{ backgroundColor: `${fg}30` }} />
              </div>
              <div className="h-1 w-12 rounded-sm" style={{ backgroundColor: `${fg}15` }} />
              <div className="h-1 w-6 rounded-sm" style={{ backgroundColor: `${fg}15` }} />
              <div className="flex items-center gap-0.5">
                <span style={{ color: green }}>$</span>
                <div className="h-1 w-1 rounded-sm animate-pulse" style={{ backgroundColor: `${fg}50` }} />
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Bottom color palette bar with pickers */}
      <div
        className="h-10 flex items-center justify-center gap-6 px-4"
        style={{
          backgroundColor: sidebarBg,
          borderTop: `1px solid ${fg}08`,
        }}
      >
        <ColorPickerPopover
          color={primary}
          label="Primary"
          onChange={(color) => onColorChange("primary", color)}
          isHighlighted={hoveredRole === "primary"}
          onHover={handleHover("primary")}
        />
        <ColorPickerPopover
          color={green}
          label="Success"
          onChange={(color) => onColorChange("success", color)}
          isHighlighted={hoveredRole === "success"}
          onHover={handleHover("success")}
        />
        <ColorPickerPopover
          color={red}
          label="Error"
          onChange={(color) => onColorChange("error", color)}
          isHighlighted={hoveredRole === "error"}
          onHover={handleHover("error")}
        />
        <ColorPickerPopover
          color={purple}
          label="Accent"
          onChange={(color) => onColorChange("accent", color)}
          isHighlighted={hoveredRole === "accent"}
          onHover={handleHover("accent")}
        />

        {/* Reset button - always rendered to preserve layout, invisible when not needed */}
        <button
          onClick={onReset}
          className={cn(
            "p-1 rounded-md transition-all ml-2",
            hasCustomColors
              ? "opacity-100 hover:bg-white/10"
              : "opacity-0 pointer-events-none",
          )}
          title="Reset to preset colors"
        >
          <RotateCcw className="w-3 h-3" />
        </button>
      </div>
    </div>
  )
}

/** Custom colors map: scheme_id -> (role -> color) */
type CustomColorsMap = Record<string, Record<string, string>>

export function ColorSchemeSettings({
  lightSchemeId,
  darkSchemeId,
  lightCustomColors,
  darkCustomColors,
  onLightSchemeChange,
  onDarkSchemeChange,
  onThemeChange,
  onCustomColorsChange,
}: ColorSchemeSettingsProps & {
  lightCustomColors?: CustomColorsMap
  darkCustomColors?: CustomColorsMap
  onCustomColorsChange?: (schemeId: string, colors: Record<string, string> | null, mode: "light" | "dark") => void
}) {
  const { theme, resolvedTheme } = useTheme()
  const isDark = resolvedTheme === "dark"

  // Track selected scheme locally for immediate UI updates (before server responds)
  const [localLightSchemeId, setLocalLightSchemeId] = useState(lightSchemeId)
  const [localDarkSchemeId, setLocalDarkSchemeId] = useState(darkSchemeId)

  // Track custom color overrides per scheme - keyed by scheme_id
  const [lightOverridesMap, setLightOverridesMap] = useState<CustomColorsMap>(
    () => lightCustomColors ?? {},
  )
  const [darkOverridesMap, setDarkOverridesMap] = useState<CustomColorsMap>(
    () => darkCustomColors ?? {},
  )

  // Sync local state when props change (e.g., from server or initial load)
  useEffect(() => {
    setLocalLightSchemeId(lightSchemeId)
  }, [lightSchemeId])
  useEffect(() => {
    setLocalDarkSchemeId(darkSchemeId)
  }, [darkSchemeId])

  // Sync custom colors map when props change
  useEffect(() => {
    setLightOverridesMap(lightCustomColors ?? {})
  }, [lightCustomColors])
  useEffect(() => {
    setDarkOverridesMap(darkCustomColors ?? {})
  }, [darkCustomColors])

  const currentSchemeId = isDark ? localDarkSchemeId : localLightSchemeId
  const currentSchemes = isDark ? darkSchemes : lightSchemes
  const currentOverridesMap = isDark ? darkOverridesMap : lightOverridesMap
  const setCurrentOverridesMap = isDark ? setDarkOverridesMap : setLightOverridesMap
  const setCurrentSchemeId = isDark ? setLocalDarkSchemeId : setLocalLightSchemeId

  // Get overrides for the current scheme from the map
  const currentOverrides = useMemo(
    () => (currentOverridesMap[currentSchemeId] as ColorOverrides) ?? {},
    [currentOverridesMap, currentSchemeId],
  )

  const handleSchemeSelect = useCallback((id: string) => {
    // Update local state immediately for instant UI feedback
    setCurrentSchemeId(id)

    // Get saved custom colors for the new scheme (if any)
    const savedColors = currentOverridesMap[id]

    // Immediately apply the new scheme's full palette to the DOM
    if (id === "default") {
      // For default, clear all inline styles to fall back to globals.css
      clearPalette()
    } else {
      // Apply the full new palette
      const newScheme = getSchemeOrDefault(id, isDark ? "dark" : "light")
      applyPalette(newScheme.palette)
    }

    // Apply saved custom colors on top if they exist
    if (savedColors && Object.keys(savedColors).length > 0) {
      applyCustomColors(savedColors)
    }

    // Update the scheme selection on server (don't clear custom colors - they're per-scheme now)
    if (isDark) {
      onDarkSchemeChange(id)
    } else {
      onLightSchemeChange(id)
    }
  }, [isDark, onDarkSchemeChange, onLightSchemeChange, setCurrentSchemeId, currentOverridesMap])

  // Auto-apply color changes
  const handleColorChange = useCallback((role: ColorRole, color: string) => {
    const newOverrides = {
      ...currentOverrides,
      [role]: color,
    }

    // Update the map with new overrides for this scheme
    setCurrentOverridesMap(prev => ({
      ...prev,
      [currentSchemeId]: newOverrides,
    }))

    // Apply colors directly to DOM for instant visual feedback
    applyCustomColors(newOverrides)

    // Notify parent of color changes immediately (with scheme_id)
    onCustomColorsChange?.(currentSchemeId, newOverrides, isDark ? "dark" : "light")
  }, [currentOverrides, currentSchemeId, setCurrentOverridesMap, isDark, onCustomColorsChange])

  const handleReset = useCallback(() => {
    // Remove this scheme's custom colors from the map
    setCurrentOverridesMap(prev => {
      const next = { ...prev }
      delete next[currentSchemeId]
      return next
    })

    const root = document.documentElement
    const colorVars = [
      "--primary",
      "--ring",
      "--sidebar-primary",
      "--sidebar-ring",
      "--chart-2",
      "--destructive",
      "--accent",
      "--sidebar-accent",
    ]

    if (currentSchemeId === "default") {
      // For default scheme, remove inline styles to fall back to globals.css
      for (const v of colorVars) {
        root.style.removeProperty(v)
      }
    } else {
      // Re-apply the full palette to reset custom values
      const scheme = getSchemeOrDefault(currentSchemeId, isDark ? "dark" : "light")
      const palette = scheme.palette
      root.style.setProperty("--primary", palette.primary)
      root.style.setProperty("--ring", palette.ring)
      root.style.setProperty("--sidebar-primary", palette.sidebarPrimary)
      root.style.setProperty("--sidebar-ring", palette.sidebarRing)
      root.style.setProperty("--chart-2", palette.chart2)
      root.style.setProperty("--destructive", palette.destructive)
      root.style.setProperty("--accent", palette.accent)
      root.style.setProperty("--sidebar-accent", palette.sidebarAccent)
    }

    // Notify parent that custom colors are cleared for this scheme
    onCustomColorsChange?.(currentSchemeId, null, isDark ? "dark" : "light")
  }, [currentSchemeId, setCurrentOverridesMap, isDark, onCustomColorsChange])

  const activeScheme = useMemo(
    () => getSchemeOrDefault(currentSchemeId, isDark ? "dark" : "light"),
    [currentSchemeId, isDark],
  )

  return (
    <LubanPreview
      scheme={activeScheme}
      theme={theme}
      onThemeChange={onThemeChange}
      schemes={currentSchemes}
      selectedSchemeId={currentSchemeId}
      onSchemeSelect={handleSchemeSelect}
      colorOverrides={currentOverrides}
      onColorChange={handleColorChange}
      onReset={handleReset}
    />
  )
}
