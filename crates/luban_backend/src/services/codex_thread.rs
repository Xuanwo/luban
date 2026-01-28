use luban_domain::{CodexThreadEvent, CodexThreadItem};
use rand::{Rng as _, rngs::OsRng};

use crate::time::unix_epoch_micros_now;

pub(super) fn generate_turn_scope_id() -> String {
    let micros = unix_epoch_micros_now();
    let rand: u64 = OsRng.r#gen();
    format!("turn-{micros:x}-{rand:x}")
}

pub(super) fn codex_item_id(item: &CodexThreadItem) -> &str {
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

fn codex_item_id_mut(item: &mut CodexThreadItem) -> &mut String {
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

pub(super) fn qualify_codex_item(
    turn_scope_id: &str,
    mut item: CodexThreadItem,
) -> CodexThreadItem {
    let id = codex_item_id_mut(&mut item);
    if id.starts_with(turn_scope_id) {
        return item;
    }

    let qualified_id = {
        let raw_id = id.as_str();
        format!("{turn_scope_id}/{raw_id}")
    };
    *id = qualified_id;
    item
}

pub(super) fn qualify_event(turn_scope_id: &str, event: CodexThreadEvent) -> CodexThreadEvent {
    match event {
        CodexThreadEvent::ItemStarted { item } => CodexThreadEvent::ItemStarted {
            item: qualify_codex_item(turn_scope_id, item),
        },
        CodexThreadEvent::ItemUpdated { item } => CodexThreadEvent::ItemUpdated {
            item: qualify_codex_item(turn_scope_id, item),
        },
        CodexThreadEvent::ItemCompleted { item } => CodexThreadEvent::ItemCompleted {
            item: qualify_codex_item(turn_scope_id, item),
        },
        other => other,
    }
}
