use crate::{
    AgentRunnerKind, AgentThreadEvent, AttachmentRef, ContextItem, ConversationEntry,
    ConversationSnapshot, ConversationThreadMeta, PersistedAppState, QueuedPrompt, SystemTaskKind,
    TaskStatus, ThinkingEffort,
};
use std::collections::HashMap;
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
pub enum PullRequestCiState {
    Pending,
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullRequestInfo {
    pub number: u64,
    pub is_draft: bool,
    pub state: PullRequestState,
    pub ci_state: Option<PullRequestCiState>,
    pub merge_ready: bool,
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
    pub attachments: Vec<AttachmentRef>,
    pub runner: AgentRunnerKind,
    pub amp_mode: Option<String>,
    pub model: Option<String>,
    pub model_reasoning_effort: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TaskIntentKind {
    Fix,
    Implement,
    Review,
    Discuss,
    Other,
}

impl TaskIntentKind {
    pub const ALL: [TaskIntentKind; 5] = [
        TaskIntentKind::Fix,
        TaskIntentKind::Implement,
        TaskIntentKind::Review,
        TaskIntentKind::Discuss,
        TaskIntentKind::Other,
    ];

    pub fn as_key(self) -> &'static str {
        match self {
            TaskIntentKind::Fix => "fix",
            TaskIntentKind::Implement => "implement",
            TaskIntentKind::Review => "review",
            TaskIntentKind::Discuss => "discuss",
            TaskIntentKind::Other => "other",
        }
    }

    pub fn parse_key(raw: &str) -> TaskIntentKind {
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("fix") || raw.eq_ignore_ascii_case("fix_issue") {
            return TaskIntentKind::Fix;
        }
        if raw.eq_ignore_ascii_case("implement") || raw.eq_ignore_ascii_case("implement_feature") {
            return TaskIntentKind::Implement;
        }
        if raw.eq_ignore_ascii_case("review") || raw.eq_ignore_ascii_case("review_pull_request") {
            return TaskIntentKind::Review;
        }
        if raw.eq_ignore_ascii_case("discuss") {
            return TaskIntentKind::Discuss;
        }
        TaskIntentKind::Other
    }

    pub fn label(self) -> &'static str {
        match self {
            TaskIntentKind::Fix => "Fix",
            TaskIntentKind::Implement => "Implement",
            TaskIntentKind::Review => "Review",
            TaskIntentKind::Discuss => "Discuss",
            TaskIntentKind::Other => "Other",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_intent_kind_parse_key_is_ascii_case_insensitive_and_trimmed() {
        assert_eq!(TaskIntentKind::parse_key(" fix "), TaskIntentKind::Fix);
        assert_eq!(TaskIntentKind::parse_key("FIX"), TaskIntentKind::Fix);
        assert_eq!(TaskIntentKind::parse_key("Fix_Issue"), TaskIntentKind::Fix);

        assert_eq!(
            TaskIntentKind::parse_key(" Implement "),
            TaskIntentKind::Implement
        );
        assert_eq!(
            TaskIntentKind::parse_key("IMPLEMENT_FEATURE"),
            TaskIntentKind::Implement
        );

        assert_eq!(TaskIntentKind::parse_key("review"), TaskIntentKind::Review);
        assert_eq!(
            TaskIntentKind::parse_key("Review_Pull_Request"),
            TaskIntentKind::Review
        );

        assert_eq!(
            TaskIntentKind::parse_key("  discuss  "),
            TaskIntentKind::Discuss
        );
    }

    #[test]
    fn task_intent_kind_parse_key_defaults_to_other() {
        assert_eq!(TaskIntentKind::parse_key(""), TaskIntentKind::Other);
        assert_eq!(TaskIntentKind::parse_key("unknown"), TaskIntentKind::Other);
    }
}

#[derive(Clone, Debug)]
pub struct TaskIssueInfo {
    pub number: u64,
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct TaskStatusAutoUpdateSuggestion {
    pub task_status: TaskStatus,
    pub validation_pr_number: Option<u64>,
    pub validation_pr_url: Option<String>,
    pub explanation_markdown: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectIdentity {
    pub root_path: PathBuf,
    pub github_repo: Option<String>,
    pub is_git: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenTarget {
    Vscode,
    Cursor,
    Zed,
    Ghostty,
    Finder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexConfigEntryKind {
    File,
    Folder,
}

#[derive(Clone, Debug)]
pub struct CodexConfigEntry {
    pub path: String,
    pub name: String,
    pub kind: CodexConfigEntryKind,
    pub children: Vec<CodexConfigEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmpConfigEntryKind {
    File,
    Folder,
}

#[derive(Clone, Debug)]
pub struct AmpConfigEntry {
    pub path: String,
    pub name: String,
    pub kind: AmpConfigEntryKind,
    pub children: Vec<AmpConfigEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaudeConfigEntryKind {
    File,
    Folder,
}

#[derive(Clone, Debug)]
pub struct ClaudeConfigEntry {
    pub path: String,
    pub name: String,
    pub kind: ClaudeConfigEntryKind,
    pub children: Vec<ClaudeConfigEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DroidConfigEntryKind {
    File,
    Folder,
}

#[derive(Clone, Debug)]
pub struct DroidConfigEntry {
    pub path: String,
    pub name: String,
    pub kind: DroidConfigEntryKind,
    pub children: Vec<DroidConfigEntry>,
}

#[derive(Clone, Debug)]
pub struct NewTaskDraft {
    pub id: String,
    pub text: String,
    pub project_id: Option<String>,
    pub workspace_id: Option<u64>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Clone, Debug)]
pub struct NewTaskStash {
    pub text: String,
    pub project_id: Option<String>,
    pub workspace_id: Option<u64>,
    pub editing_draft_id: Option<String>,
    pub updated_at_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TaskDocumentKind {
    Task,
    Plan,
    Memory,
}

impl TaskDocumentKind {
    pub fn as_key(self) -> &'static str {
        match self {
            TaskDocumentKind::Task => "task",
            TaskDocumentKind::Plan => "plan",
            TaskDocumentKind::Memory => "memory",
        }
    }

    pub fn parse_key(raw: &str) -> Option<TaskDocumentKind> {
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("task") {
            return Some(TaskDocumentKind::Task);
        }
        if raw.eq_ignore_ascii_case("plan") {
            return Some(TaskDocumentKind::Plan);
        }
        if raw.eq_ignore_ascii_case("memory") {
            return Some(TaskDocumentKind::Memory);
        }
        None
    }

    pub fn file_name(self) -> &'static str {
        match self {
            TaskDocumentKind::Task => "TASK.md",
            TaskDocumentKind::Plan => "PLAN.md",
            TaskDocumentKind::Memory => "MEMORY.md",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskDocumentEventType {
    Initialized,
    Updated,
}

impl TaskDocumentEventType {
    pub fn as_key(self) -> &'static str {
        match self {
            TaskDocumentEventType::Initialized => "initialized",
            TaskDocumentEventType::Updated => "updated",
        }
    }

    pub fn parse_key(raw: &str) -> Option<TaskDocumentEventType> {
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("initialized") {
            return Some(TaskDocumentEventType::Initialized);
        }
        if raw.eq_ignore_ascii_case("updated") {
            return Some(TaskDocumentEventType::Updated);
        }
        None
    }
}

#[derive(Clone, Debug)]
pub struct TaskDocumentIndex {
    pub kind: TaskDocumentKind,
    pub rel_path: String,
    pub content_hash: String,
    pub byte_len: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Clone, Debug)]
pub struct TaskDocumentEvent {
    pub event_id: u64,
    pub kind: TaskDocumentKind,
    pub event_type: TaskDocumentEventType,
    pub rel_path: String,
    pub content_hash: String,
    pub byte_len: u64,
    pub created_at_unix_ms: u64,
}

pub trait ProjectWorkspaceService: Send + Sync {
    fn load_app_state(&self) -> Result<PersistedAppState, String>;

    fn save_app_state(&self, snapshot: PersistedAppState) -> Result<(), String>;

    fn create_workspace(
        &self,
        project_path: PathBuf,
        project_slug: String,
        branch_name_hint: Option<String>,
    ) -> Result<CreatedWorkspace, String>;

    fn open_workspace_in_ide(&self, worktree_path: PathBuf) -> Result<(), String>;

    fn open_workspace_with(
        &self,
        _worktree_path: PathBuf,
        _target: OpenTarget,
    ) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn archive_workspace(
        &self,
        project_path: PathBuf,
        worktree_path: PathBuf,
        branch_name: String,
    ) -> Result<(), String>;

    fn rename_workspace_branch(
        &self,
        worktree_path: PathBuf,
        requested_branch_name: String,
    ) -> Result<String, String>;

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

    fn load_conversation_page(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
        before: Option<u64>,
        limit: u64,
    ) -> Result<ConversationSnapshot, String>;

    fn append_conversation_entries(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
        _entries: Vec<ConversationEntry>,
    ) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn delete_conversation_thread(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
    ) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    #[allow(clippy::too_many_arguments)]
    fn save_conversation_queue_state(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
        _queue_paused: bool,
        _run_started_at_unix_ms: Option<u64>,
        _run_finished_at_unix_ms: Option<u64>,
        _pending_prompts: Vec<QueuedPrompt>,
    ) -> Result<(), String> {
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn save_conversation_run_config(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
        _runner: AgentRunnerKind,
        _model_id: String,
        _thinking_effort: ThinkingEffort,
        _amp_mode: Option<String>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn save_conversation_task_status(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
        _task_status: TaskStatus,
    ) -> Result<(), String> {
        Ok(())
    }

    fn save_conversation_task_status_last_analyzed(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
    ) -> Result<(), String> {
        Ok(())
    }

    fn save_conversation_task_validation_pr(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
        _pr_number: u64,
        _pr_url: Option<String>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn list_conversation_tasks_for_merged_pr(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _pr_number: u64,
    ) -> Result<Vec<u64>, String> {
        Ok(Vec::new())
    }

    fn store_context_image(
        &self,
        project_slug: String,
        workspace_name: String,
        image: ContextImage,
    ) -> Result<AttachmentRef, String>;

    fn store_context_text(
        &self,
        project_slug: String,
        workspace_name: String,
        text: String,
        extension: String,
    ) -> Result<AttachmentRef, String>;

    fn store_context_file(
        &self,
        project_slug: String,
        workspace_name: String,
        source_path: PathBuf,
    ) -> Result<AttachmentRef, String>;

    fn record_context_item(
        &self,
        project_slug: String,
        workspace_name: String,
        attachment: AttachmentRef,
        created_at_unix_ms: u64,
    ) -> Result<u64, String>;

    fn list_context_items(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> Result<Vec<ContextItem>, String>;

    fn delete_context_item(
        &self,
        project_slug: String,
        workspace_name: String,
        context_id: u64,
    ) -> Result<(), String>;

    fn list_new_task_drafts(&self) -> Result<Vec<NewTaskDraft>, String> {
        Err("unimplemented".to_owned())
    }

    fn create_new_task_draft(
        &self,
        _text: String,
        _project_id: Option<String>,
        _workspace_id: Option<u64>,
    ) -> Result<NewTaskDraft, String> {
        Err("unimplemented".to_owned())
    }

    fn update_new_task_draft(
        &self,
        _draft_id: String,
        _text: String,
        _project_id: Option<String>,
        _workspace_id: Option<u64>,
    ) -> Result<NewTaskDraft, String> {
        Err("unimplemented".to_owned())
    }

    fn delete_new_task_draft(&self, _draft_id: String) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn load_new_task_stash(&self) -> Result<Option<NewTaskStash>, String> {
        Err("unimplemented".to_owned())
    }

    fn save_new_task_stash(&self, _stash: NewTaskStash) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn clear_new_task_stash(&self) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn upsert_task_document_index(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
        _index: TaskDocumentIndex,
    ) -> Result<(), String> {
        Ok(())
    }

    fn list_task_document_indices(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
    ) -> Result<Vec<TaskDocumentIndex>, String> {
        Ok(Vec::new())
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_task_document_event(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
        _kind: TaskDocumentKind,
        _event_type: TaskDocumentEventType,
        _rel_path: String,
        _content_hash: String,
        _byte_len: u64,
        _created_at_unix_ms: u64,
    ) -> Result<u64, String> {
        Ok(0)
    }

    fn list_task_document_events(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
    ) -> Result<Vec<TaskDocumentEvent>, String> {
        Ok(Vec::new())
    }

    fn run_agent_turn_streamed(
        &self,
        request: RunAgentTurnRequest,
        cancel: Arc<AtomicBool>,
        on_event: Arc<dyn Fn(AgentThreadEvent) + Send + Sync>,
    ) -> Result<(), String>;

    /// Clean up any persistent Claude process for the given thread.
    ///
    /// This should be called when a thread/tab is closed to free resources.
    /// The default implementation is a no-op; implementations that support
    /// persistent processes should override this.
    fn cleanup_claude_process(
        &self,
        _project_slug: &str,
        _workspace_name: &str,
        _thread_local_id: u64,
    ) {
        // Default: no-op
    }

    fn gh_is_authorized(&self) -> Result<bool, String>;

    fn gh_pull_request_info(
        &self,
        worktree_path: PathBuf,
    ) -> Result<Option<PullRequestInfo>, String>;

    fn gh_open_pull_request(&self, worktree_path: PathBuf) -> Result<(), String>;

    fn gh_open_pull_request_failed_action(&self, worktree_path: PathBuf) -> Result<(), String>;

    fn feedback_create_issue(
        &self,
        _title: String,
        _body: String,
        _labels: Vec<String>,
    ) -> Result<TaskIssueInfo, String> {
        Err("unimplemented".to_owned())
    }

    fn feedback_task_prompt(
        &self,
        _issue: TaskIssueInfo,
        _intent_kind: TaskIntentKind,
    ) -> Result<String, String> {
        Err("unimplemented".to_owned())
    }

    fn task_prompt_templates_load(&self) -> Result<HashMap<TaskIntentKind, String>, String> {
        Ok(HashMap::new())
    }

    fn task_prompt_template_store(
        &self,
        _intent_kind: TaskIntentKind,
        _template: String,
    ) -> Result<(), String> {
        Ok(())
    }

    fn task_prompt_template_delete(&self, _intent_kind: TaskIntentKind) -> Result<(), String> {
        Ok(())
    }

    fn system_prompt_templates_load(&self) -> Result<HashMap<SystemTaskKind, String>, String> {
        Ok(HashMap::new())
    }

    fn system_prompt_template_store(
        &self,
        _kind: SystemTaskKind,
        _template: String,
    ) -> Result<(), String> {
        Ok(())
    }

    fn system_prompt_template_delete(&self, _kind: SystemTaskKind) -> Result<(), String> {
        Ok(())
    }

    fn task_suggest_branch_name(
        &self,
        _input: String,
        _runner: AgentRunnerKind,
        _model_id: String,
        _thinking_effort: ThinkingEffort,
        _amp_mode: Option<String>,
    ) -> Result<String, String> {
        Err("unimplemented".to_owned())
    }

    fn task_suggest_thread_title(
        &self,
        _input: String,
        _runner: AgentRunnerKind,
        _model_id: String,
        _thinking_effort: ThinkingEffort,
        _amp_mode: Option<String>,
    ) -> Result<String, String> {
        Err("unimplemented".to_owned())
    }

    fn task_suggest_task_status(
        &self,
        _input: String,
        _runner: AgentRunnerKind,
        _model_id: String,
        _thinking_effort: ThinkingEffort,
        _amp_mode: Option<String>,
    ) -> Result<TaskStatus, String> {
        Err("unimplemented".to_owned())
    }

    fn task_suggest_task_status_auto_update(
        &self,
        input: String,
        runner: AgentRunnerKind,
        model_id: String,
        thinking_effort: ThinkingEffort,
        amp_mode: Option<String>,
    ) -> Result<TaskStatusAutoUpdateSuggestion, String> {
        let task_status =
            self.task_suggest_task_status(input, runner, model_id, thinking_effort, amp_mode)?;
        Ok(TaskStatusAutoUpdateSuggestion {
            task_status,
            validation_pr_number: None,
            validation_pr_url: None,
            explanation_markdown: None,
        })
    }

    fn conversation_update_title_if_matches(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
        _expected_current_title: String,
        _new_title: String,
    ) -> Result<bool, String> {
        Ok(false)
    }

    fn codex_check(&self) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn codex_config_tree(&self) -> Result<Vec<CodexConfigEntry>, String> {
        Err("unimplemented".to_owned())
    }

    fn codex_config_list_dir(&self, _path: String) -> Result<Vec<CodexConfigEntry>, String> {
        Err("unimplemented".to_owned())
    }

    fn codex_config_read_file(&self, _path: String) -> Result<String, String> {
        Err("unimplemented".to_owned())
    }

    fn codex_config_write_file(&self, _path: String, _contents: String) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn amp_check(&self) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn amp_config_tree(&self) -> Result<Vec<AmpConfigEntry>, String> {
        Err("unimplemented".to_owned())
    }

    fn amp_config_list_dir(&self, _path: String) -> Result<Vec<AmpConfigEntry>, String> {
        Err("unimplemented".to_owned())
    }

    fn amp_config_read_file(&self, _path: String) -> Result<String, String> {
        Err("unimplemented".to_owned())
    }

    fn amp_config_write_file(&self, _path: String, _contents: String) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn claude_check(&self) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn claude_config_tree(&self) -> Result<Vec<ClaudeConfigEntry>, String> {
        Err("unimplemented".to_owned())
    }

    fn claude_config_list_dir(&self, _path: String) -> Result<Vec<ClaudeConfigEntry>, String> {
        Err("unimplemented".to_owned())
    }

    fn claude_config_read_file(&self, _path: String) -> Result<String, String> {
        Err("unimplemented".to_owned())
    }

    fn claude_config_write_file(&self, _path: String, _contents: String) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn droid_check(&self) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn droid_config_tree(&self) -> Result<Vec<DroidConfigEntry>, String> {
        Err("unimplemented".to_owned())
    }

    fn droid_config_list_dir(&self, _path: String) -> Result<Vec<DroidConfigEntry>, String> {
        Err("unimplemented".to_owned())
    }

    fn droid_config_read_file(&self, _path: String) -> Result<String, String> {
        Err("unimplemented".to_owned())
    }

    fn droid_config_write_file(&self, _path: String, _contents: String) -> Result<(), String> {
        Err("unimplemented".to_owned())
    }

    fn project_identity(&self, path: PathBuf) -> Result<ProjectIdentity, String> {
        Ok(ProjectIdentity {
            root_path: path,
            github_repo: None,
            is_git: false,
        })
    }
}
