import type {
  AgentItem,
  AmpConfigEntrySnapshot,
  AppSnapshot,
  AttachmentKind,
  AttachmentRef,
  ChangedFileSnapshot,
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

let fixtureIdSeq = 0

function nextFixtureId(prefix: string): string {
  fixtureIdSeq += 1
  return `${prefix}_${fixtureIdSeq}`
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

function agentMessage(text: string): AgentItem {
  return { id: nextFixtureId("agent_message"), kind: "agent_message", payload: { text } }
}

function reasoning(text: string): AgentItem {
  return { id: nextFixtureId("reasoning"), kind: "reasoning", payload: { text } }
}

function commandExecution(command: string, aggregatedOutput: string): AgentItem {
  return {
    id: nextFixtureId("cmd"),
    kind: "command_execution",
    payload: { command, aggregated_output: aggregatedOutput, status: "done" },
  }
}

function fileChange(paths: string[]): AgentItem {
  return {
    id: nextFixtureId("file_change"),
    kind: "file_change",
    payload: { changes: paths.map((path) => ({ kind: "update", path })) },
  }
}

function changedFile(args: {
  id: string
  path: string
  name: string
  status: FileChangeStatus
  group: FileChangeGroup
  additions?: number | null
  deletions?: number | null
  oldPath?: string | null
}): ChangedFileSnapshot {
  return {
    id: args.id,
    path: args.path,
    name: args.name,
    status: args.status,
    group: args.group,
    additions: args.additions ?? null,
    deletions: args.deletions ?? null,
    old_path: args.oldPath ?? null,
  }
}

function workspaceTabs(args: { open: number[]; archived?: number[]; active: number }): WorkspaceTabsSnapshot {
  return {
    open_tabs: args.open,
    archived_tabs: args.archived ?? [],
    active_tab: args.active,
  }
}

function op(status: OperationStatus): OperationStatus {
  return status
}

function key(workspaceId: WorkspaceId, threadId: WorkspaceThreadId): string {
  return `${workspaceId}:${threadId}`
}

export function defaultMockFixtures(): MockFixtures {
  const workspace1: WorkspaceId = 1
  const workspace2: WorkspaceId = 2
  const workspace3: WorkspaceId = 3

  const thread1: WorkspaceThreadId = 1
  const thread2: WorkspaceThreadId = 2
  const thread3: WorkspaceThreadId = 3

  const thread10: WorkspaceThreadId = 10
  const thread11: WorkspaceThreadId = 11

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
        create_workspace_status: op("idle"),
        workspaces: [
          {
            id: workspace1,
            short_id: "W1",
            workspace_name: "main",
            branch_name: "main",
            worktree_path: "/mock/git/project",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: false,
            pull_request: {
              number: 42,
              is_draft: false,
              state: "open",
              ci_state: "success",
              merge_ready: true,
            },
          },
          {
            id: workspace2,
            short_id: "W2",
            workspace_name: "feat-ui",
            branch_name: "feat/ui-mock",
            worktree_path: "/mock/git/project-feat-ui",
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
        create_workspace_status: op("idle"),
        workspaces: [
          {
            id: workspace3,
            short_id: "W3",
            workspace_name: "main",
            branch_name: "",
            worktree_path: "/mock/local/project",
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
      active_workspace_id: workspace1,
      active_thread_id: thread1,
      open_button_selection: "vscode",
    },
  }

  const threadsByWorkspace: Record<number, ThreadsSnapshot> = {
    [workspace1]: {
      rev: 1,
      workspace_id: workspace1,
      tabs: workspaceTabs({ open: [thread1, thread2, thread3], active: thread1 }),
      threads: [
        { thread_id: thread1, remote_thread_id: null, title: "Design iteration", updated_at_unix_seconds: unixSeconds(0) },
        { thread_id: thread2, remote_thread_id: null, title: "Bugfix notes", updated_at_unix_seconds: unixSeconds(-3600) },
        { thread_id: thread3, remote_thread_id: null, title: "Release checklist", updated_at_unix_seconds: unixSeconds(-7200) },
      ],
    },
    [workspace2]: {
      rev: 1,
      workspace_id: workspace2,
      tabs: workspaceTabs({ open: [thread10, thread11], active: thread10 }),
      threads: [
        { thread_id: thread10, remote_thread_id: null, title: "UI mock mode", updated_at_unix_seconds: unixSeconds(-120) },
        { thread_id: thread11, remote_thread_id: null, title: "Contracts", updated_at_unix_seconds: unixSeconds(-540) },
      ],
    },
    [workspace3]: {
      rev: 1,
      workspace_id: workspace3,
      tabs: workspaceTabs({ open: [thread1], active: thread1 }),
      threads: [
        { thread_id: thread1, remote_thread_id: null, title: "Local workspace", updated_at_unix_seconds: unixSeconds(-60) },
      ],
    },
  }

  const conversationBase = (args: {
    workspaceId: WorkspaceId
    threadId: WorkspaceThreadId
    title: string
    entries: ConversationEntry[]
    agentModelId?: string
    thinkingEffort?: "minimal" | "low" | "medium" | "high" | "xhigh"
  }): ConversationSnapshot => ({
    rev: 1,
    workspace_id: args.workspaceId,
    thread_id: args.threadId,
    agent_model_id: args.agentModelId ?? "gpt-5",
    thinking_effort: args.thinkingEffort ?? "medium",
    run_status: op("idle"),
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
  })

  const conversationsByWorkspaceThread: Record<string, ConversationSnapshot> = {
    [key(workspace1, thread1)]: conversationBase({
      workspaceId: workspace1,
      threadId: thread1,
      title: "Design iteration",
      entries: [
        { type: "user_message", text: "Hello from mock mode.", attachments: [] },
        { type: "agent_item", id: "seed_agent_message_1", kind: "agent_message", payload: { text: "This is mock mode. UI and interaction changes should iterate here." } },
        { type: "agent_item", id: "seed_reasoning_1", kind: "reasoning", payload: { text: "Contracts keep the server aligned while UI iterates quickly." } },
        { type: "agent_item", id: "seed_cmd_1", kind: "command_execution", payload: { command: "just web dev-mock", aggregated_output: "Starting web dev server in mock mode...", status: "done" } },
        { type: "user_message", text: "Can you show attachments?", attachments: [imgA, fileA] },
        { type: "agent_item", id: "seed_agent_message_2", kind: "agent_message", payload: { text: "Attachments should render using object URLs in mock mode." } },
      ],
    }),
    [key(workspace1, thread2)]: conversationBase({
      workspaceId: workspace1,
      threadId: thread2,
      title: "Bugfix notes",
      entries: [
        { type: "user_message", text: "Repro steps: ...", attachments: [] },
        { type: "agent_item", id: "seed_agent_message_3", kind: "agent_message", payload: { text: "Mock data is deterministic so UI tests remain stable." } },
      ],
    }),
    [key(workspace1, thread3)]: conversationBase({
      workspaceId: workspace1,
      threadId: thread3,
      title: "Release checklist",
      entries: [
        { type: "user_message", text: "Checklist item 1", attachments: [] },
        { type: "agent_item", id: "seed_agent_message_4", kind: "agent_message", payload: { text: "- Verify\n- Package\n- Ship" } },
      ],
    }),
    [key(workspace2, thread10)]: conversationBase({
      workspaceId: workspace2,
      threadId: thread10,
      title: "UI mock mode",
      entries: [
        { type: "user_message", text: "Test streaming UI.", attachments: [] },
        { type: "agent_item", id: "seed_agent_message_5", kind: "agent_message", payload: { text: "Streaming is simulated with a short delay in mock mode." } },
      ],
    }),
    [key(workspace2, thread11)]: conversationBase({
      workspaceId: workspace2,
      threadId: thread11,
      title: "Contracts",
      entries: [
        { type: "user_message", text: "Update contracts when changing /api routes.", attachments: [] },
        { type: "agent_item", id: "seed_agent_message_6", kind: "agent_message", payload: { text: "Run `just test` to ensure contracts progress covers server routes." } },
      ],
    }),
  }

  const contextItemsByWorkspace: Record<number, ContextItemSnapshot[]> = {
    [workspace1]: [
      {
        context_id: 1,
        created_at_unix_ms: unixMs(-60_000),
        attachment: imgA,
      },
    ],
    [workspace2]: [],
    [workspace3]: [],
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
    changedFile({
      id: "mock_change_3",
      path: "README.old.md",
      name: "README.old.md",
      status: "renamed",
      group: "committed",
      additions: null,
      deletions: null,
      oldPath: "README.md",
    }),
  ]

  const workspaceChangesByWorkspace: Record<number, WorkspaceChangesSnapshot> = {
    [workspace1]: { workspace_id: workspace1, files },
    [workspace2]: { workspace_id: workspace2, files: [] },
    [workspace3]: { workspace_id: workspace3, files: [] },
  }

  const diffFiles: WorkspaceDiffFileSnapshot[] = [
    {
      file: files[0]!,
      old_file: { name: "fixtures.ts", contents: "export const x = 1\n" },
      new_file: { name: "fixtures.ts", contents: "export const x = 2\n" },
    },
    {
      file: files[1]!,
      old_file: { name: "progress.md", contents: "# Contract Progress Tracker\n" },
      new_file: { name: "progress.md", contents: "# Contract Progress Tracker\n\nUpdated\n" },
    },
  ]

  const workspaceDiffByWorkspace: Record<number, WorkspaceDiffSnapshot> = {
    [workspace1]: { workspace_id: workspace1, files: diffFiles },
    [workspace2]: { workspace_id: workspace2, files: [] },
    [workspace3]: { workspace_id: workspace3, files: [] },
  }

  const codexCustomPrompts: CodexCustomPromptSnapshot[] = [
    {
      id: "templates/fix-bug",
      label: "Fix bug",
      description: "Write a minimal reproduction, then fix it.",
      contents: "# Fix bug\n\n- Repro\n- Fix\n- Tests\n",
    },
    {
      id: "templates/implement-feature",
      label: "Implement feature",
      description: "Implement the feature with tests.",
      contents: "# Implement feature\n\n- Plan\n- Implement\n- Verify\n",
    },
  ]

  const mentionIndex: MentionItemSnapshot[] = [
    { id: "file:web/app/page.tsx", name: "page.tsx", path: "web/app/page.tsx", kind: "file" as MentionItemKind },
    { id: "file:web/lib/luban-http.ts", name: "luban-http.ts", path: "web/lib/luban-http.ts", kind: "file" as MentionItemKind },
    { id: "folder:web/lib/mock", name: "mock", path: "web/lib/mock", kind: "folder" as MentionItemKind },
    { id: "file:docs/contracts/progress.md", name: "progress.md", path: "docs/contracts/progress.md", kind: "file" as MentionItemKind },
  ]

  const codexConfigTree: CodexConfigEntrySnapshot[] = [
    {
      path: "prompts",
      name: "prompts",
      kind: "folder",
      children: [
        { path: "prompts/default.md", name: "default.md", kind: "file", children: [] },
        { path: "prompts/agent.md", name: "agent.md", kind: "file", children: [] },
      ],
    },
    {
      path: "config.json",
      name: "config.json",
      kind: "file",
      children: [],
    },
  ]

  const codexConfigFiles: Record<string, string> = {
    "prompts/default.md": "# Default prompt\n\nBe concise.\n",
    "prompts/agent.md": "# Agent prompt\n\nFollow repository conventions.\n",
    "config.json": "{\n  \"model\": \"gpt-5\"\n}\n",
  }

  const ampConfigTree: AmpConfigEntrySnapshot[] = [
    {
      path: "config",
      name: "config",
      kind: "folder",
      children: [
        { path: "config/default.json", name: "default.json", kind: "file", children: [] },
        { path: "config/agents.json", name: "agents.json", kind: "file", children: [] },
      ],
    },
    {
      path: "README.md",
      name: "README.md",
      kind: "file",
      children: [],
    },
  ]

  const ampConfigFiles: Record<string, string> = {
    "config/default.json": "{\n  \"amp_mode\": \"default\"\n}\n",
    "config/agents.json": "{\n  \"agents\": []\n}\n",
    "README.md": "# Amp config\n\nThis is mock content.\n",
  }

  return {
    app,
    threadsByWorkspace,
    conversationsByWorkspaceThread,
    contextItemsByWorkspace,
    attachmentUrlsById,
    workspaceChangesByWorkspace,
    workspaceDiffByWorkspace,
    codexCustomPrompts,
    mentionIndex,
    codexConfig: { tree: codexConfigTree, files: codexConfigFiles },
    ampConfig: { tree: ampConfigTree, files: ampConfigFiles },
  }
}
