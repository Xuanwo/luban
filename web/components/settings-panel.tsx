"use client"

import type { ElementType, MouseEvent } from "react"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useTheme } from "next-themes"
import {
  Check,
  ChevronDown,
  ChevronRight,
  AlertTriangle,
  Bot,
  CheckCircle2,
  ClipboardType,
  GitBranch,
  GitPullRequest,
  Lightbulb,
  ListTodo,
  MessageSquare,
  HelpCircle,
  Bug,
  FileCode,
  FileText,
  Folder,
  Loader2,
  Monitor,
  Moon,
  Palette,
  Pencil,
  Play,
  RefreshCw,
  Settings,
  ShieldCheck,
  Sparkle,
  Sparkles,
  Sun,
  Type,
  UserPen,
  X,
  XCircle,
} from "lucide-react"
import type { Highlighter } from "shiki"
import { createHighlighter } from "shiki"
import { toast } from "sonner"

import { useAppearance } from "@/components/appearance-provider"
import { useLuban } from "@/lib/luban-context"
import { cn } from "@/lib/utils"
import type {
  AmpConfigEntrySnapshot,
  AppearanceTheme,
  ClaudeConfigEntrySnapshot,
  CodexConfigEntrySnapshot,
  SystemTaskKind,
  TaskIntentKind,
} from "@/lib/luban-api"
import { addProjectAndOpen } from "@/lib/add-project-and-open"

interface SettingsPanelProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  initialSectionId?: "theme" | "fonts" | "agent" | "task"
  initialAgentId?: string
  initialAgentFilePath?: string
}

type TocItem = {
  id: string
  label: string
  icon: ElementType
  children?: { id: string; label: string; icon: ElementType }[]
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

const themeOptions: { id: AppearanceTheme; label: string; icon: ElementType }[] = [
  { id: "light", label: "Light", icon: Sun },
  { id: "dark", label: "Dark", icon: Moon },
  { id: "system", label: "System", icon: Monitor },
]

type TaskPromptTemplate = { intent_kind: TaskIntentKind; template: string }
type SystemPromptTemplate = { kind: SystemTaskKind; template: string }

type TaskTypeConfig = {
  id: TaskIntentKind | SystemTaskKind
  label: string
  icon: ElementType
  description: string
}

type TaskType = TaskIntentKind | SystemTaskKind

const systemTaskTypes: TaskTypeConfig[] = [
  { id: "infer-type", label: "Infer Type", icon: Sparkle, description: "Infer task type from the input" },
  { id: "rename-branch", label: "Rename Branch", icon: GitBranch, description: "Generate a branch name from the task" },
  { id: "auto-title-thread", label: "Auto Title Thread", icon: Type, description: "Generate a short thread title from the first user message" },
]

const taskTypes: TaskTypeConfig[] = [
  { id: "fix", label: "Fix", icon: Bug, description: "Fix bugs or issues in the code" },
  { id: "implement", label: "Implement", icon: Lightbulb, description: "Implement new features or functionality" },
  { id: "review", label: "Review", icon: GitPullRequest, description: "Review pull request code changes" },
  { id: "discuss", label: "Discuss", icon: MessageSquare, description: "Discuss and explore ideas or questions" },
  { id: "other", label: "Other", icon: HelpCircle, description: "Other types of tasks" },
]

const allTaskTypes: TaskTypeConfig[] = [...systemTaskTypes, ...taskTypes]

type TemplateVariable = {
  id: string
  label: string
  description: string
}

const templateVariables: TemplateVariable[] = [
  { id: "repo", label: "repo", description: "Repository name (e.g., owner/repo)" },
  { id: "issue", label: "issue", description: "Issue details (title/body/comments)" },
  { id: "pr", label: "pr", description: "Pull request details (title/diff/comments)" },
  { id: "task_input", label: "task_input", description: "Raw task input from the user" },
  { id: "intent_label", label: "intent_label", description: "Intent label derived from task analysis" },
  { id: "known_context", label: "known_context", description: "Known context retrieved during task preview" },
  { id: "context_json", label: "context_json", description: "Structured JSON context from task preview" },
]

const variablesByTaskType: Record<string, string[]> = {
  "infer-type": ["task_input", "context_json"],
  "rename-branch": ["task_input", "context_json"],
  "auto-title-thread": ["task_input", "context_json"],
  fix: ["repo", "issue", "task_input", "intent_label", "known_context"],
  implement: ["repo", "issue", "task_input", "intent_label", "known_context"],
  review: ["repo", "pr", "task_input", "intent_label", "known_context"],
  discuss: ["repo", "issue", "task_input", "intent_label", "known_context"],
  other: ["repo", "issue", "task_input", "intent_label", "known_context"],
}

function variablesForTaskType(taskType: TaskType): TemplateVariable[] {
  const ids = variablesByTaskType[taskType] ?? []
  return ids.map((id) => templateVariables.find((v) => v.id === id)).filter(Boolean) as TemplateVariable[]
}

let highlighterPromise: Promise<Highlighter> | null = null

function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ["github-light", "github-dark"],
      langs: ["markdown", "toml", "yaml", "json"],
    })
  }
  return highlighterPromise
}

function useShikiHighlighter(): Highlighter | null {
  const [highlighter, setHighlighter] = useState<Highlighter | null>(null)
  useEffect(() => {
    void getHighlighter().then(setHighlighter)
  }, [])
  return highlighter
}

function MarkdownHighlight({
  text,
  highlighter,
  lang = "markdown",
}: {
  text: string
  highlighter: Highlighter | null
  lang?: string
}) {
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

  if (!html) return <span className="text-foreground">{text}</span>

  return (
    <div
      className="shiki-highlight [&_pre]:!bg-transparent [&_code]:!bg-transparent [&_.shiki]:!bg-transparent [&_pre]:!whitespace-pre-wrap [&_code]:!whitespace-pre-wrap [&_pre]:!break-words [&_code]:!break-words"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}

function TaskPromptEditor({
  templates,
  defaultTemplates,
  systemTemplates,
  defaultSystemTemplates,
  setTaskPromptTemplate,
  setSystemPromptTemplate,
}: {
  templates: TaskPromptTemplate[]
  defaultTemplates: TaskPromptTemplate[]
  systemTemplates: SystemPromptTemplate[]
  defaultSystemTemplates: SystemPromptTemplate[]
  setTaskPromptTemplate: (kind: TaskIntentKind, template: string) => void
  setSystemPromptTemplate: (kind: SystemTaskKind, template: string) => void
}) {
  const userTemplatesByKind = useMemo(
    () => new Map(templates.map((t) => [t.intent_kind, t.template])),
    [templates],
  )
  const defaultUserTemplatesByKind = useMemo(
    () => new Map(defaultTemplates.map((t) => [t.intent_kind, t.template])),
    [defaultTemplates],
  )
  const systemTemplatesByKind = useMemo(
    () => new Map(systemTemplates.map((t) => [t.kind, t.template])),
    [systemTemplates],
  )
  const defaultSystemTemplatesByKind = useMemo(
    () => new Map(defaultSystemTemplates.map((t) => [t.kind, t.template])),
    [defaultSystemTemplates],
  )

  const isSystemTask = (taskType: TaskType): taskType is SystemTaskKind =>
    taskType === "infer-type" || taskType === "rename-branch" || taskType === "auto-title-thread"

  const [selectedType, setSelectedType] = useState<TaskType>("infer-type")
  const [typePrompts, setTypePrompts] = useState<Record<string, string>>(() => {
    const initial: Record<string, string> = {}
    allTaskTypes.forEach((t) => {
      if (isSystemTask(t.id)) {
        initial[t.id] = systemTemplatesByKind.get(t.id) ?? defaultSystemTemplatesByKind.get(t.id) ?? ""
      } else {
        initial[t.id] = userTemplatesByKind.get(t.id) ?? defaultUserTemplatesByKind.get(t.id) ?? ""
      }
    })
    return initial
  })

  const editorRef = useRef<HTMLTextAreaElement>(null)
  const highlightRef = useRef<HTMLDivElement>(null)
  const highlighter = useShikiHighlighter()

  const [showAutocomplete, setShowAutocomplete] = useState(false)
  const [autocompletePosition, setAutocompletePosition] = useState({ top: 0, left: 0 })
  const [autocompleteFilter, setAutocompleteFilter] = useState("")
  const [selectedAutocompleteIndex, setSelectedAutocompleteIndex] = useState(0)

  const availableVariables = variablesForTaskType(selectedType)
  const filteredVariables = availableVariables.filter(
    (v) =>
      v.label.toLowerCase().includes(autocompleteFilter.toLowerCase()) ||
      v.description.toLowerCase().includes(autocompleteFilter.toLowerCase()),
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
    const text = typePrompts[selectedType] ?? ""

    const insertText = `{{${variableId}}}`
    let newCursorPos = start + insertText.length

    if (showAutocomplete) {
      const beforeCursor = text.slice(0, start)
      const triggerMatch = beforeCursor.match(/\{\{([^}]*)$/)
      if (triggerMatch) {
        const triggerStart = start - triggerMatch[0].length
        const newText = text.slice(0, triggerStart) + insertText + text.slice(end)
        setTypePrompts((prev) => ({ ...prev, [selectedType]: newText }))
        scheduleSave(selectedType, newText)
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
    setTypePrompts((prev) => ({ ...prev, [selectedType]: newText }))
    scheduleSave(selectedType, newText)

    requestAnimationFrame(() => {
      editor.focus()
      editor.setSelectionRange(newCursorPos, newCursorPos)
    })
  }

  const handleEditorChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = e.target.value
    const cursorPos = e.target.selectionStart
    setTypePrompts((prev) => ({ ...prev, [selectedType]: newValue }))
    scheduleSave(selectedType, newValue)

    const beforeCursor = newValue.slice(0, cursorPos)
    const triggerMatch = beforeCursor.match(/\{\{([^}\s]*)$/)

    if (triggerMatch) {
      setAutocompleteFilter(triggerMatch[1])
      setSelectedAutocompleteIndex(0)

      const textarea = editorRef.current
      if (textarea) {
        const lines = beforeCursor.split("\n")
        const currentLineIndex = lines.length - 1
        const currentLineStart = beforeCursor.lastIndexOf("\n") + 1
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

    if (e.key === "ArrowDown") {
      e.preventDefault()
      setSelectedAutocompleteIndex((prev) => (prev < filteredVariables.length - 1 ? prev + 1 : prev))
    } else if (e.key === "ArrowUp") {
      e.preventDefault()
      setSelectedAutocompleteIndex((prev) => (prev > 0 ? prev - 1 : 0))
    } else if (e.key === "Enter" || e.key === "Tab") {
      if (filteredVariables.length > 0) {
        e.preventDefault()
        insertVariable(filteredVariables[selectedAutocompleteIndex].id)
      }
    } else if (e.key === "Escape") {
      setShowAutocomplete(false)
      setAutocompleteFilter("")
    }
  }

  const currentPrompt = typePrompts[selectedType] ?? ""
  const currentTaskType = allTaskTypes.find((t) => t.id === selectedType)!
  const isCurrentSystemTask = isSystemTask(selectedType)
  const saveTimerRef = useRef<number | null>(null)
  const pendingSaveRef = useRef<{ taskType: TaskType; prompt: string } | null>(null)

  const flushPendingSave = () => {
    const pending = pendingSaveRef.current
    if (!pending) return
    pendingSaveRef.current = null

    if (saveTimerRef.current != null) {
      window.clearTimeout(saveTimerRef.current)
      saveTimerRef.current = null
    }

    const trimmed = pending.prompt.trim()
    if (!trimmed) return

    if (isSystemTask(pending.taskType)) {
      setSystemPromptTemplate(pending.taskType as SystemTaskKind, pending.prompt)
    } else {
      setTaskPromptTemplate(pending.taskType as TaskIntentKind, pending.prompt)
    }
  }

  const scheduleSave = (taskType: TaskType, prompt: string) => {
    pendingSaveRef.current = { taskType, prompt }
    if (saveTimerRef.current != null) {
      window.clearTimeout(saveTimerRef.current)
    }
    saveTimerRef.current = window.setTimeout(() => flushPendingSave(), 800)
  }

  useEffect(() => {
    return () => {
      if (saveTimerRef.current != null) {
        window.clearTimeout(saveTimerRef.current)
        saveTimerRef.current = null
      }
    }
  }, [])

  const selectTaskType = (taskType: TaskType) => {
    flushPendingSave()
    setSelectedType(taskType)
  }

  return (
    <div data-testid="task-prompt-editor" className="border border-border rounded-lg overflow-hidden bg-sidebar">
      <div className="flex h-[380px]">
        <div className="w-44 border-r border-border flex flex-col">
          <div className="flex items-center gap-2 h-11 px-3 border-b border-border">
            <ClipboardType className="w-4 h-4 text-muted-foreground" />
            <span className="text-sm font-medium text-muted-foreground">Type</span>
          </div>
          <div className="flex-1 overflow-y-auto py-1.5">
            <div className="flex items-center gap-2 px-3 py-1.5">
              <ShieldCheck className="w-3 h-3 text-muted-foreground/60" />
              <span className="text-[10px] uppercase tracking-wider text-muted-foreground/60 font-medium">System</span>
            </div>

            {systemTaskTypes.map((taskType) => {
              const Icon = taskType.icon
              const isSelected = selectedType === taskType.id

              return (
                <button
                  key={taskType.id}
                  data-testid={`task-prompt-tab-${taskType.id}`}
                  onClick={() => selectTaskType(taskType.id)}
                  className={cn(
                    "w-full flex items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors",
                    isSelected
                      ? "bg-status-warning/15 text-status-warning"
                      : "text-muted-foreground hover:text-foreground hover:bg-accent",
                  )}
                >
                  <Icon className={cn("w-4 h-4 shrink-0", isSelected ? "text-status-warning" : "text-muted-foreground")} />
                  <span className="truncate">{taskType.label}</span>
                </button>
              )
            })}

            <div className="flex items-center gap-2 px-3 py-1.5 mt-2 border-t border-border pt-3">
              <UserPen className="w-3 h-3 text-muted-foreground/60" />
              <span className="text-[10px] uppercase tracking-wider text-muted-foreground/60 font-medium">User</span>
            </div>

            {taskTypes.map((taskType) => {
              const Icon = taskType.icon
              const isSelected = selectedType === taskType.id

              return (
                <button
                  key={taskType.id}
                  data-testid={`task-prompt-tab-${taskType.id}`}
                  onClick={() => selectTaskType(taskType.id)}
                  className={cn(
                    "w-full flex items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors",
                    isSelected
                      ? "bg-primary/15 text-primary"
                      : "text-muted-foreground hover:text-foreground hover:bg-accent",
                  )}
                >
                  <Icon className={cn("w-4 h-4 shrink-0", isSelected ? "text-primary" : "text-muted-foreground")} />
                  <span className="truncate">{taskType.label}</span>
                </button>
              )
            })}
          </div>
        </div>

        <div className="flex-1 flex flex-col min-w-0 bg-background">
          <div className="flex items-center justify-between h-11 px-3 border-b border-border">
            <div className="flex items-center gap-2">
              <currentTaskType.icon className="w-4 h-4 text-primary" />
              <span className="text-sm font-medium">{currentTaskType.label}</span>
            </div>
            <div className="flex items-center gap-1">
              <button
                data-testid="task-prompt-reset"
                onClick={() => {
                  const next = isCurrentSystemTask
                    ? defaultSystemTemplatesByKind.get(selectedType as SystemTaskKind) ??
                      systemTemplatesByKind.get(selectedType as SystemTaskKind) ??
                      ""
                    : defaultUserTemplatesByKind.get(selectedType as TaskIntentKind) ??
                      userTemplatesByKind.get(selectedType as TaskIntentKind) ??
                      ""
                  setTypePrompts((prev) => ({ ...prev, [selectedType]: next }))
                  scheduleSave(selectedType, next)
                }}
                className="flex items-center gap-1.5 px-2 py-1 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
              >
                <RefreshCw className="w-3.5 h-3.5" />
                Reset
              </button>
              <button
                data-testid="task-prompt-edit-in-luban"
                onClick={() => addProjectAndOpen("~/luban")}
                className="flex items-center gap-1.5 px-2 py-1 rounded text-xs bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
              >
                <Pencil className="w-3.5 h-3.5" />
                Edit in Luban
              </button>
            </div>
          </div>

          {isCurrentSystemTask && (
            <div className="flex items-start gap-2.5 px-3 py-2.5 bg-status-warning/10 border-b border-status-warning/20">
              <AlertTriangle className="w-4 h-4 text-status-warning shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <p className="text-xs text-muted-foreground leading-relaxed">
                  <span className="font-medium text-status-warning">System Prompt</span> — Luban&apos;s core functionality depends on this prompt. Please avoid modifying unless you have specific requirements.
                </p>
              </div>
            </div>
          )}

          <div className="flex-1 relative overflow-hidden">
            <div
              ref={highlightRef}
              className="absolute inset-0 p-4 text-sm font-mono leading-relaxed whitespace-pre-wrap break-words overflow-auto pointer-events-none"
              aria-hidden="true"
            >
              <MarkdownHighlight text={currentPrompt} highlighter={highlighter} />
            </div>

            <textarea
              ref={editorRef}
              data-testid="task-prompt-template"
              value={currentPrompt}
              onChange={handleEditorChange}
              onKeyDown={handleEditorKeyDown}
              onScroll={handleEditorScroll}
              onBlur={() => {
                flushPendingSave()
                setTimeout(() => setShowAutocomplete(false), 150)
              }}
              className="absolute inset-0 w-full h-full bg-transparent text-transparent caret-foreground text-sm font-mono leading-relaxed resize-none focus:outline-none p-4 selection:bg-primary/20 selection:text-transparent overflow-auto"
              wrap="soft"
              spellCheck={false}
              placeholder="Enter prompt template..."
            />

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
                          : "hover:bg-accent text-foreground",
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

function ThemePreviewCard({
  themeId,
  label,
  icon: Icon,
  isSelected,
  onClick,
  testId,
}: {
  themeId: string
  label: string
  icon: ElementType
  isSelected: boolean
  onClick: () => void
  testId?: string
}) {
  const isSystem = themeId === "system"
  const preview = (
    <div className="flex bg-background">
      <div className="w-12 border-r border-border bg-sidebar">
        <div className="h-5 border-b border-border flex items-center px-1.5">
          <div className="w-3 h-3 rounded bg-muted-foreground/25" />
        </div>
        <div className="p-1.5 space-y-1">
          <div className="h-2 w-8 rounded bg-muted-foreground/25" />
          <div className="h-2 w-6 rounded bg-primary/15" />
          <div className="h-2 w-7 rounded bg-muted-foreground/20" />
        </div>
      </div>
      <div className="flex-1 flex flex-col min-w-0">
        <div className="h-5 border-b border-border" />
        <div className="flex-1 p-2 space-y-1.5">
          <div className="h-5 rounded bg-secondary/60" />
          <div className="h-8 rounded bg-secondary/60" />
        </div>
      </div>
      <div className="w-10 border-l border-border bg-sidebar">
        <div className="h-5 border-b border-border" />
        <div className="flex-1 p-1 bg-secondary/60">
          <div className="h-1.5 w-6 rounded bg-status-success/35" />
        </div>
      </div>
    </div>
  )

  return (
    <button
      data-testid={testId}
      onClick={onClick}
      className={cn(
        "flex-1 rounded-xl border-2 overflow-hidden transition-all",
        isSelected ? "border-primary ring-2 ring-primary/20" : "border-border hover:border-primary/50",
      )}
    >
      <div className="h-24 flex">
        {isSystem ? (
          <>
            <div className="flex-1">{preview}</div>
            <div className="flex-1 dark">{preview}</div>
          </>
        ) : (
          <div className={cn("flex-1", themeId === "dark" && "dark")}>{preview}</div>
        )}
      </div>

      <div
        className={cn(
          "flex items-center justify-center gap-2 py-2 border-t",
          isSelected ? "bg-primary/5 border-primary/20" : "bg-secondary/30 border-border",
        )}
      >
        <Icon className={cn("w-4 h-4", isSelected ? "text-primary" : "text-muted-foreground")} />
        <span className={cn("text-sm font-medium", isSelected ? "text-primary" : "text-foreground")}>{label}</span>
        {isSelected && <Check className="w-3.5 h-3.5 text-primary" />}
      </div>
    </button>
  )
}



function InlineFontInput({
  value,
  onChange,
  mono,
  label,
  vertical,
  testId,
}: {
  value: string
  onChange: (value: string) => void
  mono?: boolean
  label: string
  vertical?: boolean
  testId?: string
}) {
  return (
    <div className={cn("relative", vertical ? "flex flex-col gap-1" : "inline-flex items-center gap-1.5")}>
      <span className="text-[10px] uppercase tracking-wider text-muted-foreground/70">{label}</span>
      <input
        data-testid={testId}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Enter font name..."
        className={cn(
          "px-2 py-1 rounded-md text-xs transition-all w-32",
          "bg-muted border border-border",
          "focus:outline-none focus:ring-1 focus:ring-primary focus:border-primary",
          mono ? "font-mono" : "",
        )}
        style={{ fontFamily: `"${value}", ${mono ? "monospace" : "sans-serif"}` }}
      />
    </div>
  )
}

function WorkspacePreviewWithFonts({
  uiFont,
  chatFont,
  monoFont,
  terminalFont,
  setUiFont,
  setChatFont,
  setMonoFont,
  setTerminalFont,
}: {
  uiFont: string
  chatFont: string
  monoFont: string
  terminalFont: string
  setUiFont: (v: string) => void
  setChatFont: (v: string) => void
  setMonoFont: (v: string) => void
  setTerminalFont: (v: string) => void
}) {
  return (
    <div className="w-full border border-border rounded-xl overflow-hidden bg-card shadow-sm pointer-events-none select-none">
      <div className="flex h-80">
        <div className="w-44 border-r border-border bg-sidebar flex flex-col">
          <div className="h-11 px-3 border-b border-border flex items-center gap-2">
            <div className="w-5 h-5 rounded bg-muted-foreground/20" />
            <div className="h-3 w-16 rounded bg-muted-foreground/20" />
          </div>

          <div className="flex-1 p-3 space-y-2">
            <div className="mb-3 pointer-events-auto">
              <InlineFontInput
                testId="settings-ui-font"
                label="UI Font"
                value={uiFont}
                onChange={setUiFont}
                vertical
              />
            </div>

            <div className="px-1" style={{ fontFamily: `"${uiFont}", sans-serif` }}>
              <p className="text-xs text-muted-foreground leading-relaxed">The quick brown fox jumps over the lazy dog</p>
            </div>
          </div>

          <div className="border-t border-border p-2 opacity-40">
            <div className="h-2.5 w-20 rounded bg-muted-foreground/30" />
          </div>
        </div>

        <div className="flex-1 flex flex-col min-w-0">
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

          <div className="flex-1 p-4 overflow-hidden">
            <div className="space-y-4">
              <div className="space-y-2">
                <div className="pointer-events-auto">
                  <InlineFontInput
                    testId="settings-chat-font"
                    label="Chat Font"
                    value={chatFont}
                    onChange={setChatFont}
                  />
                </div>
                <div className="bg-secondary/40 rounded-lg p-3" style={{ fontFamily: `"${chatFont}", sans-serif` }}>
                  <p className="text-sm leading-relaxed text-muted-foreground">The quick brown fox jumps over the lazy dog</p>
                </div>
              </div>

              <div className="space-y-2">
                <div className="pointer-events-auto">
                  <InlineFontInput
                    testId="settings-code-font"
                    label="Code Font"
                    value={monoFont}
                    onChange={setMonoFont}
                    mono
                  />
                </div>
                <div className="bg-secondary/60 border border-border rounded-lg p-3" style={{ fontFamily: `"${monoFont}", monospace` }}>
                  <pre className="text-sm leading-relaxed">
                    <span className="text-base08">fn</span>{" "}
                    <span className="text-base0e">hello</span>
                    <span className="text-muted-foreground">()</span>{" "}
                    <span className="text-base08">{"->"}</span>{" "}
                    <span className="text-base0d">String</span>{" "}
                    <span className="text-muted-foreground">{"{"}</span>
                    {"\n"}
                    {"    "}
                    <span className="text-base0b">&quot;The quick brown fox&quot;</span>
                    <span className="text-muted-foreground">.to_string()</span>
                    {"\n"}
                    <span className="text-muted-foreground">{"}"}</span>
                  </pre>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="w-48 border-l border-border flex flex-col">
          <div className="h-11 border-b border-border px-3 flex items-center opacity-40">
            <div className="h-2 w-12 rounded bg-muted-foreground/30" />
          </div>
          <div className="flex-1 bg-secondary/40 flex flex-col">
            <div className="px-3 py-2 pointer-events-auto">
              <InlineFontInput
                testId="settings-terminal-font"
                label="Terminal Font"
                value={terminalFont}
                onChange={setTerminalFont}
                mono
                vertical
              />
            </div>
            <div className="flex-1 px-3 pb-3" style={{ fontFamily: `"${terminalFont}", monospace` }}>
              <p className="text-sm leading-relaxed text-muted-foreground">The quick brown fox jumps over the lazy dog</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

type CheckStatus = "idle" | "checking" | "success" | "error"

type SaveStatus = "idle" | "unsaved" | "saving" | "saved"

type AgentType = "codex" | "amp" | "claude"

type ConfigEntry = { kind: "file" | "folder"; path: string; name: string }

type SelectedFile = {
  path: string
  name: string
}

function configEntryIcon(entry: { kind: "file" | "folder"; name: string }): { icon: ElementType; className: string } {
  if (entry.kind === "folder") {
    return { icon: Folder, className: "text-base0a" }
  }

  if (entry.name.endsWith(".toml") || entry.name.endsWith(".json") || entry.name.endsWith(".yaml") || entry.name.endsWith(".yml")) {
    return { icon: FileCode, className: "text-base09" }
  }

  return { icon: FileText, className: "text-base0d" }
}

const AGENT_CONFIG: Record<
  AgentType,
  { name: string; icon: ElementType; iconClassName: string; configPath: string }
> = {
  codex: {
    name: "Codex",
    icon: ({ className }: { className?: string }) => (
      <svg className={className} viewBox="0 0 24 24" fill="currentColor" aria-hidden>
        <path d="M22.282 9.821a5.985 5.985 0 0 0-.516-4.91 6.046 6.046 0 0 0-6.51-2.9A6.065 6.065 0 0 0 4.981 4.18a5.985 5.985 0 0 0-3.998 2.9 6.046 6.046 0 0 0 .743 7.097 5.98 5.98 0 0 0 .51 4.911 6.051 6.051 0 0 0 6.515 2.9A5.985 5.985 0 0 0 13.26 24a6.056 6.056 0 0 0 5.772-4.206 5.99 5.99 0 0 0 3.997-2.9 6.056 6.056 0 0 0-.747-7.073zM13.26 22.43a4.476 4.476 0 0 1-2.876-1.04l.141-.081 4.779-2.758a.795.795 0 0 0 .392-.681v-6.737l2.02 1.168a.071.071 0 0 1 .038.052v5.583a4.504 4.504 0 0 1-4.494 4.494zM3.6 18.304a4.47 4.47 0 0 1-.535-3.014l.142.085 4.783 2.759a.771.771 0 0 0 .78 0l5.843-3.369v2.332a.08.08 0 0 1-.033.062L9.74 19.95a4.5 4.5 0 0 1-6.14-1.646zM2.34 7.896a4.485 4.485 0 0 1 2.366-1.973V11.6a.766.766 0 0 0 .388.676l5.815 3.355-2.02 1.168a.076.076 0 0 1-.071 0l-4.83-2.786A4.504 4.504 0 0 1 2.34 7.872zm16.597 3.855-5.833-3.387L15.119 7.2a.076.076 0 0 1 .071 0l4.83 2.791a4.494 4.494 0 0 1-.676 8.105v-5.678a.79.79 0 0 0-.407-.667zm2.01-3.023-.141-.085-4.774-2.782a.776.776 0 0 0-.785 0L9.409 9.23V6.897a.066.066 0 0 1 .028-.061l4.83-2.787a4.5 4.5 0 0 1 6.68 4.66zm-12.64 4.135-2.02-1.164a.08.08 0 0 1-.038-.057V6.075a4.5 4.5 0 0 1 7.375-3.453l-.142.08-4.778 2.758a.795.795 0 0 0-.393.681zm1.097-2.365 2.602-1.5 2.607 1.5v2.999l-2.597 1.5-2.607-1.5z" />
      </svg>
    ),
    iconClassName: "",
    configPath: "~/.codex",
  },
  amp: {
    name: "Amp",
    icon: Sparkle,
    iconClassName: "text-primary",
    configPath: "~/.config/amp",
  },
  claude: {
    name: "Claude",
    icon: Sparkles,
    iconClassName: "text-primary",
    configPath: "~/.claude",
  },
}

function useAgentConfig(agentType: AgentType) {
  const {
    app,
    checkCodex,
    listCodexConfigDir,
    readCodexConfigFile,
    writeCodexConfigFile,
    setCodexEnabled,
    checkAmp,
    listAmpConfigDir,
    readAmpConfigFile,
    writeAmpConfigFile,
    setAmpEnabled,
    checkClaude,
    listClaudeConfigDir,
    readClaudeConfigFile,
    writeClaudeConfigFile,
  } = useLuban()

  const enabled = useMemo(() => {
    if (agentType === "codex") return app?.agent?.codex_enabled ?? true
    if (agentType === "amp") return app?.agent?.amp_enabled ?? true
    return true
  }, [agentType, app?.agent?.codex_enabled, app?.agent?.amp_enabled])

  const setEnabled = useCallback(
    (value: boolean) => {
      if (agentType === "codex") setCodexEnabled(value)
      else if (agentType === "amp") setAmpEnabled(value)
    },
    [agentType, setCodexEnabled, setAmpEnabled],
  )

  const check = useCallback(async () => {
    if (agentType === "codex") return checkCodex()
    if (agentType === "amp") return checkAmp()
    return checkClaude()
  }, [agentType, checkCodex, checkAmp, checkClaude])

  const listDir = useCallback(
    async (path: string) => {
      if (agentType === "codex") return listCodexConfigDir(path)
      if (agentType === "amp") return listAmpConfigDir(path)
      return listClaudeConfigDir(path)
    },
    [agentType, listCodexConfigDir, listAmpConfigDir, listClaudeConfigDir],
  )

  const readFile = useCallback(
    async (path: string) => {
      if (agentType === "codex") return readCodexConfigFile(path)
      if (agentType === "amp") return readAmpConfigFile(path)
      return readClaudeConfigFile(path)
    },
    [agentType, readCodexConfigFile, readAmpConfigFile, readClaudeConfigFile],
  )

  const writeFile = useCallback(
    async (path: string, content: string) => {
      if (agentType === "codex") return writeCodexConfigFile(path, content)
      if (agentType === "amp") return writeAmpConfigFile(path, content)
      return writeClaudeConfigFile(path, content)
    },
    [agentType, writeCodexConfigFile, writeAmpConfigFile, writeClaudeConfigFile],
  )

  const hasSetEnabled = agentType === "codex" || agentType === "amp"

  return { enabled, setEnabled, check, listDir, readFile, writeFile, hasSetEnabled }
}

function ConfigFileTree({
  entries,
  level = 0,
  selectedPath,
  expandedFolders,
  loadingDirs,
  childrenForPath,
  onSelectFile,
  onToggleFolder,
}: {
  entries: ConfigEntry[]
  level?: number
  selectedPath: string | null
  expandedFolders: Set<string>
  loadingDirs: Set<string>
  childrenForPath: (path: string) => ConfigEntry[]
  onSelectFile: (file: SelectedFile) => void
  onToggleFolder: (path: string) => void
}) {
  return (
    <div className="space-y-0.5">
      {entries.map((entry) => {
        const isFolder = entry.kind === "folder"
        const isExpanded = isFolder && expandedFolders.has(entry.path)
        const isLoading = isFolder && loadingDirs.has(entry.path)
        const children = isFolder ? childrenForPath(entry.path) : []
        const isSelected = selectedPath === entry.path
        const { icon: Icon, className } = configEntryIcon(entry)

        return (
          <div key={entry.path}>
            <button
              onClick={() => {
                if (isFolder) {
                  onToggleFolder(entry.path)
                } else {
                  onSelectFile({ path: entry.path, name: entry.name })
                }
              }}
              className={cn(
                "w-full flex items-center gap-1.5 px-2 py-1 rounded text-left transition-colors text-xs",
                isSelected ? "bg-primary/15 text-primary" : "text-muted-foreground hover:text-foreground hover:bg-accent",
              )}
              style={{ paddingLeft: `${8 + level * 12}px` }}
            >
              {isFolder ? (
                <ChevronRight
                  className={cn(
                    "w-3 h-3 text-muted-foreground transition-transform flex-shrink-0",
                    isExpanded && "rotate-90",
                  )}
                />
              ) : (
                <div className="w-3" />
              )}
              <Icon className={cn("w-3.5 h-3.5 flex-shrink-0", className)} />
              <span className="truncate">{entry.name}</span>
            </button>

            {isFolder && isExpanded && (
              <ConfigFileTree
                entries={children}
                level={level + 1}
                selectedPath={selectedPath}
                expandedFolders={expandedFolders}
                loadingDirs={loadingDirs}
                childrenForPath={childrenForPath}
                onSelectFile={onSelectFile}
                onToggleFolder={onToggleFolder}
              />
            )}

            {isFolder && isExpanded && isLoading && children.length === 0 && (
              <div
                className="w-full flex items-center gap-1.5 px-2 py-1 rounded text-left text-xs text-muted-foreground"
                style={{ paddingLeft: `${8 + (level + 1) * 12}px` }}
              >
                <Loader2 className="w-3 h-3 animate-spin" />
                <span>Loading…</span>
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}

function AgentConfigPanel({
  initialAgentId,
  initialAgentFilePath,
}: {
  initialAgentId?: string | null
  initialAgentFilePath?: string | null
}) {
  const agentTypes: AgentType[] = ["codex", "amp", "claude"]
  const [selectedAgent, setSelectedAgent] = useState<AgentType>(() => {
    if (initialAgentId && agentTypes.includes(initialAgentId as AgentType)) {
      return initialAgentId as AgentType
    }
    return "codex"
  })

  const { app, setCodexEnabled, setAmpEnabled, setClaudeEnabled } = useLuban()
  const codexEnabled = app?.agent?.codex_enabled ?? true
  const ampEnabled = app?.agent?.amp_enabled ?? true
  const claudeEnabled = app?.agent?.claude_enabled ?? true

  const getAgentEnabled = (agent: AgentType) => {
    if (agent === "codex") return codexEnabled
    if (agent === "amp") return ampEnabled
    if (agent === "claude") return claudeEnabled
    return true
  }

  const toggleAgentEnabled = (agent: AgentType) => {
    if (agent === "codex") setCodexEnabled(!codexEnabled)
    else if (agent === "amp") setAmpEnabled(!ampEnabled)
    else if (agent === "claude") setClaudeEnabled(!claudeEnabled)
  }

  return (
    <div className="rounded-xl border border-border bg-card overflow-hidden shadow-sm h-[400px] flex">
      <div className="w-44 flex-shrink-0 border-r border-border bg-sidebar flex flex-col">
        <div className="h-11 px-3 flex items-center gap-2 border-b border-border">
          <Bot className="w-4 h-4 text-muted-foreground" />
          <span className="text-sm font-medium">Agent</span>
        </div>
        <div className="flex-1 overflow-y-auto py-1.5">
          {agentTypes.map((agent) => {
            const agentConfig = AGENT_CONFIG[agent]
            const Icon = agentConfig.icon
            const isSelected = selectedAgent === agent
            const isEnabled = getAgentEnabled(agent)
            return (
              <div
                key={agent}
                onClick={() => setSelectedAgent(agent)}
                className={cn(
                  "w-full flex items-center justify-between px-3 py-2 text-left transition-colors text-sm cursor-pointer",
                  isSelected ? "bg-primary/10 text-primary" : "text-muted-foreground hover:text-foreground hover:bg-accent",
                  !isEnabled && "opacity-40 text-muted-foreground/60",
                )}
              >
                <div className="flex items-center gap-2">
                  <Icon className={cn("w-4 h-4", agentConfig.iconClassName)} />
                  <span className="truncate">{agentConfig.name}</span>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    toggleAgentEnabled(agent)
                  }}
                  className={cn("relative w-7 h-4 rounded-full transition-colors flex-shrink-0", isEnabled ? "bg-primary" : "bg-muted-foreground/30")}
                  title={isEnabled ? `Disable ${agentConfig.name}` : `Enable ${agentConfig.name}`}
                >
                  <div
                    className={cn(
                      "absolute top-0.5 w-3 h-3 rounded-full bg-white shadow transition-transform",
                      isEnabled ? "translate-x-3.5" : "translate-x-0.5",
                    )}
                  />
                </button>
              </div>
            )
          })}
        </div>
      </div>

      <AgentConfigContent
        agentType={selectedAgent}
        initialFilePath={selectedAgent === initialAgentId ? initialAgentFilePath : null}
        autoFocusEditor={selectedAgent === initialAgentId && initialAgentFilePath != null}
      />
    </div>
  )
}

function AgentConfigContent({
  agentType,
  initialFilePath,
  autoFocusEditor = false,
}: {
  agentType: AgentType
  initialFilePath?: string | null
  autoFocusEditor?: boolean
}) {
  const config = AGENT_CONFIG[agentType]
  const { listDir, readFile, writeFile } = useAgentConfig(agentType)

  const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle")
  const [selectedFile, setSelectedFile] = useState<SelectedFile | null>(null)
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(() => new Set())
  const [dirEntries, setDirEntries] = useState<Record<string, ConfigEntry[]>>({})
  const [loadingDirs, setLoadingDirs] = useState<Set<string>>(() => new Set())
  const [fileContents, setFileContents] = useState<Record<string, string>>({})
  const saveTimeoutRef = useRef<number | null>(null)
  const saveIdleTimeoutRef = useRef<number | null>(null)
  const initialSelectionRef = useRef<string | null>(null)
  const editorRef = useRef<HTMLTextAreaElement>(null)
  const highlightRef = useRef<HTMLDivElement>(null)
  const highlighter = useShikiHighlighter()

  useEffect(() => {
    setSaveStatus("idle")
    setSelectedFile(null)
    setExpandedFolders(new Set())
    setDirEntries({})
    setLoadingDirs(new Set())
    setFileContents({})
    initialSelectionRef.current = null
  }, [agentType])

  const loadDirInternal = useCallback(
    async (path: string): Promise<ConfigEntry[]> => {
      setLoadingDirs((prev) => {
        const next = new Set(prev)
        next.add(path)
        return next
      })
      try {
        const res = await listDir(path)
        setDirEntries((prev) => ({ ...prev, [path]: res.entries }))
        return res.entries
      } catch (err) {
        toast.error(err instanceof Error ? err.message : String(err))
        return []
      } finally {
        setLoadingDirs((prev) => {
          const next = new Set(prev)
          next.delete(path)
          return next
        })
      }
    },
    [listDir],
  )

  useEffect(() => {
    void loadDirInternal("")
  }, [loadDirInternal])

  const handleSelectFile = useCallback(
    async (file: SelectedFile) => {
      setSelectedFile(file)
      if (fileContents[file.path] != null) return
      try {
        const contents = await readFile(file.path)
        setFileContents((prev) => ({ ...prev, [file.path]: contents }))
      } catch (err) {
        toast.error(err instanceof Error ? err.message : String(err))
      }
    },
    [fileContents, readFile],
  )

  useEffect(() => {
    const target = initialFilePath?.trim()
    if (!target) return
    if (selectedFile?.path === target) return
    if (initialSelectionRef.current === target) return
    if (!dirEntries[""]) return

    initialSelectionRef.current = target
    void (async () => {
      const segments = target.split("/").filter(Boolean)
      let parent = ""

      for (const segment of segments.slice(0, -1)) {
        parent = parent ? `${parent}/${segment}` : segment
        setExpandedFolders((prev) => {
          const next = new Set(prev)
          next.add(parent)
          return next
        })
        if (!dirEntries[parent] && !loadingDirs.has(parent)) {
          await loadDirInternal(parent)
        }
      }

      const container = parent || ""
      const entries = dirEntries[container] ?? []
      const entry = entries.find((e) => e.kind === "file" && e.path === target)

      if (entry) {
        await handleSelectFile({ path: entry.path, name: entry.name })
      }
    })()
  }, [dirEntries, handleSelectFile, initialFilePath, loadDirInternal, loadingDirs, selectedFile?.path])

  useEffect(() => {
    if (!autoFocusEditor) return
    const target = initialSelectionRef.current
    if (!target) return
    if (!selectedFile || selectedFile.path !== target) return
    if (fileContents[selectedFile.path] == null) return
    editorRef.current?.focus()
  }, [autoFocusEditor, fileContents, selectedFile])

  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current != null) window.clearTimeout(saveTimeoutRef.current)
      if (saveIdleTimeoutRef.current != null) window.clearTimeout(saveIdleTimeoutRef.current)
    }
  }, [])

  const handleEditorScroll = () => {
    if (!editorRef.current || !highlightRef.current) return
    highlightRef.current.scrollTop = editorRef.current.scrollTop
    highlightRef.current.scrollLeft = editorRef.current.scrollLeft
  }

  const getFileLanguage = (fileName: string): string => {
    if (fileName.endsWith(".md")) return "markdown"
    if (fileName.endsWith(".toml")) return "toml"
    if (fileName.endsWith(".yaml") || fileName.endsWith(".yml")) return "yaml"
    if (fileName.endsWith(".json")) return "json"
    return "markdown"
  }

  const handleEditInLuban = (e: MouseEvent) => {
    e.stopPropagation()
    addProjectAndOpen(config.configPath)
  }

  const handleToggleFolder = (path: string) => {
    const willExpand = !expandedFolders.has(path)
    setExpandedFolders((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })

    if (willExpand && !dirEntries[path] && !loadingDirs.has(path)) {
      void loadDirInternal(path)
    }
  }

  const handleContentChange = (content: string) => {
    if (!selectedFile) return

    setFileContents((prev) => ({ ...prev, [selectedFile.path]: content }))
    setSaveStatus("unsaved")

    if (saveTimeoutRef.current != null) window.clearTimeout(saveTimeoutRef.current)
    if (saveIdleTimeoutRef.current != null) window.clearTimeout(saveIdleTimeoutRef.current)

    saveTimeoutRef.current = window.setTimeout(() => {
      setSaveStatus("saving")
      const path = selectedFile.path
      void writeFile(path, content)
        .then(() => {
          setSaveStatus("saved")
          saveIdleTimeoutRef.current = window.setTimeout(() => {
            setSaveStatus("idle")
          }, 1500)
        })
        .catch((err) => {
          setSaveStatus("unsaved")
          toast.error(err instanceof Error ? err.message : String(err))
        })
    }, 800)
  }

  const currentContent = selectedFile ? (fileContents[selectedFile.path] ?? "") : ""

  return (
    <div className="flex-1 flex min-w-0">
      <div className="w-44 flex-shrink-0 border-r border-border bg-sidebar flex flex-col">
        <div className="h-11 px-3 flex items-center gap-2 border-b border-border">
          <Folder className="w-4 h-4 text-muted-foreground" />
          <span className="text-sm font-medium">Files</span>
        </div>
        <div className="flex-1 overflow-y-auto py-1.5">
          {(dirEntries[""] ?? []).length === 0 ? (
            <div className="px-2 py-1.5 text-xs text-muted-foreground">No config found.</div>
          ) : (
            <ConfigFileTree
              entries={dirEntries[""] ?? []}
              selectedPath={selectedFile?.path ?? null}
              expandedFolders={expandedFolders}
              loadingDirs={loadingDirs}
              childrenForPath={(path) => dirEntries[path] ?? []}
              onSelectFile={handleSelectFile}
              onToggleFolder={handleToggleFolder}
            />
          )}
        </div>
      </div>

      <div className="flex-1 flex flex-col min-w-0 bg-background">
        <div className="flex items-center justify-between h-11 px-3 border-b border-border">
          <div className="flex items-center gap-2">
            {saveStatus !== "idle" && (
              <span
                className={cn(
                  "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px]",
                  saveStatus === "saved" ? "bg-status-success/10 text-status-success" : "bg-status-warning/10 text-status-warning",
                )}
              >
                {saveStatus === "saving" && <Loader2 className="w-2.5 h-2.5 animate-spin" />}
                {saveStatus === "saved" && <CheckCircle2 className="w-2.5 h-2.5" />}
                {saveStatus === "saving" ? "Saving..." : saveStatus === "unsaved" ? "Unsaved" : "Saved"}
              </span>
            )}
          </div>
          <button
            data-testid={`settings-${agentType}-edit-in-luban`}
            onClick={handleEditInLuban}
            className="flex items-center gap-1.5 px-2 py-1 rounded text-xs bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
          >
            <Pencil className="w-3.5 h-3.5" />
            Edit in Luban
          </button>
        </div>

        <div className="flex-1 relative overflow-hidden">
          {selectedFile ? (
            <>
              <div
                ref={highlightRef}
                className="absolute inset-0 p-4 text-sm font-mono leading-relaxed whitespace-pre-wrap break-words overflow-hidden pointer-events-none"
                aria-hidden="true"
              >
                <MarkdownHighlight text={currentContent} highlighter={highlighter} lang={getFileLanguage(selectedFile.name)} />
              </div>
              <textarea
                ref={editorRef}
                data-testid={`settings-${agentType}-editor`}
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
    </div>
  )
}

function AllSettings({
  initialAgentId,
  initialAgentFilePath,
}: {
  initialAgentId?: string | null
  initialAgentFilePath?: string | null
}) {
  const { theme, setTheme } = useTheme()
  const { fonts, setFonts } = useAppearance()
  const { app, setAppearanceTheme, setAppearanceFonts, setTaskPromptTemplate, setSystemPromptTemplate } = useLuban()
  const resolvedTheme = theme ?? "system"

  return (
    <div className="space-y-12">
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
              isSelected={resolvedTheme === option.id}
              onClick={() => {
                setTheme(option.id)
                setAppearanceTheme(option.id)
              }}
              testId={`settings-theme-${option.id}`}
            />
          ))}
        </div>
      </section>

      <section id="fonts" className="scroll-mt-8">
        <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
          <Type className="w-4 h-4 text-muted-foreground" />
          Fonts
        </h3>
        <WorkspacePreviewWithFonts
          uiFont={fonts.uiFont}
          chatFont={fonts.chatFont}
          monoFont={fonts.codeFont}
          terminalFont={fonts.terminalFont}
          setUiFont={(uiFont) => {
            const next = { ...fonts, uiFont }
            setFonts({ uiFont })
            setAppearanceFonts({
              ui_font: next.uiFont,
              chat_font: next.chatFont,
              code_font: next.codeFont,
              terminal_font: next.terminalFont,
            })
          }}
          setChatFont={(chatFont) => {
            const next = { ...fonts, chatFont }
            setFonts({ chatFont })
            setAppearanceFonts({
              ui_font: next.uiFont,
              chat_font: next.chatFont,
              code_font: next.codeFont,
              terminal_font: next.terminalFont,
            })
          }}
          setMonoFont={(codeFont) => {
            const next = { ...fonts, codeFont }
            setFonts({ codeFont })
            setAppearanceFonts({
              ui_font: next.uiFont,
              chat_font: next.chatFont,
              code_font: next.codeFont,
              terminal_font: next.terminalFont,
            })
          }}
          setTerminalFont={(terminalFont) => {
            const next = { ...fonts, terminalFont }
            setFonts({ terminalFont })
            setAppearanceFonts({
              ui_font: next.uiFont,
              chat_font: next.chatFont,
              code_font: next.codeFont,
              terminal_font: next.terminalFont,
            })
          }}
        />
      </section>

      <section id="agent" className="scroll-mt-8">
        <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
          <Bot className="w-4 h-4 text-muted-foreground" />
          Agent
        </h3>
        <AgentConfigPanel initialAgentId={initialAgentId} initialAgentFilePath={initialAgentFilePath} />
      </section>

      <section id="task" className="scroll-mt-8">
        <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
          <ListTodo className="w-4 h-4 text-muted-foreground" />
          Task
        </h3>
        <TaskPromptEditor
          templates={app?.task?.prompt_templates ?? []}
          defaultTemplates={app?.task?.default_prompt_templates ?? []}
          systemTemplates={app?.task?.system_prompt_templates ?? []}
          defaultSystemTemplates={app?.task?.default_system_prompt_templates ?? []}
          setTaskPromptTemplate={setTaskPromptTemplate}
          setSystemPromptTemplate={setSystemPromptTemplate}
        />
      </section>
    </div>
  )
}

export function SettingsPanel({
  open,
  onOpenChange,
  initialSectionId,
  initialAgentId,
  initialAgentFilePath,
}: SettingsPanelProps) {
  const [expandedItems, setExpandedItems] = useState<Set<string>>(() => {
    const next = new Set<string>(["appearance"])
    if (initialSectionId === "theme" || initialSectionId === "fonts") next.add("appearance")
    return next
  })
  const [activeItem, setActiveItem] = useState<string>(initialSectionId ?? "theme")
  const contentRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    if (!initialSectionId) return
    if (initialSectionId === "theme" || initialSectionId === "fonts") {
      setExpandedItems((prev) => new Set(prev).add("appearance"))
    }
    setActiveItem(initialSectionId)
    window.requestAnimationFrame(() => {
      document.getElementById(initialSectionId)?.scrollIntoView({ behavior: "smooth", block: "start" })
    })
  }, [open, initialSectionId])

  const toggleExpanded = (id: string) => {
    setExpandedItems((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
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
  if (!open) return null

  return (
    <div
      data-testid="settings-panel"
      className="fixed inset-0 z-50 bg-background flex"
      onKeyDown={(e) => {
        if (e.key !== "Escape") return
        e.stopPropagation()
        ;(e.nativeEvent as unknown as { stopImmediatePropagation?: () => void }).stopImmediatePropagation?.()
        e.preventDefault()
        onOpenChange(false)
      }}
    >
      <div className="w-60 flex-shrink-0 border-r border-border bg-sidebar flex flex-col">
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
            title="Close settings"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto py-1.5">
          {tocItems.map((item) => {
            const Icon = item.icon
            const isExpanded = expandedItems.has(item.id)
            const hasChildren = !!item.children?.length

            return (
              <div key={item.id}>
                <div className="flex items-center hover:bg-sidebar-accent/50 transition-colors">
                  <button
                    onClick={() => (hasChildren ? toggleExpanded(item.id) : scrollToSection(item.id))}
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
                    <span
                      className={cn(
                        "text-sm truncate flex-1",
                        !hasChildren && activeItem === item.id ? "text-foreground" : "text-muted-foreground",
                      )}
                    >
                      {item.label}
                    </span>
                  </button>
                </div>

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
                            isActive ? "bg-sidebar-accent" : "hover:bg-sidebar-accent/30",
                          )}
                        >
                          <ChildIcon className={cn("w-3.5 h-3.5", isActive ? "text-primary" : "text-muted-foreground")} />
                          <span className={cn("text-xs", isActive ? "text-foreground" : "text-muted-foreground")}>{child.label}</span>
                        </button>
                      )
                    })}
                  </div>
                )}
              </div>
            )
          })}
        </div>

        <div className="border-t border-border p-2">
          <div className="px-3 py-1.5 text-xs text-muted-foreground">Luban v0.1.6</div>
        </div>
      </div>

      <div className="flex-1 overflow-hidden flex flex-col">
        <div className="h-11 px-8 border-b border-border flex items-center">
          <h2 className="text-sm font-medium">Settings</h2>
        </div>
        <div ref={contentRef} className="flex-1 overflow-y-auto p-8">
          <div className="max-w-4xl">
            <AllSettings initialAgentId={initialAgentId} initialAgentFilePath={initialAgentFilePath} />
          </div>
        </div>
      </div>
    </div>
  )
}
