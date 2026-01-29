use crate::{
    AgentRunConfig, AttachmentRef, OpenTarget, ProjectId, SystemTaskKind, TaskIntentKind,
    WorkspaceId, WorkspaceThreadId,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Effect {
    LoadAppState,
    SaveAppState,

    LoadCodexDefaults,

    LoadTaskPromptTemplates,
    LoadSystemPromptTemplates,
    MigrateLegacyTaskPromptTemplates {
        templates: HashMap<TaskIntentKind, String>,
    },
    StoreTaskPromptTemplate {
        intent_kind: TaskIntentKind,
        template: String,
    },
    DeleteTaskPromptTemplate {
        intent_kind: TaskIntentKind,
    },
    StoreSystemPromptTemplate {
        kind: SystemTaskKind,
        template: String,
    },
    DeleteSystemPromptTemplate {
        kind: SystemTaskKind,
    },

    CreateWorkspace {
        project_id: ProjectId,
        branch_name_hint: Option<String>,
    },
    OpenWorkspaceInIde {
        workspace_id: WorkspaceId,
    },
    OpenWorkspaceWith {
        workspace_id: WorkspaceId,
        target: OpenTarget,
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
    EnsureConversation {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    StoreConversationRunConfig {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        runner: crate::AgentRunnerKind,
        model_id: String,
        thinking_effort: crate::ThinkingEffort,
        amp_mode: Option<String>,
    },
    LoadConversation {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    RunAgentTurn {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        run_id: u64,
        text: String,
        attachments: Vec<AttachmentRef>,
        run_config: AgentRunConfig,
    },
    CancelAgentTurn {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        run_id: u64,
    },

    /// Cleanup Claude process associated with a thread.
    /// This is emitted when a thread tab is closed to free resources.
    CleanupClaudeProcess {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },

    RenameWorkspaceBranch {
        workspace_id: WorkspaceId,
        requested_branch_name: String,
    },
    AiRenameWorkspaceBranch {
        workspace_id: WorkspaceId,
        input: String,
    },
    AiAutoTitleThread {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        input: String,
        expected_current_title: String,
    },

    LoadWorkspaceThreads {
        workspace_id: WorkspaceId,
    },
}
