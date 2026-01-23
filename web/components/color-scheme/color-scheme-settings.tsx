"use client"

import { useState, useMemo } from "react"
import { Search, Sun, Moon } from "lucide-react"
import { lightSchemes, darkSchemes } from "./presets"
import { ThemeGrid } from "./theme-grid"
import type { ColorSchemeId } from "./types"

interface ColorSchemeSettingsProps {
  lightSchemeId: ColorSchemeId
  darkSchemeId: ColorSchemeId
  onLightSchemeChange: (id: ColorSchemeId) => void
  onDarkSchemeChange: (id: ColorSchemeId) => void
}

export function ColorSchemeSettings({
  lightSchemeId,
  darkSchemeId,
  onLightSchemeChange,
  onDarkSchemeChange,
}: ColorSchemeSettingsProps) {
  const [searchQuery, setSearchQuery] = useState("")

  const lightCount = useMemo(
    () => lightSchemes.filter((s) => s.name.toLowerCase().includes(searchQuery.toLowerCase())).length,
    [searchQuery],
  )

  const darkCount = useMemo(
    () => darkSchemes.filter((s) => s.name.toLowerCase().includes(searchQuery.toLowerCase())).length,
    [searchQuery],
  )

  return (
    <div className="space-y-6">
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
        <input
          type="text"
          placeholder="Search themes..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full pl-9 pr-4 py-2 text-sm border border-border rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary"
        />
      </div>

      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Sun className="w-4 h-4 text-muted-foreground" />
            <h4 className="text-sm font-medium">Light Mode Themes</h4>
          </div>
          <span className="text-xs text-muted-foreground">{lightCount} themes</span>
        </div>
        <ThemeGrid
          schemes={lightSchemes}
          selectedId={lightSchemeId}
          onSelect={onLightSchemeChange}
          searchQuery={searchQuery}
        />
      </div>

      <div className="border-t border-border" />

      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Moon className="w-4 h-4 text-muted-foreground" />
            <h4 className="text-sm font-medium">Dark Mode Themes</h4>
          </div>
          <span className="text-xs text-muted-foreground">{darkCount} themes</span>
        </div>
        <ThemeGrid
          schemes={darkSchemes}
          selectedId={darkSchemeId}
          onSelect={onDarkSchemeChange}
          searchQuery={searchQuery}
        />
      </div>
    </div>
  )
}
