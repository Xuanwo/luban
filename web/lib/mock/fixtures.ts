import type {
  AgentItem,
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
  const workspace4: WorkspaceId = 4
  const workspace5: WorkspaceId = 5
  const workspace6: WorkspaceId = 6
  const workspace7: WorkspaceId = 7
  const workspace8: WorkspaceId = 8
  const workspace9: WorkspaceId = 9
  const workspace10: WorkspaceId = 10
  const workspace11: WorkspaceId = 11

  const thread1: WorkspaceThreadId = 1
  const thread2: WorkspaceThreadId = 2
  const thread3: WorkspaceThreadId = 3
  const thread4: WorkspaceThreadId = 4

  const thread10: WorkspaceThreadId = 10
  const thread11: WorkspaceThreadId = 11

  const project1: ProjectId = "mock-project-1"
  const project2: ProjectId = "mock-project-2"
  const project3: ProjectId = "mock-project-3"
  const project4: ProjectId = "mock-project-4"

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
          {
            id: workspace4,
            short_id: "W4",
            workspace_name: "agent-running",
            branch_name: "agent/running",
            worktree_path: "/mock/git/project-agent-running",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("running"),
            has_unread_completion: false,
            pull_request: null,
          },
          {
            id: workspace5,
            short_id: "W5",
            workspace_name: "ci-failure",
            branch_name: "ci/failure",
            worktree_path: "/mock/git/project-ci-failure",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: false,
            pull_request: {
              number: 13,
              is_draft: false,
              state: "open",
              ci_state: "failure",
              merge_ready: false,
            },
          },
          {
            id: workspace6,
            short_id: "W6",
            workspace_name: "ci-success",
            branch_name: "ci/success",
            worktree_path: "/mock/git/project-ci-success",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: false,
            pull_request: {
              number: 14,
              is_draft: false,
              state: "open",
              ci_state: "success",
              merge_ready: false,
            },
          },
          {
            id: workspace7,
            short_id: "W7",
            workspace_name: "ci-unknown",
            branch_name: "ci/unknown",
            worktree_path: "/mock/git/project-ci-unknown",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: false,
            pull_request: {
              number: 15,
              is_draft: false,
              state: "open",
              ci_state: null,
              merge_ready: false,
            },
          },
          {
            id: workspace8,
            short_id: "W8",
            workspace_name: "pr-merged",
            branch_name: "pr/merged",
            worktree_path: "/mock/git/project-pr-merged",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: false,
            pull_request: {
              number: 16,
              is_draft: false,
              state: "merged",
              ci_state: "success",
              merge_ready: true,
            },
          },
          {
            id: workspace9,
            short_id: "W9",
            workspace_name: "pr-closed",
            branch_name: "pr/closed",
            worktree_path: "/mock/git/project-pr-closed",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: false,
            pull_request: {
              number: 17,
              is_draft: false,
              state: "closed",
              ci_state: "failure",
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
      {
        id: project3,
        name: "Mock Local Project (Running)",
        slug: "mock-local-project-running",
        path: "/mock/local/project-running",
        is_git: false,
        expanded: true,
        create_workspace_status: op("idle"),
        workspaces: [
          {
            id: workspace10,
            short_id: "W10",
            workspace_name: "main",
            branch_name: "",
            worktree_path: "/mock/local/project-running",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("running"),
            has_unread_completion: false,
            pull_request: null,
          },
        ],
      },
      {
        id: project4,
        name: "Mock Local Project (Pending)",
        slug: "mock-local-project-pending",
        path: "/mock/local/project-pending",
        is_git: false,
        expanded: true,
        create_workspace_status: op("idle"),
        workspaces: [
          {
            id: workspace11,
            short_id: "W11",
            workspace_name: "main",
            branch_name: "",
            worktree_path: "/mock/local/project-pending",
            status: "active",
            archive_status: op("idle"),
            branch_rename_status: op("idle"),
            agent_run_status: op("idle"),
            has_unread_completion: true,
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
      active_workspace_id: workspace1,
      active_thread_id: thread1,
      open_button_selection: "vscode",
    },
  }

  const threadsByWorkspace: Record<number, ThreadsSnapshot> = {
    [workspace1]: {
      rev: 1,
      workspace_id: workspace1,
      tabs: workspaceTabs({ open: [thread1, thread2, thread3, thread4], active: thread1 }),
      threads: [
        { thread_id: thread1, remote_thread_id: null, title: "Running: full coverage", updated_at_unix_seconds: unixSeconds(0) },
        { thread_id: thread2, remote_thread_id: null, title: "Running: with queue", updated_at_unix_seconds: unixSeconds(-30) },
        { thread_id: thread3, remote_thread_id: null, title: "Canceled", updated_at_unix_seconds: unixSeconds(-60) },
        { thread_id: thread4, remote_thread_id: null, title: "Completed", updated_at_unix_seconds: unixSeconds(-90) },
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
        { thread_id: thread1, remote_thread_id: null, title: "Local idle", updated_at_unix_seconds: unixSeconds(-60) },
      ],
    },
    [workspace4]: {
      rev: 1,
      workspace_id: workspace4,
      tabs: workspaceTabs({ open: [thread1], active: thread1 }),
      threads: [{ thread_id: thread1, remote_thread_id: null, title: "Agent running", updated_at_unix_seconds: unixSeconds(-10) }],
    },
    [workspace5]: {
      rev: 1,
      workspace_id: workspace5,
      tabs: workspaceTabs({ open: [thread1], active: thread1 }),
      threads: [{ thread_id: thread1, remote_thread_id: null, title: "PR: CI failure", updated_at_unix_seconds: unixSeconds(-20) }],
    },
    [workspace6]: {
      rev: 1,
      workspace_id: workspace6,
      tabs: workspaceTabs({ open: [thread1], active: thread1 }),
      threads: [{ thread_id: thread1, remote_thread_id: null, title: "PR: CI passed", updated_at_unix_seconds: unixSeconds(-25) }],
    },
    [workspace7]: {
      rev: 1,
      workspace_id: workspace7,
      tabs: workspaceTabs({ open: [thread1], active: thread1 }),
      threads: [{ thread_id: thread1, remote_thread_id: null, title: "PR: CI unknown", updated_at_unix_seconds: unixSeconds(-26) }],
    },
    [workspace8]: {
      rev: 1,
      workspace_id: workspace8,
      tabs: workspaceTabs({ open: [thread1], active: thread1 }),
      threads: [{ thread_id: thread1, remote_thread_id: null, title: "PR: merged", updated_at_unix_seconds: unixSeconds(-27) }],
    },
    [workspace9]: {
      rev: 1,
      workspace_id: workspace9,
      tabs: workspaceTabs({ open: [thread1], active: thread1 }),
      threads: [{ thread_id: thread1, remote_thread_id: null, title: "PR: closed", updated_at_unix_seconds: unixSeconds(-28) }],
    },
    [workspace10]: {
      rev: 1,
      workspace_id: workspace10,
      tabs: workspaceTabs({ open: [thread1], active: thread1 }),
      threads: [{ thread_id: thread1, remote_thread_id: null, title: "Local running", updated_at_unix_seconds: unixSeconds(-50) }],
    },
    [workspace11]: {
      rev: 1,
      workspace_id: workspace11,
      tabs: workspaceTabs({ open: [thread1], active: thread1 }),
      threads: [{ thread_id: thread1, remote_thread_id: null, title: "Local pending (extra)", updated_at_unix_seconds: unixSeconds(-55) }],
    },
  }

	  const conversationBase = (args: {
	    workspaceId: WorkspaceId
	    threadId: WorkspaceThreadId
	    title: string
	    entries: ConversationEntry[]
	    agentRunner?: AgentRunnerKind
	    agentModelId?: string
	    thinkingEffort?: "minimal" | "low" | "medium" | "high" | "xhigh"
	    ampMode?: string | null
	  }): ConversationSnapshot => ({
	    rev: 1,
	    workspace_id: args.workspaceId,
	    thread_id: args.threadId,
	    agent_runner: args.agentRunner ?? "codex",
	    agent_model_id: args.agentModelId ?? "gpt-5",
	    thinking_effort: args.thinkingEffort ?? "medium",
	    amp_mode: args.ampMode ?? null,
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
    [key(workspace1, thread1)]: {
      ...conversationBase({
        workspaceId: workspace1,
        threadId: thread1,
        title: "Running: full coverage",
        entries: [
          { type: "user_message", text: "Start a running turn that covers all message types.", attachments: [] },
          { type: "agent_item", id: "running_done_msg_1", kind: "agent_message", payload: { text: "Working on it." } },
          { type: "agent_item", id: "running_done_reasoning_1", kind: "reasoning", payload: { text: "Plan: gather context, run checks, then apply a minimal patch." } },
          {
            type: "agent_item",
            id: "running_done_cmd_1",
            kind: "command_execution",
            payload: { command: "zsh -lc \"rg -n \\\"mock mode\\\" -S web\"", aggregated_output: "web/lib/mock/fixtures.ts:...", status: "done" },
          },
          {
            type: "agent_item",
            id: "running_done_search_1",
            kind: "web_search",
            payload: { query: "Next.js mock mode fixtures best practices" },
          },
          {
            type: "agent_item",
            id: "running_done_mcp_1",
            kind: "mcp_tool_call",
            payload: {
              server: "fs",
              tool: "read_file",
              arguments: { path: "README.md" },
              result: { ok: true },
              error: null,
              status: "done",
            },
          },
          {
            type: "agent_item",
            id: "running_done_todo_1",
            kind: "todo_list",
            payload: { items: [{ completed: true, text: "Add mock fixtures" }, { completed: false, text: "Verify UI states" }] },
          },
          {
            type: "agent_item",
            id: "running_done_files_1",
            kind: "file_change",
            payload: { changes: [{ kind: "update", path: "web/lib/mock/fixtures.ts" }, { kind: "add", path: "web/tests/e2e/new.spec.ts" }] },
          },
          { type: "agent_item", id: "running_done_error_item_1", kind: "error", payload: { message: "Non-fatal error: transient network failure." } },
          { type: "turn_error", message: "Turn failed: command returned a non-zero exit code." },
          { type: "turn_duration", duration_ms: 894 },
          { type: "turn_usage", usage_json: null },
          { type: "user_message", text: "Continue with a second completed round.", attachments: [imgA, fileA] },
          { type: "agent_item", id: "running_done_msg_2", kind: "agent_message", payload: { text: "Acknowledged. Continuing." } },
          { type: "agent_item", id: "running_done_cmd_2", kind: "command_execution", payload: { command: "zsh -lc \"just fmt\"", aggregated_output: "Formatting...", status: "done" } },
        ],
      }),
      run_status: op("running"),
      run_started_at_unix_ms: unixMs(-18_000),
      run_finished_at_unix_ms: null,
      in_progress_items: [
        { id: "loop_agent_message_1", kind: "agent_message", payload: { text: "Streaming output..." } },
        { id: "loop_reasoning_1", kind: "reasoning", payload: { text: "Thinking about edge cases." } },
        {
          id: "loop_cmd_1",
          kind: "command_execution",
          payload: { command: "zsh -lc \"just test\"", aggregated_output: "Running tests...", status: "in_progress" },
        },
        { id: "loop_file_change_1", kind: "file_change", payload: { changes: [{ kind: "update", path: "web/lib/mock/fixtures.ts" }] } },
        {
          id: "loop_mcp_1",
          kind: "mcp_tool_call",
          payload: { server: "git", tool: "status", arguments: {}, result: null, error: null, status: "in_progress" },
        },
        { id: "loop_search_1", kind: "web_search", payload: { query: "GitHub PR status mapping ci_state merge_ready" } },
        { id: "loop_todo_1", kind: "todo_list", payload: { items: [{ completed: false, text: "Update fixtures" }] } },
        { id: "loop_error_1", kind: "error", payload: { message: "Retrying after a temporary failure." } },
      ],
    },
	    [key(workspace1, thread2)]: {
	      ...conversationBase({
	        workspaceId: workspace1,
	        threadId: thread2,
	        title: "Running: with queue",
	        entries: [
	          { type: "user_message", text: "Queue a few prompts while running.", attachments: [] },
	          { type: "agent_item", id: "queue_running_msg_1", kind: "agent_message", payload: { text: "Processing current prompt; others are queued." } },
	        ],
	      }),
	      run_status: op("running"),
	      run_started_at_unix_ms: unixMs(-12_000),
	      run_finished_at_unix_ms: null,
	      pending_prompts: [
	        {
	          id: 1,
	          text: "First queued prompt.",
	          attachments: [],
	          run_config: { runner: "codex", model_id: "gpt-5", thinking_effort: "medium" },
	        },
	        {
	          id: 2,
	          text: "Second queued prompt.",
	          attachments: [fileA],
	          run_config: { runner: "codex", model_id: "gpt-5", thinking_effort: "medium" },
	        },
	      ],
	    },
    [key(workspace1, thread3)]: {
      ...conversationBase({
        workspaceId: workspace1,
        threadId: thread3,
        title: "Canceled",
        entries: [
          { type: "user_message", text: "Start then cancel.", attachments: [] },
          {
            type: "agent_item",
            id: "canceled_cmd_1",
            kind: "command_execution",
            payload: { command: "zsh -lc \"sleep 10\"", aggregated_output: "Canceled by user.", status: "done" },
          },
          { type: "turn_canceled" },
        ],
      }),
      run_status: op("idle"),
      run_started_at_unix_ms: unixMs(-25_000),
      run_finished_at_unix_ms: unixMs(-24_000),
    },
    [key(workspace1, thread4)]: conversationBase({
      workspaceId: workspace1,
      threadId: thread4,
      title: "Completed",
      entries: [
        { type: "user_message", text: "Show all supported message types in a completed conversation.", attachments: [] },
        { type: "agent_item", id: "completed_msg_1", kind: "agent_message", payload: { text: "Done." } },
        { type: "agent_item", id: "completed_reasoning_1", kind: "reasoning", payload: { text: "This is a completed message containing reasoning." } },
        { type: "agent_item", id: "completed_search_1", kind: "web_search", payload: { query: "Example web search query" } },
        {
          type: "agent_item",
          id: "completed_mcp_1",
          kind: "mcp_tool_call",
          payload: {
            server: "fs",
            tool: "read_file",
            arguments: { path: "web/lib/luban-api.ts" },
            result: null,
            error: { message: "File not found" },
            status: "done",
          },
        },
        {
          type: "agent_item",
          id: "completed_cmd_1",
          kind: "command_execution",
          payload: { command: "zsh -lc \"echo ok\"", aggregated_output: "ok", status: "done" },
        },
        { type: "agent_item", id: "completed_files_1", kind: "file_change", payload: { changes: [{ kind: "add", path: "notes.txt" }] } },
        { type: "agent_item", id: "completed_todo_1", kind: "todo_list", payload: { items: [{ completed: true, text: "Ship" }] } },
        { type: "agent_item", id: "completed_error_item_1", kind: "error", payload: { message: "A handled error occurred." } },
        { type: "turn_error", message: "Turn failed: provider returned an error." },
        { type: "turn_duration", duration_ms: 1234 },
        { type: "turn_usage", usage_json: null },
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
        {
          type: "agent_item",
          id: "seed_agent_message_6",
          kind: "agent_message",
          payload: { text: "Run `just test` to ensure contracts progress covers server routes." },
        },
      ],
    }),
    [key(workspace4, thread1)]: {
      ...conversationBase({
        workspaceId: workspace4,
        threadId: thread1,
        title: "Agent running",
        entries: [
          { type: "user_message", text: "Agent status example (running).", attachments: [] },
          agentMessage("This workspace row should show agent running."),
        ],
      }),
      run_status: op("running"),
      run_started_at_unix_ms: unixMs(-40_000),
      run_finished_at_unix_ms: null,
      in_progress_items: [
        {
          id: "agent_running_cmd_1",
          kind: "command_execution",
          payload: { command: "zsh -lc \"echo hello\"", aggregated_output: "hello", status: "in_progress" },
        },
      ],
    },
    [key(workspace5, thread1)]: conversationBase({
      workspaceId: workspace5,
      threadId: thread1,
      title: "PR: CI failure",
      entries: [
        { type: "user_message", text: "PR status example (CI failed).", attachments: [] },
        agentMessage("This workspace row should show CI failed."),
      ],
    }),
    [key(workspace6, thread1)]: conversationBase({
      workspaceId: workspace6,
      threadId: thread1,
      title: "PR: CI passed",
      entries: [
        { type: "user_message", text: "PR status example (CI passed).", attachments: [] },
        agentMessage("This workspace row should show CI passed (not merge-ready)."),
      ],
    }),
    [key(workspace7, thread1)]: conversationBase({
      workspaceId: workspace7,
      threadId: thread1,
      title: "PR: CI unknown",
      entries: [
        { type: "user_message", text: "PR status example (CI unknown / running).", attachments: [] },
        agentMessage("This workspace row should show CI running."),
      ],
    }),
    [key(workspace8, thread1)]: conversationBase({
      workspaceId: workspace8,
      threadId: thread1,
      title: "PR: merged",
      entries: [
        { type: "user_message", text: "PR status example (merged).", attachments: [] },
        agentMessage("This workspace row should show merged / done."),
      ],
    }),
    [key(workspace9, thread1)]: conversationBase({
      workspaceId: workspace9,
      threadId: thread1,
      title: "PR: closed",
      entries: [
        { type: "user_message", text: "PR status example (closed).", attachments: [] },
        agentMessage("This workspace row should not show an open PR status."),
      ],
    }),
    [key(workspace3, thread1)]: conversationBase({
      workspaceId: workspace3,
      threadId: thread1,
      title: "Local idle",
      entries: [
        { type: "user_message", text: "Non-git project idle state.", attachments: [] },
        agentMessage("This project should show agent idle."),
      ],
    }),
    [key(workspace10, thread1)]: {
      ...conversationBase({
        workspaceId: workspace10,
        threadId: thread1,
        title: "Local running",
        entries: [
          { type: "user_message", text: "Non-git project running state.", attachments: [] },
          agentMessage("This project should show agent running."),
        ],
      }),
      run_status: op("running"),
      run_started_at_unix_ms: unixMs(-55_000),
      run_finished_at_unix_ms: null,
      in_progress_items: [
        {
          id: "local_in_progress_cmd_1",
          kind: "command_execution",
          payload: { command: "zsh -lc \"ls\"", aggregated_output: "", status: "in_progress" },
        },
      ],
    },
    [key(workspace11, thread1)]: conversationBase({
      workspaceId: workspace11,
      threadId: thread1,
      title: "Local pending (extra)",
      entries: [
        { type: "user_message", text: "Non-git project pending state (extra).", attachments: [] },
        agentMessage("This project should show agent pending (awaiting read)."),
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

  const claudeConfigTree: ClaudeConfigEntrySnapshot[] = [
    {
      path: "settings.json",
      name: "settings.json",
      kind: "file",
      children: [],
    },
    {
      path: "history.jsonl",
      name: "history.jsonl",
      kind: "file",
      children: [],
    },
  ]

  const claudeConfigFiles: Record<string, string> = {
    "settings.json": "{\n  \"permissions\": {\n    \"allow\": []\n  }\n}\n",
    "history.jsonl": "{\"type\":\"mock\"}\n",
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
    claudeConfig: { tree: claudeConfigTree, files: claudeConfigFiles },
  }
}
