export type ProjectId = string
export type WorkspaceId = number
export type WorkspaceThreadId = number

export type WorkspaceStatus = "active" | "archived"

export type AppearanceTheme = "light" | "dark" | "system"

export type AppearanceFontsSnapshot = {
  ui_font: string
  chat_font: string
  code_font: string
  terminal_font: string
}

export type AppearanceSnapshot = {
  theme: AppearanceTheme
  fonts: AppearanceFontsSnapshot
  global_zoom: number
}

export type AgentRunnerKind = "codex" | "amp" | "claude"

export type AgentSettingsSnapshot = {
  codex_enabled: boolean
  amp_enabled: boolean
  claude_enabled: boolean
  default_model_id?: string
  default_thinking_effort?: ThinkingEffort
  default_runner?: AgentRunnerKind
  amp_mode?: string
}

export type TaskPromptTemplateSnapshot = {
  intent_kind: TaskIntentKind
  template: string
}

export type SystemTaskKind = "infer-type" | "rename-branch" | "auto-title-thread"

export type SystemPromptTemplateSnapshot = {
  kind: SystemTaskKind
  template: string
}

export type TaskSettingsSnapshot = {
  prompt_templates: TaskPromptTemplateSnapshot[]
  default_prompt_templates: TaskPromptTemplateSnapshot[]
  system_prompt_templates: SystemPromptTemplateSnapshot[]
  default_system_prompt_templates: SystemPromptTemplateSnapshot[]
}

export type AppSnapshot = {
  rev: number
  projects: ProjectSnapshot[]
  appearance: AppearanceSnapshot
  agent: AgentSettingsSnapshot
  task: TaskSettingsSnapshot
  ui: UiSnapshot
}

export type UiSnapshot = {
  active_workdir_id?: WorkspaceId
  active_task_id?: WorkspaceThreadId
  open_button_selection?: string
  sidebar_project_order?: ProjectId[]
}

export type ProjectSnapshot = {
  id: ProjectId
  name: string
  slug: string
  path: string
  is_git: boolean
  expanded: boolean
  create_workdir_status: OperationStatus
  workdirs: WorkspaceSnapshot[]
}

export type WorkspaceSnapshot = {
  id: WorkspaceId
  short_id: string
  workdir_name: string
  branch_name: string
  workdir_path: string
  status: WorkspaceStatus
  archive_status: OperationStatus
  branch_rename_status: OperationStatus
  agent_run_status: OperationStatus
  has_unread_completion: boolean
  pull_request: PullRequestSnapshot | null
}

export type FileChangeStatus = "modified" | "added" | "deleted" | "renamed"

export type FileChangeGroup = "committed" | "staged" | "unstaged"

export type ChangedFileSnapshot = {
  id: string
  path: string
  name: string
  status: FileChangeStatus
  group: FileChangeGroup
  additions: number | null
  deletions: number | null
  old_path: string | null
}

export type WorkspaceChangesSnapshot = {
  workdir_id: WorkspaceId
  files: ChangedFileSnapshot[]
}

export type DiffFileContents = {
  name: string
  contents: string
}

export type WorkspaceDiffFileSnapshot = {
  file: ChangedFileSnapshot
  old_file: DiffFileContents
  new_file: DiffFileContents
}

export type WorkspaceDiffSnapshot = {
  workdir_id: WorkspaceId
  files: WorkspaceDiffFileSnapshot[]
}

export type PullRequestState = "open" | "closed" | "merged"

export type PullRequestCiState = "pending" | "success" | "failure"

export type PullRequestSnapshot = {
  number: number
  is_draft: boolean
  state: PullRequestState
  ci_state: PullRequestCiState | null
  merge_ready: boolean
}

export type ThreadsSnapshot = {
  rev: number
  workdir_id: WorkspaceId
  tabs: WorkspaceTabsSnapshot
  tasks: ThreadMeta[]
}

export type TaskSummarySnapshot = {
  project_id: ProjectId
  workdir_id: WorkspaceId
  task_id: WorkspaceThreadId
  title: string
  updated_at_unix_seconds: number
  branch_name: string
  workdir_name: string
  agent_run_status: OperationStatus
  has_unread_completion: boolean
}

export type TasksSnapshot = {
  rev: number
  tasks: TaskSummarySnapshot[]
}

export type WorkspaceTabsSnapshot = {
  open_tabs: WorkspaceThreadId[]
  archived_tabs: WorkspaceThreadId[]
  active_tab: WorkspaceThreadId
}

export type ThreadMeta = {
  task_id: WorkspaceThreadId
  remote_thread_id: string | null
  title: string
  updated_at_unix_seconds: number
}

export type AttachmentKind = "image" | "text" | "file"

export type AttachmentRef = {
  id: string
  kind: AttachmentKind
  name: string
  extension: string
  mime: string | null
  byte_len: number
}

export type ContextItemSnapshot = {
  context_id: number
  attachment: AttachmentRef
  created_at_unix_ms: number
}

export type ContextSnapshot = {
  workdir_id: WorkspaceId
  items: ContextItemSnapshot[]
}

export type ConversationSnapshot = {
  rev: number
  workdir_id: WorkspaceId
  task_id: WorkspaceThreadId
  agent_runner: AgentRunnerKind
  agent_model_id: string
  thinking_effort: ThinkingEffort
  amp_mode?: string | null
  run_status: OperationStatus
  run_started_at_unix_ms?: number | null
  run_finished_at_unix_ms?: number | null
  entries: ConversationEntry[]
  entries_total?: number
  entries_start?: number
  entries_truncated?: boolean
  in_progress_items: AgentItem[]
  pending_prompts: QueuedPromptSnapshot[]
  queue_paused: boolean
  remote_thread_id: string | null
  title: string
}

export type AgentRunConfigSnapshot = {
  runner: AgentRunnerKind
  model_id: string
  thinking_effort: ThinkingEffort
  amp_mode?: string | null
}

export type QueuedPromptSnapshot = {
  id: number
  text: string
  attachments: AttachmentRef[]
  run_config: AgentRunConfigSnapshot
}

export type OperationStatus = "idle" | "running"

export type ThinkingEffort = "minimal" | "low" | "medium" | "high" | "xhigh"

export type OpenTarget = "vscode" | "cursor" | "zed" | "ghostty" | "finder"

export type TaskIntentKind =
  | "fix"
  | "implement"
  | "review"
  | "discuss"
  | "other"

export type TaskRepoInfo = {
  full_name: string
  url: string
  default_branch: string | null
}

export type TaskIssueInfo = {
  number: number
  title: string
  url: string
}

export type FeedbackType = "bug" | "feature" | "question"

export type FeedbackSubmitAction = "create_issue" | "fix_it"

export type FeedbackSubmitResult = {
  issue: TaskIssueInfo
  task: TaskExecuteResult | null
}

export type TaskPullRequestInfo = {
  number: number
  title: string
  url: string
  head_ref: string | null
  base_ref: string | null
  mergeable: string | null
}

export type TaskProjectSpec =
  | { type: "unspecified" }
  | { type: "local_path"; path: string }
  | { type: "git_hub_repo"; full_name: string }

export type TaskDraft = {
  input: string
  project: TaskProjectSpec
  intent_kind: TaskIntentKind
  summary: string
  prompt: string
  repo: TaskRepoInfo | null
  issue: TaskIssueInfo | null
  pull_request: TaskPullRequestInfo | null
}

export type TaskExecuteMode = "create" | "start"

export type TaskExecuteResult = {
  project_id: ProjectId
  workdir_id: WorkspaceId
  task_id: WorkspaceThreadId
  workdir_path: string
  prompt: string
  mode: TaskExecuteMode
}

export type CodexConfigEntryKind = "file" | "folder"

export type CodexConfigEntrySnapshot = {
  path: string
  name: string
  kind: CodexConfigEntryKind
  children: CodexConfigEntrySnapshot[]
}

export type AmpConfigEntryKind = "file" | "folder"

export type AmpConfigEntrySnapshot = {
  path: string
  name: string
  kind: AmpConfigEntryKind
  children: AmpConfigEntrySnapshot[]
}

export type ClaudeConfigEntryKind = "file" | "folder"

export type ClaudeConfigEntrySnapshot = {
  path: string
  name: string
  kind: ClaudeConfigEntryKind
  children: ClaudeConfigEntrySnapshot[]
}

export type AgentItemKind =
  | "agent_message"
  | "reasoning"
  | "command_execution"
  | "file_change"
  | "mcp_tool_call"
  | "web_search"
  | "todo_list"
  | "error"

export type AgentItem = {
  id: string
  kind: AgentItemKind
  payload: unknown
}

export type ConversationEntry =
  | { type: "user_message"; text: string; attachments: AttachmentRef[] }
  | { type: "agent_item"; id: string; kind: AgentItemKind; payload: unknown }
  | { type: "turn_usage"; usage_json: unknown | null }
  | { type: "turn_duration"; duration_ms: number }
  | { type: "turn_canceled" }
  | { type: "turn_error"; message: string }

export type ClientAction =
  | { type: "pick_project_path" }
  | { type: "add_project"; path: string }
  | { type: "add_project_and_open"; path: string }
  | { type: "task_preview"; input: string }
  | { type: "task_execute"; draft: TaskDraft; mode: TaskExecuteMode; workdir_id: WorkspaceId }
  | {
      type: "feedback_submit"
      title: string
      body: string
      labels: string[]
      feedback_type: FeedbackType
      action: FeedbackSubmitAction
    }
  | { type: "delete_project"; project_id: ProjectId }
  | { type: "toggle_project_expanded"; project_id: ProjectId }
  | { type: "create_workdir"; project_id: ProjectId }
  | { type: "ensure_main_workdir"; project_id: ProjectId }
  | { type: "open_workdir"; workdir_id: WorkspaceId }
  | { type: "open_workdir_in_ide"; workdir_id: WorkspaceId }
  | { type: "open_workdir_with"; workdir_id: WorkspaceId; target: OpenTarget }
  | { type: "open_workdir_pull_request"; workdir_id: WorkspaceId }
  | { type: "open_workdir_pull_request_failed_action"; workdir_id: WorkspaceId }
  | { type: "archive_workdir"; workdir_id: WorkspaceId }
  | { type: "chat_model_changed"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId; model_id: string }
  | { type: "chat_runner_changed"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId; runner: AgentRunnerKind }
  | { type: "chat_amp_mode_changed"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId; amp_mode: string }
  | {
      type: "thinking_effort_changed"
      workdir_id: WorkspaceId
      task_id: WorkspaceThreadId
      thinking_effort: ThinkingEffort
    }
  | {
      type: "send_agent_message"
      workdir_id: WorkspaceId
      task_id: WorkspaceThreadId
      text: string
      attachments: AttachmentRef[]
      runner?: AgentRunnerKind
      amp_mode?: string
    }
  | {
      type: "cancel_and_send_agent_message"
      workdir_id: WorkspaceId
      task_id: WorkspaceThreadId
      text: string
      attachments: AttachmentRef[]
      runner?: AgentRunnerKind
      amp_mode?: string
    }
  | {
      type: "queue_agent_message"
      workdir_id: WorkspaceId
      task_id: WorkspaceThreadId
      text: string
      attachments: AttachmentRef[]
      runner?: AgentRunnerKind
      amp_mode?: string
    }
  | { type: "remove_queued_prompt"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId; prompt_id: number }
  | { type: "reorder_queued_prompt"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId; active_id: number; over_id: number }
  | {
      type: "update_queued_prompt"
      workdir_id: WorkspaceId
      task_id: WorkspaceThreadId
      prompt_id: number
      text: string
      attachments: AttachmentRef[]
      model_id: string
      thinking_effort: ThinkingEffort
    }
  | { type: "workdir_rename_branch"; workdir_id: WorkspaceId; branch_name: string }
  | { type: "workdir_ai_rename_branch"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId }
  | { type: "cancel_agent_turn"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId }
  | { type: "create_task"; workdir_id: WorkspaceId }
  | { type: "activate_task"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId }
  | { type: "close_task_tab"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId }
  | { type: "restore_task_tab"; workdir_id: WorkspaceId; task_id: WorkspaceThreadId }
  | {
      type: "reorder_task_tab"
      workdir_id: WorkspaceId
      task_id: WorkspaceThreadId
      to_index: number
    }
  | { type: "open_button_selection_changed"; selection: string }
  | { type: "sidebar_project_order_changed"; project_ids: ProjectId[] }
  | { type: "appearance_theme_changed"; theme: AppearanceTheme }
  | { type: "appearance_fonts_changed"; fonts: AppearanceFontsSnapshot }
  | { type: "appearance_global_zoom_changed"; zoom: number }
  | { type: "codex_enabled_changed"; enabled: boolean }
  | { type: "amp_enabled_changed"; enabled: boolean }
  | { type: "claude_enabled_changed"; enabled: boolean }
  | { type: "agent_runner_changed"; runner: AgentRunnerKind }
  | { type: "agent_amp_mode_changed"; mode: string }
  | { type: "task_prompt_template_changed"; intent_kind: TaskIntentKind; template: string }
  | { type: "system_prompt_template_changed"; kind: SystemTaskKind; template: string }
  | { type: "codex_check" }
  | { type: "codex_config_tree" }
  | { type: "codex_config_list_dir"; path: string }
  | { type: "codex_config_read_file"; path: string }
  | { type: "codex_config_write_file"; path: string; contents: string }
  | { type: "amp_check" }
  | { type: "amp_config_tree" }
  | { type: "amp_config_list_dir"; path: string }
  | { type: "amp_config_read_file"; path: string }
  | { type: "amp_config_write_file"; path: string; contents: string }
  | { type: "claude_check" }
  | { type: "claude_config_tree" }
  | { type: "claude_config_list_dir"; path: string }
  | { type: "claude_config_read_file"; path: string }
  | { type: "claude_config_write_file"; path: string; contents: string }

export type ServerEvent =
  | { type: "app_changed"; rev: number; snapshot: AppSnapshot }
  | { type: "workdir_tasks_changed"; workdir_id: WorkspaceId; tabs: WorkspaceTabsSnapshot; tasks: ThreadMeta[] }
  | { type: "conversation_changed"; snapshot: ConversationSnapshot }
  | { type: "toast"; message: string }
  | { type: "project_path_picked"; request_id: string; path: string | null }
  | { type: "add_project_and_open_ready"; request_id: string; project_id: ProjectId; workdir_id: WorkspaceId }
  | { type: "task_preview_ready"; request_id: string; draft: TaskDraft }
  | { type: "task_executed"; request_id: string; result: TaskExecuteResult }
  | { type: "feedback_submitted"; request_id: string; result: FeedbackSubmitResult }
  | { type: "codex_check_ready"; request_id: string; ok: boolean; message: string | null }
  | { type: "codex_config_tree_ready"; request_id: string; tree: CodexConfigEntrySnapshot[] }
  | {
      type: "codex_config_list_dir_ready"
      request_id: string
      path: string
      entries: CodexConfigEntrySnapshot[]
    }
  | { type: "codex_config_file_ready"; request_id: string; path: string; contents: string }
  | { type: "codex_config_file_saved"; request_id: string; path: string }
  | { type: "amp_check_ready"; request_id: string; ok: boolean; message: string | null }
  | { type: "amp_config_tree_ready"; request_id: string; tree: AmpConfigEntrySnapshot[] }
  | { type: "amp_config_list_dir_ready"; request_id: string; path: string; entries: AmpConfigEntrySnapshot[] }
  | { type: "amp_config_file_ready"; request_id: string; path: string; contents: string }
  | { type: "amp_config_file_saved"; request_id: string; path: string }
  | { type: "claude_check_ready"; request_id: string; ok: boolean; message: string | null }
  | { type: "claude_config_tree_ready"; request_id: string; tree: ClaudeConfigEntrySnapshot[] }
  | { type: "claude_config_list_dir_ready"; request_id: string; path: string; entries: ClaudeConfigEntrySnapshot[] }
  | { type: "claude_config_file_ready"; request_id: string; path: string; contents: string }
  | { type: "claude_config_file_saved"; request_id: string; path: string }

export type WsClientMessage =
  | { type: "hello"; protocol_version: number; last_seen_rev: number | null }
  | { type: "action"; request_id: string; action: ClientAction }
  | { type: "ping" }

export type WsServerMessage =
  | { type: "hello"; protocol_version: number; current_rev: number }
  | { type: "ack"; request_id: string; rev: number }
  | { type: "event"; rev: number; event: ServerEvent }
  | { type: "error"; request_id: string | null; message: string }
  | { type: "pong" }

export type MentionItemKind = "file" | "folder"

export type MentionItemSnapshot = {
  id: string
  name: string
  path: string
  kind: MentionItemKind
}

export type CodexCustomPromptSnapshot = {
  id: string
  label: string
  description: string
  contents: string
}
