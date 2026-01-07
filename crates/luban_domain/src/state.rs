use crate::{CodexThreadItem, CodexUsage, ContextTokenKind, ThinkingEffort};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ProjectId(pub(crate) u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WorkspaceId(pub(crate) u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WorkspaceThreadId(pub(crate) u64);

impl WorkspaceThreadId {
    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn from_u64(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainPane {
    None,
    Dashboard,
    ProjectSettings(ProjectId),
    Workspace(WorkspaceId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RightPane {
    None,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceStatus {
    Active,
    Archived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationStatus {
    Idle,
    Running,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationEntry {
    UserMessage { text: String },
    CodexItem { item: Box<CodexThreadItem> },
    TurnUsage { usage: Option<CodexUsage> },
    TurnDuration { duration_ms: u64 },
    TurnCanceled,
    TurnError { message: String },
}

pub(crate) fn codex_item_id(item: &CodexThreadItem) -> &str {
    match item {
        CodexThreadItem::AgentMessage { id, .. } => id,
        CodexThreadItem::Reasoning { id, .. } => id,
        CodexThreadItem::CommandExecution { id, .. } => id,
        CodexThreadItem::FileChange { id, .. } => id,
        CodexThreadItem::McpToolCall { id, .. } => id,
        CodexThreadItem::WebSearch { id, .. } => id,
        CodexThreadItem::TodoList { id, .. } => id,
        CodexThreadItem::Error { id, .. } => id,
    }
}

fn entry_is_same_codex_item(entry: &ConversationEntry, item: &CodexThreadItem) -> bool {
    match entry {
        ConversationEntry::CodexItem { item: existing } => {
            codex_item_id(existing) == codex_item_id(item)
        }
        _ => false,
    }
}

pub(crate) fn entries_contain_codex_item(
    entries: &[ConversationEntry],
    item: &CodexThreadItem,
) -> bool {
    entries.iter().any(|e| entry_is_same_codex_item(e, item))
}

pub(crate) fn flush_in_progress_items(conversation: &mut WorkspaceConversation) {
    let pending = conversation
        .in_progress_order
        .iter()
        .filter_map(|id| conversation.in_progress_items.get(id))
        .cloned()
        .collect::<Vec<_>>();

    for item in pending {
        if entries_contain_codex_item(&conversation.entries, &item) {
            continue;
        }
        conversation.entries.push(ConversationEntry::CodexItem {
            item: Box::new(item),
        });
    }
}

fn entry_is_same(a: &ConversationEntry, b: &ConversationEntry) -> bool {
    match (a, b) {
        (
            ConversationEntry::UserMessage { text: a },
            ConversationEntry::UserMessage { text: b },
        ) => a == b,
        (ConversationEntry::CodexItem { item: a }, ConversationEntry::CodexItem { item: b }) => {
            codex_item_id(a) == codex_item_id(b)
        }
        (ConversationEntry::TurnUsage { usage: a }, ConversationEntry::TurnUsage { usage: b }) => {
            a == b
        }
        (
            ConversationEntry::TurnDuration { duration_ms: a },
            ConversationEntry::TurnDuration { duration_ms: b },
        ) => a == b,
        (ConversationEntry::TurnCanceled, ConversationEntry::TurnCanceled) => true,
        (
            ConversationEntry::TurnError { message: a },
            ConversationEntry::TurnError { message: b },
        ) => a == b,
        _ => false,
    }
}

pub(crate) fn entries_is_prefix(prefix: &[ConversationEntry], full: &[ConversationEntry]) -> bool {
    if prefix.len() > full.len() {
        return false;
    }
    prefix
        .iter()
        .zip(full.iter())
        .all(|(a, b)| entry_is_same(a, b))
}

pub(crate) fn entries_is_suffix(suffix: &[ConversationEntry], full: &[ConversationEntry]) -> bool {
    if suffix.len() > full.len() {
        return false;
    }
    let offset = full.len() - suffix.len();
    suffix
        .iter()
        .zip(full.iter().skip(offset))
        .all(|(a, b)| entry_is_same(a, b))
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConversationSnapshot {
    pub thread_id: Option<String>,
    pub entries: Vec<ConversationEntry>,
}

#[derive(Clone, Debug)]
pub struct ConversationThreadMeta {
    pub thread_id: WorkspaceThreadId,
    pub remote_thread_id: Option<String>,
    pub title: String,
    pub updated_at_unix_seconds: u64,
}

#[derive(Clone, Debug)]
pub struct WorkspaceConversation {
    pub local_thread_id: WorkspaceThreadId,
    pub title: String,
    pub thread_id: Option<String>,
    pub draft: String,
    pub draft_attachments: Vec<DraftAttachment>,
    pub agent_model_id: String,
    pub thinking_effort: ThinkingEffort,
    pub entries: Vec<ConversationEntry>,
    pub run_status: OperationStatus,
    pub current_run_config: Option<AgentRunConfig>,
    pub in_progress_items: BTreeMap<String, CodexThreadItem>,
    pub in_progress_order: VecDeque<String>,
    pub pending_prompts: VecDeque<QueuedPrompt>,
    pub queue_paused: bool,
}

#[derive(Clone, Debug)]
pub struct WorkspaceTabs {
    pub open_tabs: Vec<WorkspaceThreadId>,
    pub archived_tabs: Vec<WorkspaceThreadId>,
    pub active_tab: WorkspaceThreadId,
    pub next_thread_id: u64,
}

impl WorkspaceTabs {
    pub fn new_with_initial(thread_id: WorkspaceThreadId) -> Self {
        Self {
            open_tabs: vec![thread_id],
            archived_tabs: Vec::new(),
            active_tab: thread_id,
            next_thread_id: thread_id.0 + 1,
        }
    }

    pub fn activate(&mut self, thread_id: WorkspaceThreadId) {
        self.active_tab = thread_id;
        self.archived_tabs.retain(|id| *id != thread_id);
        if !self.open_tabs.contains(&thread_id) {
            self.open_tabs.push(thread_id);
        }
    }

    pub fn archive_tab(&mut self, thread_id: WorkspaceThreadId) {
        let mut active_fallback: Option<WorkspaceThreadId> = None;
        if self.active_tab == thread_id
            && let Some(idx) = self.open_tabs.iter().position(|id| *id == thread_id)
        {
            if idx > 0 {
                active_fallback = Some(self.open_tabs[idx - 1]);
            } else if idx + 1 < self.open_tabs.len() {
                active_fallback = Some(self.open_tabs[idx + 1]);
            }
        }
        self.open_tabs.retain(|id| *id != thread_id);
        if !self.archived_tabs.contains(&thread_id) {
            self.archived_tabs.push(thread_id);
        }
        if let Some(next) = active_fallback.or_else(|| self.open_tabs.first().copied()) {
            self.active_tab = next;
        }
    }

    pub fn restore_tab(&mut self, thread_id: WorkspaceThreadId, activate: bool) {
        let was_archived = self.archived_tabs.contains(&thread_id);
        self.archived_tabs.retain(|id| *id != thread_id);
        if was_archived && !self.open_tabs.contains(&thread_id) {
            self.open_tabs.push(thread_id);
        }
        if activate {
            self.active_tab = thread_id;
        }
    }

    pub fn allocate_thread_id(&mut self) -> WorkspaceThreadId {
        let id = WorkspaceThreadId(self.next_thread_id);
        self.next_thread_id += 1;
        id
    }

    pub fn reorder_tab(&mut self, thread_id: WorkspaceThreadId, to_index: usize) {
        let Some(from_index) = self.open_tabs.iter().position(|id| *id == thread_id) else {
            return;
        };
        if from_index == to_index {
            return;
        }
        let tab = self.open_tabs.remove(from_index);
        let mut target = to_index.min(self.open_tabs.len());
        if from_index < to_index {
            target = target.saturating_sub(1);
        }
        self.open_tabs.insert(target, tab);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftAttachment {
    pub id: u64,
    pub kind: ContextTokenKind,
    pub anchor: usize,
    pub path: Option<PathBuf>,
    pub failed: bool,
}

pub(crate) fn apply_draft_text_diff(conversation: &mut WorkspaceConversation, new_text: &str) {
    let old_text = conversation.draft.as_str();
    if old_text == new_text {
        return;
    }

    let old_bytes = old_text.as_bytes();
    let new_bytes = new_text.as_bytes();

    let mut start = 0usize;
    let min_len = old_bytes.len().min(new_bytes.len());
    while start < min_len && old_bytes[start] == new_bytes[start] {
        start += 1;
    }

    let mut old_end = old_bytes.len();
    let mut new_end = new_bytes.len();
    while old_end > start && new_end > start && old_bytes[old_end - 1] == new_bytes[new_end - 1] {
        old_end -= 1;
        new_end -= 1;
    }

    let delta = new_end as isize - old_end as isize;
    for attachment in &mut conversation.draft_attachments {
        let anchor = attachment.anchor;
        if anchor <= start {
            continue;
        }
        if anchor >= old_end {
            let shifted = anchor as isize + delta;
            attachment.anchor = shifted.max(0) as usize;
        } else {
            // Preference A: snap to the start of the deleted/replaced region.
            attachment.anchor = start;
        }
        attachment.anchor = attachment.anchor.min(new_text.len());
    }

    conversation.draft = new_text.to_owned();
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunConfig {
    pub model_id: String,
    pub thinking_effort: ThinkingEffort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrompt {
    pub text: String,
    pub run_config: AgentRunConfig,
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub workspace_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub status: WorkspaceStatus,
    pub last_activity_at: Option<std::time::SystemTime>,
    pub archive_status: OperationStatus,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    pub slug: String,
    pub expanded: bool,
    pub create_workspace_status: OperationStatus,
    pub workspaces: Vec<Workspace>,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub(crate) next_project_id: u64,
    pub(crate) next_workspace_id: u64,

    pub projects: Vec<Project>,
    pub main_pane: MainPane,
    pub right_pane: RightPane,
    pub sidebar_width: Option<u16>,
    pub terminal_pane_width: Option<u16>,
    pub(crate) agent_default_model_id: String,
    pub(crate) agent_default_thinking_effort: ThinkingEffort,
    pub conversations: HashMap<(WorkspaceId, WorkspaceThreadId), WorkspaceConversation>,
    pub workspace_tabs: HashMap<WorkspaceId, WorkspaceTabs>,
    pub dashboard_preview_workspace_id: Option<WorkspaceId>,
    pub last_open_workspace_id: Option<WorkspaceId>,
    pub last_error: Option<String>,
    pub workspace_chat_scroll_y10: HashMap<(WorkspaceId, WorkspaceThreadId), i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedAppState {
    pub projects: Vec<PersistedProject>,
    pub sidebar_width: Option<u16>,
    pub terminal_pane_width: Option<u16>,
    pub agent_default_model_id: Option<String>,
    pub agent_default_thinking_effort: Option<String>,
    pub last_open_workspace_id: Option<u64>,
    pub workspace_active_thread_id: HashMap<u64, u64>,
    pub workspace_open_tabs: HashMap<u64, Vec<u64>>,
    pub workspace_archived_tabs: HashMap<u64, Vec<u64>>,
    pub workspace_next_thread_id: HashMap<u64, u64>,
    pub workspace_chat_scroll_y10: HashMap<(u64, u64), i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedProject {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    pub slug: String,
    pub expanded: bool,
    pub workspaces: Vec<PersistedWorkspace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedWorkspace {
    pub id: u64,
    pub workspace_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub status: WorkspaceStatus,
    pub last_activity_at_unix_seconds: Option<u64>,
}
