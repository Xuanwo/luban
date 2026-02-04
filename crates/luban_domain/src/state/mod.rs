mod agent;
mod appearance;
mod attachments;
mod conversation;
mod ids;
mod layout;
mod persisted;
mod tabs;
mod task;
mod workspace;

pub use agent::{AgentRunConfig, QueuedPrompt};
pub use appearance::{AppearanceFonts, AppearanceTheme};
pub use attachments::{AttachmentKind, AttachmentRef, ContextItem};
pub use conversation::{
    AgentEvent, ChatScrollAnchor, ConversationEntry, ConversationSnapshot, ConversationSystemEvent,
    ConversationThreadMeta, DraftAttachment, UserEvent, WorkspaceConversation,
};
pub use ids::{ProjectId, WorkspaceId, WorkspaceThreadId};
pub use layout::{MainPane, OperationStatus, RightPane, WorkspaceStatus};
pub use persisted::{
    PersistedAppState, PersistedProject, PersistedWorkspace,
    PersistedWorkspaceThreadRunConfigOverride,
};
pub use tabs::WorkspaceTabs;
pub use task::{TaskStatus, TurnResult, TurnStatus, parse_task_status};
pub use workspace::{AppState, Project, TelegramTopicBinding, Workspace};

pub(crate) const MAX_CONVERSATION_ENTRIES_IN_MEMORY: usize = 5000;

pub(crate) use conversation::{apply_draft_text_diff, entries_is_prefix, entries_is_suffix};
