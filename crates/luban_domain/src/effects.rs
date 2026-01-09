use crate::{AgentRunConfig, AttachmentRef, ProjectId, WorkspaceId, WorkspaceThreadId};

#[derive(Clone, Debug)]
pub enum Effect {
    LoadAppState,
    SaveAppState,

    CreateWorkspace {
        project_id: ProjectId,
    },
    OpenWorkspaceInIde {
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
    EnsureConversation {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    LoadConversation {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },
    RunAgentTurn {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        text: String,
        attachments: Vec<AttachmentRef>,
        run_config: AgentRunConfig,
    },
    CancelAgentTurn {
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    },

    LoadWorkspaceThreads {
        workspace_id: WorkspaceId,
    },
}
