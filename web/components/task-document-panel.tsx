"use client"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { ChatComposer } from "@/components/chat-composer"
import { TiptapMarkdownEditor } from "@/components/tiptap-markdown-editor"
import { TaskStatusSelector } from "@/components/shared/task-status-selector"
import { OpenButton } from "@/components/shared/open-button"
import type { ComposerAttachment } from "@/components/shared/message-editor"
import { attachmentHref } from "@/lib/attachment-href"
import { buildMessages } from "@/lib/conversation-ui"
import { buildTaskDocumentChangePrompt } from "@/lib/task-document-diff"
import { openSettingsPanel } from "@/lib/open-settings"
import type {
  AttachmentRef,
  CodexCustomPromptSnapshot,
  TaskDocumentKind,
  TaskDocumentSnapshot,
} from "@/lib/luban-api"
import {
  fetchCodexCustomPrompts,
  fetchTaskDocument,
  fetchTaskDocuments,
  updateTaskDocument,
  uploadAttachment,
} from "@/lib/luban-http"
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
    rel_path: `tasks/v1/tasks/mock-${taskId}/${TITLES[kind]}`,
    content: "",
    content_hash: "",
    byte_len: 0,
    updated_at_unix_ms: Date.now(),
  }
}

function hasLocalUnsavedEdit(doc: DocumentState): boolean {
  return doc.draft !== doc.snapshot.content
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
    setTaskStatus,
  } = useLuban()
  const [loading, setLoading] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [documents, setDocuments] = useState<Record<TaskDocumentKind, DocumentState> | null>(null)
  const [activeSelection, setActiveSelection] = useState<SelectionState | null>(null)
  const [inlineComment, setInlineComment] = useState<InlineCommentState | null>(null)
  const [commentDraft, setCommentDraft] = useState("")
  const [attachments, setAttachments] = useState<ComposerAttachment[]>([])
  const [codexCustomPrompts, setCodexCustomPrompts] = useState<CodexCustomPromptSnapshot[]>([])
  const [actionZoneVisible, setActionZoneVisible] = useState(true)
  const surfaceRef = useRef<HTMLDivElement | null>(null)
  const inlineCommentInputRef = useRef<HTMLTextAreaElement | null>(null)
  const attachmentScopeRef = useRef<string>("")
  const loadScopeRef = useRef<string>("")
  const refreshInFlightRef = useRef(false)
  const pendingRefreshRef = useRef(false)
  const suppressNextWatcherRefreshRef = useRef<Record<TaskDocumentKind, number>>({
    task: 0,
    plan: 0,
    memory: 0,
  })

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

  const refreshDocumentByKind = useCallback(
    async (kind: TaskDocumentKind, preferLocalDirty: boolean) => {
      if (activeWorkdirId == null || activeTaskId == null) return
      try {
        const snapshot = await fetchTaskDocument({
          workspaceId: activeWorkdirId,
          taskId: activeTaskId,
          kind,
        })
        setDocuments((prev) => {
          if (!prev) return prev
          const current = prev[kind]
          if (!current) return prev
          if (preferLocalDirty && hasLocalUnsavedEdit(current)) return prev
          return {
            ...prev,
            [kind]: {
              snapshot,
              draft: snapshot.content,
              isSaving: false,
              error: null,
            },
          }
        })
      } catch {
        // Fall back to full reload when single-document fetch fails.
        void reloadDocuments(preferLocalDirty)
      }
    },
    [activeTaskId, activeWorkdirId, reloadDocuments],
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
    if (activeWorkdirId == null || activeTaskId == null) return undefined
    return subscribeServerEvents((event) => {
      if (event.type !== "task_document_changed") return
      if (event.workdir_id !== activeWorkdirId || event.task_id !== activeTaskId) return
      const suppressed = suppressNextWatcherRefreshRef.current[event.kind] ?? 0
      if (suppressed > 0) {
        suppressNextWatcherRefreshRef.current[event.kind] = suppressed - 1
        return
      }
      void refreshDocumentByKind(event.kind, true)
    })
  }, [activeTaskId, activeWorkdirId, refreshDocumentByKind, subscribeServerEvents])

  useEffect(() => {
    const el = surfaceRef.current
    if (!el) return
    const onScroll = () => {
      setActionZoneVisible(el.scrollTop <= 0)
    }
    el.addEventListener("scroll", onScroll, { passive: true })
    return () => el.removeEventListener("scroll", onScroll)
  }, [scope])

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

  const handleTiptapSelectionChange = useCallback(
    (kind: TaskDocumentKind, info: {
      text: string
      startLine: number
      endLine: number
      from: number
      to: number
      editorElement: HTMLElement
    } | null) => {
      if (!info) {
        setActiveSelection((prev) => (prev?.kind === kind ? null : prev))
        if (inlineComment?.kind === kind) setInlineComment(null)
        return
      }

      const selection: DocumentSelection = {
        start: info.from,
        end: info.to,
        startLine: info.startLine,
        endLine: info.endLine,
        text: info.text,
      }

      const surface = surfaceRef.current
      let top = 8
      let left = 8
      if (surface) {
        const surfaceRect = surface.getBoundingClientRect()
        const editorRect = info.editorElement.getBoundingClientRect()
        const sel = window.getSelection()
        if (sel && sel.rangeCount > 0) {
          const range = sel.getRangeAt(0)
          const rects = range.getClientRects()
          if (rects.length > 0) {
            const lastRect = rects[rects.length - 1]
            top = Math.max(8, lastRect.bottom - surfaceRect.top + 4)
            left = Math.max(8, Math.min(lastRect.left - surfaceRect.left, surface.clientWidth - 360))
          } else {
            top = Math.max(8, editorRect.top - surfaceRect.top + 24)
            left = Math.max(8, editorRect.left - surfaceRect.left + 40)
          }
        }
      }

      setActiveSelection({
        kind,
        selection,
        toolbarTop: top,
        toolbarLeft: left,
      })
    },
    [inlineComment?.kind],
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
        return {
          kind: docKind,
          draft: doc.draft,
          before: doc.snapshot.content,
        }
      }).filter((v): v is { kind: TaskDocumentKind; draft: string; before: string } => v != null)

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
            taskId: activeTaskId,
            kind: item.kind,
            content: item.draft,
          })

          suppressNextWatcherRefreshRef.current[item.kind] =
            (suppressNextWatcherRefreshRef.current[item.kind] ?? 0) + 1

          const prompt = buildTaskDocumentChangePrompt({
            kind: item.kind,
            path: updated.rel_path,
            before: item.before,
            after: item.draft,
          })
          if (prompt) {
            sendAgentMessageTo(activeWorkdirId, activeTaskId, prompt)
          }

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

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault()
        void saveDirtyDocuments()
      }
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [saveDirtyDocuments])

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
      <div className="h-full flex items-center justify-center" style={{ fontSize: '13px', color: '#6b6b6b' }}>
        Select a task to view documents.
      </div>
    )
  }

  return (
    <div className="h-full min-h-0 flex flex-col" style={{ backgroundColor: '#fcfcfc' }} data-testid="task-document-panel">
      {/* Collapsible action zone */}
      <div
        className="shrink-0 overflow-hidden"
        style={{
          maxHeight: actionZoneVisible ? '48px' : '0px',
          opacity: actionZoneVisible ? 1 : 0,
          transition: 'max-height 0.2s ease, opacity 0.15s ease',
          borderBottom: actionZoneVisible ? '1px solid #ebebeb' : '1px solid transparent',
        }}
        data-testid="task-document-action-zone"
      >
        <div
          className="flex items-center gap-3"
          style={{ padding: '10px 20px' }}
        >
          <TaskStatusSelector
            status={conversation?.task_status ?? "todo"}
            onStatusChange={(status) => {
              if (activeWorkdirId == null || activeTaskId == null) return
              setTaskStatus(activeWorkdirId, activeTaskId, status)
            }}
            variant="pill"
            triggerTestId="task-document-status-trigger"
          />
          <div className="flex-1" />
          <OpenButton />
        </div>
      </div>

      {/* Scrollable document surface */}
      <div className="flex-1 min-h-0 relative">
        <div
          ref={surfaceRef}
          className="h-full overflow-auto relative"
          data-testid="task-document-surface"
        >
        <div style={{ padding: '16px 28px 60px' }}>
          {loading && (
            <div style={{ fontSize: '13px', color: '#6b6b6b', padding: '40px 0' }}>Loading documents...</div>
          )}
          {!loading && loadError && (
            <div style={{ fontSize: '13px', color: '#eb5757', padding: '40px 0' }}>{loadError}</div>
          )}

          {!loading && !loadError && ordered.map((doc, index) => {
            const kind = doc.snapshot.kind
            const unsaved = hasLocalUnsavedEdit(doc)

            return (
              <section
                key={kind}
                data-testid={`task-document-section-${kind}`}
                style={index > 0 ? { marginTop: '24px' } : undefined}
              >
                {/* Section header */}
                <div
                  className="group/header flex items-center gap-1.5"
                  style={{
                    marginBottom: '8px',
                  }}
                >
                  <span style={{ fontSize: '11px', fontWeight: 500, letterSpacing: '0.3px', color: '#9b9b9b', textTransform: 'uppercase' }}>
                    {TITLES[kind]}
                  </span>
                  {unsaved && (
                    <span
                      data-testid={`task-document-unsaved-${kind}`}
                      style={{
                        display: 'inline-block',
                        width: '5px',
                        height: '5px',
                        borderRadius: '50%',
                        backgroundColor: '#5e6ad2',
                      }}
                    />
                  )}
                  {doc.isSaving && (
                    <span style={{ fontSize: '11px', color: '#6b6b6b' }}>Saving...</span>
                  )}
                </div>

                {/* TipTap WYSIWYG editor - seamless inline editing */}
                <div
                  data-testid={`task-document-editor-${kind}`}
                  style={{ minHeight: '24px' }}
                >
                  <TiptapMarkdownEditor
                    content={doc.draft}
                    onChange={(md) => updateDraft(kind, md)}
                    onSelectionChange={(info) => handleTiptapSelectionChange(kind, info)}
                    placeholder={`Start writing ${TITLES[kind]}...`}
                    data-testid={`task-document-rendered-${kind}`}
                  />
                </div>

                {doc.error && (
                  <div style={{ fontSize: '12px', color: '#eb5757', marginTop: '4px' }}>{doc.error}</div>
                )}
              </section>
            )
          })}
        </div>

        {/* Selection toolbar */}
        {activeSelection && !inlineComment && (
          <div
            className="absolute z-20 flex items-center gap-2"
            style={{
              top: activeSelection.toolbarTop,
              left: activeSelection.toolbarLeft,
              border: '1px solid #ebebeb',
              borderRadius: '4px',
              backgroundColor: '#fcfcfc',
              boxShadow: 'rgba(0,0,0,0.022) 0px 3px 6px -2px, rgba(0,0,0,0.044) 0px 1px 1px 0px',
              padding: '4px 8px',
            }}
            data-testid="task-document-selection-toolbar"
          >
            <button
              type="button"
              onMouseDown={(event) => event.preventDefault()}
              onClick={bindSelectionToComment}
              data-testid="task-document-selection-toolbar-comment"
              className="transition-colors"
              style={{ fontSize: '12px', fontWeight: 500, color: '#5e6ad2' }}
            >
              Comment
            </button>
            <span style={{ fontSize: '11px', color: '#9b9b9b' }}>
              L{activeSelection.selection.startLine}–{activeSelection.selection.endLine}
            </span>
          </div>
        )}

        {/* Inline comment popover */}
        {inlineComment && (
          <div
            className="absolute z-30"
            style={{
              top: inlineComment.top,
              left: inlineComment.left,
              width: '320px',
              border: '1px solid #ebebeb',
              borderRadius: '4px',
              backgroundColor: '#fcfcfc',
              boxShadow: 'rgba(0,0,0,0.022) 0px 3px 6px -2px, rgba(0,0,0,0.044) 0px 1px 1px 0px',
              padding: '10px 12px',
            }}
            data-testid="task-document-inline-comment"
          >
            <div className="flex items-center gap-2 mb-2">
              <span style={{ fontSize: '11px', color: '#6b6b6b' }}>
                {TITLES[inlineComment.kind]} L{inlineComment.selection.startLine}–{inlineComment.selection.endLine}
              </span>
            </div>
            <textarea
              ref={inlineCommentInputRef}
              value={inlineComment.draft}
              onChange={(e) =>
                setInlineComment((prev) => (prev ? { ...prev, draft: e.target.value } : prev))
              }
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                  e.preventDefault()
                  sendInlineComment()
                }
                if (e.key === "Escape") {
                  e.preventDefault()
                  setInlineComment(null)
                }
              }}
              data-testid="task-document-inline-comment-input"
              placeholder="Review comment... (⌘↵ to send)"
              className="w-full resize-none focus:outline-none"
              style={{
                minHeight: '60px',
                fontSize: '13px',
                lineHeight: '20px',
                color: '#1b1b1b',
                border: '1px solid #ebebeb',
                borderRadius: '4px',
                padding: '6px 8px',
                backgroundColor: '#fcfcfc',
              }}
            />
            <div className="mt-2 flex items-center justify-between">
              <button
                type="button"
                onClick={() => setInlineComment(null)}
                data-testid="task-document-inline-comment-cancel"
                className="transition-colors"
                style={{ fontSize: '11px', color: '#6b6b6b' }}
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={sendInlineComment}
                disabled={inlineComment.draft.trim().length === 0 || inlineComment.isSending}
                data-testid="task-document-inline-comment-submit"
                className="transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                style={{
                  fontSize: '11px',
                  fontWeight: 500,
                  color: '#ffffff',
                  backgroundColor: '#5e6ad2',
                  borderRadius: '4px',
                  padding: '3px 10px',
                }}
                title="Submit inline review comment"
              >
                {inlineComment.isSending ? "Sending..." : "Send"}
              </button>
            </div>
          </div>
        )}
      </div>
        <div
          className="pointer-events-none absolute bottom-0 left-0 right-0"
          style={{
            height: '40px',
            background: 'linear-gradient(to bottom, transparent, #fcfcfc)',
          }}
        />
      </div>

      {/* Bottom comment bar */}
      <div className="shrink-0" data-testid="task-document-fixed-comment">
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
            placeholder="Review comment or instruction to agent..."
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
