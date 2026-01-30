use crate::services::GitWorkspaceService;
use anyhow::anyhow;
use luban_domain::{
    ProjectWorkspaceService, SystemTaskKind, THREAD_TITLE_MAX_CHARS, TaskIntentKind,
    default_system_prompt_template, derive_thread_title,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn render_system_prompt_template(template: &str, task_input: &str, context_json: &str) -> String {
    let mut out = template.to_owned();
    out = out.replace("{{task_input}}", task_input.trim());
    out = out.replace("{{context_json}}", context_json.trim());
    out
}

fn system_prompt_for_task(
    service: &GitWorkspaceService,
    kind: SystemTaskKind,
    task_input: &str,
    context_json: &str,
) -> String {
    let template = service
        .system_prompt_templates_load()
        .ok()
        .and_then(|templates| templates.get(&kind).cloned())
        .filter(|template| !template.trim().is_empty())
        .unwrap_or_else(|| default_system_prompt_template(kind));
    render_system_prompt_template(&template, task_input, context_json)
}

fn normalize_branch_name(raw: &str) -> Option<String> {
    let mut value = raw.trim();
    if value.is_empty() {
        return None;
    }

    if let Some(stripped) = value.strip_prefix("refs/heads/") {
        value = stripped;
    }
    if let Some(stripped) = value.strip_prefix("luban/") {
        value = stripped;
    }

    let mut out = String::new();
    let mut prev_hyphen = false;
    for ch in value.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };
        if next == '-' {
            if prev_hyphen {
                continue;
            }
            prev_hyphen = true;
            out.push('-');
            continue;
        }
        prev_hyphen = false;
        out.push(next);
    }

    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        return None;
    }

    const MAX_SUFFIX_LEN: usize = 24;
    let suffix = trimmed.chars().take(MAX_SUFFIX_LEN).collect::<String>();
    let suffix = suffix.trim_matches('-');
    if suffix.is_empty() {
        return None;
    }

    Some(format!("luban/{suffix}"))
}

fn extract_branch_candidate(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    for token in trimmed.split_whitespace() {
        let t =
            token.trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ';' || c == ')');
        if t.starts_with("luban/") {
            return Some(t.to_owned());
        }
        if let Some(rest) = t.strip_prefix("refs/heads/luban/") {
            return Some(format!("luban/{rest}"));
        }
        if let Some(rest) = t.strip_prefix("refs/heads/") {
            return Some(rest.to_owned());
        }
    }

    trimmed.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        Some(line.to_owned())
    })
}

pub(super) fn render_task_prompt_template(
    template: &str,
    task_input: &str,
    intent_label: &str,
    known_context: &str,
) -> String {
    let mut out = template.to_owned();
    out = out.replace("{{task_input}}", task_input.trim());
    out = out.replace("{{intent_label}}", intent_label);
    out = out.replace("{{known_context}}", known_context.trim_end());
    out
}

pub(super) fn task_suggest_branch_name(
    service: &GitWorkspaceService,
    input: String,
) -> anyhow::Result<String> {
    let context_json = serde_json::json!({
        "kind": "rename_branch",
    })
    .to_string();

    let prompt = system_prompt_for_task(
        service,
        SystemTaskKind::RenameBranch,
        input.trim(),
        &context_json,
    );

    let cancel = Arc::new(AtomicBool::new(false));
    let mut agent_messages: Vec<String> = Vec::new();
    service.run_codex_turn_streamed_via_cli(
        super::CodexTurnParams {
            thread_id: None,
            worktree_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            prompt,
            image_paths: Vec::new(),
            model: Some("gpt-5.1-codex-mini".to_owned()),
            model_reasoning_effort: Some("low".to_owned()),
            sandbox_mode: Some("read-only".to_owned()),
        },
        cancel,
        |event| {
            if let luban_domain::CodexThreadEvent::ItemCompleted {
                item: luban_domain::CodexThreadItem::AgentMessage { text, .. },
            } = event
            {
                agent_messages.push(text);
            }
            Ok(())
        },
    )?;

    let raw = agent_messages
        .into_iter()
        .rev()
        .find(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("codex returned no agent_message output"))?;

    let branch_name = extract_branch_candidate(raw.trim())
        .and_then(|candidate| normalize_branch_name(&candidate))
        .unwrap_or_else(|| "luban/misc".to_owned());

    Ok(branch_name)
}

pub(super) fn task_suggest_thread_title(
    service: &GitWorkspaceService,
    input: String,
) -> anyhow::Result<String> {
    let input_trimmed = input.trim();
    let fallback = derive_thread_title(input_trimmed);
    if input_trimmed.is_empty() {
        return Ok("Thread".to_owned());
    }

    let context_json = serde_json::json!({
        "max_chars": THREAD_TITLE_MAX_CHARS,
        "intent": TaskIntentKind::Other.label(),
    })
    .to_string();

    let prompt = system_prompt_for_task(
        service,
        SystemTaskKind::AutoTitleThread,
        input_trimmed,
        &context_json,
    );

    let cancel = Arc::new(AtomicBool::new(false));
    let mut agent_messages: Vec<String> = Vec::new();
    service.run_codex_turn_streamed_via_cli(
        super::CodexTurnParams {
            thread_id: None,
            worktree_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            prompt,
            image_paths: Vec::new(),
            model: Some("gpt-5.1-codex-mini".to_owned()),
            model_reasoning_effort: Some("low".to_owned()),
            sandbox_mode: Some("read-only".to_owned()),
        },
        cancel,
        |event| {
            if let luban_domain::CodexThreadEvent::ItemCompleted {
                item: luban_domain::CodexThreadItem::AgentMessage { text, .. },
            } = event
            {
                agent_messages.push(text);
            }
            Ok(())
        },
    )?;

    let raw = agent_messages
        .into_iter()
        .rev()
        .find(|s| !s.trim().is_empty())
        .unwrap_or_else(|| fallback.clone());

    let mut candidate = raw.trim().to_owned();
    if (candidate.starts_with('"') && candidate.ends_with('"') && candidate.len() >= 2)
        || (candidate.starts_with('\'') && candidate.ends_with('\'') && candidate.len() >= 2)
    {
        candidate = candidate[1..candidate.len().saturating_sub(1)]
            .trim()
            .to_owned();
    }

    let title = derive_thread_title(&candidate);
    if !title.is_empty() {
        return Ok(title);
    }
    if !fallback.is_empty() {
        return Ok(fallback);
    }
    Ok("Thread".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_branch_name_accepts_plain_suffixes() {
        assert_eq!(
            normalize_branch_name("fix star ui"),
            Some("luban/fix-star-ui".to_owned())
        );
        assert_eq!(
            normalize_branch_name("luban/fix-star-ui"),
            Some("luban/fix-star-ui".to_owned())
        );
        assert_eq!(
            normalize_branch_name("refs/heads/luban/fix-star-ui"),
            Some("luban/fix-star-ui".to_owned())
        );
    }
}
