use crate::{
    CodexThreadItem, CodexUsage, ContextTokenKind, SystemTaskKind, TaskIntentKind, ThinkingEffort,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ProjectId(pub(crate) u64);

impl ProjectId {
    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn from_u64(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WorkspaceId(pub(crate) u64);

impl WorkspaceId {
    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn from_u64(id: u64) -> Self {
        Self(id)
    }
}

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppearanceTheme {
    Light,
    Dark,
    #[default]
    System,
}

impl AppearanceTheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppearanceFonts {
    pub ui_font: String,
    pub chat_font: String,
    pub code_font: String,
    pub terminal_font: String,
}

impl Default for AppearanceFonts {
    fn default() -> Self {
        Self {
            ui_font: "Inter".to_owned(),
            chat_font: "Inter".to_owned(),
            code_font: "Geist Mono".to_owned(),
            terminal_font: "Geist Mono".to_owned(),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationEntry {
    UserMessage {
        text: String,
        #[serde(default)]
        attachments: Vec<AttachmentRef>,
    },
    CodexItem {
        item: Box<CodexThreadItem>,
    },
    TurnUsage {
        usage: Option<CodexUsage>,
    },
    TurnDuration {
        duration_ms: u64,
    },
    TurnCanceled,
    TurnError {
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    Text,
    File,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AttachmentRef {
    pub id: String,
    pub kind: AttachmentKind,
    pub name: String,
    pub extension: String,
    pub mime: Option<String>,
    pub byte_len: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContextItem {
    pub id: u64,
    pub attachment: AttachmentRef,
    pub created_at_unix_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatScrollAnchor {
    FollowTail,
    Block {
        block_id: String,
        block_index: u32,
        offset_in_block_y10: i32,
    },
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

pub(crate) fn flush_in_progress_items(conversation: &mut WorkspaceConversation) {
    let order = std::mem::take(&mut conversation.in_progress_order);
    for id in order.iter() {
        let item = conversation.in_progress_items.get(id).cloned();
        if let Some(item) = item {
            conversation.push_codex_item_if_new(item);
        }
    }
    conversation.in_progress_order = order;
}

fn entry_is_same(a: &ConversationEntry, b: &ConversationEntry) -> bool {
    match (a, b) {
        (
            ConversationEntry::UserMessage {
                text: a,
                attachments: a_attachments,
            },
            ConversationEntry::UserMessage {
                text: b,
                attachments: b_attachments,
            },
        ) => a == b && a_attachments == b_attachments,
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
    #[serde(default)]
    pub entries_total: u64,
    #[serde(default)]
    pub entries_start: u64,
    #[serde(default)]
    pub pending_prompts: Vec<QueuedPrompt>,
    #[serde(default)]
    pub queue_paused: bool,
    #[serde(default)]
    pub run_started_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub run_finished_at_unix_ms: Option<u64>,
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
    pub entries_total: u64,
    pub entries_start: u64,
    pub codex_item_ids: HashSet<String>,
    pub active_run_id: Option<u64>,
    pub next_run_id: u64,
    pub run_status: OperationStatus,
    pub run_started_at_unix_ms: Option<u64>,
    pub run_finished_at_unix_ms: Option<u64>,
    pub current_run_config: Option<AgentRunConfig>,
    pub in_progress_items: BTreeMap<String, CodexThreadItem>,
    pub in_progress_order: VecDeque<String>,
    pub next_queued_prompt_id: u64,
    pub pending_prompts: VecDeque<QueuedPrompt>,
    pub queue_paused: bool,
}

pub(crate) const MAX_CONVERSATION_ENTRIES_IN_MEMORY: usize = 5000;

impl WorkspaceConversation {
    pub(crate) fn reset_entries_from_snapshot(&mut self, snapshot: ConversationSnapshot) {
        self.entries = snapshot.entries;
        self.entries_total = snapshot.entries_total.max(
            snapshot
                .entries_start
                .saturating_add(self.entries.len() as u64),
        );
        self.entries_start = snapshot.entries_start;
        self.run_started_at_unix_ms = snapshot.run_started_at_unix_ms;
        self.run_finished_at_unix_ms = snapshot.run_finished_at_unix_ms;
        self.rebuild_codex_item_ids();
        self.trim_entries_to_limit();
    }

    pub(crate) fn rebuild_codex_item_ids(&mut self) {
        self.codex_item_ids.clear();
        for entry in &self.entries {
            if let ConversationEntry::CodexItem { item } = entry {
                self.codex_item_ids
                    .insert(codex_item_id(item.as_ref()).to_owned());
            }
        }
    }

    pub(crate) fn push_entry(&mut self, entry: ConversationEntry) {
        if let ConversationEntry::CodexItem { item } = &entry {
            self.codex_item_ids
                .insert(codex_item_id(item.as_ref()).to_owned());
        }
        self.entries.push(entry);
        self.entries_total = self
            .entries_total
            .max(self.entries_start.saturating_add(self.entries.len() as u64));
        self.trim_entries_to_limit();
    }

    pub(crate) fn push_codex_item_if_new(&mut self, item: CodexThreadItem) -> bool {
        let id = codex_item_id(&item);
        if self.codex_item_ids.contains(id) {
            return false;
        }
        self.codex_item_ids.insert(id.to_owned());
        self.entries.push(ConversationEntry::CodexItem {
            item: Box::new(item),
        });
        self.entries_total = self
            .entries_total
            .max(self.entries_start.saturating_add(self.entries.len() as u64));
        self.trim_entries_to_limit();
        true
    }

    fn trim_entries_to_limit(&mut self) {
        if self.entries.len() <= MAX_CONVERSATION_ENTRIES_IN_MEMORY {
            return;
        }
        let overflow = self.entries.len() - MAX_CONVERSATION_ENTRIES_IN_MEMORY;
        let (entries, codex_item_ids) = (&mut self.entries, &mut self.codex_item_ids);
        for entry in entries.drain(0..overflow) {
            if let ConversationEntry::CodexItem { item } = entry {
                codex_item_ids.remove(codex_item_id(item.as_ref()));
            }
        }
        self.entries_start = self.entries_start.saturating_add(overflow as u64);
    }
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
        self.archived_tabs.retain(|id| *id != thread_id);
        if !self.open_tabs.contains(&thread_id) {
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

    pub fn reorder_tab(&mut self, thread_id: WorkspaceThreadId, to_index: usize) -> bool {
        let Some(from_index) = self.open_tabs.iter().position(|id| *id == thread_id) else {
            return false;
        };
        if from_index == to_index {
            return false;
        }
        let tab = self.open_tabs.remove(from_index);
        let mut target = to_index.min(self.open_tabs.len());
        if from_index < to_index {
            target = target.saturating_sub(1);
        }
        self.open_tabs.insert(target, tab);
        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftAttachment {
    pub id: u64,
    pub kind: ContextTokenKind,
    pub anchor: usize,
    pub attachment: Option<AttachmentRef>,
    pub failed: bool,
}

fn draft_text_diff_window(old_text: &str, new_text: &str) -> (usize, usize, usize) {
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

    (start, old_end, new_end)
}

pub(crate) fn apply_draft_text_diff(conversation: &mut WorkspaceConversation, new_text: &str) {
    let old_text = conversation.draft.as_str();
    if old_text == new_text {
        return;
    }

    let (start, old_end, new_end) = draft_text_diff_window(old_text, new_text);

    let delta = new_end as isize - old_end as isize;
    let new_len = new_text.len();
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
        attachment.anchor = attachment.anchor.min(new_len);
    }

    conversation.draft = new_text.to_owned();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_tab_opens_even_when_not_archived() {
        let mut tabs = WorkspaceTabs::new_with_initial(WorkspaceThreadId(1));
        tabs.restore_tab(WorkspaceThreadId(2), true);
        assert_eq!(tabs.active_tab, WorkspaceThreadId(2));
        assert!(tabs.open_tabs.contains(&WorkspaceThreadId(2)));
        assert!(!tabs.archived_tabs.contains(&WorkspaceThreadId(2)));
    }

    fn conversation_with_draft(draft: &str, anchors: &[usize]) -> WorkspaceConversation {
        let state = crate::AppState::new();
        let mut conversation = state.default_conversation(WorkspaceThreadId(1));
        conversation.draft = draft.to_owned();
        conversation.draft_attachments = anchors
            .iter()
            .copied()
            .enumerate()
            .map(|(idx, anchor)| DraftAttachment {
                id: idx as u64 + 1,
                kind: crate::ContextTokenKind::File,
                anchor,
                attachment: None,
                failed: false,
            })
            .collect();
        conversation
    }

    #[test]
    fn draft_diff_shifts_anchors_after_insertion() {
        let mut conversation = conversation_with_draft("hello", &[0, 2, 3, 5]);
        apply_draft_text_diff(&mut conversation, "heXllo");

        let anchors: Vec<usize> = conversation
            .draft_attachments
            .iter()
            .map(|a| a.anchor)
            .collect();
        assert_eq!(anchors, vec![0, 2, 4, 6]);
    }

    #[test]
    fn draft_diff_shifts_anchors_after_deletion() {
        let mut conversation = conversation_with_draft("hello", &[0, 1, 2, 4]);
        apply_draft_text_diff(&mut conversation, "hllo");

        let anchors: Vec<usize> = conversation
            .draft_attachments
            .iter()
            .map(|a| a.anchor)
            .collect();
        assert_eq!(anchors, vec![0, 1, 1, 3]);
    }

    #[test]
    fn draft_diff_snaps_anchors_inside_replaced_region() {
        let mut conversation = conversation_with_draft("hello", &[1, 3, 4]);
        apply_draft_text_diff(&mut conversation, "heLLo");

        let anchors: Vec<usize> = conversation
            .draft_attachments
            .iter()
            .map(|a| a.anchor)
            .collect();
        assert_eq!(anchors, vec![1, 2, 4]);
    }

    #[test]
    fn trim_entries_drops_overflow_and_updates_codex_item_ids() {
        let state = crate::AppState::new();
        let mut conversation = state.default_conversation(WorkspaceThreadId(1));
        conversation.entries_start = 100;

        let mut entries = Vec::with_capacity(MAX_CONVERSATION_ENTRIES_IN_MEMORY + 2);
        entries.push(ConversationEntry::CodexItem {
            item: Box::new(CodexThreadItem::AgentMessage {
                id: "item-a".to_owned(),
                text: "a".to_owned(),
            }),
        });
        entries.push(ConversationEntry::CodexItem {
            item: Box::new(CodexThreadItem::AgentMessage {
                id: "item-b".to_owned(),
                text: "b".to_owned(),
            }),
        });
        entries.push(ConversationEntry::CodexItem {
            item: Box::new(CodexThreadItem::AgentMessage {
                id: "item-c".to_owned(),
                text: "c".to_owned(),
            }),
        });
        for idx in 0..(MAX_CONVERSATION_ENTRIES_IN_MEMORY - 1) {
            entries.push(ConversationEntry::UserMessage {
                text: format!("user-{idx}"),
                attachments: Vec::new(),
            });
        }

        conversation.entries = entries;
        conversation.rebuild_codex_item_ids();
        conversation.trim_entries_to_limit();

        assert_eq!(
            conversation.entries.len(),
            MAX_CONVERSATION_ENTRIES_IN_MEMORY
        );
        assert_eq!(conversation.entries_start, 102);
        assert!(!conversation.codex_item_ids.contains("item-a"));
        assert!(!conversation.codex_item_ids.contains("item-b"));
        assert!(conversation.codex_item_ids.contains("item-c"));

        let first_entry_id = match &conversation.entries[0] {
            ConversationEntry::CodexItem { item } => codex_item_id(item.as_ref()),
            other => panic!("expected codex item entry, got {other:?}"),
        };
        assert_eq!(first_entry_id, "item-c");
    }

    #[test]
    fn flush_in_progress_items_preserves_order_and_avoids_duplicates() {
        let state = crate::AppState::new();
        let mut conversation = state.default_conversation(WorkspaceThreadId(1));

        for (id, text) in [("item-a", "a"), ("item-b", "b")] {
            let id = id.to_owned();
            conversation.in_progress_items.insert(
                id.clone(),
                CodexThreadItem::AgentMessage {
                    id: id.clone(),
                    text: text.to_owned(),
                },
            );
            conversation.in_progress_order.push_back(id);
        }
        let expected_order = conversation.in_progress_order.clone();

        flush_in_progress_items(&mut conversation);
        assert_eq!(conversation.in_progress_order, expected_order);
        assert_eq!(conversation.entries.len(), 2);
        assert!(conversation.codex_item_ids.contains("item-a"));
        assert!(conversation.codex_item_ids.contains("item-b"));

        flush_in_progress_items(&mut conversation);
        assert_eq!(conversation.in_progress_order, expected_order);
        assert_eq!(conversation.entries.len(), 2);
    }

    #[test]
    fn push_codex_item_if_new_is_idempotent_and_updates_entries_total() {
        let state = crate::AppState::new();
        let mut conversation = state.default_conversation(WorkspaceThreadId(1));

        assert!(
            conversation.push_codex_item_if_new(CodexThreadItem::AgentMessage {
                id: "item-a".to_owned(),
                text: "a".to_owned(),
            })
        );
        assert_eq!(conversation.entries.len(), 1);
        assert_eq!(conversation.entries_total, 1);
        assert!(conversation.codex_item_ids.contains("item-a"));

        assert!(
            !conversation.push_codex_item_if_new(CodexThreadItem::AgentMessage {
                id: "item-a".to_owned(),
                text: "a2".to_owned(),
            })
        );
        assert_eq!(conversation.entries.len(), 1);
        assert_eq!(conversation.entries_total, 1);

        let first_entry_id = match &conversation.entries[0] {
            ConversationEntry::CodexItem { item } => codex_item_id(item.as_ref()),
            other => panic!("expected codex item entry, got {other:?}"),
        };
        assert_eq!(first_entry_id, "item-a");
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentRunConfig {
    #[serde(default = "crate::default_agent_runner_kind")]
    pub runner: crate::AgentRunnerKind,
    pub model_id: String,
    pub thinking_effort: ThinkingEffort,
    #[serde(default)]
    pub amp_mode: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QueuedPrompt {
    pub id: u64,
    pub text: String,
    pub attachments: Vec<AttachmentRef>,
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
    pub branch_rename_status: OperationStatus,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    pub slug: String,
    pub is_git: bool,
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
    pub global_zoom_percent: u16,
    pub appearance_theme: AppearanceTheme,
    pub appearance_fonts: AppearanceFonts,
    pub(crate) agent_default_model_id: String,
    pub(crate) agent_default_thinking_effort: ThinkingEffort,
    pub(crate) agent_default_runner: crate::AgentRunnerKind,
    pub(crate) agent_amp_mode: String,
    pub(crate) agent_codex_enabled: bool,
    pub(crate) agent_amp_enabled: bool,
    pub conversations: HashMap<(WorkspaceId, WorkspaceThreadId), WorkspaceConversation>,
    pub workspace_tabs: HashMap<WorkspaceId, WorkspaceTabs>,
    pub dashboard_preview_workspace_id: Option<WorkspaceId>,
    pub last_open_workspace_id: Option<WorkspaceId>,
    pub open_button_selection: Option<String>,
    pub last_error: Option<String>,
    pub workspace_chat_scroll_y10: HashMap<(WorkspaceId, WorkspaceThreadId), i32>,
    pub workspace_chat_scroll_anchor: HashMap<(WorkspaceId, WorkspaceThreadId), ChatScrollAnchor>,
    pub workspace_unread_completions: HashSet<WorkspaceId>,
    pub task_prompt_templates: HashMap<TaskIntentKind, String>,
    pub system_prompt_templates: HashMap<SystemTaskKind, String>,
}

impl AppState {
    pub fn agent_codex_enabled(&self) -> bool {
        self.agent_codex_enabled
    }

    pub fn agent_amp_enabled(&self) -> bool {
        self.agent_amp_enabled
    }

    pub fn agent_default_model_id(&self) -> &str {
        &self.agent_default_model_id
    }

    pub fn agent_default_thinking_effort(&self) -> ThinkingEffort {
        self.agent_default_thinking_effort
    }

    pub fn agent_default_runner(&self) -> crate::AgentRunnerKind {
        self.agent_default_runner
    }

    pub fn agent_amp_mode(&self) -> &str {
        &self.agent_amp_mode
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedAppState {
    pub projects: Vec<PersistedProject>,
    pub sidebar_width: Option<u16>,
    pub terminal_pane_width: Option<u16>,
    pub global_zoom_percent: Option<u16>,
    pub appearance_theme: Option<String>,
    pub appearance_ui_font: Option<String>,
    pub appearance_chat_font: Option<String>,
    pub appearance_code_font: Option<String>,
    pub appearance_terminal_font: Option<String>,
    pub agent_default_model_id: Option<String>,
    pub agent_default_thinking_effort: Option<String>,
    pub agent_default_runner: Option<String>,
    pub agent_amp_mode: Option<String>,
    pub agent_codex_enabled: Option<bool>,
    pub agent_amp_enabled: Option<bool>,
    pub last_open_workspace_id: Option<u64>,
    pub open_button_selection: Option<String>,
    pub workspace_active_thread_id: HashMap<u64, u64>,
    pub workspace_open_tabs: HashMap<u64, Vec<u64>>,
    pub workspace_archived_tabs: HashMap<u64, Vec<u64>>,
    pub workspace_next_thread_id: HashMap<u64, u64>,
    pub workspace_chat_scroll_y10: HashMap<(u64, u64), i32>,
    pub workspace_chat_scroll_anchor: HashMap<(u64, u64), ChatScrollAnchor>,
    pub workspace_unread_completions: HashMap<u64, bool>,
    pub task_prompt_templates: HashMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedProject {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    pub slug: String,
    pub is_git: bool,
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
