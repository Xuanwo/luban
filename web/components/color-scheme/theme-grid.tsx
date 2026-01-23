"use client"

import type { ColorScheme, ColorSchemeId } from "./types"
import { ThemeCard } from "./theme-card"

interface ThemeGridProps {
  schemes: ColorScheme[]
  selectedId: ColorSchemeId
  onSelect: (id: ColorSchemeId) => void
  searchQuery?: string
}

export function ThemeGrid({ schemes, selectedId, onSelect, searchQuery = "" }: ThemeGridProps) {
  const filteredSchemes = schemes.filter((scheme) =>
    scheme.name.toLowerCase().includes(searchQuery.toLowerCase()),
  )

  if (filteredSchemes.length === 0) {
    return (
      <div className="text-center py-8 text-muted-foreground text-sm">
        No themes found matching &quot;{searchQuery}&quot;
      </div>
    )
  }

  return (
    <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
      {filteredSchemes.map((scheme) => (
        <ThemeCard
          key={`${scheme.id}-${scheme.mode}`}
          scheme={scheme}
          isSelected={selectedId === scheme.id}
          onClick={() => onSelect(scheme.id)}
        />
      ))}
    </div>
  )
}
