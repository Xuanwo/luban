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
    StoreConversationTaskStatus {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        task_status: crate::TaskStatus,
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
        runner: crate::AgentRunnerKind,
        model_id: String,
        thinking_effort: crate::ThinkingEffort,
        amp_mode: Option<String>,
    },
    AiAutoTitleThread {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        input: String,
        expected_current_title: String,
        runner: crate::AgentRunnerKind,
        model_id: String,
        thinking_effort: crate::ThinkingEffort,
        amp_mode: Option<String>,
    },
    AiAutoUpdateTaskStatus {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        input: String,
        expected_current_task_status: crate::TaskStatus,
        runner: crate::AgentRunnerKind,
        model_id: String,
        thinking_effort: crate::ThinkingEffort,
        amp_mode: Option<String>,
    },

    LoadWorkspaceThreads {
        workspace_id: WorkspaceId,
    },

    /// After a task is moved to a closed state (`done`/`canceled`), the provider may be able to
    /// cleanup the workspace worktree/branch and mark the workdir as archived.
    ///
    /// The engine decides whether cleanup is safe (for example: all threads are closed + idle).
    MaybeAutoArchiveWorkspace {
        workspace_id: WorkspaceId,
    },
}
