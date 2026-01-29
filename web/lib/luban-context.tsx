"use client"

import type React from "react"

import { createContext, useContext, useEffect, useMemo, useRef } from "react"
import { toast } from "sonner"

import type {
  AmpConfigEntrySnapshot,
  AppSnapshot,
  AppearanceFontsSnapshot,
  AppearanceTheme,
  AttachmentRef,
  AgentRunnerKind,
  AgentRunConfigSnapshot,
  ClaudeConfigEntrySnapshot,
  CodexConfigEntrySnapshot,
  ConversationSnapshot,
  FeedbackSubmitAction,
  FeedbackSubmitResult,
  FeedbackType,
  ProjectId,
  ServerEvent,
  SystemTaskKind,
  ThreadMeta,
  TaskDraft,
  TaskExecuteMode,
  TaskExecuteResult,
  TaskIntentKind,
  ThinkingEffort,
  OpenTarget,
  WorkspaceId,
  WorkspaceThreadId,
  WorkspaceSnapshot,
  WorkspaceTabsSnapshot,
} from "./luban-api"
import { createLubanActions } from "./luban-actions"
import { fetchApp, fetchConversation, fetchThreads } from "./luban-http"
import { isMockMode } from "./luban-mode"
import { useLubanStore } from "./luban-store"
import { createLubanServerEventHandler } from "./luban-store-events"
import { useExternalLinkInterceptor } from "./external-link-interceptor"
import { useLubanTransport } from "./luban-transport"
import { focusChatInput } from "./focus-chat-input"

function normalizePathLike(raw: string): string {
  return raw.trim().replace(/\/+$/, "")
}

type LubanContextValue = {
  app: AppSnapshot | null
  activeWorkspaceId: WorkspaceId | null
  activeWorkspace: WorkspaceSnapshot | null
  activeThreadId: number | null
  threads: ThreadMeta[]
  workspaceTabs: WorkspaceTabsSnapshot | null
  conversation: ConversationSnapshot | null
  wsConnected: boolean

  pickProjectPath: () => Promise<string | null>
  addProject: (path: string) => void
  addProjectAndOpen: (path: string) => Promise<{ projectId: ProjectId; workspaceId: WorkspaceId }>
  deleteProject: (projectId: ProjectId) => void
  createWorkspace: (projectId: ProjectId) => void
  ensureMainWorkspace: (projectId: ProjectId) => void
  openWorkspaceInIde: (workspaceId: WorkspaceId) => void
  openWorkspaceWith: (workspaceId: WorkspaceId, target: OpenTarget) => void
  openWorkspacePullRequest: (workspaceId: WorkspaceId) => void
  openWorkspacePullRequestFailedAction: (workspaceId: WorkspaceId) => void
  archiveWorkspace: (workspaceId: number) => void
  toggleProjectExpanded: (projectId: ProjectId) => void

  previewTask: (input: string) => Promise<TaskDraft>
  executeTask: (draft: TaskDraft, mode: TaskExecuteMode) => Promise<TaskExecuteResult>
  submitFeedback: (args: {
    title: string
    body: string
    labels: string[]
    feedbackType: FeedbackType
    action: FeedbackSubmitAction
  }) => Promise<FeedbackSubmitResult>

  openWorkspace: (workspaceId: WorkspaceId) => Promise<void>
  selectThread: (threadId: number) => Promise<void>
  loadConversationBefore: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, before: number) => Promise<void>
  createThread: () => void
  closeThreadTab: (threadId: number) => Promise<void>
  restoreThreadTab: (threadId: number) => Promise<void>

  sendAgentMessage: (
    text: string,
    attachments?: AttachmentRef[],
    runConfig?: { runner?: AgentRunnerKind | null; amp_mode?: string | null },
  ) => void
  queueAgentMessage: (
    text: string,
    attachments?: AttachmentRef[],
    runConfig?: { runner?: AgentRunnerKind | null; amp_mode?: string | null },
  ) => void
  sendAgentMessageTo: (
    workspaceId: WorkspaceId,
    threadId: number,
    text: string,
    attachments?: AttachmentRef[],
    runConfig?: { runner?: AgentRunnerKind | null; amp_mode?: string | null },
  ) => void
  removeQueuedPrompt: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, promptId: number) => void
  reorderQueuedPrompt: (
    workspaceId: WorkspaceId,
    threadId: WorkspaceThreadId,
    activeId: number,
    overId: number,
  ) => void
  updateQueuedPrompt: (
    workspaceId: WorkspaceId,
    threadId: WorkspaceThreadId,
    promptId: number,
    args: { text: string; attachments: AttachmentRef[]; runConfig: AgentRunConfigSnapshot },
  ) => void
  cancelAgentTurn: () => void
  cancelAndSendAgentMessage: (
    text: string,
    attachments?: AttachmentRef[],
    runConfig?: { runner?: AgentRunnerKind | null; amp_mode?: string | null },
  ) => void

  renameWorkspaceBranch: (workspaceId: WorkspaceId, branchName: string) => void
  aiRenameWorkspaceBranch: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId) => void

  setChatModel: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, modelId: string) => void
  setThinkingEffort: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, effort: ThinkingEffort) => void
  setChatRunner: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, runner: AgentRunnerKind) => void
  setChatAmpMode: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, ampMode: string) => void
  setAppearanceTheme: (theme: AppearanceTheme) => void
  setAppearanceFonts: (fonts: AppearanceFontsSnapshot) => void
  setGlobalZoom: (zoom: number) => void
  setOpenButtonSelection: (selection: string) => void
  setSidebarProjectOrder: (projectIds: ProjectId[]) => void
  setSidebarWorktreeOrder: (projectId: ProjectId, workspaceIds: WorkspaceId[]) => void

  setCodexEnabled: (enabled: boolean) => void
  setAmpEnabled: (enabled: boolean) => void
  setClaudeEnabled: (enabled: boolean) => void
  setAgentRunner: (runner: AgentRunnerKind) => void
  setAgentAmpMode: (mode: string) => void
  setTaskPromptTemplate: (intentKind: TaskIntentKind, template: string) => void
  setSystemPromptTemplate: (kind: SystemTaskKind, template: string) => void
  checkCodex: () => Promise<{ ok: boolean; message: string | null }>
  getCodexConfigTree: () => Promise<CodexConfigEntrySnapshot[]>
  listCodexConfigDir: (path: string) => Promise<{ path: string; entries: CodexConfigEntrySnapshot[] }>
  readCodexConfigFile: (path: string) => Promise<string>
  writeCodexConfigFile: (path: string, contents: string) => Promise<void>
  checkAmp: () => Promise<{ ok: boolean; message: string | null }>
  getAmpConfigTree: () => Promise<AmpConfigEntrySnapshot[]>
  listAmpConfigDir: (path: string) => Promise<{ path: string; entries: AmpConfigEntrySnapshot[] }>
  readAmpConfigFile: (path: string) => Promise<string>
  writeAmpConfigFile: (path: string, contents: string) => Promise<void>
  checkClaude: () => Promise<{ ok: boolean; message: string | null }>
  getClaudeConfigTree: () => Promise<ClaudeConfigEntrySnapshot[]>
  listClaudeConfigDir: (path: string) => Promise<{ path: string; entries: ClaudeConfigEntrySnapshot[] }>
  readClaudeConfigFile: (path: string) => Promise<string>
  writeClaudeConfigFile: (path: string, contents: string) => Promise<void>
}

const LubanContext = createContext<LubanContextValue | null>(null)

export function LubanProvider({ children }: { children: React.ReactNode }) {
  const store = useLubanStore()
  const { app, activeWorkspaceId, activeThreadId, threads, workspaceTabs, conversation, activeWorkspace } = store.state
  const eventHandlerRef = useRef<(event: ServerEvent) => void>(() => {})
  const pendingAutoOpenWorkspaceIdRef = useRef<WorkspaceId | null>(null)
  const lastActiveProjectIdxRef = useRef<number | null>(null)
  const prevWsConnectedRef = useRef<boolean>(false)

  useExternalLinkInterceptor()

  const { wsConnected, sendAction: sendActionTransport, request: requestTransport } = useLubanTransport({
    onEvent: (event) => eventHandlerRef.current(event),
    onError: (message) => {
      console.warn("server error:", message)
      toast.error(message)
    },
  })

  useEffect(() => {
    if (isMockMode()) return

    const prev = prevWsConnectedRef.current
    prevWsConnectedRef.current = wsConnected
    if (prev || !wsConnected) return

    void (async () => {
      try {
        const snap = await fetchApp()
        store.setApp(snap)
      } catch (err) {
        console.warn("resync fetchApp failed", err)
      }

      const wid = store.refs.activeWorkspaceIdRef.current
      if (wid == null) return

      let threadsSnap = null as Awaited<ReturnType<typeof fetchThreads>> | null
      try {
        threadsSnap = await fetchThreads(wid)
      } catch (err) {
        console.warn("resync fetchThreads failed", err)
        return
      }

      if (threadsSnap == null) return

      store.cacheThreads(wid, threadsSnap.threads)
      store.setThreads(threadsSnap.threads)
      store.cacheWorkspaceTabs(wid, threadsSnap.tabs)
      store.setWorkspaceTabs(threadsSnap.tabs)

      const threadIds = new Set(threadsSnap.threads.map((t) => t.thread_id))
      const currentTid = store.refs.activeThreadIdRef.current
      const preferred = currentTid != null && threadIds.has(currentTid) ? currentTid : threadsSnap.tabs.active_tab
      const resolvedTid = threadIds.has(preferred) ? preferred : threadsSnap.threads[0]?.thread_id ?? null
      store.setActiveThreadId(resolvedTid)

      if (resolvedTid == null) {
        store.setConversation(null)
        return
      }

      try {
        store.setConversation(store.getCachedConversation(wid, resolvedTid))
        const convo = await fetchConversation(wid, resolvedTid)
        store.cacheConversation(convo)
        store.setConversation(convo)
      } catch (err) {
        console.warn("resync fetchConversation failed", err)
      }
    })()
  }, [wsConnected, store])

  const actions = useMemo(
    () =>
      createLubanActions({
        store,
        sendAction: sendActionTransport,
        request: requestTransport,
      }),
    [requestTransport, sendActionTransport, store],
  )

  const serverEventHandler = useMemo(
    () =>
      createLubanServerEventHandler({
        store,
        onToast: (message) => {
          console.warn("server toast:", message)
          toast(message)
        },
        onSelectThreadInWorkspace: (workspaceId, threadId) => {
          void actions.selectThreadInWorkspace(workspaceId, threadId)
        },
      }),
    [actions, store],
  )
  eventHandlerRef.current = serverEventHandler

  useEffect(() => {
    if (app == null) return
    if (activeWorkspaceId != null) return
    const fromApp = app.ui.active_workspace_id ?? null
    const stored = fromApp
    if (!stored || !Number.isFinite(stored)) return
    const existsAndActive = app.projects.some((p) =>
      p.workspaces.some((w) => w.id === stored && w.status === "active"),
    )
    if (!existsAndActive) return
    void actions.openWorkspace(stored)
  }, [actions, activeWorkspaceId, app])

  useEffect(() => {
    if (app == null) return
    if (activeWorkspaceId == null) return

    const locateActiveWorkspace = (): { projectIdx: number; workspaceId: WorkspaceId } | null => {
      for (let i = 0; i < app.projects.length; i++) {
        const project = app.projects[i]!
        const workspace = project.workspaces.find((w) => w.id === activeWorkspaceId) ?? null
        if (workspace) return { projectIdx: i, workspaceId: activeWorkspaceId }
      }
      return null
    }

    const located = locateActiveWorkspace()
    if (activeWorkspace?.status === "active" && located) {
      lastActiveProjectIdxRef.current = located.projectIdx
      return
    }

    const pickMainOrFirstActive = (projectIdx: number): WorkspaceId | null => {
      const project = app.projects[projectIdx] ?? null
      if (!project) return null
      const activeWorkspaces = project.workspaces.filter((w) => w.status === "active")
      if (activeWorkspaces.length === 0) return null
      const main =
        activeWorkspaces.find(
          (w) =>
            w.workspace_name === "main" &&
            normalizePathLike(w.worktree_path) === normalizePathLike(project.path),
        ) ??
        activeWorkspaces.find((w) => w.workspace_name === "main") ??
        activeWorkspaces[0] ??
        null
      return (main?.id as WorkspaceId | undefined) ?? null
    }

    const currentIdx = located?.projectIdx ?? lastActiveProjectIdxRef.current ?? -1

    const fromCurrent =
      currentIdx >= 0 && currentIdx < app.projects.length ? pickMainOrFirstActive(currentIdx) : null

    let fallback: WorkspaceId | null = fromCurrent
    if (fallback == null && app.projects.length > 0) {
      const start = currentIdx >= 0 ? (currentIdx + 1) % app.projects.length : 0
      for (let scanned = 0; scanned < app.projects.length; scanned++) {
        const idx = (start + scanned) % app.projects.length
        const candidate = pickMainOrFirstActive(idx)
        if (candidate != null) {
          fallback = candidate
          break
        }
      }
    }

    if (fallback == null || fallback === activeWorkspaceId || pendingAutoOpenWorkspaceIdRef.current === fallback) {
      return
    }

    pendingAutoOpenWorkspaceIdRef.current = fallback
    void actions.openWorkspace(fallback).finally(() => {
      pendingAutoOpenWorkspaceIdRef.current = null
      focusChatInput()
    })
  }, [actions, activeWorkspace?.status, activeWorkspaceId, app])

  const value: LubanContextValue = {
    app,
    activeWorkspaceId,
    activeWorkspace,
    activeThreadId,
    threads,
    workspaceTabs,
    conversation,
    wsConnected,
    pickProjectPath: actions.pickProjectPath,
    addProject: actions.addProject,
    addProjectAndOpen: actions.addProjectAndOpen,
    deleteProject: actions.deleteProject,
    createWorkspace: actions.createWorkspace,
    ensureMainWorkspace: actions.ensureMainWorkspace,
    openWorkspaceInIde: actions.openWorkspaceInIde,
    openWorkspaceWith: actions.openWorkspaceWith,
    openWorkspacePullRequest: actions.openWorkspacePullRequest,
    openWorkspacePullRequestFailedAction: actions.openWorkspacePullRequestFailedAction,
    archiveWorkspace: actions.archiveWorkspace,
    toggleProjectExpanded: actions.toggleProjectExpanded,
    previewTask: actions.previewTask,
    executeTask: actions.executeTask,
    submitFeedback: actions.submitFeedback,
    openWorkspace: actions.openWorkspace,
    selectThread: actions.selectThread,
    loadConversationBefore: actions.loadConversationBefore,
    createThread: actions.createThread,
    closeThreadTab: actions.closeThreadTab,
    restoreThreadTab: actions.restoreThreadTab,
    sendAgentMessage: actions.sendAgentMessage,
    queueAgentMessage: actions.queueAgentMessage,
    sendAgentMessageTo: actions.sendAgentMessageTo,
    removeQueuedPrompt: actions.removeQueuedPrompt,
    reorderQueuedPrompt: actions.reorderQueuedPrompt,
    updateQueuedPrompt: actions.updateQueuedPrompt,
    cancelAgentTurn: actions.cancelAgentTurn,
    cancelAndSendAgentMessage: actions.cancelAndSendAgentMessage,
    renameWorkspaceBranch: actions.renameWorkspaceBranch,
    aiRenameWorkspaceBranch: actions.aiRenameWorkspaceBranch,
    setChatModel: actions.setChatModel,
    setThinkingEffort: actions.setThinkingEffort,
    setChatRunner: actions.setChatRunner,
    setChatAmpMode: actions.setChatAmpMode,
    setAppearanceTheme: actions.setAppearanceTheme,
    setAppearanceFonts: actions.setAppearanceFonts,
    setGlobalZoom: actions.setGlobalZoom,
    setOpenButtonSelection: actions.setOpenButtonSelection,
    setSidebarProjectOrder: actions.setSidebarProjectOrder,
    setSidebarWorktreeOrder: actions.setSidebarWorktreeOrder,
    setCodexEnabled: actions.setCodexEnabled,
    setAmpEnabled: actions.setAmpEnabled,
    setClaudeEnabled: actions.setClaudeEnabled,
    setAgentRunner: actions.setAgentRunner,
    setAgentAmpMode: actions.setAgentAmpMode,
    setTaskPromptTemplate: actions.setTaskPromptTemplate,
    setSystemPromptTemplate: actions.setSystemPromptTemplate,
    checkCodex: actions.checkCodex,
    getCodexConfigTree: actions.getCodexConfigTree,
    listCodexConfigDir: actions.listCodexConfigDir,
    readCodexConfigFile: actions.readCodexConfigFile,
    writeCodexConfigFile: actions.writeCodexConfigFile,
    checkAmp: actions.checkAmp,
    getAmpConfigTree: actions.getAmpConfigTree,
    listAmpConfigDir: actions.listAmpConfigDir,
    readAmpConfigFile: actions.readAmpConfigFile,
    writeAmpConfigFile: actions.writeAmpConfigFile,
    checkClaude: actions.checkClaude,
    getClaudeConfigTree: actions.getClaudeConfigTree,
    listClaudeConfigDir: actions.listClaudeConfigDir,
    readClaudeConfigFile: actions.readClaudeConfigFile,
    writeClaudeConfigFile: actions.writeClaudeConfigFile,
  }

  return <LubanContext.Provider value={value}>{children}</LubanContext.Provider>
}

export function useLuban(): LubanContextValue {
  const ctx = useContext(LubanContext)
  if (!ctx) throw new Error("useLuban must be used within LubanProvider")
  return ctx
}
