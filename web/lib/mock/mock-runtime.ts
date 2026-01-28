"use client"

import type {
  AgentItem,
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
  TaskExecuteResult,
  ThreadsSnapshot,
  WorkspaceChangesSnapshot,
  WorkspaceDiffSnapshot,
  WorkspaceId,
  WorkspaceSnapshot,
  WorkspaceThreadId,
  WorkspaceTabsSnapshot,
} from "../luban-api"
import { defaultMockFixtures } from "./fixtures"

type ConversationState = Omit<ConversationSnapshot, "entries"> & {
  entries: ConversationEntry[]
}

type MockRuntimeState = {
  rev: number
  app: AppSnapshot
  threadsByWorkspace: Map<WorkspaceId, ThreadsSnapshot>
  conversationsByWorkspaceThread: Map<string, ConversationState>
  contextItemsByWorkspace: Map<WorkspaceId, ContextItemSnapshot[]>
  attachmentUrlsById: Map<string, string>
  workspaceChangesByWorkspace: Map<WorkspaceId, WorkspaceChangesSnapshot>
  workspaceDiffByWorkspace: Map<WorkspaceId, WorkspaceDiffSnapshot>
  codexCustomPrompts: CodexCustomPromptSnapshot[]
  mentionIndex: MentionItemSnapshot[]
  codexConfig: {
    tree: CodexConfigEntrySnapshot[]
    files: Map<string, string>
  }
  ampConfig: {
    tree: AmpConfigEntrySnapshot[]
    files: Map<string, string>
  }
  claudeConfig: {
    tree: ClaudeConfigEntrySnapshot[]
    files: Map<string, string>
  }
  nextContextId: number
  nextThreadId: number
  pendingAgentTimersByKey: Map<string, number>
  pendingAgentSeqByKey: Map<string, number>
  pendingWorkspaceCreateTimersByProjectId: Map<ProjectId, number>
}

let runtime: MockRuntimeState | null = null

function clone<T>(value: T): T {
  if (typeof structuredClone === "function") return structuredClone(value)
  return JSON.parse(JSON.stringify(value)) as T
}

function workspaceThreadKey(workspaceId: WorkspaceId, threadId: WorkspaceThreadId): string {
  return `${workspaceId}:${threadId}`
}

function maxFrom(values: number[], fallback: number): number {
  let out = fallback
  for (const v of values) out = Math.max(out, v)
  return out
}

function locateWorkspace(app: AppSnapshot, workspaceId: WorkspaceId): WorkspaceSnapshot | null {
  for (const p of app.projects) {
    const found = p.workspaces.find((w) => w.id === workspaceId)
    if (found) return found
  }
  return null
}

function listAllThreadIds(threadsByWorkspace: Map<WorkspaceId, ThreadsSnapshot>): number[] {
  const out: number[] = []
  for (const snap of threadsByWorkspace.values()) {
    for (const t of snap.threads) out.push(t.thread_id)
  }
  return out
}

function listAllContextIds(contextByWorkspace: Map<WorkspaceId, ContextItemSnapshot[]>): number[] {
  const out: number[] = []
  for (const items of contextByWorkspace.values()) {
    for (const i of items) out.push(i.context_id)
  }
  return out
}

function bumpRev(state: MockRuntimeState): number {
  state.rev += 1
  state.app.rev = state.rev
  return state.rev
}

function initRuntime(): MockRuntimeState {
  const fixtures = defaultMockFixtures()

  const threadsByWorkspace = new Map<WorkspaceId, ThreadsSnapshot>()
  for (const [k, v] of Object.entries(fixtures.threadsByWorkspace)) threadsByWorkspace.set(Number(k), clone(v))

  const conversationsByWorkspaceThread = new Map<string, ConversationState>()
  for (const [k, v] of Object.entries(fixtures.conversationsByWorkspaceThread)) {
    conversationsByWorkspaceThread.set(k, clone(v) as ConversationState)
  }

  const contextItemsByWorkspace = new Map<WorkspaceId, ContextItemSnapshot[]>()
  for (const [k, v] of Object.entries(fixtures.contextItemsByWorkspace)) {
    contextItemsByWorkspace.set(Number(k), clone(v))
  }

  const attachmentUrlsById = new Map<string, string>(Object.entries(fixtures.attachmentUrlsById))

  const workspaceChangesByWorkspace = new Map<WorkspaceId, WorkspaceChangesSnapshot>()
  for (const [k, v] of Object.entries(fixtures.workspaceChangesByWorkspace)) {
    workspaceChangesByWorkspace.set(Number(k), clone(v))
  }

  const workspaceDiffByWorkspace = new Map<WorkspaceId, WorkspaceDiffSnapshot>()
  for (const [k, v] of Object.entries(fixtures.workspaceDiffByWorkspace)) {
    workspaceDiffByWorkspace.set(Number(k), clone(v))
  }

  const nextThreadId = maxFrom(listAllThreadIds(threadsByWorkspace), 0) + 1
  const nextContextId = maxFrom(listAllContextIds(contextItemsByWorkspace), 0) + 1

  const codexConfigFiles = new Map<string, string>(Object.entries(fixtures.codexConfig.files))
  const ampConfigFiles = new Map<string, string>(Object.entries(fixtures.ampConfig.files))
  const claudeConfigFiles = new Map<string, string>(Object.entries(fixtures.claudeConfig.files))

  return {
    rev: fixtures.app.rev,
    app: clone(fixtures.app),
    threadsByWorkspace,
    conversationsByWorkspaceThread,
    contextItemsByWorkspace,
    attachmentUrlsById,
    workspaceChangesByWorkspace,
    workspaceDiffByWorkspace,
    codexCustomPrompts: clone(fixtures.codexCustomPrompts),
    mentionIndex: clone(fixtures.mentionIndex),
    codexConfig: { tree: clone(fixtures.codexConfig.tree), files: codexConfigFiles },
    ampConfig: { tree: clone(fixtures.ampConfig.tree), files: ampConfigFiles },
    claudeConfig: { tree: clone(fixtures.claudeConfig.tree), files: claudeConfigFiles },
    nextContextId,
    nextThreadId,
    pendingAgentTimersByKey: new Map(),
    pendingAgentSeqByKey: new Map(),
    pendingWorkspaceCreateTimersByProjectId: new Map(),
  }
}

function getRuntime(): MockRuntimeState {
  if (!runtime) runtime = initRuntime()
  return runtime
}

export function mockAttachmentUrl(attachmentId: string): string | null {
  return getRuntime().attachmentUrlsById.get(attachmentId) ?? null
}

export async function mockFetchApp(): Promise<AppSnapshot> {
  return clone(getRuntime().app)
}

export async function mockFetchThreads(workspaceId: WorkspaceId): Promise<ThreadsSnapshot> {
  const state = getRuntime()
  const snap = state.threadsByWorkspace.get(workspaceId)
  if (!snap) throw new Error(`mock: unknown workspace_id: ${workspaceId}`)
  return clone(snap)
}

function paginateConversation(args: {
  conversation: ConversationState
  before?: number
  limit?: number
}): ConversationSnapshot {
  const total = args.conversation.entries.length
  const limit = Math.max(1, Math.min(2000, args.limit ?? 2000))
  const before = Math.max(0, Math.min(total, args.before ?? total))
  const end = before
  const start = Math.max(0, end - limit)
  const page = args.conversation.entries.slice(start, end)

  return clone({
    ...args.conversation,
    entries: page,
    entries_total: total,
    entries_start: start,
    entries_truncated: start > 0 || page.length < total,
  } satisfies ConversationSnapshot)
}

export async function mockFetchConversation(
  workspaceId: WorkspaceId,
  threadId: WorkspaceThreadId,
  args: { before?: number; limit?: number } = {},
): Promise<ConversationSnapshot> {
  const state = getRuntime()
  const snap = state.conversationsByWorkspaceThread.get(workspaceThreadKey(workspaceId, threadId))
  if (!snap) throw new Error(`mock: unknown conversation: ${workspaceId}:${threadId}`)
  return paginateConversation({ conversation: snap, before: args.before, limit: args.limit })
}

export async function mockFetchWorkspaceChanges(workspaceId: WorkspaceId): Promise<WorkspaceChangesSnapshot> {
  const state = getRuntime()
  const snap = state.workspaceChangesByWorkspace.get(workspaceId)
  return clone(snap ?? { workspace_id: workspaceId, files: [] })
}

export async function mockFetchWorkspaceDiff(workspaceId: WorkspaceId): Promise<WorkspaceDiffSnapshot> {
  const state = getRuntime()
  const snap = state.workspaceDiffByWorkspace.get(workspaceId)
  return clone(snap ?? { workspace_id: workspaceId, files: [] })
}

export async function mockFetchContext(workspaceId: WorkspaceId): Promise<ContextSnapshot> {
  const state = getRuntime()
  const items = state.contextItemsByWorkspace.get(workspaceId) ?? []
  return { workspace_id: workspaceId, items: clone(items) }
}

export async function mockDeleteContextItem(workspaceId: WorkspaceId, contextId: number): Promise<void> {
  const state = getRuntime()
  const items = state.contextItemsByWorkspace.get(workspaceId) ?? []
  state.contextItemsByWorkspace.set(
    workspaceId,
    items.filter((i) => i.context_id !== contextId),
  )
}

export async function mockFetchCodexCustomPrompts(): Promise<CodexCustomPromptSnapshot[]> {
  return clone(getRuntime().codexCustomPrompts)
}

export async function mockFetchMentionItems(args: {
  workspaceId: WorkspaceId
  query: string
}): Promise<MentionItemSnapshot[]> {
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
    id: `mock_att_${Math.random().toString(16).slice(2)}_${Date.now().toString(16)}`,
    kind: args.kind,
    name,
    extension,
    mime: args.file.type || null,
    byte_len: args.file.size,
  }
}

export async function mockUploadAttachment(args: {
  workspaceId: WorkspaceId
  file: File
  kind: AttachmentKind
}): Promise<AttachmentRef> {
  const state = getRuntime()
  const ref = attachmentFromFile({ file: args.file, kind: args.kind })
  const url = URL.createObjectURL(args.file)
  state.attachmentUrlsById.set(ref.id, url)

  const nextId = state.nextContextId++
  const items = state.contextItemsByWorkspace.get(args.workspaceId) ?? []
  items.push({ context_id: nextId, attachment: ref, created_at_unix_ms: Date.now() })
  state.contextItemsByWorkspace.set(args.workspaceId, items)

  return ref
}

function getThreads(state: MockRuntimeState, workspaceId: WorkspaceId): ThreadsSnapshot {
  const snap = state.threadsByWorkspace.get(workspaceId)
  if (!snap) throw new Error(`mock: unknown workspace_id: ${workspaceId}`)
  return snap
}

function getConversationState(state: MockRuntimeState, workspaceId: WorkspaceId, threadId: WorkspaceThreadId): ConversationState {
  const snap = state.conversationsByWorkspaceThread.get(workspaceThreadKey(workspaceId, threadId))
  if (!snap) throw new Error(`mock: unknown conversation: ${workspaceId}:${threadId}`)
  return snap
}

function cancelPendingAgentRun(state: MockRuntimeState, conversationKey: string) {
  const timer = state.pendingAgentTimersByKey.get(conversationKey) ?? null
  if (timer != null) {
    window.clearTimeout(timer)
    state.pendingAgentTimersByKey.delete(conversationKey)
  }
}

function setWorkspaceAgentStatus(state: MockRuntimeState, workspaceId: WorkspaceId, status: "idle" | "running") {
  const workspace = locateWorkspace(state.app, workspaceId)
  if (!workspace) return
  workspace.agent_run_status = status
}

function emitWorkspaceThreadsChanged(args: {
  state: MockRuntimeState
  workspaceId: WorkspaceId
  onEvent: (event: ServerEvent) => void
}) {
  const snap = getThreads(args.state, args.workspaceId)
  args.onEvent({
    type: "workspace_threads_changed",
    workspace_id: args.workspaceId,
    tabs: clone(snap.tabs),
    threads: clone(snap.threads),
  })
}

function emitConversationChanged(args: {
  state: MockRuntimeState
  workspaceId: WorkspaceId
  threadId: WorkspaceThreadId
  onEvent: (event: ServerEvent) => void
}) {
  const snap = getConversationState(args.state, args.workspaceId, args.threadId)
  args.onEvent({ type: "conversation_changed", snapshot: paginateConversation({ conversation: snap }) })
}

function emitAppChanged(args: { state: MockRuntimeState; onEvent: (event: ServerEvent) => void }) {
  bumpRev(args.state)
  args.onEvent({ type: "app_changed", rev: args.state.rev, snapshot: clone(args.state.app) })
}

function normalizeTabsAfterRemoval(tabs: WorkspaceTabsSnapshot) {
  const open = tabs.open_tabs
  if (open.length === 0) {
    tabs.active_tab = tabs.active_tab
    return
  }
  if (!open.includes(tabs.active_tab)) tabs.active_tab = open[0]!
}

function reorder<T>(items: T[], fromIndex: number, toIndex: number): T[] {
  const next = items.slice()
  const [moved] = next.splice(fromIndex, 1)
  next.splice(toIndex, 0, moved!)
  return next
}

function startMockAgentRun(args: {
  state: MockRuntimeState
  workspaceId: WorkspaceId
  threadId: WorkspaceThreadId
  userText: string
  onEvent: (event: ServerEvent) => void
}) {
  const conversationKey = workspaceThreadKey(args.workspaceId, args.threadId)
  cancelPendingAgentRun(args.state, conversationKey)

  const seq = (args.state.pendingAgentSeqByKey.get(conversationKey) ?? 0) + 1
  args.state.pendingAgentSeqByKey.set(conversationKey, seq)

  const convo = getConversationState(args.state, args.workspaceId, args.threadId)
  convo.run_status = "running"
  convo.run_started_at_unix_ms = Date.now()
  convo.run_finished_at_unix_ms = null
  convo.in_progress_items = [
    {
      id: `in_progress_reasoning_${seq}`,
      kind: "reasoning",
      payload: { text: "Mock agent is running..." },
    } satisfies AgentItem,
  ]
  setWorkspaceAgentStatus(args.state, args.workspaceId, "running")
  emitConversationChanged({ state: args.state, workspaceId: args.workspaceId, threadId: args.threadId, onEvent: args.onEvent })
  emitAppChanged({ state: args.state, onEvent: args.onEvent })

  const timer = window.setTimeout(() => {
    const currentSeq = args.state.pendingAgentSeqByKey.get(conversationKey) ?? 0
    if (currentSeq !== seq) return

    convo.in_progress_items = []
    convo.entries.push({ type: "agent_item", id: `mock_reasoning_${seq}`, kind: "reasoning", payload: { text: "Thinking..." } })
    convo.entries.push({
      type: "agent_item",
      id: `mock_cmd_${seq}`,
      kind: "command_execution",
      payload: { command: "rg -n \"mock\" web", aggregated_output: "Found 3 matches.", status: "done" },
    })
    convo.entries.push({
      type: "agent_item",
      id: `mock_file_change_${seq}`,
      kind: "file_change",
      payload: { changes: [{ kind: "update", path: "web/lib/mock/mock-runtime.ts" }] },
    })
    convo.entries.push({
      type: "agent_item",
      id: `mock_agent_message_${seq}`,
      kind: "agent_message",
      payload: {
        text: `Mock reply: ${args.userText.trim().slice(0, 200)}`,
      },
    })
    convo.entries.push({ type: "turn_duration", duration_ms: 420 })

    convo.run_status = "idle"
    convo.run_finished_at_unix_ms = Date.now()
    setWorkspaceAgentStatus(args.state, args.workspaceId, "idle")
    emitConversationChanged({ state: args.state, workspaceId: args.workspaceId, threadId: args.threadId, onEvent: args.onEvent })
    emitAppChanged({ state: args.state, onEvent: args.onEvent })
  }, 600)

  args.state.pendingAgentTimersByKey.set(conversationKey, timer)
}

function cancelPendingWorkspaceCreate(state: MockRuntimeState, projectId: ProjectId) {
  const t = state.pendingWorkspaceCreateTimersByProjectId.get(projectId)
  if (t != null) {
    window.clearTimeout(t)
    state.pendingWorkspaceCreateTimersByProjectId.delete(projectId)
  }
}

export function mockDispatchAction(args: {
  action: ClientAction
  onEvent: (event: ServerEvent) => void
}): void {
  const state = getRuntime()
  const a = args.action

  if (a.type === "add_project") {
    const id: ProjectId = `mock_project_${Math.random().toString(16).slice(2)}`
    state.app.projects.push({
      id,
      name: a.path.split("/").slice(-1)[0] || "Project",
      slug: id,
      path: a.path,
      is_git: true,
      expanded: true,
      create_workspace_status: "idle",
      workspaces: [],
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
    for (const p of state.app.projects) {
      if (p.id !== a.project_id) continue
      p.expanded = !p.expanded
      emitAppChanged({ state, onEvent: args.onEvent })
      return
    }
    return
  }

  if (a.type === "ensure_main_workspace") {
    for (const p of state.app.projects) {
      if (p.id !== a.project_id) continue
      const exists = p.workspaces.some((w) => w.workspace_name === "main" && w.status === "active")
      if (exists) return
      const workspaceId = Math.max(1, ...p.workspaces.map((w) => w.id), ...state.app.projects.flatMap((x) => x.workspaces.map((w) => w.id))) + 1
      p.workspaces.push({
        id: workspaceId,
        short_id: `W${workspaceId}`,
        workspace_name: "main",
        branch_name: p.is_git ? "main" : "",
        worktree_path: p.path,
        status: "active",
        archive_status: "idle",
        branch_rename_status: "idle",
        agent_run_status: "idle",
        has_unread_completion: false,
        pull_request: null,
      })
      state.threadsByWorkspace.set(workspaceId, {
        rev: state.rev,
        workspace_id: workspaceId,
        tabs: { open_tabs: [], archived_tabs: [], active_tab: 1 },
        threads: [],
      })
      emitAppChanged({ state, onEvent: args.onEvent })
      return
    }
    return
  }

  if (a.type === "create_workspace") {
    for (const p of state.app.projects) {
      if (p.id !== a.project_id) continue

      if (p.create_workspace_status === "running") return
      cancelPendingWorkspaceCreate(state, p.id)

      p.create_workspace_status = "running"
      emitAppChanged({ state, onEvent: args.onEvent })

      const timer = window.setTimeout(() => {
        const p2 = state.app.projects.find((x) => x.id === a.project_id) ?? null
        if (!p2 || p2.create_workspace_status !== "running") return

        const workspaceId = Math.max(0, ...state.app.projects.flatMap((x) => x.workspaces.map((w) => w.id))) + 1
        const name = `ws-${workspaceId}`
        p2.workspaces.push({
          id: workspaceId,
          short_id: `W${workspaceId}`,
          workspace_name: name,
          branch_name: p2.is_git ? name : "",
          worktree_path: `${p2.path}-${name}`,
          status: "active",
          archive_status: "idle",
          branch_rename_status: "idle",
          agent_run_status: "idle",
          has_unread_completion: false,
          pull_request: null,
        })

        const nextRev = state.rev + 1
        state.threadsByWorkspace.set(workspaceId, {
          rev: nextRev,
          workspace_id: workspaceId,
          tabs: { open_tabs: [], archived_tabs: [], active_tab: 1 },
          threads: [],
        })

        p2.create_workspace_status = "idle"
        cancelPendingWorkspaceCreate(state, p2.id)
        emitAppChanged({ state, onEvent: args.onEvent })
      }, 450)

      state.pendingWorkspaceCreateTimersByProjectId.set(p.id, timer)
      return
    }
    return
  }

  if (a.type === "archive_workspace") {
    const w = locateWorkspace(state.app, a.workspace_id)
    if (w) {
      w.status = "archived"
      emitAppChanged({ state, onEvent: args.onEvent })
    }
    return
  }

  if (a.type === "open_workspace") {
    state.app.ui.active_workspace_id = a.workspace_id
    const snap = state.threadsByWorkspace.get(a.workspace_id) ?? null
    if (snap && snap.tabs.open_tabs.length > 0) {
      state.app.ui.active_thread_id = snap.tabs.active_tab
    }
    emitAppChanged({ state, onEvent: args.onEvent })
    emitWorkspaceThreadsChanged({ state, workspaceId: a.workspace_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "activate_workspace_thread") {
    const snap = getThreads(state, a.workspace_id)
    snap.tabs.active_tab = a.thread_id
    state.app.ui.active_thread_id = a.thread_id
    emitAppChanged({ state, onEvent: args.onEvent })
    emitWorkspaceThreadsChanged({ state, workspaceId: a.workspace_id, onEvent: args.onEvent })
    emitConversationChanged({ state, workspaceId: a.workspace_id, threadId: a.thread_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "create_workspace_thread") {
    const snap = getThreads(state, a.workspace_id)
    const threadId = state.nextThreadId++
    snap.threads.push({
      thread_id: threadId,
      remote_thread_id: null,
      title: `Mock thread ${threadId}`,
      updated_at_unix_seconds: Math.floor(Date.now() / 1000),
    })
    snap.tabs.open_tabs.push(threadId)
    bumpRev(state)
    snap.rev = state.rev
    const convo: ConversationState = {
      rev: state.rev,
      workspace_id: a.workspace_id,
      thread_id: threadId,
      agent_model_id: state.app.agent.default_model_id ?? "gpt-5",
      thinking_effort: state.app.agent.default_thinking_effort ?? "medium",
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
      title: `Mock thread ${threadId}`,
    }
    state.conversationsByWorkspaceThread.set(workspaceThreadKey(a.workspace_id, threadId), convo)
    emitWorkspaceThreadsChanged({ state, workspaceId: a.workspace_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "close_workspace_thread_tab") {
    const snap = getThreads(state, a.workspace_id)
    snap.tabs.open_tabs = snap.tabs.open_tabs.filter((id) => id !== a.thread_id)
    if (!snap.tabs.archived_tabs.includes(a.thread_id)) snap.tabs.archived_tabs.push(a.thread_id)
    normalizeTabsAfterRemoval(snap.tabs)
    if (state.app.ui.active_workspace_id === a.workspace_id) {
      state.app.ui.active_thread_id = snap.tabs.active_tab
    }
    bumpRev(state)
    snap.rev = state.rev
    emitAppChanged({ state, onEvent: args.onEvent })
    emitWorkspaceThreadsChanged({ state, workspaceId: a.workspace_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "restore_workspace_thread_tab") {
    const snap = getThreads(state, a.workspace_id)
    snap.tabs.archived_tabs = snap.tabs.archived_tabs.filter((id) => id !== a.thread_id)
    if (!snap.tabs.open_tabs.includes(a.thread_id)) snap.tabs.open_tabs.push(a.thread_id)
    bumpRev(state)
    snap.rev = state.rev
    emitAppChanged({ state, onEvent: args.onEvent })
    emitWorkspaceThreadsChanged({ state, workspaceId: a.workspace_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "reorder_workspace_thread_tab") {
    const snap = getThreads(state, a.workspace_id)
    const from = snap.tabs.open_tabs.indexOf(a.thread_id)
    const to = Math.max(0, Math.min(snap.tabs.open_tabs.length - 1, a.to_index))
    if (from >= 0 && from !== to) {
      snap.tabs.open_tabs = reorder(snap.tabs.open_tabs, from, to)
      bumpRev(state)
      snap.rev = state.rev
      emitWorkspaceThreadsChanged({ state, workspaceId: a.workspace_id, onEvent: args.onEvent })
    }
    return
  }

  if (a.type === "send_agent_message") {
    const convo = getConversationState(state, a.workspace_id, a.thread_id)
    convo.entries.push({ type: "user_message", text: a.text, attachments: a.attachments })
    startMockAgentRun({ state, workspaceId: a.workspace_id, threadId: a.thread_id, userText: a.text, onEvent: args.onEvent })
    return
  }

  if (a.type === "cancel_agent_turn") {
    const conversationKey = workspaceThreadKey(a.workspace_id, a.thread_id)
    cancelPendingAgentRun(state, conversationKey)
    const seq = (state.pendingAgentSeqByKey.get(conversationKey) ?? 0) + 1
    state.pendingAgentSeqByKey.set(conversationKey, seq)
    const convo = getConversationState(state, a.workspace_id, a.thread_id)
    if (convo.run_status === "running") {
      convo.run_status = "idle"
      convo.in_progress_items = []
      convo.entries.push({ type: "turn_canceled" })
      setWorkspaceAgentStatus(state, a.workspace_id, "idle")
      emitConversationChanged({ state, workspaceId: a.workspace_id, threadId: a.thread_id, onEvent: args.onEvent })
      emitAppChanged({ state, onEvent: args.onEvent })
    }
    return
  }

  if (a.type === "cancel_and_send_agent_message") {
    mockDispatchAction({ action: { type: "cancel_agent_turn", workspace_id: a.workspace_id, thread_id: a.thread_id }, onEvent: args.onEvent })
    mockDispatchAction({ action: { type: "send_agent_message", workspace_id: a.workspace_id, thread_id: a.thread_id, text: a.text, attachments: a.attachments }, onEvent: args.onEvent })
    return
  }

  if (a.type === "queue_agent_message") {
    const convo = getConversationState(state, a.workspace_id, a.thread_id)
    const nextId = Math.max(0, ...convo.pending_prompts.map((p) => p.id)) + 1
    convo.pending_prompts.push({
      id: nextId,
      text: a.text,
      attachments: a.attachments,
      run_config: { model_id: convo.agent_model_id, thinking_effort: convo.thinking_effort },
    })
    emitConversationChanged({ state, workspaceId: a.workspace_id, threadId: a.thread_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "remove_queued_prompt") {
    const convo = getConversationState(state, a.workspace_id, a.thread_id)
    convo.pending_prompts = convo.pending_prompts.filter((p) => p.id !== a.prompt_id)
    emitConversationChanged({ state, workspaceId: a.workspace_id, threadId: a.thread_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "reorder_queued_prompt") {
    const convo = getConversationState(state, a.workspace_id, a.thread_id)
    const from = convo.pending_prompts.findIndex((p) => p.id === a.active_id)
    const to = convo.pending_prompts.findIndex((p) => p.id === a.over_id)
    if (from >= 0 && to >= 0 && from !== to) {
      convo.pending_prompts = reorder(convo.pending_prompts, from, to)
      emitConversationChanged({ state, workspaceId: a.workspace_id, threadId: a.thread_id, onEvent: args.onEvent })
    }
    return
  }

  if (a.type === "update_queued_prompt") {
    const convo = getConversationState(state, a.workspace_id, a.thread_id)
    const idx = convo.pending_prompts.findIndex((p) => p.id === a.prompt_id)
    if (idx >= 0) {
      convo.pending_prompts[idx] = {
        id: a.prompt_id,
        text: a.text,
        attachments: a.attachments,
        run_config: { model_id: a.model_id, thinking_effort: a.thinking_effort },
      }
      emitConversationChanged({ state, workspaceId: a.workspace_id, threadId: a.thread_id, onEvent: args.onEvent })
    }
    return
  }

  if (a.type === "chat_model_changed") {
    const convo = getConversationState(state, a.workspace_id, a.thread_id)
    convo.agent_model_id = a.model_id
    emitConversationChanged({ state, workspaceId: a.workspace_id, threadId: a.thread_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "thinking_effort_changed") {
    const convo = getConversationState(state, a.workspace_id, a.thread_id)
    convo.thinking_effort = a.thinking_effort
    emitConversationChanged({ state, workspaceId: a.workspace_id, threadId: a.thread_id, onEvent: args.onEvent })
    return
  }

  if (a.type === "sidebar_project_order_changed") {
    state.app.ui.sidebar_project_order = a.project_ids
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "sidebar_worktree_order_changed") {
    if (!state.app.ui.sidebar_worktree_order) state.app.ui.sidebar_worktree_order = {}
    state.app.ui.sidebar_worktree_order[a.project_id] = a.workspace_ids
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "open_button_selection_changed") {
    state.app.ui.open_button_selection = a.selection
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

  if (a.type === "codex_enabled_changed") {
    state.app.agent.codex_enabled = a.enabled
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "amp_enabled_changed") {
    state.app.agent.amp_enabled = a.enabled
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "agent_runner_changed") {
    state.app.agent.default_runner = a.runner
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "agent_amp_mode_changed") {
    state.app.agent.amp_mode = a.mode
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "task_prompt_template_changed") {
    state.app.task.prompt_templates = state.app.task.prompt_templates.filter((p) => p.intent_kind !== a.intent_kind)
    state.app.task.prompt_templates.push({ intent_kind: a.intent_kind, template: a.template })
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "system_prompt_template_changed") {
    state.app.task.system_prompt_templates = state.app.task.system_prompt_templates.filter((p) => p.kind !== a.kind)
    state.app.task.system_prompt_templates.push({ kind: a.kind, template: a.template })
    emitAppChanged({ state, onEvent: args.onEvent })
    return
  }

  if (a.type === "workspace_rename_branch") {
    const w = locateWorkspace(state.app, a.workspace_id)
    if (w) {
      w.branch_name = a.branch_name
      w.branch_rename_status = "idle"
      emitAppChanged({ state, onEvent: args.onEvent })
    }
    return
  }

  if (a.type === "workspace_ai_rename_branch") {
    const w = locateWorkspace(state.app, a.workspace_id)
    if (w) {
      w.branch_name = `ai/rename-${a.thread_id}`
      w.branch_rename_status = "idle"
      emitAppChanged({ state, onEvent: args.onEvent })
    }
    return
  }

  if (a.type === "open_workspace_in_ide" || a.type === "open_workspace_with" || a.type === "open_workspace_pull_request" || a.type === "open_workspace_pull_request_failed_action") {
    args.onEvent({ type: "toast", message: `Mock: ${a.type}` })
    return
  }

  if (
    a.type === "pick_project_path" ||
    a.type === "task_preview" ||
    a.type === "task_execute" ||
    a.type === "feedback_submit" ||
    a.type === "codex_check" ||
    a.type === "codex_config_tree" ||
    a.type === "codex_config_list_dir" ||
    a.type === "codex_config_read_file" ||
    a.type === "codex_config_write_file" ||
    a.type === "amp_check" ||
    a.type === "amp_config_tree" ||
    a.type === "amp_config_list_dir" ||
    a.type === "amp_config_read_file" ||
    a.type === "amp_config_write_file" ||
    a.type === "claude_check" ||
    a.type === "claude_config_tree" ||
    a.type === "claude_config_list_dir" ||
    a.type === "claude_config_read_file" ||
    a.type === "claude_config_write_file" ||
    a.type === "add_project_and_open"
  ) {
    args.onEvent({ type: "toast", message: `Mock: request-only action used as sendAction: ${a.type}` })
    return
  }

  const actionType = (a as { type: string }).type
  const _exhaustive: never = a
  args.onEvent({ type: "toast", message: `Mock: action not implemented: ${actionType}` })
}

function listCodexDir(tree: CodexConfigEntrySnapshot[], prefix: string): CodexConfigEntrySnapshot[] {
  const normalized = prefix.replace(/^\/+/, "").replace(/\/+$/, "")
  if (normalized.length === 0) return tree

  const parts = normalized.split("/").filter(Boolean)
  let cursor = tree
  for (const part of parts) {
    const next = cursor.find((e) => e.name === part || e.path === part)
    if (!next || next.kind !== "folder") return []
    cursor = next.children ?? []
  }
  return cursor
}

function listAmpDir(tree: AmpConfigEntrySnapshot[], prefix: string): AmpConfigEntrySnapshot[] {
  const normalized = prefix.replace(/^\/+/, "").replace(/\/+$/, "")
  if (normalized.length === 0) return tree

  const parts = normalized.split("/").filter(Boolean)
  let cursor = tree
  for (const part of parts) {
    const next = cursor.find((e) => e.name === part || e.path === part)
    if (!next || next.kind !== "folder") return []
    cursor = next.children ?? []
  }
  return cursor
}

function listClaudeDir(tree: ClaudeConfigEntrySnapshot[], prefix: string): ClaudeConfigEntrySnapshot[] {
  const normalized = prefix.replace(/^\/+/, "").replace(/\/+$/, "")
  if (normalized.length === 0) return tree

  const parts = normalized.split("/").filter(Boolean)
  let cursor = tree
  for (const part of parts) {
    const next = cursor.find((e) => e.name === part || e.path === part)
    if (!next || next.kind !== "folder") return []
    cursor = next.children ?? []
  }
  return cursor
}

export async function mockRequest<T>(action: ClientAction): Promise<T> {
  const state = getRuntime()

  if (action.type === "pick_project_path") {
    const value = window.prompt("Enter a project path (mock):", "/mock/new/project")
    return (value && value.trim().length > 0 ? value.trim() : null) as T
  }

  if (action.type === "add_project_and_open") {
    const projectId: ProjectId = `mock_project_${Math.random().toString(16).slice(2)}`
    const workspaceId = Math.max(0, ...state.app.projects.flatMap((p) => p.workspaces.map((w) => w.id))) + 1

    state.app.projects.push({
      id: projectId,
      name: action.path.split("/").slice(-1)[0] || "Project",
      slug: projectId,
      path: action.path,
      is_git: true,
      expanded: true,
      create_workspace_status: "idle",
      workspaces: [
        {
          id: workspaceId,
          short_id: `W${workspaceId}`,
          workspace_name: "main",
          branch_name: "main",
          worktree_path: action.path,
          status: "active",
          archive_status: "idle",
          branch_rename_status: "idle",
          agent_run_status: "idle",
          has_unread_completion: false,
          pull_request: null,
        },
      ],
    })

    state.app.ui.active_workspace_id = workspaceId
    state.app.ui.active_thread_id = 1
    bumpRev(state)

    state.threadsByWorkspace.set(workspaceId, {
      rev: state.rev,
      workspace_id: workspaceId,
      tabs: { open_tabs: [1], archived_tabs: [], active_tab: 1 },
      threads: [{ thread_id: 1, remote_thread_id: null, title: "Mock thread", updated_at_unix_seconds: Math.floor(Date.now() / 1000) }],
    })
    state.conversationsByWorkspaceThread.set(workspaceThreadKey(workspaceId, 1), {
      rev: state.rev,
      workspace_id: workspaceId,
      thread_id: 1,
      agent_model_id: state.app.agent.default_model_id ?? "gpt-5",
      thinking_effort: state.app.agent.default_thinking_effort ?? "medium",
      run_status: "idle",
      run_started_at_unix_ms: null,
      run_finished_at_unix_ms: null,
      entries: [
        { type: "user_message", text: "New project added (mock).", attachments: [] },
        { type: "agent_item", id: "agent_message_welcome", kind: "agent_message", payload: { text: "This project is created by mock mode." } },
      ],
      entries_total: 2,
      entries_start: 0,
      entries_truncated: false,
      in_progress_items: [],
      pending_prompts: [],
      queue_paused: false,
      remote_thread_id: null,
      title: "Mock thread",
    })

    return { projectId, workspaceId } as T
  }

  if (action.type === "task_preview") {
    const defaultProjectPath =
      state.app.projects.find((p) => p.workspaces.some((w) => w.id === state.app.ui.active_workspace_id))?.path ??
      state.app.projects[0]?.path ??
      "/mock/project"

    const draft: TaskDraft = {
      input: action.input,
      project: { type: "local_path", path: defaultProjectPath },
      intent_kind: "other",
      summary: "Mock preview",
      prompt: action.input,
      repo: null,
      issue: null,
      pull_request: null,
    }
    return draft as T
  }

  if (action.type === "task_execute") {
    const ensureProjectByPath = (path: string): ProjectId => {
      const existing = state.app.projects.find((p) => p.path === path) ?? null
      if (existing) return existing.id

      const projectId: ProjectId = `mock_project_${Math.random().toString(16).slice(2)}`
      state.app.projects.push({
        id: projectId,
        name: path.split("/").slice(-1)[0] || "Project",
        slug: projectId,
        path,
        is_git: true,
        expanded: true,
        create_workspace_status: "idle",
        workspaces: [],
      })
      return projectId
    }

    const resolveProjectId = (): ProjectId => {
      const spec = action.draft.project
      if (spec.type === "local_path") return ensureProjectByPath(spec.path)
      if (spec.type === "git_hub_repo") return ensureProjectByPath(`/mock/github/${spec.full_name}`)
      return state.app.projects[0]?.id ?? ensureProjectByPath("/mock/project")
    }

    const projectId = resolveProjectId()
    const project = state.app.projects.find((p) => p.id === projectId) ?? null
    if (!project) throw new Error(`mock: project not found: ${projectId}`)

    if (action.mode === "create") {
      const workspaceId = Math.max(0, ...state.app.projects.flatMap((p) => p.workspaces.map((w) => w.id))) + 1
      const threadId: WorkspaceThreadId = 1
      const name = `task-${workspaceId}`
      const worktreePath = `${project.path}-${name}`

      project.workspaces.push({
        id: workspaceId,
        short_id: `W${workspaceId}`,
        workspace_name: name,
        branch_name: project.is_git ? name : "",
        worktree_path: worktreePath,
        status: "active",
        archive_status: "idle",
        branch_rename_status: "idle",
        agent_run_status: "idle",
        has_unread_completion: false,
        pull_request: null,
      })

      bumpRev(state)
      state.app.ui.active_workspace_id = workspaceId
      state.app.ui.active_thread_id = threadId

      state.threadsByWorkspace.set(workspaceId, {
        rev: state.rev,
        workspace_id: workspaceId,
        tabs: { open_tabs: [threadId], archived_tabs: [], active_tab: threadId },
        threads: [
          {
            thread_id: threadId,
            remote_thread_id: null,
            title: "Mock task thread",
            updated_at_unix_seconds: Math.floor(Date.now() / 1000),
          },
        ],
      })

      state.conversationsByWorkspaceThread.set(workspaceThreadKey(workspaceId, threadId), {
        rev: state.rev,
        workspace_id: workspaceId,
        thread_id: threadId,
        agent_model_id: state.app.agent.default_model_id ?? "gpt-5",
        thinking_effort: state.app.agent.default_thinking_effort ?? "medium",
        run_status: "idle",
        run_started_at_unix_ms: null,
        run_finished_at_unix_ms: null,
        entries: [{ type: "user_message", text: action.draft.prompt, attachments: [] }],
        entries_total: 1,
        entries_start: 0,
        entries_truncated: false,
        in_progress_items: [],
        pending_prompts: [],
        queue_paused: false,
        remote_thread_id: null,
        title: "Mock task thread",
      })

      const ids: TaskExecuteResult = {
        project_id: projectId,
        workspace_id: workspaceId,
        thread_id: threadId,
        worktree_path: worktreePath,
        prompt: action.draft.prompt,
        mode: action.mode,
      }
      return ids as T
    }

    const workspaceId = state.app.ui.active_workspace_id ?? project.workspaces[0]?.id ?? 1
    const threadId = state.app.ui.active_thread_id ?? 1
    const worktreePath = locateWorkspace(state.app, workspaceId)?.worktree_path ?? project.path

    const ids: TaskExecuteResult = {
      project_id: projectId,
      workspace_id: workspaceId,
      thread_id: threadId,
      worktree_path: worktreePath,
      prompt: action.draft.prompt,
      mode: action.mode,
    }
    return ids as T
  }

  if (action.type === "feedback_submit") {
    const result: FeedbackSubmitResult = {
      issue: { number: 1, title: action.title, url: "https://example.invalid/issue/1" },
      task: null,
    }
    return result as T
  }

  if (action.type === "codex_check") {
    return { ok: true, message: "Mock check ok" } as T
  }

  if (action.type === "codex_config_tree") {
    return clone(state.codexConfig.tree) as T
  }

  if (action.type === "codex_config_list_dir") {
    const entries = listCodexDir(state.codexConfig.tree, action.path)
    return { path: action.path, entries: clone(entries) } as unknown as T
  }

  if (action.type === "codex_config_read_file") {
    const value = state.codexConfig.files.get(action.path)
    if (value == null) throw new Error(`mock: file not found: ${action.path}`)
    return value as unknown as T
  }

  if (action.type === "codex_config_write_file") {
    state.codexConfig.files.set(action.path, action.contents)
    return null as unknown as T
  }

  if (action.type === "amp_check") {
    return { ok: true, message: "Mock check ok" } as T
  }

  if (action.type === "amp_config_tree") {
    return clone(state.ampConfig.tree) as T
  }

  if (action.type === "amp_config_list_dir") {
    const entries = listAmpDir(state.ampConfig.tree, action.path)
    return { path: action.path, entries: clone(entries) } as unknown as T
  }

  if (action.type === "amp_config_read_file") {
    const value = state.ampConfig.files.get(action.path)
    if (value == null) throw new Error(`mock: file not found: ${action.path}`)
    return value as unknown as T
  }

  if (action.type === "amp_config_write_file") {
    state.ampConfig.files.set(action.path, action.contents)
    return null as unknown as T
  }

  if (action.type === "claude_check") {
    return { ok: true, message: "Mock check ok" } as T
  }

  if (action.type === "claude_config_tree") {
    return clone(state.claudeConfig.tree) as T
  }

  if (action.type === "claude_config_list_dir") {
    const entries = listClaudeDir(state.claudeConfig.tree, action.path)
    return { path: action.path, entries: clone(entries) } as unknown as T
  }

  if (action.type === "claude_config_read_file") {
    const value = state.claudeConfig.files.get(action.path)
    if (value == null) throw new Error(`mock: file not found: ${action.path}`)
    return value as unknown as T
  }

  if (action.type === "claude_config_write_file") {
    state.claudeConfig.files.set(action.path, action.contents)
    return null as unknown as T
  }

  throw new Error(`mock: request not implemented: ${action.type}`)
}
