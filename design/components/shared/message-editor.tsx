"use client"

import type React from "react"
import { useState, useRef, useEffect, useLayoutEffect, useCallback, useMemo } from "react"
import {
  Send,
  X,
  Paperclip,
  ImageIcon,
  FileText,
  FileCode,
  File,
  Folder,
} from "lucide-react"
import { cn } from "@/lib/utils"
import {
  ControlledAgentSelector,
  useAgentSelector,
} from "./agent-selector"
import type { MessageAttachment } from "./chat-message"

// ============================================================================
// Types
// ============================================================================

interface Command {
  id: string
  label: string
  description: string
}

interface FileItem {
  id: string
  name: string
  path: string
  type: "file" | "folder"
  icon?: React.ReactNode
}

interface MessageEditorProps {
  value: string
  onChange: (value: string) => void
  attachments: MessageAttachment[]
  onAttachmentsChange: (attachments: MessageAttachment[]) => void
  onSubmit: () => void
  onCancel?: () => void
  placeholder?: string
  autoFocus?: boolean
  className?: string
  // New props for history navigation
  messageHistory?: string[]
  onCommand?: (commandId: string) => void
}

// ============================================================================
// Mock Data
// ============================================================================

const COMMANDS: Command[] = [
  { id: "abc", label: "abc", description: "Example command abc" },
  { id: "def", label: "def", description: "Example command def" },
  { id: "ghi", label: "ghi", description: "Example command ghi" },
  { id: "jkl", label: "jkl", description: "Example command jkl" },
  { id: "mno", label: "mno", description: "Example command mno" },
]

const MOCK_FILES: FileItem[] = [
  { id: "d1", name: "components", path: "components", type: "folder" },
  { id: "d2", name: "shared", path: "components/shared", type: "folder" },
  { id: "d3", name: "app", path: "app", type: "folder" },
  { id: "d4", name: "lib", path: "lib", type: "folder" },
  { id: "d5", name: "ui", path: "components/ui", type: "folder" },
  { id: "f1", name: "message-editor.tsx", path: "components/shared/message-editor.tsx", type: "file" },
  { id: "f2", name: "chat-panel.tsx", path: "components/chat-panel.tsx", type: "file" },
  { id: "f3", name: "sidebar.tsx", path: "components/sidebar.tsx", type: "file" },
  { id: "f4", name: "agent-selector.tsx", path: "components/shared/agent-selector.tsx", type: "file" },
  { id: "f5", name: "kanban-board.tsx", path: "components/kanban-board.tsx", type: "file" },
  { id: "f6", name: "activity-item.tsx", path: "components/shared/activity-item.tsx", type: "file" },
  { id: "f7", name: "globals.css", path: "app/globals.css", type: "file" },
  { id: "f8", name: "layout.tsx", path: "app/layout.tsx", type: "file" },
  { id: "f9", name: "page.tsx", path: "app/page.tsx", type: "file" },
  { id: "f10", name: "utils.ts", path: "lib/utils.ts", type: "file" },
]

// ============================================================================
// Helper Components
// ============================================================================

function getFileIcon(item: FileItem) {
  if (item.type === "folder") return <Folder className="w-3.5 h-3.5 text-base0d" />
  
  const ext = item.name.split(".").pop()?.toLowerCase()
  switch (ext) {
    case "tsx":
    case "ts":
      return <FileCode className="w-3.5 h-3.5 text-base0d" />
    case "css":
      return <FileText className="w-3.5 h-3.5 text-base0e" />
    case "json":
      return <FileCode className="w-3.5 h-3.5 text-base09" />
    default:
      return <File className="w-3.5 h-3.5 text-muted-foreground" />
  }
}

// ============================================================================
// Command Menu Component
// ============================================================================

interface CommandMenuProps {
  query: string
  selectedIndex: number
  onSelect: (command: Command) => void
  onClose: () => void
}

function CommandMenu({ query, selectedIndex, onSelect, onClose }: CommandMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null)
  
  const filteredCommands = useMemo(() => {
    if (!query) return COMMANDS
    const q = query.toLowerCase()
    return COMMANDS.filter(
      cmd => cmd.label.toLowerCase().includes(q) || cmd.description.toLowerCase().includes(q)
    )
  }, [query])

  useEffect(() => {
    if (selectedIndex === 0) {
      menuRef.current?.scrollTo({ top: 0 })
    } else {
      const item = menuRef.current?.querySelector(`[data-index="${selectedIndex}"]`)
      item?.scrollIntoView({ block: "nearest" })
    }
  }, [selectedIndex])

  if (filteredCommands.length === 0) {
    return (
      <div className="absolute bottom-full left-0 right-0 mb-2 bg-popover border border-border rounded-lg shadow-xl overflow-hidden z-50">
        <div className="px-3 py-4 text-center text-sm text-muted-foreground">
          No commands found
        </div>
      </div>
    )
  }

  return (
    <>
      <div className="fixed inset-0 z-40" onClick={onClose} />
      <div 
        ref={menuRef}
        className="absolute bottom-full left-0 right-0 mb-2 bg-popover border border-border rounded-lg shadow-xl overflow-hidden z-50 max-h-[280px] overflow-y-auto"
      >
        <div className="px-2 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground border-b border-border bg-muted/30">
          Commands
        </div>
        <div className="py-1">
          {filteredCommands.map((cmd, idx) => (
            <button
              key={cmd.id}
              data-index={idx}
              onClick={() => onSelect(cmd)}
              onMouseDown={(e) => e.preventDefault()}
              className={cn(
                "w-full flex items-center gap-3 px-3 py-2 text-left transition-colors",
                idx === selectedIndex ? "bg-primary/10 text-primary" : "hover:bg-muted/50"
              )}
            >
              <div className="flex-1 min-w-0">
                <span className="text-sm font-medium">/{cmd.label}</span>
                <span className="text-xs text-muted-foreground ml-2">{cmd.description}</span>
              </div>
            </button>
          ))}
        </div>
      </div>
    </>
  )
}

// ============================================================================
// Mention Menu Component
// ============================================================================

interface MentionMenuProps {
  query: string
  selectedIndex: number
  onSelect: (item: FileItem) => void
  onClose: () => void
  position: { top: number; left: number } | null
}

function MentionMenu({ query, selectedIndex, onSelect, onClose, position }: MentionMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null)
  
  // Filter and sort: folders first, then files
  const filteredItems = useMemo(() => {
    // Don't search when query is empty - show prompt instead
    if (!query) return []
    
    const q = query.toLowerCase()
    const items = MOCK_FILES.filter(
      item => item.name.toLowerCase().includes(q) || item.path.toLowerCase().includes(q)
    )
    // Sort: folders first, then files
    const folders = items.filter(i => i.type === "folder")
    const files = items.filter(i => i.type === "file")
    return [...folders, ...files].slice(0, 10)
  }, [query])

  useEffect(() => {
    if (selectedIndex === 0) {
      menuRef.current?.scrollTo({ top: 0 })
    } else {
      const item = menuRef.current?.querySelector(`[data-index="${selectedIndex}"]`)
      item?.scrollIntoView({ block: "nearest" })
    }
  }, [selectedIndex])

  // Show prompt when query is empty (user just typed @)
  if (!query) {
    return (
      <>
        <div className="fixed inset-0 z-40" onClick={onClose} />
        <div className="absolute bottom-full left-0 right-0 mb-2 bg-popover border border-border rounded-lg shadow-xl overflow-hidden z-50">
          <div className="px-2 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground border-b border-border bg-muted/30">
            Reference files
          </div>
          <div className="px-3 py-3 text-sm text-muted-foreground">
            Type to search files...
          </div>
        </div>
      </>
    )
  }

  if (filteredItems.length === 0) {
    return (
      <>
        <div className="fixed inset-0 z-40" onClick={onClose} />
        <div className="absolute bottom-full left-0 right-0 mb-2 bg-popover border border-border rounded-lg shadow-xl overflow-hidden z-50">
          <div className="px-2 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground border-b border-border bg-muted/30">
            Reference files
          </div>
          <div className="px-3 py-4 text-center text-sm text-muted-foreground">
            No files found
          </div>
        </div>
      </>
    )
  }

  return (
    <>
      <div className="fixed inset-0 z-40" onClick={onClose} />
      <div 
        ref={menuRef}
        className="absolute bottom-full left-0 right-0 mb-2 bg-popover border border-border rounded-lg shadow-xl overflow-hidden z-50 max-h-[320px] overflow-y-auto"
      >
        <div className="px-2 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground border-b border-border bg-muted/30">
          Reference files
        </div>
        <div className="py-1">
          {filteredItems.map((item, idx) => (
            <button
              key={item.id}
              data-index={idx}
              onClick={() => onSelect(item)}
              onMouseDown={(e) => e.preventDefault()}
              className={cn(
                "w-full flex items-center gap-2.5 px-3 py-1.5 text-left transition-colors",
                idx === selectedIndex ? "bg-primary/10 text-primary" : "hover:bg-muted/50"
              )}
            >
              {getFileIcon(item)}
              <div className="flex-1 min-w-0">
                <span className="text-sm truncate block">{item.name}</span>
                <span className="text-[11px] text-muted-foreground truncate block">{item.path}</span>
              </div>
            </button>
          ))}
        </div>
      </div>
    </>
  )
}

// ============================================================================
// Main Component
// ============================================================================

export function MessageEditor({
  value,
  onChange,
  attachments,
  onAttachmentsChange,
  onSubmit,
  onCancel,
  placeholder = "Let's chart the cosmos of ideas...",
  autoFocus = false,
  className,
  messageHistory = [],
  onCommand,
}: MessageEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const [isFocused, setIsFocused] = useState(false)
  const [isDragging, setIsDragging] = useState(false)
  const agentSelector = useAgentSelector()

  // Command menu state
  const [showCommandMenu, setShowCommandMenu] = useState(false)
  const [commandQuery, setCommandQuery] = useState("")
  const [commandSelectedIndex, setCommandSelectedIndex] = useState(0)

  // Mention menu state
  const [showMentionMenu, setShowMentionMenu] = useState(false)
  const [mentionQuery, setMentionQuery] = useState("")
  const [mentionSelectedIndex, setMentionSelectedIndex] = useState(0)
  const [mentionStartPos, setMentionStartPos] = useState<number | null>(null)

  // History navigation state
  const [historyIndex, setHistoryIndex] = useState(-1)
  const [savedInput, setSavedInput] = useState("")

  // ============================================================================
  // Auto-resize textarea (scrollHeight-based)
  // ============================================================================
  useLayoutEffect(() => {
    const el = textareaRef.current
    if (el) {
      el.style.height = "auto"
      el.style.height = `${Math.min(el.scrollHeight, 160)}px`
    }
  }, [value])

  // ============================================================================
  // Autofocus
  // ============================================================================
  useEffect(() => {
    if (autoFocus && textareaRef.current) {
      textareaRef.current.focus()
      textareaRef.current.select()
    }
  }, [autoFocus])

  // ============================================================================
  // Filter counts for menu navigation
  // ============================================================================
  const filteredCommandCount = useMemo(() => {
    if (!commandQuery) return COMMANDS.length
    const q = commandQuery.toLowerCase()
    return COMMANDS.filter(
      cmd => cmd.label.toLowerCase().includes(q) || cmd.description.toLowerCase().includes(q)
    ).length
  }, [commandQuery])

  const filteredMentionCount = useMemo(() => {
    // Return 0 when query is empty - no items to navigate
    if (!mentionQuery) return 0
    const q = mentionQuery.toLowerCase()
    return MOCK_FILES.filter(
      item => item.name.toLowerCase().includes(q) || item.path.toLowerCase().includes(q)
    ).slice(0, 10).length
  }, [mentionQuery])

  // ============================================================================
  // Handle input change with @ and / detection
  // ============================================================================
  const handleChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = e.target.value
    const cursorPos = e.target.selectionStart
    onChange(newValue)

    // Reset history navigation when typing
    setHistoryIndex(-1)

    // Check for / at start of input
    if (newValue.startsWith("/")) {
      setShowCommandMenu(true)
      setCommandQuery(newValue.slice(1).split(" ")[0] || "")
      setCommandSelectedIndex(0)
      setShowMentionMenu(false)
    } else {
      setShowCommandMenu(false)
    }

    // Check for @ trigger
    const textBeforeCursor = newValue.slice(0, cursorPos)
    const lastAtIndex = textBeforeCursor.lastIndexOf("@")
    
    if (lastAtIndex !== -1) {
      const textAfterAt = textBeforeCursor.slice(lastAtIndex + 1)
      // Only show menu if @ is at start or preceded by whitespace, and no space after @
      const charBeforeAt = lastAtIndex > 0 ? newValue[lastAtIndex - 1] : " "
      if ((charBeforeAt === " " || charBeforeAt === "\n" || lastAtIndex === 0) && !textAfterAt.includes(" ")) {
        setShowMentionMenu(true)
        setMentionQuery(textAfterAt)
        setMentionSelectedIndex(0)
        setMentionStartPos(lastAtIndex)
      } else {
        setShowMentionMenu(false)
        setMentionStartPos(null)
      }
    } else {
      setShowMentionMenu(false)
      setMentionStartPos(null)
    }
  }, [onChange])

  // ============================================================================
  // Command selection
  // ============================================================================
  const handleCommandSelect = useCallback((command: Command) => {
    setShowCommandMenu(false)
    setCommandQuery("")
    onChange("")
    onCommand?.(command.id)
  }, [onChange, onCommand])

  // ============================================================================
  // Mention selection
  // ============================================================================
  const handleMentionSelect = useCallback((item: FileItem) => {
    if (mentionStartPos === null) return
    
    const before = value.slice(0, mentionStartPos)
    const after = value.slice(textareaRef.current?.selectionStart || mentionStartPos)
    const mention = `@${item.path} `
    const newValue = before + mention + after
    
    onChange(newValue)
    setShowMentionMenu(false)
    setMentionQuery("")
    setMentionStartPos(null)

    // Set cursor position after mention
    setTimeout(() => {
      const newPos = before.length + mention.length
      textareaRef.current?.setSelectionRange(newPos, newPos)
      textareaRef.current?.focus()
    }, 0)
  }, [value, mentionStartPos, onChange])

  // ============================================================================
  // Keyboard handling
  // ============================================================================
  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Command menu navigation
    if (showCommandMenu) {
      if (e.key === "ArrowDown") {
        e.preventDefault()
        setCommandSelectedIndex(i => Math.min(i + 1, filteredCommandCount - 1))
        return
      }
      if (e.key === "ArrowUp") {
        e.preventDefault()
        setCommandSelectedIndex(i => Math.max(i - 1, 0))
        return
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault()
        const q = commandQuery.toLowerCase()
        const filtered = COMMANDS.filter(
          cmd => cmd.label.toLowerCase().includes(q) || cmd.description.toLowerCase().includes(q)
        )
        if (filtered[commandSelectedIndex]) {
          handleCommandSelect(filtered[commandSelectedIndex])
        }
        return
      }
      if (e.key === "Escape") {
        e.preventDefault()
        setShowCommandMenu(false)
        onChange("")
        return
      }
    }

    // Mention menu navigation
    if (showMentionMenu) {
      if (e.key === "ArrowDown") {
        e.preventDefault()
        setMentionSelectedIndex(i => Math.min(i + 1, filteredMentionCount - 1))
        return
      }
      if (e.key === "ArrowUp") {
        e.preventDefault()
        setMentionSelectedIndex(i => Math.max(i - 1, 0))
        return
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault()
        const q = mentionQuery.toLowerCase()
        const filtered = MOCK_FILES.filter(
          item => item.name.toLowerCase().includes(q) || item.path.toLowerCase().includes(q)
        ).slice(0, 10)
        if (filtered[mentionSelectedIndex]) {
          handleMentionSelect(filtered[mentionSelectedIndex])
        }
        return
      }
      if (e.key === "Escape") {
        e.preventDefault()
        setShowMentionMenu(false)
        setMentionStartPos(null)
        return
      }
    }

    // History navigation (↑↓ when not in menu)
    if (e.key === "ArrowUp" && !showCommandMenu && !showMentionMenu) {
      const el = textareaRef.current
      if (el) {
        // Only trigger if cursor is at the beginning or input is empty
        const atStart = el.selectionStart === 0 && el.selectionEnd === 0
        const isEmpty = value === ""
        if ((atStart || isEmpty) && messageHistory.length > 0) {
          e.preventDefault()
          if (historyIndex === -1) {
            setSavedInput(value)
          }
          const newIndex = Math.min(historyIndex + 1, messageHistory.length - 1)
          setHistoryIndex(newIndex)
          onChange(messageHistory[messageHistory.length - 1 - newIndex])
          return
        }
      }
    }

    if (e.key === "ArrowDown" && !showCommandMenu && !showMentionMenu) {
      const el = textareaRef.current
      if (el && historyIndex >= 0) {
        // Check if cursor is at the end
        const atEnd = el.selectionStart === value.length
        if (atEnd || value === messageHistory[messageHistory.length - 1 - historyIndex]) {
          e.preventDefault()
          const newIndex = historyIndex - 1
          if (newIndex < 0) {
            setHistoryIndex(-1)
            onChange(savedInput)
          } else {
            setHistoryIndex(newIndex)
            onChange(messageHistory[messageHistory.length - 1 - newIndex])
          }
          return
        }
      }
    }

    // Submit on Enter (without shift)
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault()
      onSubmit()
      setHistoryIndex(-1)
      setSavedInput("")
    } else if (e.key === "Escape" && onCancel) {
      onCancel()
    }
  }, [
    showCommandMenu, showMentionMenu, commandQuery, mentionQuery,
    commandSelectedIndex, mentionSelectedIndex, filteredCommandCount, filteredMentionCount,
    handleCommandSelect, handleMentionSelect, onChange, onSubmit, onCancel,
    value, messageHistory, historyIndex, savedInput
  ])

  // ============================================================================
  // File handling
  // ============================================================================
  const handleFileSelect = (files: FileList | null) => {
    if (!files) return
    const newAttachments: MessageAttachment[] = Array.from(files).map((file) => ({
      id: Math.random().toString(36).slice(2),
      type: file.type.startsWith("image/") ? "image" : "file",
      name: file.name,
      size: file.size,
      preview: file.type.startsWith("image/") ? URL.createObjectURL(file) : undefined,
    }))
    onAttachmentsChange([...attachments, ...newAttachments])
  }

  const removeAttachment = (id: string) => {
    onAttachmentsChange(attachments.filter((a) => a.id !== id))
  }

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault()
    setIsDragging(true)
  }

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault()
    setIsDragging(false)
  }

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault()
    setIsDragging(false)
    handleFileSelect(e.dataTransfer.files)
  }

  const handlePaste = (e: React.ClipboardEvent) => {
    const items = e.clipboardData.items
    const imageItems = Array.from(items).filter((item) => item.type.startsWith("image/"))
    if (imageItems.length > 0) {
      e.preventDefault()
      const files = imageItems
        .map((item) => item.getAsFile())
        .filter((file): file is File => file !== null)
      if (files.length > 0) {
        const dataTransfer = new DataTransfer()
        files.forEach((file) => dataTransfer.items.add(file))
        handleFileSelect(dataTransfer.files)
      }
    }
  }

  // ============================================================================
  // Render
  // ============================================================================
  return (
    <div
      className={cn(
        "relative bg-background border rounded-lg shadow-lg transition-all",
        isFocused ? "border-primary/50 ring-1 ring-primary/20 shadow-xl" : "border-border",
        isDragging && "border-primary ring-2 ring-primary/30 bg-primary/5",
        className,
      )}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {/* Command Menu */}
      {showCommandMenu && (
        <CommandMenu
          query={commandQuery}
          selectedIndex={commandSelectedIndex}
          onSelect={handleCommandSelect}
          onClose={() => {
            setShowCommandMenu(false)
            onChange("")
          }}
        />
      )}

      {/* Mention Menu */}
      {showMentionMenu && (
        <MentionMenu
          query={mentionQuery}
          selectedIndex={mentionSelectedIndex}
          onSelect={handleMentionSelect}
          onClose={() => {
            setShowMentionMenu(false)
            setMentionStartPos(null)
          }}
          position={null}
        />
      )}

      {/* Drag overlay */}
      {isDragging && (
        <div className="absolute inset-0 z-10 flex items-center justify-center bg-primary/5 rounded-lg border-2 border-dashed border-primary">
          <div className="flex flex-col items-center gap-2 text-primary">
            <ImageIcon className="w-8 h-8" />
            <span className="text-sm font-medium">Drop files here</span>
          </div>
        </div>
      )}

      {/* Attachments */}
      {attachments.length > 0 && (
        <div className="px-3 pt-3 pb-1 flex flex-wrap gap-3">
          {attachments.map((attachment) => (
            <div key={attachment.id} className="group relative">
              <div className="relative">
                <div className="w-20 h-20 rounded-lg overflow-hidden border border-border/50 hover:border-border transition-colors bg-muted/40 flex items-center justify-center">
                  {attachment.type === "image" && attachment.preview ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={attachment.preview}
                      alt={attachment.name}
                      className="w-full h-full object-cover"
                    />
                  ) : (
                    <div className="flex flex-col items-center gap-1.5">
                      {attachment.name.endsWith(".pdf") ? (
                        <FileText className="w-6 h-6 text-base08" />
                      ) : attachment.name.endsWith(".txt") || attachment.name.endsWith(".md") ? (
                        <FileText className="w-6 h-6 text-muted-foreground" />
                      ) : attachment.name.endsWith(".json") ? (
                        <FileCode className="w-6 h-6 text-base09" />
                      ) : (
                        <FileText className="w-6 h-6 text-muted-foreground" />
                      )}
                      <span className="text-[9px] text-muted-foreground uppercase font-medium tracking-wide">
                        {attachment.name.split(".").pop()}
                      </span>
                    </div>
                  )}
                </div>
                <button
                  onClick={() => removeAttachment(attachment.id)}
                  className="absolute -top-1.5 -right-1.5 p-1 bg-background border border-border rounded-full shadow-sm opacity-0 group-hover:opacity-100 transition-opacity hover:bg-destructive hover:border-destructive hover:text-destructive-foreground"
                >
                  <X className="w-3 h-3" />
                </button>
                <div className="absolute -bottom-1 left-1/2 -translate-x-1/2 px-1.5 py-0.5 bg-popover border border-border rounded text-[9px] text-muted-foreground truncate max-w-[90px] opacity-0 group-hover:opacity-100 transition-opacity shadow-sm pointer-events-none">
                  {attachment.name}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Textarea */}
      <div className="px-2.5 pt-2">
        <textarea
          ref={textareaRef}
          value={value}
          onChange={handleChange}
          onFocus={() => setIsFocused(true)}
          onBlur={() => setIsFocused(false)}
          onPaste={handlePaste}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          className="w-full bg-transparent text-sm leading-5 text-foreground placeholder:text-muted-foreground resize-none focus:outline-none min-h-[20px] max-h-[160px]"
          style={{ overflow: "hidden" }}
        />
      </div>

      {/* Toolbar */}
      <div className="flex items-center px-2 pb-2 pt-1">
        <input
          ref={fileInputRef}
          type="file"
          multiple
          accept="image/*,.pdf,.txt,.md,.json,.csv,.xml,.yaml,.yml"
          className="hidden"
          onChange={(e) => handleFileSelect(e.target.files)}
        />
        <button
          onClick={() => fileInputRef.current?.click()}
          onMouseDown={(e) => e.preventDefault()}
          className="inline-flex items-center gap-1 p-1.5 hover:bg-muted rounded text-muted-foreground hover:text-foreground transition-colors"
          title="Attach files (images, documents)"
        >
          <Paperclip className="w-4 h-4" />
        </button>

        <div className="w-px h-4 bg-border mx-1" />

        <ControlledAgentSelector
          selectedAgentId={agentSelector.selectedAgentId}
          selections={agentSelector.selections}
          displayName={agentSelector.displayName}
          showSelector={agentSelector.showSelector}
          isUsingConfigDefaults={agentSelector.isUsingConfigDefaults}
          panelAgent={agentSelector.panelAgent}
          tempAgentId={agentSelector.tempAgentId}
          tempSelections={agentSelector.tempSelections}
          onOpen={agentSelector.openSelector}
          onClose={agentSelector.closeSelector}
          onAgentClick={agentSelector.handleAgentClick}
          onColumnClick={agentSelector.handleColumnClick}
          getPanelSelection={agentSelector.getPanelSelection}
          shouldShowColumn={agentSelector.shouldShowColumn}
          isConfigDefault={agentSelector.isConfigDefault}
          onResetToDefaults={agentSelector.resetToConfigDefaults}
          onOpenAgentSettings={(agentId) => {
            console.log("Open settings for agent:", agentId)
          }}
          dropdownPosition="top"
        />

        <div className="flex-1" />

        {/* History indicator */}
        {historyIndex >= 0 && (
          <span className="text-[10px] text-muted-foreground mr-2">
            History {historyIndex + 1}/{messageHistory.length}
          </span>
        )}

        <button
          onMouseDown={(e) => e.preventDefault()}
          onClick={onSubmit}
          className={cn(
            "p-1.5 rounded-md transition-all flex-shrink-0",
            value.trim() || attachments.length > 0
              ? "bg-primary text-primary-foreground hover:bg-primary/90"
              : "bg-muted text-muted-foreground",
          )}
        >
          <Send className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  )
}
