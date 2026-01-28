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

pub(super) fn qualify_codex_item(turn_scope_id: &str, item: CodexThreadItem) -> CodexThreadItem {
    let raw_id = codex_item_id(&item);
    if raw_id.starts_with(turn_scope_id) {
        return item;
    }

    let qualified_id = format!("{turn_scope_id}/{raw_id}");
    match item {
        CodexThreadItem::AgentMessage { id: _, text } => CodexThreadItem::AgentMessage {
            id: qualified_id,
            text,
        },
        CodexThreadItem::Reasoning { id: _, text } => CodexThreadItem::Reasoning {
            id: qualified_id,
            text,
        },
        CodexThreadItem::CommandExecution {
            id: _,
            command,
            aggregated_output,
            exit_code,
            status,
        } => CodexThreadItem::CommandExecution {
            id: qualified_id,
            command,
            aggregated_output,
            exit_code,
            status,
        },
        CodexThreadItem::FileChange {
            id: _,
            changes,
            status,
        } => CodexThreadItem::FileChange {
            id: qualified_id,
            changes,
            status,
        },
        CodexThreadItem::McpToolCall {
            id: _,
            server,
            tool,
            arguments,
            result,
            error,
            status,
        } => CodexThreadItem::McpToolCall {
            id: qualified_id,
            server,
            tool,
            arguments,
            result,
            error,
            status,
        },
        CodexThreadItem::WebSearch { id: _, query } => CodexThreadItem::WebSearch {
            id: qualified_id,
            query,
        },
        CodexThreadItem::TodoList { id: _, items } => CodexThreadItem::TodoList {
            id: qualified_id,
            items,
        },
        CodexThreadItem::Error { id: _, message } => CodexThreadItem::Error {
            id: qualified_id,
            message,
        },
    }
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
