"use client"
import { cn } from "@/lib/utils"
import { Terminal, PanelRightClose, PanelRightOpen } from "lucide-react"
import { PtyTerminal } from "./pty-terminal"

interface RightSidebarProps {
  isOpen: boolean
  onToggle: () => void
  widthPx: number
}

export function RightSidebar({ isOpen, onToggle, widthPx }: RightSidebarProps) {
  if (!isOpen) {
    return (
      <button
        onClick={onToggle}
        className="absolute right-3 top-2 p-1.5 text-muted-foreground hover:text-foreground hover:bg-muted rounded transition-colors z-10"
        title="Open sidebar"
      >
        <PanelRightOpen className="w-4 h-4" />
      </button>
    )
  }

  return (
    <div className="border-l border-border bg-card flex flex-col" style={{ width: `${widthPx}px` }}>
      <div className="flex items-center h-11 px-1.5 border-b border-border gap-1">
        {/* Terminal tab button - highlighted when active */}
        <button className={cn("p-2 rounded transition-colors", "bg-primary/15 text-primary")} title="Terminal">
          <Terminal className="w-4 h-4" />
        </button>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Collapse button */}
        <button
          onClick={onToggle}
          className="p-2 text-muted-foreground hover:text-foreground hover:bg-muted rounded transition-colors"
          title="Close sidebar"
        >
          <PanelRightClose className="w-4 h-4" />
        </button>
      </div>

      {/* Terminal Content */}
      <div className="flex-1 overflow-auto overscroll-contain">
        <PtyTerminal />
      </div>
    </div>
  )
}
