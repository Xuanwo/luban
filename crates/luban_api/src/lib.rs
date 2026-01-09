use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub u64);

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub id: ProjectId,
    pub name: String,
    pub slug: String,
    pub path: String,
    pub expanded: bool,
    pub create_workspace_status: OperationStatus,
    pub workspaces: Vec<WorkspaceSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub id: WorkspaceId,
    pub short_id: String,
    pub workspace_name: String,
    pub branch_name: String,
    pub worktree_path: String,
    pub status: WorkspaceStatus,
    pub agent_run_status: OperationStatus,
    pub has_unread_completion: bool,
    pub pull_request: Option<PullRequestSnapshot>,
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
    pub workspace_id: WorkspaceId,
    pub thread_id: WorkspaceThreadId,
    pub agent_model_id: String,
    pub thinking_effort: ThinkingEffort,
    pub run_status: OperationStatus,
    pub entries: Vec<ConversationEntry>,
    #[serde(default)]
    pub in_progress_items: Vec<AgentItem>,
    pub remote_thread_id: Option<String>,
    pub title: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Idle,
    Running,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingEffort {
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadsSnapshot {
    pub rev: u64,
    pub workspace_id: WorkspaceId,
    pub threads: Vec<ThreadMeta>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationEntry {
    UserMessage(UserMessage),
    AgentItem(AgentItem),
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
pub struct UserMessage {
    pub text: String,
    pub attachments: Vec<AttachmentRef>,
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
pub struct AgentItem {
    pub id: String,
    pub kind: AgentItemKind,
    pub payload: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentItemKind {
    AgentMessage,
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
        action: ClientAction,
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
        event: ServerEvent,
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
    DeleteProject {
        project_id: ProjectId,
    },
    ToggleProjectExpanded {
        project_id: ProjectId,
    },
    CreateWorkspace {
        project_id: ProjectId,
    },
    OpenWorkspace {
        workspace_id: WorkspaceId,
    },
    OpenWorkspacePullRequest {
        workspace_id: WorkspaceId,
    },
    OpenWorkspacePullRequestFailedAction {
        workspace_id: WorkspaceId,
    },
    ArchiveWorkspace {
        workspace_id: WorkspaceId,
    },
    SendAgentMessage {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        text: String,
        attachments: Vec<AttachmentRef>,
    },
    CancelAgentTurn {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    CreateWorkspaceThread {
        workspace_id: WorkspaceId,
    },
    ActivateWorkspaceThread {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    CloseWorkspaceThreadTab {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    RestoreWorkspaceThreadTab {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    ReorderWorkspaceThreadTab {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        to_index: usize,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    AppChanged {
        rev: u64,
        snapshot: AppSnapshot,
    },
    WorkspaceThreadsChanged {
        workspace_id: WorkspaceId,
        threads: Vec<ThreadMeta>,
    },
    ConversationChanged {
        snapshot: ConversationSnapshot,
    },
    Toast {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadMeta {
    pub thread_id: WorkspaceThreadId,
    pub remote_thread_id: Option<String>,
    pub title: String,
    pub updated_at_unix_seconds: u64,
}
