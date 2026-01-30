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
  ContextItemSnapshot,
  ContextSnapshot,
  ConversationEntry,
  ConversationSnapshot,
  FeedbackSubmitResult,
  MentionItemSnapshot,
  ProjectId,
  ServerEvent,
  TaskDraft,
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
  contextItemsByWorkdir: Map<WorkspaceId, ContextItemSnapshot[]>
  attachmentUrlsById: Map<string, string>
  workdirChangesById: Map<WorkspaceId, WorkspaceChangesSnapshot>
  workdirDiffById: Map<WorkspaceId, WorkspaceDiffSnapshot>
  codexCustomPrompts: CodexCustomPromptSnapshot[]
  mentionIndex: MentionItemSnapshot[]
  codexConfig: { tree: CodexConfigEntrySnapshot[]; files: Map<string, string> }
  ampConfig: { tree: AmpConfigEntrySnapshot[]; files: Map<string, string> }
  claudeConfig: { tree: ClaudeConfigEntrySnapshot[]; files: Map<string, string> }
  nextContextId: number
  nextWorkdirId: number
  nextTaskId: number
}

let runtime: RuntimeState | null = null

function clone<T>(value: T): T {
  if (typeof structuredClone === "function") return structuredClone(value)
  return JSON.parse(JSON.stringify(value)) as T
}

function workdirTaskKey(workdirId: WorkspaceId, taskId: WorkspaceThreadId): string {
  return `${workdirId}:${taskId}`
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

function allContextIds(contextByWorkdir: Map<WorkspaceId, ContextItemSnapshot[]>): number[] {
  const out: number[] = []
  for (const items of contextByWorkdir.values()) for (const i of items) out.push(i.context_id)
  return out
}

function initRuntime(): RuntimeState {
  const fixtures = defaultMockFixtures()

  const threadsByWorkdir = new Map<WorkspaceId, ThreadsSnapshot>()
  for (const [k, v] of Object.entries(fixtures.threadsByWorkspace)) threadsByWorkdir.set(Number(k), clone(v))

  const conversationsByWorkdirTask = new Map<string, ConversationSnapshot>()
  for (const [k, v] of Object.entries(fixtures.conversationsByWorkspaceThread)) conversationsByWorkdirTask.set(k, clone(v))

  const contextItemsByWorkdir = new Map<WorkspaceId, ContextItemSnapshot[]>()
  for (const [k, v] of Object.entries(fixtures.contextItemsByWorkspace)) contextItemsByWorkdir.set(Number(k), clone(v))

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

  const nextContextId = Math.max(0, ...allContextIds(contextItemsByWorkdir)) + 1
  const nextWorkdirId = Math.max(0, ...allWorkdirIds(fixtures.app)) + 1
  const nextTaskId = Math.max(0, ...allTaskIds(threadsByWorkdir)) + 1

  return {
    rev: fixtures.app.rev,
    app: clone(fixtures.app),
    threadsByWorkdir,
    starredTasks: new Set<string>(),
    conversationsByWorkdirTask,
    contextItemsByWorkdir,
    attachmentUrlsById,
    workdirChangesById,
    workdirDiffById,
    codexCustomPrompts: clone(fixtures.codexCustomPrompts),
    mentionIndex: clone(fixtures.mentionIndex),
    codexConfig: { tree: clone(fixtures.codexConfig.tree), files: codexFiles },
    ampConfig: { tree: clone(fixtures.ampConfig.tree), files: ampFiles },
    claudeConfig: { tree: clone(fixtures.claudeConfig.tree), files: claudeFiles },
    nextContextId,
    nextWorkdirId,
    nextTaskId,
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
          updated_at_unix_seconds: t.updated_at_unix_seconds,
          branch_name: workdir.branch_name,
          workdir_name: workdir.workdir_name,
          agent_run_status: workdir.agent_run_status,
          has_unread_completion: workdir.has_unread_completion,
          is_starred: state.starredTasks.has(workdirTaskKey(workdir.id, t.task_id)),
        })
      }
    }
  }
  return { rev: state.rev, tasks: clone(tasks) }
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
  const snap = state.conversationsByWorkdirTask.get(workdirTaskKey(workdirId, taskId))
  if (!snap) throw new Error(`mock: unknown conversation: ${workdirId}:${taskId}`)
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

export async function mockFetchContext(workdirId: WorkspaceId): Promise<ContextSnapshot> {
  const state = getRuntime()
  const items = state.contextItemsByWorkdir.get(workdirId) ?? []
  return { workdir_id: workdirId, items: clone(items) }
}

export async function mockDeleteContextItem(workdirId: WorkspaceId, contextId: number): Promise<void> {
  const state = getRuntime()
  const items = state.contextItemsByWorkdir.get(workdirId) ?? []
  state.contextItemsByWorkdir.set(
    workdirId,
    items.filter((i) => i.context_id !== contextId),
  )
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
  snap.tasks = [{ task_id: taskId, remote_thread_id: null, title, updated_at_unix_seconds: now }, ...snap.tasks]
  snap.tabs.open_tabs = [taskId, ...snap.tabs.open_tabs.filter((id) => id !== taskId)]
  snap.tabs.active_tab = taskId
  snap.rev = state.rev

  const convo: ConversationSnapshot = {
    rev: state.rev,
    workdir_id: workdirId,
    task_id: taskId,
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
    in_progress_items: [],
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
    return
  }

  if (a.type === "create_task") {
    const taskId = createTaskInWorkdir(state, a.workdir_id, "New task")
    setActiveWorkdirTask(state, { workdirId: a.workdir_id, taskId })
    emitWorkdirTasksChanged({ state, workdirId: a.workdir_id, onEvent: args.onEvent })
    emitAppChanged({ state, onEvent: args.onEvent })
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
    return
  }

  if (a.type === "workdir_ai_rename_branch") {
    const located = findWorkdir(state.app, a.workdir_id)
    if (located) located.workdir.branch_name = `ai/rename-${a.task_id}`
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "send_agent_message") {
    const key = workdirTaskKey(a.workdir_id, a.task_id)
    const convo = state.conversationsByWorkdirTask.get(key) ?? null
    if (!convo) return
    const next: ConversationEntry[] = [...convo.entries, { type: "user_message", text: a.text, attachments: a.attachments ?? [] }]
    state.conversationsByWorkdirTask.set(key, { ...convo, entries: next, entries_total: next.length })
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
    return
  }

  args.onEvent({ type: "toast", message: `Mock: action not implemented: ${a.type}` })
}

function inferDraft(input: string): TaskDraft {
  const trimmed = input.trim()
  const kind: TaskDraft["intent_kind"] =
    trimmed.toLowerCase().includes("fix") ? "fix" : trimmed.toLowerCase().includes("review") ? "review" : "other"
  return {
    input,
    intent_kind: kind,
    summary: inferTitleFromPrompt(trimmed),
    prompt: trimmed,
    repo: null,
    issue: null,
    pull_request: null,
    project: { type: "local_path", path: "/mock/unknown" },
  }
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

  if (action.type === "task_preview") {
    return inferDraft(action.input) as unknown as T
  }

  if (action.type === "task_execute") {
    if (action.workdir_id == null) throw new Error("mock: task_execute requires workdir_id")
    const workdirId = action.workdir_id
    const title = inferTitleFromPrompt(action.draft.prompt)
    const taskId = createTaskInWorkdir(state, workdirId, title)
    setActiveWorkdirTask(state, { workdirId, taskId })

    const projectId =
      action.draft.project.type === "local_path" ? resolveProjectIdForLocalPath(state, action.draft.project.path) : null
    const workdir = findWorkdir(state.app, workdirId)?.workdir ?? null
    const workdirPath =
      workdir?.workdir_path ??
      (action.draft.project.type === "local_path" ? action.draft.project.path : "/mock")

    const result: TaskExecuteResult = {
      project_id: projectId ?? "mock_project_unknown",
      workdir_id: workdirId,
      task_id: taskId,
      workdir_path: workdirPath,
      prompt: action.draft.prompt,
      mode: action.mode as TaskExecuteMode,
    }

    return clone(result) as unknown as T
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
