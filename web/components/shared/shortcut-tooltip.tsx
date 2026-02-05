"use client"

import * as React from "react"

import { cn } from "@/lib/utils"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip"

function ShortcutKbd({ children }: { children: React.ReactNode }) {
  return (
    <kbd
      className={cn(
        "inline-flex items-center justify-center rounded-[6px] border px-1.5 py-0.5 text-[11px] font-medium",
        "min-w-[18px]",
      )}
      style={{ backgroundColor: "#fcfcfc", borderColor: "#ebebeb", color: "#6b6b6b" }}
    >
      {children}
    </kbd>
  )
}

export function ShortcutTooltip({
  label,
  keys,
  children,
  side = "bottom",
  align = "center",
  delayDuration = 120,
}: {
  label: string
  keys: string | string[]
  children: React.ReactElement
  side?: React.ComponentProps<typeof TooltipContent>["side"]
  align?: React.ComponentProps<typeof TooltipContent>["align"]
  delayDuration?: number
}) {
  const normalized = Array.isArray(keys) ? keys : [keys]
  return (
    <TooltipProvider delayDuration={delayDuration}>
      <Tooltip>
        <TooltipTrigger asChild>{children}</TooltipTrigger>
        <TooltipContent side={side} align={align}>
          <div className="flex items-center gap-2">
            <span className="truncate">{label}</span>
            <span className="flex items-center gap-1">
              {normalized.map((k, idx) => (
                <ShortcutKbd key={`${k}-${idx}`}>{k}</ShortcutKbd>
              ))}
            </span>
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

