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
import { normalizeWorkspaceTabsSnapshot } from "./workspace-tabs"

function normalizePathLike(raw: string): string {
  return raw.trim().replace(/\/+$/, "")
}

type LubanContextValue = {
  app: AppSnapshot | null
  activeWorkdirId: WorkspaceId | null
  activeWorkdir: WorkspaceSnapshot | null
  activeTaskId: number | null
  tasks: ThreadMeta[]
  taskTabs: WorkspaceTabsSnapshot | null
  conversation: ConversationSnapshot | null
  wsConnected: boolean

  pickProjectPath: () => Promise<string | null>
  addProject: (path: string) => void
  addProjectAndOpen: (path: string) => Promise<{ projectId: ProjectId; workdirId: WorkspaceId }>
  deleteProject: (projectId: ProjectId) => void
  createWorkdir: (projectId: ProjectId) => void
  ensureMainWorkdir: (projectId: ProjectId) => void
  openWorkdirInIde: (workdirId: WorkspaceId) => void
  openWorkdirWith: (workdirId: WorkspaceId, target: OpenTarget) => void
  openWorkdirPullRequest: (workdirId: WorkspaceId) => void
  openWorkdirPullRequestFailedAction: (workdirId: WorkspaceId) => void
  archiveWorkdir: (workdirId: number) => void
  toggleProjectExpanded: (projectId: ProjectId) => void

  previewTask: (input: string) => Promise<TaskDraft>
  executeTask: (draft: TaskDraft, mode: TaskExecuteMode, workdirId: WorkspaceId) => Promise<TaskExecuteResult>
  submitFeedback: (args: {
    title: string
    body: string
    labels: string[]
    feedbackType: FeedbackType
    action: FeedbackSubmitAction
  }) => Promise<FeedbackSubmitResult>

  openWorkdir: (workdirId: WorkspaceId) => Promise<void>
  activateTask: (taskId: number) => Promise<void>
  loadConversationBefore: (workdirId: WorkspaceId, taskId: WorkspaceThreadId, before: number) => Promise<void>
  createTask: () => void
  closeTaskTab: (taskId: number) => Promise<void>
  restoreTaskTab: (taskId: number) => Promise<void>

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

  renameWorkdirBranch: (workdirId: WorkspaceId, branchName: string) => void
  aiRenameWorkdirBranch: (workdirId: WorkspaceId, taskId: WorkspaceThreadId) => void

  setChatModel: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, modelId: string) => void
  setThinkingEffort: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, effort: ThinkingEffort) => void
  setChatRunner: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, runner: AgentRunnerKind) => void
  setChatAmpMode: (workspaceId: WorkspaceId, threadId: WorkspaceThreadId, ampMode: string) => void
  setAppearanceTheme: (theme: AppearanceTheme) => void
  setAppearanceFonts: (fonts: AppearanceFontsSnapshot) => void
  setGlobalZoom: (zoom: number) => void
  setOpenButtonSelection: (selection: string) => void
  setSidebarProjectOrder: (projectIds: ProjectId[]) => void

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
  const {
    app,
    activeWorkspaceId: activeWorkdirId,
    activeThreadId: activeTaskId,
    threads: tasks,
    workspaceTabs: taskTabs,
    conversation,
    activeWorkspace: activeWorkdir,
  } = store.state
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

	      store.cacheThreads(wid, threadsSnap.tasks)
	      store.setThreads(threadsSnap.tasks)
	      const normalizedTabs = normalizeWorkspaceTabsSnapshot({ tabs: threadsSnap.tabs, threads: threadsSnap.tasks })
	      store.cacheWorkspaceTabs(wid, normalizedTabs)
	      store.setWorkspaceTabs(normalizedTabs)

	      const taskIds = new Set(threadsSnap.tasks.map((t) => t.task_id))
	      const openTaskIds = (normalizedTabs.open_tabs ?? []).filter((id) => taskIds.has(id))
	      const currentTid = store.refs.activeThreadIdRef.current
	      const currentIsOpen = currentTid != null && openTaskIds.includes(currentTid)
	      const preferredOpen =
	        (openTaskIds.includes(normalizedTabs.active_tab) ? normalizedTabs.active_tab : null) ?? openTaskIds[0] ?? null
	      const resolvedTid = (currentIsOpen ? currentTid : preferredOpen) ?? threadsSnap.tasks[0]?.task_id ?? null
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
    if (activeWorkdirId != null) return
    const fromApp = app.ui.active_workdir_id ?? null
    const stored = fromApp
    if (!stored || !Number.isFinite(stored)) return
    const existsAndActive = app.projects.some((p) =>
      p.workdirs.some((w) => w.id === stored && w.status === "active"),
    )
    if (!existsAndActive) return
    void actions.openWorkdir(stored)
  }, [actions, activeWorkdirId, app])

  useEffect(() => {
    if (app == null) return
    if (activeWorkdirId == null) return

    const locateActiveWorkdir = (): { projectIdx: number; workdirId: WorkspaceId } | null => {
      for (let i = 0; i < app.projects.length; i++) {
        const project = app.projects[i]!
        const workdir = project.workdirs.find((w) => w.id === activeWorkdirId) ?? null
        if (workdir) return { projectIdx: i, workdirId: activeWorkdirId }
      }
      return null
    }

    const located = locateActiveWorkdir()
    if (activeWorkdir?.status === "active" && located) {
      lastActiveProjectIdxRef.current = located.projectIdx
      return
    }

    const pickMainOrFirstActive = (projectIdx: number): WorkspaceId | null => {
      const project = app.projects[projectIdx] ?? null
      if (!project) return null
      const activeWorkdirs = project.workdirs.filter((w) => w.status === "active")
      if (activeWorkdirs.length === 0) return null
      const main =
        activeWorkdirs.find(
          (w) =>
            w.workdir_name === "main" &&
            normalizePathLike(w.workdir_path) === normalizePathLike(project.path),
        ) ??
        activeWorkdirs.find((w) => w.workdir_name === "main") ??
        activeWorkdirs[0] ??
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

    if (fallback == null || fallback === activeWorkdirId || pendingAutoOpenWorkspaceIdRef.current === fallback) {
      return
    }

    pendingAutoOpenWorkspaceIdRef.current = fallback
    void actions.openWorkdir(fallback).finally(() => {
      pendingAutoOpenWorkspaceIdRef.current = null
      focusChatInput()
    })
  }, [actions, activeWorkdir?.status, activeWorkdirId, app])

  const value: LubanContextValue = {
    app,
    activeWorkdirId,
    activeWorkdir,
    activeTaskId,
    tasks,
    taskTabs,
    conversation,
    wsConnected,
    pickProjectPath: actions.pickProjectPath,
    addProject: actions.addProject,
    addProjectAndOpen: actions.addProjectAndOpen,
    deleteProject: actions.deleteProject,
    createWorkdir: actions.createWorkdir,
    ensureMainWorkdir: actions.ensureMainWorkdir,
    openWorkdirInIde: actions.openWorkdirInIde,
    openWorkdirWith: actions.openWorkdirWith,
    openWorkdirPullRequest: actions.openWorkdirPullRequest,
    openWorkdirPullRequestFailedAction: actions.openWorkdirPullRequestFailedAction,
    archiveWorkdir: actions.archiveWorkdir,
    toggleProjectExpanded: actions.toggleProjectExpanded,
    previewTask: actions.previewTask,
    executeTask: actions.executeTask,
    submitFeedback: actions.submitFeedback,
    openWorkdir: actions.openWorkdir,
    activateTask: actions.activateTask,
    loadConversationBefore: actions.loadConversationBefore,
    createTask: actions.createTask,
    closeTaskTab: actions.closeTaskTab,
    restoreTaskTab: actions.restoreTaskTab,
    sendAgentMessage: actions.sendAgentMessage,
    queueAgentMessage: actions.queueAgentMessage,
    sendAgentMessageTo: actions.sendAgentMessageTo,
    removeQueuedPrompt: actions.removeQueuedPrompt,
    reorderQueuedPrompt: actions.reorderQueuedPrompt,
    updateQueuedPrompt: actions.updateQueuedPrompt,
    cancelAgentTurn: actions.cancelAgentTurn,
    cancelAndSendAgentMessage: actions.cancelAndSendAgentMessage,
    renameWorkdirBranch: actions.renameWorkdirBranch,
    aiRenameWorkdirBranch: actions.aiRenameWorkdirBranch,
    setChatModel: actions.setChatModel,
    setThinkingEffort: actions.setThinkingEffort,
    setChatRunner: actions.setChatRunner,
    setChatAmpMode: actions.setChatAmpMode,
    setAppearanceTheme: actions.setAppearanceTheme,
    setAppearanceFonts: actions.setAppearanceFonts,
    setGlobalZoom: actions.setGlobalZoom,
    setOpenButtonSelection: actions.setOpenButtonSelection,
    setSidebarProjectOrder: actions.setSidebarProjectOrder,
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
