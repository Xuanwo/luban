"use client"

import { Check } from "lucide-react"
import type { ColorScheme } from "./types"
import { cn } from "@/lib/utils"

interface ThemeCardProps {
  scheme: ColorScheme
  isSelected: boolean
  onClick: () => void
}

export function ThemeCard({ scheme, isSelected, onClick }: ThemeCardProps) {
  const previewColors = scheme.previewColors.slice(0, 6)

  return (
    <button
      onClick={onClick}
      className={cn(
        "group relative rounded-lg border-2 p-3 transition-all text-left",
        isSelected
          ? "border-primary bg-primary/5 ring-2 ring-primary/20"
          : "border-border hover:border-primary/50 hover:bg-accent/50",
      )}
    >
      <div className="flex items-center gap-2 mb-2">
        <span className={cn("text-sm font-medium", isSelected ? "text-primary" : "text-foreground")}>
          {scheme.name}
        </span>
        {isSelected && <Check className="w-3.5 h-3.5 text-primary" />}
      </div>
      <div className="flex items-center gap-1.5">
        {previewColors.map((color, index) => (
          <div
            key={index}
            className="w-5 h-5 rounded-full border border-border/50 shadow-sm"
            style={{ backgroundColor: color }}
          />
        ))}
      </div>
    </button>
  )
}
