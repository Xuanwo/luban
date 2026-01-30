"use client"

import type {
  AgentRunConfigSnapshot,
  AgentRunnerKind,
  AmpConfigEntrySnapshot,
  ClaudeConfigEntrySnapshot,
  AppearanceFontsSnapshot,
  AppearanceTheme,
  AttachmentRef,
  ClientAction,
  CodexConfigEntrySnapshot,
  FeedbackSubmitAction,
  FeedbackSubmitResult,
  FeedbackType,
  OpenTarget,
  ProjectId,
  SystemTaskKind,
  TaskIntentKind,
  TaskDraft,
  TaskExecuteMode,
  TaskExecuteResult,
  ThinkingEffort,
  WorkspaceId,
  WorkspaceThreadId,
} from "./luban-api"
import { fetchConversation, fetchThreads } from "./luban-http"
import type { LubanStore } from "./luban-store"
import { waitForNewThread } from "./luban-thread-flow"
import { prependConversationSnapshot } from "./conversation-pagination"
import { pickThreadId } from "./thread-ui"
import { normalizeWorkspaceTabsSnapshot } from "./workspace-tabs"

export type LubanActions = {
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
    workdirId: WorkspaceId,
    taskId: number,
    text: string,
    attachments?: AttachmentRef[],
    runConfig?: { runner?: AgentRunnerKind | null; amp_mode?: string | null },
  ) => void
  removeQueuedPrompt: (workspaceId: WorkspaceId, taskId: WorkspaceThreadId, promptId: number) => void
  reorderQueuedPrompt: (
    workspaceId: WorkspaceId,
    taskId: WorkspaceThreadId,
    activeId: number,
    overId: number,
  ) => void
  updateQueuedPrompt: (
    workspaceId: WorkspaceId,
    taskId: WorkspaceThreadId,
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

  setChatModel: (workdirId: WorkspaceId, taskId: WorkspaceThreadId, modelId: string) => void
  setThinkingEffort: (workdirId: WorkspaceId, taskId: WorkspaceThreadId, effort: ThinkingEffort) => void
  setChatRunner: (workdirId: WorkspaceId, taskId: WorkspaceThreadId, runner: AgentRunnerKind) => void
  setChatAmpMode: (workdirId: WorkspaceId, taskId: WorkspaceThreadId, ampMode: string) => void
  setAppearanceTheme: (theme: AppearanceTheme) => void
  setAppearanceFonts: (fonts: AppearanceFontsSnapshot) => void
  setGlobalZoom: (zoom: number) => void
  setOpenButtonSelection: (selection: string) => void
  setSidebarProjectOrder: (projectIds: ProjectId[]) => void
}

export type LubanActionsInternal = LubanActions & {
  selectThreadInWorkspace: (workspaceId: WorkspaceId, threadId: number) => Promise<void>
}

export function createLubanActions(args: {
  store: LubanStore
  sendAction: (action: ClientAction, requestId?: string) => void
  request: <T>(action: ClientAction) => Promise<T>
}): LubanActionsInternal {
  const store = args.store

  function addProject(path: string) {
    args.sendAction({ type: "add_project", path })
  }

  function addProjectAndOpen(path: string): Promise<{ projectId: ProjectId; workdirId: WorkspaceId }> {
    return args.request<{ projectId: ProjectId; workdirId: WorkspaceId }>({ type: "add_project_and_open", path })
  }

  function deleteProject(projectId: ProjectId) {
    args.sendAction({ type: "delete_project", project_id: projectId })
  }

  function pickProjectPath(): Promise<string | null> {
    return args.request<string | null>({ type: "pick_project_path" })
  }

  function createWorkdir(projectId: ProjectId) {
    args.sendAction({ type: "create_workdir", project_id: projectId })
  }

  function ensureMainWorkdir(projectId: ProjectId) {
    args.sendAction({ type: "ensure_main_workdir", project_id: projectId })
  }

  function openWorkdirInIde(workdirId: WorkspaceId) {
    args.sendAction({ type: "open_workdir_in_ide", workdir_id: workdirId })
  }

  function openWorkdirWith(workdirId: WorkspaceId, target: OpenTarget) {
    args.sendAction({ type: "open_workdir_with", workdir_id: workdirId, target })
  }

  function openWorkdirPullRequest(workdirId: WorkspaceId) {
    args.sendAction({ type: "open_workdir_pull_request", workdir_id: workdirId })
  }

  function openWorkdirPullRequestFailedAction(workdirId: WorkspaceId) {
    args.sendAction({ type: "open_workdir_pull_request_failed_action", workdir_id: workdirId })
  }

  function archiveWorkdir(workdirId: number) {
    args.sendAction({ type: "archive_workdir", workdir_id: workdirId })
  }

  function toggleProjectExpanded(projectId: ProjectId) {
    store.setApp((prev) => {
      if (!prev) return prev
      return {
        ...prev,
        projects: prev.projects.map((p) => (p.id === projectId ? { ...p, expanded: !p.expanded } : p)),
      }
    })
    args.sendAction({ type: "toggle_project_expanded", project_id: projectId })
  }

  function setCodexEnabled(enabled: boolean) {
    args.sendAction({ type: "codex_enabled_changed", enabled })
  }

  function setAmpEnabled(enabled: boolean) {
    args.sendAction({ type: "amp_enabled_changed", enabled })
  }

  function setClaudeEnabled(enabled: boolean) {
    args.sendAction({ type: "claude_enabled_changed", enabled })
  }

  function setAgentRunner(runner: AgentRunnerKind) {
    args.sendAction({ type: "agent_runner_changed", runner })
  }

  function setAgentAmpMode(mode: string) {
    args.sendAction({ type: "agent_amp_mode_changed", mode })
  }

  function setTaskPromptTemplate(intentKind: TaskIntentKind, template: string) {
    const trimmed = template.trim()
    if (!trimmed) return

    store.setApp((prev) => {
      if (!prev) return prev
      const task = prev.task ?? {
        prompt_templates: [],
        default_prompt_templates: [],
        system_prompt_templates: [],
        default_system_prompt_templates: [],
      }
      const existing = task.prompt_templates
      const nextTemplates = [...existing]
      const idx = nextTemplates.findIndex((t) => t.intent_kind === intentKind)
      if (idx >= 0) nextTemplates[idx] = { intent_kind: intentKind, template: trimmed }
      else nextTemplates.push({ intent_kind: intentKind, template: trimmed })
      return {
        ...prev,
        task: {
          ...task,
          prompt_templates: nextTemplates,
        },
      }
    })

    args.sendAction({ type: "task_prompt_template_changed", intent_kind: intentKind, template: trimmed })
  }

  function setSystemPromptTemplate(kind: SystemTaskKind, template: string) {
    const trimmed = template.trim()
    if (!trimmed) return

    store.setApp((prev) => {
      if (!prev) return prev
      const task = prev.task ?? {
        prompt_templates: [],
        default_prompt_templates: [],
        system_prompt_templates: [],
        default_system_prompt_templates: [],
      }
      const existing = task.system_prompt_templates
      const nextTemplates = [...existing]
      const idx = nextTemplates.findIndex((t) => t.kind === kind)
      if (idx >= 0) nextTemplates[idx] = { kind, template: trimmed }
      else nextTemplates.push({ kind, template: trimmed })
      return {
        ...prev,
        task: {
          ...task,
          system_prompt_templates: nextTemplates,
        },
      }
    })

    args.sendAction({ type: "system_prompt_template_changed", kind, template: trimmed })
  }

  function checkCodex(): Promise<{ ok: boolean; message: string | null }> {
    return args.request<{ ok: boolean; message: string | null }>({ type: "codex_check" })
  }

  function getCodexConfigTree(): Promise<CodexConfigEntrySnapshot[]> {
    return args.request<CodexConfigEntrySnapshot[]>({ type: "codex_config_tree" })
  }

  function listCodexConfigDir(path: string): Promise<{ path: string; entries: CodexConfigEntrySnapshot[] }> {
    return args.request<{ path: string; entries: CodexConfigEntrySnapshot[] }>({
      type: "codex_config_list_dir",
      path,
    })
  }

  function readCodexConfigFile(path: string): Promise<string> {
    return args.request<string>({ type: "codex_config_read_file", path })
  }

  async function writeCodexConfigFile(path: string, contents: string): Promise<void> {
    await args.request<null>({ type: "codex_config_write_file", path, contents })
  }

  function checkAmp(): Promise<{ ok: boolean; message: string | null }> {
    return args.request<{ ok: boolean; message: string | null }>({ type: "amp_check" })
  }

  function getAmpConfigTree(): Promise<AmpConfigEntrySnapshot[]> {
    return args.request<AmpConfigEntrySnapshot[]>({ type: "amp_config_tree" })
  }

  function listAmpConfigDir(path: string): Promise<{ path: string; entries: AmpConfigEntrySnapshot[] }> {
    return args.request<{ path: string; entries: AmpConfigEntrySnapshot[] }>({
      type: "amp_config_list_dir",
      path,
    })
  }

  function readAmpConfigFile(path: string): Promise<string> {
    return args.request<string>({ type: "amp_config_read_file", path })
  }

  async function writeAmpConfigFile(path: string, contents: string): Promise<void> {
    await args.request<null>({ type: "amp_config_write_file", path, contents })
  }

  function checkClaude(): Promise<{ ok: boolean; message: string | null }> {
    return args.request<{ ok: boolean; message: string | null }>({ type: "claude_check" })
  }

  function getClaudeConfigTree(): Promise<ClaudeConfigEntrySnapshot[]> {
    return args.request<ClaudeConfigEntrySnapshot[]>({ type: "claude_config_tree" })
  }

  function listClaudeConfigDir(path: string): Promise<{ path: string; entries: ClaudeConfigEntrySnapshot[] }> {
    return args.request<{ path: string; entries: ClaudeConfigEntrySnapshot[] }>({
      type: "claude_config_list_dir",
      path,
    })
  }

  function readClaudeConfigFile(path: string): Promise<string> {
    return args.request<string>({ type: "claude_config_read_file", path })
  }

  async function writeClaudeConfigFile(path: string, contents: string): Promise<void> {
    await args.request<null>({ type: "claude_config_write_file", path, contents })
  }

  function previewTask(input: string): Promise<TaskDraft> {
    return args.request<TaskDraft>({ type: "task_preview", input })
  }

  function executeTask(draft: TaskDraft, mode: TaskExecuteMode, workdirId: WorkspaceId): Promise<TaskExecuteResult> {
    return args.request<TaskExecuteResult>({ type: "task_execute", draft, mode, workdir_id: workdirId })
  }

  function submitFeedback(args2: {
    title: string
    body: string
    labels: string[]
    feedbackType: FeedbackType
    action: FeedbackSubmitAction
  }): Promise<FeedbackSubmitResult> {
    return args.request<FeedbackSubmitResult>({
      type: "feedback_submit",
      title: args2.title,
      body: args2.body,
      labels: args2.labels,
      feedback_type: args2.feedbackType,
      action: args2.action,
    })
  }

  async function selectThreadInWorkspace(workspaceId: WorkspaceId, threadId: number) {
    store.setActiveThreadId(threadId)
    store.setConversation(store.getCachedConversation(workspaceId, threadId))

    args.sendAction({
      type: "activate_task",
      workdir_id: workspaceId,
      task_id: threadId,
    })

    try {
      const convo = await fetchConversation(workspaceId, threadId)
      store.cacheConversation(convo)
      store.setConversation(convo)
    } catch (err) {
      console.warn("fetchConversation failed", err)
    }
  }

  async function refreshThreads(workspaceId: WorkspaceId): Promise<{
    threadId: number | null
  }> {
    const snap = await fetchThreads(workspaceId)
    store.cacheThreads(workspaceId, snap.tasks)
    store.setThreads(snap.tasks)
    const normalizedTabs = normalizeWorkspaceTabsSnapshot({ tabs: snap.tabs, threads: snap.tasks })
    store.cacheWorkspaceTabs(workspaceId, normalizedTabs)
    store.setWorkspaceTabs(normalizedTabs)

    const threadIds = new Set(snap.tasks.map((t) => t.task_id))
    const openThreadIds = (normalizedTabs.open_tabs ?? []).filter((id) => threadIds.has(id))
    const current = store.refs.activeThreadIdRef.current
    const currentIsOpen = current != null && openThreadIds.includes(current)
    const preferredOpen = (openThreadIds.includes(normalizedTabs.active_tab) ? normalizedTabs.active_tab : null) ?? openThreadIds[0] ?? null
    const initial =
      (currentIsOpen ? current : preferredOpen) ??
      pickThreadId({
        threads: snap.tasks,
        preferredThreadId: normalizedTabs.active_tab,
      })

    store.setActiveThreadId(initial)
    if (initial == null) {
      store.setConversation(null)
      return { threadId: null }
    }

    try {
      store.setConversation(store.getCachedConversation(workspaceId, initial))
      const convo = await fetchConversation(workspaceId, initial)
      store.cacheConversation(convo)
      store.setConversation(convo)
    } catch (err) {
      console.warn("fetchConversation failed", err)
    }

    return { threadId: initial }
  }

  async function waitAndActivateNewThread(args2: {
    workspaceId: WorkspaceId
    existingThreadIds: Set<number>
  }): Promise<boolean> {
    const created = await waitForNewThread({
      workspaceId: args2.workspaceId,
      existingThreadIds: args2.existingThreadIds,
      fetchThreads,
    })
    if (created.createdThreadId == null) return false

    store.refs.pendingCreateThreadRef.current = null
    store.setThreads(created.threads)
    store.setWorkspaceTabs(normalizeWorkspaceTabsSnapshot({ tabs: created.tabs, threads: created.threads }))
    await selectThreadInWorkspace(args2.workspaceId, created.createdThreadId)
    return true
  }

  async function openWorkdir(workdirId: WorkspaceId) {
    store.setActiveWorkspaceId(workdirId)

    const cachedThreads = store.getCachedThreads(workdirId)
    store.setThreads(cachedThreads ?? [])

    const cachedTabs = store.getCachedWorkspaceTabs(workdirId)
    store.setWorkspaceTabs(cachedTabs)

    const initialFromCache = pickThreadId({
      threads: cachedThreads ?? [],
      preferredThreadId: cachedTabs?.active_tab ?? null,
    })
    store.setActiveThreadId(initialFromCache)
    if (initialFromCache != null) {
      store.setConversation(store.getCachedConversation(workdirId, initialFromCache))
    } else {
      store.setConversation(null)
    }

    args.sendAction({ type: "open_workdir", workdir_id: workdirId })

    try {
      const snap = await fetchThreads(workdirId)
      store.cacheThreads(workdirId, snap.tasks)
      store.setThreads(snap.tasks)
      const normalizedTabs = normalizeWorkspaceTabsSnapshot({ tabs: snap.tabs, threads: snap.tasks })
      store.cacheWorkspaceTabs(workdirId, normalizedTabs)
      store.setWorkspaceTabs(normalizedTabs)

      const initial = pickThreadId({
        threads: snap.tasks,
        preferredThreadId: snap.tabs.active_tab,
      })

      if (initial == null) {
        const existing = new Set<number>()
        store.markPendingCreateThread({ workspaceId: workdirId, existingThreadIds: existing })
        args.sendAction({ type: "create_task", workdir_id: workdirId })
        store.setActiveThreadId(null)

        await waitAndActivateNewThread({ workspaceId: workdirId, existingThreadIds: existing })
        return
      }

      store.setActiveThreadId(initial)
      if (initial !== snap.tabs.active_tab) {
        args.sendAction({
          type: "activate_task",
          workdir_id: workdirId,
          task_id: initial,
        })
      }
      try {
        store.setConversation(store.getCachedConversation(workdirId, initial))
        const convo = await fetchConversation(workdirId, initial)
        store.cacheConversation(convo)
        store.setConversation(convo)
      } catch (err) {
        console.warn("fetchConversation failed", err)
      }
    } catch (err) {
      console.warn("fetchThreads failed", err)
    }
  }

  async function activateTask(taskId: number) {
    const wid = store.refs.activeWorkspaceIdRef.current
    if (wid == null) return
    await selectThreadInWorkspace(wid, taskId)
  }

  async function loadConversationBefore(workdirId: WorkspaceId, taskId: WorkspaceThreadId, before: number) {
    try {
      const convo = await fetchConversation(workdirId, taskId, { before })
      const wid = store.refs.activeWorkspaceIdRef.current
      const tid = store.refs.activeThreadIdRef.current
      if (wid !== workdirId || tid !== taskId) return
      store.setConversation((prev) => {
        if (!prev) return convo
        return prependConversationSnapshot(prev, convo)
      })
    } catch (err) {
      console.warn("fetchConversation failed", err)
    }
  }

  function createTask() {
    const wid = store.refs.activeWorkspaceIdRef.current
    if (wid == null) return

    void (async () => {
      const existingThreadIds = new Set(store.refs.threadsRef.current.map((t) => t.task_id))
      store.markPendingCreateThread({ workspaceId: wid, existingThreadIds })
      args.sendAction({ type: "open_workdir", workdir_id: wid })
      args.sendAction({ type: "create_task", workdir_id: wid })

      await waitAndActivateNewThread({ workspaceId: wid, existingThreadIds })
    })()
  }

  async function closeTaskTab(taskId: number) {
    const wid = store.refs.activeWorkspaceIdRef.current
    if (wid == null) return
    args.sendAction({ type: "close_task_tab", workdir_id: wid, task_id: taskId })
    try {
      await refreshThreads(wid)
    } catch (err) {
      console.warn("fetchThreads failed", err)
    }
  }

  async function restoreTaskTab(taskId: number) {
    const wid = store.refs.activeWorkspaceIdRef.current
    if (wid == null) return
    args.sendAction({ type: "restore_task_tab", workdir_id: wid, task_id: taskId })
    try {
      await refreshThreads(wid)
    } catch (err) {
      console.warn("fetchThreads failed", err)
    }
  }

  function activeWorkspaceThread(): { workspaceId: WorkspaceId; threadId: number } | null {
    const wid = store.refs.activeWorkspaceIdRef.current
    const tid = store.refs.activeThreadIdRef.current
    if (wid == null || tid == null) return null
    return { workspaceId: wid, threadId: tid }
  }

  function sendAgentMessage(
    text: string,
    attachments: AttachmentRef[] = [],
    runConfig?: { runner?: AgentRunnerKind | null; amp_mode?: string | null },
  ) {
    const ids = activeWorkspaceThread()
    if (!ids) return
    args.sendAction({
      type: "send_agent_message",
      workdir_id: ids.workspaceId,
      task_id: ids.threadId,
      text,
      attachments,
      ...(runConfig?.runner != null ? { runner: runConfig.runner } : {}),
      ...(runConfig?.amp_mode != null ? { amp_mode: runConfig.amp_mode } : {}),
    })
  }

  function queueAgentMessage(
    text: string,
    attachments: AttachmentRef[] = [],
    runConfig?: { runner?: AgentRunnerKind | null; amp_mode?: string | null },
  ) {
    const ids = activeWorkspaceThread()
    if (!ids) return
    args.sendAction({
      type: "queue_agent_message",
      workdir_id: ids.workspaceId,
      task_id: ids.threadId,
      text,
      attachments,
      ...(runConfig?.runner != null ? { runner: runConfig.runner } : {}),
      ...(runConfig?.amp_mode != null ? { amp_mode: runConfig.amp_mode } : {}),
    })
  }

  function sendAgentMessageTo(
    workdirId: WorkspaceId,
    taskId: number,
    text: string,
    attachments: AttachmentRef[] = [],
    runConfig?: { runner?: AgentRunnerKind | null; amp_mode?: string | null },
  ) {
    args.sendAction({
      type: "send_agent_message",
      workdir_id: workdirId,
      task_id: taskId,
      text,
      attachments,
      ...(runConfig?.runner != null ? { runner: runConfig.runner } : {}),
      ...(runConfig?.amp_mode != null ? { amp_mode: runConfig.amp_mode } : {}),
    })
  }

  function removeQueuedPrompt(workspaceId: WorkspaceId, threadId: WorkspaceThreadId, promptId: number) {
    store.setConversation((prev) => {
      if (!prev) return prev
      if (prev.workdir_id !== workspaceId || prev.task_id !== threadId) return prev
      return { ...prev, pending_prompts: prev.pending_prompts.filter((p) => p.id !== promptId) }
    })
    args.sendAction({ type: "remove_queued_prompt", workdir_id: workspaceId, task_id: threadId, prompt_id: promptId })
  }

  function reorderQueuedPrompt(
    workspaceId: WorkspaceId,
    threadId: WorkspaceThreadId,
    activeId: number,
    overId: number,
  ) {
    store.setConversation((prev) => {
      if (!prev) return prev
      if (prev.workdir_id !== workspaceId || prev.task_id !== threadId) return prev
      const from = prev.pending_prompts.findIndex((p) => p.id === activeId)
      const to = prev.pending_prompts.findIndex((p) => p.id === overId)
      if (from < 0 || to < 0 || from === to) return prev
      const next = [...prev.pending_prompts]
      const [item] = next.splice(from, 1)
      if (!item) return prev
      next.splice(to, 0, item)
      return { ...prev, pending_prompts: next }
    })
    args.sendAction({
      type: "reorder_queued_prompt",
      workdir_id: workspaceId,
      task_id: threadId,
      active_id: activeId,
      over_id: overId,
    })
  }

  function updateQueuedPrompt(
    workspaceId: WorkspaceId,
    threadId: WorkspaceThreadId,
    promptId: number,
    args2: { text: string; attachments: AttachmentRef[]; runConfig: AgentRunConfigSnapshot },
  ) {
    const text = args2.text.trim()
    const attachments = args2.attachments
    const runConfig = args2.runConfig
    store.setConversation((prev) => {
      if (!prev) return prev
      if (prev.workdir_id !== workspaceId || prev.task_id !== threadId) return prev
      const next = prev.pending_prompts.map((p) =>
        p.id === promptId ? { ...p, text, attachments, run_config: runConfig } : p,
      )
      return { ...prev, pending_prompts: next }
    })
    args.sendAction({
      type: "update_queued_prompt",
      workdir_id: workspaceId,
      task_id: threadId,
      prompt_id: promptId,
      text,
      attachments,
      model_id: runConfig.model_id,
      thinking_effort: runConfig.thinking_effort,
    })
  }

  function cancelAgentTurn() {
    const ids = activeWorkspaceThread()
    if (!ids) return
    args.sendAction({ type: "cancel_agent_turn", workdir_id: ids.workspaceId, task_id: ids.threadId })
  }

  function cancelAndSendAgentMessage(
    text: string,
    attachments: AttachmentRef[] = [],
    runConfig?: { runner?: AgentRunnerKind | null; amp_mode?: string | null },
  ) {
    const ids = activeWorkspaceThread()
    if (!ids) return
    args.sendAction({
      type: "cancel_and_send_agent_message",
      workdir_id: ids.workspaceId,
      task_id: ids.threadId,
      text,
      attachments,
      ...(runConfig?.runner != null ? { runner: runConfig.runner } : {}),
      ...(runConfig?.amp_mode != null ? { amp_mode: runConfig.amp_mode } : {}),
    })
  }

  function renameWorkdirBranch(workdirId: WorkspaceId, branchName: string) {
    args.sendAction({ type: "workdir_rename_branch", workdir_id: workdirId, branch_name: branchName })
  }

  function aiRenameWorkdirBranch(workdirId: WorkspaceId, taskId: WorkspaceThreadId) {
    args.sendAction({ type: "workdir_ai_rename_branch", workdir_id: workdirId, task_id: taskId })
  }

  function setChatModel(workdirId: WorkspaceId, taskId: WorkspaceThreadId, modelId: string) {
    args.sendAction({
      type: "chat_model_changed",
      workdir_id: workdirId,
      task_id: taskId,
      model_id: modelId,
    })
  }

  function setThinkingEffort(
    workdirId: WorkspaceId,
    taskId: WorkspaceThreadId,
    effort: ThinkingEffort,
  ) {
    args.sendAction({
      type: "thinking_effort_changed",
      workdir_id: workdirId,
      task_id: taskId,
      thinking_effort: effort,
    })
  }

  function setChatRunner(workdirId: WorkspaceId, taskId: WorkspaceThreadId, runner: AgentRunnerKind) {
    store.setConversation((prev) => {
      if (!prev) return prev
      if (prev.workdir_id !== workdirId || prev.task_id !== taskId) return prev
      const nextAmpMode =
        runner === "amp" ? (prev.amp_mode ?? store.state.app?.agent.amp_mode ?? null) : null
      return {
        ...prev,
        agent_runner: runner,
        amp_mode: nextAmpMode,
      }
    })
    args.sendAction({ type: "chat_runner_changed", workdir_id: workdirId, task_id: taskId, runner })
  }

  function setChatAmpMode(workdirId: WorkspaceId, taskId: WorkspaceThreadId, ampMode: string) {
    const trimmed = ampMode.trim()
    if (!trimmed) return
    store.setConversation((prev) => {
      if (!prev) return prev
      if (prev.workdir_id !== workdirId || prev.task_id !== taskId) return prev
      if (prev.agent_runner !== "amp") return prev
      return { ...prev, amp_mode: trimmed }
    })
    args.sendAction({ type: "chat_amp_mode_changed", workdir_id: workdirId, task_id: taskId, amp_mode: trimmed })
  }

  function setAppearanceTheme(theme: AppearanceTheme) {
    args.sendAction({ type: "appearance_theme_changed", theme })
  }

  function setAppearanceFonts(fonts: AppearanceFontsSnapshot) {
    args.sendAction({ type: "appearance_fonts_changed", fonts })
  }

  function setGlobalZoom(zoom: number) {
    args.sendAction({ type: "appearance_global_zoom_changed", zoom })
  }

  function setOpenButtonSelection(selection: string) {
    args.sendAction({ type: "open_button_selection_changed", selection })
  }

  function setSidebarProjectOrder(projectIds: ProjectId[]) {
    store.setApp((prev) => {
      if (!prev) return prev
      return {
        ...prev,
        ui: {
          ...prev.ui,
          sidebar_project_order: projectIds,
        },
      }
    })
    args.sendAction({ type: "sidebar_project_order_changed", project_ids: projectIds })
  }

  return {
    pickProjectPath,
    addProject,
    addProjectAndOpen,
    deleteProject,
    createWorkdir,
    ensureMainWorkdir,
    openWorkdirInIde,
    openWorkdirWith,
    openWorkdirPullRequest,
    openWorkdirPullRequestFailedAction,
    archiveWorkdir,
    toggleProjectExpanded,
    setCodexEnabled,
    setAmpEnabled,
    setClaudeEnabled,
    setAgentRunner,
    setAgentAmpMode,
    setTaskPromptTemplate,
    setSystemPromptTemplate,
    checkCodex,
    getCodexConfigTree,
    listCodexConfigDir,
    readCodexConfigFile,
    writeCodexConfigFile,
    checkAmp,
    getAmpConfigTree,
    listAmpConfigDir,
    readAmpConfigFile,
    writeAmpConfigFile,
    checkClaude,
    getClaudeConfigTree,
    listClaudeConfigDir,
    readClaudeConfigFile,
    writeClaudeConfigFile,
    previewTask,
    executeTask,
    submitFeedback,
    openWorkdir,
    activateTask,
    loadConversationBefore,
    selectThreadInWorkspace,
    createTask,
    closeTaskTab,
    restoreTaskTab,
    sendAgentMessage,
    queueAgentMessage,
    sendAgentMessageTo,
    removeQueuedPrompt,
    reorderQueuedPrompt,
    updateQueuedPrompt,
    cancelAgentTurn,
    cancelAndSendAgentMessage,
    renameWorkdirBranch,
    aiRenameWorkdirBranch,
    setChatModel,
    setThinkingEffort,
    setChatRunner,
    setChatAmpMode,
    setAppearanceTheme,
    setAppearanceFonts,
    setGlobalZoom,
    setOpenButtonSelection,
    setSidebarProjectOrder,
  }
}
