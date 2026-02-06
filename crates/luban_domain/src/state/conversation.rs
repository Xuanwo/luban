use super::{
    MAX_CONVERSATION_ENTRIES_IN_MEMORY, WorkspaceThreadId,
    agent::{AgentRunConfig, QueuedPrompt},
    attachments::AttachmentRef,
    layout::OperationStatus,
};
use crate::{CodexThreadItem, CodexUsage, ContextTokenKind, TaskStatus, ThinkingEffort};
use std::collections::VecDeque;

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ConversationSystemEvent {
    TaskCreated,
    TaskArchived,
    TaskStatusChanged {
        from: TaskStatus,
        to: TaskStatus,
    },
    TaskStatusSuggestion {
        from: TaskStatus,
        to: TaskStatus,
        #[serde(default)]
        title: String,
        #[serde(default)]
        explanation_markdown: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserEvent {
    Message {
        text: String,
        #[serde(default)]
        attachments: Vec<AttachmentRef>,
    },
    TerminalCommandStarted {
        id: String,
        command: String,
        reconnect: String,
    },
    TerminalCommandFinished {
        id: String,
        command: String,
        reconnect: String,
        #[serde(default)]
        output_base64: String,
        #[serde(default)]
        output_byte_len: u64,
    },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    Message { id: String, text: String },
    Item { item: Box<CodexThreadItem> },
    TurnUsage { usage: Option<CodexUsage> },
    TurnDuration { duration_ms: u64 },
    TurnCanceled,
    TurnError { message: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationEntry {
    SystemEvent {
        #[serde(rename = "entry_id", alias = "id")]
        entry_id: String,
        created_at_unix_ms: u64,
        event: ConversationSystemEvent,
    },
    UserEvent {
        #[serde(default)]
        entry_id: String,
        #[serde(default)]
        created_at_unix_ms: u64,
        event: UserEvent,
    },
    AgentEvent {
        #[serde(default)]
        entry_id: String,
        #[serde(default)]
        created_at_unix_ms: u64,
        /// Which agent runner produced this entry. Populated automatically by
        /// `push_entry_and_update_totals` from the active run config so that
        /// each message retains the icon of the model that generated it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runner: Option<crate::AgentRunnerKind>,
        event: AgentEvent,
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

fn entry_is_same(a: &ConversationEntry, b: &ConversationEntry) -> bool {
    match (a, b) {
        (
            ConversationEntry::SystemEvent {
                entry_id: a_id,
                created_at_unix_ms: a_created_at,
                event: a_event,
            },
            ConversationEntry::SystemEvent {
                entry_id: b_id,
                created_at_unix_ms: b_created_at,
                event: b_event,
            },
        ) => a_id == b_id && a_created_at == b_created_at && a_event == b_event,
        (
            ConversationEntry::UserEvent {
                entry_id: a_id,
                created_at_unix_ms: a_created_at,
                event: a,
            },
            ConversationEntry::UserEvent {
                entry_id: b_id,
                created_at_unix_ms: b_created_at,
                event: b,
            },
        ) => a_id == b_id && a_created_at == b_created_at && a == b,
        (
            ConversationEntry::AgentEvent {
                entry_id: a_entry_id,
                created_at_unix_ms: a_created_at,
                event: a,
                ..
            },
            ConversationEntry::AgentEvent {
                entry_id: b_entry_id,
                created_at_unix_ms: b_created_at,
                event: b,
                ..
            },
        ) => match (a, b) {
            (
                AgentEvent::Message {
                    id: a_id,
                    text: a_text,
                },
                AgentEvent::Message {
                    id: b_id,
                    text: b_text,
                },
            ) => {
                a_entry_id == b_entry_id
                    && a_created_at == b_created_at
                    && a_id == b_id
                    && a_text == b_text
            }
            (AgentEvent::Item { item: a_item }, AgentEvent::Item { item: b_item }) => {
                a_entry_id == b_entry_id
                    && a_created_at == b_created_at
                    && codex_item_id(a_item) == codex_item_id(b_item)
            }
            (
                AgentEvent::TurnUsage { usage: a_usage },
                AgentEvent::TurnUsage { usage: b_usage },
            ) => a_entry_id == b_entry_id && a_created_at == b_created_at && a_usage == b_usage,
            (
                AgentEvent::TurnDuration { duration_ms: a },
                AgentEvent::TurnDuration { duration_ms: b },
            ) => a_entry_id == b_entry_id && a_created_at == b_created_at && a == b,
            (AgentEvent::TurnCanceled, AgentEvent::TurnCanceled) => {
                a_entry_id == b_entry_id && a_created_at == b_created_at
            }
            (AgentEvent::TurnError { message: a }, AgentEvent::TurnError { message: b }) => {
                a_entry_id == b_entry_id && a_created_at == b_created_at && a == b
            }
            _ => false,
        },
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
    #[serde(default)]
    pub title: Option<String>,
    pub thread_id: Option<String>,
    #[serde(default = "default_task_status")]
    pub task_status: TaskStatus,
    #[serde(default)]
    pub runner: Option<crate::AgentRunnerKind>,
    #[serde(default)]
    pub agent_model_id: Option<String>,
    #[serde(default)]
    pub thinking_effort: Option<crate::ThinkingEffort>,
    #[serde(default)]
    pub amp_mode: Option<String>,
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

impl ConversationSnapshot {
    pub(crate) fn ensure_entry_ids(&mut self) {
        let base = self.entries_start;
        for (idx, entry) in self.entries.iter_mut().enumerate() {
            let slot = match entry {
                ConversationEntry::SystemEvent { entry_id, .. } => entry_id,
                ConversationEntry::UserEvent { entry_id, .. } => entry_id,
                ConversationEntry::AgentEvent { entry_id, .. } => entry_id,
            };
            if slot.is_empty() {
                *slot = format!("e_{}", base.saturating_add(idx as u64).saturating_add(1));
            }
        }
    }
}

fn default_task_status() -> TaskStatus {
    TaskStatus::Todo
}

#[derive(Clone, Debug)]
pub struct ConversationThreadMeta {
    pub thread_id: WorkspaceThreadId,
    pub remote_thread_id: Option<String>,
    pub title: String,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    pub task_status: TaskStatus,
    pub last_message_seq: u64,
    pub task_status_last_analyzed_message_seq: u64,
    pub turn_status: crate::TurnStatus,
    pub last_turn_result: Option<crate::TurnResult>,
}

#[derive(Clone, Debug)]
pub struct WorkspaceConversation {
    pub local_thread_id: WorkspaceThreadId,
    pub title: String,
    pub thread_id: Option<String>,
    pub task_status: TaskStatus,
    pub draft: String,
    pub draft_attachments: Vec<DraftAttachment>,
    pub run_config_overridden_by_user: bool,
    pub agent_runner: crate::AgentRunnerKind,
    pub agent_model_id: String,
    pub thinking_effort: ThinkingEffort,
    pub amp_mode: Option<String>,
    pub entries: Vec<ConversationEntry>,
    pub entries_total: u64,
    pub entries_start: u64,
    pub active_run_id: Option<u64>,
    pub next_run_id: u64,
    pub run_status: OperationStatus,
    pub run_started_at_unix_ms: Option<u64>,
    pub run_finished_at_unix_ms: Option<u64>,
    pub current_run_config: Option<AgentRunConfig>,
    pub next_queued_prompt_id: u64,
    pub pending_prompts: VecDeque<QueuedPrompt>,
    pub queue_paused: bool,
}

impl WorkspaceConversation {
    pub(crate) fn reset_entries_from_snapshot(&mut self, snapshot: ConversationSnapshot) {
        self.task_status = snapshot.task_status;
        self.entries = snapshot.entries;
        self.entries_total = snapshot.entries_total.max(
            snapshot
                .entries_start
                .saturating_add(self.entries.len() as u64),
        );
        self.entries_start = snapshot.entries_start;
        self.ensure_loaded_entry_ids();
        self.run_started_at_unix_ms = snapshot.run_started_at_unix_ms;
        self.run_finished_at_unix_ms = snapshot.run_finished_at_unix_ms;
        self.trim_entries_to_limit();
    }

    fn push_entry_and_update_totals(&mut self, entry: ConversationEntry) {
        let mut entry = entry;
        // Reason: auto-fill runner so each entry records which agent produced it,
        // enabling per-message model icons even when the user switches models.
        if let ConversationEntry::AgentEvent { runner, .. } = &mut entry
            && runner.is_none()
        {
            *runner = Some(
                self.current_run_config
                    .as_ref()
                    .map(|c| c.runner)
                    .unwrap_or(self.agent_runner),
            );
        }
        self.ensure_entry_created_at(&mut entry);
        self.ensure_entry_id(&mut entry);
        self.entries.push(entry);
        self.entries_total = self
            .entries_total
            .max(self.entries_start.saturating_add(self.entries.len() as u64));
        self.trim_entries_to_limit();
    }

    pub(crate) fn push_entry(&mut self, entry: ConversationEntry) {
        self.push_entry_and_update_totals(entry);
    }

    pub(crate) fn push_codex_item(&mut self, item: CodexThreadItem) {
        if self.should_skip_codex_item(&item) {
            return;
        }

        let entry = match item {
            CodexThreadItem::AgentMessage { id, text } => ConversationEntry::AgentEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                runner: None,
                event: AgentEvent::Message { id, text },
            },
            other => ConversationEntry::AgentEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                runner: None,
                event: AgentEvent::Item {
                    item: Box::new(other),
                },
            },
        };
        self.push_entry(entry);
    }

    fn should_skip_codex_item(&self, item: &CodexThreadItem) -> bool {
        let incoming_id = codex_item_id(item);
        for entry in self.entries.iter().rev() {
            let ConversationEntry::AgentEvent { event, .. } = entry else {
                continue;
            };

            match event {
                AgentEvent::Message { id, text } if id == incoming_id => {
                    return match item {
                        CodexThreadItem::AgentMessage { text: incoming, .. } => incoming == text,
                        _ => false,
                    };
                }
                AgentEvent::Item { item: existing }
                    if codex_item_id(existing.as_ref()) == incoming_id =>
                {
                    let existing = serde_json::to_value(existing.as_ref());
                    let incoming = serde_json::to_value(item);
                    return existing.ok() == incoming.ok();
                }
                _ => continue,
            };
        }
        false
    }

    fn ensure_entry_id(&mut self, entry: &mut ConversationEntry) {
        let slot = match entry {
            ConversationEntry::SystemEvent { entry_id, .. } => entry_id,
            ConversationEntry::UserEvent { entry_id, .. } => entry_id,
            ConversationEntry::AgentEvent { entry_id, .. } => entry_id,
        };

        if slot.is_empty() {
            *slot = format!("e_{}", self.entries_total.saturating_add(1));
        }
    }

    fn ensure_entry_created_at(&self, entry: &mut ConversationEntry) {
        match entry {
            ConversationEntry::SystemEvent { .. } => {}
            ConversationEntry::UserEvent {
                created_at_unix_ms, ..
            } => {
                if *created_at_unix_ms == 0 {
                    *created_at_unix_ms = now_unix_ms();
                }
            }
            ConversationEntry::AgentEvent {
                created_at_unix_ms, ..
            } => {
                if *created_at_unix_ms == 0 {
                    *created_at_unix_ms = now_unix_ms();
                }
            }
        }
    }

    fn ensure_loaded_entry_ids(&mut self) {
        let base = self.entries_start;
        for (idx, entry) in self.entries.iter_mut().enumerate() {
            let slot = match entry {
                ConversationEntry::SystemEvent { entry_id, .. } => entry_id,
                ConversationEntry::UserEvent { entry_id, .. } => entry_id,
                ConversationEntry::AgentEvent { entry_id, .. } => entry_id,
            };
            if slot.is_empty() {
                *slot = format!("e_{}", base.saturating_add(idx as u64).saturating_add(1));
            }
        }
    }

    fn trim_entries_to_limit(&mut self) {
        if self.entries.len() <= MAX_CONVERSATION_ENTRIES_IN_MEMORY {
            return;
        }
        let overflow = self.entries.len() - MAX_CONVERSATION_ENTRIES_IN_MEMORY;
        self.entries.drain(0..overflow);
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
    fn trim_entries_drops_overflow_and_updates_entries_start() {
        let state = crate::AppState::new();
        let mut conversation = state.default_conversation(WorkspaceThreadId(1));
        conversation.entries_start = 100;

        let mut entries = Vec::with_capacity(MAX_CONVERSATION_ENTRIES_IN_MEMORY + 2);
        entries.push(ConversationEntry::AgentEvent {
            entry_id: "e_item_a".to_owned(),
            created_at_unix_ms: 1,
            runner: None,
            event: AgentEvent::Message {
                id: "item-a".to_owned(),
                text: "a".to_owned(),
            },
        });
        entries.push(ConversationEntry::AgentEvent {
            entry_id: "e_item_b".to_owned(),
            created_at_unix_ms: 2,
            runner: None,
            event: AgentEvent::Message {
                id: "item-b".to_owned(),
                text: "b".to_owned(),
            },
        });
        entries.push(ConversationEntry::AgentEvent {
            entry_id: "e_item_c".to_owned(),
            created_at_unix_ms: 3,
            runner: None,
            event: AgentEvent::Message {
                id: "item-c".to_owned(),
                text: "c".to_owned(),
            },
        });
        for idx in 0..(MAX_CONVERSATION_ENTRIES_IN_MEMORY - 1) {
            entries.push(ConversationEntry::UserEvent {
                entry_id: format!("e_user_{idx}"),
                created_at_unix_ms: 10 + idx as u64,
                event: UserEvent::Message {
                    text: format!("user-{idx}"),
                    attachments: Vec::new(),
                },
            });
        }

        conversation.entries = entries;
        conversation.trim_entries_to_limit();

        assert_eq!(
            conversation.entries.len(),
            MAX_CONVERSATION_ENTRIES_IN_MEMORY
        );
        assert_eq!(conversation.entries_start, 102);

        let first_entry_id = match &conversation.entries[0] {
            ConversationEntry::AgentEvent { event, .. } => match event {
                AgentEvent::Message { id, .. } => id.as_str(),
                AgentEvent::Item { item } => codex_item_id(item.as_ref()),
                other => panic!("expected agent event entry, got {other:?}"),
            },
            other => panic!("expected codex item entry, got {other:?}"),
        };
        assert_eq!(first_entry_id, "item-c");
    }

    #[test]
    fn push_codex_item_appends_updates_and_assigns_entry_ids() {
        let state = crate::AppState::new();
        let mut conversation = state.default_conversation(WorkspaceThreadId(1));

        conversation.push_codex_item(CodexThreadItem::CommandExecution {
            id: "cmd_1".to_owned(),
            command: "echo hi".to_owned(),
            aggregated_output: String::new(),
            exit_code: None,
            status: crate::CodexCommandExecutionStatus::InProgress,
        });
        conversation.push_codex_item(CodexThreadItem::CommandExecution {
            id: "cmd_1".to_owned(),
            command: "echo hi".to_owned(),
            aggregated_output: "hi\n".to_owned(),
            exit_code: Some(0),
            status: crate::CodexCommandExecutionStatus::Completed,
        });

        assert_eq!(conversation.entries.len(), 2);
        assert_eq!(conversation.entries_total, 2);

        let (first_entry_id, first_item_id) = match &conversation.entries[0] {
            ConversationEntry::AgentEvent {
                entry_id, event, ..
            } => match event {
                AgentEvent::Item { item } => (entry_id.as_str(), codex_item_id(item.as_ref())),
                other => panic!("expected agent item entry, got {other:?}"),
            },
            other => panic!("expected agent event entry, got {other:?}"),
        };
        let (second_entry_id, second_item_id) = match &conversation.entries[1] {
            ConversationEntry::AgentEvent {
                entry_id, event, ..
            } => match event {
                AgentEvent::Item { item } => (entry_id.as_str(), codex_item_id(item.as_ref())),
                other => panic!("expected agent item entry, got {other:?}"),
            },
            other => panic!("expected agent event entry, got {other:?}"),
        };

        assert_eq!(first_item_id, "cmd_1");
        assert_eq!(second_item_id, "cmd_1");
        assert_ne!(first_entry_id, second_entry_id);
    }

    #[test]
    fn push_entry_fills_created_at_unix_ms_for_user_and_agent_entries() {
        let state = crate::AppState::new();
        let mut conversation = state.default_conversation(WorkspaceThreadId(1));

        conversation.push_entry(ConversationEntry::UserEvent {
            entry_id: String::new(),
            created_at_unix_ms: 0,
            event: UserEvent::Message {
                text: "hello".to_owned(),
                attachments: Vec::new(),
            },
        });
        let user_created_at = match conversation.entries.last().expect("user entry") {
            ConversationEntry::UserEvent {
                created_at_unix_ms, ..
            } => *created_at_unix_ms,
            other => panic!("expected UserEvent entry, got {other:?}"),
        };
        assert!(user_created_at > 0);

        conversation.push_entry(ConversationEntry::AgentEvent {
            entry_id: String::new(),
            created_at_unix_ms: 0,
            runner: None,
            event: AgentEvent::Message {
                id: "agent-msg".to_owned(),
                text: "hi".to_owned(),
            },
        });
        let agent_created_at = match conversation.entries.last().expect("agent entry") {
            ConversationEntry::AgentEvent {
                created_at_unix_ms, ..
            } => *created_at_unix_ms,
            other => panic!("expected AgentEvent entry, got {other:?}"),
        };
        assert!(agent_created_at > 0);
    }
}
