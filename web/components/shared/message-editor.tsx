"use client"

import type React from "react"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  File,
  FileCode,
  FileText,
  Folder,
  ImageIcon,
  Loader2,
  Paperclip,
  X,
} from "lucide-react"

import { cn } from "@/lib/utils"
import { fetchMentionItems } from "@/lib/luban-http"
import type { AttachmentRef, CodexCustomPromptSnapshot, MentionItemSnapshot } from "@/lib/luban-api"

export type ComposerAttachment = {
  id: string
  type: "image" | "file"
  name: string
  size: number
  preview?: string
  previewUrl?: string
  status: "uploading" | "ready" | "failed"
  attachment?: AttachmentRef
}

function normalizeImeSpacedAsciiAtCursor(text: string, cursorPos: number | null) {
  // Some IMEs can (briefly) commit ASCII composition text with spaces between each character
  // when switching input sources (e.g. "h e l l o"). Collapse the *active* run near the cursor.
  if (cursorPos == null) return { text, cursorPos }

  const spacedAsciiRun = /(?:[0-9A-Za-z@._/:\-]\s+){2,}[0-9A-Za-z@._/:\-]/g
  let match: RegExpExecArray | null
  while ((match = spacedAsciiRun.exec(text)) != null) {
    const run = match[0]
    const start = match.index
    const end = start + run.length
    if (cursorPos < start || cursorPos > end) continue

    const collapsed = run.replace(/\s+/g, "")
    const next = text.slice(0, start) + collapsed + text.slice(end)
    const cursorInRun = text.slice(start, cursorPos)
    const nextCursorPos = start + cursorInRun.replace(/\s+/g, "").length
    return { text: next, cursorPos: nextCursorPos }
  }

  return { text, cursorPos }
}

function normalizeImeCompositionCommit(data: string) {
  // Some IMEs may commit ASCII composition text with extra whitespace when switching input sources
  // (e.g. "h e l l o" or "da t"). For chat input, treat this as a single token.
  if (!/\s/.test(data)) return data
  if (!/^[\x00-\x7F]+$/.test(data)) return data
  if (!/[0-9A-Za-z]/.test(data)) return data
  return data.replace(/\s+/g, "")
}

export function MessageEditor({
  value,
  onChange,
  attachments,
  onRemoveAttachment,
  onFileSelect,
  onPaste,
  onAddAttachmentRef,
  workspaceId,
  commands,
  messageHistory,
  onCommand,
  placeholder = "Let's chart the cosmos of ideas...",
  disabled,
  autoFocus,
  agentSelector,
  primaryAction,
  secondaryAction,
  testIds,
}: {
  value: string
  onChange: (value: string) => void
  attachments: ComposerAttachment[]
  onRemoveAttachment: (id: string) => void
  onFileSelect: (files: FileList | null) => void
  onPaste: (e: React.ClipboardEvent) => void
  onAddAttachmentRef?: (attachment: AttachmentRef) => void
  workspaceId?: number | null
  commands?: CodexCustomPromptSnapshot[]
  messageHistory?: string[]
  onCommand?: (commandId: string) => void
  placeholder?: string
  disabled: boolean
  autoFocus?: boolean
  agentSelector?: React.ReactNode
  primaryAction: {
    onClick: () => void
    disabled: boolean
    icon: React.ReactNode
    ariaLabel: string
    testId?: string
  }
  secondaryAction?: {
    onClick: () => void
    ariaLabel: string
    icon: React.ReactNode
    testId?: string
  }
  testIds: {
    textInput: string
    attachInput: string
    attachButton: string
    attachmentTile: string
  }
}) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null)
  const fileInputRef = useRef<HTMLInputElement | null>(null)
  const [isFocused, setIsFocused] = useState(false)
  const [isDragging, setIsDragging] = useState(false)
  const history = useMemo(() => messageHistory ?? [], [messageHistory])

  const isComposingRef = useRef(false)
  const ignoreEnterUntilRef = useRef<number>(0)
  const compositionSequenceRef = useRef(0)
  const compositionStartRef = useRef<{ sequence: number; value: string; start: number; end: number } | null>(null)

  const [showCommandMenu, setShowCommandMenu] = useState(false)
  const [commandQuery, setCommandQuery] = useState("")
  const [commandSelectedIndex, setCommandSelectedIndex] = useState(0)
  const commandMenuRef = useRef<HTMLDivElement | null>(null)

  const [showMentionMenu, setShowMentionMenu] = useState(false)
  const [mentionQuery, setMentionQuery] = useState("")
  const [mentionSelectedIndex, setMentionSelectedIndex] = useState(0)
  const [mentionStartPos, setMentionStartPos] = useState<number | null>(null)
  const [mentionItems, setMentionItems] = useState<MentionItemSnapshot[]>([])
  const mentionRequestIdRef = useRef(0)
  const mentionMenuRef = useRef<HTMLDivElement | null>(null)
  const mentionSelectedIndexRef = useRef(0)

  const [historyIndex, setHistoryIndex] = useState(-1)
  const [savedInput, setSavedInput] = useState("")

  useEffect(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = "auto"
    const maxHeightPx = 160
    const nextHeight = Math.min(el.scrollHeight, maxHeightPx)
    el.style.height = `${nextHeight}px`
    el.style.overflowX = "hidden"
    el.style.overflowY = el.scrollHeight > maxHeightPx ? "auto" : "hidden"
  }, [value])

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault()
    if (disabled) return
    setIsDragging(false)

    const raw = e.dataTransfer.getData("luban-context-attachment")
    if (raw && onAddAttachmentRef) {
      try {
        const attachment = JSON.parse(raw) as AttachmentRef
        if (attachment && typeof attachment.id === "string") {
          onAddAttachmentRef(attachment)
          return
        }
      } catch {
        // Ignore invalid payloads.
      }
    }
    onFileSelect(e.dataTransfer.files)
  }

  const filteredCommands = useMemo(() => {
    const list = commands ?? []
    if (!showCommandMenu) return list
    const q = commandQuery.trim().toLowerCase()
    if (!q) return list
    return list.filter((cmd) => {
      const label = cmd.label.toLowerCase()
      const desc = cmd.description.toLowerCase()
      return label.includes(q) || desc.includes(q)
    })
  }, [commands, commandQuery, showCommandMenu])

  const filteredMentions = useMemo(() => {
    if (!showMentionMenu) return []
    if (!mentionQuery.trim()) return []
    return mentionItems.slice(0, 10)
  }, [mentionItems, mentionQuery, showMentionMenu])

  useEffect(() => {
    if (!showCommandMenu) return
    const menu = commandMenuRef.current
    if (!menu) return
    if (commandSelectedIndex === 0) {
      menu.scrollTo({ top: 0 })
      return
    }
    const item = menu.querySelector(`[data-index="${commandSelectedIndex}"]`)
    if (item && "scrollIntoView" in item) {
      ;(item as HTMLElement).scrollIntoView({ block: "nearest" })
    }
  }, [commandSelectedIndex, showCommandMenu])

  useEffect(() => {
    if (!showMentionMenu) return
    const menu = mentionMenuRef.current
    if (!menu) return
    if (mentionSelectedIndex === 0) {
      menu.scrollTo({ top: 0 })
      return
    }
    const item = menu.querySelector(`[data-index="${mentionSelectedIndex}"]`)
    if (item && "scrollIntoView" in item) {
      ;(item as HTMLElement).scrollIntoView({ block: "nearest" })
    }
  }, [mentionSelectedIndex, showMentionMenu])

  useEffect(() => {
    if (!showMentionMenu) return
    const q = mentionQuery.trim()
    if (!q) {
      setMentionItems([])
      return
    }
    if (workspaceId == null) return

    const requestId = ++mentionRequestIdRef.current
    const timer = window.setTimeout(() => {
      void fetchMentionItems({ workspaceId, query: q })
        .then((items) => {
          if (mentionRequestIdRef.current !== requestId) return
          setMentionItems(items)
        })
        .catch((err) => {
          console.warn("mention search failed:", err)
          if (mentionRequestIdRef.current !== requestId) return
          setMentionItems([])
        })
    }, 120)

    return () => window.clearTimeout(timer)
  }, [mentionQuery, showMentionMenu, workspaceId])

  function getMentionIcon(item: MentionItemSnapshot) {
    if (item.kind === "folder") return <Folder className="w-3.5 h-3.5 text-base0d" />
    const ext = item.name.split(".").pop()?.toLowerCase()
    switch (ext) {
      case "tsx":
      case "ts":
      case "js":
      case "jsx":
        return <FileCode className="w-3.5 h-3.5 text-base0d" />
      case "css":
        return <FileText className="w-3.5 h-3.5 text-base0e" />
      case "json":
      case "toml":
      case "yaml":
      case "yml":
        return <FileCode className="w-3.5 h-3.5 text-base09" />
      default:
        return <File className="w-3.5 h-3.5 text-muted-foreground" />
    }
  }

  const handleCommandSelect = useCallback(
    (command: CodexCustomPromptSnapshot) => {
      setShowCommandMenu(false)
      setCommandQuery("")
      setCommandSelectedIndex(0)
      onChange("")
      onCommand?.(command.id)
    },
    [onChange, onCommand],
  )

  const handleMentionSelect = useCallback(
    (item: MentionItemSnapshot) => {
      if (mentionStartPos == null) return
      const el = textareaRef.current
      const cursor = el?.selectionStart ?? mentionStartPos
      const before = value.slice(0, mentionStartPos)
      const after = value.slice(cursor)
      const mention = `@${item.path} `
      const next = before + mention + after

      onChange(next)
      setShowMentionMenu(false)
      setMentionQuery("")
      setMentionStartPos(null)
      setMentionSelectedIndex(0)
      mentionSelectedIndexRef.current = 0

      window.setTimeout(() => {
        const el = textareaRef.current
        if (!el) return
        const pos = before.length + mention.length
        el.focus()
        el.setSelectionRange(pos, pos)
      }, 0)
    },
    [mentionStartPos, onChange, value],
  )

  const handleTextChange = useCallback(
    (next: string, cursorPos: number | null) => {
      onChange(next)
      setHistoryIndex(-1)

      if (next.startsWith("/")) {
        const query = next.slice(1).split(" ")[0] ?? ""
        setShowCommandMenu(true)
        setCommandQuery(query)
        setCommandSelectedIndex(0)
        setShowMentionMenu(false)
        setMentionQuery("")
        setMentionStartPos(null)
        setMentionSelectedIndex(0)
        return
      }

      setShowCommandMenu(false)
      setCommandQuery("")

      if (cursorPos == null) {
        setShowMentionMenu(false)
        setMentionStartPos(null)
        return
      }

      const beforeCursor = next.slice(0, cursorPos)
      const lastAtIndex = beforeCursor.lastIndexOf("@")
      if (lastAtIndex >= 0) {
        const charBefore = lastAtIndex === 0 ? " " : beforeCursor[lastAtIndex - 1] ?? " "
        const isWordStart = /\s/.test(charBefore)
        if (isWordStart) {
          const textAfterAt = beforeCursor.slice(lastAtIndex + 1)
          if (!textAfterAt.includes(" ")) {
            setShowMentionMenu(true)
            setMentionQuery(textAfterAt)
            setMentionSelectedIndex(0)
            mentionSelectedIndexRef.current = 0
            setMentionStartPos(lastAtIndex)
            return
          }
        }
      }

      setShowMentionMenu(false)
      setMentionStartPos(null)
    },
    [onChange],
  )

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (showCommandMenu) {
        if (e.key === "ArrowDown") {
          e.preventDefault()
          setCommandSelectedIndex((i) => Math.min(i + 1, Math.max(filteredCommands.length - 1, 0)))
          return
        }
        if (e.key === "ArrowUp") {
          e.preventDefault()
          setCommandSelectedIndex((i) => Math.max(i - 1, 0))
          return
        }
        if (e.key === "Enter" || e.key === "Tab") {
          e.preventDefault()
          const target = filteredCommands[commandSelectedIndex]
          if (target) handleCommandSelect(target)
          return
        }
        if (e.key === "Escape") {
          e.preventDefault()
          setShowCommandMenu(false)
          setCommandQuery("")
          onChange("")
          return
        }
      }

      if (showMentionMenu) {
        if (e.key === "ArrowDown") {
          e.preventDefault()
          setMentionSelectedIndex((i) => {
            const next = Math.min(i + 1, Math.max(filteredMentions.length - 1, 0))
            mentionSelectedIndexRef.current = next
            return next
          })
          return
        }
        if (e.key === "ArrowUp") {
          e.preventDefault()
          setMentionSelectedIndex((i) => {
            const next = Math.max(i - 1, 0)
            mentionSelectedIndexRef.current = next
            return next
          })
          return
        }
        if (e.key === "Enter" || e.key === "Tab") {
          e.preventDefault()
          const target = filteredMentions[mentionSelectedIndexRef.current]
          if (target) handleMentionSelect(target)
          return
        }
        if (e.key === "Escape") {
          e.preventDefault()
          setShowMentionMenu(false)
          setMentionStartPos(null)
          return
        }
      }

      if (e.key === "ArrowUp" && !showCommandMenu && !showMentionMenu) {
        const el = textareaRef.current
        if (el) {
          const atStart = el.selectionStart === 0 && el.selectionEnd === 0
          const isEmpty = value === ""
          if ((atStart || isEmpty) && history.length > 0) {
            e.preventDefault()
            if (historyIndex === -1) setSavedInput(value)
            const nextIndex = Math.min(historyIndex + 1, history.length - 1)
            setHistoryIndex(nextIndex)
            onChange(history[history.length - 1 - nextIndex] ?? "")
            return
          }
        }
      }

      if (e.key === "ArrowDown" && !showCommandMenu && !showMentionMenu) {
        const el = textareaRef.current
        if (el && historyIndex >= 0) {
          const atEnd = el.selectionStart === value.length
          if (atEnd) {
            e.preventDefault()
            const nextIndex = historyIndex - 1
            if (nextIndex < 0) {
              setHistoryIndex(-1)
              onChange(savedInput)
            } else {
              setHistoryIndex(nextIndex)
              onChange(history[history.length - 1 - nextIndex] ?? "")
            }
            return
          }
        }
      }

      const isEnter =
        e.key === "Enter" ||
        e.code === "Enter" ||
        e.code === "NumpadEnter" ||
        // Some embedded WebViews report an unhelpful `key` but still populate legacy fields.
        (e as unknown as { keyCode?: number; which?: number }).keyCode === 13 ||
        (e as unknown as { keyCode?: number; which?: number }).which === 13

      if (isEnter && !e.shiftKey) {
        const now = typeof performance !== "undefined" && typeof performance.now === "function" ? performance.now() : Date.now()
        const isImeComposing = isComposingRef.current || e.nativeEvent.isComposing || e.key === "Process"
        if (isImeComposing) return
        if (now < ignoreEnterUntilRef.current) return
        e.preventDefault()
        if (primaryAction.disabled) return
        primaryAction.onClick()
        setHistoryIndex(-1)
        setSavedInput("")
      }
      if (e.key === "Escape" && secondaryAction) {
        e.preventDefault()
        secondaryAction.onClick()
      }
    },
    [
      commandSelectedIndex,
      filteredCommands,
      filteredMentions,
      handleCommandSelect,
      handleMentionSelect,
      history,
      historyIndex,
      onChange,
      primaryAction,
      savedInput,
      secondaryAction,
      showCommandMenu,
      showMentionMenu,
      value,
    ],
  )

  return (
    <div
      className={cn(
        "relative bg-background border rounded-lg shadow-lg transition-all",
        isFocused ? "border-primary/50 ring-1 ring-primary/20 shadow-xl" : "border-border",
        isDragging && "border-primary ring-2 ring-primary/30 bg-primary/5",
      )}
      onDragOver={(e) => {
        e.preventDefault()
        if (disabled) return
        setIsDragging(true)
      }}
      onDragLeave={(e) => {
        e.preventDefault()
        setIsDragging(false)
      }}
      onDrop={handleDrop}
    >
      {showCommandMenu &&
        (filteredCommands.length === 0 ? (
          <div className="absolute bottom-full left-0 right-0 mb-2 bg-popover border border-border rounded-lg shadow-xl overflow-hidden z-50">
            <div className="px-3 py-4 text-center text-sm text-muted-foreground">No commands found</div>
          </div>
        ) : (
          <>
            <div
              className="fixed inset-0 z-40"
              onClick={() => {
                setShowCommandMenu(false)
                onChange("")
              }}
            />
            <div
              ref={commandMenuRef}
              data-testid="chat-command-menu"
              className="absolute bottom-full left-0 right-0 mb-2 bg-popover border border-border rounded-lg shadow-xl overflow-hidden z-50 max-h-[280px] overflow-y-auto"
            >
              <div className="px-2 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground border-b border-border bg-muted/30">
                Commands
              </div>
              <div className="py-1">
                {filteredCommands.map((cmd, idx) => (
                  <button
                    key={cmd.id}
                    type="button"
                    data-testid="chat-command-item"
                    data-index={idx}
                    onMouseEnter={() => setCommandSelectedIndex(idx)}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => handleCommandSelect(cmd)}
                    className={cn(
                      "w-full flex items-center gap-3 px-3 py-2 text-left transition-colors",
                      idx === commandSelectedIndex ? "bg-primary/10 text-primary" : "hover:bg-muted/50",
                    )}
                  >
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium">/{cmd.label}</span>
                      {cmd.description ? (
                        <span className="text-xs text-muted-foreground ml-2">{cmd.description}</span>
                      ) : null}
                    </div>
                  </button>
                ))}
              </div>
            </div>
          </>
        ))}

      {showMentionMenu && (
        <>
          <div
            className="fixed inset-0 z-40"
            onClick={() => {
              setShowMentionMenu(false)
              setMentionStartPos(null)
            }}
          />
          <div
            data-testid="chat-mention-menu"
            className="absolute bottom-full left-0 right-0 mb-2 bg-popover border border-border rounded-lg shadow-xl overflow-hidden z-50 max-h-[320px] overflow-y-auto"
          >
            <div className="px-2 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground border-b border-border bg-muted/30">
              Reference files
            </div>
            {!mentionQuery.trim() ? (
              <div className="px-3 py-3 text-sm text-muted-foreground">Type to search files...</div>
            ) : filteredMentions.length === 0 ? (
              <div className="px-3 py-4 text-center text-sm text-muted-foreground">No files found</div>
            ) : (
              <div ref={mentionMenuRef} className="py-1">
                {filteredMentions.map((item, idx) => (
                  <button
                    key={item.id}
                    type="button"
                    data-testid="chat-mention-item"
                    data-index={idx}
                    onMouseEnter={() => {
                      mentionSelectedIndexRef.current = idx
                      setMentionSelectedIndex(idx)
                    }}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => handleMentionSelect(item)}
                    className={cn(
                      "w-full flex items-center gap-2.5 px-3 py-1.5 text-left transition-colors",
                      idx === mentionSelectedIndex ? "bg-primary/10 text-primary" : "hover:bg-muted/50",
                    )}
                  >
                    {getMentionIcon(item)}
                    <div className="flex-1 min-w-0">
                      <span className="text-sm truncate block">{item.name}</span>
                      <span className="text-[11px] text-muted-foreground truncate block">{item.path}</span>
                    </div>
                  </button>
                ))}
              </div>
            )}
          </div>
        </>
      )}

      {isDragging && (
        <div className="absolute inset-0 z-10 flex items-center justify-center bg-primary/5 rounded-lg border-2 border-dashed border-primary">
          <div className="flex flex-col items-center gap-2 text-primary">
            <ImageIcon className="w-8 h-8" />
            <span className="text-sm font-medium">Drop files here</span>
          </div>
        </div>
      )}

      {attachments.length > 0 && (
        <div className="px-3 pt-3 pb-1 flex flex-wrap gap-3">
          {attachments.map((attachment) => (
            <div key={attachment.id} data-testid={testIds.attachmentTile} className="group relative">
              <div className="relative">
                <div className="w-20 h-20 rounded-lg overflow-hidden border border-border/50 hover:border-border transition-colors bg-muted/40 flex items-center justify-center">
                  {attachment.type === "image" && (attachment.preview || attachment.previewUrl) ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={attachment.preview ?? attachment.previewUrl}
                      alt={attachment.name}
                      className="w-full h-full object-cover"
                    />
                  ) : (
                    <div className="flex flex-col items-center gap-1.5">
                      {attachment.name.toLowerCase().endsWith(".json") ? (
                        <FileCode className="w-6 h-6 text-base09" />
                      ) : attachment.name.toLowerCase().endsWith(".pdf") ? (
                        <FileText className="w-6 h-6 text-base08" />
                      ) : (
                        <FileText className="w-6 h-6 text-muted-foreground" />
                      )}
                      <span className="text-[9px] text-muted-foreground uppercase font-medium tracking-wide">
                        {attachment.name.split(".").pop()}
                      </span>
                    </div>
                  )}
                </div>
                {attachment.status === "uploading" && (
                  <div className="absolute inset-0 flex items-center justify-center bg-background/60">
                    <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
                  </div>
                )}
                <button
                  onClick={() => onRemoveAttachment(attachment.id)}
                  className={cn(
                    "absolute -top-1.5 -right-1.5 p-1 bg-background border border-border rounded-full shadow-sm transition-opacity hover:bg-destructive hover:border-destructive hover:text-destructive-foreground",
                    attachment.status === "uploading"
                      ? "opacity-0 pointer-events-none"
                      : "opacity-0 group-hover:opacity-100",
                  )}
                  aria-label="Remove attachment"
                >
                  <X className="w-3 h-3" />
                </button>
                <div className="absolute -bottom-1 left-1/2 -translate-x-1/2 px-1.5 py-0.5 bg-popover border border-border rounded text-[9px] text-muted-foreground truncate max-w-[90px] opacity-0 group-hover:opacity-100 transition-opacity shadow-sm pointer-events-none">
                  {attachment.name}
                </div>
                {attachment.status === "failed" && (
                  <div className="absolute inset-x-0 bottom-0 px-1 py-0.5 text-[9px] text-destructive bg-background/80 text-center">
                    Upload failed
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="px-2.5 pt-2">
        <textarea
          ref={textareaRef}
          data-testid={testIds.textInput}
          value={value}
          onChange={(e) => handleTextChange(e.target.value, e.target.selectionStart)}
          onCompositionStart={() => {
            isComposingRef.current = true
            const el = textareaRef.current
            const sequence = ++compositionSequenceRef.current
            if (!el) return
            compositionStartRef.current = {
              sequence,
              value: el.value,
              start: el.selectionStart,
              end: el.selectionEnd,
            }
          }}
          onCompositionEnd={(e) => {
            isComposingRef.current = false
            const now =
              typeof performance !== "undefined" && typeof performance.now === "function" ? performance.now() : Date.now()
            ignoreEnterUntilRef.current = now + 50
            const sequence = compositionSequenceRef.current
            const committed = normalizeImeCompositionCommit(e.data ?? "")
            window.setTimeout(() => {
              if (compositionSequenceRef.current !== sequence) return
              const el = textareaRef.current
              if (!el) return

              const snapshot = compositionStartRef.current
              if (!snapshot || snapshot.sequence !== sequence) return
              const raw = el.value

              // If the native value did not change during composition (e.g. synthetic events in tests),
              // do not attempt to "re-commit" CompositionEvent.data.
              if (raw === snapshot.value) {
                compositionStartRef.current = null
                return
              }

              if (!committed) {
                const normalized = normalizeImeSpacedAsciiAtCursor(raw, el.selectionStart)
                if (normalized.text !== raw) {
                  handleTextChange(normalized.text, normalized.cursorPos)
                  if (normalized.cursorPos != null) {
                    window.setTimeout(() => {
                      const el = textareaRef.current
                      if (!el) return
                      el.setSelectionRange(normalized.cursorPos, normalized.cursorPos)
                    }, 0)
                  }
                }
                compositionStartRef.current = null
                return
              }

              const next = snapshot.value.slice(0, snapshot.start) + committed + snapshot.value.slice(snapshot.end)
              if (next !== raw) {
                const cursorPos = snapshot.start + committed.length
                handleTextChange(next, cursorPos)
                window.setTimeout(() => {
                  const el = textareaRef.current
                  if (!el) return
                  el.setSelectionRange(cursorPos, cursorPos)
                }, 0)
              }

              compositionStartRef.current = null
            }, 0)
          }}
          onFocus={() => setIsFocused(true)}
          onBlur={() => setIsFocused(false)}
          onPaste={onPaste}
          placeholder={placeholder}
          className="w-full bg-transparent text-sm leading-5 text-foreground placeholder:text-muted-foreground resize-none focus:outline-none min-h-[20px] max-h-[160px] luban-font-chat"
          disabled={disabled}
          autoFocus={autoFocus}
          onKeyDown={handleKeyDown}
        />
      </div>

      <div className="flex items-center px-2 pb-2 pt-1">
        <input
          ref={fileInputRef}
          data-testid={testIds.attachInput}
          type="file"
          multiple
          accept="image/*,.pdf,.txt,.md,.json,.csv,.xml,.yaml,.yml"
          className="hidden"
          onChange={(e) => onFileSelect(e.target.files)}
        />
        <button
          data-testid={testIds.attachButton}
          onClick={() => fileInputRef.current?.click()}
          onMouseDown={(e) => e.preventDefault()}
          className="inline-flex items-center gap-1 p-1.5 hover:bg-muted rounded text-muted-foreground hover:text-foreground transition-colors"
          title="Attach files (images, documents)"
          disabled={disabled}
        >
          <Paperclip className="w-4 h-4" />
        </button>

        {agentSelector ? (
          <>
            <div className="w-px h-4 bg-border mx-1" />
            {agentSelector}
          </>
        ) : null}

        <div className="flex-1" />

        {historyIndex >= 0 && history.length > 0 && (
          <span className="text-[10px] text-muted-foreground mr-2">
            History {historyIndex + 1}/{history.length}
          </span>
        )}

        {secondaryAction && (
          <button
            onClick={secondaryAction.onClick}
            aria-label={secondaryAction.ariaLabel}
            data-testid={secondaryAction.testId}
            className="p-1.5 rounded-md transition-all flex-shrink-0 bg-muted text-muted-foreground hover:text-foreground hover:bg-muted/70"
          >
            {secondaryAction.icon}
          </button>
        )}
        <button
          data-testid={primaryAction.testId}
          aria-label={primaryAction.ariaLabel}
          className={cn(
            "p-1.5 rounded-md transition-all flex-shrink-0 disabled:opacity-50",
            !primaryAction.disabled
              ? "bg-primary text-primary-foreground hover:bg-primary/90"
              : "bg-muted text-muted-foreground",
            secondaryAction && "ml-2",
          )}
          onMouseDown={(e) => e.preventDefault()}
          onClick={primaryAction.onClick}
          disabled={primaryAction.disabled}
        >
          {primaryAction.icon}
        </button>
      </div>
    </div>
  )
}
