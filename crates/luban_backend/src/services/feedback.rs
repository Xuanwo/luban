use super::GitWorkspaceService;
use super::gh_cli::{ensure_gh_cli, run_gh_json};
use super::github_url::extract_first_github_url;
use anyhow::{Context as _, anyhow};
use luban_domain::{ProjectWorkspaceService, TaskIntentKind, TaskIssueInfo};
use rand::{Rng as _, rngs::OsRng};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

const FEEDBACK_REPO: &str = "xuanwo/luban";

fn write_temp_file(prefix: &str, suffix: &str, contents: &str) -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir();
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let rand: u64 = OsRng.r#gen();
    let path = dir.join(format!("{prefix}-{micros:x}-{rand:x}.{suffix}"));
    std::fs::write(&path, contents.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

#[derive(Deserialize)]
struct GhDefaultBranchRef {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct GhRepoView {
    url: String,
    #[serde(rename = "defaultBranchRef")]
    default_branch_ref: Option<GhDefaultBranchRef>,
}

#[derive(Clone, Debug)]
struct RepoInfo {
    url: String,
    default_branch: Option<String>,
}

#[derive(Deserialize)]
struct GhIssueView {
    number: u64,
    title: String,
    url: String,
}

pub(super) fn feedback_create_issue(
    title: String,
    body: String,
    labels: Vec<String>,
) -> anyhow::Result<TaskIssueInfo> {
    ensure_gh_cli()?;

    let title = title.trim().to_owned();
    if title.is_empty() {
        return Err(anyhow!("issue title is empty"));
    }

    let body_file = write_temp_file("luban-feedback-issue", "md", body.trim_end())?;

    let try_create = |labels2: &[String]| -> anyhow::Result<String> {
        let mut cmd = Command::new("gh");
        cmd.arg("issue")
            .arg("create")
            .arg("-R")
            .arg(FEEDBACK_REPO)
            .arg("--title")
            .arg(&title)
            .arg("--body-file")
            .arg(&body_file);
        for label in labels2 {
            let trimmed = label.trim();
            if trimmed.is_empty() {
                continue;
            }
            cmd.arg("--label").arg(trimmed);
        }

        let out = cmd.output().context("failed to spawn gh issue create")?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            if stderr.is_empty() {
                return Err(anyhow!(
                    "gh issue create failed with status: {}",
                    out.status
                ));
            }
            return Err(anyhow!("{stderr}"));
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        let url = extract_first_github_url(&stdout)
            .ok_or_else(|| anyhow!("gh issue create returned no issue url"))?;
        Ok(url)
    };

    let created_url = match try_create(&labels) {
        Ok(url) => url,
        Err(err) => try_create(&[]).with_context(|| format!("failed to create issue: {err:#}"))?,
    };

    let _ = std::fs::remove_file(&body_file);

    let issue = run_gh_json::<GhIssueView>(&[
        "issue",
        "view",
        &created_url,
        "-R",
        FEEDBACK_REPO,
        "--json",
        "number,title,url",
    ])?;

    Ok(TaskIssueInfo {
        number: issue.number,
        title: issue.title,
        url: issue.url,
    })
}

pub(super) fn feedback_task_prompt(
    service: &GitWorkspaceService,
    issue: TaskIssueInfo,
    intent_kind: TaskIntentKind,
) -> anyhow::Result<String> {
    ensure_gh_cli()?;

    let repo = run_gh_json::<GhRepoView>(&[
        "repo",
        "view",
        FEEDBACK_REPO,
        "--json",
        "nameWithOwner,url,defaultBranchRef",
    ])
    .ok()
    .map(|view| RepoInfo {
        url: view.url,
        default_branch: view
            .default_branch_ref
            .and_then(|r| r.name)
            .filter(|s| !s.trim().is_empty()),
    });

    feedback_task_prompt_with_repo(service, issue, intent_kind, repo)
}

fn feedback_task_prompt_with_repo(
    service: &GitWorkspaceService,
    issue: TaskIssueInfo,
    intent_kind: TaskIntentKind,
    repo: Option<RepoInfo>,
) -> anyhow::Result<String> {
    let known_context = render_feedback_known_context(&repo, &issue);

    let template = service
        .task_prompt_templates_load()
        .ok()
        .and_then(|templates| templates.get(&intent_kind).cloned())
        .filter(|template| !template.trim().is_empty())
        .unwrap_or_else(|| luban_domain::default_task_prompt_template(intent_kind));

    let input = issue.url.clone();
    Ok(super::task::render_task_prompt_template(
        &template,
        &input,
        intent_kind.label(),
        &known_context,
    ))
}

fn render_feedback_known_context(repo: &Option<RepoInfo>, issue: &TaskIssueInfo) -> String {
    let mut out = String::new();
    out.push_str("Known context:\n");

    if let Some(r) = repo {
        out.push_str(&format!("- Repo URL: {}\n", r.url));
        if let Some(branch) = &r.default_branch {
            out.push_str(&format!("- Default branch: {branch}\n"));
        }
    }
    out.push_str(&format!("- Issue: #{} {}\n", issue.number, issue.url));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_task_prompt_includes_issue_url() {
        let issue = TaskIssueInfo {
            number: 123,
            title: "Example".to_owned(),
            url: "https://github.com/xuanwo/luban/issues/123".to_owned(),
        };
        let service =
            GitWorkspaceService::new_with_options(crate::sqlite_store::SqliteStoreOptions {
                persist_ui_state: false,
            })
            .unwrap();
        let prompt =
            feedback_task_prompt_with_repo(&service, issue.clone(), TaskIntentKind::Fix, None)
                .unwrap();
        assert!(prompt.contains(&issue.url));
    }
}
