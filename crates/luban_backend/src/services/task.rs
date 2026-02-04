use crate::services::GitWorkspaceService;
use anyhow::anyhow;
use luban_domain::{
    AgentRunnerKind, ProjectWorkspaceService, SystemTaskKind, THREAD_TITLE_MAX_CHARS,
    TaskIntentKind, TaskStatus, ThinkingEffort, default_system_prompt_template,
    derive_thread_title, parse_task_status,
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

fn run_system_task_and_collect_messages(
    service: &GitWorkspaceService,
    runner: AgentRunnerKind,
    model_id: String,
    thinking_effort: ThinkingEffort,
    amp_mode: Option<String>,
    prompt: String,
) -> anyhow::Result<Vec<String>> {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut agent_messages: Vec<String> = Vec::new();

    let worktree_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match runner {
        AgentRunnerKind::Codex => {
            let model = model_id.trim();
            service.run_codex_turn_streamed_via_cli(
                super::CodexTurnParams {
                    thread_id: None,
                    worktree_path,
                    prompt,
                    image_paths: Vec::new(),
                    model: if model.is_empty() {
                        None
                    } else {
                        Some(model.to_owned())
                    },
                    model_reasoning_effort: Some(thinking_effort.as_str().to_owned()),
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
        }
        AgentRunnerKind::Amp => {
            service.run_amp_turn_streamed_via_cli(
                super::AmpTurnParams {
                    thread_id: None,
                    worktree_path,
                    prompt,
                    mode: amp_mode,
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
        }
        AgentRunnerKind::Claude => {
            service.run_claude_turn_streamed_via_cli(
                super::ClaudeTurnParams {
                    thread_id: None,
                    worktree_path,
                    prompt,
                    add_dirs: Vec::new(),
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
        }
    }

    Ok(agent_messages)
}

fn run_system_task_and_find_last_message(
    service: &GitWorkspaceService,
    runner: AgentRunnerKind,
    model_id: String,
    thinking_effort: ThinkingEffort,
    amp_mode: Option<String>,
    prompt: String,
) -> anyhow::Result<String> {
    let agent_messages = run_system_task_and_collect_messages(
        service,
        runner,
        model_id,
        thinking_effort,
        amp_mode,
        prompt,
    )?;

    agent_messages
        .into_iter()
        .rev()
        .find(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("runner returned no agent_message output"))
}

pub(super) fn task_suggest_branch_name(
    service: &GitWorkspaceService,
    input: String,
    runner: AgentRunnerKind,
    model_id: String,
    thinking_effort: ThinkingEffort,
    amp_mode: Option<String>,
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

    let raw = run_system_task_and_find_last_message(
        service,
        runner,
        model_id,
        thinking_effort,
        amp_mode,
        prompt,
    )?;

    let branch_name = extract_branch_candidate(raw.trim())
        .and_then(|candidate| normalize_branch_name(&candidate))
        .unwrap_or_else(|| "luban/misc".to_owned());

    Ok(branch_name)
}

pub(super) fn task_suggest_thread_title(
    service: &GitWorkspaceService,
    input: String,
    runner: AgentRunnerKind,
    model_id: String,
    thinking_effort: ThinkingEffort,
    amp_mode: Option<String>,
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

    let raw = run_system_task_and_find_last_message(
        service,
        runner,
        model_id,
        thinking_effort,
        amp_mode,
        prompt,
    )
    .unwrap_or_else(|_| fallback.clone());

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

fn strip_json_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    let without_prefix = trimmed.strip_prefix("```json").unwrap_or(trimmed);
    let without_prefix = without_prefix.strip_prefix("```").unwrap_or(without_prefix);
    let without_suffix = without_prefix.strip_suffix("```").unwrap_or(without_prefix);
    without_suffix.trim()
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&trimmed[start..=end])
}

pub(super) fn task_suggest_task_status(
    service: &GitWorkspaceService,
    input: String,
    runner: AgentRunnerKind,
    model_id: String,
    thinking_effort: ThinkingEffort,
    amp_mode: Option<String>,
) -> anyhow::Result<TaskStatus> {
    let input_trimmed = input.trim();
    let context_json = serde_json::json!({
        "allowed_task_status": [
            "backlog",
            "todo",
            "iterating",
            "validating",
            "done",
            "canceled"
        ],
    })
    .to_string();

    let prompt = system_prompt_for_task(
        service,
        SystemTaskKind::AutoUpdateTaskStatus,
        input_trimmed,
        &context_json,
    );

    let raw = run_system_task_and_find_last_message(
        service,
        runner,
        model_id,
        thinking_effort,
        amp_mode,
        prompt,
    )?;

    parse_task_status_auto_update_output(input_trimmed, &raw)
}

#[derive(Debug, serde::Deserialize)]
struct TaskStatusAutoUpdateEvidence {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    pr_number: Option<u64>,
    #[serde(default)]
    text: String,
}

#[derive(Debug, serde::Deserialize)]
struct TaskStatusAutoUpdateOutput {
    task_status: String,
    #[serde(default)]
    evidence: Option<TaskStatusAutoUpdateEvidence>,
}

fn parse_current_task_status_from_input(input: &str) -> Option<TaskStatus> {
    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("Current task_status:") {
            return parse_task_status(rest.trim());
        }
    }
    None
}

fn parse_merged_pull_request_number_from_input(input: &str) -> Option<u64> {
    let mut merged = false;
    let mut number: Option<u64> = None;
    for line in input.lines() {
        if line
            .trim()
            .eq_ignore_ascii_case("Workspace pull request state: merged")
        {
            merged = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix("Workspace pull request number:") {
            number = rest.trim().parse::<u64>().ok();
        }
    }
    if merged { number } else { None }
}

fn parse_task_status_auto_update_output(input: &str, raw: &str) -> anyhow::Result<TaskStatus> {
    let input_trimmed = input.trim();
    let raw = strip_json_fences(raw);
    if let Some(status) = parse_task_status(raw) {
        return Ok(status);
    }

    let Some(obj) = extract_json_object(raw) else {
        return Err(anyhow!("runner returned no json output"));
    };

    let output: TaskStatusAutoUpdateOutput = serde_json::from_str(obj)?;
    let Some(mut suggested) = parse_task_status(output.task_status.as_str()) else {
        return Err(anyhow!("missing or invalid task_status in json output"));
    };

    let current = parse_current_task_status_from_input(input_trimmed);
    let pr_number = parse_merged_pull_request_number_from_input(input_trimmed);

    if let Some(pr_number) = pr_number
        && current == Some(TaskStatus::Validating)
        && suggested == TaskStatus::Done
    {
        let evidence = output.evidence.as_ref();
        let valid = evidence.is_some_and(|e| {
            e.kind == "pr_reference"
                && e.pr_number == Some(pr_number)
                && !e.text.trim().is_empty()
                && e.text.contains(&pr_number.to_string())
        });

        if !valid {
            suggested = current.unwrap_or(TaskStatus::Validating);
        }
    }

    Ok(suggested)
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

    #[test]
    fn auto_update_task_status_requires_pr_evidence_when_pr_is_merged() {
        let input = r#"
Current task_status: validating
Turn outcome: pr_merged

Recent user messages (newest first):
1.
Please validate PR #123

Workspace pull request state: merged
Workspace pull request number: 123
"#;

        let raw_missing =
            r#"{"task_status":"done","evidence":{"kind":"none","pr_number":null,"text":""}}"#;
        let status = parse_task_status_auto_update_output(input, raw_missing).unwrap();
        assert_eq!(status, TaskStatus::Validating);

        let raw_wrong = r#"{"task_status":"done","evidence":{"kind":"pr_reference","pr_number":124,"text":"PR #124 is merged"}}"#;
        let status = parse_task_status_auto_update_output(input, raw_wrong).unwrap();
        assert_eq!(status, TaskStatus::Validating);

        let raw_ok = r#"{"task_status":"done","evidence":{"kind":"pr_reference","pr_number":123,"text":"Please validate PR #123 (merged)"}}"#;
        let status = parse_task_status_auto_update_output(input, raw_ok).unwrap();
        assert_eq!(status, TaskStatus::Done);
    }
}
