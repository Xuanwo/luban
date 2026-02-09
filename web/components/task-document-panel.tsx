"use client"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Terminal } from "lucide-react"

import { ChatComposer } from "@/components/chat-composer"
import type { ComposerAttachment } from "@/components/shared/message-editor"
import { attachmentHref } from "@/lib/attachment-href"
import { buildMessages } from "@/lib/conversation-ui"
import { openSettingsPanel } from "@/lib/open-settings"
import type {
  AttachmentRef,
  CodexCustomPromptSnapshot,
  TaskDocumentKind,
  TaskDocumentSnapshot,
} from "@/lib/luban-api"
import { fetchCodexCustomPrompts, fetchTaskDocuments, updateTaskDocument, uploadAttachment } from "@/lib/luban-http"
import { useLuban } from "@/lib/luban-context"

type DocumentState = {
  snapshot: TaskDocumentSnapshot
  draft: string
  isSaving: boolean
  error: string | null
}

type DocumentSelection = {
  start: number
  end: number
  startLine: number
  endLine: number
  text: string
}

type SelectionState = {
  kind: TaskDocumentKind
  selection: DocumentSelection
  toolbarTop: number
  toolbarLeft: number
}

type InlineCommentState = {
  kind: TaskDocumentKind
  selection: DocumentSelection
  top: number
  left: number
  draft: string
  isSending: boolean
}

const ORDER: TaskDocumentKind[] = ["task", "plan", "memory"]

const TITLES: Record<TaskDocumentKind, string> = {
  task: "TASK.md",
  plan: "PLAN.md",
  memory: "MEMORY.md",
}

function sortByOrder(documents: TaskDocumentSnapshot[]): TaskDocumentSnapshot[] {
  const rank = new Map<TaskDocumentKind, number>(ORDER.map((kind, index) => [kind, index]))
  return [...documents].sort((a, b) => (rank.get(a.kind) ?? 99) - (rank.get(b.kind) ?? 99))
}

function normalizeDocumentSnapshot(
  kind: TaskDocumentKind,
  snapshot: TaskDocumentSnapshot | undefined,
  taskId: number,
): TaskDocumentSnapshot {
  if (snapshot) return snapshot
  return {
    kind,
    rel_path: `.luban/tasks/${taskId}/${TITLES[kind]}`,
    content: "",
    content_hash: "",
    byte_len: 0,
    updated_at_unix_ms: Date.now(),
  }
}

function hasLocalUnsavedEdit(doc: DocumentState): boolean {
  return doc.draft !== doc.snapshot.content
}

function documentEditPrompt(path: string): string {
  return [
    "I updated a task document.",
    "",
    `Path: ${path}`,
    "Please review the change, update plan/memory as needed, and continue this task.",
  ].join("\n")
}

function toLineNumber(content: string, index: number): number {
  return content.slice(0, index).split("\n").length
}

function computeSelection(content: string, start: number, end: number): DocumentSelection | null {
  if (end <= start) return null
  const selected = content.slice(start, end)
  if (selected.trim().length === 0) return null
  const maxLen = 1200
  return {
    start,
    end,
    startLine: toLineNumber(content, start),
    endLine: toLineNumber(content, end),
    text: selected.length > maxLen ? `${selected.slice(0, maxLen)}\n...[truncated]` : selected,
  }
}

function autosizeTextarea(textarea: HTMLTextAreaElement | null) {
  if (!textarea) return
  textarea.style.height = "0px"
  textarea.style.height = `${Math.max(180, textarea.scrollHeight)}px`
}

function sectionReviewPrompt(args: {
  path: string
  title: string
  selection: DocumentSelection
  comment: string
}): string {
  return [
    "Document review feedback for this task.",
    "",
    `Section: ${args.title}`,
    `Path: ${args.path}`,
    `Selected lines: ${args.selection.startLine}-${args.selection.endLine}`,
    "",
    "Selected text:",
    "```",
    args.selection.text,
    "```",
    "",
    "Review comment:",
    args.comment,
    "",
    "Please respond to the feedback and update TASK/PLAN/MEMORY if needed.",
  ].join("\n")
}

function overallReviewPrompt(comment: string): string {
  return [
    "Overall review feedback for this task documents set (TASK/PLAN/MEMORY).",
    "",
    "Review comment:",
    comment,
    "",
    "Please respond to the feedback and update TASK/PLAN/MEMORY if needed.",
  ].join("\n")
}

export function TaskDocumentPanel() {
  const {
    app,
    conversation,
    activeWorkdirId,
    activeTaskId,
    sendAgentMessageTo,
    subscribeServerEvents,
    setChatModel,
    setThinkingEffort,
    setChatRunner,
    setChatAmpMode,
  } = useLuban()
  const [loading, setLoading] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [documents, setDocuments] = useState<Record<TaskDocumentKind, DocumentState> | null>(null)
  const [activeSelection, setActiveSelection] = useState<SelectionState | null>(null)
  const [inlineComment, setInlineComment] = useState<InlineCommentState | null>(null)
  const [commentDraft, setCommentDraft] = useState("")
  const [attachments, setAttachments] = useState<ComposerAttachment[]>([])
  const [codexCustomPrompts, setCodexCustomPrompts] = useState<CodexCustomPromptSnapshot[]>([])
  const editorRefs = useRef<Record<TaskDocumentKind, HTMLTextAreaElement | null>>({
    task: null,
    plan: null,
    memory: null,
  })
  const surfaceRef = useRef<HTMLDivElement | null>(null)
  const inlineCommentInputRef = useRef<HTMLTextAreaElement | null>(null)
  const attachmentScopeRef = useRef<string>("")
  const loadScopeRef = useRef<string>("")
  const refreshInFlightRef = useRef(false)
  const pendingRefreshRef = useRef(false)

  const scope = `${activeWorkdirId ?? "none"}:${activeTaskId ?? "none"}`
  const attachmentScope = `${activeWorkdirId ?? "none"}:${activeTaskId ?? "none"}`

  const reloadDocuments = useCallback(
    async (preferLocalDirty: boolean) => {
      if (activeWorkdirId == null || activeTaskId == null) {
        setDocuments(null)
        setLoadError(null)
        setLoading(false)
        return
      }

      if (refreshInFlightRef.current) {
        pendingRefreshRef.current = true
        return
      }
      refreshInFlightRef.current = true
      setLoading(true)
      if (!preferLocalDirty) setLoadError(null)
      try {
        const snapshot = await fetchTaskDocuments(activeWorkdirId, activeTaskId)
        const fetched = new Map<TaskDocumentKind, TaskDocumentSnapshot>(
          sortByOrder(snapshot.documents).map((doc) => [doc.kind, doc]),
        )
        setDocuments((prev) => {
          const next: Partial<Record<TaskDocumentKind, DocumentState>> = {}
          for (const kind of ORDER) {
            const latest = normalizeDocumentSnapshot(kind, fetched.get(kind), activeTaskId)
            const current = prev?.[kind]
            if (preferLocalDirty && current && hasLocalUnsavedEdit(current)) {
              next[kind] = current
              continue
            }
            next[kind] = {
              snapshot: latest,
              draft: latest.content,
              isSaving: false,
              error: null,
            }
          }
          return next as Record<TaskDocumentKind, DocumentState>
        })
        setLoadError(null)
      } catch (err) {
        setLoadError(err instanceof Error ? err.message : String(err))
        if (!preferLocalDirty) {
          setDocuments(null)
        }
      } finally {
        refreshInFlightRef.current = false
        setLoading(false)
        if (pendingRefreshRef.current) {
          pendingRefreshRef.current = false
          void reloadDocuments(preferLocalDirty)
        }
      }
    },
    [activeTaskId, activeWorkdirId],
  )

  useEffect(() => {
    if (scope === loadScopeRef.current) return
    loadScopeRef.current = scope
    setActiveSelection(null)
    setInlineComment(null)
    setCommentDraft("")
    void reloadDocuments(false)
  }, [reloadDocuments, scope])

  useEffect(() => {
    if (attachmentScope === attachmentScopeRef.current) return
    attachmentScopeRef.current = attachmentScope
    setAttachments([])
  }, [attachmentScope])

  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const prompts = await fetchCodexCustomPrompts()
        if (cancelled) return
        setCodexCustomPrompts(prompts)
      } catch (err) {
        console.warn("fetchCodexCustomPrompts failed", err)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [app?.rev])

  useEffect(() => {
    for (const kind of ORDER) {
      autosizeTextarea(editorRefs.current[kind])
    }
  }, [documents])

  useEffect(() => {
    if (activeWorkdirId == null || activeTaskId == null) return undefined
    return subscribeServerEvents((event) => {
      if (event.type !== "task_document_changed") return
      if (event.workdir_id !== activeWorkdirId || event.task_id !== activeTaskId) return
      void reloadDocuments(true)
    })
  }, [activeTaskId, activeWorkdirId, reloadDocuments, subscribeServerEvents])

  const updateDraft = useCallback((kind: TaskDocumentKind, draft: string) => {
    setDocuments((prev) => {
      if (!prev) return prev
      const current = prev[kind]
      if (!current) return prev
      return {
        ...prev,
        [kind]: {
          ...current,
          draft,
          error: null,
        },
      }
    })
  }, [])

  const updateSelection = useCallback(
    (kind: TaskDocumentKind, textarea: HTMLTextAreaElement) => {
      setInlineComment(null)
      const nextSelection = computeSelection(
        textarea.value,
        textarea.selectionStart ?? 0,
        textarea.selectionEnd ?? 0,
      )
      if (!nextSelection) {
        setActiveSelection((prev) => (prev?.kind === kind ? null : prev))
        return
      }

      const surface = surfaceRef.current
      if (!surface) {
        setActiveSelection({
          kind,
          selection: nextSelection,
          toolbarTop: 8,
          toolbarLeft: 8,
        })
        return
      }

      const surfaceRect = surface.getBoundingClientRect()
      const textareaRect = textarea.getBoundingClientRect()
      const lineHeight = Number.parseFloat(window.getComputedStyle(textarea).lineHeight) || 20
      const lineIndex = Math.max(0, toLineNumber(textarea.value, nextSelection.start) - 1)
      const lineOffset = Math.max(0, lineIndex * lineHeight - textarea.scrollTop)
      const rawTop = textareaRect.top - surfaceRect.top + lineOffset - 40
      const rawLeft = textareaRect.left - surfaceRect.left + 12
      const top = Math.max(8, rawTop)
      const left = Math.max(8, Math.min(rawLeft, Math.max(8, surface.clientWidth - 160)))

      setActiveSelection({
        kind,
        selection: nextSelection,
        toolbarTop: top,
        toolbarLeft: left,
      })
    },
    [],
  )

  const saveDirtyDocuments = useCallback(
    async (scopeKinds?: TaskDocumentKind[]) => {
      if (activeWorkdirId == null || activeTaskId == null || !documents) return

      const kindScope = scopeKinds ? new Set(scopeKinds) : null
      const pending = ORDER.map((docKind) => {
        if (kindScope && !kindScope.has(docKind)) return null
        const doc = documents[docKind]
        if (!doc) return null
        if (doc.isSaving || !hasLocalUnsavedEdit(doc)) return null
        return { kind: docKind, draft: doc.draft }
      }).filter((v): v is { kind: TaskDocumentKind; draft: string } => v != null)

      if (pending.length === 0) return

      setDocuments((prev) => {
        if (!prev) return prev
        const next = { ...prev }
        for (const item of pending) {
          const current = next[item.kind]
          if (!current) continue
          next[item.kind] = {
            ...current,
            isSaving: true,
            error: null,
          }
        }
        return next
      })

      for (const item of pending) {
        try {
          const updated = await updateTaskDocument({
            workspaceId: activeWorkdirId,
            threadId: activeTaskId,
            kind: item.kind,
            content: item.draft,
          })
          sendAgentMessageTo(activeWorkdirId, activeTaskId, documentEditPrompt(updated.rel_path))

          setDocuments((prev) => {
            if (!prev) return prev
            const current = prev[item.kind]
            if (!current) return prev
            return {
              ...prev,
              [item.kind]: {
                snapshot: updated,
                draft: current.draft === item.draft ? updated.content : current.draft,
                isSaving: false,
                error: null,
              },
            }
          })
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err)
          setDocuments((prev) => {
            if (!prev) return prev
            const current = prev[item.kind]
            if (!current) return prev
            return {
              ...prev,
              [item.kind]: {
                ...current,
                isSaving: false,
                error: message,
              },
            }
          })
        }
      }
    },
    [activeTaskId, activeWorkdirId, documents, sendAgentMessageTo],
  )

  const bindSelectionToComment = useCallback(() => {
    if (!activeSelection) return
    setInlineComment({
      kind: activeSelection.kind,
      selection: activeSelection.selection,
      top: activeSelection.toolbarTop + 34,
      left: activeSelection.toolbarLeft,
      draft: "",
      isSending: false,
    })
    setActiveSelection(null)
    requestAnimationFrame(() => inlineCommentInputRef.current?.focus())
  }, [activeSelection])

  const sendInlineComment = useCallback(() => {
    if (activeWorkdirId == null || activeTaskId == null || !inlineComment || !documents) return
    const comment = inlineComment.draft.trim()
    if (comment.length === 0) return

    const doc = documents[inlineComment.kind]
    if (!doc) return

    setInlineComment((prev) => (prev ? { ...prev, isSending: true } : prev))
    try {
      sendAgentMessageTo(
        activeWorkdirId,
        activeTaskId,
        sectionReviewPrompt({
          path: doc.snapshot.rel_path,
          title: TITLES[inlineComment.kind],
          selection: inlineComment.selection,
          comment,
        }),
      )
      setInlineComment(null)
    } finally {
      setInlineComment((prev) => (prev ? { ...prev, isSending: false } : prev))
    }
  }, [activeTaskId, activeWorkdirId, documents, inlineComment, sendAgentMessageTo])

  const sendUnifiedComment = useCallback(() => {
    if (activeWorkdirId == null || activeTaskId == null) return
    const comment = commentDraft.trim()
    const readyAttachments = attachments.filter((a) => a.status === "ready" && a.attachment)
    const refs = readyAttachments.map((a) => a.attachment as AttachmentRef)
    if (comment.length === 0 && refs.length === 0) return

    const prompt =
      comment.length > 0
        ? overallReviewPrompt(comment)
        : overallReviewPrompt("Please review attached files for TASK/PLAN/MEMORY.")
    sendAgentMessageTo(activeWorkdirId, activeTaskId, prompt, refs.length > 0 ? refs : undefined)
    setCommentDraft("")
    setAttachments([])
  }, [activeTaskId, activeWorkdirId, attachments, commentDraft, sendAgentMessageTo])

  const ordered = useMemo(() => {
    if (!documents) return []
    return ORDER.map((kind) => documents[kind]).filter((v): v is DocumentState => !!v)
  }, [documents])
  const messageHistory = useMemo(() => {
    const messages = buildMessages(conversation, { agentTurns: "grouped" })
    return messages.filter((message) => message.type === "user").map((message) => message.content)
  }, [conversation])

  const hasUnsavedChanges = useMemo(
    () => ordered.some((doc) => hasLocalUnsavedEdit(doc)),
    [ordered],
  )
  const isSavingAny = useMemo(
    () => ordered.some((doc) => doc.isSaving),
    [ordered],
  )
  const hasErrors = useMemo(() => ordered.some((doc) => doc.error != null), [ordered])
  const commentCanSend = useMemo(() => {
    const hasUploading = attachments.some((a) => a.status === "uploading")
    if (hasUploading) return false
    const hasReady = attachments.some((a) => a.status === "ready" && a.attachment != null)
    return commentDraft.trim().length > 0 || hasReady
  }, [attachments, commentDraft])

  const removeAttachment = useCallback((id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id))
  }, [])

  const handleFileSelect = useCallback(
    (files: FileList | null) => {
      if (!files || files.length === 0) return
      if (activeWorkdirId == null) return

      const scopeAtStart = attachmentScopeRef.current
      const workspaceId = activeWorkdirId

      Array.from(files).forEach((file) => {
        const tempId = `${Date.now()}-${Math.random().toString(36).slice(2)}`
        const isImage = file.type.startsWith("image/")
        const previewUrl = isImage ? URL.createObjectURL(file) : undefined

        const item: ComposerAttachment = {
          id: tempId,
          type: isImage ? "image" : "file",
          name: file.name,
          size: file.size,
          previewUrl,
          status: "uploading",
        }
        setAttachments((prev) => [...prev, item])

        void (async () => {
          try {
            const kind = file.type.startsWith("image/") ? "image" : "file"
            const uploaded = await uploadAttachment({ workspaceId, file, kind })
            if (attachmentScopeRef.current !== scopeAtStart) return
            setAttachments((prev) =>
              prev.map((a) => (a.id === tempId ? { ...a, status: "ready", attachment: uploaded } : a)),
            )
          } catch (err) {
            console.error("upload failed", err)
            if (attachmentScopeRef.current !== scopeAtStart) return
            setAttachments((prev) => prev.filter((a) => a.id !== tempId))
          }
        })()
      })
    },
    [activeWorkdirId],
  )

  const handlePaste = useCallback(
    (e: React.ClipboardEvent) => {
      const items = e.clipboardData?.items
      if (!items) return

      const files: File[] = []
      for (let i = 0; i < items.length; i++) {
        const item = items[i]
        if (item.kind === "file") {
          const file = item.getAsFile()
          if (file) files.push(file)
        }
      }
      if (files.length > 0) {
        e.preventDefault()
        const dt = new DataTransfer()
        files.forEach((f) => dt.items.add(f))
        handleFileSelect(dt.files)
      }
    },
    [handleFileSelect],
  )

  const handleCommand = useCallback((commandId: string) => {
    const cmd = codexCustomPrompts.find((c) => c.id === commandId)
    if (!cmd) return
    setCommentDraft(cmd.contents)
  }, [codexCustomPrompts])

  if (activeWorkdirId == null || activeTaskId == null) {
    return (
      <div className="h-full flex items-center justify-center text-sm text-muted-foreground">
        Select a task to view documents.
      </div>
    )
  }

  return (
    <div className="h-full min-h-0 flex flex-col border-r border-border bg-background" data-testid="task-document-panel">
      <div className="flex-1 min-h-0 overflow-auto" onScroll={() => {
        setActiveSelection(null)
        setInlineComment(null)
      }}>
        <div className="relative max-w-4xl mx-auto px-5 py-4">
          {(hasUnsavedChanges || isSavingAny || hasErrors) && (
            <div className="pointer-events-none absolute right-5 top-4 z-30" data-testid="task-document-save-area">
              <button
                type="button"
                onClick={() => void saveDirtyDocuments()}
                disabled={!hasUnsavedChanges || isSavingAny}
                data-testid="task-document-save-check"
                className="pointer-events-auto h-8 px-3 rounded-full border border-border bg-background/95 text-[12px] shadow-sm backdrop-blur hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed"
                title="Save document edits"
              >
                {isSavingAny ? "Saving..." : hasErrors ? "Save (Retry)" : "Save"}
              </button>
            </div>
          )}
          {loading && <div className="text-xs text-muted-foreground">Loading documents...</div>}
          {!loading && loadError && <div className="text-xs text-destructive">{loadError}</div>}

          {!loading && !loadError && (
            <div ref={surfaceRef} className="relative" data-testid="task-document-surface">
              {activeSelection && (
                <div
                  className="absolute z-20 rounded-xl bg-zinc-900 text-zinc-100 shadow-2xl px-2 py-1 flex items-center gap-2"
                  style={{ top: activeSelection.toolbarTop, left: activeSelection.toolbarLeft }}
                  data-testid="task-document-selection-toolbar"
                >
                  <button
                    type="button"
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={bindSelectionToComment}
                    data-testid="task-document-selection-toolbar-comment"
                    className="text-[12px] px-2 py-1 rounded-md bg-zinc-800 hover:bg-zinc-700"
                  >
                    Comment
                  </button>
                  <span className="text-[10px] text-zinc-300">
                    {TITLES[activeSelection.kind]}:{activeSelection.selection.startLine}-{activeSelection.selection.endLine}
                  </span>
                </div>
              )}
              {inlineComment && (
                <div
                  className="absolute z-30 w-[340px] rounded-2xl border border-zinc-300 bg-background p-3 shadow-[0_16px_32px_rgba(0,0,0,0.18)]"
                  style={{ top: inlineComment.top, left: inlineComment.left }}
                  data-testid="task-document-inline-comment"
                >
                  <div className="mb-2 text-[11px] text-zinc-600">
                    {TITLES[inlineComment.kind]} lines {inlineComment.selection.startLine}-{inlineComment.selection.endLine}
                  </div>
                  <textarea
                    ref={inlineCommentInputRef}
                    value={inlineComment.draft}
                    onChange={(e) =>
                      setInlineComment((prev) => (prev ? { ...prev, draft: e.target.value } : prev))
                    }
                    data-testid="task-document-inline-comment-input"
                    placeholder="Write inline review comment..."
                    className="w-full min-h-[92px] resize-y rounded-xl border border-border bg-background px-3 py-2 text-[14px] leading-6 focus:outline-none focus:ring-2 focus:ring-zinc-300"
                  />
                  <div className="mt-2 flex items-center justify-end gap-2">
                    <button
                      type="button"
                      onClick={() => setInlineComment(null)}
                      data-testid="task-document-inline-comment-cancel"
                      className="h-7 px-2 rounded-md border border-border bg-background text-[12px] hover:bg-muted"
                    >
                      Cancel
                    </button>
                    <button
                      type="button"
                      onClick={sendInlineComment}
                      disabled={inlineComment.draft.trim().length === 0 || inlineComment.isSending}
                      data-testid="task-document-inline-comment-submit"
                      className="text-[12px] w-7 h-7 rounded-full border border-border bg-background hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed"
                      title="Submit inline review comment"
                    >
                      {inlineComment.isSending ? "…" : "✓"}
                    </button>
                  </div>
                </div>
              )}

              <div
                className="rounded-2xl border border-border bg-background shadow-[0_1px_0_rgba(0,0,0,0.02)]"
                data-testid="task-document-connected-sections"
              >
                {ordered.map((doc, index) => (
                  <section
                    key={doc.snapshot.kind}
                    className={index === 0 ? "" : "border-t border-border/70"}
                    data-testid={`task-document-section-${doc.snapshot.kind}`}
                  >
                    <div className="px-5 pt-4 pb-1 flex items-center gap-2">
                      <h3 className="text-[12px] font-semibold tracking-wide text-zinc-500">{TITLES[doc.snapshot.kind]}</h3>
                      <code className="text-[10px] text-zinc-400 truncate">{doc.snapshot.rel_path}</code>
                    </div>
                    <textarea
                      ref={(el) => {
                        editorRefs.current[doc.snapshot.kind] = el
                      }}
                      value={doc.draft}
                      onChange={(e) => updateDraft(doc.snapshot.kind, e.target.value)}
                      onInput={(e) => autosizeTextarea(e.currentTarget)}
                      onSelect={(e) => updateSelection(doc.snapshot.kind, e.currentTarget)}
                      onMouseUp={(e) => updateSelection(doc.snapshot.kind, e.currentTarget)}
                      onKeyUp={(e) => updateSelection(doc.snapshot.kind, e.currentTarget)}
                      data-testid={`task-document-editor-${doc.snapshot.kind}`}
                      className="w-full min-h-[220px] resize-none border-0 bg-background px-5 pb-4 text-[15px] font-normal leading-8 text-zinc-800 focus:outline-none focus:ring-0"
                      spellCheck={false}
                    />
                    {doc.error && <p className="px-5 pb-3 text-xs text-destructive">{doc.error}</p>}
                  </section>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
      <div className="shrink-0 border-t border-border bg-background" data-testid="task-document-fixed-comment">
        <div className="px-3 py-2">
          <ChatComposer
            value={commentDraft}
            onChange={setCommentDraft}
            attachments={attachments}
            onRemoveAttachment={removeAttachment}
            onFileSelect={handleFileSelect}
            onPaste={handlePaste}
            onAddAttachmentRef={(attachment) => {
              const isImage = attachment.kind === "image"
              const previewUrl =
                isImage && activeWorkdirId != null
                  ? attachmentHref({ workspaceId: activeWorkdirId, attachment }) ?? undefined
                  : undefined
              setAttachments((prev) => [
                ...prev,
                {
                  id: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
                  type: isImage ? "image" : "file",
                  name: attachment.name,
                  size: attachment.byte_len,
                  previewUrl,
                  status: "ready",
                  attachment,
                },
              ])
            }}
            workspaceId={activeWorkdirId}
            commands={codexCustomPrompts}
            messageHistory={messageHistory}
            onCommand={handleCommand}
            placeholder="Let's chart the cosmos of ideas..."
            attachmentsEnabled
            agentSelectorEnabled
            disabled={activeWorkdirId == null || activeTaskId == null}
            agentModelId={conversation?.agent_model_id}
            agentThinkingEffort={conversation?.thinking_effort}
            defaultModelId={app?.agent.default_model_id ?? null}
            defaultThinkingEffort={app?.agent.default_thinking_effort ?? null}
            defaultAmpMode={app?.agent.amp_mode ?? null}
            onOpenAgentSettings={(agentId, agentFilePath) => openSettingsPanel("agent", { agentId, agentFilePath })}
            onChangeModelId={(modelId) => {
              if (activeWorkdirId == null || activeTaskId == null) return
              setChatModel(activeWorkdirId, activeTaskId, modelId)
            }}
            onChangeThinkingEffort={(effort) => {
              if (activeWorkdirId == null || activeTaskId == null) return
              setThinkingEffort(activeWorkdirId, activeTaskId, effort)
            }}
            defaultRunner={app?.agent.default_runner ?? null}
            runner={conversation?.agent_runner ?? null}
            ampMode={conversation?.amp_mode ?? null}
            onChangeRunner={(runner) => {
              if (activeWorkdirId == null || activeTaskId == null) return
              setChatRunner(activeWorkdirId, activeTaskId, runner)
            }}
            onChangeAmpMode={(mode) => {
              if (activeWorkdirId == null || activeTaskId == null) return
              if (mode == null) return
              setChatAmpMode(activeWorkdirId, activeTaskId, mode)
            }}
            onSend={sendUnifiedComment}
            secondaryAction={{
              onClick: () => {},
              ariaLabel: "Switch to shell",
              icon: <Terminal className="w-3.5 h-3.5" />,
              testId: "chat-mode-toggle",
            }}
            canSend={commentCanSend}
            codexEnabled={app?.agent.codex_enabled ?? true}
            ampEnabled={app?.agent.amp_enabled ?? true}
            runnerDefaultModels={app?.agent.runner_default_models ?? null}
            compact
          />
        </div>
      </div>
    </div>
  )
}
