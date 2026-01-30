import type {
  AgentRunnerKind,
  AmpConfigEntrySnapshot,
  AppSnapshot,
  AttachmentKind,
  AttachmentRef,
  ChangedFileSnapshot,
  ClaudeConfigEntrySnapshot,
  CodexConfigEntrySnapshot,
  CodexCustomPromptSnapshot,
  ContextItemSnapshot,
  ConversationEntry,
  ConversationSnapshot,
  FileChangeGroup,
  FileChangeStatus,
  MentionItemKind,
  MentionItemSnapshot,
  OperationStatus,
  ProjectId,
  TaskSummarySnapshot,
  TasksSnapshot,
  ThreadsSnapshot,
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
  contextItemsByWorkspace: Record<number, ContextItemSnapshot[]>
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

function unixMs(offsetMs: number = 0): number {
  return FIXTURE_BASE_UNIX_MS + offsetMs
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

function userMessage(text: string): ConversationEntry {
  return { type: "user_message", text, attachments: [] }
}

function agentMessage(text: string): ConversationEntry {
  return { type: "agent_item", id: `agent_msg_${Math.random().toString(16).slice(2)}`, kind: "agent_message", payload: { text } }
}

function conversationBase(args: {
  workdirId: WorkspaceId
  taskId: WorkspaceThreadId
  title: string
  runner?: AgentRunnerKind
  runStatus?: OperationStatus
  entries: ConversationEntry[]
}): ConversationSnapshot {
  const runner = args.runner ?? "codex"
  return {
    rev: 1,
    workdir_id: args.workdirId,
    task_id: args.taskId,
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
    in_progress_items: [],
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
  }

  const tabs1: WorkspaceTabsSnapshot = { open_tabs: [task1, task2], archived_tabs: [], active_tab: task1 }
  const tabs2: WorkspaceTabsSnapshot = { open_tabs: [task3], archived_tabs: [], active_tab: task3 }
  const tabs3: WorkspaceTabsSnapshot = { open_tabs: [task1], archived_tabs: [], active_tab: task1 }

  const threadsByWorkspace: Record<number, ThreadsSnapshot> = {
    [workdir1]: {
      rev: 1,
      workdir_id: workdir1,
      tabs: tabs1,
      tasks: [
        { task_id: task1, remote_thread_id: null, title: "Mock task 1", updated_at_unix_seconds: unixSeconds(-30) },
        { task_id: task2, remote_thread_id: null, title: "Mock task 2", updated_at_unix_seconds: unixSeconds(-10) },
      ],
    },
    [workdir2]: {
      rev: 1,
      workdir_id: workdir2,
      tabs: tabs2,
      tasks: [{ task_id: task3, remote_thread_id: null, title: "PR: pending", updated_at_unix_seconds: unixSeconds(-5) }],
    },
    [workdir3]: {
      rev: 1,
      workdir_id: workdir3,
      tabs: tabs3,
      tasks: [{ task_id: task1, remote_thread_id: null, title: "Local task", updated_at_unix_seconds: unixSeconds(-120) }],
    },
  }

  const conversationsByWorkspaceThread: Record<string, ConversationSnapshot> = {
    [key(workdir1, task1)]: conversationBase({
      workdirId: workdir1,
      taskId: task1,
      title: "Mock task 1",
      entries: [userMessage("Hello from mock task 1."), agentMessage("Mock agent reply.")],
    }),
    [key(workdir1, task2)]: conversationBase({
      workdirId: workdir1,
      taskId: task2,
      title: "Mock task 2",
      entries: [userMessage("Hello from mock task 2.")],
    }),
    [key(workdir2, task3)]: conversationBase({
      workdirId: workdir2,
      taskId: task3,
      title: "PR: pending",
      entries: [userMessage("Please open a PR."), agentMessage("Working on it.")],
      runStatus: "running",
    }),
    [key(workdir3, task1)]: conversationBase({
      workdirId: workdir3,
      taskId: task1,
      title: "Local task",
      entries: [userMessage("Local project task.")],
    }),
  }

  const contextItemsByWorkspace: Record<number, ContextItemSnapshot[]> = {
    [workdir1]: [
      {
        context_id: 1,
        created_at_unix_ms: unixMs(-60_000),
        attachment: imgA,
      },
      {
        context_id: 2,
        created_at_unix_ms: unixMs(-50_000),
        attachment: fileA,
      },
    ],
    [workdir2]: [],
    [workdir3]: [],
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
        updated_at_unix_seconds: unixSeconds(-30),
        branch_name: "main",
        workdir_name: "main",
        agent_run_status: "idle",
        has_unread_completion: false,
      } satisfies TaskSummarySnapshot,
      {
        project_id: project1,
        workdir_id: workdir2,
        task_id: task3,
        title: "PR: pending",
        updated_at_unix_seconds: unixSeconds(-5),
        branch_name: "feat/ui-mock",
        workdir_name: "feat-ui",
        agent_run_status: "running",
        has_unread_completion: true,
      } satisfies TaskSummarySnapshot,
    ],
  }

  return {
    app,
    threadsByWorkspace,
    tasksSnapshot,
    conversationsByWorkspaceThread,
    contextItemsByWorkspace,
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

