use super::gh_cli::{ensure_gh_cli, run_gh_json};
use crate::env::home_dir;
use crate::services::GitWorkspaceService;
use anyhow::{Context as _, anyhow};
use luban_domain::ProjectWorkspaceService;
use luban_domain::{
    SystemTaskKind, THREAD_TITLE_MAX_CHARS, TaskDraft, TaskIntentKind, TaskIssueInfo,
    TaskProjectSpec, TaskPullRequestInfo, TaskRepoInfo, default_system_prompt_template,
    derive_thread_title,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParsedTaskInput {
    Unspecified,
    LocalPath(PathBuf),
    GitHubRepo { full_name: String },
    GitHubIssue { full_name: String, number: u64 },
    GitHubPullRequest { full_name: String, number: u64 },
}

fn extract_first_github_url(input: &str) -> Option<String> {
    let needle = "https://github.com/";
    let start = input.find(needle)?;
    let rest = &input[start..];
    let end = rest
        .find(|c: char| {
            c.is_whitespace() || c == '"' || c == '\'' || c == ')' || c == ']' || c == '>'
        })
        .unwrap_or(rest.len());
    let url = rest[..end].trim_end_matches('/').to_owned();
    Some(url)
}

fn parse_github_url(url: &str) -> Option<ParsedTaskInput> {
    let url = url.trim_end_matches('/');
    let prefix = "https://github.com/";
    if !url.starts_with(prefix) {
        return None;
    }
    let path = &url[prefix.len()..];
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return None;
    }
    let full_name = format!("{}/{}", parts[0], parts[1]);
    if parts.len() == 2 {
        return Some(ParsedTaskInput::GitHubRepo { full_name });
    }
    if parts.len() >= 4 && parts[2] == "issues" {
        let number = parts[3].parse::<u64>().ok()?;
        return Some(ParsedTaskInput::GitHubIssue { full_name, number });
    }
    if parts.len() >= 4 && parts[2] == "pull" {
        let number = parts[3].parse::<u64>().ok()?;
        return Some(ParsedTaskInput::GitHubPullRequest { full_name, number });
    }
    Some(ParsedTaskInput::GitHubRepo { full_name })
}

fn looks_like_local_path(token: &str) -> bool {
    let t = token.trim();
    if t.starts_with("~/") || t.starts_with('/') || t.starts_with("./") || t.starts_with("../") {
        return true;
    }
    if t.len() >= 3 {
        let bytes = t.as_bytes();
        if bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/')
            && bytes[0].is_ascii_alphabetic()
        {
            return true;
        }
    }
    false
}

fn expand_tilde(path: &str) -> anyhow::Result<PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest));
    }
    Ok(PathBuf::from(path))
}

fn parse_task_input(input: &str) -> anyhow::Result<ParsedTaskInput> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(ParsedTaskInput::Unspecified);
    }

    if let Some(url) = extract_first_github_url(trimmed) {
        return Ok(parse_github_url(&url).unwrap_or(ParsedTaskInput::Unspecified));
    }

    let tokens = trimmed.split_whitespace().map(|s| {
        s.trim_matches(|c: char| {
            c == '"' || c == '\'' || c == '(' || c == ')' || c == ',' || c == ';'
        })
    });

    for token in tokens.clone() {
        if token.is_empty() {
            continue;
        }
        if looks_like_local_path(token) {
            let path = expand_tilde(token)?;
            return Ok(ParsedTaskInput::LocalPath(path));
        }
    }

    for token in tokens {
        if token.is_empty() {
            continue;
        }
        let parts: Vec<&str> = token.trim_end_matches('/').split('/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Ok(ParsedTaskInput::GitHubRepo {
                full_name: token.trim_end_matches('/').to_owned(),
            });
        }
    }

    Ok(ParsedTaskInput::Unspecified)
}

#[derive(Deserialize)]
struct GhDefaultBranchRef {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct GhRepoView {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
    url: String,
    #[serde(rename = "defaultBranchRef")]
    default_branch_ref: Option<GhDefaultBranchRef>,
}

#[derive(Deserialize)]
struct GhIssueView {
    title: String,
    url: String,
}

#[derive(Deserialize)]
struct GhPrView {
    title: String,
    url: String,
    #[serde(rename = "headRefName")]
    head_ref_name: Option<String>,
    #[serde(rename = "baseRefName")]
    base_ref_name: Option<String>,
    mergeable: Option<String>,
}

#[derive(Deserialize)]
struct TaskIntentModelOutput {
    #[serde(default)]
    intent_kind: Option<String>,
}

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

fn intent_kind_label(kind: TaskIntentKind) -> &'static str {
    kind.label()
}

fn project_label(spec: &TaskProjectSpec) -> String {
    match spec {
        TaskProjectSpec::Unspecified => "Unspecified".to_owned(),
        TaskProjectSpec::LocalPath { path } => format!("Local path: {}", path.display()),
        TaskProjectSpec::GitHubRepo { full_name } => format!("GitHub repo: {full_name}"),
    }
}

pub(super) fn compose_task_summary(
    intent_kind: TaskIntentKind,
    project: &TaskProjectSpec,
    repo: &Option<TaskRepoInfo>,
    issue: &Option<TaskIssueInfo>,
    pull_request: &Option<TaskPullRequestInfo>,
    notes: &[String],
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Intent: {}", intent_kind_label(intent_kind)));
    lines.push(format!("Project: {}", project_label(project)));

    if let Some(i) = issue {
        lines.push(format!("Issue: #{} {}", i.number, i.title));
    } else if let Some(pr) = pull_request {
        lines.push(format!("PR: #{} {}", pr.number, pr.title));
    } else if let Some(r) = repo {
        lines.push(format!("Repo: {}", r.full_name));
    } else {
        lines.push("Context: None".to_owned());
    }

    if !notes.is_empty() {
        lines.push(format!("Notes: {}", notes.join("; ")));
    }

    lines.join("\n")
}

pub(super) fn render_known_context(
    project: &TaskProjectSpec,
    repo: &Option<TaskRepoInfo>,
    issue: &Option<TaskIssueInfo>,
    pull_request: &Option<TaskPullRequestInfo>,
) -> String {
    let mut out = String::new();
    out.push_str("Known context:\n");
    match project {
        TaskProjectSpec::Unspecified => out.push_str("- Project: Unspecified\n"),
        TaskProjectSpec::LocalPath { path } => {
            out.push_str(&format!("- Project: Local path {}\n", path.display()));
        }
        TaskProjectSpec::GitHubRepo { full_name } => {
            out.push_str(&format!("- Project: GitHub repo {full_name}\n"));
        }
    }

    if let Some(r) = repo {
        out.push_str(&format!("- Repo URL: {}\n", r.url));
        if let Some(branch) = &r.default_branch {
            out.push_str(&format!("- Default branch: {branch}\n"));
        }
    }
    if let Some(i) = issue {
        out.push_str(&format!("- Issue: #{} {}\n", i.number, i.url));
    }
    if let Some(pr) = pull_request {
        out.push_str(&format!("- PR: #{} {}\n", pr.number, pr.url));
        if let Some(head) = &pr.head_ref {
            out.push_str(&format!("- PR head: {head}\n"));
        }
        if let Some(base) = &pr.base_ref {
            out.push_str(&format!("- PR base: {base}\n"));
        }
    }
    out
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

#[cfg(test)]
fn compose_agent_prompt(
    input: &str,
    intent_kind: TaskIntentKind,
    project: &TaskProjectSpec,
    repo: &Option<TaskRepoInfo>,
    issue: &Option<TaskIssueInfo>,
    pull_request: &Option<TaskPullRequestInfo>,
) -> String {
    let template = luban_domain::default_task_prompt_template(intent_kind);
    let known_context = render_known_context(project, repo, issue, pull_request);
    render_task_prompt_template(
        &template,
        input,
        intent_kind_label(intent_kind),
        &known_context,
    )
}

fn extract_first_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&raw[start..=end])
}

fn parse_intent_kind(raw: &str) -> TaskIntentKind {
    match raw.trim().to_ascii_lowercase().as_str() {
        "fix" | "fix_issue" => TaskIntentKind::Fix,
        "implement" | "implement_feature" => TaskIntentKind::Implement,
        "review" | "review_pull_request" => TaskIntentKind::Review,
        "discuss" => TaskIntentKind::Discuss,
        _ => TaskIntentKind::Other,
    }
}

fn parse_local_repo_root(path: &Path) -> anyhow::Result<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .context("failed to run git rev-parse")?;
    if !out.status.success() {
        return Err(anyhow!("path is not a git repository: {}", path.display()));
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let root = raw.trim();
    if root.is_empty() {
        return Err(anyhow!("git rev-parse returned empty path"));
    }
    Ok(PathBuf::from(root))
}

pub(super) fn task_preview(
    service: &GitWorkspaceService,
    input: String,
) -> anyhow::Result<TaskDraft> {
    let parsed = parse_task_input(&input)?;
    if matches!(
        parsed,
        ParsedTaskInput::GitHubRepo { .. }
            | ParsedTaskInput::GitHubIssue { .. }
            | ParsedTaskInput::GitHubPullRequest { .. }
    ) {
        ensure_gh_cli()?;
    }

    let mut repo: Option<TaskRepoInfo> = None;
    let mut issue: Option<TaskIssueInfo> = None;
    let mut pull_request: Option<TaskPullRequestInfo> = None;
    let mut notes: Vec<String> = Vec::new();

    let project = match &parsed {
        ParsedTaskInput::Unspecified => TaskProjectSpec::Unspecified,
        ParsedTaskInput::LocalPath(path) => TaskProjectSpec::LocalPath { path: path.clone() },
        ParsedTaskInput::GitHubRepo { full_name } => TaskProjectSpec::GitHubRepo {
            full_name: full_name.clone(),
        },
        ParsedTaskInput::GitHubIssue { full_name, .. } => TaskProjectSpec::GitHubRepo {
            full_name: full_name.clone(),
        },
        ParsedTaskInput::GitHubPullRequest { full_name, .. } => TaskProjectSpec::GitHubRepo {
            full_name: full_name.clone(),
        },
    };

    let context_json = match &parsed {
        ParsedTaskInput::Unspecified => {
            serde_json::json!({
                "kind": "unspecified",
                "input": input.trim(),
            })
        }
        ParsedTaskInput::LocalPath(path) => {
            serde_json::json!({
                "kind": "local_path",
                "path": path.to_string_lossy(),
            })
        }
        ParsedTaskInput::GitHubRepo { full_name } => {
            let view = run_gh_json::<GhRepoView>(&[
                "repo",
                "view",
                full_name,
                "--json",
                "nameWithOwner,url,defaultBranchRef",
            ]);
            if let Ok(view) = view {
                repo = Some(TaskRepoInfo {
                    full_name: view.name_with_owner.clone(),
                    url: view.url.clone(),
                    default_branch: view
                        .default_branch_ref
                        .and_then(|r| r.name)
                        .filter(|s| !s.trim().is_empty()),
                });
            } else if let Err(err) = view {
                notes.push(format!("Failed to retrieve repo metadata via gh: {err}"));
            }
            serde_json::json!({
                "kind": "repo",
                "full_name": full_name,
                "repo": repo.as_ref().map(|r| {
                    serde_json::json!({
                        "full_name": r.full_name,
                        "url": r.url,
                        "default_branch": r.default_branch,
                    })
                }),
            })
        }
        ParsedTaskInput::GitHubIssue { full_name, number } => {
            let repo_view = run_gh_json::<GhRepoView>(&[
                "repo",
                "view",
                full_name,
                "--json",
                "nameWithOwner,url,defaultBranchRef",
            ]);
            if let Ok(repo_view) = repo_view {
                repo = Some(TaskRepoInfo {
                    full_name: repo_view.name_with_owner.clone(),
                    url: repo_view.url.clone(),
                    default_branch: repo_view
                        .default_branch_ref
                        .and_then(|r| r.name)
                        .filter(|s| !s.trim().is_empty()),
                });
            } else if let Err(err) = repo_view {
                notes.push(format!("Failed to retrieve repo metadata via gh: {err}"));
            }

            let issue_view = run_gh_json::<GhIssueView>(&[
                "issue",
                "view",
                &number.to_string(),
                "-R",
                full_name,
                "--json",
                "title,url",
            ]);
            if let Ok(issue_view) = issue_view {
                issue = Some(TaskIssueInfo {
                    number: *number,
                    title: issue_view.title,
                    url: issue_view.url,
                });
            } else if let Err(err) = issue_view {
                notes.push(format!("Failed to retrieve issue metadata via gh: {err}"));
            }
            serde_json::json!({
                "kind": "issue",
                "full_name": full_name,
                "number": number,
                "repo": repo.as_ref().map(|r| {
                    serde_json::json!({
                        "full_name": r.full_name,
                        "url": r.url,
                        "default_branch": r.default_branch,
                    })
                }),
                "issue": issue.as_ref().map(|i| {
                    serde_json::json!({
                        "number": i.number,
                        "title": i.title,
                        "url": i.url,
                    })
                }),
            })
        }
        ParsedTaskInput::GitHubPullRequest { full_name, number } => {
            let repo_view = run_gh_json::<GhRepoView>(&[
                "repo",
                "view",
                full_name,
                "--json",
                "nameWithOwner,url,defaultBranchRef",
            ]);
            if let Ok(repo_view) = repo_view {
                repo = Some(TaskRepoInfo {
                    full_name: repo_view.name_with_owner.clone(),
                    url: repo_view.url.clone(),
                    default_branch: repo_view
                        .default_branch_ref
                        .and_then(|r| r.name)
                        .filter(|s| !s.trim().is_empty()),
                });
            } else if let Err(err) = repo_view {
                notes.push(format!("Failed to retrieve repo metadata via gh: {err}"));
            }

            let pr_view = run_gh_json::<GhPrView>(&[
                "pr",
                "view",
                &number.to_string(),
                "-R",
                full_name,
                "--json",
                "title,url,headRefName,baseRefName,mergeable",
            ]);
            if let Ok(pr_view) = pr_view {
                pull_request = Some(TaskPullRequestInfo {
                    number: *number,
                    title: pr_view.title,
                    url: pr_view.url,
                    head_ref: pr_view.head_ref_name,
                    base_ref: pr_view.base_ref_name,
                    mergeable: pr_view.mergeable,
                });
            } else if let Err(err) = pr_view {
                notes.push(format!(
                    "Failed to retrieve pull request metadata via gh: {err}"
                ));
            }
            serde_json::json!({
                "kind": "pull_request",
                "full_name": full_name,
                "number": number,
                "repo": repo.as_ref().map(|r| {
                    serde_json::json!({
                        "full_name": r.full_name,
                        "url": r.url,
                        "default_branch": r.default_branch,
                    })
                }),
                "pull_request": pull_request.as_ref().map(|pr| {
                    serde_json::json!({
                        "number": pr.number,
                        "title": pr.title,
                        "url": pr.url,
                        "head_ref": pr.head_ref,
                        "base_ref": pr.base_ref,
                        "mergeable": pr.mergeable,
                    })
                }),
            })
        }
    };

    let prompt = system_prompt_for_task(
        service,
        SystemTaskKind::InferType,
        &input,
        &context_json.to_string(),
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

    let output = extract_first_json_object(raw.trim())
        .and_then(|json| serde_json::from_str::<TaskIntentModelOutput>(json).ok())
        .unwrap_or(TaskIntentModelOutput { intent_kind: None });

    let intent_kind = parse_intent_kind(output.intent_kind.as_deref().unwrap_or("other"));
    let summary = compose_task_summary(intent_kind, &project, &repo, &issue, &pull_request, &notes);
    let known_context = render_known_context(&project, &repo, &issue, &pull_request);
    let template = service
        .task_prompt_templates_load()
        .ok()
        .and_then(|templates| templates.get(&intent_kind).cloned())
        .filter(|template| !template.trim().is_empty())
        .unwrap_or_else(|| luban_domain::default_task_prompt_template(intent_kind));
    let prompt = render_task_prompt_template(
        &template,
        &input,
        intent_kind_label(intent_kind),
        &known_context,
    );

    Ok(TaskDraft {
        input,
        project,
        intent_kind,
        summary,
        prompt,
        repo,
        issue,
        pull_request,
    })
}

#[allow(dead_code)]
fn context_json_for_task_draft(draft: &TaskDraft) -> serde_json::Value {
    match &draft.project {
        TaskProjectSpec::Unspecified => serde_json::json!({
            "kind": "unspecified",
            "input": draft.input.trim(),
        }),
        TaskProjectSpec::LocalPath { path } => serde_json::json!({
            "kind": "local_path",
            "path": path.display().to_string(),
        }),
        TaskProjectSpec::GitHubRepo { full_name } => {
            if let Some(issue) = &draft.issue {
                serde_json::json!({
                    "kind": "issue",
                    "full_name": full_name,
                    "number": issue.number,
                    "repo": draft.repo.as_ref().map(|r| {
                        serde_json::json!({
                            "full_name": r.full_name,
                            "url": r.url,
                            "default_branch": r.default_branch,
                        })
                    }),
                    "issue": serde_json::json!({
                        "number": issue.number,
                        "title": issue.title,
                        "url": issue.url,
                    }),
                })
            } else if let Some(pr) = &draft.pull_request {
                serde_json::json!({
                    "kind": "pull_request",
                    "full_name": full_name,
                    "number": pr.number,
                    "repo": draft.repo.as_ref().map(|r| {
                        serde_json::json!({
                            "full_name": r.full_name,
                            "url": r.url,
                            "default_branch": r.default_branch,
                        })
                    }),
                    "pull_request": serde_json::json!({
                        "number": pr.number,
                        "title": pr.title,
                        "url": pr.url,
                        "head_ref": pr.head_ref,
                        "base_ref": pr.base_ref,
                        "mergeable": pr.mergeable,
                    }),
                })
            } else {
                serde_json::json!({
                    "kind": "repo",
                    "full_name": full_name,
                    "repo": draft.repo.as_ref().map(|r| {
                        serde_json::json!({
                            "full_name": r.full_name,
                            "url": r.url,
                            "default_branch": r.default_branch,
                        })
                    }),
                })
            }
        }
    }
}

fn branch_rename_context_for_task_draft(draft: &TaskDraft) -> String {
    let mut sections = Vec::new();

    let summary = draft.summary.trim();
    if !summary.is_empty() {
        sections.push(format!("Task summary:\n{summary}"));
    }

    let known_context = render_known_context(
        &draft.project,
        &draft.repo,
        &draft.issue,
        &draft.pull_request,
    );
    let known_context = known_context.trim();
    if !known_context.is_empty() {
        sections.push(known_context.to_owned());
    }

    sections.join("\n\n")
}

pub(super) fn task_suggest_branch_name(
    service: &GitWorkspaceService,
    draft: TaskDraft,
) -> anyhow::Result<String> {
    let context = branch_rename_context_for_task_draft(&draft);
    let prompt = system_prompt_for_task(
        service,
        SystemTaskKind::RenameBranch,
        &draft.input,
        &context,
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

pub(super) fn task_prepare_project(
    service: &GitWorkspaceService,
    spec: TaskProjectSpec,
) -> anyhow::Result<PathBuf> {
    ensure_gh_cli()?;

    match spec {
        TaskProjectSpec::Unspecified => Err(anyhow!(
            "project is unspecified: provide a local path or a GitHub repo"
        )),
        TaskProjectSpec::LocalPath { path } => {
            if !path.exists() {
                return Err(anyhow!("path does not exist: {}", path.display()));
            }
            parse_local_repo_root(&path)
        }
        TaskProjectSpec::GitHubRepo { full_name } => {
            let mut it = full_name.split('/');
            let owner = it
                .next()
                .ok_or_else(|| anyhow!("invalid repo: {full_name}"))?;
            let name = it
                .next()
                .ok_or_else(|| anyhow!("invalid repo: {full_name}"))?;
            if it.next().is_some() {
                return Err(anyhow!("invalid repo: {full_name}"));
            }

            let luban_root = service
                .worktrees_root
                .parent()
                .ok_or_else(|| {
                    anyhow!(
                        "invalid worktrees_root: {}",
                        service.worktrees_root.display()
                    )
                })?
                .to_path_buf();
            let projects_root = luban_domain::paths::projects_root(&luban_root);
            let dest = projects_root.join(owner).join(name);

            if dest.exists() {
                return Ok(dest);
            }

            std::fs::create_dir_all(dest.parent().unwrap_or(&projects_root))
                .with_context(|| format!("failed to create {}", projects_root.display()))?;

            let status = Command::new("gh")
                .args(["repo", "clone", &full_name])
                .arg(&dest)
                .status()
                .map_err(|err| {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        anyhow!("missing gh executable: install GitHub CLI (gh) and ensure it is available on PATH")
                    } else {
                        anyhow!(err).context("failed to spawn gh")
                    }
                })?;

            if !status.success() {
                return Err(anyhow!("gh repo clone failed with status: {status}"));
            }

            Ok(dest)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_global_constraints(prompt: &str) {
        assert!(
            prompt.contains("Do NOT commit")
                && prompt.contains("Do NOT commit, push, open a pull request"),
            "prompt must include global no-commit/no-pr constraint"
        );
        assert!(
            !prompt.contains("provide a patch") && !prompt.contains("patch/diff"),
            "prompt must not require patch/diff output for code agents"
        );
        assert!(
            prompt.contains("target repository's own practices")
                || prompt.contains("target repository's conventions"),
            "prompt must instruct the agent to follow the target repository's practices"
        );
        assert!(
            !prompt.contains("`just") && !prompt.contains("\njust "),
            "prompt must not include luban-specific just commands"
        );
        assert!(
            !prompt.contains("pnpm")
                && !prompt.contains("npm")
                && !prompt.contains("yarn")
                && !prompt.contains("cargo"),
            "prompt must not hardcode specific build/test tools"
        );
    }

    #[test]
    fn parse_github_repo_url() {
        let parsed = parse_task_input("https://github.com/openai/openai-cookbook").unwrap();
        assert_eq!(
            parsed,
            ParsedTaskInput::GitHubRepo {
                full_name: "openai/openai-cookbook".to_owned()
            }
        );
    }

    #[test]
    fn parse_github_issue_url() {
        let parsed =
            parse_task_input("https://github.com/openai/openai-cookbook/issues/123").unwrap();
        assert_eq!(
            parsed,
            ParsedTaskInput::GitHubIssue {
                full_name: "openai/openai-cookbook".to_owned(),
                number: 123
            }
        );
    }

    #[test]
    fn parse_github_pr_url() {
        let parsed =
            parse_task_input("https://github.com/openai/openai-cookbook/pull/456").unwrap();
        assert_eq!(
            parsed,
            ParsedTaskInput::GitHubPullRequest {
                full_name: "openai/openai-cookbook".to_owned(),
                number: 456
            }
        );
    }

    #[test]
    fn parse_owner_repo_token() {
        let parsed = parse_task_input("openai/openai-cookbook").unwrap();
        assert_eq!(
            parsed,
            ParsedTaskInput::GitHubRepo {
                full_name: "openai/openai-cookbook".to_owned()
            }
        );
    }

    #[test]
    fn parse_local_path_token() {
        let parsed = parse_task_input("~/repo").unwrap();
        match parsed {
            ParsedTaskInput::LocalPath(path) => {
                assert!(path.to_string_lossy().contains("repo"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn extract_branch_candidate_prefers_luban_prefix_tokens() {
        assert_eq!(
            extract_branch_candidate("luban/hello-world\n"),
            Some("luban/hello-world".to_owned())
        );
        assert_eq!(
            extract_branch_candidate("branch: luban/foo-bar"),
            Some("luban/foo-bar".to_owned())
        );
        assert_eq!(
            extract_branch_candidate("refs/heads/luban/foo"),
            Some("luban/foo".to_owned())
        );
    }

    #[test]
    fn normalize_branch_name_accepts_plain_suffixes() {
        assert_eq!(normalize_branch_name("misc").as_deref(), Some("luban/misc"));
        assert_eq!(
            normalize_branch_name("luban/Feature X").as_deref(),
            Some("luban/feature-x")
        );
    }

    #[test]
    fn prompts_are_intent_specific_and_no_commit_by_default() {
        let input = "example task";
        let project = TaskProjectSpec::Unspecified;

        let fix = compose_agent_prompt(input, TaskIntentKind::Fix, &project, &None, &None, &None);
        assert!(fix.contains("Goal: identify the root cause"), "{fix}");
        assert!(fix.contains("Operating mode:"), "{fix}");
        assert_global_constraints(&fix);

        let feature = compose_agent_prompt(
            input,
            TaskIntentKind::Implement,
            &project,
            &None,
            &None,
            &None,
        );
        assert!(
            feature.contains("Goal: implement the requested feature"),
            "{feature}"
        );
        assert_global_constraints(&feature);

        let review =
            compose_agent_prompt(input, TaskIntentKind::Review, &project, &None, &None, &None);
        assert!(
            review.contains("produce a high-quality code review"),
            "{review}"
        );
        assert!(review.contains("Do NOT implement changes"), "{review}");
        assert!(
            review.contains("gh pr checkout"),
            "review prompt should mention gh pr checkout for GitHub PR review workflows: {review}"
        );
        assert_global_constraints(&review);

        let discuss = compose_agent_prompt(
            input,
            TaskIntentKind::Discuss,
            &project,
            &None,
            &None,
            &None,
        );
        assert!(
            discuss.contains("explore a question")
                || discuss.contains("converge on a concrete next step")
                || discuss.contains("Goal: explore"),
            "{discuss}"
        );
        assert_global_constraints(&discuss);

        let other =
            compose_agent_prompt(input, TaskIntentKind::Other, &project, &None, &None, &None);
        assert!(other.contains("move it forward"), "{other}");
        assert_global_constraints(&other);
    }

    #[test]
    fn rename_branch_system_prompt_includes_context_placeholder() {
        let template = luban_domain::default_system_prompt_template(SystemTaskKind::RenameBranch);
        assert!(
            template.contains("{{context_json}}"),
            "rename branch system prompt must include context placeholder"
        );
    }
}
