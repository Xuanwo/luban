use crate::{CodexThreadEvent, ConversationSnapshot, ConversationThreadMeta, PersistedAppState};
use std::{path::PathBuf, sync::Arc, sync::atomic::AtomicBool};

#[derive(Clone, Debug)]
pub struct ContextImage {
    pub extension: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

impl PullRequestState {
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Closed | Self::Merged)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullRequestInfo {
    pub number: u64,
    pub is_draft: bool,
    pub state: PullRequestState,
}

#[derive(Clone, Debug)]
pub struct CreatedWorkspace {
    pub workspace_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct RunAgentTurnRequest {
    pub project_slug: String,
    pub workspace_name: String,
    pub worktree_path: PathBuf,
    pub thread_local_id: u64,
    pub thread_id: Option<String>,
    pub prompt: String,
    pub model: Option<String>,
    pub model_reasoning_effort: Option<String>,
}

pub trait ProjectWorkspaceService: Send + Sync {
    fn load_app_state(&self) -> Result<PersistedAppState, String>;

    fn save_app_state(&self, snapshot: PersistedAppState) -> Result<(), String>;

    fn create_workspace(
        &self,
        project_path: PathBuf,
        project_slug: String,
    ) -> Result<CreatedWorkspace, String>;

    fn open_workspace_in_ide(&self, worktree_path: PathBuf) -> Result<(), String>;

    fn archive_workspace(
        &self,
        project_path: PathBuf,
        worktree_path: PathBuf,
    ) -> Result<(), String>;

    fn ensure_conversation(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
    ) -> Result<(), String>;

    fn list_conversation_threads(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> Result<Vec<ConversationThreadMeta>, String>;

    fn load_conversation(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
    ) -> Result<ConversationSnapshot, String>;

    fn store_context_image(
        &self,
        project_slug: String,
        workspace_name: String,
        image: ContextImage,
    ) -> Result<PathBuf, String>;

    fn store_context_text(
        &self,
        project_slug: String,
        workspace_name: String,
        text: String,
        extension: String,
    ) -> Result<PathBuf, String>;

    fn store_context_file(
        &self,
        project_slug: String,
        workspace_name: String,
        source_path: PathBuf,
    ) -> Result<PathBuf, String>;

    fn run_agent_turn_streamed(
        &self,
        request: RunAgentTurnRequest,
        cancel: Arc<AtomicBool>,
        on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync>,
    ) -> Result<(), String>;

    fn gh_is_authorized(&self) -> Result<bool, String>;

    fn gh_pull_request_info(
        &self,
        worktree_path: PathBuf,
    ) -> Result<Option<PullRequestInfo>, String>;

    fn gh_open_pull_request(&self, worktree_path: PathBuf) -> Result<(), String>;
}
