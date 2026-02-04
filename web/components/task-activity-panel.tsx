"use client"

import type React from "react"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { ArrowLeft, Clock, X } from "lucide-react"
import { useLuban } from "@/lib/luban-context"
import { cn } from "@/lib/utils"
import { buildMessages, type Message } from "@/lib/conversation-ui"
import { TaskActivityView } from "@/components/task-activity-view"
import { fetchCodexCustomPrompts, fetchWorkspaceDiff, uploadAttachment } from "@/lib/luban-http"
import type {
  AttachmentRef,
  CodexCustomPromptSnapshot,
} from "@/lib/luban-api"
import { attachmentHref } from "@/lib/attachment-href"
import {
  draftKey,
  followTailKey,
  loadJson,
  saveJson,
} from "@/lib/ui-prefs"
import type { ChangedFile } from "./right-sidebar"
import { type ComposerAttachment as EditorComposerAttachment } from "@/components/shared/message-editor"
import { openSettingsPanel } from "@/lib/open-settings"
import { focusChatInput } from "@/lib/focus-chat-input"
import { useAgentCancelHotkey } from "@/lib/use-agent-cancel-hotkey"
import { EscCancelHint } from "@/components/esc-cancel-hint"
import { ChatComposer } from "@/components/chat-composer"
import { DiffTabPanel, type DiffFileData, type DiffStyle } from "@/components/diff-tab-panel"

type ComposerAttachment = EditorComposerAttachment

type PersistedChatDraft = {
  text: string
  attachments?: AttachmentRef[]
}

export function TaskActivityPanel({
  pendingDiffFile,
  onDiffFileOpened,
}: {
  pendingDiffFile?: ChangedFile | null
  onDiffFileOpened?: () => void
}) {
  const [codexCustomPrompts, setCodexCustomPrompts] = useState<CodexCustomPromptSnapshot[]>([])

  const {
    app,
    activeWorkdirId: activeWorkspaceId,
    activeTaskId: activeThreadId,
    tasks: threads,
    conversation,
    sendAgentMessage,
    queueAgentMessage,
    cancelAgentTurn,
    removeQueuedPrompt,
    setChatModel,
    setThinkingEffort,
    setChatRunner,
    setChatAmpMode,
  } = useLuban()

  const [draftText, setDraftText] = useState("")
  const [followTail, setFollowTail] = useState(true)
  const [attachments, setAttachments] = useState<ComposerAttachment[]>([])
  const attachmentScopeRef = useRef<string>("")
  const attachmentScope = `${activeWorkspaceId ?? "none"}:${activeThreadId ?? "none"}`

  const attachmentsFromRefs = useCallback(
    (workspaceId: number | null, refs: AttachmentRef[]): ComposerAttachment[] => {
      return refs.map((attachment) => {
        const isImage = attachment.kind === "image"
        const previewUrl =
          isImage && workspaceId != null ? attachmentHref({ workspaceId, attachment }) ?? undefined : undefined
        return {
          id: `ref-${attachment.id}`,
          type: isImage ? "image" : "file",
          name: attachment.name,
          size: attachment.byte_len,
          status: "ready",
          attachment,
          previewUrl,
        }
      })
    },
    [],
  )

  const messages = useMemo(() => buildMessages(conversation, { agentTurns: "grouped" }), [conversation])
  const queuedPrompts = useMemo(() => conversation?.pending_prompts ?? [], [conversation?.pending_prompts])

  const messageHistory = useMemo(() => {
    return messages.filter((m) => m.type === "user").map((m) => m.content)
  }, [messages])

  const isAgentRunning = conversation?.run_status === "running"

  const [activePanel, setActivePanel] = useState<"activity" | "diff">("activity")
  const [diffStyle, setDiffStyle] = useState<DiffStyle>("split")
  const [diffFiles, setDiffFiles] = useState<DiffFileData[]>([])
  const [diffActiveFileId, setDiffActiveFileId] = useState<string | undefined>(undefined)
  const [isDiffLoading, setIsDiffLoading] = useState(false)
  const [diffError, setDiffError] = useState<string | null>(null)

  const ESC_TIMEOUT_MS = 3000
  const { escHintVisible } = useAgentCancelHotkey({
    enabled: isAgentRunning,
    blocked: false,
    onCancel: () => {
      if (activeWorkspaceId == null || activeThreadId == null) return
      cancelAgentTurn()
    },
    timeoutMs: ESC_TIMEOUT_MS,
  })

  useEffect(() => {
    if (attachmentScope === attachmentScopeRef.current) return
    attachmentScopeRef.current = attachmentScope

    if (activeWorkspaceId == null || activeThreadId == null) {
      setDraftText("")
      setAttachments([])
      return
    }

    const saved = loadJson<PersistedChatDraft>(draftKey(activeWorkspaceId, activeThreadId))
    setDraftText(saved?.text ?? "")
    setAttachments(attachmentsFromRefs(activeWorkspaceId, saved?.attachments ?? []))
    setFollowTail(localStorage.getItem(followTailKey(activeWorkspaceId, activeThreadId)) !== "false")
  }, [attachmentScope, activeWorkspaceId, activeThreadId, attachmentsFromRefs])

  useEffect(() => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    const readyAttachments = attachments.filter((a) => a.status === "ready" && a.attachment)
    const refs = readyAttachments.map((a) => a.attachment as AttachmentRef)
    saveJson(draftKey(activeWorkspaceId, activeThreadId), { text: draftText, attachments: refs })
  }, [draftText, attachments, activeWorkspaceId, activeThreadId])

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

  const removeAttachment = (id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id))
  }

  const handleFileSelect = (files: FileList | null) => {
    if (!files || files.length === 0) return
    if (activeWorkspaceId == null) return

    const scopeAtStart = attachmentScopeRef.current
    const workspaceId = activeWorkspaceId

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
            prev.map((a) =>
              a.id === tempId ? { ...a, status: "ready", attachment: uploaded } : a
            )
          )
        } catch (err) {
          console.error("upload failed", err)
          if (attachmentScopeRef.current !== scopeAtStart) return
          setAttachments((prev) => prev.filter((a) => a.id !== tempId))
        }
      })()
    })
  }

  const handlePaste = (e: React.ClipboardEvent) => {
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
  }

  const handleCommand = (commandId: string) => {
    const cmd = codexCustomPrompts.find((c) => c.id === commandId)
    if (!cmd) return
    setDraftText(cmd.contents)
    focusChatInput()
  }

  const handleSend = () => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    const text = draftText.trim()
    const readyAttachments = attachments.filter((a) => a.status === "ready" && a.attachment)
    const refs = readyAttachments.map((a) => a.attachment as AttachmentRef)
    if (text.length === 0 && refs.length === 0) return

    if (isAgentRunning) {
      queueAgentMessage(text, refs.length > 0 ? refs : undefined)
    } else {
      sendAgentMessage(text, refs.length > 0 ? refs : undefined)
    }
    setDraftText("")
    setAttachments([])
  }

  const openDiffTab = useCallback(
    async (targetFile: ChangedFile) => {
      if (activeWorkspaceId == null) return
      setActivePanel("diff")
      setDiffActiveFileId(targetFile.id)
      setIsDiffLoading(true)
      setDiffError(null)

      try {
        const snap = await fetchWorkspaceDiff(activeWorkspaceId)
        const files: DiffFileData[] = (snap.files ?? []).map((file) => ({
          file: file.file,
          oldFile: { name: file.old_file.name, contents: file.old_file.contents },
          newFile: { name: file.new_file.name, contents: file.new_file.contents },
        }))
        setDiffFiles(files)
      } catch (err) {
        setDiffError(err instanceof Error ? err.message : String(err))
        setDiffFiles([])
      } finally {
        setIsDiffLoading(false)
      }
    },
    [activeWorkspaceId],
  )

  useEffect(() => {
    if (!pendingDiffFile) return
    void (async () => {
      await openDiffTab(pendingDiffFile)
      onDiffFileOpened?.()
    })()
  }, [onDiffFileOpened, openDiffTab, pendingDiffFile])

  const taskTitle = useMemo(() => {
    if (activeThreadId == null) return "Untitled Task"
    const thread = threads.find((t) => t.task_id === activeThreadId)
    return thread?.title ?? "Untitled Task"
  }, [threads, activeThreadId])

  const canSend = useMemo(() => {
    if (activeWorkspaceId == null || activeThreadId == null) return false
    const hasUploading = attachments.some((a) => a.status === "uploading")
    if (hasUploading) return false
    const hasReady = attachments.some((a) => a.status === "ready" && a.attachment != null)
    return draftText.trim().length > 0 || hasReady
  }, [activeWorkspaceId, activeThreadId, attachments, draftText])

  const handleCancelQueuedPrompt = useCallback(
    (promptId: number) => {
      if (activeWorkspaceId == null || activeThreadId == null) return
      removeQueuedPrompt(activeWorkspaceId, activeThreadId, promptId)
    },
    [activeWorkspaceId, activeThreadId, removeQueuedPrompt],
  )

  const inputComponent = (
    <div className="relative">
      <EscCancelHint visible={escHintVisible} timeoutMs={ESC_TIMEOUT_MS} />
      {queuedPrompts.length > 0 && (
        <div className="mb-3 space-y-2" data-testid="queued-prompts">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <div className="h-px flex-1 bg-border" />
            <span className="flex items-center gap-1.5 px-2">
              <Clock className="w-3 h-3" />
              {queuedPrompts.length} queued
            </span>
            <div className="h-px flex-1 bg-border" />
          </div>

          {queuedPrompts.map((prompt) => (
            <div
              key={prompt.id}
              className="group transition-all duration-200"
              data-testid="queued-prompt-item"
              data-prompt-id={prompt.id}
            >
              <div
                className="group/activity relative"
                data-testid="queued-prompt-bubble"
                style={{
                  border: "1px dashed #e8e8e8",
                  borderRadius: "8px",
                  backgroundColor: "#ffffff",
                  boxShadow:
                    "rgba(0,0,0,0.022) 0px 3px 6px -2px, rgba(0,0,0,0.044) 0px 1px 1px 0px",
                  padding: "12px 16px",
                  marginLeft: "-6px",
                  marginRight: "-6px",
                }}
              >
                <div className="flex items-center gap-2.5 mb-2">
                  <div
                    className="flex items-center justify-center text-white flex-shrink-0"
                    style={{
                      width: "20px",
                      height: "20px",
                      borderRadius: "50%",
                      backgroundColor: "#5e6ad2",
                      fontSize: "9px",
                      fontWeight: 500,
                    }}
                  >
                    U
                  </div>
                  <span style={{ fontSize: "14px", fontWeight: 500, color: "#1b1b1b" }}>You</span>
                  <span style={{ fontSize: "14px", fontWeight: 400, color: "#5b5b5d" }}>Queued</span>
                  <button
                    type="button"
                    onClick={() => handleCancelQueuedPrompt(prompt.id)}
                    className="ml-auto p-1 bg-background border border-border rounded-full shadow-sm opacity-0 group-hover/activity:opacity-100 transition-opacity hover:bg-destructive hover:border-destructive hover:text-destructive-foreground"
                    aria-label="Remove queued message"
                    data-testid="queued-prompt-cancel"
                  >
                    <X className="w-3 h-3" />
                  </button>
                </div>

                <div style={{ fontSize: "13px", fontWeight: 400, lineHeight: "1.625", color: "#1b1b1b" }}>
                  {prompt.text.split("\n").map((line, idx) => (
                    <p key={idx} className="min-h-[1.625em]">
                      {line || "\u00A0"}
                    </p>
                  ))}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
      <ChatComposer
        value={draftText}
        onChange={setDraftText}
        attachments={attachments}
        onRemoveAttachment={removeAttachment}
        onFileSelect={handleFileSelect}
        onPaste={handlePaste}
        onAddAttachmentRef={(attachment) => {
          const isImage = attachment.kind === "image"
          const previewUrl =
            isImage && activeWorkspaceId != null
              ? attachmentHref({ workspaceId: activeWorkspaceId, attachment }) ?? undefined
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
        workspaceId={activeWorkspaceId}
        commands={codexCustomPrompts}
        messageHistory={messageHistory}
        onCommand={handleCommand}
        disabled={activeWorkspaceId == null || activeThreadId == null}
        agentModelId={conversation?.agent_model_id}
        agentThinkingEffort={conversation?.thinking_effort}
        defaultModelId={app?.agent.default_model_id ?? null}
        defaultThinkingEffort={app?.agent.default_thinking_effort ?? null}
        defaultAmpMode={app?.agent.amp_mode ?? null}
        defaultRunner={app?.agent.default_runner ?? null}
        runner={conversation?.agent_runner ?? null}
        ampMode={conversation?.amp_mode ?? null}
        onChangeRunner={(runner) => {
          if (activeWorkspaceId == null || activeThreadId == null) return
          setChatRunner(activeWorkspaceId, activeThreadId, runner)
        }}
        onChangeAmpMode={(mode) => {
          if (activeWorkspaceId == null || activeThreadId == null) return
          if (mode == null) return
          setChatAmpMode(activeWorkspaceId, activeThreadId, mode)
        }}
        onOpenAgentSettings={(agentId, agentFilePath) => openSettingsPanel("agent", { agentId, agentFilePath })}
        onChangeModelId={(modelId) => {
          if (activeWorkspaceId == null || activeThreadId == null) return
          setChatModel(activeWorkspaceId, activeThreadId, modelId)
        }}
        onChangeThinkingEffort={(effort) => {
          if (activeWorkspaceId == null || activeThreadId == null) return
          setThinkingEffort(activeWorkspaceId, activeThreadId, effort)
        }}
        onSend={handleSend}
        canSend={canSend}
        codexEnabled={app?.agent.codex_enabled ?? true}
        ampEnabled={app?.agent.amp_enabled ?? true}
        compact
      />
    </div>
  )

  const taskDescription = useMemo(() => undefined, [])

  return (
    <div className="flex-1 min-w-0 flex flex-col">
      {activePanel === "diff" ? (
        <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
          <div
            className="flex items-center gap-2 h-[39px] flex-shrink-0"
            style={{ padding: "0 20px", borderBottom: "1px solid #ebebeb" }}
          >
            <button
              type="button"
              className="inline-flex items-center gap-2 text-[13px] rounded px-2 py-1 hover:bg-black/[0.03]"
              style={{ color: "#1b1b1b" }}
              onClick={() => setActivePanel("activity")}
            >
              <ArrowLeft className="w-3.5 h-3.5" style={{ color: "#6b6b6b" }} />
              Back
            </button>
            <span className="text-[13px]" style={{ color: "#1b1b1b", fontWeight: 500 }}>
              Diff
            </span>
          </div>

          <DiffTabPanel
            isLoading={isDiffLoading}
            error={diffError}
            files={diffFiles}
            activeFileId={diffActiveFileId}
            diffStyle={diffStyle}
            onStyleChange={setDiffStyle}
          />
        </div>
      ) : (
	        <TaskActivityView
	          listKey={`${activeWorkspaceId ?? "none"}:${activeThreadId ?? "none"}`}
	          title={taskTitle}
	          description={taskDescription}
	          workspaceId={activeWorkspaceId ?? undefined}
	          messages={messages}
          isLoading={isAgentRunning}
          onCancelAgentTurn={isAgentRunning ? () => cancelAgentTurn() : undefined}
          inputComponent={inputComponent}
          className="flex-1 min-w-0"
        />
      )}
    </div>
  )
}
