use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub String);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceThreadId(pub u64);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub rev: u64,
    pub projects: Vec<ProjectSnapshot>,
    pub appearance: AppearanceSnapshot,
    #[serde(default)]
    pub agent: AgentSettingsSnapshot,
    #[serde(default)]
    pub task: TaskSettingsSnapshot,
    #[serde(default)]
    pub ui: UiSnapshot,
    #[serde(default)]
    pub integrations: IntegrationsSnapshot,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IntegrationsSnapshot {
    #[serde(default)]
    pub telegram: TelegramIntegrationSnapshot,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelegramIntegrationSnapshot {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub has_token: bool,
    #[serde(default)]
    pub bot_username: Option<String>,
    #[serde(default)]
    pub paired_chat_id: Option<i64>,
    #[serde(default)]
    pub config_rev: u64,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UiSnapshot {
    #[serde(default)]
    #[serde(rename = "active_workdir_id", alias = "active_workspace_id")]
    pub active_workspace_id: Option<WorkspaceId>,
    #[serde(default)]
    #[serde(rename = "active_task_id", alias = "active_thread_id")]
    pub active_thread_id: Option<WorkspaceThreadId>,
    #[serde(default)]
    pub open_button_selection: Option<String>,
    #[serde(default)]
    pub sidebar_project_order: Vec<ProjectId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppearanceSnapshot {
    pub theme: AppearanceTheme,
    pub fonts: AppearanceFontsSnapshot,
    #[serde(default = "default_global_zoom")]
    pub global_zoom: f64,
}

fn default_global_zoom() -> f64 {
    1.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentSettingsSnapshot {
    pub codex_enabled: bool,
    pub amp_enabled: bool,
    pub claude_enabled: bool,
    #[serde(default = "default_true")]
    pub droid_enabled: bool,
    #[serde(default)]
    pub default_model_id: Option<String>,
    #[serde(default)]
    pub runner_default_models: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub default_thinking_effort: Option<ThinkingEffort>,
    #[serde(default)]
    pub default_runner: Option<AgentRunnerKind>,
    #[serde(default)]
    pub amp_mode: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for AgentSettingsSnapshot {
    fn default() -> Self {
        Self {
            codex_enabled: true,
            amp_enabled: true,
            claude_enabled: true,
            droid_enabled: true,
            default_model_id: None,
            runner_default_models: std::collections::HashMap::new(),
            default_thinking_effort: None,
            default_runner: None,
            amp_mode: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunnerKind {
    Codex,
    Amp,
    Claude,
    Droid,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TaskSettingsSnapshot {
    #[serde(default)]
    pub prompt_templates: Vec<TaskPromptTemplateSnapshot>,
    #[serde(default)]
    pub default_prompt_templates: Vec<TaskPromptTemplateSnapshot>,
    #[serde(default)]
    pub system_prompt_templates: Vec<SystemPromptTemplateSnapshot>,
    #[serde(default)]
    pub default_system_prompt_templates: Vec<SystemPromptTemplateSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskPromptTemplateSnapshot {
    pub intent_kind: TaskIntentKind,
    pub template: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemTaskKind {
    InferType,
    RenameBranch,
    AutoTitleThread,
    AutoUpdateTaskStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemPromptTemplateSnapshot {
    pub kind: SystemTaskKind,
    pub template: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexConfigEntryKind {
    File,
    Folder,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodexConfigEntrySnapshot {
    pub path: String,
    pub name: String,
    pub kind: CodexConfigEntryKind,
    #[serde(default)]
    pub children: Vec<CodexConfigEntrySnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmpConfigEntryKind {
    File,
    Folder,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmpConfigEntrySnapshot {
    pub path: String,
    pub name: String,
    pub kind: AmpConfigEntryKind,
    #[serde(default)]
    pub children: Vec<AmpConfigEntrySnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeConfigEntryKind {
    File,
    Folder,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaudeConfigEntrySnapshot {
    pub path: String,
    pub name: String,
    pub kind: ClaudeConfigEntryKind,
    #[serde(default)]
    pub children: Vec<ClaudeConfigEntrySnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroidConfigEntryKind {
    File,
    Folder,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DroidConfigEntrySnapshot {
    pub path: String,
    pub name: String,
    pub kind: DroidConfigEntryKind,
    #[serde(default)]
    pub children: Vec<DroidConfigEntrySnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceTheme {
    Light,
    Dark,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppearanceFontsSnapshot {
    pub ui_font: String,
    pub chat_font: String,
    pub code_font: String,
    pub terminal_font: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub id: ProjectId,
    pub name: String,
    pub slug: String,
    pub path: String,
    #[serde(default)]
    pub is_git: bool,
    pub expanded: bool,
    #[serde(rename = "create_workdir_status", alias = "create_workspace_status")]
    pub create_workspace_status: OperationStatus,
    #[serde(rename = "workdirs", alias = "workspaces")]
    pub workspaces: Vec<WorkspaceSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub id: WorkspaceId,
    pub short_id: String,
    #[serde(rename = "workdir_name", alias = "workspace_name")]
    pub workspace_name: String,
    pub branch_name: String,
    #[serde(rename = "workdir_path", alias = "worktree_path")]
    pub worktree_path: String,
    pub status: WorkspaceStatus,
    pub archive_status: OperationStatus,
    pub branch_rename_status: OperationStatus,
    pub agent_run_status: OperationStatus,
    pub has_unread_completion: bool,
    pub pull_request: Option<PullRequestSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeGroup {
    Committed,
    Staged,
    Unstaged,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangedFileSnapshot {
    pub id: String,
    pub path: String,
    pub name: String,
    pub status: FileChangeStatus,
    pub group: FileChangeGroup,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub old_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceChangesSnapshot {
    #[serde(rename = "workdir_id", alias = "workspace_id")]
    pub workspace_id: WorkspaceId,
    pub files: Vec<ChangedFileSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffFileContents {
    pub name: String,
    pub contents: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceDiffFileSnapshot {
    pub file: ChangedFileSnapshot,
    pub old_file: DiffFileContents,
    pub new_file: DiffFileContents,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceDiffSnapshot {
    #[serde(rename = "workdir_id", alias = "workspace_id")]
    pub workspace_id: WorkspaceId,
    pub files: Vec<WorkspaceDiffFileSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestCiState {
    Pending,
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestSnapshot {
    pub number: u64,
    pub is_draft: bool,
    pub state: PullRequestState,
    pub ci_state: Option<PullRequestCiState>,
    pub merge_ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStatus {
    Active,
    Archived,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationSnapshot {
    pub rev: u64,
    #[serde(rename = "workdir_id", alias = "workspace_id")]
    pub workspace_id: WorkspaceId,
    #[serde(rename = "task_id", alias = "thread_id")]
    pub thread_id: WorkspaceThreadId,
    #[serde(default)]
    pub task_status: TaskStatus,
    pub agent_runner: AgentRunnerKind,
    pub agent_model_id: String,
    pub thinking_effort: ThinkingEffort,
    #[serde(default)]
    pub amp_mode: Option<String>,
    pub run_status: OperationStatus,
    #[serde(default)]
    pub run_started_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub run_finished_at_unix_ms: Option<u64>,
    pub entries: Vec<ConversationEntry>,
    #[serde(default)]
    pub entries_total: u64,
    #[serde(default)]
    pub entries_start: u64,
    #[serde(default)]
    pub entries_truncated: bool,
    #[serde(default)]
    pub pending_prompts: Vec<QueuedPromptSnapshot>,
    #[serde(default)]
    pub queue_paused: bool,
    pub remote_thread_id: Option<String>,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedPromptSnapshot {
    pub id: u64,
    pub text: String,
    pub attachments: Vec<AttachmentRef>,
    pub run_config: AgentRunConfigSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRunConfigSnapshot {
    pub runner: AgentRunnerKind,
    pub model_id: String,
    pub thinking_effort: ThinkingEffort,
    #[serde(default)]
    pub amp_mode: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Idle,
    Running,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Backlog,
    #[default]
    Todo,
    #[serde(alias = "in_progress")]
    Iterating,
    #[serde(alias = "in_review")]
    Validating,
    Done,
    Canceled,
}

#[cfg(test)]
mod task_status_tests {
    use super::TaskStatus;

    #[test]
    fn task_status_roundtrips_with_current_values() {
        let json = serde_json::to_string(&TaskStatus::Iterating).expect("serialize");
        assert_eq!(json, "\"iterating\"");

        let parsed: TaskStatus = serde_json::from_str("\"validating\"").expect("deserialize");
        assert_eq!(parsed, TaskStatus::Validating);
    }

    #[test]
    fn task_status_deserialize_accepts_legacy_aliases() {
        let parsed: TaskStatus = serde_json::from_str("\"in_progress\"").expect("deserialize");
        assert_eq!(parsed, TaskStatus::Iterating);

        let parsed: TaskStatus = serde_json::from_str("\"in_review\"").expect("deserialize");
        assert_eq!(parsed, TaskStatus::Validating);
    }
}

#[cfg(test)]
mod conversation_system_event_tests {
    use super::ConversationSystemEvent;

    #[test]
    fn conversation_system_event_archived_roundtrips() {
        let json =
            serde_json::to_string(&ConversationSystemEvent::TaskArchived).expect("serialize");
        assert_eq!(json, "{\"event_type\":\"task_archived\"}");

        let parsed: ConversationSystemEvent =
            serde_json::from_str("{\"event_type\":\"task_archived\"}").expect("deserialize");
        assert!(matches!(parsed, ConversationSystemEvent::TaskArchived));
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    #[default]
    Idle,
    Running,
    Awaiting,
    Paused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnResult {
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingEffort {
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenTarget {
    Vscode,
    Cursor,
    Zed,
    Ghostty,
    Finder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskIntentKind {
    Fix,
    Implement,
    Review,
    Discuss,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskIssueInfo {
    pub number: u64,
    pub title: String,
    pub url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    Bug,
    Feature,
    Question,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackSubmitAction {
    CreateIssue,
    FixIt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeedbackSubmitResult {
    pub issue: TaskIssueInfo,
    pub task: Option<TaskExecuteResult>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecuteMode {
    Create,
    Start,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskExecuteResult {
    pub project_id: ProjectId,
    #[serde(rename = "workdir_id", alias = "workspace_id")]
    pub workspace_id: WorkspaceId,
    #[serde(rename = "task_id", alias = "thread_id")]
    pub thread_id: WorkspaceThreadId,
    #[serde(rename = "workdir_path", alias = "worktree_path")]
    pub worktree_path: String,
    pub prompt: String,
    pub mode: TaskExecuteMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadsSnapshot {
    pub rev: u64,
    #[serde(rename = "workdir_id", alias = "workspace_id")]
    pub workspace_id: WorkspaceId,
    #[serde(default)]
    pub tabs: WorkspaceTabsSnapshot,
    #[serde(rename = "tasks", alias = "threads")]
    pub threads: Vec<ThreadMeta>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskSummarySnapshot {
    pub project_id: ProjectId,
    #[serde(rename = "workdir_id", alias = "workspace_id")]
    pub workspace_id: WorkspaceId,
    #[serde(rename = "task_id", alias = "thread_id")]
    pub thread_id: WorkspaceThreadId,
    pub title: String,
    #[serde(default)]
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    pub branch_name: String,
    #[serde(rename = "workdir_name", alias = "workspace_name")]
    pub workspace_name: String,
    pub agent_run_status: OperationStatus,
    pub has_unread_completion: bool,
    #[serde(default)]
    pub task_status: TaskStatus,
    #[serde(default)]
    pub turn_status: TurnStatus,
    #[serde(default)]
    pub last_turn_result: Option<TurnResult>,
    #[serde(default)]
    pub is_starred: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TasksSnapshot {
    pub rev: u64,
    #[serde(default)]
    pub tasks: Vec<TaskSummarySnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceTabsSnapshot {
    pub open_tabs: Vec<WorkspaceThreadId>,
    pub archived_tabs: Vec<WorkspaceThreadId>,
    pub active_tab: WorkspaceThreadId,
}

impl Default for WorkspaceTabsSnapshot {
    fn default() -> Self {
        Self {
            open_tabs: Vec::new(),
            archived_tabs: Vec::new(),
            active_tab: WorkspaceThreadId(1),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationEntry {
    SystemEvent(ConversationSystemEventEntry),
    UserEvent(UserEventEntry),
    AgentEvent(AgentEventEntry),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationSystemEventEntry {
    #[serde(rename = "entry_id", alias = "id")]
    pub entry_id: String,
    pub created_at_unix_ms: u64,
    pub event: ConversationSystemEvent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ConversationSystemEvent {
    TaskCreated,
    TaskArchived,
    TaskStatusChanged {
        from: TaskStatus,
        to: TaskStatus,
    },
    TaskStatusSuggestion {
        from: TaskStatus,
        to: TaskStatus,
        #[serde(default)]
        title: String,
        #[serde(default)]
        explanation_markdown: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserEventEntry {
    #[serde(default)]
    pub entry_id: String,
    #[serde(default)]
    pub created_at_unix_ms: u64,
    pub event: UserEvent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserEvent {
    Message(UserMessage),
    TerminalCommandStarted(TerminalCommandStarted),
    TerminalCommandFinished(TerminalCommandFinished),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserMessage {
    pub text: String,
    pub attachments: Vec<AttachmentRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalCommandStarted {
    pub id: String,
    pub command: String,
    pub reconnect: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalCommandFinished {
    pub id: String,
    pub command: String,
    pub reconnect: String,
    #[serde(default)]
    pub output_base64: String,
    #[serde(default)]
    pub output_byte_len: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentEventEntry {
    #[serde(default)]
    pub entry_id: String,
    #[serde(default)]
    pub created_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<AgentRunnerKind>,
    pub event: AgentEvent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    Message(AgentMessage),
    Item(AgentItem),
    TurnUsage {
        usage_json: Option<serde_json::Value>,
    },
    TurnDuration {
        duration_ms: u64,
    },
    TurnCanceled,
    TurnError {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttachmentRef {
    pub id: String,
    pub kind: AttachmentKind,
    pub name: String,
    pub extension: String,
    pub mime: Option<String>,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    Text,
    File,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextItemSnapshot {
    pub context_id: u64,
    pub attachment: AttachmentRef,
    pub created_at_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextSnapshot {
    #[serde(rename = "workdir_id", alias = "workspace_id")]
    pub workspace_id: WorkspaceId,
    pub items: Vec<ContextItemSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewTaskDraftSnapshot {
    pub id: String,
    pub text: String,
    pub project_id: Option<ProjectId>,
    #[serde(rename = "workdir_id", alias = "workspace_id")]
    pub workspace_id: Option<WorkspaceId>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewTaskDraftsSnapshot {
    pub drafts: Vec<NewTaskDraftSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewTaskStashSnapshot {
    pub text: String,
    pub project_id: Option<ProjectId>,
    #[serde(rename = "workdir_id", alias = "workspace_id")]
    pub workspace_id: Option<WorkspaceId>,
    pub editing_draft_id: Option<String>,
    pub updated_at_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewTaskStashResponse {
    pub stash: Option<NewTaskStashSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentionItemKind {
    File,
    Folder,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MentionItemSnapshot {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: MentionItemKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodexCustomPromptSnapshot {
    pub id: String,
    pub label: String,
    pub description: String,
    pub contents: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentItem {
    pub id: String,
    pub kind: AgentItemKind,
    pub payload: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentItemKind {
    Reasoning,
    CommandExecution,
    FileChange,
    McpToolCall,
    WebSearch,
    TodoList,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsClientMessage {
    Hello {
        protocol_version: u32,
        last_seen_rev: Option<u64>,
    },
    Action {
        request_id: String,
        action: Box<ClientAction>,
    },
    Ping,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsServerMessage {
    Hello {
        protocol_version: u32,
        current_rev: u64,
    },
    Ack {
        request_id: String,
        rev: u64,
    },
    Event {
        rev: u64,
        event: Box<ServerEvent>,
    },
    Error {
        request_id: Option<String>,
        message: String,
    },
    Pong,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientAction {
    PickProjectPath,
    AddProject {
        path: String,
    },
    AddProjectAndOpen {
        path: String,
    },
    TaskExecute {
        prompt: String,
        mode: TaskExecuteMode,
        #[serde(default, rename = "workdir_id", alias = "workspace_id")]
        workdir_id: Option<WorkspaceId>,
        #[serde(default)]
        attachments: Vec<AttachmentRef>,
        #[serde(default)]
        model_id: Option<String>,
        #[serde(default)]
        thinking_effort: Option<ThinkingEffort>,
    },
    TelegramBotTokenSet {
        token: String,
    },
    TelegramBotTokenClear,
    TelegramPairStart,
    TelegramUnpair,
    TaskStarSet {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        starred: bool,
    },
    TaskStatusSet {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        task_status: TaskStatus,
    },
    FeedbackSubmit {
        title: String,
        body: String,
        #[serde(default)]
        labels: Vec<String>,
        feedback_type: FeedbackType,
        action: FeedbackSubmitAction,
    },
    DeleteProject {
        project_id: ProjectId,
    },
    ToggleProjectExpanded {
        project_id: ProjectId,
    },
    #[serde(rename = "create_workdir", alias = "create_workspace")]
    CreateWorkspace {
        project_id: ProjectId,
    },
    #[serde(rename = "open_workdir", alias = "open_workspace")]
    OpenWorkspace {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
    },
    #[serde(rename = "open_workdir_in_ide", alias = "open_workspace_in_ide")]
    OpenWorkspaceInIde {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
    },
    #[serde(rename = "open_workdir_with", alias = "open_workspace_with")]
    OpenWorkspaceWith {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        target: OpenTarget,
    },
    #[serde(
        rename = "open_workdir_pull_request",
        alias = "open_workspace_pull_request"
    )]
    OpenWorkspacePullRequest {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
    },
    #[serde(
        rename = "open_workdir_pull_request_failed_action",
        alias = "open_workspace_pull_request_failed_action"
    )]
    OpenWorkspacePullRequestFailedAction {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
    },
    #[serde(rename = "archive_workdir", alias = "archive_workspace")]
    ArchiveWorkspace {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
    },
    #[serde(rename = "ensure_main_workdir", alias = "ensure_main_workspace")]
    EnsureMainWorkspace {
        project_id: ProjectId,
    },
    ChatModelChanged {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        model_id: String,
    },
    ChatRunnerChanged {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        runner: AgentRunnerKind,
    },
    ChatAmpModeChanged {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        amp_mode: String,
    },
    ThinkingEffortChanged {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        thinking_effort: ThinkingEffort,
    },
    TerminalCommandStart {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        command: String,
    },
    SendAgentMessage {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        text: String,
        attachments: Vec<AttachmentRef>,
        #[serde(default)]
        runner: Option<AgentRunnerKind>,
        #[serde(default)]
        amp_mode: Option<String>,
    },
    CancelAndSendAgentMessage {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        text: String,
        attachments: Vec<AttachmentRef>,
        #[serde(default)]
        runner: Option<AgentRunnerKind>,
        #[serde(default)]
        amp_mode: Option<String>,
    },
    QueueAgentMessage {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        text: String,
        attachments: Vec<AttachmentRef>,
        #[serde(default)]
        runner: Option<AgentRunnerKind>,
        #[serde(default)]
        amp_mode: Option<String>,
    },
    RemoveQueuedPrompt {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        prompt_id: u64,
    },
    ReorderQueuedPrompt {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        active_id: u64,
        over_id: u64,
    },
    UpdateQueuedPrompt {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        prompt_id: u64,
        text: String,
        attachments: Vec<AttachmentRef>,
        model_id: String,
        thinking_effort: ThinkingEffort,
    },
    #[serde(rename = "workdir_rename_branch", alias = "workspace_rename_branch")]
    WorkspaceRenameBranch {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        branch_name: String,
    },
    #[serde(
        rename = "workdir_ai_rename_branch",
        alias = "workspace_ai_rename_branch"
    )]
    WorkspaceAiRenameBranch {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
    },
    CancelAgentTurn {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
    },
    #[serde(rename = "create_task", alias = "create_workspace_thread")]
    CreateWorkspaceThread {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(default)]
        model_id: Option<String>,
        #[serde(default)]
        thinking_effort: Option<ThinkingEffort>,
    },
    #[serde(rename = "activate_task", alias = "activate_workspace_thread")]
    ActivateWorkspaceThread {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
    },
    #[serde(rename = "close_task_tab", alias = "close_workspace_thread_tab")]
    CloseWorkspaceThreadTab {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
    },
    #[serde(rename = "delete_task", alias = "delete_workspace_thread")]
    DeleteWorkspaceThread {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
    },
    #[serde(rename = "restore_task_tab", alias = "restore_workspace_thread_tab")]
    RestoreWorkspaceThreadTab {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
    },
    #[serde(rename = "reorder_task_tab", alias = "reorder_workspace_thread_tab")]
    ReorderWorkspaceThreadTab {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(rename = "task_id", alias = "thread_id")]
        thread_id: WorkspaceThreadId,
        to_index: usize,
    },
    OpenButtonSelectionChanged {
        selection: String,
    },
    SidebarProjectOrderChanged {
        #[serde(default)]
        project_ids: Vec<ProjectId>,
    },
    AppearanceThemeChanged {
        theme: AppearanceTheme,
    },
    AppearanceFontsChanged {
        fonts: AppearanceFontsSnapshot,
    },
    AppearanceGlobalZoomChanged {
        zoom: f64,
    },
    CodexEnabledChanged {
        enabled: bool,
    },
    AmpEnabledChanged {
        enabled: bool,
    },
    ClaudeEnabledChanged {
        enabled: bool,
    },
    DroidEnabledChanged {
        enabled: bool,
    },
    AgentRunnerChanged {
        runner: AgentRunnerKind,
    },
    AgentAmpModeChanged {
        mode: String,
    },
    TaskPromptTemplateChanged {
        intent_kind: TaskIntentKind,
        template: String,
    },
    SystemPromptTemplateChanged {
        kind: SystemTaskKind,
        template: String,
    },
    CodexCheck,
    CodexConfigTree,
    CodexConfigListDir {
        path: String,
    },
    CodexConfigReadFile {
        path: String,
    },
    CodexConfigWriteFile {
        path: String,
        contents: String,
    },
    AmpCheck,
    AmpConfigTree,
    AmpConfigListDir {
        path: String,
    },
    AmpConfigReadFile {
        path: String,
    },
    AmpConfigWriteFile {
        path: String,
        contents: String,
    },
    ClaudeCheck,
    ClaudeConfigTree,
    ClaudeConfigListDir {
        path: String,
    },
    ClaudeConfigReadFile {
        path: String,
    },
    ClaudeConfigWriteFile {
        path: String,
        contents: String,
    },
    DroidCheck,
    DroidConfigTree,
    DroidConfigListDir {
        path: String,
    },
    DroidConfigReadFile {
        path: String,
    },
    DroidConfigWriteFile {
        path: String,
        contents: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    AppChanged {
        rev: u64,
        snapshot: Box<AppSnapshot>,
    },
    TelegramPairReady {
        request_id: String,
        url: String,
    },
    TaskSummariesChanged {
        project_id: ProjectId,
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        tasks: Vec<TaskSummarySnapshot>,
    },
    #[serde(rename = "workdir_tasks_changed", alias = "workspace_threads_changed")]
    WorkspaceThreadsChanged {
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
        #[serde(default)]
        tabs: WorkspaceTabsSnapshot,
        #[serde(rename = "tasks", alias = "threads")]
        threads: Vec<ThreadMeta>,
    },
    ConversationChanged {
        snapshot: Box<ConversationSnapshot>,
    },
    Toast {
        message: String,
    },
    ProjectPathPicked {
        request_id: String,
        path: Option<String>,
    },
    AddProjectAndOpenReady {
        request_id: String,
        project_id: ProjectId,
        #[serde(rename = "workdir_id", alias = "workspace_id")]
        workspace_id: WorkspaceId,
    },
    TaskExecuted {
        request_id: String,
        result: TaskExecuteResult,
    },
    FeedbackSubmitted {
        request_id: String,
        result: FeedbackSubmitResult,
    },
    CodexCheckReady {
        request_id: String,
        ok: bool,
        message: Option<String>,
    },
    AmpCheckReady {
        request_id: String,
        ok: bool,
        message: Option<String>,
    },
    CodexConfigTreeReady {
        request_id: String,
        tree: Vec<CodexConfigEntrySnapshot>,
    },
    CodexConfigListDirReady {
        request_id: String,
        path: String,
        entries: Vec<CodexConfigEntrySnapshot>,
    },
    CodexConfigFileReady {
        request_id: String,
        path: String,
        contents: String,
    },
    CodexConfigFileSaved {
        request_id: String,
        path: String,
    },
    AmpConfigTreeReady {
        request_id: String,
        tree: Vec<AmpConfigEntrySnapshot>,
    },
    AmpConfigListDirReady {
        request_id: String,
        path: String,
        entries: Vec<AmpConfigEntrySnapshot>,
    },
    AmpConfigFileReady {
        request_id: String,
        path: String,
        contents: String,
    },
    AmpConfigFileSaved {
        request_id: String,
        path: String,
    },
    ClaudeCheckReady {
        request_id: String,
        ok: bool,
        message: Option<String>,
    },
    ClaudeConfigTreeReady {
        request_id: String,
        tree: Vec<ClaudeConfigEntrySnapshot>,
    },
    ClaudeConfigListDirReady {
        request_id: String,
        path: String,
        entries: Vec<ClaudeConfigEntrySnapshot>,
    },
    ClaudeConfigFileReady {
        request_id: String,
        path: String,
        contents: String,
    },
    ClaudeConfigFileSaved {
        request_id: String,
        path: String,
    },
    DroidCheckReady {
        request_id: String,
        ok: bool,
        message: Option<String>,
    },
    DroidConfigTreeReady {
        request_id: String,
        tree: Vec<DroidConfigEntrySnapshot>,
    },
    DroidConfigListDirReady {
        request_id: String,
        path: String,
        entries: Vec<DroidConfigEntrySnapshot>,
    },
    DroidConfigFileReady {
        request_id: String,
        path: String,
        contents: String,
    },
    DroidConfigFileSaved {
        request_id: String,
        path: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadMeta {
    #[serde(rename = "task_id", alias = "thread_id")]
    pub thread_id: WorkspaceThreadId,
    pub remote_thread_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    #[serde(default)]
    pub task_status: TaskStatus,
    #[serde(default)]
    pub turn_status: TurnStatus,
    #[serde(default)]
    pub last_turn_result: Option<TurnResult>,
}
