"use client"

import type { ElementType, MouseEvent } from "react"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useTheme } from "next-themes"
import {
  ArrowLeft,
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
import { buildFontFamilyList } from "@/lib/font-family"
import type {
  AgentRunnerKind,
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
  initialSectionId?: "theme" | "fonts" | "agent" | "task" | "telegram"
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
  {
    id: "integrations",
    label: "Integrations",
    icon: MessageSquare,
    children: [{ id: "telegram", label: "Telegram", icon: MessageSquare }],
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
  {
    id: "auto-update-task-status",
    label: "Suggest Task Status",
    icon: CheckCircle2,
    description: "Suggest task status based on the latest agent progress (manual apply)",
  },
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
  { id: "known_context", label: "known_context", description: "Known context collected for the task" },
  { id: "context_json", label: "context_json", description: "Structured JSON context collected for the task" },
]

const variablesByTaskType: Record<string, string[]> = {
  "infer-type": ["task_input", "context_json"],
  "rename-branch": ["task_input", "context_json"],
  "auto-title-thread": ["task_input", "context_json"],
  "auto-update-task-status": ["task_input", "context_json"],
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
    taskType === "infer-type" ||
    taskType === "rename-branch" ||
    taskType === "auto-title-thread" ||
    taskType === "auto-update-task-status"

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
        placeholder="Enter font family..."
        className={cn(
          "px-2 py-1 rounded-md text-xs transition-all w-32",
          "bg-muted border border-border",
          "focus:outline-none focus:ring-1 focus:ring-primary focus:border-primary",
          mono ? "font-mono" : "",
        )}
        style={{ fontFamily: buildFontFamilyList(value, [mono ? "monospace" : "sans-serif"]) }}
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

            <div className="px-1" style={{ fontFamily: buildFontFamilyList(uiFont, ["sans-serif"]) }}>
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
                <div className="bg-secondary/40 rounded-lg p-3" style={{ fontFamily: buildFontFamilyList(chatFont, ["sans-serif"]) }}>
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
                <div className="bg-secondary/60 border border-border rounded-lg p-3" style={{ fontFamily: buildFontFamilyList(monoFont, ["monospace"]) }}>
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
            <div
              data-testid="settings-terminal-font-preview"
              className="flex-1 px-3 pb-3"
              style={{ fontFamily: buildFontFamilyList(terminalFont, ["monospace"]) }}
            >
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

type AgentType = "codex" | "amp" | "claude" | "droid"

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
  droid: {
    name: "Droid",
    icon: ({ className }: { className?: string }) => (
      <svg className={className} viewBox="0 0 20 20" fill="currentColor" aria-hidden>
        <path d="M13.9062 3.20484C13.8706 3.19623 13.8373 3.18019 13.8086 3.15788C13.7799 3.13552 13.7565 3.10743 13.74 3.07546C13.7234 3.04349 13.7142 3.00841 13.7128 2.97263C13.7115 2.93682 13.7181 2.90121 13.7322 2.86815C14.2188 1.71337 14.4334 0.789376 14.087 0.402562C13.1693 -0.623688 9.48933 1.41705 8.31598 2.10822C8.28457 2.12665 8.24942 2.13825 8.21299 2.14222C8.17656 2.14618 8.13967 2.14242 8.10486 2.13122C8.07004 2.12003 8.03809 2.10163 8.01123 2.0773C7.98437 2.05297 7.96323 2.02326 7.94918 1.99024C7.45597 0.83788 6.93748 0.036362 6.41194 0.00160127C5.01894 -0.0913635 3.89616 3.88951 3.56749 5.18697C3.5587 5.22173 3.54233 5.25423 3.5195 5.2822C3.49662 5.31021 3.46781 5.33305 3.43507 5.34918C3.40233 5.3653 3.36639 5.37432 3.32971 5.37561C3.29307 5.37694 3.25656 5.37052 3.22266 5.35677C2.03854 4.88225 1.09066 4.67288 0.694439 5.01078C-0.35788 5.90567 1.73432 9.49452 2.44305 10.6388C2.462 10.6694 2.47393 10.7037 2.47804 10.7392C2.48214 10.7748 2.47833 10.8108 2.46684 10.8447C2.45536 10.8787 2.43646 10.9099 2.41147 10.9361C2.38648 10.9623 2.35598 10.9828 2.32203 10.9965C1.14081 11.4775 0.318936 11.9831 0.282878 12.4956C0.187966 13.8541 4.26959 14.9491 5.60043 15.2696C5.63599 15.2783 5.66919 15.2943 5.69783 15.3167C5.72642 15.339 5.7498 15.3671 5.76625 15.399C5.78275 15.4309 5.79199 15.4659 5.79332 15.5016C5.79469 15.5373 5.78814 15.5729 5.77409 15.6059C5.28751 16.7607 5.07282 17.6851 5.41931 18.0715C6.33693 19.0977 10.0173 17.0574 11.1907 16.3662C11.2221 16.3477 11.2573 16.3361 11.2937 16.3321C11.3302 16.3281 11.3671 16.3318 11.4019 16.343C11.4368 16.3542 11.4687 16.3727 11.4955 16.397C11.5224 16.4214 11.5435 16.4511 11.5575 16.4842C12.0507 17.6362 12.5688 18.4377 13.0947 18.4729C14.4877 18.5654 15.6105 14.5849 15.9388 13.2871C15.9476 13.2524 15.9641 13.2199 15.987 13.192C16.0099 13.164 16.0388 13.1412 16.0716 13.1251C16.1043 13.1091 16.1403 13.1001 16.177 13.0988C16.2136 13.0975 16.2502 13.1039 16.284 13.1177C17.4681 13.5922 18.4156 13.8012 18.8122 13.4637C19.8646 12.5688 17.7719 8.97953 17.0632 7.83525C17.0444 7.80462 17.0326 7.77034 17.0286 7.73485C17.0245 7.69932 17.0284 7.66335 17.0399 7.62944C17.0514 7.59549 17.0702 7.56436 17.0951 7.53817C17.12 7.51198 17.1504 7.49129 17.1842 7.47758C18.3659 6.99659 19.1877 6.4909 19.2234 5.97838C19.3187 4.61989 15.2367 3.52497 13.9062 3.20484ZM12.3081 1.90249C12.5758 2.37055 11.1961 5.48935 10.1699 7.67079C10.1527 7.70725 10.1245 7.73772 10.0891 7.75809C10.0536 7.77847 10.0126 7.78776 9.97155 7.78473C9.93052 7.7817 9.8914 7.76646 9.85952 7.74112C9.82761 7.71573 9.80444 7.68146 9.79313 7.64286C9.37866 6.22454 8.90493 4.55809 8.39805 3.14341C8.37815 3.08787 8.37915 3.02724 8.40087 2.97239C8.42258 2.9175 8.46361 2.87195 8.51658 2.84386C9.78235 2.16966 11.9483 1.27437 12.3081 1.90249ZM6.24201 2.28849C6.77045 2.43481 8.05612 5.59161 8.91198 7.84176C8.92628 7.87935 8.92839 7.9203 8.91811 7.95914C8.90779 7.99794 8.88558 8.03274 8.85441 8.0589C8.8232 8.08505 8.78457 8.10126 8.74367 8.10534C8.70276 8.10946 8.66156 8.10126 8.62559 8.08185C7.30304 7.36603 5.76082 6.51318 4.37652 5.86242C4.32231 5.83676 4.27921 5.79322 4.25483 5.73947C4.23046 5.68575 4.22644 5.62532 4.24348 5.56898C4.65089 4.22058 5.53204 2.09246 6.24201 2.28849ZM2.23251 6.74474C2.71204 6.48363 5.91044 7.82923 8.14688 8.83002C8.18431 8.84675 8.21556 8.87424 8.23645 8.90884C8.25734 8.94339 8.26687 8.98341 8.26376 9.02343C8.26061 9.06344 8.24503 9.10156 8.219 9.13268C8.19301 9.16376 8.15787 9.1864 8.11828 9.19743C6.66435 9.60162 4.95511 10.0636 3.50449 10.558C3.44763 10.5772 3.38554 10.5762 3.32938 10.555C3.27322 10.5339 3.22655 10.4939 3.19779 10.4423C2.50771 9.2079 1.58802 7.09558 2.23251 6.74474ZM2.62832 12.6606C2.77794 12.1452 6.0153 10.8914 8.32261 10.0567C8.36116 10.0428 8.40319 10.0407 8.44297 10.0508C8.4828 10.0608 8.51849 10.0825 8.5453 10.1129C8.57208 10.1433 8.58874 10.181 8.59293 10.2209C8.59711 10.2607 8.5887 10.3009 8.56881 10.336C7.83438 11.6258 6.95986 13.1298 6.29258 14.4794C6.26651 14.5325 6.22187 14.5747 6.16671 14.5986C6.11158 14.6224 6.04954 14.6263 5.99168 14.6096C4.60903 14.2147 2.42689 13.353 2.62832 12.6606ZM7.19776 16.5707C6.92961 16.1031 8.30977 12.9839 9.33597 10.8029C9.35313 10.7664 9.38136 10.7359 9.41679 10.7155C9.45227 10.6952 9.49326 10.6859 9.53429 10.6889C9.57537 10.6919 9.61445 10.7072 9.64632 10.7325C9.67824 10.7579 9.7014 10.7922 9.71272 10.8308C10.1272 12.2487 10.6009 13.9156 11.1078 15.3303C11.1276 15.3858 11.1265 15.4463 11.1047 15.5011C11.0829 15.5559 11.0418 15.6014 10.9888 15.6294C9.7235 16.3023 7.5571 17.1993 7.19901 16.5707H7.19776ZM13.2638 16.1847C12.735 16.0388 11.4493 12.8817 10.5935 10.6315C10.5791 10.5938 10.5769 10.5528 10.5872 10.5139C10.5975 10.475 10.6198 10.4401 10.6511 10.414C10.6823 10.3878 10.7211 10.3716 10.762 10.3676C10.803 10.3635 10.8443 10.3719 10.8803 10.3914C12.2024 11.1072 13.745 11.9605 15.1289 12.6113C15.1832 12.6368 15.2264 12.6803 15.2508 12.7341C15.2752 12.7878 15.2792 12.8483 15.262 12.9047C14.855 14.2551 13.9738 16.3811 13.2638 16.1847ZM17.2733 11.7285C16.7934 11.99 13.5954 10.644 11.3586 9.64322C11.3211 9.62648 11.2899 9.599 11.269 9.5644C11.2481 9.52984 11.2386 9.48983 11.2417 9.44981C11.2448 9.40979 11.2604 9.37168 11.2864 9.34056C11.3124 9.30947 11.3476 9.28688 11.3871 9.27584C12.8415 8.87165 14.5503 8.40962 16.0009 7.91529C16.0579 7.89601 16.1201 7.89706 16.1763 7.91832C16.2326 7.93958 16.2793 7.97963 16.3081 8.03133C16.9977 9.26534 17.9174 11.378 17.2733 11.7285ZM16.8775 5.81266C16.7275 6.32842 13.4905 7.58227 11.1832 8.41693C11.1446 8.43092 11.1025 8.43302 11.0627 8.42299C11.0228 8.41293 10.987 8.39122 10.9602 8.36075C10.9334 8.33027 10.9168 8.29248 10.9126 8.2525C10.9085 8.21257 10.917 8.17231 10.937 8.13719C11.6711 6.84781 12.5456 5.3434 13.2129 3.99379C13.2391 3.94088 13.2837 3.8988 13.3389 3.87504C13.394 3.85127 13.456 3.84735 13.5138 3.86404C14.8964 4.26096 17.0785 5.12028 16.8775 5.81266Z" />
      </svg>
    ),
    iconClassName: "",
    configPath: "~/.factory",
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
    checkDroid,
    listDroidConfigDir,
    readDroidConfigFile,
    writeDroidConfigFile,
    setDroidEnabled,
  } = useLuban()

  const enabled = useMemo(() => {
    if (agentType === "codex") return app?.agent?.codex_enabled ?? true
    if (agentType === "amp") return app?.agent?.amp_enabled ?? true
    if (agentType === "droid") return app?.agent?.droid_enabled ?? true
    return true
  }, [agentType, app?.agent?.codex_enabled, app?.agent?.amp_enabled, app?.agent?.droid_enabled])

  const setEnabled = useCallback(
    (value: boolean) => {
      if (agentType === "codex") setCodexEnabled(value)
      else if (agentType === "amp") setAmpEnabled(value)
      else if (agentType === "droid") setDroidEnabled(value)
    },
    [agentType, setCodexEnabled, setAmpEnabled, setDroidEnabled],
  )

  const check = useCallback(async () => {
    if (agentType === "codex") return checkCodex()
    if (agentType === "amp") return checkAmp()
    if (agentType === "droid") return checkDroid()
    return checkClaude()
  }, [agentType, checkCodex, checkAmp, checkClaude, checkDroid])

  const listDir = useCallback(
    async (path: string) => {
      if (agentType === "codex") return listCodexConfigDir(path)
      if (agentType === "amp") return listAmpConfigDir(path)
      if (agentType === "droid") return listDroidConfigDir(path)
      return listClaudeConfigDir(path)
    },
    [agentType, listCodexConfigDir, listAmpConfigDir, listClaudeConfigDir, listDroidConfigDir],
  )

  const readFile = useCallback(
    async (path: string) => {
      if (agentType === "codex") return readCodexConfigFile(path)
      if (agentType === "amp") return readAmpConfigFile(path)
      if (agentType === "droid") return readDroidConfigFile(path)
      return readClaudeConfigFile(path)
    },
    [agentType, readCodexConfigFile, readAmpConfigFile, readClaudeConfigFile, readDroidConfigFile],
  )

  const writeFile = useCallback(
    async (path: string, content: string) => {
      if (agentType === "codex") return writeCodexConfigFile(path, content)
      if (agentType === "amp") return writeAmpConfigFile(path, content)
      if (agentType === "droid") return writeDroidConfigFile(path, content)
      return writeClaudeConfigFile(path, content)
    },
    [agentType, writeCodexConfigFile, writeAmpConfigFile, writeClaudeConfigFile, writeDroidConfigFile],
  )

  const hasSetEnabled = agentType === "codex" || agentType === "amp" || agentType === "droid"

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
  const agentTypes: AgentType[] = ["codex", "amp", "claude", "droid"]
  const [selectedAgent, setSelectedAgent] = useState<AgentType>(() => {
    if (initialAgentId && agentTypes.includes(initialAgentId as AgentType)) {
      return initialAgentId as AgentType
    }
    return "codex"
  })

  const { app, setCodexEnabled, setAmpEnabled, setClaudeEnabled, setDroidEnabled, setAgentRunner } = useLuban()
  const codexEnabled = app?.agent?.codex_enabled ?? true
  const ampEnabled = app?.agent?.amp_enabled ?? true
  const claudeEnabled = app?.agent?.claude_enabled ?? true
  const droidEnabled = app?.agent?.droid_enabled ?? true

  // Reason: Resolve effective runner like the backend's resolve_enabled_runner().
  // If the stored default_runner is disabled, fall back to the first enabled runner.
  const effectiveRunner: AgentRunnerKind = useMemo(() => {
    const stored = app?.agent?.default_runner ?? "codex"
    const enabledMap: Record<AgentRunnerKind, boolean> = {
      codex: codexEnabled,
      amp: ampEnabled,
      claude: claudeEnabled,
      droid: droidEnabled,
    }
    if (enabledMap[stored]) return stored
    const fallbackOrder: AgentRunnerKind[] = ["codex", "droid", "amp", "claude"]
    return fallbackOrder.find((r) => enabledMap[r]) ?? stored
  }, [app?.agent?.default_runner, codexEnabled, ampEnabled, claudeEnabled, droidEnabled])

  const getAgentEnabled = (agent: AgentType) => {
    if (agent === "codex") return codexEnabled
    if (agent === "amp") return ampEnabled
    if (agent === "claude") return claudeEnabled
    if (agent === "droid") return droidEnabled
    return true
  }

  const toggleAgentEnabled = (agent: AgentType) => {
    if (agent === "codex") setCodexEnabled(!codexEnabled)
    else if (agent === "amp") setAmpEnabled(!ampEnabled)
    else if (agent === "claude") setClaudeEnabled(!claudeEnabled)
    else if (agent === "droid") setDroidEnabled(!droidEnabled)
  }

  return (
    <div className="rounded-xl border border-border bg-card overflow-hidden shadow-sm h-[400px] flex">
      <div className="w-52 flex-shrink-0 border-r border-border bg-sidebar flex flex-col">
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
            const isDefault = agent === effectiveRunner
            return (
              <div
                key={agent}
                onClick={() => setSelectedAgent(agent)}
                className={cn(
                  "group w-full flex items-center justify-between px-3 py-2 text-left transition-colors text-sm cursor-pointer",
                  isSelected ? "bg-primary/10 text-primary" : "text-muted-foreground hover:text-foreground hover:bg-accent",
                  !isEnabled && "opacity-40 text-muted-foreground/60",
                )}
              >
                <div className="flex items-center gap-1.5 min-w-0">
                  <Icon className={cn("w-4 h-4 flex-shrink-0", agentConfig.iconClassName)} />
                  <span className="truncate">{agentConfig.name}</span>
                  {isDefault && isEnabled && (
                    <span className="text-[9px] leading-[14px] px-1 rounded bg-primary/15 text-primary flex-shrink-0">
                      Default
                    </span>
                  )}
                  {!isDefault && isEnabled && (
                    <button
                      onClick={(e) => {
                        e.stopPropagation()
                        setAgentRunner(agent as AgentRunnerKind)
                      }}
                      className="text-[9px] leading-[14px] px-1 rounded bg-muted text-muted-foreground hover:bg-primary/15 hover:text-primary flex-shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
                    >
                      Set Default
                    </button>
                  )}
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    toggleAgentEnabled(agent)
                  }}
                  className={cn("relative w-7 h-4 rounded-full transition-colors flex-shrink-0 ml-1", isEnabled ? "bg-primary" : "bg-muted-foreground/30")}
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

      <section id="telegram" className="scroll-mt-8">
        <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
          <MessageSquare className="w-4 h-4 text-muted-foreground" />
          Telegram
        </h3>
        <TelegramIntegrationPanel />
      </section>
    </div>
  )
}

function TelegramIntegrationPanel() {
  const {
    app,
    setTelegramBotToken,
    clearTelegramBotToken,
    startTelegramPairing,
    unpairTelegram,
  } = useLuban()

  const telegram = app?.integrations?.telegram ?? null
  const [token, setToken] = useState("")
  const [pairUrl, setPairUrl] = useState<string | null>(null)
  const [pairing, setPairing] = useState(false)

  const enabled = telegram?.enabled ?? false
  const hasToken = telegram?.has_token ?? false
  const pairedChatId = telegram?.paired_chat_id ?? null
  const botUsername = telegram?.bot_username ?? null
  const lastError = telegram?.last_error ?? null

  return (
    <div className="space-y-4">
      <div className="rounded border p-3" style={{ borderColor: "#ebebeb" }}>
        <div className="flex items-center justify-between">
          <div>
            <div className="text-[13px] font-medium" style={{ color: "#1b1b1b" }}>
              Status
            </div>
            <div className="text-[12px] mt-1" style={{ color: "#6b6b6b" }}>
              {enabled ? "Enabled" : "Disabled"} · {hasToken ? "Token set" : "No token"} ·{" "}
              {pairedChatId != null ? `Paired (${pairedChatId})` : "Not paired"}
              {botUsername ? ` · @${botUsername}` : ""}
            </div>
          </div>
          {lastError ? (
            <div className="flex items-center gap-1 text-[12px]" style={{ color: "#b45309" }}>
              <AlertTriangle className="w-4 h-4" />
              <span className="max-w-[360px] truncate">{lastError}</span>
            </div>
          ) : (
            <div className="flex items-center gap-1 text-[12px]" style={{ color: "#16a34a" }}>
              <CheckCircle2 className="w-4 h-4" />
              <span>OK</span>
            </div>
          )}
        </div>
      </div>

      <div className="rounded border p-3 space-y-3" style={{ borderColor: "#ebebeb" }}>
        <div>
          <div className="text-[13px] font-medium" style={{ color: "#1b1b1b" }}>
            Bot Token
          </div>
          <div className="text-[12px] mt-1" style={{ color: "#6b6b6b" }}>
            Stored locally on this machine.
          </div>
        </div>

        <div className="flex items-center gap-2">
          <input
            type="password"
            data-testid="telegram-bot-token-input"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="123456:ABCDEF..."
            className="flex-1 px-3 py-2 rounded text-[13px] border outline-none"
            style={{ borderColor: "#ebebeb", backgroundColor: "#ffffff", color: "#1b1b1b" }}
          />
          <button
            data-testid="telegram-bot-token-save"
            onClick={() => {
              setTelegramBotToken(token)
              setToken("")
              setPairUrl(null)
            }}
            disabled={!token.trim()}
            className="px-3 py-2 rounded text-[13px] transition-colors disabled:opacity-50"
            style={{ backgroundColor: "#5e6ad2", color: "#ffffff" }}
          >
            Save
          </button>
          <button
            data-testid="telegram-bot-token-clear"
            onClick={() => {
              clearTelegramBotToken()
              setToken("")
              setPairUrl(null)
            }}
            className="px-3 py-2 rounded text-[13px] transition-colors"
            style={{ backgroundColor: "#eeeeee", color: "#1b1b1b" }}
          >
            Clear
          </button>
        </div>
      </div>

      <div className="rounded border p-3 space-y-3" style={{ borderColor: "#ebebeb" }}>
        <div className="flex items-center justify-between">
          <div>
            <div className="text-[13px] font-medium" style={{ color: "#1b1b1b" }}>
              Pairing
            </div>
            <div className="text-[12px] mt-1" style={{ color: "#6b6b6b" }}>
              Generate a deep link and open it on the target Telegram device.
            </div>
          </div>
          <button
            data-testid="telegram-pair-generate"
            onClick={async () => {
              setPairing(true)
              setPairUrl(null)
              try {
                const url = await startTelegramPairing()
                setPairUrl(url)
              } finally {
                setPairing(false)
              }
            }}
            disabled={!hasToken || pairing}
            className="px-3 py-2 rounded text-[13px] transition-colors disabled:opacity-50 flex items-center gap-2"
            style={{ backgroundColor: "#eeeeee", color: "#1b1b1b" }}
          >
            {pairing ? <Loader2 className="w-4 h-4 animate-spin" /> : null}
            Generate Link
          </button>
        </div>

        {pairUrl ? (
          <div className="flex items-center gap-2">
            <input
              readOnly
              data-testid="telegram-pair-url"
              value={pairUrl}
              className="flex-1 px-3 py-2 rounded text-[13px] border"
              style={{ borderColor: "#ebebeb", backgroundColor: "#ffffff", color: "#1b1b1b" }}
            />
            <button
              onClick={async () => {
                try {
                  await navigator.clipboard.writeText(pairUrl)
                } catch {}
              }}
              className="px-3 py-2 rounded text-[13px] transition-colors"
              style={{ backgroundColor: "#eeeeee", color: "#1b1b1b" }}
            >
              Copy
            </button>
          </div>
        ) : null}

        <div className="flex items-center justify-between">
          <div className="text-[12px]" style={{ color: "#6b6b6b" }}>
            {pairedChatId != null ? `Paired chat_id: ${pairedChatId}` : "Not paired"}
          </div>
          <button
            data-testid="telegram-unpair"
            onClick={() => {
              unpairTelegram()
              setPairUrl(null)
            }}
            disabled={pairedChatId == null}
            className="px-3 py-2 rounded text-[13px] transition-colors disabled:opacity-50"
            style={{ backgroundColor: "#eeeeee", color: "#1b1b1b" }}
          >
            Unpair
          </button>
        </div>
      </div>
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
    if (initialSectionId === "telegram") next.add("integrations")
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
    if (initialSectionId === "telegram") {
      setExpandedItems((prev) => new Set(prev).add("integrations"))
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
      className="fixed inset-0 z-50 flex"
      style={{ backgroundColor: '#f5f5f5' }}
      onKeyDown={(e) => {
        if (e.key !== "Escape") return
        e.stopPropagation()
        ;(e.nativeEvent as unknown as { stopImmediatePropagation?: () => void }).stopImmediatePropagation?.()
        e.preventDefault()
        onOpenChange(false)
      }}
    >
      {/* Left Sidebar */}
      <div className="flex flex-col flex-shrink-0" style={{ width: '244px' }}>
        {/* Back button */}
        <div className="h-[52px] px-4 flex items-center">
          <button
            onClick={() => onOpenChange(false)}
            className="flex items-center gap-2 px-2 py-1.5 rounded transition-colors"
            style={{ color: '#6b6b6b' }}
            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#e8e8e8'}
            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
          >
            <ArrowLeft className="w-4 h-4" />
            <span className="text-[13px]">Back</span>
          </button>
        </div>

        {/* Navigation */}
        <div className="flex-1 overflow-y-auto px-3">
          {tocItems.map((item) => {
            const Icon = item.icon
            const isExpanded = expandedItems.has(item.id)
            const hasChildren = !!item.children?.length

            return (
              <div key={item.id} className="mb-1">
                <button
                  onClick={() => (hasChildren ? toggleExpanded(item.id) : scrollToSection(item.id))}
                  className={cn(
                    "w-full flex items-center gap-2 px-2 py-1.5 rounded text-[13px] transition-colors text-left",
                    !hasChildren && activeItem === item.id ? "bg-[#e8e8e8]" : "hover:bg-[#eeeeee]"
                  )}
                >
                  {hasChildren ? (
                    isExpanded ? (
                      <ChevronDown className="w-3.5 h-3.5" style={{ color: '#9b9b9b' }} />
                    ) : (
                      <ChevronRight className="w-3.5 h-3.5" style={{ color: '#9b9b9b' }} />
                    )
                  ) : (
                    <Icon className="w-4 h-4" style={{ color: '#6b6b6b' }} />
                  )}
                  <span style={{ color: '#1b1b1b' }}>{item.label}</span>
                </button>

                {isExpanded && hasChildren && (
                  <div className="ml-3 mt-0.5 space-y-0.5">
                    {item.children!.map((child) => {
                      const isActive = activeItem === child.id
                      const ChildIcon = child.icon
                      return (
                        <button
                          key={child.id}
                          onClick={() => scrollToSection(child.id)}
                          className={cn(
                            "w-full flex items-center gap-2 px-2 py-1.5 rounded text-[13px] transition-colors text-left",
                            isActive ? "bg-[#e8e8e8]" : "hover:bg-[#eeeeee]"
                          )}
                        >
                          <ChildIcon className="w-4 h-4" style={{ color: '#6b6b6b' }} />
                          <span style={{ color: '#1b1b1b' }}>{child.label}</span>
                        </button>
                      )
                    })}
                  </div>
                )}
              </div>
            )
          })}
        </div>

        {/* Version */}
        <div className="px-5 py-3">
          <span className="text-[12px]" style={{ color: '#9b9b9b' }}>Luban v0.1.6</span>
        </div>
      </div>

      {/* Right Content Panel */}
      <div
        className="flex-1 min-w-0 overflow-hidden flex flex-col"
        style={{
          margin: '8px 8px 8px 0',
          backgroundColor: '#fcfcfc',
          borderRadius: '4px',
          boxShadow: 'rgba(0, 0, 0, 0.022) 0px 3px 6px -2px, rgba(0, 0, 0, 0.044) 0px 1px 1px 0px'
        }}
      >
        {/* Content Header */}
        <div
          className="h-[52px] px-8 flex items-center flex-shrink-0"
          style={{ borderBottom: '1px solid #ebebeb' }}
        >
          <h1 className="text-[14px] font-medium" style={{ color: '#1b1b1b' }}>Settings</h1>
        </div>

        {/* Content Body */}
        <div ref={contentRef} className="flex-1 overflow-y-auto p-8">
          <div className="max-w-4xl">
            <AllSettings initialAgentId={initialAgentId} initialAgentFilePath={initialAgentFilePath} />
          </div>
        </div>
      </div>
    </div>
  )
}
