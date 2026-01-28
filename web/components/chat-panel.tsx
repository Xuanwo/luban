"use client"

import type React from "react"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  ArrowDown,
  Clock,
  X,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { useLuban } from "@/lib/luban-context"
import { buildMessages, type Message } from "@/lib/conversation-ui"
import { ConversationView } from "@/components/conversation-view"
import { VirtualizedConversationView } from "@/components/virtualized-conversation-view"
import { fetchCodexCustomPrompts, fetchWorkspaceDiff, uploadAttachment } from "@/lib/luban-http"
import type {
  AttachmentRef,
  CodexCustomPromptSnapshot,
  QueuedPromptSnapshot,
  ThinkingEffort,
} from "@/lib/luban-api"
import { attachmentHref } from "@/lib/attachment-href"
import { onAddChatAttachments } from "@/lib/chat-attachment-events"
import { emitContextChanged } from "@/lib/context-events"
import {
  draftKey,
  followTailKey,
  loadJson,
  saveJson,
} from "@/lib/ui-prefs"
import type { ChangedFile } from "./right-sidebar"
import { type ComposerAttachment as EditorComposerAttachment } from "@/components/shared/message-editor"
import { AgentRunningCard, type AgentRunningStatus } from "@/components/shared/agent-running-card"
import { openSettingsPanel } from "@/lib/open-settings"
import { focusChatInput } from "@/lib/focus-chat-input"
import { useAgentCancelHotkey } from "@/lib/use-agent-cancel-hotkey"
import { useThreadTabs, type ArchivedTab } from "@/lib/use-thread-tabs"
import { ThreadTabsBar } from "@/components/thread-tabs-bar"
import { DiffTabPanel, type DiffFileData, type DiffStyle } from "@/components/diff-tab-panel"
import { QueuedPromptRow } from "@/components/queued-prompts"
import { EscCancelHint } from "@/components/esc-cancel-hint"
import { ChatComposer } from "@/components/chat-composer"
import { WorkspaceChatHeader } from "@/components/workspace-chat-header"
import { getActiveProjectInfo } from "@/lib/active-project-info"
import type { AgentRunnerOverride, AmpModeOverride } from "@/components/shared/agent-selector"

type ComposerAttachment = EditorComposerAttachment

type PersistedChatDraft = {
  text: string
  attachments?: AttachmentRef[]
}

export function ChatPanel({
  pendingDiffFile,
  onDiffFileOpened,
}: {
  pendingDiffFile?: ChangedFile | null
  onDiffFileOpened?: () => void
}) {
  const [codexCustomPrompts, setCodexCustomPrompts] = useState<CodexCustomPromptSnapshot[]>([])

  const scrollContainerRef = useRef<HTMLDivElement | null>(null)
  const [scrollContainerEl, setScrollContainerEl] = useState<HTMLDivElement | null>(null)
  const setScrollContainer = useCallback((el: HTMLDivElement | null) => {
    scrollContainerRef.current = el
    setScrollContainerEl(el)
  }, [])
  const prependLoadRef = useRef<{
    prevEntriesStart: number
    prevScrollTop: number
    prevScrollHeight: number
  } | null>(null)

  const {
    app,
    activeWorkspaceId,
    activeWorkspace,
    activeThreadId,
    threads,
    workspaceTabs,
    conversation,
    selectThread,
    createThread,
    closeThreadTab,
    restoreThreadTab,
    sendAgentMessage,
    queueAgentMessage,
    cancelAgentTurn,
    cancelAndSendAgentMessage,
    renameWorkspaceBranch,
    aiRenameWorkspaceBranch,
    removeQueuedPrompt,
    reorderQueuedPrompt,
    updateQueuedPrompt,
    loadConversationBefore,
    setChatModel,
    setThinkingEffort,
  } = useLuban()

  const [draftText, setDraftText] = useState("")
  const [runnerOverride, setRunnerOverride] = useState<AgentRunnerOverride>(null)
  const [ampModeOverride, setAmpModeOverride] = useState<AmpModeOverride>(null)
  const [followTail, setFollowTail] = useState(true)
  const [freezeConversationRender, setFreezeConversationRender] = useState(false)
  const programmaticScrollRef = useRef(false)
  const pinToBottomRef = useRef<{ epoch: number; until: number; raf: number | null } | null>(null)
  const pinToBottomEpochRef = useRef(0)
  const [isLoadingOlder, setIsLoadingOlder] = useState(false)
  const loadingOlderRef = useRef(false)
  const lastScrollTopRef = useRef(0)
  const freezeConversationRenderRef = useRef(false)
  const latestMessagesRef = useRef<Message[]>([])
  const frozenMessagesRef = useRef<Message[] | null>(null)

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

  const queuedPrompts = conversation?.pending_prompts ?? []
  const queuePaused = conversation?.queue_paused ?? false
  const [editingQueuedPromptId, setEditingQueuedPromptId] = useState<number | null>(null)
  const [draggingQueuedPromptId, setDraggingQueuedPromptId] = useState<number | null>(null)
  const [queuedDraftText, setQueuedDraftText] = useState("")
  const [queuedDraftAttachments, setQueuedDraftAttachments] = useState<ComposerAttachment[]>([])
  const [queuedDraftModelId, setQueuedDraftModelId] = useState<string | null>(null)
  const [queuedDraftThinkingEffort, setQueuedDraftThinkingEffort] = useState<ThinkingEffort | null>(null)
  const queuedAttachmentScopeRef = useRef<string>("")

  const [agentOverrideStatus, setAgentOverrideStatus] = useState<AgentRunningStatus | null>(null)
  const [agentEditorValue, setAgentEditorValue] = useState("")
  const [agentEditorAttachments, setAgentEditorAttachments] = useState<ComposerAttachment[]>([])
  const agentAttachmentScopeRef = useRef<string>("")
  const [agentRunNowMs, setAgentRunNowMs] = useState(() => Date.now())

  const [activePanel, setActivePanel] = useState<"thread" | "diff">("thread")
  const [diffStyle, setDiffStyle] = useState<DiffStyle>("split")
  const [diffFiles, setDiffFiles] = useState<DiffFileData[]>([])
  const [diffActiveFileId, setDiffActiveFileId] = useState<string | undefined>(undefined)
  const [isDiffTabOpen, setIsDiffTabOpen] = useState(false)
  const [isDiffLoading, setIsDiffLoading] = useState(false)
  const [diffError, setDiffError] = useState<string | null>(null)

  const [isEditingBranchName, setIsEditingBranchName] = useState(false)
  const [branchRenamePending, setBranchRenamePending] = useState<{ initialBranch: string; startedAt: number } | null>(
    null,
  )
  const branchRenameSawRunningRef = useRef(false)
  const branchRenameTimeoutRef = useRef<number | null>(null)

  useEffect(() => {
    return onAddChatAttachments((incoming) => {
      if (activeWorkspaceId == null || activeThreadId == null) return
      const scopeAtStart = attachmentScopeRef.current
      const items: ComposerAttachment[] = incoming.map((attachment) => {
        const isImage = attachment.kind === "image"
        const previewUrl = isImage ? attachmentHref({ workspaceId: activeWorkspaceId, attachment }) ?? undefined : undefined
        return {
          id: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
          type: isImage ? "image" : "file",
          name: attachment.name,
          size: attachment.byte_len,
          previewUrl,
          status: "ready",
          attachment,
        }
      })

      if (attachmentScopeRef.current !== scopeAtStart) return
      setAttachments((prev) => [...prev, ...items])
    })
  }, [activeWorkspaceId, activeThreadId])

  useEffect(() => {
    agentAttachmentScopeRef.current = `${attachmentScope}:agent:${Date.now()}`
    setAgentOverrideStatus(null)
    setAgentEditorValue("")
    setAgentEditorAttachments([])
    setRunnerOverride(null)
    setAmpModeOverride(null)
    setAgentRunNowMs(Date.now())
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeWorkspaceId, activeThreadId])

  const messages = useMemo(() => buildMessages(conversation), [conversation])
  useEffect(() => {
    latestMessagesRef.current = messages
  }, [messages])

  useEffect(() => {
    freezeConversationRenderRef.current = freezeConversationRender
  }, [freezeConversationRender])

  const selectionActiveInConversation = (() => {
    const root = scrollContainerRef.current
    if (!root) return false

    const sel = document.getSelection()
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) return false

    const range = sel.getRangeAt(0)
    let node: Node | null = range.commonAncestorContainer
    if (node && node.nodeType === Node.TEXT_NODE) node = node.parentNode
    if (!(node instanceof Element)) return false
    if (!root.contains(node)) return false
    if (node.closest("input, textarea, [contenteditable='true']")) return false
    return true
  })()

  if (selectionActiveInConversation) {
    if (!frozenMessagesRef.current) {
      frozenMessagesRef.current = latestMessagesRef.current
    }
  } else if (frozenMessagesRef.current) {
    frozenMessagesRef.current = null
  }

  useEffect(() => {
    const computeFrozen = (): boolean => {
      const root = scrollContainerRef.current
      if (!root) return false

      const sel = document.getSelection()
      if (!sel || sel.isCollapsed || sel.rangeCount === 0) return false

      const range = sel.getRangeAt(0)
      let node: Node | null = range.commonAncestorContainer
      if (node && node.nodeType === Node.TEXT_NODE) node = node.parentNode
      if (!(node instanceof Element)) return false
      if (!root.contains(node)) return false
      if (node.closest("input, textarea, [contenteditable='true']")) return false
      return true
    }

    const onSelectionChange = () => {
      const next = computeFrozen()
      const prev = freezeConversationRenderRef.current
      if (next === prev) return

      if (next) {
        frozenMessagesRef.current = latestMessagesRef.current
      } else {
        frozenMessagesRef.current = null
      }
      setFreezeConversationRender(next)
    }

    document.addEventListener("selectionchange", onSelectionChange)
    window.addEventListener("mouseup", onSelectionChange)
    window.addEventListener("keyup", onSelectionChange)
    return () => {
      document.removeEventListener("selectionchange", onSelectionChange)
      window.removeEventListener("mouseup", onSelectionChange)
      window.removeEventListener("keyup", onSelectionChange)
    }
  }, [])

  const displayMessages = frozenMessagesRef.current ?? messages
  const agentCardMessageId = useMemo(() => {
    for (let idx = displayMessages.length - 1; idx >= 0; idx -= 1) {
      const msg = displayMessages[idx]
      if (!msg) continue
      if (msg.type === "assistant" && msg.activities && msg.activities.length > 0) return msg.id
    }
    return null
  }, [displayMessages])
  const messageHistory = useMemo(() => {
    const entries = conversation?.entries ?? []
    const isUserMessage = (
      entry: (typeof entries)[number],
    ): entry is Extract<(typeof entries)[number], { type: "user_message" }> => entry.type === "user_message"
    const items = entries
      .filter(isUserMessage)
      .map((entry) => entry.text)
      .filter((text) => text.trim().length > 0)
    return items.slice(-50)
  }, [conversation?.entries])

  const requestOlderConversationPage = useCallback(async () => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    const snap = conversation
    if (!snap) return
    const start = snap.entries_start ?? 0
    if (start <= 0) return
    if (loadingOlderRef.current) return

    const el = scrollContainerRef.current
    if (!el) return

    loadingOlderRef.current = true
    setIsLoadingOlder(true)
    prependLoadRef.current = {
      prevEntriesStart: start,
      prevScrollTop: el.scrollTop,
      prevScrollHeight: el.scrollHeight,
    }

    await loadConversationBefore(activeWorkspaceId, activeThreadId, start)
    loadingOlderRef.current = false
    setIsLoadingOlder(false)
  }, [activeThreadId, activeWorkspaceId, conversation, loadConversationBefore])

  useEffect(() => {
    const pending = prependLoadRef.current
    if (!pending) return
    const el = scrollContainerRef.current
    if (!el) return
    if (!conversation) return
    const start = conversation.entries_start ?? 0
    if (start >= pending.prevEntriesStart) return

    prependLoadRef.current = null
    const nextScrollHeight = el.scrollHeight
    const delta = nextScrollHeight - pending.prevScrollHeight
    if (delta <= 0) return

    programmaticScrollRef.current = true
    requestAnimationFrame(() => {
      el.scrollTop = pending.prevScrollTop + delta
      requestAnimationFrame(() => {
        programmaticScrollRef.current = false
      })
    })
  }, [conversation?.entries_start])

  useEffect(() => {
    void fetchCodexCustomPrompts()
      .then((prompts) => setCodexCustomPrompts(prompts))
      .catch((err) => {
        console.warn("failed to load codex prompts:", err)
        setCodexCustomPrompts([])
      })
  }, [])

  const handleCommand = useCallback(
    (commandId: string) => {
      const match = codexCustomPrompts.find((p) => p.id === commandId) ?? null
      if (!match) return
      setDraftText(match.contents)
      focusChatInput()
    },
    [codexCustomPrompts],
  )

  const handleAgentCommand = useCallback(
    (commandId: string) => {
      const match = codexCustomPrompts.find((p) => p.id === commandId) ?? null
      if (!match) return
      setAgentEditorValue(match.contents)
    },
    [codexCustomPrompts],
  )

  const projectInfo = useMemo(() => getActiveProjectInfo(app, activeWorkspaceId), [app, activeWorkspaceId])

  const isBranchRenaming = activeWorkspace?.branch_rename_status === "running" || branchRenamePending != null

  useEffect(() => {
    if (branchRenamePending == null) {
      branchRenameSawRunningRef.current = false
      if (branchRenameTimeoutRef.current != null) {
        window.clearTimeout(branchRenameTimeoutRef.current)
        branchRenameTimeoutRef.current = null
      }
      return
    }

    if (projectInfo.branch !== branchRenamePending.initialBranch) {
      setBranchRenamePending(null)
      return
    }

    if (activeWorkspace?.branch_rename_status === "running") {
      branchRenameSawRunningRef.current = true
      return
    }

    if (branchRenameSawRunningRef.current) {
      setBranchRenamePending(null)
      return
    }

    if (branchRenameTimeoutRef.current == null) {
      branchRenameTimeoutRef.current = window.setTimeout(() => {
        branchRenameTimeoutRef.current = null
        setBranchRenamePending(null)
      }, 2000)
    }
  }, [activeWorkspace?.branch_rename_status, branchRenamePending, projectInfo.branch])

  const handleRenameBranch = useCallback(
    (nextBranch: string) => {
      if (activeWorkspaceId == null) return
      if (!projectInfo.branch) return
      setBranchRenamePending({ initialBranch: projectInfo.branch, startedAt: Date.now() })
      renameWorkspaceBranch(activeWorkspaceId, nextBranch)
    },
    [activeWorkspaceId, projectInfo.branch, renameWorkspaceBranch],
  )

  const handleAiRenameBranch = useCallback(() => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    if (!projectInfo.branch) return
    setBranchRenamePending({ initialBranch: projectInfo.branch, startedAt: Date.now() })
    aiRenameWorkspaceBranch(activeWorkspaceId, activeThreadId)
  }, [activeWorkspaceId, activeThreadId, aiRenameWorkspaceBranch, projectInfo.branch])

  const { tabs, archivedTabs, openThreadIds, activeTabId } = useThreadTabs({
    threads,
    workspaceTabs,
    activeThreadId,
  })

  const stopPinToBottom = useCallback(() => {
    const pending = pinToBottomRef.current
    if (pending?.raf != null) {
      window.cancelAnimationFrame(pending.raf)
    }
    pinToBottomRef.current = null
    programmaticScrollRef.current = false
  }, [])

  const isPinningBottom = useCallback((): boolean => {
    const pending = pinToBottomRef.current
    return pending != null && Date.now() < pending.until
  }, [])

  const startPinToBottom = useCallback(
    (durationMs: number) => {
      pinToBottomEpochRef.current += 1
      const epoch = pinToBottomEpochRef.current
      const until = Date.now() + durationMs

      const prev = pinToBottomRef.current
      if (prev?.raf != null) window.cancelAnimationFrame(prev.raf)
      pinToBottomRef.current = { epoch, until, raf: null }
      programmaticScrollRef.current = true

      const tick = () => {
        const pending = pinToBottomRef.current
        if (!pending || pending.epoch !== epoch) return
        const el = scrollContainerRef.current
        if (!el) {
          if (Date.now() >= until) stopPinToBottom()
          else pending.raf = window.requestAnimationFrame(tick)
          return
        }

        el.scrollTop = el.scrollHeight

        if (Date.now() >= until) {
          stopPinToBottom()
          return
        }

        pending.raf = window.requestAnimationFrame(tick)
      }

      pinToBottomRef.current.raf = window.requestAnimationFrame(tick)
    },
    [stopPinToBottom],
  )

  useEffect(() => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    if (attachmentScopeRef.current !== attachmentScope) return

    const readyAttachments = attachments
      .filter((a) => a.status === "ready" && a.attachment != null)
      .map((a) => a.attachment!)

    saveJson(draftKey(activeWorkspaceId, activeThreadId), {
      text: draftText,
      attachments: readyAttachments,
    } satisfies PersistedChatDraft)
  }, [activeThreadId, activeWorkspaceId, attachmentScope, attachments, draftText])

  useEffect(() => {
    if (activeWorkspaceId == null || activeThreadId == null) {
      setDraftText("")
      setAttachments([])
      setEditingQueuedPromptId(null)
      setQueuedDraftText("")
      setQueuedDraftAttachments([])
      setQueuedDraftModelId(null)
      setQueuedDraftThinkingEffort(null)
      return
    }

    setFollowTail(true)
    localStorage.setItem(followTailKey(activeWorkspaceId, activeThreadId), "true")
    startPinToBottom(500)

    attachmentScopeRef.current = attachmentScope

    const saved = loadJson<PersistedChatDraft>(draftKey(activeWorkspaceId, activeThreadId))
    setDraftText(saved?.text ?? "")
    setAttachments(saved?.attachments ? attachmentsFromRefs(activeWorkspaceId, saved.attachments) : [])
    setEditingQueuedPromptId(null)
    setQueuedDraftText("")
    setQueuedDraftAttachments([])
    setQueuedDraftModelId(null)
    setQueuedDraftThinkingEffort(null)
  }, [activeWorkspaceId, activeThreadId, startPinToBottom])

  useEffect(() => {
    if (!followTail) stopPinToBottom()
  }, [followTail, stopPinToBottom])

  useEffect(() => {
    if (!scrollContainerEl) return
    if (activeWorkspaceId == null || activeThreadId == null) return
    if (!followTail) return
    if (!isPinningBottom()) return
    startPinToBottom(250)
  }, [activeThreadId, activeWorkspaceId, followTail, isPinningBottom, scrollContainerEl, startPinToBottom])

  useEffect(() => stopPinToBottom, [stopPinToBottom])

  useEffect(() => {
    setIsDiffTabOpen(false)
    setActivePanel("thread")
    setDiffFiles([])
    setDiffActiveFileId(undefined)
    setIsDiffLoading(false)
    setDiffError(null)
  }, [activeWorkspaceId])

  const openDiffTab = useCallback(
    async (targetFile: ChangedFile) => {
      if (activeWorkspaceId == null) return
      setIsDiffTabOpen(true)
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

  function scheduleScrollToBottom() {
    const el = scrollContainerRef.current
    if (!el) return

    programmaticScrollRef.current = true
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        el.scrollTop = el.scrollHeight
        if (!isPinningBottom()) programmaticScrollRef.current = false
      })
    })
  }

  useEffect(() => {
    if (freezeConversationRender) return
    if (!followTail) return
    if (displayMessages.length === 0) return
    scheduleScrollToBottom()
  }, [displayMessages.length, followTail, activeWorkspaceId, activeThreadId, freezeConversationRender])

  const handleTabClick = (tabId: string) => {
    const id = Number(tabId)
    if (!Number.isFinite(id)) return
    setActivePanel("thread")
    void selectThread(id)
  }

  const handleDiffTabClick = () => {
    if (!isDiffTabOpen) return
    setActivePanel("diff")
  }

  const handleCloseDiffTab = (e: React.MouseEvent) => {
    e.stopPropagation()
    setIsDiffTabOpen(false)
    setActivePanel("thread")
    setDiffActiveFileId(undefined)
  }

  const handleCloseTab = (tabId: string, e: React.MouseEvent) => {
    e.stopPropagation()
    const id = Number(tabId)
    if (!Number.isFinite(id)) return
    if (openThreadIds.length <= 1) return
    void closeThreadTab(id)
  }

  const handleAddTab = () => {
    if (activeWorkspaceId == null) return
    createThread()
  }

  const handleRestoreTab = (tab: ArchivedTab) => {
    if (activeWorkspaceId == null) return
    const id = Number(tab.id)
    if (!Number.isFinite(id)) return
    void restoreThreadTab(id)
  }

  const handleSend = () => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    const text = draftText.trim()
    const ready = attachments
      .filter((a) => a.status === "ready" && a.attachment != null)
      .map((a) => a.attachment!)
    const hasUploading = attachments.some((a) => a.status === "uploading")
    if (hasUploading) return
    if (text.length === 0 && ready.length === 0) return

    const defaultRunner = app?.agent?.default_runner ?? "codex"
    const effectiveRunner = runnerOverride ?? defaultRunner
    const runConfig = {
      runner: runnerOverride,
      amp_mode: effectiveRunner === "amp" && ampModeOverride != null ? ampModeOverride : null,
    }
    if (queuePaused && queuedPrompts.length > 0) {
      queueAgentMessage(text, ready, runConfig)
    } else {
      sendAgentMessage(text, ready, runConfig)
    }
    setDraftText("")
    setAttachments([])
    setFollowTail(true)
    localStorage.setItem(followTailKey(activeWorkspaceId, activeThreadId), "true")
    scheduleScrollToBottom()
  }

  const baseAgentStatus = useMemo<AgentRunningStatus | null>(() => {
    if (!conversation) return null
    if (conversation.run_status === "running") return "running"
    if (queuePaused && queuedPrompts.length > 0) return "paused"
    return null
  }, [conversation, queuePaused, queuedPrompts.length])

  const agentStatus = agentOverrideStatus ?? baseAgentStatus

  const agentTurnIsRunning = conversation?.run_status === "running"
  const agentRunStartedAtMs = conversation?.run_started_at_unix_ms ?? null
  const agentRunFinishedAtMs = conversation?.run_finished_at_unix_ms ?? null

  useEffect(() => {
    if (!agentTurnIsRunning) return
    if (agentRunStartedAtMs == null) return
    const timer = window.setInterval(() => {
      setAgentRunNowMs(Date.now())
    }, 250)

    return () => window.clearInterval(timer)
  }, [agentTurnIsRunning, agentRunStartedAtMs])

  const agentRunElapsedLabel = useMemo(() => {
    if (agentRunStartedAtMs == null) return "00:00"
    const end = agentTurnIsRunning ? agentRunNowMs : (agentRunFinishedAtMs ?? agentRunNowMs)
    const totalSeconds = Math.max(0, Math.floor((end - agentRunStartedAtMs) / 1000))
    const minutes = Math.floor(totalSeconds / 60)
    const seconds = totalSeconds % 60
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`
  }, [agentRunFinishedAtMs, agentRunNowMs, agentRunStartedAtMs, agentTurnIsRunning])

  useEffect(() => {
    if (baseAgentStatus == null && agentOverrideStatus != null) {
      setAgentOverrideStatus(null)
    }
  }, [agentOverrideStatus, baseAgentStatus])

  const handleAgentCancel = useCallback(() => {
    if (!agentStatus) return
    setAgentOverrideStatus("cancelling")
  }, [agentStatus])

  const handleAgentResume = useCallback(() => {
    if (!agentStatus) return
    setAgentOverrideStatus("resuming")
  }, [agentStatus])

  const { escHintVisible, escTimeoutMs: ESC_TIMEOUT_MS, clearEscHint } = useAgentCancelHotkey({
    enabled: agentStatus === "running",
    blocked: editingQueuedPromptId != null || isEditingBranchName,
    onCancel: handleAgentCancel,
  })

  const clearAgentEditor = () => {
    setAgentEditorValue("")
    setAgentEditorAttachments([])
  }

  const handleAgentDismiss = () => {
    if (agentOverrideStatus === "cancelling") {
      cancelAgentTurn()
    }
    setAgentOverrideStatus(null)
    clearAgentEditor()
  }

  const handleAgentSubmit = () => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    const text = agentEditorValue.trim()
    const hasUploading = agentEditorAttachments.some((a) => a.status === "uploading")
    if (hasUploading) return
    const ready = agentEditorAttachments
      .filter((a) => a.status === "ready" && a.attachment != null)
      .map((a) => a.attachment!)
    if (text.length === 0 && ready.length === 0) return

    const defaultRunner = app?.agent?.default_runner ?? "codex"
    const effectiveRunner = runnerOverride ?? defaultRunner
    const runConfig = {
      runner: runnerOverride,
      amp_mode: effectiveRunner === "amp" && ampModeOverride != null ? ampModeOverride : null,
    }

    if (agentOverrideStatus === "cancelling") {
      cancelAndSendAgentMessage(text, ready, runConfig)
    } else if (agentOverrideStatus === "resuming") {
      sendAgentMessage(text, ready, runConfig)
    }

    setAgentOverrideStatus(null)
    clearAgentEditor()
    setFollowTail(true)
    if (activeWorkspaceId != null && activeThreadId != null) {
      localStorage.setItem(followTailKey(activeWorkspaceId, activeThreadId), "true")
      scheduleScrollToBottom()
    }
  }

  const handleStartQueuedEdit = (promptId: number) => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    const prompt = queuedPrompts.find((p) => p.id === promptId) ?? null
    if (!prompt) return

    setEditingQueuedPromptId(promptId)
    setQueuedDraftText(prompt.text)
    setQueuedDraftAttachments(attachmentsFromRefs(activeWorkspaceId, prompt.attachments ?? []))
    setQueuedDraftModelId(prompt.run_config?.model_id ?? null)
    setQueuedDraftThinkingEffort(prompt.run_config?.thinking_effort ?? null)
    queuedAttachmentScopeRef.current = `${attachmentScope}:queued:${promptId}:${Date.now()}`
  }

  const handleCancelQueuedEdit = () => {
    setEditingQueuedPromptId(null)
    setQueuedDraftText("")
    setQueuedDraftAttachments([])
    setQueuedDraftModelId(null)
    setQueuedDraftThinkingEffort(null)
  }

  const handleSaveQueuedEdit = () => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    if (editingQueuedPromptId == null) return

    const text = queuedDraftText.trim()
    const hasUploading = queuedDraftAttachments.some((a) => a.status === "uploading")
    if (hasUploading) return
    const ready = queuedDraftAttachments
      .filter((a) => a.status === "ready" && a.attachment != null)
      .map((a) => a.attachment!)

    if (text.length === 0 && ready.length === 0) {
      handleCancelQueuedEdit()
      return
    }

    const modelId = queuedDraftModelId ?? conversation?.agent_model_id ?? ""
    const effort = queuedDraftThinkingEffort ?? conversation?.thinking_effort ?? "minimal"
    updateQueuedPrompt(activeWorkspaceId, activeThreadId, editingQueuedPromptId, {
      text,
      attachments: ready,
      runConfig: { model_id: modelId, thinking_effort: effort },
    })
    handleCancelQueuedEdit()
  }

  const handleCancelQueuedPrompt = (promptId: number) => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    if (editingQueuedPromptId === promptId) {
      handleCancelQueuedEdit()
    }
    removeQueuedPrompt(activeWorkspaceId, activeThreadId, promptId)
  }

  useEffect(() => {
    if (editingQueuedPromptId == null) return
    if (!queuedPrompts.some((p) => p.id === editingQueuedPromptId)) {
      handleCancelQueuedEdit()
    }
  }, [queuedPrompts, editingQueuedPromptId])
  const handleQueueDrop = (activeId: number, overId: number) => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    if (editingQueuedPromptId != null) return
    if (activeId === overId) return
    setDraggingQueuedPromptId(null)
    reorderQueuedPrompt(activeWorkspaceId, activeThreadId, activeId, overId)
  }

  const handleFileSelect = (files: FileList | null) => {
    if (!files) return
    if (activeWorkspaceId == null || activeThreadId == null) return

    const scopeAtStart = attachmentScopeRef.current

    for (const file of Array.from(files)) {
      const isImage = file.type.startsWith("image/")
      const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`
      const initial: ComposerAttachment = {
        id,
        type: isImage ? "image" : "file",
        name: file.name,
        size: file.size,
        status: "uploading",
      }

      if (isImage) {
        const reader = new FileReader()
        reader.onload = (e) => {
          const preview = typeof e.target?.result === "string" ? e.target.result : undefined
          setAttachments((prev) => prev.map((a) => (a.id === id ? { ...a, preview } : a)))
        }
        reader.readAsDataURL(file)
      }

      setAttachments((prev) => [...prev, initial])

      void uploadAttachment({ workspaceId: activeWorkspaceId, file, kind: isImage ? "image" : "file" })
        .then((attachment) => {
          if (attachmentScopeRef.current !== scopeAtStart) return
          setAttachments((prev) =>
            prev.map((a) => (a.id === id ? { ...a, status: "ready", attachment, name: attachment.name } : a)),
          )
          emitContextChanged(activeWorkspaceId)
        })
        .catch(() => {
          if (attachmentScopeRef.current !== scopeAtStart) return
          setAttachments((prev) => prev.map((a) => (a.id === id ? { ...a, status: "failed" } : a)))
        })
    }
  }

  const handleQueuedFileSelect = (files: FileList | null) => {
    if (!files) return
    if (activeWorkspaceId == null || activeThreadId == null) return
    if (editingQueuedPromptId == null) return

    const scopeAtStart = queuedAttachmentScopeRef.current

    for (const file of Array.from(files)) {
      const isImage = file.type.startsWith("image/")
      const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`
      const initial: ComposerAttachment = {
        id,
        type: isImage ? "image" : "file",
        name: file.name,
        size: file.size,
        status: "uploading",
      }

      if (isImage) {
        const reader = new FileReader()
        reader.onload = (e) => {
          const preview = typeof e.target?.result === "string" ? e.target.result : undefined
          setQueuedDraftAttachments((prev) => prev.map((a) => (a.id === id ? { ...a, preview } : a)))
        }
        reader.readAsDataURL(file)
      }

      setQueuedDraftAttachments((prev) => [...prev, initial])

      void uploadAttachment({ workspaceId: activeWorkspaceId, file, kind: isImage ? "image" : "file" })
        .then((attachment) => {
          if (queuedAttachmentScopeRef.current !== scopeAtStart) return
          setQueuedDraftAttachments((prev) =>
            prev.map((a) => (a.id === id ? { ...a, status: "ready", attachment, name: attachment.name } : a)),
          )
          emitContextChanged(activeWorkspaceId)
        })
        .catch(() => {
          if (queuedAttachmentScopeRef.current !== scopeAtStart) return
          setQueuedDraftAttachments((prev) => prev.map((a) => (a.id === id ? { ...a, status: "failed" } : a)))
        })
    }
  }

  const handleAgentFileSelect = (files: FileList | null) => {
    if (!files) return
    if (activeWorkspaceId == null || activeThreadId == null) return

    const scopeAtStart = agentAttachmentScopeRef.current

    for (const file of Array.from(files)) {
      const isImage = file.type.startsWith("image/")
      const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`
      const initial: ComposerAttachment = {
        id,
        type: isImage ? "image" : "file",
        name: file.name,
        size: file.size,
        status: "uploading",
      }

      if (isImage) {
        const reader = new FileReader()
        reader.onload = (e) => {
          const preview = typeof e.target?.result === "string" ? e.target.result : undefined
          setAgentEditorAttachments((prev) => prev.map((a) => (a.id === id ? { ...a, preview } : a)))
        }
        reader.readAsDataURL(file)
      }

      setAgentEditorAttachments((prev) => [...prev, initial])

      void uploadAttachment({ workspaceId: activeWorkspaceId, file, kind: isImage ? "image" : "file" })
        .then((attachment) => {
          if (agentAttachmentScopeRef.current !== scopeAtStart) return
          setAgentEditorAttachments((prev) =>
            prev.map((a) => (a.id === id ? { ...a, status: "ready", attachment, name: attachment.name } : a)),
          )
          emitContextChanged(activeWorkspaceId)
        })
        .catch(() => {
          if (agentAttachmentScopeRef.current !== scopeAtStart) return
          setAgentEditorAttachments((prev) => prev.map((a) => (a.id === id ? { ...a, status: "failed" } : a)))
        })
    }
  }

  const handlePaste = (e: React.ClipboardEvent) => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    const items = e.clipboardData?.items
    if (!items) return

    const imageItems = Array.from(items).filter((item) => item.type.startsWith("image/"))
    if (imageItems.length === 0) return

    e.preventDefault()
    const dt = new DataTransfer()
    for (const item of imageItems) {
      const file = item.getAsFile()
      if (file) dt.items.add(file)
    }
    handleFileSelect(dt.files)
  }

  const handleAgentPaste = (e: React.ClipboardEvent) => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    const items = e.clipboardData?.items
    if (!items) return

    const imageItems = Array.from(items).filter((item) => item.type.startsWith("image/"))
    if (imageItems.length === 0) return

    e.preventDefault()
    const dt = new DataTransfer()
    for (const item of imageItems) {
      const file = item.getAsFile()
      if (file) dt.items.add(file)
    }
    handleAgentFileSelect(dt.files)
  }

  const handleQueuedPaste = (e: React.ClipboardEvent) => {
    if (activeWorkspaceId == null || activeThreadId == null) return
    if (editingQueuedPromptId == null) return
    const items = e.clipboardData?.items
    if (!items) return

    const imageItems = Array.from(items).filter((item) => item.type.startsWith("image/"))
    if (imageItems.length === 0) return

    e.preventDefault()
    const dt = new DataTransfer()
    for (const item of imageItems) {
      const file = item.getAsFile()
      if (file) dt.items.add(file)
    }
    handleQueuedFileSelect(dt.files)
  }

  const removeAttachment = (id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id))
  }

  const removeAgentEditorAttachment = (id: string) => {
    setAgentEditorAttachments((prev) => prev.filter((a) => a.id !== id))
  }

  const renderAgentRunningCardForMessage = useCallback(
    (message: (typeof messages)[number]) => {
      if (!agentStatus) return null
      if (!agentCardMessageId || message.id !== agentCardMessageId) return null
      if (!message.activities || message.activities.length === 0) return null
      const filtered = message.activities.filter((event) => event.title !== "Turn canceled")
      const activities = filtered.length > 0 ? filtered : message.activities
      return (
        <AgentRunningCard
          activities={activities}
          elapsedTime={agentRunElapsedLabel}
          turnStartedAtMs={agentRunStartedAtMs}
          status={agentStatus}
          hasQueuedMessages={queuedPrompts.length > 0}
          editorValue={agentEditorValue}
          editorAttachments={agentEditorAttachments}
          onEditorChange={setAgentEditorValue}
          onEditorAttachmentsChange={setAgentEditorAttachments}
          onRemoveEditorAttachment={removeAgentEditorAttachment}
          onEditorFileSelect={handleAgentFileSelect}
          onEditorPaste={handleAgentPaste}
          onAddEditorAttachmentRef={(attachment) => {
            setAgentEditorAttachments((prev) => [
              ...prev,
              ...attachmentsFromRefs(activeWorkspaceId ?? null, [attachment]),
            ])
          }}
          workspaceId={activeWorkspaceId ?? null}
          commands={codexCustomPrompts}
          messageHistory={messageHistory}
          onCommand={handleAgentCommand}
          onCancel={handleAgentCancel}
          onResume={handleAgentResume}
          onSubmit={handleAgentSubmit}
          onDismiss={handleAgentDismiss}
        />
      )
    },
    [
      activeWorkspaceId,
      agentCardMessageId,
      agentEditorAttachments,
      agentEditorValue,
      agentRunElapsedLabel,
      agentRunStartedAtMs,
      agentStatus,
      attachmentsFromRefs,
      codexCustomPrompts,
      handleAgentCancel,
      handleAgentCommand,
      handleAgentDismiss,
      handleAgentFileSelect,
      handleAgentPaste,
      handleAgentResume,
      handleAgentSubmit,
      messageHistory,
      queuedPrompts.length,
      removeAgentEditorAttachment,
    ],
  )

  const removeQueuedDraftAttachment = (id: string) => {
    setQueuedDraftAttachments((prev) => prev.filter((a) => a.id !== id))
  }

  const canSend = useMemo(() => {
    if (activeWorkspaceId == null || activeThreadId == null) return false
    const hasUploading = attachments.some((a) => a.status === "uploading")
    if (hasUploading) return false
    const hasReady = attachments.some((a) => a.status === "ready" && a.attachment != null)
    return draftText.trim().length > 0 || hasReady
  }, [activeWorkspaceId, activeThreadId, attachments, draftText])

  return (
    <div className="flex-1 min-h-0 flex flex-col min-w-0 bg-background">
      <WorkspaceChatHeader
        projectName={projectInfo.name}
        branchName={projectInfo.branch}
        isGit={projectInfo.isGit}
        isMainBranch={projectInfo.isMainBranch}
        isBranchRenaming={isBranchRenaming}
        onRenameBranch={handleRenameBranch}
        onAiRenameBranch={handleAiRenameBranch}
        onEditingBranchChange={setIsEditingBranchName}
      />

      <ThreadTabsBar
        tabs={tabs}
        archivedTabs={archivedTabs}
        activeTabId={activeTabId}
        activePanel={activePanel}
        isDiffTabOpen={isDiffTabOpen}
        onTabClick={handleTabClick}
        onCloseTab={handleCloseTab}
        onAddTab={handleAddTab}
        onDiffTabClick={handleDiffTabClick}
        onCloseDiffTab={handleCloseDiffTab}
        onRestoreTab={handleRestoreTab}
      />

      {activePanel === "diff" ? (
        <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
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
        <>
          <div
            data-testid="chat-scroll-container"
            className="flex-1 overflow-y-auto relative"
            ref={setScrollContainer}
            onScroll={(e) => {
              if (activeWorkspaceId == null || activeThreadId == null) return
              const el = e.target as HTMLDivElement

              const prevScrollTop = lastScrollTopRef.current
              lastScrollTopRef.current = el.scrollTop

              const isNearTop = el.scrollTop < 400
              const isAtTop = el.scrollTop <= 0
              const isScrollingUp = el.scrollTop < prevScrollTop

              if (
                !programmaticScrollRef.current &&
                !loadingOlderRef.current &&
                (conversation?.entries_start ?? 0) > 0 &&
                isNearTop &&
                (isAtTop || isScrollingUp)
              ) {
                void requestOlderConversationPage()
              }

              const distanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight
              const isNearBottom = distanceToBottom < 50
              if (!programmaticScrollRef.current) {
                setFollowTail(isNearBottom)
                localStorage.setItem(
                  followTailKey(activeWorkspaceId, activeThreadId),
                  isNearBottom ? "true" : "false",
                )
              }
            }}
          >
            <div className="max-w-3xl mx-auto py-4 px-4 pb-20">
              {isLoadingOlder && (
                <div className="flex items-center justify-center py-2 text-xs text-muted-foreground">
                  Loading…
                </div>
              )}
              {displayMessages.length > 200 ? (
                <VirtualizedConversationView
                  messages={displayMessages}
                  workspaceId={activeWorkspaceId ?? undefined}
                  listKey={`${activeWorkspaceId ?? "none"}:${activeThreadId ?? "none"}`}
                  scrollElement={scrollContainerEl}
                  renderAgentRunningCard={renderAgentRunningCardForMessage}
                  emptyState={
                    <div className="text-sm text-muted-foreground">
                      {activeWorkspaceId == null
                        ? "Select a workspace to start."
                        : "Select a thread to load conversation."}
                    </div>
                  }
                />
              ) : (
                <ConversationView
                  messages={displayMessages}
                  workspaceId={activeWorkspaceId ?? undefined}
                  className=""
                  renderAgentRunningCard={renderAgentRunningCardForMessage}
                  emptyState={
                    <div className="text-sm text-muted-foreground">
                      {activeWorkspaceId == null
                        ? "Select a workspace to start."
                      : "Select a thread to load conversation."}
                  </div>
                }
              />
              )}

              {queuedPrompts.length > 0 && (
                <div className="mt-6 space-y-2" data-testid="queued-prompts">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <div className="h-px flex-1 bg-border" />
                    <span className="flex items-center gap-1.5 px-2">
                      <Clock className="w-3 h-3" />
                      {queuedPrompts.length} queued
                    </span>
                    <div className="h-px flex-1 bg-border" />
                  </div>

                  {queuedPrompts.map((prompt) => (
                    <QueuedPromptRow
                      key={prompt.id}
                      prompt={prompt}
                      isEditing={editingQueuedPromptId === prompt.id}
                      isDragging={draggingQueuedPromptId === prompt.id}
                      workspaceId={activeWorkspaceId}
                      commands={codexCustomPrompts}
                      messageHistory={messageHistory}
                      editingText={queuedDraftText}
                      editingAttachments={queuedDraftAttachments}
                      editingModelId={queuedDraftModelId}
                      editingThinkingEffort={queuedDraftThinkingEffort}
                      defaultModelId={app?.agent?.default_model_id ?? null}
                      defaultThinkingEffort={app?.agent?.default_thinking_effort ?? null}
                      onStartEdit={() => handleStartQueuedEdit(prompt.id)}
                      onSaveEdit={handleSaveQueuedEdit}
                      onCancelEdit={handleCancelQueuedEdit}
                      onCancelPrompt={() => handleCancelQueuedPrompt(prompt.id)}
                      onEditingTextChange={setQueuedDraftText}
                      onEditingModelIdChange={setQueuedDraftModelId}
                      onEditingThinkingEffortChange={setQueuedDraftThinkingEffort}
                      onQueuedFileSelect={handleQueuedFileSelect}
                      onQueuedPaste={handleQueuedPaste}
                      onRemoveEditingAttachment={removeQueuedDraftAttachment}
                      onAddEditingAttachmentRef={(attachment) => {
                        setQueuedDraftAttachments((prev) => [
                          ...prev,
                          ...attachmentsFromRefs(activeWorkspaceId ?? null, [attachment]),
                        ])
                      }}
                      onOpenAgentSettings={(agentId, agentFilePath) =>
                        openSettingsPanel("agent", { agentId, agentFilePath })
                      }
                      onQueueDragStart={() => setDraggingQueuedPromptId(prompt.id)}
                      onQueueDragEnd={() => setDraggingQueuedPromptId(null)}
                      onQueueDrop={handleQueueDrop}
                    />
                  ))}
                </div>
              )}
            </div>
          </div>

          <div className="relative z-10 -mt-16 pt-8 bg-gradient-to-t from-background via-background to-transparent pointer-events-none">
            <div className="pointer-events-auto">
              <EscCancelHint visible={escHintVisible} timeoutMs={ESC_TIMEOUT_MS} />

              {!followTail && displayMessages.length > 0 && editingQueuedPromptId == null ? (
                <div className="flex justify-center pb-2">
                  <button
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-card border border-border rounded-full text-xs text-muted-foreground hover:text-foreground hover:border-primary/50 transition-all shadow-sm hover:shadow-md"
                    onClick={() => {
                      if (activeWorkspaceId == null || activeThreadId == null) return
                      setFollowTail(true)
                      localStorage.setItem(followTailKey(activeWorkspaceId, activeThreadId), "true")
                      scheduleScrollToBottom()
                    }}
                  >
                    <ArrowDown className="w-3 h-3" />
                    Scroll to bottom
                  </button>
                </div>
              ) : null}

              {editingQueuedPromptId == null && (
                <ChatComposer
                  value={draftText}
                  onChange={(value) => {
                    setDraftText(value)
                  }}
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
                  runnerOverride={runnerOverride}
                  ampModeOverride={ampModeOverride}
                  onChangeRunnerOverride={setRunnerOverride}
                  onChangeAmpModeOverride={setAmpModeOverride}
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
                />
              )}
            </div>
          </div>
        </>
      )}
    </div>
  )
}
