"use client"

import { useState } from "react"
import {
  ChevronDown,
  ChevronRight,
  Inbox,
  Search,
  Plus,
  Layers,
  Star,
  Settings,
} from "lucide-react"
import { cn } from "@/lib/utils"
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@/components/ui/dropdown-menu"

export type NavView = "inbox" | "tasks" | string

interface LubanSidebarProps {
  width?: number
  activeView?: NavView
  onViewChange?: (view: NavView) => void
}

interface NavItemProps {
  icon: React.ReactNode
  label: string
  badge?: number
  active?: boolean
  onClick?: () => void
}

function NavItem({ icon, label, badge, active, onClick }: NavItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "w-full flex items-center gap-2 px-2 py-1.5 rounded text-[13px] transition-colors",
        active
          ? "bg-[#e8e8e8]"
          : "hover:bg-[#eeeeee]"
      )}
      style={{ color: '#1b1b1b' }}
    >
      <span className="w-4 h-4 flex items-center justify-center" style={{ color: '#6b6b6b' }}>{icon}</span>
      <span className="flex-1 text-left truncate">{label}</span>
      {badge !== undefined && badge > 0 && (
        <span
          className="px-1.5 py-0.5 text-[11px] font-medium rounded"
          style={{ backgroundColor: '#e8e8e8', color: '#6b6b6b' }}
        >
          {badge}
        </span>
      )}
    </button>
  )
}

interface SectionProps {
  title: string
  defaultExpanded?: boolean
  children: React.ReactNode
}

function Section({ title, defaultExpanded = true, children }: SectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded)

  return (
    <div className="mb-1">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-1 px-2 py-1.5 text-[11px] font-medium transition-colors hover:bg-[#eeeeee] rounded"
        style={{ color: '#9b9b9b' }}
      >
        {expanded ? (
          <ChevronDown className="w-3 h-3" />
        ) : (
          <ChevronRight className="w-3 h-3" />
        )}
        <span className="flex-1 text-left">{title}</span>
      </button>
      {expanded && <div className="mt-0.5 space-y-0.5">{children}</div>}
    </div>
  )
}

interface ProjectItemProps {
  name: string
  color?: string
  active?: boolean
  onClick?: () => void
}

function ProjectItem({ name, color = "bg-[#5e6ad2]", active, onClick }: ProjectItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "w-full flex items-center gap-2 px-2 py-1.5 rounded cursor-pointer transition-colors",
        active ? "bg-[#e8e8e8]" : "hover:bg-[#eeeeee]"
      )}
    >
      <span
        className={cn(
          "w-[18px] h-[18px] rounded flex items-center justify-center text-[10px] font-semibold text-white",
          color
        )}
      >
        {name.charAt(0).toUpperCase()}
      </span>
      <span className="text-[13px] truncate" style={{ color: '#1b1b1b' }}>{name}</span>
    </button>
  )
}

export function LubanSidebar({ width = 244, activeView = "tasks", onViewChange }: LubanSidebarProps) {
  const handleNavClick = (view: NavView) => {
    onViewChange?.(view)
  }

  return (
    <div
      className="h-full flex flex-col"
      style={{ width: `${width}px` }}
    >
      {/* Header - Workspace Switcher */}
      <div className="flex items-center justify-between h-[52px] px-3">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button className="flex items-center gap-2 hover:bg-[#eeeeee] px-2 py-1 rounded transition-colors outline-none">
              <div className="w-5 h-5 rounded bg-[#5e6ad2] flex items-center justify-center">
                <Layers className="w-3 h-3 text-white" />
              </div>
              <span className="text-[13px] font-semibold" style={{ color: '#1b1b1b' }}>Luban</span>
              <ChevronDown className="w-3 h-3" style={{ color: '#9b9b9b' }} />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="start"
            sideOffset={4}
            className="w-[240px] rounded-lg border-[#e5e5e5] bg-white shadow-[0_4px_16px_rgba(0,0,0,0.12)] p-1.5"
          >
            <DropdownMenuItem
              onClick={() => onViewChange?.("settings")}
              className="flex items-center gap-2.5 px-2.5 py-2 text-[13px] rounded-md cursor-pointer hover:bg-[#f5f5f5] focus:bg-[#f5f5f5]"
              style={{ color: '#1b1b1b' }}
            >
              <Settings className="w-4 h-4" style={{ color: '#6b6b6b' }} />
              Settings
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <div className="flex items-center gap-0.5">
          <button
            className="p-1.5 rounded hover:bg-[#eeeeee] transition-colors"
            style={{ color: '#6b6b6b' }}
            title="Search"
          >
            <Search className="w-4 h-4" />
          </button>
          <button
            className="p-1.5 rounded hover:bg-[#eeeeee] transition-colors"
            style={{ color: '#6b6b6b' }}
            title="New task"
          >
            <Plus className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Navigation */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden py-2 px-2">
        {/* Main Navigation */}
        <div className="space-y-0.5 mb-4">
          <NavItem
            icon={<Inbox className="w-4 h-4" />}
            label="Inbox"
            badge={3}
            active={activeView === "inbox"}
            onClick={() => handleNavClick("inbox")}
          />
        </div>

        {/* Favorites Section */}
        <Section title="Favorites" defaultExpanded={true}>
          <NavItem
            icon={<Star className="w-4 h-4 text-yellow-500" />}
            label="Important Tasks"
            active={activeView === "favorites-1"}
            onClick={() => handleNavClick("favorites-1")}
          />
        </Section>

        {/* Projects Section */}
        <Section title="Projects">
          <ProjectItem
            name="Luban"
            color="bg-violet-500"
            active={activeView === "project-luban"}
            onClick={() => handleNavClick("project-luban")}
          />
          <ProjectItem
            name="Backend"
            color="bg-emerald-500"
            active={activeView === "project-backend"}
            onClick={() => handleNavClick("project-backend")}
          />
          <ProjectItem
            name="Frontend"
            color="bg-blue-500"
            active={activeView === "project-frontend"}
            onClick={() => handleNavClick("project-frontend")}
          />
        </Section>
      </div>
    </div>
  )
}
