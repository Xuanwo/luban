use crate::{
    AgentRunnerKind, AgentThreadEvent, AppearanceTheme, AttachmentRef, ChatScrollAnchor,
    ContextTokenKind, ConversationSnapshot, ConversationThreadMeta, OpenTarget, PersistedAppState,
    ProjectId, SystemTaskKind, TaskIntentKind, ThinkingEffort, WorkspaceId, WorkspaceThreadId,
};
use std::collections::HashMap;
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
        is_git: bool,
    },
    ToggleProjectExpanded {
        project_id: ProjectId,
    },
    DeleteProject {
        project_id: ProjectId,
    },
    OpenProjectSettings {
        project_id: ProjectId,
    },

    CreateWorkspace {
        project_id: ProjectId,
        branch_name_hint: Option<String>,
    },
    EnsureMainWorkspace {
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
    OpenWorkspaceWith {
        workspace_id: WorkspaceId,
        target: OpenTarget,
    },
    OpenWorkspaceWithFailed {
        message: String,
    },
    OpenWorkspacePullRequest {
        workspace_id: WorkspaceId,
    },
    OpenWorkspacePullRequestFailed {
        message: String,
    },
    OpenWorkspacePullRequestFailedAction {
        workspace_id: WorkspaceId,
    },
    OpenWorkspacePullRequestFailedActionFailed {
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

    WorkspaceBranchRenameRequested {
        workspace_id: WorkspaceId,
        requested_branch_name: String,
    },
    WorkspaceBranchAiRenameRequested {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    WorkspaceBranchRenamed {
        workspace_id: WorkspaceId,
        branch_name: String,
    },
    WorkspaceBranchSynced {
        workspace_id: WorkspaceId,
        branch_name: String,
    },
    WorkspaceBranchRenameFailed {
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
        attachments: Vec<AttachmentRef>,
        runner: Option<AgentRunnerKind>,
        amp_mode: Option<String>,
    },
    QueueAgentMessage {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        text: String,
        attachments: Vec<AttachmentRef>,
        runner: Option<AgentRunnerKind>,
        amp_mode: Option<String>,
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
        attachment: AttachmentRef,
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
        prompt_id: u64,
    },
    ReorderQueuedPrompt {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        active_id: u64,
        over_id: u64,
    },
    UpdateQueuedPrompt {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        prompt_id: u64,
        text: String,
        attachments: Vec<AttachmentRef>,
        model_id: String,
        thinking_effort: ThinkingEffort,
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
        run_id: u64,
        event: AgentThreadEvent,
    },
    AgentRunStartedAt {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        run_id: u64,
        started_at_unix_ms: u64,
    },
    AgentRunFinishedAt {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        run_id: u64,
        finished_at_unix_ms: u64,
    },
    AgentTurnFinished {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        run_id: u64,
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
    AppearanceGlobalZoomChanged {
        zoom: f64,
    },
    AppearanceThemeChanged {
        theme: AppearanceTheme,
    },
    AppearanceFontsChanged {
        ui_font: String,
        chat_font: String,
        code_font: String,
        terminal_font: String,
    },
    AgentCodexEnabledChanged {
        enabled: bool,
    },
    AgentAmpEnabledChanged {
        enabled: bool,
    },
    AgentRunnerChanged {
        runner: AgentRunnerKind,
    },
    AgentAmpModeChanged {
        mode: String,
    },
    CodexDefaultsLoaded {
        model_id: Option<String>,
        thinking_effort: Option<ThinkingEffort>,
    },
    TaskPromptTemplateChanged {
        intent_kind: TaskIntentKind,
        template: String,
    },
    TaskPromptTemplatesLoaded {
        templates: HashMap<TaskIntentKind, String>,
    },
    SystemPromptTemplateChanged {
        kind: SystemTaskKind,
        template: String,
    },
    SystemPromptTemplatesLoaded {
        templates: HashMap<SystemTaskKind, String>,
    },
    WorkspaceChatScrollSaved {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        offset_y10: i32,
    },
    WorkspaceChatScrollAnchorSaved {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        anchor: ChatScrollAnchor,
    },

    SidebarProjectOrderChanged {
        project_ids: Vec<String>,
    },
    SidebarWorktreeOrderChanged {
        project_id: String,
        workspace_ids: Vec<WorkspaceId>,
    },

    OpenButtonSelectionChanged {
        selection: String,
    },

    SaveAppState,

    AppStateLoaded {
        persisted: Box<PersistedAppState>,
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
