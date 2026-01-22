"use client"

import { useState, useRef, useEffect, useMemo } from "react"
import { createHighlighter, type Highlighter } from "shiki"
import {
  X,
  Palette,
  Check,
  Sun,
  Moon,
  Monitor,
  ChevronDown,
  ChevronRight,
  Sparkles,
  Square,
  Settings,
  Type,
  Bot,
  Terminal,
  FolderOpen,
  ExternalLink,
  Loader2,
  CheckCircle2,
  XCircle,
  RefreshCw,
  FileText,
  FileCode,
  Folder,
  Pencil,
  Plus,
  ListTodo,
  Bug,
  Lightbulb,
  GitPullRequest,
  MessageSquare,
  HelpCircle,
  Play,
  Wrench,
  Brain,
  Clock,
  GitBranch,
  Sparkle,
  AlertTriangle,
  Info,
  ShieldCheck,
  UserPen,
  ClipboardType,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { AgentIcon, agentConfigs } from "./shared/agent-selector"


interface SettingsPanelProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

type TocItem = {
  id: string
  label: string
  icon: React.ElementType
  children?: { id: string; label: string; icon: React.ElementType }[]
}

const tocItems: TocItem[] = [
  {
    id: "appearance",
    label: "Appearance",
    icon: Palette,
    children: [
      { id: "theme", label: "Theme", icon: Sun },
      { id: "fonts", label: "Fonts", icon: Type },
    ],
  },
  {
    id: "agent",
    label: "Agent",
    icon: Bot,
  },
  {
    id: "task",
    label: "Task",
    icon: ListTodo,
  },
]

const themeOptions = [
  { id: "light", label: "Light", icon: Sun },
  { id: "dark", label: "Dark", icon: Moon },
  { id: "system", label: "System", icon: Monitor },
]

// Theme color schemes for previews
const themeColors = {
  light: {
    bg: "bg-gray-50",
    sidebar: "bg-gray-100",
    border: "border-gray-200",
    text: "text-gray-900",
    textMuted: "text-gray-500",
    secondary: "bg-gray-200/60",
    primary: "text-blue-600",
    primaryBg: "bg-blue-600",
    accent: "bg-blue-50",
  },
  dark: {
    bg: "bg-zinc-900",
    sidebar: "bg-zinc-800",
    border: "border-zinc-700",
    text: "text-zinc-100",
    textMuted: "text-zinc-400",
    secondary: "bg-zinc-700/60",
    primary: "text-blue-400",
    primaryBg: "bg-blue-500",
    accent: "bg-blue-900/30",
  },
}

function ThemePreviewCard({
  themeId,
  label,
  icon: Icon,
  isSelected,
  onClick,
}: {
  themeId: string
  label: string
  icon: React.ElementType
  isSelected: boolean
  onClick: () => void
}) {
  const isSystem = themeId === "system"
  const colors = themeColors[themeId as keyof typeof themeColors] || themeColors.light

  return (
    <button
      onClick={onClick}
      className={cn(
        "flex-1 rounded-xl border-2 overflow-hidden transition-all",
        isSelected
          ? "border-primary ring-2 ring-primary/20"
          : "border-border hover:border-primary/50"
      )}
    >
      {/* Mini preview */}
      <div className={cn("h-24 flex", isSystem ? "" : colors.bg)}>
        {isSystem ? (
          // System: left half light, right half dark - direct split layout
          <>
            {/* Left half - Light theme (Sidebar + half Main) */}
            <div className="flex-1 flex bg-gray-50">
              {/* Sidebar */}
              <div className={cn("w-12 border-r", themeColors.light.sidebar, themeColors.light.border)}>
                <div className={cn("h-5 border-b flex items-center px-1.5", themeColors.light.border)}>
                  <div className="w-3 h-3 rounded bg-gray-300" />
                </div>
                <div className="p-1.5 space-y-1">
                  <div className="h-2 w-8 rounded bg-gray-300" />
                  <div className="h-2 w-6 rounded bg-blue-100" />
                  <div className="h-2 w-7 rounded bg-gray-200" />
                </div>
              </div>
              {/* Half main content */}
              <div className="flex-1 flex flex-col">
                <div className={cn("h-5 border-b", themeColors.light.border)} />
                <div className="flex-1 p-2 space-y-1.5">
                  <div className="h-5 rounded bg-gray-200/60" />
                  <div className="h-8 rounded bg-gray-200/60" />
                </div>
              </div>
            </div>
            {/* Right half - Dark theme (half Main + Terminal) */}
            <div className="flex-1 flex bg-zinc-900">
              {/* Half main content */}
              <div className="flex-1 flex flex-col">
                <div className={cn("h-5 border-b", themeColors.dark.border)} />
                <div className="flex-1 p-2 space-y-1.5">
                  <div className="h-5 rounded bg-zinc-700/60" />
                  <div className="h-8 rounded bg-zinc-700/60" />
                </div>
              </div>
              {/* Terminal */}
              <div className={cn("w-10 border-l", themeColors.dark.border)}>
                <div className={cn("h-5 border-b", themeColors.dark.border)} />
                <div className="flex-1 p-1 bg-zinc-700/60">
                  <div className="h-1.5 w-6 rounded bg-green-500/40" />
                </div>
              </div>
            </div>
          </>
        ) : (
          // Light or Dark: full preview
          <>
            {/* Sidebar */}
            <div className={cn("w-12 border-r", colors.sidebar, colors.border)}>
              <div className={cn("h-5 border-b flex items-center px-1.5", colors.border)}>
                <div className={cn("w-3 h-3 rounded", colors.secondary)} />
              </div>
              <div className="p-1.5 space-y-1">
                <div className={cn("h-2 w-8 rounded", colors.secondary)} />
                <div className={cn("h-2 w-6 rounded", colors.accent)} />
                <div className={cn("h-2 w-7 rounded", colors.secondary)} />
              </div>
            </div>
            {/* Main content */}
            <div className="flex-1 flex flex-col">
              <div className={cn("h-5 border-b", colors.border)} />
              <div className="flex-1 p-2 space-y-1.5">
                <div className={cn("h-5 rounded", colors.secondary)} />
                <div className={cn("h-8 rounded", colors.secondary)} />
              </div>
            </div>
            {/* Terminal */}
            <div className={cn("w-10 border-l", colors.border)}>
              <div className={cn("h-5 border-b", colors.border)} />
              <div className={cn("flex-1 p-1", colors.secondary)}>
                <div className={cn("h-1.5 w-6 rounded", themeId === "dark" ? "bg-green-500/40" : "bg-green-600/30")} />
              </div>
            </div>
          </>
        )}
      </div>

      {/* Label */}
      <div className={cn(
        "flex items-center justify-center gap-2 py-2 border-t",
        isSelected ? "bg-primary/5 border-primary/20" : "bg-secondary/30 border-border"
      )}>
        <Icon className={cn("w-4 h-4", isSelected ? "text-primary" : "text-muted-foreground")} />
        <span className={cn("text-sm font-medium", isSelected ? "text-primary" : "text-foreground")}>
          {label}
        </span>
        {isSelected && <Check className="w-3.5 h-3.5 text-primary" />}
      </div>
    </button>
  )
}

// Mock local fonts
const mockLocalFonts = [
  "Inter",
  "SF Pro",
  "SF Pro Display",
  "Helvetica Neue",
  "Arial",
  "Roboto",
  "Open Sans",
  "Lato",
  "Montserrat",
  "Poppins",
  "Nunito",
  "Source Sans Pro",
  "Segoe UI",
  "Georgia",
  "Source Serif Pro",
  "Merriweather",
  "Lora",
]

const mockMonoFonts = [
  "Geist Mono",
  "JetBrains Mono",
  "Fira Code",
  "SF Mono",
  "Menlo",
  "Monaco",
  "Consolas",
  "Source Code Pro",
  "IBM Plex Mono",
  "Cascadia Code",
  "Roboto Mono",
  "Ubuntu Mono",
  "Inconsolata",
]

interface InlineFontSelectProps {
  value: string
  onChange: (value: string) => void
  fonts: string[]
  mono?: boolean
}

function InlineFontSelect({ value, onChange, fonts, mono, label, vertical }: InlineFontSelectProps & { label: string; vertical?: boolean }) {
  const [open, setOpen] = useState(false)

  return (
    <div className={cn("relative", vertical ? "flex flex-col gap-1" : "inline-flex items-center gap-1.5")}>
      <span className="text-[10px] uppercase tracking-wider text-muted-foreground/70">{label}</span>
      <button
        onClick={() => setOpen(!open)}
        className={cn(
          "inline-flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium transition-all",
          "bg-primary/15 hover:bg-primary/25 text-primary border border-primary/30",
          "shadow-sm hover:shadow",
          open && "ring-2 ring-primary/40 bg-primary/25"
        )}
      >
        <span
          className={mono ? "font-mono" : ""}
          style={{ fontFamily: `"${value}", ${mono ? "monospace" : "sans-serif"}` }}
        >
          {value}
        </span>
        <ChevronDown className={cn("w-3 h-3 transition-transform", open && "rotate-180")} />
      </button>

      {open && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
          <div className="absolute top-full left-0 mt-1.5 z-50 bg-popover border border-border rounded-lg shadow-xl max-h-52 w-44 overflow-y-auto">
            <div className="p-1">
              {fonts.map((font) => {
                const isSelected = value === font
                return (
                  <button
                    key={font}
                    onClick={() => {
                      onChange(font)
                      setOpen(false)
                    }}
                    className={cn(
                      "w-full flex items-center justify-between px-2.5 py-1.5 text-left transition-colors text-xs rounded-md",
                      isSelected ? "bg-primary/10 text-primary" : "hover:bg-accent"
                    )}
                  >
                    <span
                      className={mono ? "font-mono" : ""}
                      style={{ fontFamily: `"${font}", ${mono ? "monospace" : "sans-serif"}` }}
                    >
                      {font}
                    </span>
                    {isSelected && <Check className="w-3 h-3" />}
                  </button>
                )
              })}
            </div>
          </div>
        </>
      )}
    </div>
  )
}

function WorkspacePreviewWithFonts({
  uiFont,
  setUiFont,
  chatFont,
  setChatFont,
  monoFont,
  setMonoFont,
  terminalFont,
  setTerminalFont,
}: {
  uiFont: string
  setUiFont: (v: string) => void
  chatFont: string
  setChatFont: (v: string) => void
  monoFont: string
  setMonoFont: (v: string) => void
  terminalFont: string
  setTerminalFont: (v: string) => void
}) {
  return (
    <div className="w-full border border-border rounded-xl overflow-hidden bg-card shadow-sm pointer-events-none select-none">
      <div className="flex h-80">
        {/* Sidebar - blurred/simplified */}
        <div className="w-44 border-r border-border bg-sidebar flex flex-col">
          {/* Sidebar header - placeholder */}
          <div className="h-11 px-3 border-b border-border flex items-center gap-2">
            <div className="w-5 h-5 rounded bg-muted-foreground/20" />
            <div className="h-3 w-16 rounded bg-muted-foreground/20" />
          </div>

          {/* Sidebar content */}
          <div className="flex-1 p-3 space-y-2">
            {/* Font picker - highlighted */}
            <div className="mb-3 pointer-events-auto">
              <InlineFontSelect label="UI Font" value={uiFont} onChange={setUiFont} fonts={mockLocalFonts} vertical />
            </div>
            
            {/* UI Font preview */}
            <div
              className="px-1"
              style={{ fontFamily: `"${uiFont}", sans-serif` }}
            >
              <p className="text-xs text-muted-foreground leading-relaxed">
                The quick brown fox jumps over the lazy dog
              </p>
            </div>
          </div>

          {/* Footer placeholder */}
          <div className="border-t border-border p-2 opacity-40">
            <div className="h-2.5 w-20 rounded bg-muted-foreground/30" />
          </div>
        </div>

        {/* Main content area */}
        <div className="flex-1 flex flex-col min-w-0">
          {/* Tab bar - placeholder */}
          <div className="h-11 border-b border-border px-3 flex items-center gap-2 opacity-40">
            <div className="flex items-center gap-1.5 px-2 py-1 rounded-md bg-muted-foreground/20">
              <div className="w-2 h-2 rounded-full bg-muted-foreground/40" />
              <div className="h-2 w-8 rounded bg-muted-foreground/30" />
            </div>
            <div className="flex items-center gap-1.5 px-2 py-1">
              <div className="w-2 h-2 rounded-full bg-muted-foreground/40" />
              <div className="h-2 w-12 rounded bg-muted-foreground/30" />
            </div>
          </div>

          {/* Chat area */}
          <div className="flex-1 p-4 space-y-4 overflow-hidden">
            {/* Chat block - highlighted */}
            <div className="space-y-2">
              <div className="pointer-events-auto">
                <InlineFontSelect label="Chat Font" value={chatFont} onChange={setChatFont} fonts={mockLocalFonts} />
              </div>
              <div
                className="bg-secondary/40 rounded-lg p-3"
                style={{ fontFamily: `"${chatFont}", sans-serif` }}
              >
                <p className="text-sm leading-relaxed text-muted-foreground">
                  The quick brown fox jumps over the lazy dog
                </p>
              </div>
            </div>

            {/* Code block - highlighted */}
            <div className="space-y-2">
              <div className="pointer-events-auto">
                <InlineFontSelect label="Code Font" value={monoFont} onChange={setMonoFont} fonts={mockMonoFonts} mono />
              </div>
              <div
                className="bg-secondary/60 border border-border rounded-lg p-3"
                style={{ fontFamily: `"${monoFont}", monospace` }}
              >
                <pre className="text-sm leading-relaxed">
                  <span className="text-[#cf222e] dark:text-[#ff7b72]">fn</span>{" "}
                  <span className="text-[#8250df] dark:text-[#d2a8ff]">hello</span>
                  <span className="text-muted-foreground">()</span>{" "}
                  <span className="text-[#cf222e] dark:text-[#ff7b72]">{"->"}</span>{" "}
                  <span className="text-[#0550ae] dark:text-[#79c0ff]">String</span>{" "}
                  <span className="text-muted-foreground">{"{"}</span>
                  {"\n"}
                  {"    "}
                  <span className="text-[#0a3069] dark:text-[#a5d6ff]">{'"The quick brown fox"'}</span>
                  <span className="text-muted-foreground">.to_string()</span>
                  {"\n"}
                  <span className="text-muted-foreground">{"}"}</span>
                </pre>
              </div>
            </div>
          </div>
        </div>

        {/* Terminal panel */}
        <div className="w-48 border-l border-border flex flex-col">
          {/* Header placeholder */}
          <div className="h-11 border-b border-border px-3 flex items-center opacity-40">
            <div className="h-2 w-12 rounded bg-muted-foreground/30" />
          </div>
          <div className="flex-1 bg-secondary/40 flex flex-col">
            {/* Font picker - highlighted */}
            <div className="px-3 py-2 pointer-events-auto">
              <InlineFontSelect label="Terminal Font" value={terminalFont} onChange={setTerminalFont} fonts={mockMonoFonts} mono vertical />
            </div>
            {/* Terminal content - highlighted */}
            <div
              className="flex-1 px-3 pb-3"
              style={{ fontFamily: `"${terminalFont}", monospace` }}
            >
              <p className="text-sm leading-relaxed text-muted-foreground">
                The quick brown fox jumps over the lazy dog
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

type CheckStatus = "idle" | "checking" | "success" | "error"

type ConfigFile = {
  id: string
  name: string
  type: "file" | "folder"
  icon: React.ElementType
  iconColor?: string
  children?: ConfigFile[]
  content?: string
}

const mockConfigTree: ConfigFile[] = [
  {
    id: "agents-md",
    name: "AGENTS.md",
    type: "file",
    icon: FileText,
    iconColor: "text-blue-500",
    content: `# Codex Agent Configuration

## Model Settings
- Default model: gpt-4o
- Temperature: 0.7
- Max tokens: 4096

## Behavior
- Auto-approve safe commands: true
- Sandbox mode: enabled

## Instructions
You are a helpful coding assistant. Follow best practices
and write clean, maintainable code.`,
  },
  {
    id: "config-toml",
    name: "config.toml",
    type: "file",
    icon: FileCode,
    iconColor: "text-orange-500",
    content: `[model]
default = "gpt-4o"
temperature = 0.7
max_tokens = 4096

[behavior]
auto_approve_safe = true
sandbox = true

[editor]
theme = "dark"
font_size = 14`,
  },
  {
    id: "skills",
    name: "skills",
    type: "folder",
    icon: Folder,
    iconColor: "text-yellow-500",
    children: [
      {
        id: "skills-web-browser",
        name: "web-browser",
        type: "folder",
        icon: Folder,
        iconColor: "text-yellow-500/70",
        children: [
          {
            id: "skills-web-browser-skill",
            name: "SKILL.md",
            type: "file",
            icon: FileText,
            iconColor: "text-blue-400",
            content: `# Web Browser Skill

Enables web browsing and page interaction.

## Capabilities
- Navigate to URLs
- Extract page content
- Take screenshots
- Fill forms`,
          },
        ],
      },
      {
        id: "skills-code-review",
        name: "code-review",
        type: "folder",
        icon: Folder,
        iconColor: "text-yellow-500/70",
        children: [
          {
            id: "skills-code-review-skill",
            name: "SKILL.md",
            type: "file",
            icon: FileText,
            iconColor: "text-blue-400",
            content: `# Code Review Skill

Provides automated code review capabilities.

## Features
- Static analysis
- Best practice checks
- Security scanning
- Performance suggestions`,
          },
        ],
      },
    ],
  },
]

function ConfigFileTree({
  files,
  level = 0,
  selectedFile,
  expandedFolders,
  onSelectFile,
  onToggleFolder,
}: {
  files: ConfigFile[]
  level?: number
  selectedFile: string | null
  expandedFolders: Set<string>
  onSelectFile: (file: ConfigFile) => void
  onToggleFolder: (id: string) => void
}) {
  return (
    <div className="space-y-0.5">
      {files.map((file) => {
        const Icon = file.icon
        const isExpanded = expandedFolders.has(file.id)
        const isSelected = selectedFile === file.id
        const isFolder = file.type === "folder"

        return (
          <div key={file.id}>
            <button
              onClick={() => {
                if (isFolder) {
                  onToggleFolder(file.id)
                } else {
                  onSelectFile(file)
                }
              }}
              className={cn(
                "w-full flex items-center gap-1.5 px-2 py-1 rounded text-left transition-colors text-xs",
                isSelected
                  ? "bg-primary/15 text-primary"
                  : "text-muted-foreground hover:text-foreground hover:bg-accent"
              )}
              style={{ paddingLeft: `${8 + level * 12}px` }}
            >
              {isFolder && (
                <ChevronRight
                  className={cn(
                    "w-3 h-3 text-muted-foreground transition-transform flex-shrink-0",
                    isExpanded && "rotate-90"
                  )}
                />
              )}
              {!isFolder && <div className="w-3" />}
              <Icon className={cn("w-3.5 h-3.5 flex-shrink-0", file.iconColor || "text-muted-foreground")} />
              <span className="truncate">{file.name}</span>
            </button>
            {isFolder && isExpanded && file.children && (
              <ConfigFileTree
                files={file.children}
                level={level + 1}
                selectedFile={selectedFile}
                expandedFolders={expandedFolders}
                onSelectFile={onSelectFile}
                onToggleFolder={onToggleFolder}
              />
            )}
          </div>
        )
      })}
    </div>
  )
}

type SaveStatus = "idle" | "unsaved" | "saving" | "saved"

function DefaultAgentSelector() {
  const [selectedAgentId, setSelectedAgentId] = useState("claude-code")
  const [isOpen, setIsOpen] = useState(false)

  const selectedAgent = agentConfigs.find((a) => a.id === selectedAgentId) ?? agentConfigs[0]

  return (
    <div className="flex items-center justify-between py-2">
      <div>
        <div className="text-sm">Default Agent</div>
        <div className="text-xs text-muted-foreground">
          Agent selected by default when starting a new chat
        </div>
      </div>

      <div className="relative">
        <button
          onClick={() => setIsOpen(!isOpen)}
          className={cn(
            "inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-all",
            "bg-primary/15 hover:bg-primary/25 text-primary border border-primary/30",
            "shadow-sm hover:shadow",
            isOpen && "ring-2 ring-primary/40 bg-primary/25"
          )}
        >
          <AgentIcon agentId={selectedAgentId} className="w-3.5 h-3.5" />
          <span>{selectedAgent.name}</span>
          <ChevronDown className={cn("w-3 h-3 transition-transform", isOpen && "rotate-180")} />
        </button>

        {isOpen && (
          <>
            <div className="fixed inset-0 z-40" onClick={() => setIsOpen(false)} />
            <div className="absolute right-0 top-full mt-1.5 z-50 bg-popover border border-border rounded-lg shadow-xl w-44 overflow-hidden">
              <div className="p-1">
                {agentConfigs.map((agent) => {
                  const isSelected = agent.id === selectedAgentId
                  return (
                    <button
                      key={agent.id}
                      onClick={() => {
                        setSelectedAgentId(agent.id)
                        setIsOpen(false)
                      }}
                      className={cn(
                        "w-full flex items-center gap-2 px-2.5 py-1.5 text-left transition-colors text-xs rounded-md",
                        isSelected ? "bg-primary/10 text-primary" : "hover:bg-accent"
                      )}
                    >
                      <AgentIcon agentId={agent.id} className="w-3.5 h-3.5" />
                      <span className="flex-1">{agent.name}</span>
                      {isSelected && <Check className="w-3 h-3" />}
                    </button>
                  )
                })}
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  )
}

function CodexSettings() {
  const [enabled, setEnabled] = useState(true)
  const [checkStatus, setCheckStatus] = useState<CheckStatus>("idle")
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle")
  const [selectedFile, setSelectedFile] = useState<ConfigFile | null>(null)
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set(["skills"]))
  const [fileContents, setFileContents] = useState<Record<string, string>>({})
  const saveTimeoutRef = useRef<NodeJS.Timeout | null>(null)
  const editorRef = useRef<HTMLTextAreaElement>(null)
  const highlightRef = useRef<HTMLDivElement>(null)
  const highlighter = useShikiHighlighter()

  const handleEditorScroll = () => {
    if (!editorRef.current || !highlightRef.current) return
    highlightRef.current.scrollTop = editorRef.current.scrollTop
    highlightRef.current.scrollLeft = editorRef.current.scrollLeft
  }

  const getFileLanguage = (fileName: string): string => {
    if (fileName.endsWith(".md")) return "markdown"
    if (fileName.endsWith(".toml")) return "toml"
    return "markdown"
  }

  const handleCheck = () => {
    setCheckStatus("checking")
    setTimeout(() => {
      setCheckStatus(Math.random() > 0.3 ? "success" : "error")
    }, 1500)
  }

  const handleEditInLuban = () => {
    alert("Adding ~/.codex as a project in Luban...")
  }

  const handleToggleFolder = (id: string) => {
    setExpandedFolders((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  const handleSelectFile = (file: ConfigFile) => {
    if (file.type === "file") {
      setSelectedFile(file)
      if (file.content && !fileContents[file.id]) {
        setFileContents(prev => ({ ...prev, [file.id]: file.content || "" }))
      }
    }
  }

  const handleContentChange = (content: string) => {
    if (!selectedFile) return
    
    setFileContents(prev => ({ ...prev, [selectedFile.id]: content }))
    setSaveStatus("unsaved")

    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current)
    }

    saveTimeoutRef.current = setTimeout(() => {
      setSaveStatus("saving")
      setTimeout(() => {
        setSaveStatus("saved")
        setTimeout(() => {
          setSaveStatus("idle")
        }, 1500)
      }, 500)
    }, 800)
  }

  const currentContent = selectedFile 
    ? (fileContents[selectedFile.id] ?? selectedFile.content ?? "")
    : ""

  return (
    <div className={cn(
      "rounded-xl border border-border bg-card overflow-hidden shadow-sm",
      !enabled && "w-44"
    )}>
      <div className={cn("flex", enabled ? "h-[320px]" : "h-11")}>
        {/* Left sidebar - Codex header + file tree */}
        <div className={cn(
          "w-44 flex flex-col bg-sidebar",
          enabled && "border-r border-border",
          !enabled && "opacity-60"
        )}>
          {/* Sidebar header with Codex branding + toggle */}
          <div className={cn(
            "flex items-center justify-between h-11 px-3",
            enabled && "border-b border-border"
          )}>
            <div className="flex items-center gap-2">
              <svg className="w-4 h-4" viewBox="0 0 24 24" fill="currentColor">
                <path d="M22.282 9.821a5.985 5.985 0 0 0-.516-4.91 6.046 6.046 0 0 0-6.51-2.9A6.065 6.065 0 0 0 4.981 4.18a5.985 5.985 0 0 0-3.998 2.9 6.046 6.046 0 0 0 .743 7.097 5.98 5.98 0 0 0 .51 4.911 6.051 6.051 0 0 0 6.515 2.9A5.985 5.985 0 0 0 13.26 24a6.056 6.056 0 0 0 5.772-4.206 5.99 5.99 0 0 0 3.997-2.9 6.056 6.056 0 0 0-.747-7.073zM13.26 22.43a4.476 4.476 0 0 1-2.876-1.04l.141-.081 4.779-2.758a.795.795 0 0 0 .392-.681v-6.737l2.02 1.168a.071.071 0 0 1 .038.052v5.583a4.504 4.504 0 0 1-4.494 4.494zM3.6 18.304a4.47 4.47 0 0 1-.535-3.014l.142.085 4.783 2.759a.771.771 0 0 0 .78 0l5.843-3.369v2.332a.08.08 0 0 1-.033.062L9.74 19.95a4.5 4.5 0 0 1-6.14-1.646zM2.34 7.896a4.485 4.485 0 0 1 2.366-1.973V11.6a.766.766 0 0 0 .388.676l5.815 3.355-2.02 1.168a.076.076 0 0 1-.071 0l-4.83-2.786A4.504 4.504 0 0 1 2.34 7.872zm16.597 3.855-5.833-3.387L15.119 7.2a.076.076 0 0 1 .071 0l4.83 2.791a4.494 4.494 0 0 1-.676 8.105v-5.678a.79.79 0 0 0-.407-.667zm2.01-3.023-.141-.085-4.774-2.782a.776.776 0 0 0-.785 0L9.409 9.23V6.897a.066.066 0 0 1 .028-.061l4.83-2.787a4.5 4.5 0 0 1 6.68 4.66zm-12.64 4.135-2.02-1.164a.08.08 0 0 1-.038-.057V6.075a4.5 4.5 0 0 1 7.375-3.453l-.142.08-4.778 2.758a.795.795 0 0 0-.393.681zm1.097-2.365 2.602-1.5 2.607 1.5v2.999l-2.597 1.5-2.607-1.5z" />
              </svg>
              <span className="text-sm font-medium">Codex</span>
            </div>
            {/* Enable toggle */}
            <button
              onClick={() => setEnabled(!enabled)}
              className={cn(
                "relative w-9 h-5 rounded-full transition-colors",
                enabled ? "bg-primary" : "bg-muted"
              )}
            >
              <div
                className={cn(
                  "absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform",
                  enabled ? "translate-x-4" : "translate-x-0.5"
                )}
              />
            </button>
          </div>
          {/* File tree - only shown when enabled */}
          {enabled && (
            <div className="flex-1 overflow-y-auto py-1.5">
              <ConfigFileTree
                files={mockConfigTree}
                selectedFile={selectedFile?.id || null}
                expandedFolders={expandedFolders}
                onSelectFile={handleSelectFile}
                onToggleFolder={handleToggleFolder}
              />
            </div>
          )}
        </div>

        {/* Right side: Editor header + Editor - only shown when enabled */}
        {enabled && (
          <div className="flex-1 flex flex-col min-w-0 bg-background">
            {/* Editor header */}
            <div className="flex items-center justify-between h-11 px-3 border-b border-border">
              <div className="flex items-center gap-2">
                {selectedFile ? (
                  <>
                    <selectedFile.icon className={cn("w-4 h-4", selectedFile.iconColor || "text-muted-foreground")} />
                    <span className="text-sm font-medium">{selectedFile.name}</span>
                  </>
                ) : (
                  <span className="text-sm text-muted-foreground">Select a file</span>
                )}
                {saveStatus !== "idle" && (
                  <span className={cn(
                    "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px]",
                    saveStatus === "saved" 
                      ? "bg-green-500/10 text-green-600 dark:text-green-400"
                      : "bg-amber-500/10 text-amber-600 dark:text-amber-400"
                  )}>
                    {saveStatus === "saving" && <Loader2 className="w-2.5 h-2.5 animate-spin" />}
                    {saveStatus === "saved" && <CheckCircle2 className="w-2.5 h-2.5" />}
                    {saveStatus === "saving" ? "Saving..." : 
                     saveStatus === "unsaved" ? "Unsaved" : "Saved"}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-1">
                {/* Check button */}
                <button
                  onClick={handleCheck}
                  disabled={checkStatus === "checking"}
                  className={cn(
                    "flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-all",
                    checkStatus === "checking"
                      ? "text-muted-foreground cursor-not-allowed"
                      : "text-muted-foreground hover:text-foreground hover:bg-accent"
                  )}
                >
                  {checkStatus === "checking" ? (
                    <>
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                      Checking...
                    </>
                  ) : checkStatus === "success" ? (
                    <>
                      <CheckCircle2 className="w-3.5 h-3.5 text-green-600 dark:text-green-400" />
                      Connected
                    </>
                  ) : checkStatus === "error" ? (
                    <>
                      <XCircle className="w-3.5 h-3.5 text-red-600 dark:text-red-400" />
                      Failed
                    </>
                  ) : (
                    <>
                      <Play className="w-3.5 h-3.5" />
                      Check
                    </>
                  )}
                </button>
                {/* Edit in Luban button */}
                <button
                  onClick={handleEditInLuban}
                  className="flex items-center gap-1.5 px-2 py-1 rounded text-xs bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
                >
                  <Pencil className="w-3.5 h-3.5" />
                  Edit in Luban
                </button>
              </div>
            </div>
            
            {/* Editor area with highlighting overlay */}
            <div className="flex-1 relative overflow-hidden">
              {selectedFile ? (
                <>
                  {/* Syntax highlighting layer (behind) */}
                  <div
                    ref={highlightRef}
                    className="absolute inset-0 p-4 text-sm font-mono leading-relaxed whitespace-pre-wrap break-words overflow-hidden pointer-events-none"
                    aria-hidden="true"
                  >
                    <MarkdownHighlight text={currentContent} highlighter={highlighter} lang={getFileLanguage(selectedFile.name)} />
                  </div>
                  
                  {/* Actual textarea (transparent text, visible caret) */}
                  <textarea
                    ref={editorRef}
                    value={currentContent}
                    onChange={(e) => handleContentChange(e.target.value)}
                    onScroll={handleEditorScroll}
                    className="absolute inset-0 p-4 bg-transparent text-sm font-mono text-transparent caret-foreground leading-relaxed resize-none focus:outline-none"
                    spellCheck={false}
                  />
                </>
              ) : (
                <div className="flex-1 h-full flex items-center justify-center text-muted-foreground">
                  <div className="text-center">
                    <FileText className="w-8 h-8 mx-auto mb-2 opacity-30" />
                    <p className="text-xs">Select a file to edit</p>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// ============ Task Prompt Editor ============

type TaskType = "fix" | "implement" | "review" | "discuss" | "other" | "infer-type" | "rename-branch"

// System-level tasks that Luban depends on
const systemTaskTypes: TaskType[] = ["infer-type", "rename-branch"]

function isSystemTask(taskType: TaskType): boolean {
  return systemTaskTypes.includes(taskType)
}

interface TemplateVariable {
  id: string
  label: string
  description: string
  fullDescription: string
  example: string
}

const allTemplateVariables: TemplateVariable[] = [
  {
    id: "repo",
    label: "repo",
    description: "Repository name",
    fullDescription: "The full name of the repository (e.g., owner/repo-name)",
    example: "acme/web-app",
  },
  {
    id: "issue",
    label: "issue",
    description: "Issue details",
    fullDescription: "The full issue description including title, body, and comments",
    example: "#142: Login button not working",
  },
  {
    id: "pr",
    label: "pr",
    description: "Pull request info",
    fullDescription: "Pull request details including title, description, and diff",
    example: "PR #256: Add dark mode",
  },
]

const variablesByTaskType: Record<TaskType, string[]> = {
  "fix": ["repo", "issue"],
  "implement": ["repo", "issue"],
  "review": ["repo", "pr"],
  "discuss": ["repo", "issue"],
  "other": ["repo", "issue"],
  "infer-type": ["repo", "issue"],
  "rename-branch": ["repo", "issue"],
}

function getVariablesForTaskType(taskType: TaskType): TemplateVariable[] {
  const varIds = variablesByTaskType[taskType]
  return varIds.map(id => allTemplateVariables.find(v => v.id === id)!).filter(Boolean)
}

interface TaskTypeConfig {
  id: TaskType
  label: string
  icon: React.ElementType
  description: string
  defaultPrompt: string
}

const taskTypes: TaskTypeConfig[] = [
  {
    id: "fix",
    label: "Fix",
    icon: Bug,
    description: "Fix bugs or issues in the code",
    defaultPrompt: `You are fixing an issue in **{{repo}}**.

## Issue
{{issue}}

## Instructions
1. Analyze the problem and understand the root cause
2. Locate the relevant code
3. Implement a minimal fix
4. Verify with tests`,
  },
  {
    id: "implement",
    label: "Implement",
    icon: Lightbulb,
    description: "Implement new features or functionality",
    defaultPrompt: `You are implementing a feature in **{{repo}}**.

## Feature Request
{{issue}}

## Instructions
1. Understand the requirements
2. Design the solution
3. Implement the code
4. Write tests`,
  },
  {
    id: "review",
    label: "Review",
    icon: GitPullRequest,
    description: "Review Pull Request code changes",
    defaultPrompt: `You are reviewing code in **{{repo}}**.

## Pull Request
{{pr}}

## Review Checklist
1. Functional correctness
2. Code quality
3. Security concerns
4. Test coverage`,
  },
  {
    id: "discuss",
    label: "Discuss",
    icon: MessageSquare,
    description: "Discuss and explore ideas or questions",
    defaultPrompt: `You are discussing a topic related to **{{repo}}**.

## Topic
{{issue}}

## Guidelines
1. Explore the question thoroughly
2. Consider multiple perspectives
3. Provide insights and recommendations
4. Suggest next steps if applicable`,
  },
  {
    id: "other",
    label: "Other",
    icon: HelpCircle,
    description: "Other types of tasks",
    defaultPrompt: `You are working on **{{repo}}**.

## Task
{{issue}}

## Instructions
1. Understand the task
2. Create an execution plan
3. Complete step by step
4. Verify results`,
  },
]

// System-level tasks - these are special tasks that Luban depends on
const systemTaskTypes_config: TaskTypeConfig[] = [
  {
    id: "infer-type",
    label: "Infer Type",
    icon: Sparkle,
    description: "Infer task type from issue/PR content",
    defaultPrompt: `You are analyzing an issue or pull request in **{{repo}}** to determine the appropriate task type.

## Content to Analyze
{{issue}}

## Instructions
Analyze the content and determine the most appropriate task type:
- **fix**: Bug reports, error fixes, issue resolutions
- **implement**: Feature requests, new functionality
- **review**: Code review requests, PR reviews
- **discuss**: Questions, discussions, explorations

## Output Format
Return a JSON object with the following structure:
\`\`\`json
{
  "task_type": "fix" | "implement" | "review" | "discuss",
  "confidence": 0.0-1.0,
  "reasoning": "Brief explanation of why this type was chosen"
}
\`\`\``,
  },
  {
    id: "rename-branch",
    label: "Rename Branch",
    icon: GitBranch,
    description: "Generate branch name from task content",
    defaultPrompt: `You are generating a branch name for a task in **{{repo}}**.

## Task Content
{{issue}}

## Instructions
Generate a concise, descriptive branch name following these conventions:
- Use lowercase letters, numbers, and hyphens only
- Start with a prefix based on task type: \`fix/\`, \`feat/\`, \`docs/\`, \`refactor/\`
- Keep it under 50 characters
- Be descriptive but concise

## Output Format
Return a JSON object with the following structure:
\`\`\`json
{
  "branch_name": "fix/issue-123-login-button",
  "reasoning": "Brief explanation of the naming choice"
}
\`\`\``,
  },
]

// Combined task types for display
const allTaskTypes: TaskTypeConfig[] = [...taskTypes, ...systemTaskTypes_config]

let highlighterPromise: Promise<Highlighter> | null = null

function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ["github-light", "github-dark"],
      langs: ["markdown", "toml"],
    })
  }
  return highlighterPromise
}

function useShikiHighlighter() {
  const [highlighter, setHighlighter] = useState<Highlighter | null>(null)

  useEffect(() => {
    getHighlighter().then(setHighlighter)
  }, [])

  return highlighter
}

function MarkdownHighlight({ text, highlighter, lang = "markdown" }: { text: string; highlighter: Highlighter | null; lang?: string }) {
  const html = useMemo(() => {
    if (!highlighter) return null
    
    return highlighter.codeToHtml(text, {
      lang,
      themes: {
        light: "github-light",
        dark: "github-dark",
      },
    })
  }, [text, highlighter, lang])

  if (!html) {
    return <span className="text-foreground">{text}</span>
  }

  return (
    <div
      className="shiki-highlight [&_pre]:!bg-transparent [&_code]:!bg-transparent [&_.shiki]:!bg-transparent [&_pre]:!whitespace-pre-wrap [&_code]:!whitespace-pre-wrap [&_pre]:!break-words [&_code]:!break-words"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}

function TaskPromptEditor() {
  const [selectedType, setSelectedType] = useState<TaskType>("infer-type")
  const [typePrompts, setTypePrompts] = useState<Record<TaskType, string>>(() => {
    const initial: Record<string, string> = {}
    allTaskTypes.forEach(t => {
      initial[t.id] = t.defaultPrompt
    })
    return initial as Record<TaskType, string>
  })

  const editorRef = useRef<HTMLTextAreaElement>(null)
  const highlightRef = useRef<HTMLDivElement>(null)
  const highlighter = useShikiHighlighter()
  
  const [showAutocomplete, setShowAutocomplete] = useState(false)
  const [autocompletePosition, setAutocompletePosition] = useState({ top: 0, left: 0 })
  const [autocompleteFilter, setAutocompleteFilter] = useState("")
  const [selectedAutocompleteIndex, setSelectedAutocompleteIndex] = useState(0)

  const availableVariables = getVariablesForTaskType(selectedType)
  
  const filteredVariables = availableVariables.filter(v => 
    v.label.toLowerCase().includes(autocompleteFilter.toLowerCase()) ||
    v.description.toLowerCase().includes(autocompleteFilter.toLowerCase())
  )

  const handleEditorScroll = () => {
    if (!editorRef.current || !highlightRef.current) return
    highlightRef.current.scrollTop = editorRef.current.scrollTop
    highlightRef.current.scrollLeft = editorRef.current.scrollLeft
    setShowAutocomplete(false)
  }

  const insertVariable = (variableId: string) => {
    const editor = editorRef.current
    if (!editor) return
    
    const start = editor.selectionStart
    const end = editor.selectionEnd
    const text = typePrompts[selectedType]
    
    let insertText = `{{${variableId}}}`
    let newCursorPos = start + insertText.length
    
    if (showAutocomplete) {
      const beforeCursor = text.slice(0, start)
      const triggerMatch = beforeCursor.match(/\{\{([^}]*)$/)
      if (triggerMatch) {
        const triggerStart = start - triggerMatch[0].length
        const newText = text.slice(0, triggerStart) + insertText + text.slice(end)
        setTypePrompts(prev => ({ ...prev, [selectedType]: newText }))
        newCursorPos = triggerStart + insertText.length
        setShowAutocomplete(false)
        setAutocompleteFilter("")
        
        requestAnimationFrame(() => {
          editor.focus()
          editor.setSelectionRange(newCursorPos, newCursorPos)
        })
        return
      }
    }
    
    const newText = text.slice(0, start) + insertText + text.slice(end)
    setTypePrompts(prev => ({ ...prev, [selectedType]: newText }))
    
    requestAnimationFrame(() => {
      editor.focus()
      editor.setSelectionRange(newCursorPos, newCursorPos)
    })
  }

  const handleEditorChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = e.target.value
    const cursorPos = e.target.selectionStart
    setTypePrompts(prev => ({ ...prev, [selectedType]: newValue }))
    
    const beforeCursor = newValue.slice(0, cursorPos)
    const triggerMatch = beforeCursor.match(/\{\{([^}\s]*)$/)
    
    if (triggerMatch) {
      setAutocompleteFilter(triggerMatch[1])
      setSelectedAutocompleteIndex(0)
      
      const textarea = editorRef.current
      if (textarea) {
        const lines = beforeCursor.split('\n')
        const currentLineIndex = lines.length - 1
        const currentLineStart = beforeCursor.lastIndexOf('\n') + 1
        const charInLine = cursorPos - currentLineStart
        
        const lineHeight = 20
        const charWidth = 7.2
        const paddingTop = 12
        const paddingLeft = 12
        
        setAutocompletePosition({
          top: paddingTop + (currentLineIndex + 1) * lineHeight - textarea.scrollTop,
          left: paddingLeft + charInLine * charWidth - triggerMatch[0].length * charWidth,
        })
        setShowAutocomplete(true)
      }
    } else {
      setShowAutocomplete(false)
      setAutocompleteFilter("")
    }
  }

  const handleEditorKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (!showAutocomplete) return
    
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedAutocompleteIndex(prev => 
        prev < filteredVariables.length - 1 ? prev + 1 : prev
      )
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedAutocompleteIndex(prev => prev > 0 ? prev - 1 : 0)
    } else if (e.key === 'Enter' || e.key === 'Tab') {
      if (filteredVariables.length > 0) {
        e.preventDefault()
        insertVariable(filteredVariables[selectedAutocompleteIndex].id)
      }
    } else if (e.key === 'Escape') {
      setShowAutocomplete(false)
      setAutocompleteFilter("")
    }
  }

  const currentPrompt = typePrompts[selectedType]
  const currentTaskType = allTaskTypes.find(t => t.id === selectedType)!
  const isCurrentSystemTask = isSystemTask(selectedType)

  return (
    <div className="border border-border rounded-lg overflow-hidden bg-sidebar">
      <div className="flex h-[380px]">
        {/* Left sidebar - Task type list */}
        <div className="w-44 border-r border-border flex flex-col">
          <div className="flex items-center gap-2 h-11 px-3 border-b border-border">
            <ClipboardType className="w-4 h-4 text-muted-foreground" />
            <span className="text-sm font-medium text-muted-foreground">Type</span>
          </div>
          <div className="flex-1 overflow-y-auto py-1.5">
            {/* System task types header */}
            <div className="flex items-center gap-2 px-3 py-1.5">
              <ShieldCheck className="w-3 h-3 text-muted-foreground/60" />
              <span className="text-[10px] uppercase tracking-wider text-muted-foreground/60 font-medium">System</span>
            </div>
            
            {/* System task types */}
            {systemTaskTypes_config.map((taskType) => {
              const Icon = taskType.icon
              const isSelected = selectedType === taskType.id

              return (
                <button
                  key={taskType.id}
                  onClick={() => setSelectedType(taskType.id)}
                  className={cn(
                    "w-full flex items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors",
                    isSelected
                      ? "bg-amber-500/15 text-amber-600 dark:text-amber-400"
                      : "text-muted-foreground hover:text-foreground hover:bg-accent"
                  )}
                >
                  <Icon className={cn("w-4 h-4 shrink-0", isSelected ? "text-amber-500" : "text-muted-foreground")} />
                  <span className="truncate">{taskType.label}</span>
                </button>
              )
            })}
            
            {/* User task types divider */}
            <div className="flex items-center gap-2 px-3 py-1.5 mt-2 border-t border-border pt-3">
              <UserPen className="w-3 h-3 text-muted-foreground/60" />
              <span className="text-[10px] uppercase tracking-wider text-muted-foreground/60 font-medium">User</span>
            </div>
            
            {/* User task types */}
            {taskTypes.map((taskType) => {
              const Icon = taskType.icon
              const isSelected = selectedType === taskType.id

              return (
                <button
                  key={taskType.id}
                  onClick={() => setSelectedType(taskType.id)}
                  className={cn(
                    "w-full flex items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors",
                    isSelected
                      ? "bg-primary/15 text-primary"
                      : "text-muted-foreground hover:text-foreground hover:bg-accent"
                  )}
                >
                  <Icon className={cn("w-4 h-4 shrink-0", isSelected ? "text-primary" : "text-muted-foreground")} />
                  <span className="truncate">{taskType.label}</span>
                </button>
              )
            })}
          </div>
        </div>

        {/* Right: Editor with syntax highlighting */}
        <div className="flex-1 flex flex-col min-w-0 bg-background">
          {/* Editor header */}
          <div className="flex items-center justify-between h-11 px-3 border-b border-border">
            <div className="flex items-center gap-2">
              <currentTaskType.icon className="w-4 h-4 text-primary" />
              <span className="text-sm font-medium">{currentTaskType.label}</span>
            </div>
            <div className="flex items-center gap-1">
              <button
                onClick={() => setTypePrompts(prev => ({ ...prev, [selectedType]: currentTaskType.defaultPrompt }))}
                className="flex items-center gap-1.5 px-2 py-1 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
              >
                <RefreshCw className="w-3.5 h-3.5" />
                Reset
              </button>
              <button
                onClick={() => {}}
                className="flex items-center gap-1.5 px-2 py-1 rounded text-xs bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
              >
                <Pencil className="w-3.5 h-3.5" />
                Edit in Luban
              </button>
            </div>
          </div>
          
          {/* System task warning banner */}
          {isCurrentSystemTask && (
            <div className="flex items-start gap-2.5 px-3 py-2.5 bg-amber-500/10 border-b border-amber-500/20">
              <AlertTriangle className="w-4 h-4 text-amber-500 shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <p className="text-xs text-amber-700 dark:text-amber-400 leading-relaxed">
                  <span className="font-medium">System Prompt</span> — Luban&apos;s core functionality depends on this prompt. Please avoid modifying unless you have specific requirements.
                </p>
              </div>
            </div>
          )}
          
          {/* Editor area with highlighting overlay */}
          <div className="flex-1 relative overflow-hidden">
            {/* Syntax highlighting layer (behind) */}
            <div
              ref={highlightRef}
              className="absolute inset-0 p-4 text-sm font-mono leading-relaxed whitespace-pre-wrap break-words overflow-auto pointer-events-none"
              aria-hidden="true"
            >
              <MarkdownHighlight text={currentPrompt} highlighter={highlighter} />
            </div>
            
            {/* Actual textarea (transparent text, visible caret) */}
            <textarea
              ref={editorRef}
              value={currentPrompt}
              onChange={handleEditorChange}
              onKeyDown={handleEditorKeyDown}
              onScroll={handleEditorScroll}
              onBlur={() => setTimeout(() => setShowAutocomplete(false), 150)}
              className="absolute inset-0 w-full h-full bg-transparent text-transparent caret-foreground text-sm font-mono leading-relaxed resize-none focus:outline-none p-4 selection:bg-primary/20 selection:text-transparent overflow-auto"
              wrap="soft"
              spellCheck={false}
              placeholder="Enter prompt template..."
            />
            
            {/* Autocomplete dropdown */}
            {showAutocomplete && filteredVariables.length > 0 && (
              <div 
                className="absolute z-50 bg-popover border border-border rounded-lg shadow-lg overflow-hidden"
                style={{ top: autocompletePosition.top, left: autocompletePosition.left }}
              >
                <div className="max-h-48 overflow-y-auto p-1">
                  {filteredVariables.map((variable, idx) => (
                    <button
                      key={variable.id}
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={() => insertVariable(variable.id)}
                      className={cn(
                        "w-full flex items-center gap-3 px-2.5 py-1.5 text-left text-xs transition-colors rounded-md",
                        idx === selectedAutocompleteIndex
                          ? "bg-primary/10 text-primary"
                          : "hover:bg-accent text-foreground"
                      )}
                    >
                      <span className="font-mono text-primary bg-primary/10 px-1.5 py-0.5 rounded">
                        {variable.label}
                      </span>
                      <span className="text-muted-foreground truncate">{variable.description}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}

function TaskSettings() {
  return <TaskPromptEditor />
}

function AllSettings() {
  const [theme, setTheme] = useState("system")
  const [uiFont, setUiFont] = useState("Inter")
  const [chatFont, setChatFont] = useState("Inter")
  const [monoFont, setMonoFont] = useState("Geist Mono")
  const [terminalFont, setTerminalFont] = useState("Geist Mono")

  return (
    <div className="space-y-12">
      {/* Theme section */}
      <section id="theme" className="scroll-mt-8">
        <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
          <Sun className="w-4 h-4 text-muted-foreground" />
          Theme
        </h3>
        <div className="flex gap-4">
          {themeOptions.map((option) => (
            <ThemePreviewCard
              key={option.id}
              themeId={option.id}
              label={option.label}
              icon={option.icon}
              isSelected={theme === option.id}
              onClick={() => setTheme(option.id)}
            />
          ))}
        </div>
      </section>

      {/* Fonts section */}
      <section id="fonts" className="scroll-mt-8">
        <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
          <Type className="w-4 h-4 text-muted-foreground" />
          Fonts
        </h3>
        <WorkspacePreviewWithFonts
          uiFont={uiFont}
          setUiFont={setUiFont}
          chatFont={chatFont}
          setChatFont={setChatFont}
          monoFont={monoFont}
          setMonoFont={setMonoFont}
          terminalFont={terminalFont}
          setTerminalFont={setTerminalFont}
        />
      </section>

      {/* Agent section */}
      <section id="agent" className="scroll-mt-8">
        <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
          <Bot className="w-4 h-4 text-muted-foreground" />
          Agent
        </h3>
        <div className="space-y-4">
          <DefaultAgentSelector />
          <CodexSettings />
        </div>
      </section>

      {/* Task section */}
      <section id="task" className="scroll-mt-8">
        <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
          <ListTodo className="w-4 h-4 text-muted-foreground" />
          Task
        </h3>
        <TaskSettings />
      </section>
    </div>
  )
}

export function SettingsPanel({ open, onOpenChange }: SettingsPanelProps) {
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set(["appearance"]))
  const [activeItem, setActiveItem] = useState<string>("theme")
  const contentRef = useRef<HTMLDivElement>(null)

  if (!open) return null

  const toggleExpanded = (id: string) => {
    setExpandedItems((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  const scrollToSection = (id: string) => {
    setActiveItem(id)
    const element = document.getElementById(id)
    if (element && contentRef.current) {
      element.scrollIntoView({ behavior: "smooth", block: "start" })
    }
  }

  return (
    <div className="fixed inset-0 z-50 bg-background flex">
      {/* Left sidebar */}
      <div className="w-60 flex-shrink-0 border-r border-border bg-sidebar flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between h-11 px-3 border-b border-border">
          <div className="flex items-center gap-2 px-1.5 py-1">
            <div className="flex items-center justify-center w-6 h-6 rounded bg-secondary">
              <Settings className="w-4 h-4 text-foreground" />
            </div>
            <span className="text-sm font-medium">Settings</span>
          </div>
          <button
            onClick={() => onOpenChange(false)}
            className="p-1.5 text-muted-foreground hover:text-foreground hover:bg-sidebar-accent rounded transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Navigation - TOC style */}
        <div className="flex-1 overflow-y-auto py-1.5">
          {tocItems.map((item) => {
            const Icon = item.icon
            const isExpanded = expandedItems.has(item.id)
            const hasChildren = item.children && item.children.length > 0

            return (
              <div key={item.id}>
                {/* Parent item */}
                <div className="flex items-center hover:bg-sidebar-accent/50 transition-colors">
                  <button
                    onClick={() => hasChildren ? toggleExpanded(item.id) : scrollToSection(item.id)}
                    className="flex-1 flex items-center gap-2 px-3 py-1.5 text-left"
                  >
                    {hasChildren ? (
                      isExpanded ? (
                        <ChevronDown className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                      ) : (
                        <ChevronRight className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                      )
                    ) : (
                      <Icon className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                    )}
                    <span className={cn(
                      "text-sm truncate flex-1",
                      !hasChildren && activeItem === item.id ? "text-foreground" : "text-muted-foreground"
                    )}>{item.label}</span>
                  </button>
                </div>

                {/* Children items */}
                {isExpanded && hasChildren && (
                  <div className="ml-4 pl-3 border-l border-border-subtle">
                    {item.children!.map((child) => {
                      const isActive = activeItem === child.id
                      const ChildIcon = child.icon
                      return (
                        <button
                          key={child.id}
                          onClick={() => scrollToSection(child.id)}
                          className={cn(
                            "w-full flex items-center gap-2 px-2 py-1.5 text-left transition-colors rounded mx-1",
                            isActive
                              ? "bg-sidebar-accent"
                              : "hover:bg-sidebar-accent/30"
                          )}
                        >
                          <ChildIcon className={cn(
                            "w-3.5 h-3.5",
                            isActive ? "text-primary" : "text-muted-foreground"
                          )} />
                          <span className={cn(
                            "text-xs",
                            isActive ? "text-foreground" : "text-muted-foreground"
                          )}>
                            {child.label}
                          </span>
                        </button>
                      )
                    })}
                  </div>
                )}
              </div>
            )
          })}
        </div>

        {/* Footer */}
        <div className="border-t border-border p-2">
          <div className="px-3 py-1.5 text-xs text-muted-foreground">
            Luban v0.1.2
          </div>
        </div>
      </div>

      {/* Main content */}
      <div className="flex-1 overflow-hidden flex flex-col">
        {/* Content header */}
        <div className="h-11 px-8 border-b border-border flex items-center">
          <h2 className="text-sm font-medium">Settings</h2>
        </div>

        {/* Scrollable content */}
        <div ref={contentRef} className="flex-1 overflow-y-auto p-8">
          <div className="max-w-4xl">
            <AllSettings />
          </div>
        </div>
      </div>
    </div>
  )
}
