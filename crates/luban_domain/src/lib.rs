mod codex;
pub use codex::{
    CodexCommandExecutionStatus, CodexErrorMessage, CodexFileUpdateChange, CodexMcpToolCallStatus,
    CodexPatchApplyStatus, CodexPatchChangeKind, CodexThreadError, CodexThreadEvent,
    CodexThreadItem, CodexTodoItem, CodexUsage,
};

mod agent_thread;
pub use agent_thread::{
    AgentCommandExecutionStatus, AgentErrorMessage, AgentFileUpdateChange, AgentMcpToolCallStatus,
    AgentPatchApplyStatus, AgentPatchChangeKind, AgentThreadError, AgentThreadEvent,
    AgentThreadItem, AgentTodoItem, AgentUsage,
};

mod adapters;
pub use adapters::{
    AmpConfigEntry, AmpConfigEntryKind, ClaudeConfigEntry, ClaudeConfigEntryKind, CodexConfigEntry,
    CodexConfigEntryKind, ContextImage, CreatedWorkspace, OpenTarget, ProjectIdentity,
    ProjectWorkspaceService, PullRequestCiState, PullRequestInfo, PullRequestState,
    RunAgentTurnRequest, TaskDraft, TaskIntentKind, TaskIssueInfo, TaskProjectSpec,
    TaskPullRequestInfo, TaskRepoInfo,
};
mod context_tokens;
pub use context_tokens::{
    ContextToken, ContextTokenKind, extract_context_image_paths_in_order, find_context_tokens,
};
mod chat_draft;
pub use chat_draft::{
    compose_user_message_text, ordered_draft_attachments_for_display,
    ordered_draft_attachments_for_send,
};
mod actions;
pub use actions::Action;
mod effects;
pub use effects::Effect;
mod agent_settings;
pub mod paths;
mod task_prompts;
pub use agent_settings::{
    AgentModelSpec, AgentRunnerKind, ThinkingEffort, agent_model_label, agent_models,
    default_agent_model_id, default_agent_runner_kind, default_amp_mode, default_thinking_effort,
    normalize_thinking_effort, parse_agent_runner_kind, parse_thinking_effort,
    thinking_effort_supported,
};
pub use task_prompts::{default_task_prompt_template, default_task_prompt_templates};
mod system_prompts;
pub use system_prompts::{
    SystemTaskKind, default_system_prompt_template, default_system_prompt_templates,
};
mod dashboard;
mod time;
pub use dashboard::{
    DashboardCardModel, DashboardPreviewMessage, DashboardPreviewModel, DashboardStage,
    dashboard_cards, dashboard_preview,
};

mod persistence;
mod state;
pub use state::*;

mod reducer;
pub use reducer::derive_thread_title;

pub const THREAD_TITLE_MAX_CHARS: usize = 40;
