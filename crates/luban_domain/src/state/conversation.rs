use super::{
    MAX_CONVERSATION_ENTRIES_IN_MEMORY, WorkspaceThreadId,
    agent::{AgentRunConfig, QueuedPrompt},
    attachments::AttachmentRef,
    layout::OperationStatus,
};
use crate::{CodexThreadItem, CodexUsage, ContextTokenKind, ThinkingEffort};
use std::collections::{BTreeMap, HashSet, VecDeque};

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
    #[serde(default)]
    pub agent_model_id: Option<String>,
    #[serde(default)]
    pub thinking_effort: Option<crate::ThinkingEffort>,
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
    pub run_config_overridden_by_user: bool,
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
        self.codex_item_ids.reserve(self.entries.len());
        for entry in &self.entries {
            if let ConversationEntry::CodexItem { item } = entry {
                self.codex_item_ids
                    .insert(codex_item_id(item.as_ref()).to_owned());
            }
        }
    }

    fn push_entry_and_update_totals(&mut self, entry: ConversationEntry) {
        self.entries.push(entry);
        self.entries_total = self
            .entries_total
            .max(self.entries_start.saturating_add(self.entries.len() as u64));
        self.trim_entries_to_limit();
    }

    pub(crate) fn push_entry(&mut self, entry: ConversationEntry) {
        if let ConversationEntry::CodexItem { item } = &entry {
            self.codex_item_ids
                .insert(codex_item_id(item.as_ref()).to_owned());
        }
        self.push_entry_and_update_totals(entry);
    }

    pub(crate) fn push_codex_item_if_new(&mut self, item: CodexThreadItem) -> bool {
        let id = codex_item_id(&item);
        if self.codex_item_ids.contains(id) {
            return false;
        }
        self.codex_item_ids.insert(id.to_owned());
        self.push_entry_and_update_totals(ConversationEntry::CodexItem {
            item: Box::new(item),
        });
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
            attachment.anchor = start;
        }
        attachment.anchor = attachment.anchor.min(new_len);
    }

    conversation.draft = new_text.to_owned();
}

#[cfg(test)]
mod tests {
    use super::*;

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
