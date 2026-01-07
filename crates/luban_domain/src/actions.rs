use crate::{
    CodexThreadEvent, ContextTokenKind, ConversationSnapshot, ConversationThreadMeta,
    PersistedAppState, ProjectId, ThinkingEffort, WorkspaceId, WorkspaceThreadId,
};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Action {
    AppStarted,

    OpenDashboard,
    DashboardPreviewOpened {
        workspace_id: WorkspaceId,
    },
    DashboardPreviewClosed,

    AddProject {
        path: PathBuf,
    },
    ToggleProjectExpanded {
        project_id: ProjectId,
    },
    OpenProjectSettings {
        project_id: ProjectId,
    },

    CreateWorkspace {
        project_id: ProjectId,
    },
    WorkspaceCreated {
        project_id: ProjectId,
        workspace_name: String,
        branch_name: String,
        worktree_path: PathBuf,
    },
    WorkspaceCreateFailed {
        project_id: ProjectId,
        message: String,
    },

    OpenWorkspace {
        workspace_id: WorkspaceId,
    },
    OpenWorkspaceInIde {
        workspace_id: WorkspaceId,
    },
    OpenWorkspaceInIdeFailed {
        message: String,
    },
    OpenWorkspacePullRequest {
        workspace_id: WorkspaceId,
    },
    OpenWorkspacePullRequestFailed {
        message: String,
    },
    ArchiveWorkspace {
        workspace_id: WorkspaceId,
    },
    WorkspaceArchived {
        workspace_id: WorkspaceId,
    },
    WorkspaceArchiveFailed {
        workspace_id: WorkspaceId,
        message: String,
    },

    ConversationLoaded {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        snapshot: ConversationSnapshot,
    },
    ConversationLoadFailed {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        message: String,
    },
    SendAgentMessage {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        text: String,
    },
    ChatModelChanged {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        model_id: String,
    },
    ThinkingEffortChanged {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        thinking_effort: ThinkingEffort,
    },
    ChatDraftChanged {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        text: String,
    },
    ChatDraftAttachmentAdded {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        id: u64,
        kind: ContextTokenKind,
        anchor: usize,
    },
    ChatDraftAttachmentResolved {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        id: u64,
        path: PathBuf,
    },
    ChatDraftAttachmentFailed {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        id: u64,
    },
    ChatDraftAttachmentRemoved {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        id: u64,
    },
    RemoveQueuedPrompt {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        index: usize,
    },
    ClearQueuedPrompts {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    ResumeQueuedPrompts {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    AgentEventReceived {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        event: CodexThreadEvent,
    },
    AgentTurnFinished {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
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

    WorkspaceThreadsLoaded {
        workspace_id: WorkspaceId,
        threads: Vec<ConversationThreadMeta>,
    },
    WorkspaceThreadsLoadFailed {
        workspace_id: WorkspaceId,
        message: String,
    },

    ToggleTerminalPane,
    TerminalPaneWidthChanged {
        width: u16,
    },
    SidebarWidthChanged {
        width: u16,
    },
    WorkspaceChatScrollSaved {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        offset_y10: i32,
    },

    SaveAppState,

    AppStateLoaded {
        persisted: PersistedAppState,
    },
    AppStateLoadFailed {
        message: String,
    },
    AppStateSaved,
    AppStateSaveFailed {
        message: String,
    },

    ClearError,
}
