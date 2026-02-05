import type {
  AgentItemKind,
  AgentRunnerKind,
  AmpConfigEntrySnapshot,
  AppSnapshot,
  AttachmentKind,
  AttachmentRef,
  ChangedFileSnapshot,
  ClaudeConfigEntrySnapshot,
  CodexConfigEntrySnapshot,
  CodexCustomPromptSnapshot,
  ConversationEntry,
  ConversationSnapshot,
  FileChangeGroup,
  FileChangeStatus,
  MentionItemKind,
  MentionItemSnapshot,
  OperationStatus,
  ProjectId,
  QueuedPromptSnapshot,
  TaskStatus,
  TaskSummarySnapshot,
  TasksSnapshot,
  ThinkingEffort,
  ThreadsSnapshot,
  TurnResult,
  TurnStatus,
  WorkspaceChangesSnapshot,
  WorkspaceDiffFileSnapshot,
  WorkspaceDiffSnapshot,
  WorkspaceId,
  WorkspaceThreadId,
  WorkspaceTabsSnapshot,
} from "../luban-api"

export type MockFixtures = {
  app: AppSnapshot
  threadsByWorkspace: Record<number, ThreadsSnapshot>
  tasksSnapshot: TasksSnapshot
  conversationsByWorkspaceThread: Record<string, ConversationSnapshot>
  attachmentUrlsById: Record<string, string>
  workspaceChangesByWorkspace: Record<number, WorkspaceChangesSnapshot>
  workspaceDiffByWorkspace: Record<number, WorkspaceDiffSnapshot>
  codexCustomPrompts: CodexCustomPromptSnapshot[]
  mentionIndex: MentionItemSnapshot[]
  codexConfig: {
    tree: CodexConfigEntrySnapshot[]
    files: Record<string, string>
  }
  ampConfig: {
    tree: AmpConfigEntrySnapshot[]
    files: Record<string, string>
  }
  claudeConfig: {
    tree: ClaudeConfigEntrySnapshot[]
    files: Record<string, string>
  }
}

const FIXTURE_BASE_UNIX_MS = Date.UTC(2026, 0, 22, 12, 0, 0)
let fixtureEntryCursorUnixMs = FIXTURE_BASE_UNIX_MS

function unixMs(offsetMs: number = 0): number {
  return FIXTURE_BASE_UNIX_MS + offsetMs
}

function nextEntryCreatedAtUnixMs(stepMs: number = 250): number {
  fixtureEntryCursorUnixMs += stepMs
  return fixtureEntryCursorUnixMs
}

function unixSeconds(offsetSeconds: number = 0): number {
  return Math.floor(FIXTURE_BASE_UNIX_MS / 1000) + offsetSeconds
}

function dataUrlSvg(text: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><rect width="100%" height="100%" fill="#111827"/><text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" fill="#E5E7EB" font-family="ui-sans-serif,system-ui" font-size="28">${text}</text></svg>`
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

function dataUrlText(text: string): string {
  return `data:text/plain;charset=utf-8,${encodeURIComponent(text)}`
}

function op(value: OperationStatus): OperationStatus {
  return value
}

function attachment(args: {
  id: string
  kind: AttachmentKind
  name: string
  extension: string
  byteLen: number
  mime?: string | null
}): AttachmentRef {
  return {
    id: args.id,
    kind: args.kind,
    name: args.name,
    extension: args.extension,
    mime: args.mime ?? null,
    byte_len: args.byteLen,
  }
}

function key(workdirId: WorkspaceId, taskId: WorkspaceThreadId): string {
  return `${workdirId}:${taskId}`
}

function newEntryId(prefix: string): string {
  return `${prefix}_${Math.random().toString(16).slice(2)}`
}

function userMessage(text: string): ConversationEntry {
  return {
    type: "user_event",
    entry_id: newEntryId("ue"),
    created_at_unix_ms: nextEntryCreatedAtUnixMs(),
    event: { type: "message", text, attachments: [] },
  }
}

function agentMessage(text: string): ConversationEntry {
  return {
    type: "agent_event",
    entry_id: newEntryId("ae"),
    created_at_unix_ms: nextEntryCreatedAtUnixMs(),
    event: { type: "message", id: `agent_msg_${Math.random().toString(16).slice(2)}`, text },
  }
}

function agentActivity(kind: AgentItemKind, payload: unknown): ConversationEntry {
  return {
    type: "agent_event",
    entry_id: newEntryId("ae"),
    created_at_unix_ms: nextEntryCreatedAtUnixMs(),
    event: { type: "item", id: `agent_act_${Math.random().toString(16).slice(2)}`, kind, payload },
  }
}

function agentTurnDuration(durationMs: number): ConversationEntry {
  return {
    type: "agent_event",
    entry_id: newEntryId("ae"),
    created_at_unix_ms: nextEntryCreatedAtUnixMs(),
    event: { type: "turn_duration", duration_ms: durationMs },
  }
}

function agentTurnError(message: string): ConversationEntry {
  return {
    type: "agent_event",
    entry_id: newEntryId("ae"),
    created_at_unix_ms: nextEntryCreatedAtUnixMs(),
    event: { type: "turn_error", message },
  }
}

function agentTurnCanceled(): ConversationEntry {
  return {
    type: "agent_event",
    entry_id: newEntryId("ae"),
    created_at_unix_ms: nextEntryCreatedAtUnixMs(),
    event: { type: "turn_canceled" },
  }
}

function queuedPrompt(args: {
  id: number
  text: string
  attachments?: AttachmentRef[]
  runner?: AgentRunnerKind
  modelId?: string
  thinkingEffort?: ThinkingEffort
  ampMode?: string | null
}): QueuedPromptSnapshot {
  return {
    id: args.id,
    text: args.text,
    attachments: args.attachments ?? [],
    run_config: {
      runner: args.runner ?? "codex",
      model_id: args.modelId ?? "gpt-5",
      thinking_effort: args.thinkingEffort ?? "medium",
      amp_mode: args.ampMode ?? null,
    },
  }
}

function systemEvent(args: {
  id: string
  createdAtUnixMs: number
  event:
    | { event_type: "task_created" }
    | { event_type: "task_archived" }
    | { event_type: "task_status_changed"; from: TaskStatus; to: TaskStatus }
    | {
        event_type: "task_status_suggestion"
        from: TaskStatus
        to: TaskStatus
        title: string
        explanation_markdown: string
      }
}): ConversationEntry {
  return {
    type: "system_event",
    entry_id: args.id,
    created_at_unix_ms: args.createdAtUnixMs,
    event: args.event,
  }
}

function longConversationEntries(args: { pairs: number }): ConversationEntry[] {
  const out: ConversationEntry[] = [
    systemEvent({
      id: "sys_long_1",
      createdAtUnixMs: unixMs(-3 * 60 * 60 * 1000),
      event: { event_type: "task_created" },
    }),
  ]

  for (let i = 0; i < args.pairs; i += 1) {
    out.push(userMessage(`User message ${i}`))
    out.push(agentMessage(`Assistant message ${i}`))
  }

  return out
}

function conversationBase(args: {
  workdirId: WorkspaceId
  taskId: WorkspaceThreadId
  title: string
  runner?: AgentRunnerKind
  runStatus?: OperationStatus
  taskStatus?: TaskStatus
  entries: ConversationEntry[]
}): ConversationSnapshot {
  const runner = args.runner ?? "codex"
  return {
    rev: 1,
    workdir_id: args.workdirId,
    task_id: args.taskId,
    task_status: args.taskStatus ?? "todo",
    agent_runner: runner,
    agent_model_id: "gpt-5",
    thinking_effort: "medium",
    amp_mode: runner === "amp" ? "default" : null,
    run_status: args.runStatus ?? "idle",
    run_started_at_unix_ms: null,
    run_finished_at_unix_ms: null,
    entries: args.entries,
    entries_total: args.entries.length,
    entries_start: 0,
    entries_truncated: false,
    pending_prompts: [],
    queue_paused: false,
    remote_thread_id: null,
    title: args.title,
  }
}

function changedFile(args: {
  id: string
  path: string
  name: string
  status: FileChangeStatus
  group: FileChangeGroup
  additions: number | null
  deletions: number | null
  oldPath?: string | null
}): ChangedFileSnapshot {
  return {
    id: args.id,
    path: args.path,
    name: args.name,
    status: args.status,
    group: args.group,
    additions: args.additions,
    deletions: args.deletions,
    old_path: args.oldPath ?? null,
  }
}

export function defaultMockFixtures(): MockFixtures {
  const workdir1: WorkspaceId = 1
  const workdir2: WorkspaceId = 2
  const workdir3: WorkspaceId = 3

  const task1: WorkspaceThreadId = 1
  const task2: WorkspaceThreadId = 2
  const task3: WorkspaceThreadId = 3
  const task4: WorkspaceThreadId = 4
  const task5: WorkspaceThreadId = 5
  const task6: WorkspaceThreadId = 6
  const task7: WorkspaceThreadId = 7
  const task8: WorkspaceThreadId = 8
  const task9: WorkspaceThreadId = 9
  const task10: WorkspaceThreadId = 10
  const task11: WorkspaceThreadId = 11

  const project1: ProjectId = "mock-project-1"
  const project2: ProjectId = "mock-project-2"

  const imgA = attachment({
    id: "mock_att_img_a",
    kind: "image",
    name: "mock-image-a.png",
    extension: "png",
    byteLen: 12_345,
    mime: "image/png",
  })
  const fileA = attachment({
    id: "mock_att_file_a",
    kind: "file",
    name: "notes.txt",
    extension: "txt",
    byteLen: 2_048,
    mime: "text/plain",
  })

  const attachmentUrlsById: Record<string, string> = {
    [imgA.id]: dataUrlSvg("Mock Image A"),
    [fileA.id]: dataUrlText("Mock file contents\n"),
  }

  const app: AppSnapshot = {
    rev: 1,
    projects: [
      {
        id: project1,
        name: "Mock Git Project",
        slug: "mock-git-project",
        path: "/mock/git/project",
        is_git: true,
        expanded: true,
        create_workdir_status: op("idle"),
        workdirs: [
          {
            id: workdir1,
            short_id: "W1",
            workdir_name: "main",
            branch_name: "main",
            workdir_path: "/mock/git/project",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: false,
            pull_request: null,
          },
          {
            id: workdir2,
            short_id: "W2",
            workdir_name: "feat-ui",
            branch_name: "feat/ui-mock",
            workdir_path: "/mock/git/project-feat-ui",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: true,
            pull_request: {
              number: 77,
              is_draft: true,
              state: "open",
              ci_state: "pending",
              merge_ready: false,
            },
          },
        ],
      },
      {
        id: project2,
        name: "Mock Local Project",
        slug: "mock-local-project",
        path: "/mock/local/project",
        is_git: false,
        expanded: true,
        create_workdir_status: op("idle"),
        workdirs: [
          {
            id: workdir3,
            short_id: "W3",
            workdir_name: "main",
            branch_name: "",
            workdir_path: "/mock/local/project",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: false,
            pull_request: null,
          },
        ],
      },
    ],
    appearance: {
      theme: "system",
      fonts: {
        ui_font: "Inter",
        chat_font: "Inter",
        code_font: "Geist Mono",
        terminal_font: "Geist Mono",
      },
      global_zoom: 1.0,
    },
    agent: {
      codex_enabled: true,
      amp_enabled: true,
      claude_enabled: true,
      default_model_id: "gpt-5",
      default_thinking_effort: "medium",
      default_runner: "codex",
      amp_mode: "default",
    },
    task: {
      prompt_templates: [],
      default_prompt_templates: [],
      system_prompt_templates: [],
      default_system_prompt_templates: [],
    },
    ui: {
      active_workdir_id: workdir1,
      active_task_id: task1,
      open_button_selection: "vscode",
    },
    integrations: {
      telegram: {
        enabled: false,
        has_token: false,
        config_rev: 0,
      },
    },
  }

  const tabs1: WorkspaceTabsSnapshot = { open_tabs: [task1, task9, task7, task4, task2, task8, task5, task6], archived_tabs: [], active_tab: task1 }
  const tabs2: WorkspaceTabsSnapshot = { open_tabs: [task3, task10], archived_tabs: [], active_tab: task3 }
  const tabs3: WorkspaceTabsSnapshot = { open_tabs: [task1, task9, task7, task4, task8, task5, task6], archived_tabs: [], active_tab: task1 }

  const threadsByWorkspace: Record<number, ThreadsSnapshot> = {
    [workdir1]: {
      rev: 1,
      workdir_id: workdir1,
      tabs: tabs1,
      tasks: [
        { task_id: task1, remote_thread_id: null, title: "Mock task 1", created_at_unix_seconds: unixSeconds(-30), updated_at_unix_seconds: unixSeconds(-30), task_status: "todo" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "completed" as TurnResult },
        { task_id: task2, remote_thread_id: null, title: "Mock task 2", created_at_unix_seconds: unixSeconds(-10), updated_at_unix_seconds: unixSeconds(-10), task_status: "backlog" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: null },
        { task_id: task11, remote_thread_id: null, title: "Mock: Long conversation", created_at_unix_seconds: unixSeconds(-2), updated_at_unix_seconds: unixSeconds(-2), task_status: "todo" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "completed" as TurnResult },
        { task_id: task4, remote_thread_id: null, title: "Validating: awaiting feedback", created_at_unix_seconds: unixSeconds(-8), updated_at_unix_seconds: unixSeconds(-8), task_status: "validating" as TaskStatus, turn_status: "awaiting" as TurnStatus, last_turn_result: "completed" as TurnResult },
        { task_id: task5, remote_thread_id: null, title: "Done: completed successfully", created_at_unix_seconds: unixSeconds(-20), updated_at_unix_seconds: unixSeconds(-20), task_status: "done" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "completed" as TurnResult },
        { task_id: task6, remote_thread_id: null, title: "Canceled: aborted by user", created_at_unix_seconds: unixSeconds(-15), updated_at_unix_seconds: unixSeconds(-15), task_status: "canceled" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: null },
        { task_id: task7, remote_thread_id: null, title: "Iterating: queue paused", created_at_unix_seconds: unixSeconds(-12), updated_at_unix_seconds: unixSeconds(-12), task_status: "iterating" as TaskStatus, turn_status: "paused" as TurnStatus, last_turn_result: null },
        { task_id: task8, remote_thread_id: null, title: "Todo: last turn failed", created_at_unix_seconds: unixSeconds(-18), updated_at_unix_seconds: unixSeconds(-18), task_status: "todo" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "failed" as TurnResult },
        { task_id: task9, remote_thread_id: null, title: "Mock: Turn states", created_at_unix_seconds: unixSeconds(-1), updated_at_unix_seconds: unixSeconds(-1), task_status: "todo" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "completed" as TurnResult },
      ],
    },
    [workdir2]: {
      rev: 1,
      workdir_id: workdir2,
      tabs: tabs2,
      tasks: [
        { task_id: task3, remote_thread_id: null, title: "PR: pending", created_at_unix_seconds: unixSeconds(-5), updated_at_unix_seconds: unixSeconds(-5), task_status: "iterating" as TaskStatus, turn_status: "running" as TurnStatus, last_turn_result: null },
        { task_id: task10, remote_thread_id: null, title: "Todo: awaiting ack", created_at_unix_seconds: unixSeconds(-3), updated_at_unix_seconds: unixSeconds(-3), task_status: "todo" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "completed" as TurnResult },
      ],
    },
    [workdir3]: {
      rev: 1,
      workdir_id: workdir3,
      tabs: tabs3,
      tasks: [
        { task_id: task1, remote_thread_id: null, title: "Local task", created_at_unix_seconds: unixSeconds(-120), updated_at_unix_seconds: unixSeconds(-120), task_status: "todo" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "failed" as TurnResult },
        { task_id: task4, remote_thread_id: null, title: "Local: validating", created_at_unix_seconds: unixSeconds(-8), updated_at_unix_seconds: unixSeconds(-8), task_status: "validating" as TaskStatus, turn_status: "awaiting" as TurnStatus, last_turn_result: "completed" as TurnResult },
        { task_id: task5, remote_thread_id: null, title: "Local: done", created_at_unix_seconds: unixSeconds(-20), updated_at_unix_seconds: unixSeconds(-20), task_status: "done" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "completed" as TurnResult },
        { task_id: task6, remote_thread_id: null, title: "Local: canceled", created_at_unix_seconds: unixSeconds(-15), updated_at_unix_seconds: unixSeconds(-15), task_status: "canceled" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: null },
        { task_id: task7, remote_thread_id: null, title: "Local: paused queue", created_at_unix_seconds: unixSeconds(-12), updated_at_unix_seconds: unixSeconds(-12), task_status: "iterating" as TaskStatus, turn_status: "paused" as TurnStatus, last_turn_result: null },
        { task_id: task8, remote_thread_id: null, title: "Local: failed turn", created_at_unix_seconds: unixSeconds(-18), updated_at_unix_seconds: unixSeconds(-18), task_status: "todo" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "failed" as TurnResult },
        { task_id: task9, remote_thread_id: null, title: "Local: turn states", created_at_unix_seconds: unixSeconds(-1), updated_at_unix_seconds: unixSeconds(-1), task_status: "todo" as TaskStatus, turn_status: "idle" as TurnStatus, last_turn_result: "completed" as TurnResult },
      ],
    },
  }

  const conversationsByWorkspaceThread: Record<string, ConversationSnapshot> = {
    [key(workdir1, task1)]: conversationBase({
      workdirId: workdir1,
      taskId: task1,
      title: "Mock task 1",
      taskStatus: "todo",
      runStatus: "running",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-2 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        systemEvent({
          id: "sys_2",
          createdAtUnixMs: unixMs(-2 * 60 * 60 * 1000 + 20_000),
          event: { event_type: "task_status_changed", from: "backlog", to: "todo" },
        }),
        // First user message
        userMessage("Please help me refactor the authentication module."),
        // First agent turn with many activities (should fully collapse between cards)
        agentActivity("reasoning", { text: "Analyzing the authentication module structure" }),
        agentActivity("command_execution", { command: "find src -name '*auth*'", status: "completed", aggregated_output: "src/auth/index.ts\nsrc/auth/jwt.ts" }),
        agentActivity("file_change", { changes: [{ path: "src/auth/index.ts", kind: "update" }] }),
        agentActivity("command_execution", { command: "pnpm run typecheck", status: "completed", aggregated_output: "No errors" }),
        agentActivity("reasoning", { text: "Reviewing the JWT implementation" }),
        agentActivity("file_change", { changes: [{ path: "src/auth/jwt.ts", kind: "update" }] }),
        agentMessage("I've refactored the authentication module. The main changes include:\n\n1. Extracted JWT logic into a separate utility\n2. Added proper error handling\n3. Improved type safety"),
        // Second user message
        userMessage("Can you also add unit tests for the changes?"),
        // Second agent turn with many activities (should fully collapse between cards)
        agentActivity("reasoning", { text: "Planning test coverage for auth module" }),
        agentActivity("file_change", { changes: [{ path: "src/auth/__tests__/index.test.ts", kind: "create" }] }),
        agentActivity("file_change", { changes: [{ path: "src/auth/__tests__/jwt.test.ts", kind: "create" }] }),
        agentActivity("command_execution", { command: "pnpm run test src/auth", status: "completed", aggregated_output: "Test Suites: 2 passed\nTests: 8 passed" }),
        agentActivity("reasoning", { text: "All tests passing, adding edge case tests" }),
        agentActivity("file_change", { changes: [{ path: "src/auth/__tests__/jwt.test.ts", kind: "update" }] }),
        agentActivity("command_execution", { command: "pnpm run test src/auth", status: "completed", aggregated_output: "Test Suites: 2 passed\nTests: 12 passed" }),
        agentMessage("I've added comprehensive unit tests for the authentication module:\n\n- `index.test.ts`: Tests for the main auth flow\n- `jwt.test.ts`: Tests for JWT token handling including edge cases\n\nAll 12 tests are passing."),
        // Third user message (latest turn - should keep last 3 visible)
        userMessage("Great! Now please update the documentation."),
        // Third agent turn - in progress (should keep last 3 visible)
        agentActivity("reasoning", { text: "Reviewing existing documentation" }),
        agentActivity("command_execution", { command: "cat docs/auth.md", status: "completed", aggregated_output: "# Authentication\n..." }),
        agentActivity("file_change", { changes: [{ path: "docs/auth.md", kind: "update" }] }),
        agentActivity("reasoning", { text: "Adding API reference section" }),
        agentActivity("file_change", { changes: [{ path: "docs/api/auth.md", kind: "create" }] }),
        agentActivity("command_execution", { command: "pnpm run docs:build", status: "completed", aggregated_output: "Documentation built successfully" }),
        agentMessage("I've updated the documentation:\n\n1. Updated `docs/auth.md` with the new refactored API\n2. Created `docs/api/auth.md` with detailed API reference\n\nThe documentation has been built successfully."),
        // Tool / activity updates with the same id should collapse to the latest state.
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "tail_dedupe",
            kind: "command_execution",
            payload: { command: "Dedupe update", status: "in_progress", aggregated_output: "" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "tail_dedupe",
            kind: "command_execution",
            payload: { command: "Dedupe update", status: "completed", aggregated_output: "ok" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "tail_progress",
            kind: "command_execution",
            payload: { command: "Progress update 1", status: "completed", aggregated_output: "" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "tail_progress",
            kind: "command_execution",
            payload: { command: "Progress update 2", status: "completed", aggregated_output: "" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "tail_progress",
            kind: "command_execution",
            payload: { command: "Progress update 3", status: "in_progress", aggregated_output: "" },
          },
        },
      ],
    }),
    [key(workdir1, task11)]: conversationBase({
      workdirId: workdir1,
      taskId: task11,
      title: "Mock: Long conversation",
      taskStatus: "todo",
      runStatus: "idle",
      entries: longConversationEntries({ pairs: 320 }),
    }),
    [key(workdir1, task2)]: conversationBase({
      workdirId: workdir1,
      taskId: task2,
      title: "Mock task 2",
      taskStatus: "backlog",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-90 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        userMessage("Hello from mock task 2."),
      ],
    }),
    [key(workdir1, task4)]: conversationBase({
      workdirId: workdir1,
      taskId: task4,
      title: "Validating: awaiting feedback",
      taskStatus: "validating",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-3 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        systemEvent({
          id: "sys_2",
          createdAtUnixMs: unixMs(-3 * 60 * 60 * 1000 + 20_000),
          event: { event_type: "task_status_changed", from: "todo", to: "validating" },
        }),
        userMessage("Please review the changes and suggest improvements."),
        agentActivity("reasoning", { text: "Reviewing the diff and checking for edge cases" }),
        agentActivity("command_execution", { command: "rg -n \"TODO\" web", status: "completed", aggregated_output: "web/lib/mock/fixtures.ts:1:..." }),
        agentMessage("Review completed. Left a few actionable suggestions and questions."),
        agentTurnDuration(18_500),
      ],
    }),
    [key(workdir1, task5)]: conversationBase({
      workdirId: workdir1,
      taskId: task5,
      title: "Done: completed successfully",
      taskStatus: "done",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-6 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        systemEvent({
          id: "sys_2",
          createdAtUnixMs: unixMs(-6 * 60 * 60 * 1000 + 15_000),
          event: { event_type: "task_status_changed", from: "todo", to: "iterating" },
        }),
        systemEvent({
          id: "sys_3",
          createdAtUnixMs: unixMs(-6 * 60 * 60 * 1000 + 60_000),
          event: { event_type: "task_status_changed", from: "iterating", to: "done" },
        }),
        userMessage("Implement the requested change and make sure tests pass."),
        agentActivity("reasoning", { text: "Implementing the change and validating behavior" }),
        agentActivity("file_change", { changes: [{ path: "src/main.rs", kind: "update" }, { path: "src/lib.rs", kind: "update" }] }),
        agentActivity("command_execution", { command: "just fmt && just lint && just test", status: "completed", aggregated_output: "All checks passed" }),
        agentMessage("Implemented the change and verified tests locally."),
        agentTurnDuration(62_000),
      ],
    }),
    [key(workdir1, task6)]: conversationBase({
      workdirId: workdir1,
      taskId: task6,
      title: "Canceled: aborted by user",
      taskStatus: "canceled",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-4 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        systemEvent({
          id: "sys_2",
          createdAtUnixMs: unixMs(-4 * 60 * 60 * 1000 + 10_000),
          event: { event_type: "task_status_changed", from: "todo", to: "iterating" },
        }),
        userMessage("Start the task, but I might cancel it midway."),
        agentActivity("reasoning", { text: "Starting work and preparing a safe plan" }),
        agentActivity("command_execution", { command: "just test-fast", status: "in_progress", aggregated_output: "" }),
        agentTurnCanceled(),
        systemEvent({
          id: "sys_3",
          createdAtUnixMs: unixMs(-4 * 60 * 60 * 1000 + 30_000),
          event: { event_type: "task_status_changed", from: "iterating", to: "canceled" },
        }),
      ],
    }),
    [key(workdir1, task7)]: {
      ...conversationBase({
        workdirId: workdir1,
        taskId: task7,
        title: "Iterating: queue paused",
        taskStatus: "iterating",
        runStatus: "idle",
        entries: [
          systemEvent({
            id: "sys_1",
            createdAtUnixMs: unixMs(-2 * 60 * 60 * 1000),
            event: { event_type: "task_created" },
          }),
          systemEvent({
            id: "sys_2",
            createdAtUnixMs: unixMs(-2 * 60 * 60 * 1000 + 10_000),
            event: { event_type: "task_status_changed", from: "todo", to: "iterating" },
          }),
          userMessage("Queue a few prompts and then pause the queue."),
          agentActivity("todo_list", { items: [{ text: "Analyze", completed: true }, { text: "Implement", completed: false }, { text: "Verify", completed: false }] }),
          agentMessage("Queued work; waiting to resume."),
        ],
      }),
      queue_paused: true,
      pending_prompts: [
        queuedPrompt({ id: 1, text: "Queued prompt A (mock)" }),
        queuedPrompt({ id: 2, text: "Queued prompt B (mock)", attachments: [fileA] }),
      ],
    },
    [key(workdir1, task8)]: conversationBase({
      workdirId: workdir1,
      taskId: task8,
      title: "Todo: last turn failed",
      taskStatus: "todo",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-5 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        userMessage("Try to run the command and handle failures gracefully."),
        agentActivity("command_execution", { command: "just lint", status: "completed", aggregated_output: "error: clippy::some_lint\n..." }),
        agentTurnError("Command failed: clippy reported errors (mock)."),
      ],
    }),
    [key(workdir1, task9)]: conversationBase({
      workdirId: workdir1,
      taskId: task9,
      title: "Mock: Turn states",
      taskStatus: "todo",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-45 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        userMessage("Show a completed turn."),
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "turn_duration_smoke_command",
            kind: "command_execution",
            payload: { command: "rg -n \"FIXME\" -S", status: "in_progress", aggregated_output: "" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(3_200),
          event: {
            type: "item",
            id: "turn_duration_smoke_command",
            kind: "command_execution",
            payload: { command: "rg -n \"FIXME\" -S", status: "completed", aggregated_output: "No matches" },
          },
        },
        agentTurnDuration(2_400),
        agentMessage("Done."),
        userMessage("Show a failed turn."),
        agentActivity("command_execution", { command: "just lint", status: "in_progress", aggregated_output: "" }),
        agentTurnError("mock: command failed: exit status 1"),
        agentMessage("I hit an error and stopped."),
        userMessage("Show a canceled turn."),
        agentActivity("command_execution", { command: "just test", status: "in_progress", aggregated_output: "" }),
        agentTurnCanceled(),
        agentMessage("Canceled as requested."),
      ],
    }),
    [key(workdir2, task3)]: conversationBase({
      workdirId: workdir2,
      taskId: task3,
      title: "PR: pending",
      taskStatus: "iterating",
      runStatus: "running",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-30 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
	        systemEvent({
	          id: "sys_2",
	          createdAtUnixMs: unixMs(-25 * 60 * 1000),
	          event: { event_type: "task_status_changed", from: "backlog", to: "iterating" },
	        }),
	        userMessage("Please open a PR."),
	        agentMessage("Ok. I'll open a PR and share the link."),
	        {
	          type: "agent_event",
	          entry_id: newEntryId("ae"),
            created_at_unix_ms: nextEntryCreatedAtUnixMs(),
	          event: {
            type: "item",
            id: "prog_1",
            kind: "reasoning",
            payload: { text: "Analyzing the codebase structure to understand the project layout" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "prog_2",
            kind: "command_execution",
            payload: { command: "git status", status: "completed", aggregated_output: "On branch main\nnothing to commit" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "prog_3",
            kind: "file_change",
            payload: { changes: [{ path: "src/utils/helpers.ts", kind: "update" }] },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "prog_4",
            kind: "command_execution",
            payload: { command: "pnpm run lint", status: "completed", aggregated_output: "✓ No ESLint warnings or errors" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: { type: "item", id: "prog_5", kind: "web_search", payload: { query: "TypeScript best practices for error handling" } },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "prog_6",
            kind: "file_change",
            payload: { changes: [{ path: "src/lib/api.ts", kind: "update" }, { path: "src/lib/types.ts", kind: "create" }] },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "prog_7",
            kind: "command_execution",
            payload: { command: "pnpm run test", status: "completed", aggregated_output: "Test Suites: 12 passed\nTests: 48 passed" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
          event: {
            type: "item",
            id: "prog_8",
            kind: "reasoning",
            payload: { text: "Preparing the pull request with proper commit message" },
          },
        },
        {
          type: "agent_event",
          entry_id: newEntryId("ae"),
          created_at_unix_ms: nextEntryCreatedAtUnixMs(),
	          event: {
	            type: "item",
	            id: "prog_9",
	            kind: "command_execution",
	            payload: { command: "git add -A && git commit -m 'feat: add new API endpoints'", status: "in_progress" },
	          },
	        },
	        userMessage("Also include tests."),
	      ],
	    }),
    [key(workdir2, task10)]: conversationBase({
      workdirId: workdir2,
      taskId: task10,
      title: "Todo: awaiting ack",
      taskStatus: "todo",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-20 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        userMessage("Please implement the agent status pill in the task list."),
        agentMessage("Done. Please review and acknowledge the result."),
      ],
    }),
    [key(workdir3, task1)]: conversationBase({
      workdirId: workdir3,
      taskId: task1,
      title: "Local task",
      taskStatus: "todo",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-24 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        userMessage("Local project task."),
      ],
    }),
    [key(workdir3, task4)]: conversationBase({
      workdirId: workdir3,
      taskId: task4,
      title: "Local: validating",
      taskStatus: "validating",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-3 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        systemEvent({
          id: "sys_2",
          createdAtUnixMs: unixMs(-3 * 60 * 60 * 1000 + 20_000),
          event: { event_type: "task_status_changed", from: "todo", to: "validating" },
        }),
        userMessage("Please review the local changes."),
        agentActivity("reasoning", { text: "Reviewing local diff" }),
        agentMessage("Review done."),
        agentTurnDuration(9_500),
      ],
    }),
    [key(workdir3, task5)]: conversationBase({
      workdirId: workdir3,
      taskId: task5,
      title: "Local: done",
      taskStatus: "done",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-6 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        systemEvent({
          id: "sys_2",
          createdAtUnixMs: unixMs(-6 * 60 * 60 * 1000 + 60_000),
          event: { event_type: "task_status_changed", from: "iterating", to: "done" },
        }),
        userMessage("Finish the local task."),
        agentActivity("command_execution", { command: "just test-fast", status: "completed", aggregated_output: "ok" }),
        agentMessage("Done."),
        agentTurnDuration(12_000),
      ],
    }),
    [key(workdir3, task6)]: conversationBase({
      workdirId: workdir3,
      taskId: task6,
      title: "Local: canceled",
      taskStatus: "canceled",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-4 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        userMessage("Cancel the local task."),
        agentTurnCanceled(),
      ],
    }),
    [key(workdir3, task7)]: {
      ...conversationBase({
        workdirId: workdir3,
        taskId: task7,
        title: "Local: paused queue",
        taskStatus: "iterating",
        runStatus: "idle",
        entries: [
          systemEvent({
            id: "sys_1",
            createdAtUnixMs: unixMs(-2 * 60 * 60 * 1000),
            event: { event_type: "task_created" },
          }),
          userMessage("Queue a prompt in local project."),
          agentMessage("Queue is paused."),
        ],
      }),
      queue_paused: true,
      pending_prompts: [
        queuedPrompt({ id: 1, text: "Local queued prompt (mock)" }),
      ],
    },
    [key(workdir3, task8)]: conversationBase({
      workdirId: workdir3,
      taskId: task8,
      title: "Local: failed turn",
      taskStatus: "todo",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-5 * 60 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        userMessage("Fail in local project."),
        agentTurnError("mock: local failure"),
      ],
    }),
    [key(workdir3, task9)]: conversationBase({
      workdirId: workdir3,
      taskId: task9,
      title: "Local: turn states",
      taskStatus: "todo",
      runStatus: "idle",
      entries: [
        systemEvent({
          id: "sys_1",
          createdAtUnixMs: unixMs(-45 * 60 * 1000),
          event: { event_type: "task_created" },
        }),
        userMessage("Turn done."),
        agentTurnDuration(1_200),
        agentMessage("Ok."),
        userMessage("Turn error."),
        agentTurnError("mock: error"),
        userMessage("Turn canceled."),
        agentTurnCanceled(),
      ],
    }),
  }

  const files: ChangedFileSnapshot[] = [
    changedFile({
      id: "mock_change_1",
      path: "web/lib/mock/fixtures.ts",
      name: "fixtures.ts",
      status: "modified",
      group: "unstaged",
      additions: 42,
      deletions: 7,
    }),
    changedFile({
      id: "mock_change_2",
      path: "docs/contracts/progress.md",
      name: "progress.md",
      status: "modified",
      group: "staged",
      additions: 10,
      deletions: 2,
    }),
  ]

  const workspaceChangesByWorkspace: Record<number, WorkspaceChangesSnapshot> = {
    [workdir1]: { workdir_id: workdir1, files },
    [workdir2]: { workdir_id: workdir2, files: [] },
    [workdir3]: { workdir_id: workdir3, files: [] },
  }

  const diffFiles: WorkspaceDiffFileSnapshot[] = [
    {
      file: files[0]!,
      old_file: { name: "fixtures.ts", contents: "export const x = 1\n" },
      new_file: { name: "fixtures.ts", contents: "export const x = 2\n" },
    },
  ]

  const workspaceDiffByWorkspace: Record<number, WorkspaceDiffSnapshot> = {
    [workdir1]: { workdir_id: workdir1, files: diffFiles },
    [workdir2]: { workdir_id: workdir2, files: [] },
    [workdir3]: { workdir_id: workdir3, files: [] },
  }

  const codexCustomPrompts: CodexCustomPromptSnapshot[] = [
    {
      id: "templates/fix-bug",
      label: "Fix bug",
      description: "Write a minimal reproduction, then fix it.",
      contents: "# Fix bug\n\n- Repro\n- Fix\n- Tests\n",
    },
  ]

  const mentionIndex: MentionItemSnapshot[] = [
    { id: "file:web/app/page.tsx", name: "page.tsx", path: "web/app/page.tsx", kind: "file" as MentionItemKind },
    { id: "file:web/lib/luban-http.ts", name: "luban-http.ts", path: "web/lib/luban-http.ts", kind: "file" as MentionItemKind },
    { id: "folder:web/lib/mock", name: "mock", path: "web/lib/mock", kind: "folder" as MentionItemKind },
  ]

  const codexConfigTree: CodexConfigEntrySnapshot[] = [
    { path: "prompts", name: "prompts", kind: "folder", children: [{ path: "prompts/default.md", name: "default.md", kind: "file", children: [] }] },
  ]

  const ampConfigTree: AmpConfigEntrySnapshot[] = [
    { path: "config.toml", name: "config.toml", kind: "file", children: [] },
  ]

  const claudeConfigTree: ClaudeConfigEntrySnapshot[] = [
    { path: "claude.yaml", name: "claude.yaml", kind: "file", children: [] },
  ]

  const tasksSnapshot: TasksSnapshot = {
    rev: 1,
    tasks: [
      {
        project_id: project1,
        workdir_id: workdir1,
        task_id: task1,
        title: "Mock task 1",
        created_at_unix_seconds: unixSeconds(-30),
        updated_at_unix_seconds: unixSeconds(-30),
        branch_name: "main",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "todo",
        turn_status: "idle",
        last_turn_result: "completed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir1,
        task_id: task2,
        title: "Mock task 2",
        created_at_unix_seconds: unixSeconds(-10),
        updated_at_unix_seconds: unixSeconds(-10),
        branch_name: "main",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "backlog",
        turn_status: "idle",
        last_turn_result: null,
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir1,
        task_id: task4,
        title: "Validating: awaiting feedback",
        created_at_unix_seconds: unixSeconds(-8),
        updated_at_unix_seconds: unixSeconds(-8),
        branch_name: "main",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "validating",
        turn_status: "awaiting",
        last_turn_result: "completed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir1,
        task_id: task5,
        title: "Done: completed successfully",
        created_at_unix_seconds: unixSeconds(-20),
        updated_at_unix_seconds: unixSeconds(-20),
        branch_name: "main",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "done",
        turn_status: "idle",
        last_turn_result: "completed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir1,
        task_id: task6,
        title: "Canceled: aborted by user",
        created_at_unix_seconds: unixSeconds(-15),
        updated_at_unix_seconds: unixSeconds(-15),
        branch_name: "main",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "canceled",
        turn_status: "idle",
        last_turn_result: null,
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir1,
        task_id: task7,
        title: "Iterating: queue paused",
        created_at_unix_seconds: unixSeconds(-12),
        updated_at_unix_seconds: unixSeconds(-12),
        branch_name: "main",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "iterating",
        turn_status: "paused",
        last_turn_result: null,
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir1,
        task_id: task8,
        title: "Todo: last turn failed",
        created_at_unix_seconds: unixSeconds(-18),
        updated_at_unix_seconds: unixSeconds(-18),
        branch_name: "main",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "todo",
        turn_status: "idle",
        last_turn_result: "failed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir1,
        task_id: task9,
        title: "Mock: Turn states",
        created_at_unix_seconds: unixSeconds(-1),
        updated_at_unix_seconds: unixSeconds(-1),
        branch_name: "main",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "todo",
        turn_status: "idle",
        last_turn_result: "completed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir2,
        task_id: task3,
        title: "PR: pending",
        created_at_unix_seconds: unixSeconds(-5),
        updated_at_unix_seconds: unixSeconds(-5),
        branch_name: "feat/ui-mock",
        workdir_name: "feat-ui",
        agent_run_status: "running",
        has_unread_completion: true,
        task_status: "iterating",
        turn_status: "running",
        last_turn_result: null,
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir2,
        task_id: task10,
        title: "Todo: awaiting ack",
        created_at_unix_seconds: unixSeconds(-3),
        updated_at_unix_seconds: unixSeconds(-3),
        branch_name: "feat/ui-mock",
        workdir_name: "feat-ui",
        agent_run_status: "idle",
        has_unread_completion: true,
        task_status: "todo",
        turn_status: "idle",
        last_turn_result: "completed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project2,
        workdir_id: workdir3,
        task_id: task1,
        title: "Local task",
        created_at_unix_seconds: unixSeconds(-120),
        updated_at_unix_seconds: unixSeconds(-120),
        branch_name: "",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "todo",
        turn_status: "idle",
        last_turn_result: "failed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project2,
        workdir_id: workdir3,
        task_id: task4,
        title: "Local: validating",
        created_at_unix_seconds: unixSeconds(-8),
        updated_at_unix_seconds: unixSeconds(-8),
        branch_name: "",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "validating",
        turn_status: "awaiting",
        last_turn_result: "completed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project2,
        workdir_id: workdir3,
        task_id: task5,
        title: "Local: done",
        created_at_unix_seconds: unixSeconds(-20),
        updated_at_unix_seconds: unixSeconds(-20),
        branch_name: "",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "done",
        turn_status: "idle",
        last_turn_result: "completed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project2,
        workdir_id: workdir3,
        task_id: task6,
        title: "Local: canceled",
        created_at_unix_seconds: unixSeconds(-15),
        updated_at_unix_seconds: unixSeconds(-15),
        branch_name: "",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "canceled",
        turn_status: "idle",
        last_turn_result: null,
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project2,
        workdir_id: workdir3,
        task_id: task7,
        title: "Local: paused queue",
        created_at_unix_seconds: unixSeconds(-12),
        updated_at_unix_seconds: unixSeconds(-12),
        branch_name: "",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "iterating",
        turn_status: "paused",
        last_turn_result: null,
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project2,
        workdir_id: workdir3,
        task_id: task8,
        title: "Local: failed turn",
        created_at_unix_seconds: unixSeconds(-18),
        updated_at_unix_seconds: unixSeconds(-18),
        branch_name: "",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "todo",
        turn_status: "idle",
        last_turn_result: "failed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project2,
        workdir_id: workdir3,
        task_id: task9,
        title: "Local: turn states",
        created_at_unix_seconds: unixSeconds(-1),
        updated_at_unix_seconds: unixSeconds(-1),
        branch_name: "",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
        task_status: "todo",
        turn_status: "idle",
        last_turn_result: "completed",
        is_starred: false,
      } satisfies TaskSummarySnapshot,
    ],
  }

  return {
    app,
    threadsByWorkspace,
    tasksSnapshot,
    conversationsByWorkspaceThread,
    attachmentUrlsById,
    workspaceChangesByWorkspace,
    workspaceDiffByWorkspace,
    codexCustomPrompts,
    mentionIndex,
    codexConfig: { tree: codexConfigTree, files: { "prompts/default.md": "Default prompt\n" } },
    ampConfig: { tree: ampConfigTree, files: { "config.toml": "amp = true\n" } },
    claudeConfig: { tree: claudeConfigTree, files: { "claude.yaml": "claude: true\n" } },
  }
}
