"use client"

import type {
  AmpConfigEntrySnapshot,
  AppSnapshot,
  AttachmentKind,
  AttachmentRef,
  ClientAction,
  ClaudeConfigEntrySnapshot,
  CodexConfigEntrySnapshot,
  CodexCustomPromptSnapshot,
  ConversationEntry,
  ConversationSnapshot,
  FeedbackSubmitResult,
  MentionItemSnapshot,
  NewTaskDraftSnapshot,
  NewTaskDraftsSnapshot,
  NewTaskStashResponse,
  NewTaskStashSnapshot,
  ProjectId,
  ServerEvent,
  TaskExecuteMode,
  TaskExecuteResult,
  TasksSnapshot,
  TaskSummarySnapshot,
  ThreadsSnapshot,
  WorkspaceChangesSnapshot,
  WorkspaceDiffSnapshot,
  WorkspaceId,
  WorkspaceSnapshot,
  WorkspaceThreadId,
  WorkspaceTabsSnapshot,
} from "../luban-api"
import { defaultMockFixtures } from "./fixtures"

type RuntimeState = {
  rev: number
  app: AppSnapshot
  threadsByWorkdir: Map<WorkspaceId, ThreadsSnapshot>
  starredTasks: Set<string>
  conversationsByWorkdirTask: Map<string, ConversationSnapshot>
  attachmentUrlsById: Map<string, string>
  workdirChangesById: Map<WorkspaceId, WorkspaceChangesSnapshot>
  workdirDiffById: Map<WorkspaceId, WorkspaceDiffSnapshot>
  codexCustomPrompts: CodexCustomPromptSnapshot[]
  mentionIndex: MentionItemSnapshot[]
  codexConfig: { tree: CodexConfigEntrySnapshot[]; files: Map<string, string> }
  ampConfig: { tree: AmpConfigEntrySnapshot[]; files: Map<string, string> }
  claudeConfig: { tree: ClaudeConfigEntrySnapshot[]; files: Map<string, string> }
  nextWorkdirId: number
  nextTaskId: number
  newTaskDrafts: NewTaskDraftSnapshot[]
  newTaskStash: NewTaskStashSnapshot | null
}

let runtime: RuntimeState | null = null

function clone<T>(value: T): T {
  if (typeof structuredClone === "function") return structuredClone(value)
  return JSON.parse(JSON.stringify(value)) as T
}

function workdirTaskKey(workdirId: WorkspaceId, taskId: WorkspaceThreadId): string {
  return `${workdirId}:${taskId}`
}

function newEntryId(prefix: string): string {
  return `${prefix}_${Math.random().toString(16).slice(2)}`
}

function bumpRev(state: RuntimeState): number {
  state.rev += 1
  state.app.rev = state.rev
  return state.rev
}

function allWorkdirIds(app: AppSnapshot): WorkspaceId[] {
  const out: WorkspaceId[] = []
  for (const p of app.projects) for (const w of p.workdirs) out.push(w.id)
  return out
}

function allTaskIds(threadsByWorkdir: Map<WorkspaceId, ThreadsSnapshot>): WorkspaceThreadId[] {
  const out: WorkspaceThreadId[] = []
  for (const snap of threadsByWorkdir.values()) for (const t of snap.tasks) out.push(t.task_id)
  return out
}

function initRuntime(): RuntimeState {
  const fixtures = defaultMockFixtures()

  const threadsByWorkdir = new Map<WorkspaceId, ThreadsSnapshot>()
  for (const [k, v] of Object.entries(fixtures.threadsByWorkspace)) threadsByWorkdir.set(Number(k), clone(v))

  const conversationsByWorkdirTask = new Map<string, ConversationSnapshot>()
  for (const [k, v] of Object.entries(fixtures.conversationsByWorkspaceThread)) conversationsByWorkdirTask.set(k, clone(v))

  const attachmentUrlsById = new Map<string, string>()
  for (const [k, v] of Object.entries(fixtures.attachmentUrlsById)) attachmentUrlsById.set(k, v)

  const workdirChangesById = new Map<WorkspaceId, WorkspaceChangesSnapshot>()
  for (const [k, v] of Object.entries(fixtures.workspaceChangesByWorkspace)) workdirChangesById.set(Number(k), clone(v))

  const workdirDiffById = new Map<WorkspaceId, WorkspaceDiffSnapshot>()
  for (const [k, v] of Object.entries(fixtures.workspaceDiffByWorkspace)) workdirDiffById.set(Number(k), clone(v))

  const codexFiles = new Map<string, string>()
  for (const [k, v] of Object.entries(fixtures.codexConfig.files)) codexFiles.set(k, v)
  const ampFiles = new Map<string, string>()
  for (const [k, v] of Object.entries(fixtures.ampConfig.files)) ampFiles.set(k, v)
  const claudeFiles = new Map<string, string>()
  for (const [k, v] of Object.entries(fixtures.claudeConfig.files)) claudeFiles.set(k, v)

  const nextWorkdirId = Math.max(0, ...allWorkdirIds(fixtures.app)) + 1
  const nextTaskId = Math.max(0, ...allTaskIds(threadsByWorkdir)) + 1

  return {
    rev: fixtures.app.rev,
    app: clone(fixtures.app),
    threadsByWorkdir,
    starredTasks: new Set<string>(),
    conversationsByWorkdirTask,
    attachmentUrlsById,
    workdirChangesById,
    workdirDiffById,
    codexCustomPrompts: clone(fixtures.codexCustomPrompts),
    mentionIndex: clone(fixtures.mentionIndex),
    codexConfig: { tree: clone(fixtures.codexConfig.tree), files: codexFiles },
    ampConfig: { tree: clone(fixtures.ampConfig.tree), files: ampFiles },
    claudeConfig: { tree: clone(fixtures.claudeConfig.tree), files: claudeFiles },
    nextWorkdirId,
    nextTaskId,
    newTaskDrafts: [],
    newTaskStash: null,
  }
}

function getRuntime(): RuntimeState {
  if (!runtime) runtime = initRuntime()
  return runtime
}

function findProject(app: AppSnapshot, projectId: ProjectId): { projectIdx: number; project: AppSnapshot["projects"][number] } | null {
  const idx = app.projects.findIndex((p) => p.id === projectId)
  if (idx < 0) return null
  const project = app.projects[idx]
  if (!project) return null
  return { projectIdx: idx, project }
}

function findWorkdir(app: AppSnapshot, workdirId: WorkspaceId): { projectId: ProjectId; workdir: WorkspaceSnapshot } | null {
  for (const p of app.projects) {
    const w = p.workdirs.find((x) => x.id === workdirId) ?? null
    if (w) return { projectId: p.id, workdir: w }
  }
  return null
}

function emitAppChanged(args: { state: RuntimeState; onEvent: (event: ServerEvent) => void }) {
  const rev = bumpRev(args.state)
  args.onEvent({ type: "app_changed", rev, snapshot: clone(args.state.app) })
}

function emitWorkdirTasksChanged(args: { state: RuntimeState; workdirId: WorkspaceId; onEvent: (event: ServerEvent) => void }) {
  const snap = args.state.threadsByWorkdir.get(args.workdirId) ?? null
  if (!snap) return
  args.onEvent({ type: "workdir_tasks_changed", workdir_id: args.workdirId, tabs: clone(snap.tabs), tasks: clone(snap.tasks) })
}

function emitTaskSummariesChanged(args: { state: RuntimeState; workdirId: WorkspaceId; onEvent: (event: ServerEvent) => void }) {
  const located = findWorkdir(args.state.app, args.workdirId) ?? null
  if (!located) return
  const snap = args.state.threadsByWorkdir.get(args.workdirId) ?? null
  if (!snap) return

  const tasks: TaskSummarySnapshot[] = snap.tasks.map((t) => ({
    project_id: located.projectId,
    workdir_id: args.workdirId,
    task_id: t.task_id,
    title: t.title,
    created_at_unix_seconds: t.created_at_unix_seconds,
    updated_at_unix_seconds: t.updated_at_unix_seconds,
    branch_name: located.workdir.branch_name,
    workdir_name: located.workdir.workdir_name,
    agent_run_status: located.workdir.agent_run_status,
    has_unread_completion: located.workdir.has_unread_completion,
    task_status: t.task_status,
    turn_status: t.turn_status,
    last_turn_result: t.last_turn_result,
    is_starred: args.state.starredTasks.has(workdirTaskKey(args.workdirId, t.task_id)),
  }))

  args.onEvent({ type: "task_summaries_changed", project_id: located.projectId, workdir_id: args.workdirId, tasks: clone(tasks) })
}

function emitConversationChanged(args: { state: RuntimeState; workdirId: WorkspaceId; taskId: WorkspaceThreadId; onEvent: (event: ServerEvent) => void }) {
  const snap = args.state.conversationsByWorkdirTask.get(workdirTaskKey(args.workdirId, args.taskId)) ?? null
  if (!snap) return
  args.onEvent({ type: "conversation_changed", snapshot: clone(snap) })
}

export function mockAttachmentUrl(attachmentId: string): string | null {
  return getRuntime().attachmentUrlsById.get(attachmentId) ?? null
}

export async function mockFetchApp(): Promise<AppSnapshot> {
  return clone(getRuntime().app)
}

export async function mockFetchTasks(args: { projectId?: string } = {}): Promise<TasksSnapshot> {
  const state = getRuntime()
  const tasks: TaskSummarySnapshot[] = []
  for (const project of state.app.projects) {
    if (args.projectId && project.id !== args.projectId) continue
    for (const workdir of project.workdirs) {
      const snap = state.threadsByWorkdir.get(workdir.id) ?? null
      if (!snap) continue
      for (const t of snap.tasks) {
        tasks.push({
          project_id: project.id,
          workdir_id: workdir.id,
          task_id: t.task_id,
          title: t.title,
          created_at_unix_seconds: t.created_at_unix_seconds,
          updated_at_unix_seconds: t.updated_at_unix_seconds,
          branch_name: workdir.branch_name,
          workdir_name: workdir.workdir_name,
          agent_run_status: workdir.agent_run_status,
          has_unread_completion: workdir.has_unread_completion,
          task_status: t.task_status,
          turn_status: t.turn_status,
          last_turn_result: t.last_turn_result,
          is_starred: state.starredTasks.has(workdirTaskKey(workdir.id, t.task_id)),
        })
      }
    }
  }
  return { rev: state.rev, tasks: clone(tasks) }
}

export async function mockFetchNewTaskDrafts(): Promise<NewTaskDraftsSnapshot> {
  const state = getRuntime()
  return { drafts: clone(state.newTaskDrafts) }
}

export async function mockCreateNewTaskDraft(args: {
  text: string
  project_id: string | null
  workdir_id: number | null
}): Promise<NewTaskDraftSnapshot> {
  if (args.workdir_id != null && (!Number.isInteger(args.workdir_id) || args.workdir_id < 0)) {
    throw new Error(`mock: invalid workdir_id: ${args.workdir_id}`)
  }
  const state = getRuntime()
  const now = Date.now()
  const draft: NewTaskDraftSnapshot = {
    id: `draft_${Math.random().toString(16).slice(2)}_${now.toString(16)}`,
    text: args.text,
    project_id: args.project_id,
    workdir_id: args.workdir_id,
    created_at_unix_ms: now,
    updated_at_unix_ms: now,
  }
  state.newTaskDrafts.unshift(draft)
  return clone(draft)
}

export async function mockUpdateNewTaskDraft(
  draftId: string,
  args: { text: string; project_id: string | null; workdir_id: number | null },
): Promise<NewTaskDraftSnapshot> {
  if (args.workdir_id != null && (!Number.isInteger(args.workdir_id) || args.workdir_id < 0)) {
    throw new Error(`mock: invalid workdir_id: ${args.workdir_id}`)
  }
  const state = getRuntime()
  const idx = state.newTaskDrafts.findIndex((d) => d.id === draftId)
  if (idx < 0) throw new Error(`mock: unknown draft id: ${draftId}`)
  const prev = state.newTaskDrafts[idx]!
  const now = Date.now()
  const next: NewTaskDraftSnapshot = {
    ...prev,
    text: args.text,
    project_id: args.project_id,
    workdir_id: args.workdir_id,
    updated_at_unix_ms: now,
  }
  state.newTaskDrafts[idx] = next
  state.newTaskDrafts.sort((a, b) => b.updated_at_unix_ms - a.updated_at_unix_ms)
  return clone(next)
}

export async function mockDeleteNewTaskDraft(draftId: string): Promise<void> {
  const state = getRuntime()
  state.newTaskDrafts = state.newTaskDrafts.filter((d) => d.id !== draftId)
}

export async function mockFetchNewTaskStash(): Promise<NewTaskStashResponse> {
  const state = getRuntime()
  return { stash: state.newTaskStash ? clone(state.newTaskStash) : null }
}

export async function mockSaveNewTaskStash(args: {
  text: string
  project_id: string | null
  workdir_id: number | null
  editing_draft_id: string | null
}): Promise<void> {
  if (args.workdir_id != null && (!Number.isInteger(args.workdir_id) || args.workdir_id < 0)) {
    throw new Error(`mock: invalid workdir_id: ${args.workdir_id}`)
  }
  const state = getRuntime()
  state.newTaskStash = {
    text: args.text,
    project_id: args.project_id,
    workdir_id: args.workdir_id,
    editing_draft_id: args.editing_draft_id,
    updated_at_unix_ms: Date.now(),
  }
}

export async function mockClearNewTaskStash(): Promise<void> {
  const state = getRuntime()
  state.newTaskStash = null
}

export async function mockFetchThreads(workdirId: WorkspaceId): Promise<ThreadsSnapshot> {
  const state = getRuntime()
  const snap = state.threadsByWorkdir.get(workdirId)
  if (!snap) throw new Error(`mock: unknown workdir_id: ${workdirId}`)
  return clone(snap)
}

export async function mockFetchConversation(
  workdirId: WorkspaceId,
  taskId: WorkspaceThreadId,
  _args: { before?: number; limit?: number } = {},
): Promise<ConversationSnapshot> {
  const state = getRuntime()
  const key = workdirTaskKey(workdirId, taskId)
  let snap = state.conversationsByWorkdirTask.get(key)
  if (!snap) throw new Error(`mock: unknown conversation: ${workdirId}:${taskId}`)
  if (snap.run_status === "running" && snap.run_started_at_unix_ms == null) {
    const runStartedAtUnixMs = Date.now() - 5 * 60_000
    snap = { ...snap, run_started_at_unix_ms: runStartedAtUnixMs }
    state.conversationsByWorkdirTask.set(key, snap)
  }
  return clone(snap)
}

export async function mockFetchWorkspaceChanges(workdirId: WorkspaceId): Promise<WorkspaceChangesSnapshot> {
  const state = getRuntime()
  return clone(state.workdirChangesById.get(workdirId) ?? { workdir_id: workdirId, files: [] })
}

export async function mockFetchWorkspaceDiff(workdirId: WorkspaceId): Promise<WorkspaceDiffSnapshot> {
  const state = getRuntime()
  return clone(state.workdirDiffById.get(workdirId) ?? { workdir_id: workdirId, files: [] })
}

export async function mockFetchCodexCustomPrompts(): Promise<CodexCustomPromptSnapshot[]> {
  return clone(getRuntime().codexCustomPrompts)
}

export async function mockFetchMentionItems(args: { workspaceId: WorkspaceId; query: string }): Promise<MentionItemSnapshot[]> {
  const q = args.query.trim().toLowerCase()
  if (!q) return []
  const items = getRuntime().mentionIndex
  return items.filter((i) => i.name.toLowerCase().includes(q) || i.path.toLowerCase().includes(q)).slice(0, 20)
}

function attachmentFromFile(args: { file: File; kind: AttachmentKind }): AttachmentRef {
  const name = args.file.name || "file"
  const lastDot = name.lastIndexOf(".")
  const extension = lastDot >= 0 ? name.slice(lastDot + 1) : ""
  return {
    id: `mock_att_${Math.random().toString(16).slice(2)}`,
    kind: args.kind,
    name,
    extension,
    mime: args.file.type || null,
    byte_len: args.file.size,
  }
}

export async function mockUploadAttachment(args: { workspaceId: number; file: File; kind: AttachmentKind }): Promise<AttachmentRef> {
  const state = getRuntime()
  const att = attachmentFromFile({ file: args.file, kind: args.kind })
  state.attachmentUrlsById.set(att.id, URL.createObjectURL(args.file))
  return clone(att)
}

function ensureThreadsSnapshot(state: RuntimeState, workdirId: WorkspaceId): ThreadsSnapshot {
  const existing = state.threadsByWorkdir.get(workdirId) ?? null
  if (existing) return existing
  const snap: ThreadsSnapshot = { rev: state.rev, workdir_id: workdirId, tabs: { open_tabs: [], archived_tabs: [], active_tab: 1 }, tasks: [] }
  state.threadsByWorkdir.set(workdirId, snap)
  return snap
}

function createTaskInWorkdir(state: RuntimeState, workdirId: WorkspaceId, title: string): WorkspaceThreadId {
  const snap = ensureThreadsSnapshot(state, workdirId)
  const taskId: WorkspaceThreadId = state.nextTaskId
  state.nextTaskId += 1
  const now = Math.floor(Date.now() / 1000)
  snap.tasks = [{
    task_id: taskId,
    remote_thread_id: null,
    title,
    created_at_unix_seconds: now,
    updated_at_unix_seconds: now,
    task_status: "backlog",
    turn_status: "idle",
    last_turn_result: null,
  }, ...snap.tasks]
  snap.tabs.open_tabs = [taskId, ...snap.tabs.open_tabs.filter((id) => id !== taskId)]
  snap.tabs.active_tab = taskId
  snap.rev = state.rev

		  const convo: ConversationSnapshot = {
	    rev: state.rev,
	    workdir_id: workdirId,
	    task_id: taskId,
	    task_status: "backlog",
	    agent_runner: "codex",
	    agent_model_id: state.app.agent.default_model_id ?? "gpt-5",
	    thinking_effort: state.app.agent.default_thinking_effort ?? "medium",
	    amp_mode: null,
	    run_status: "idle",
	    run_started_at_unix_ms: null,
	    run_finished_at_unix_ms: null,
	    entries: [],
		    entries_total: 0,
		    entries_start: 0,
		    entries_truncated: false,
		    pending_prompts: [],
		    queue_paused: false,
		    remote_thread_id: null,
		    title,
		  }
  state.conversationsByWorkdirTask.set(workdirTaskKey(workdirId, taskId), convo)
  return taskId
}

function setActiveWorkdirTask(state: RuntimeState, args: { workdirId: WorkspaceId; taskId: WorkspaceThreadId | null }) {
  state.app.ui.active_workdir_id = args.workdirId
  if (args.taskId == null) {
    delete state.app.ui.active_task_id
  } else {
    state.app.ui.active_task_id = args.taskId
  }
}

export function mockDispatchAction(args: { action: ClientAction; onEvent: (event: ServerEvent) => void }): void {
  const state = getRuntime()
  const a = args.action

  if (a.type === "telegram_bot_token_set") {
    const prev = state.app.integrations?.telegram ?? { enabled: false, has_token: false, config_rev: 0 }
    state.app.integrations = {
      ...(state.app.integrations ?? { telegram: prev }),
      telegram: {
        ...prev,
        enabled: true,
        has_token: true,
        bot_username: "mock_bot",
        config_rev: (prev.config_rev ?? 0) + 1,
        last_error: undefined,
      },
    }
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "telegram_bot_token_clear") {
    const prev = state.app.integrations?.telegram ?? { enabled: false, has_token: false, config_rev: 0 }
    state.app.integrations = {
      ...(state.app.integrations ?? { telegram: prev }),
      telegram: {
        ...prev,
        enabled: false,
        has_token: false,
        bot_username: undefined,
        paired_chat_id: undefined,
        config_rev: (prev.config_rev ?? 0) + 1,
        last_error: undefined,
      },
    }
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "telegram_unpair") {
    const prev = state.app.integrations?.telegram ?? { enabled: false, has_token: false, config_rev: 0 }
    state.app.integrations = {
      ...(state.app.integrations ?? { telegram: prev }),
      telegram: {
        ...prev,
        paired_chat_id: undefined,
        config_rev: (prev.config_rev ?? 0) + 1,
      },
    }
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "add_project") {
    const projectId: ProjectId = `mock_project_${Math.random().toString(16).slice(2)}`
    state.app.projects.push({
      id: projectId,
      name: a.path.split("/").slice(-1)[0] || "Project",
      slug: projectId,
      path: a.path,
      is_git: true,
      expanded: true,
      create_workdir_status: "idle",
      workdirs: [],
    })
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "delete_project") {
    state.app.projects = state.app.projects.filter((p) => p.id !== a.project_id)
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "toggle_project_expanded") {
    state.app.projects = state.app.projects.map((p) => (p.id === a.project_id ? { ...p, expanded: !p.expanded } : p))
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "create_workdir") {
    const found = findProject(state.app, a.project_id)
    if (!found) return
    found.project.create_workdir_status = "running"
    emitAppChanged({ state, onEvent: args.onEvent })

    const projectId = a.project_id
    const onEvent = args.onEvent
    window.setTimeout(() => {
      const state = getRuntime()
      let found = findProject(state.app, projectId)
      if (!found) return

      let workdirId: WorkspaceId = state.nextWorkdirId
      state.nextWorkdirId += 1
      found.project.workdirs.push({
        id: workdirId,
        short_id: `W${workdirId}`,
        workdir_name: `workdir-${workdirId}`,
        branch_name: state.app.projects[found.projectIdx]?.is_git ? `task/${workdirId}` : "",
        workdir_path: `${found.project.path}-workdir-${workdirId}`,
        status: "active",
        archive_status: "idle",
        branch_rename_status: "idle",
        agent_run_status: "idle",
        has_unread_completion: false,
        pull_request: null,
      })
      ensureThreadsSnapshot(state, workdirId)
      found = findProject(state.app, projectId)
      if (found) found.project.create_workdir_status = "idle"

      emitAppChanged({ state, onEvent })
    }, 600)
    return
  }

  if (a.type === "ensure_main_workdir") {
    const found = findProject(state.app, a.project_id)
    if (!found) return
    const exists = found.project.workdirs.some((w) => w.workdir_name === "main" && w.status === "active")
    if (exists) return
    const workdirId: WorkspaceId = state.nextWorkdirId
    state.nextWorkdirId += 1
    found.project.workdirs.push({
      id: workdirId,
      short_id: `W${workdirId}`,
      workdir_name: "main",
      branch_name: found.project.is_git ? "main" : "",
      workdir_path: found.project.path,
      status: "active",
      archive_status: "idle",
      branch_rename_status: "idle",
      agent_run_status: "idle",
      has_unread_completion: false,
      pull_request: null,
    })
    ensureThreadsSnapshot(state, workdirId)
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "open_workdir") {
    setActiveWorkdirTask(state, { workdirId: a.workdir_id, taskId: null })
    const threads = state.threadsByWorkdir.get(a.workdir_id) ?? null
    const active = threads?.tabs.active_tab ?? null
    if (active != null && threads?.tasks.some((t) => t.task_id === active)) {
      setActiveWorkdirTask(state, { workdirId: a.workdir_id, taskId: active })
    }
    const workdir = findWorkdir(state.app, a.workdir_id)?.workdir ?? null
    if (workdir) {
      workdir.has_unread_completion = false
    }
    emitAppChanged({ state, onEvent: args.onEvent })
    emitTaskSummariesChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "create_task") {
    const taskId = createTaskInWorkdir(state, a.workdir_id, "New task")
    setActiveWorkdirTask(state, { workdirId: a.workdir_id, taskId })
    emitWorkdirTasksChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    emitAppChanged({ state, onEvent: args.onEvent })
    emitTaskSummariesChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "activate_task") {
    setActiveWorkdirTask(state, { workdirId: a.workdir_id, taskId: a.task_id })
    emitAppChanged({ state, onEvent: args.onEvent })
    emitConversationChanged({ state, workdirId: a.workdir_id, taskId: a.task_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "close_task_tab" || a.type === "restore_task_tab") {
    const snap = ensureThreadsSnapshot(state, a.workdir_id)
    const id = a.task_id
    if (a.type === "close_task_tab") {
      snap.tabs.open_tabs = snap.tabs.open_tabs.filter((x) => x !== id)
      if (!snap.tabs.archived_tabs.includes(id)) snap.tabs.archived_tabs = [id, ...snap.tabs.archived_tabs]
    } else {
      snap.tabs.archived_tabs = snap.tabs.archived_tabs.filter((x) => x !== id)
      if (!snap.tabs.open_tabs.includes(id)) snap.tabs.open_tabs = [id, ...snap.tabs.open_tabs]
    }
    if (!snap.tabs.open_tabs.includes(snap.tabs.active_tab)) {
      snap.tabs.active_tab = snap.tabs.open_tabs[0] ?? snap.tabs.active_tab
    }
    emitWorkdirTasksChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    emitTaskSummariesChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "open_workdir_in_ide" || a.type === "open_workdir_with" || a.type === "open_workdir_pull_request" || a.type === "open_workdir_pull_request_failed_action") {
    args.onEvent({ type: "toast", message: `Mock: ${a.type}` })
    return
  }

  if (a.type === "open_button_selection_changed") {
    state.app.ui.open_button_selection = a.selection
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "sidebar_project_order_changed") {
    state.app.ui.sidebar_project_order = a.project_ids
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "appearance_theme_changed") {
    state.app.appearance.theme = a.theme
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "appearance_fonts_changed") {
    state.app.appearance.fonts = a.fonts
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "appearance_global_zoom_changed") {
    state.app.appearance.global_zoom = a.zoom
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "workdir_rename_branch") {
    const located = findWorkdir(state.app, a.workdir_id)
    if (located) located.workdir.branch_name = a.branch_name
    emitAppChanged({ state, onEvent: args.onEvent })
    emitTaskSummariesChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "workdir_ai_rename_branch") {
    const located = findWorkdir(state.app, a.workdir_id)
    if (located) located.workdir.branch_name = `ai/rename-${a.task_id}`
    emitAppChanged({ state, onEvent: args.onEvent })
    emitTaskSummariesChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "send_agent_message") {
    const key = workdirTaskKey(a.workdir_id, a.task_id)
    const convo = state.conversationsByWorkdirTask.get(key) ?? null
    if (!convo) return
    const threads = state.threadsByWorkdir.get(a.workdir_id) ?? null
    if (threads) {
      const now = Math.floor(Date.now() / 1000)
      const bumpedNow =
        Math.max(
          now,
          ...threads.tasks.map((t) => t.updated_at_unix_seconds),
        ) + 1
      threads.tasks = threads.tasks.map((t) =>
        t.task_id === a.task_id
          ? { ...t, updated_at_unix_seconds: bumpedNow }
          : t,
      )
    }
    const next: ConversationEntry[] = [
      ...convo.entries,
      {
        type: "user_event",
        entry_id: newEntryId("ue"),
        event: { type: "message", text: a.text, attachments: a.attachments ?? [] },
      },
    ]
    const rev = bumpRev(state)
    if (threads) threads.rev = rev
    state.conversationsByWorkdirTask.set(key, { ...convo, entries: next, entries_total: next.length, rev })
    args.onEvent({ type: "app_changed", rev, snapshot: clone(state.app) })
    emitWorkdirTasksChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    emitTaskSummariesChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    emitConversationChanged({ state, workdirId: a.workdir_id, taskId: a.task_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "terminal_command_start") {
    const key = workdirTaskKey(a.workdir_id, a.task_id)
    const convo = state.conversationsByWorkdirTask.get(key) ?? null
    if (!convo) return

    const command = a.command.trim()
    if (!command) return

    const commandId = newEntryId("cmd")
    const reconnect = newEntryId("reconnect")

    const startedEntry: ConversationEntry = {
      type: "user_event",
      entry_id: newEntryId("ue"),
      event: { type: "terminal_command_started", id: commandId, command, reconnect },
    }

    const startedEntries = [...convo.entries, startedEntry]
    const rev = bumpRev(state)
    state.conversationsByWorkdirTask.set(key, { ...convo, entries: startedEntries, entries_total: startedEntries.length, rev })
    emitConversationChanged({ state, workdirId: a.workdir_id, taskId: a.task_id, onEvent: args.onEvent })

    const workdirId = a.workdir_id
    const taskId = a.task_id
    const onEvent = args.onEvent
    window.setTimeout(() => {
      const state = getRuntime()
      const key = workdirTaskKey(workdirId, taskId)
      const convo = state.conversationsByWorkdirTask.get(key) ?? null
      if (!convo) return

      const noOutput = /^(?:true|:|sleep)(?:\\s|$)/.test(command)
      const outputText = noOutput ? "" : `mock: ${command}\\r\\n`
      const outputBase64 = outputText ? btoa(outputText) : ""
      const outputByteLen = outputText ? outputText.length : 0

      const finishedEntry: ConversationEntry = {
        type: "user_event",
        entry_id: newEntryId("ue"),
        event: {
          type: "terminal_command_finished",
          id: commandId,
          command,
          reconnect,
          output_base64: outputBase64,
          output_byte_len: outputByteLen,
        },
      }

      const nextEntries = [...convo.entries, finishedEntry]
      const rev = bumpRev(state)
      state.conversationsByWorkdirTask.set(key, { ...convo, entries: nextEntries, entries_total: nextEntries.length, rev })
      emitConversationChanged({ state, workdirId, taskId, onEvent })
    }, 250)
    return
  }

  if (a.type === "queue_agent_message") {
    const key = workdirTaskKey(a.workdir_id, a.task_id)
    const convo = state.conversationsByWorkdirTask.get(key) ?? null
    if (!convo) return

    const runner = a.runner ?? convo.agent_runner
    const ampMode = runner === "amp" ? a.amp_mode ?? convo.amp_mode ?? "default" : null
    const runConfig = {
      runner,
      model_id: convo.agent_model_id,
      thinking_effort: convo.thinking_effort,
      amp_mode: ampMode,
    }

    const nextId = Math.max(0, ...(convo.pending_prompts ?? []).map((p) => p.id)) + 1
    const pending = [
      ...(convo.pending_prompts ?? []),
      { id: nextId, text: a.text, attachments: a.attachments ?? [], run_config: runConfig },
    ]
    state.conversationsByWorkdirTask.set(key, { ...convo, pending_prompts: pending })
    emitConversationChanged({ state, workdirId: a.workdir_id, taskId: a.task_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "remove_queued_prompt") {
    const key = workdirTaskKey(a.workdir_id, a.task_id)
    const convo = state.conversationsByWorkdirTask.get(key) ?? null
    if (!convo) return
    const pending = (convo.pending_prompts ?? []).filter((p) => p.id !== a.prompt_id)
    state.conversationsByWorkdirTask.set(key, { ...convo, pending_prompts: pending })
    emitConversationChanged({ state, workdirId: a.workdir_id, taskId: a.task_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "reorder_queued_prompt") {
    const key = workdirTaskKey(a.workdir_id, a.task_id)
    const convo = state.conversationsByWorkdirTask.get(key) ?? null
    if (!convo) return
    const pending = [...(convo.pending_prompts ?? [])]
    const from = pending.findIndex((p) => p.id === a.active_id)
    const to = pending.findIndex((p) => p.id === a.over_id)
    if (from < 0 || to < 0 || from === to) return
    const [item] = pending.splice(from, 1)
    if (!item) return
    pending.splice(to, 0, item)
    state.conversationsByWorkdirTask.set(key, { ...convo, pending_prompts: pending })
    emitConversationChanged({ state, workdirId: a.workdir_id, taskId: a.task_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "update_queued_prompt") {
    const key = workdirTaskKey(a.workdir_id, a.task_id)
    const convo = state.conversationsByWorkdirTask.get(key) ?? null
    if (!convo) return
    const pending = (convo.pending_prompts ?? []).map((p) => {
      if (p.id !== a.prompt_id) return p
      return {
        ...p,
        text: a.text,
        attachments: a.attachments ?? [],
        run_config: {
          ...p.run_config,
          model_id: a.model_id ?? p.run_config.model_id,
          thinking_effort: a.thinking_effort ?? p.run_config.thinking_effort,
        },
      }
    })
    state.conversationsByWorkdirTask.set(key, { ...convo, pending_prompts: pending })
    emitConversationChanged({ state, workdirId: a.workdir_id, taskId: a.task_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "task_status_set") {
    const snap = state.threadsByWorkdir.get(a.workdir_id) ?? null
    if (!snap) return

    const now = Math.floor(Date.now() / 1000)
    const bumpedNow =
      Math.max(
        now,
        ...snap.tasks.map((t) => t.updated_at_unix_seconds),
      ) + 1
    snap.tasks = snap.tasks.map((t) =>
      t.task_id === a.task_id
        ? {
            ...t,
            task_status: a.task_status,
            updated_at_unix_seconds: bumpedNow,
            turn_status: a.task_status === "canceled" ? "idle" : t.turn_status,
            last_turn_result: a.task_status === "canceled" ? "failed" : t.last_turn_result,
          }
        : t,
    )

    const key = workdirTaskKey(a.workdir_id, a.task_id)
    const convo = state.conversationsByWorkdirTask.get(key) ?? null
    if (convo) {
      const shouldCancel = a.task_status === "canceled" && convo.run_status === "running"
      const nextEntries = (() => {
        if (!shouldCancel) return convo.entries
        const alreadyCanceled = convo.entries.some((e) => e.type === "agent_event" && e.event?.type === "turn_canceled")
        if (alreadyCanceled) return convo.entries
        return [
          ...convo.entries,
          {
            type: "agent_event",
            entry_id: newEntryId("ae"),
            event: { type: "turn_canceled" },
          },
        ] satisfies ConversationEntry[]
      })()

      state.conversationsByWorkdirTask.set(key, {
        ...convo,
        task_status: a.task_status,
        run_status: shouldCancel ? "idle" : convo.run_status,
        run_finished_at_unix_ms: shouldCancel ? Date.now() : convo.run_finished_at_unix_ms,
        queue_paused: shouldCancel ? true : convo.queue_paused,
        entries: nextEntries,
      })
    }

    const shouldArchive =
      (a.task_status === "done" || a.task_status === "canceled") &&
      snap.tasks.every((t) => t.task_status === "done" || t.task_status === "canceled")
    if (shouldArchive) {
      state.app.projects = state.app.projects.map((p) => ({
        ...p,
        workdirs: p.workdirs.map((w) => (w.id === a.workdir_id ? { ...w, status: "archived" } : w)),
      }))

      const updatedConvo = state.conversationsByWorkdirTask.get(key) ?? null
      if (updatedConvo) {
        const alreadyArchived = updatedConvo.entries.some(
          (e) => e.type === "system_event" && e.event?.event_type === "task_archived",
        )
        if (!alreadyArchived) {
          state.conversationsByWorkdirTask.set(key, {
            ...updatedConvo,
            entries: [
              ...updatedConvo.entries,
              {
                type: "system_event",
                entry_id: newEntryId("se"),
                created_at_unix_ms: Date.now(),
                event: { event_type: "task_archived" },
              },
            ],
          })
        }
      }
    }

    const rev = bumpRev(state)
    snap.rev = rev
    if (convo) {
      const updated = state.conversationsByWorkdirTask.get(key) ?? null
      if (updated) state.conversationsByWorkdirTask.set(key, { ...updated, rev })
    }

    args.onEvent({ type: "app_changed", rev, snapshot: clone(state.app) })
    emitWorkdirTasksChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    emitTaskSummariesChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    emitConversationChanged({ state, workdirId: a.workdir_id, taskId: a.task_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "task_star_set") {
    const key = workdirTaskKey(a.workdir_id, a.task_id)
    if (a.starred) {
      state.starredTasks.add(key)
    } else {
      state.starredTasks.delete(key)
    }
    emitAppChanged({ state, onEvent: args.onEvent })
    emitTaskSummariesChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    return
  }

  args.onEvent({ type: "toast", message: `Mock: action not implemented: ${a.type}` })
}

function inferTitleFromPrompt(prompt: string): string {
  const trimmed = prompt.trim()
  if (!trimmed) return "New task"
  const first = trimmed.split("\n")[0] ?? trimmed
  const clipped = first.length > 60 ? `${first.slice(0, 57)}...` : first
  return clipped
}

function resolveProjectIdForLocalPath(state: RuntimeState, path: string): ProjectId | null {
  const normalized = path.trim().replace(/\/+$/, "")
  const found = state.app.projects.find((p) => p.path.replace(/\/+$/, "") === normalized) ?? null
  return found?.id ?? null
}

function createProjectFromPath(state: RuntimeState, path: string): ProjectId {
  const projectId: ProjectId = `mock_project_${Math.random().toString(16).slice(2)}`
  state.app.projects.push({
    id: projectId,
    name: path.split("/").slice(-1)[0] || "Project",
    slug: projectId,
    path,
    is_git: true,
    expanded: true,
    create_workdir_status: "idle",
    workdirs: [],
  })
  return projectId
}

function ensureMainWorkdir(state: RuntimeState, projectId: ProjectId): WorkspaceId {
  const found = findProject(state.app, projectId)
  if (!found) throw new Error(`mock: project not found: ${projectId}`)
  const existing = found.project.workdirs.find((w) => w.workdir_name === "main" && w.status === "active") ?? null
  if (existing) return existing.id
  const workdirId: WorkspaceId = state.nextWorkdirId
  state.nextWorkdirId += 1
  found.project.workdirs.push({
    id: workdirId,
    short_id: `W${workdirId}`,
    workdir_name: "main",
    branch_name: found.project.is_git ? "main" : "",
    workdir_path: found.project.path,
    status: "active",
    archive_status: "idle",
    branch_rename_status: "idle",
    agent_run_status: "idle",
    has_unread_completion: false,
    pull_request: null,
  })
  ensureThreadsSnapshot(state, workdirId)
  return workdirId
}

export async function mockRequest<T>(action: ClientAction): Promise<T> {
  const state = getRuntime()

  if (action.type === "pick_project_path") {
    const value = window.prompt("Enter a project path (mock):", "/mock/new/project")
    return (value && value.trim().length > 0 ? value.trim() : null) as T
  }

  if (action.type === "add_project_and_open") {
    const projectId = createProjectFromPath(state, action.path)
    const workdirId = ensureMainWorkdir(state, projectId)
    setActiveWorkdirTask(state, { workdirId, taskId: null })
    return { projectId, workdirId } as unknown as T
  }

  if (action.type === "task_execute") {
    if (action.workdir_id == null) throw new Error("mock: task_execute requires workdir_id")
    const workdirId = action.workdir_id
    const title = inferTitleFromPrompt(action.prompt)
    const taskId = createTaskInWorkdir(state, workdirId, title)
    setActiveWorkdirTask(state, { workdirId, taskId })

    const workdir = findWorkdir(state.app, workdirId)?.workdir ?? null
    const workdirPath = workdir?.workdir_path ?? "/mock"

    const result: TaskExecuteResult = {
      project_id: "mock_project_unknown",
      workdir_id: workdirId,
      task_id: taskId,
      workdir_path: workdirPath,
      prompt: action.prompt,
      mode: action.mode as TaskExecuteMode,
    }

    return clone(result) as unknown as T
  }

  if (action.type === "telegram_pair_start") {
    const username = state.app.integrations?.telegram?.bot_username ?? "mock_bot"
    return `https://t.me/${username}?start=mock_pairing_code` as unknown as T
  }

  if (action.type === "feedback_submit") {
    const issue = { number: 1, title: action.title, url: "https://example.invalid/issue/1" }
    if (action.action !== "fix_it") {
      const result: FeedbackSubmitResult = { issue, task: null }
      return result as unknown as T
    }

    const workdirId = state.app.ui.active_workdir_id ?? allWorkdirIds(state.app)[0] ?? 1
    const taskId = createTaskInWorkdir(state, workdirId, inferTitleFromPrompt(action.title))
    const workdir = findWorkdir(state.app, workdirId)?.workdir ?? null
    const result: FeedbackSubmitResult = {
      issue,
      task: {
        project_id: "mock_project_unknown",
        workdir_id: workdirId,
        task_id: taskId,
        workdir_path: workdir?.workdir_path ?? "/mock",
        prompt: action.body,
        mode: "start",
      },
    }
    return clone(result) as unknown as T
  }

  if (action.type === "codex_check" || action.type === "amp_check" || action.type === "claude_check") {
    return { ok: true, message: "Mock check ok" } as T
  }

  if (action.type === "codex_config_tree") return clone(state.codexConfig.tree) as unknown as T
  if (action.type === "codex_config_list_dir") return { path: action.path, entries: [] } as unknown as T
  if (action.type === "codex_config_read_file") return (state.codexConfig.files.get(action.path) ?? "") as unknown as T
  if (action.type === "codex_config_write_file") {
    state.codexConfig.files.set(action.path, action.contents)
    return null as unknown as T
  }

  if (action.type === "amp_config_tree") return clone(state.ampConfig.tree) as unknown as T
  if (action.type === "amp_config_list_dir") return { path: action.path, entries: [] } as unknown as T
  if (action.type === "amp_config_read_file") return (state.ampConfig.files.get(action.path) ?? "") as unknown as T
  if (action.type === "amp_config_write_file") {
    state.ampConfig.files.set(action.path, action.contents)
    return null as unknown as T
  }

  if (action.type === "claude_config_tree") return clone(state.claudeConfig.tree) as unknown as T
  if (action.type === "claude_config_list_dir") return { path: action.path, entries: [] } as unknown as T
  if (action.type === "claude_config_read_file") return (state.claudeConfig.files.get(action.path) ?? "") as unknown as T
  if (action.type === "claude_config_write_file") {
    state.claudeConfig.files.set(action.path, action.contents)
    return null as unknown as T
  }

  throw new Error(`mock: request not implemented: ${action.type}`)
}
