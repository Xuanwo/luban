use anyhow::{Context as _, anyhow};
use bip39::Language;
use luban_domain::paths;
use luban_domain::{
    CodexThreadEvent, CodexThreadItem, ContextImage, ConversationEntry, ConversationSnapshot,
    CreatedWorkspace, PersistedAppState, ProjectWorkspaceService, PullRequestInfo,
    PullRequestState, RunAgentTurnRequest,
};
use rand::{Rng as _, rngs::OsRng};
use std::{
    collections::HashSet,
    path::PathBuf,
    process::Command,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::sqlite_store::SqliteStore;

mod codex_cli;
mod context_blobs;
mod conversations;
mod git;
use codex_cli::CodexTurnParams;

fn contains_attempt_fraction(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if !ch.is_ascii_digit() {
            continue;
        }

        while matches!(chars.peek(), Some(next) if next.is_ascii_digit()) {
            let _ = chars.next();
        }

        if !matches!(chars.peek(), Some('/')) {
            continue;
        }
        let _ = chars.next();

        if !matches!(chars.peek(), Some(next) if next.is_ascii_digit()) {
            continue;
        }
        return true;
    }

    false
}

fn is_transient_reconnect_notice(message: &str) -> bool {
    let message = message.trim();
    if message.is_empty() {
        return false;
    }

    let lower = message.to_ascii_lowercase();
    if !lower.contains("reconnecting") {
        return false;
    }

    contains_attempt_fraction(&lower)
}

fn codex_item_id(item: &CodexThreadItem) -> &str {
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

fn qualify_codex_item(turn_scope_id: &str, item: CodexThreadItem) -> CodexThreadItem {
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

fn qualify_event(turn_scope_id: &str, event: CodexThreadEvent) -> CodexThreadEvent {
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

fn generate_turn_scope_id() -> String {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let rand: u64 = OsRng.r#gen();
    format!("turn-{micros:x}-{rand:x}")
}

#[derive(Clone)]
pub struct GitWorkspaceService {
    worktrees_root: PathBuf,
    conversations_root: PathBuf,
    sqlite: SqliteStore,
}

impl GitWorkspaceService {
    pub fn new() -> anyhow::Result<Arc<Self>> {
        let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
        let mut luban_root = PathBuf::from(home);
        luban_root.push("luban");

        std::fs::create_dir_all(&luban_root)
            .with_context(|| format!("failed to create {}", luban_root.display()))?;

        let worktrees_root = paths::worktrees_root(&luban_root);
        let conversations_root = paths::conversations_root(&luban_root);
        let sqlite_path = paths::sqlite_path(&luban_root);
        let sqlite = SqliteStore::new(sqlite_path).context("failed to init sqlite store")?;

        Ok(Arc::new(Self {
            worktrees_root,
            conversations_root,
            sqlite,
        }))
    }

    fn generate_workspace_name(&self) -> anyhow::Result<String> {
        let words = Language::English.word_list();
        let mut rng = OsRng;
        let w1 = words[rng.gen_range(0..words.len())];
        let w2 = words[rng.gen_range(0..words.len())];
        Ok(format!("{w1}-{w2}"))
    }

    fn worktree_path(&self, project_slug: &str, workspace_name: &str) -> PathBuf {
        let mut path = self.worktrees_root.clone();
        path.push(project_slug);
        path.push(workspace_name);
        path
    }

    fn codex_executable(&self) -> PathBuf {
        std::env::var_os(paths::LUBAN_CODEX_BIN_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("codex"))
    }

    fn run_codex_turn_streamed_via_cli(
        &self,
        params: CodexTurnParams,
        cancel: Arc<AtomicBool>,
        on_event: impl FnMut(CodexThreadEvent) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let codex = self.codex_executable();
        codex_cli::run_codex_turn_streamed_via_cli(&codex, params, cancel, on_event)
    }
}

impl ProjectWorkspaceService for GitWorkspaceService {
    fn load_app_state(&self) -> Result<PersistedAppState, String> {
        self.load_app_state_internal().map_err(|e| format!("{e:#}"))
    }

    fn save_app_state(&self, snapshot: PersistedAppState) -> Result<(), String> {
        self.save_app_state_internal(snapshot)
            .map_err(|e| format!("{e:#}"))
    }

    fn create_workspace(
        &self,
        project_path: PathBuf,
        project_slug: String,
    ) -> Result<CreatedWorkspace, String> {
        let result: anyhow::Result<CreatedWorkspace> = (|| {
            let remote = self.select_remote(&project_path)?;

            self.run_git(&project_path, ["fetch", "--prune", &remote])
                .with_context(|| format!("failed to fetch remote '{remote}'"))?;

            let upstream_ref = self.resolve_default_upstream_ref(&project_path, &remote)?;

            std::fs::create_dir_all(self.worktrees_root.join(&project_slug))
                .context("failed to create worktrees root")?;

            for _ in 0..64 {
                let workspace_name = self.generate_workspace_name()?;
                let branch_name = format!("luban/{workspace_name}");
                let worktree_path = self.worktree_path(&project_slug, &workspace_name);

                if worktree_path.exists() {
                    continue;
                }

                let branch_ref = format!("refs/heads/{branch_name}");
                let branch_exists = Command::new("git")
                    .args(["show-ref", "--verify", "--quiet", &branch_ref])
                    .current_dir(&project_path)
                    .status()
                    .ok()
                    .map(|s| s.success())
                    .unwrap_or(false);

                if branch_exists {
                    continue;
                }

                self.run_git(
                    &project_path,
                    ["branch", "--track", &branch_name, &upstream_ref],
                )
                .with_context(|| format!("failed to create branch '{branch_name}'"))?;

                self.run_git(
                    &project_path,
                    [
                        "worktree",
                        "add",
                        worktree_path
                            .to_str()
                            .ok_or_else(|| anyhow!("invalid worktree path"))?,
                        &branch_name,
                    ],
                )
                .with_context(|| {
                    format!("failed to create worktree at {}", worktree_path.display())
                })?;

                return Ok(CreatedWorkspace {
                    workspace_name,
                    branch_name,
                    worktree_path,
                });
            }

            Err(anyhow!(
                "failed to generate a unique workspace name after retries"
            ))
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn open_workspace_in_ide(&self, worktree_path: PathBuf) -> Result<(), String> {
        let result: anyhow::Result<()> = (|| {
            if !worktree_path.exists() {
                return Err(anyhow!(
                    "workspace path does not exist: {}",
                    worktree_path.display()
                ));
            }

            #[cfg(target_os = "macos")]
            {
                let status = Command::new("open")
                    .args(["-a", "Zed"])
                    .arg(&worktree_path)
                    .status()
                    .context("failed to spawn 'open -a Zed'")?;
                if !status.success() {
                    return Err(anyhow!("'open -a Zed' exited with status: {status}"));
                }
                Ok(())
            }

            #[cfg(not(target_os = "macos"))]
            {
                let _ = worktree_path;
                Err(anyhow!("open in IDE is only supported on macOS for now"))
            }
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn archive_workspace(
        &self,
        project_path: PathBuf,
        worktree_path: PathBuf,
    ) -> Result<(), String> {
        let result: anyhow::Result<()> = (|| {
            let path_str = worktree_path
                .to_str()
                .ok_or_else(|| anyhow!("invalid worktree path"))?;
            self.run_git(&project_path, ["worktree", "remove", "--force", path_str])
                .with_context(|| {
                    format!("failed to remove worktree at {}", worktree_path.display())
                })?;
            Ok(())
        })();
        result.map_err(|e| format!("{e:#}"))
    }

    fn ensure_conversation(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
    ) -> Result<(), String> {
        self.ensure_conversation_internal(project_slug, workspace_name, thread_id)
            .map_err(|e| format!("{e:#}"))
    }

    fn list_conversation_threads(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> Result<Vec<luban_domain::ConversationThreadMeta>, String> {
        self.sqlite
            .list_conversation_threads(project_slug, workspace_name)
            .map_err(|e| format!("{e:#}"))
    }

    fn load_conversation(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
    ) -> Result<ConversationSnapshot, String> {
        self.load_conversation_internal(project_slug, workspace_name, thread_id)
            .map_err(|e| format!("{e:#}"))
    }

    fn store_context_image(
        &self,
        project_slug: String,
        workspace_name: String,
        image: ContextImage,
    ) -> Result<PathBuf, String> {
        let result: anyhow::Result<PathBuf> = self.store_context_bytes(
            &project_slug,
            &workspace_name,
            &image.bytes,
            &image.extension,
        );
        result.map_err(|e| format!("{e:#}"))
    }

    fn store_context_text(
        &self,
        project_slug: String,
        workspace_name: String,
        text: String,
        extension: String,
    ) -> Result<PathBuf, String> {
        let result: anyhow::Result<PathBuf> =
            self.store_context_bytes(&project_slug, &workspace_name, text.as_bytes(), &extension);
        result.map_err(|e| format!("{e:#}"))
    }

    fn store_context_file(
        &self,
        project_slug: String,
        workspace_name: String,
        source_path: PathBuf,
    ) -> Result<PathBuf, String> {
        let result: anyhow::Result<PathBuf> =
            self.store_context_file_internal(&project_slug, &workspace_name, &source_path);
        result.map_err(|e| format!("{e:#}"))
    }

    fn run_agent_turn_streamed(
        &self,
        request: RunAgentTurnRequest,
        cancel: Arc<AtomicBool>,
        on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync>,
    ) -> Result<(), String> {
        let RunAgentTurnRequest {
            project_slug,
            workspace_name,
            worktree_path,
            thread_local_id,
            thread_id,
            prompt,
            model,
            model_reasoning_effort,
        } = request;

        let turn_started_at = Instant::now();
        let turn_scope_id = generate_turn_scope_id();
        let duration_appended = Arc::new(AtomicBool::new(false));
        let mut appended_item_ids = HashSet::<String>::new();

        let result: anyhow::Result<()> = (|| {
            self.ensure_conversation_internal(
                project_slug.clone(),
                workspace_name.clone(),
                thread_local_id,
            )?;

            let mut existing_thread_id = self.sqlite.get_conversation_thread_id(
                project_slug.clone(),
                workspace_name.clone(),
                thread_local_id,
            )?;
            if thread_local_id == 1
                && existing_thread_id.is_none()
                && let Some(legacy) =
                    self.load_conversation_legacy(&project_slug, &workspace_name)?
                && let Some(legacy_thread_id) = legacy.thread_id
            {
                self.sqlite.set_conversation_thread_id(
                    project_slug.clone(),
                    workspace_name.clone(),
                    thread_local_id,
                    legacy_thread_id.clone(),
                )?;
                existing_thread_id = Some(legacy_thread_id);
            }

            self.sqlite.append_conversation_entries(
                project_slug.clone(),
                workspace_name.clone(),
                thread_local_id,
                vec![ConversationEntry::UserMessage {
                    text: prompt.clone(),
                }],
            )?;

            let resolved_thread_id = thread_id.or(existing_thread_id);
            let image_paths = luban_domain::extract_context_image_paths_in_order(&prompt);

            let mut turn_error: Option<String> = None;
            let mut transient_error_seq: u64 = 0;
            let duration_appended_for_events = duration_appended.clone();

            self.run_codex_turn_streamed_via_cli(
                CodexTurnParams {
                    thread_id: resolved_thread_id,
                    worktree_path: worktree_path.clone(),
                    prompt: prompt.clone(),
                    image_paths,
                    model: model.clone(),
                    model_reasoning_effort: model_reasoning_effort.clone(),
                },
                cancel.clone(),
                |event| {
                    let mut events_to_process = Vec::with_capacity(1);
                    match &event {
                        CodexThreadEvent::Error { message }
                            if is_transient_reconnect_notice(message) =>
                        {
                            transient_error_seq = transient_error_seq.saturating_add(1);
                            events_to_process.push(CodexThreadEvent::ItemCompleted {
                                item: CodexThreadItem::Error {
                                    id: format!("transient-error-{transient_error_seq}"),
                                    message: message.clone(),
                                },
                            });
                        }
                        _ => events_to_process.push(event),
                    }

                    for event in events_to_process {
                        let event = qualify_event(&turn_scope_id, event);
                        on_event(event.clone());

                        match &event {
                            CodexThreadEvent::ThreadStarted { thread_id } => {
                                self.sqlite.set_conversation_thread_id(
                                    project_slug.clone(),
                                    workspace_name.clone(),
                                    thread_local_id,
                                    thread_id.clone(),
                                )?;
                            }
                            CodexThreadEvent::ItemCompleted { item } => {
                                let id = codex_item_id(item).to_owned();
                                if appended_item_ids.insert(id) {
                                    self.sqlite.append_conversation_entries(
                                        project_slug.clone(),
                                        workspace_name.clone(),
                                        thread_local_id,
                                        vec![ConversationEntry::CodexItem {
                                            item: Box::new(item.clone()),
                                        }],
                                    )?;
                                }
                            }
                            CodexThreadEvent::TurnCompleted { usage } => {
                                let _ = usage;
                                if duration_appended_for_events
                                    .compare_exchange(
                                        false,
                                        true,
                                        Ordering::SeqCst,
                                        Ordering::SeqCst,
                                    )
                                    .is_ok()
                                {
                                    let duration_ms = turn_started_at.elapsed().as_millis() as u64;
                                    self.sqlite.append_conversation_entries(
                                        project_slug.clone(),
                                        workspace_name.clone(),
                                        thread_local_id,
                                        vec![ConversationEntry::TurnDuration { duration_ms }],
                                    )?;
                                    on_event(CodexThreadEvent::TurnDuration { duration_ms });
                                }
                            }
                            CodexThreadEvent::TurnFailed { error } => {
                                if turn_error.is_none() {
                                    turn_error = Some(error.message.clone());
                                }
                                self.sqlite.append_conversation_entries(
                                    project_slug.clone(),
                                    workspace_name.clone(),
                                    thread_local_id,
                                    vec![ConversationEntry::TurnError {
                                        message: error.message.clone(),
                                    }],
                                )?;
                                if duration_appended_for_events
                                    .compare_exchange(
                                        false,
                                        true,
                                        Ordering::SeqCst,
                                        Ordering::SeqCst,
                                    )
                                    .is_ok()
                                {
                                    let duration_ms = turn_started_at.elapsed().as_millis() as u64;
                                    self.sqlite.append_conversation_entries(
                                        project_slug.clone(),
                                        workspace_name.clone(),
                                        thread_local_id,
                                        vec![ConversationEntry::TurnDuration { duration_ms }],
                                    )?;
                                    on_event(CodexThreadEvent::TurnDuration { duration_ms });
                                }
                            }
                            CodexThreadEvent::Error { message } => {
                                if turn_error.is_none() {
                                    turn_error = Some(message.clone());
                                }
                                self.sqlite.append_conversation_entries(
                                    project_slug.clone(),
                                    workspace_name.clone(),
                                    thread_local_id,
                                    vec![ConversationEntry::TurnError {
                                        message: message.clone(),
                                    }],
                                )?;
                                if duration_appended_for_events
                                    .compare_exchange(
                                        false,
                                        true,
                                        Ordering::SeqCst,
                                        Ordering::SeqCst,
                                    )
                                    .is_ok()
                                {
                                    let duration_ms = turn_started_at.elapsed().as_millis() as u64;
                                    self.sqlite.append_conversation_entries(
                                        project_slug.clone(),
                                        workspace_name.clone(),
                                        thread_local_id,
                                        vec![ConversationEntry::TurnDuration { duration_ms }],
                                    )?;
                                    on_event(CodexThreadEvent::TurnDuration { duration_ms });
                                }
                            }
                            CodexThreadEvent::TurnStarted
                            | CodexThreadEvent::TurnDuration { .. }
                            | CodexThreadEvent::ItemStarted { .. }
                            | CodexThreadEvent::ItemUpdated { .. } => {}
                        }
                    }

                    Ok(())
                },
            )?;

            if cancel.load(Ordering::SeqCst) {
                if duration_appended
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    let duration_ms = turn_started_at.elapsed().as_millis() as u64;
                    self.sqlite.append_conversation_entries(
                        project_slug.clone(),
                        workspace_name.clone(),
                        thread_local_id,
                        vec![ConversationEntry::TurnDuration { duration_ms }],
                    )?;
                    on_event(CodexThreadEvent::TurnDuration { duration_ms });
                }
                self.sqlite.append_conversation_entries(
                    project_slug.clone(),
                    workspace_name.clone(),
                    thread_local_id,
                    vec![ConversationEntry::TurnCanceled],
                )?;
                return Ok(());
            }

            if let Some(message) = turn_error {
                return Err(anyhow!("{message}"));
            }

            Ok(())
        })();

        if let Err(err) = &result {
            if duration_appended
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                let duration_ms = turn_started_at.elapsed().as_millis() as u64;
                let _ = self.sqlite.append_conversation_entries(
                    project_slug.clone(),
                    workspace_name.clone(),
                    thread_local_id,
                    vec![ConversationEntry::TurnDuration { duration_ms }],
                );
                on_event(CodexThreadEvent::TurnDuration { duration_ms });
            }
            let _ = self.sqlite.append_conversation_entries(
                project_slug.clone(),
                workspace_name.clone(),
                thread_local_id,
                vec![ConversationEntry::TurnError {
                    message: format!("{err:#}"),
                }],
            );
        }

        result.map_err(|e| format!("{e:#}"))
    }

    fn gh_is_authorized(&self) -> Result<bool, String> {
        let output = Command::new("gh")
            .args(["auth", "status", "-h", "github.com"])
            .output();

        Ok(output.ok().map(|o| o.status.success()).unwrap_or(false))
    }

    fn gh_pull_request_info(
        &self,
        worktree_path: PathBuf,
    ) -> Result<Option<PullRequestInfo>, String> {
        #[derive(serde::Deserialize)]
        struct GhPullRequestView {
            number: u64,
            #[serde(default, rename = "isDraft")]
            is_draft: bool,
            #[serde(default)]
            state: String,
        }

        let output = Command::new("gh")
            .args(["pr", "view", "--json", "number,isDraft,state"])
            .current_dir(worktree_path)
            .output();

        let Ok(output) = output else {
            return Ok(None);
        };
        if !output.status.success() {
            return Ok(None);
        }

        let Ok(value) = serde_json::from_slice::<GhPullRequestView>(&output.stdout) else {
            return Ok(None);
        };

        let state = match value.state.as_str() {
            "OPEN" => PullRequestState::Open,
            "CLOSED" => PullRequestState::Closed,
            "MERGED" => PullRequestState::Merged,
            _ => PullRequestState::Open,
        };
        Ok(Some(PullRequestInfo {
            number: value.number,
            is_draft: value.is_draft,
            state,
        }))
    }

    fn gh_open_pull_request(&self, worktree_path: PathBuf) -> Result<(), String> {
        let output = Command::new("gh")
            .args(["pr", "view", "--web"])
            .current_dir(worktree_path)
            .output();

        let Ok(output) = output else {
            return Err("Failed to run gh".to_owned());
        };
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if stderr.is_empty() {
            Err("Failed to open pull request".to_owned())
        } else {
            Err(stderr)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn run_git(repo_path: &Path, args: &[&str]) -> std::process::Output {
        Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .expect("git should spawn")
    }

    fn assert_git_success(repo_path: &Path, args: &[&str]) {
        let output = run_git(repo_path, args);
        if !output.status.success() {
            panic!(
                "git failed ({:?}):\nstdout:\n{}\nstderr:\n{}",
                args,
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
    }

    #[test]
    fn transient_reconnect_notice_detection_is_stable() {
        assert!(is_transient_reconnect_notice("reconnecting ...1/5"));
        assert!(is_transient_reconnect_notice("Reconnecting (12/100)"));
        assert!(!is_transient_reconnect_notice("retry/reconnect"));
        assert!(!is_transient_reconnect_notice("connection failed"));
        assert!(!is_transient_reconnect_notice("reconnecting soon"));
    }

    #[test]
    fn codex_runner_reports_missing_executable() {
        let _guard = ENV_LOCK.lock().expect("env lock should work");

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be valid")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!(
            "luban-missing-codex-check-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&base_dir).expect("temp dir should be created");

        let missing_codex = base_dir.join("missing-codex-bin");
        unsafe {
            std::env::set_var(paths::LUBAN_CODEX_BIN_ENV, missing_codex.as_os_str());
        }

        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            sqlite,
        };

        let err = service
            .run_codex_turn_streamed_via_cli(
                CodexTurnParams {
                    thread_id: None,
                    worktree_path: base_dir.clone(),
                    prompt: "Hi".to_owned(),
                    image_paths: Vec::new(),
                    model: None,
                    model_reasoning_effort: None,
                },
                Arc::new(AtomicBool::new(false)),
                |_event| Ok(()),
            )
            .expect_err("missing codex executable should fail");

        unsafe {
            std::env::remove_var(paths::LUBAN_CODEX_BIN_ENV);
        }

        assert!(err.to_string().contains("missing codex executable"));

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn codex_item_ids_are_scoped_per_turn() {
        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };
        let a = qualify_codex_item("turn-a", item.clone());
        let b = qualify_codex_item("turn-b", item);
        assert_eq!(codex_item_id(&a), "turn-a/item_0");
        assert_eq!(codex_item_id(&b), "turn-b/item_0");
        let a2 = qualify_codex_item("turn-a", a);
        assert_eq!(codex_item_id(&a2), "turn-a/item_0");
    }

    #[test]
    fn worktree_remove_force_allows_dirty_worktree() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be valid")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!(
            "luban-worktree-remove-force-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir_all(&base_dir).expect("temp dir should be created");

        let repo_path = base_dir.join("repo");
        std::fs::create_dir_all(&repo_path).expect("repo dir should be created");

        assert_git_success(&repo_path, &["init"]);
        assert_git_success(&repo_path, &["config", "user.name", "Test User"]);
        assert_git_success(&repo_path, &["config", "user.email", "test@example.com"]);

        let tracked_file = repo_path.join("tracked.txt");
        std::fs::write(&tracked_file, "hello\n").expect("write should succeed");
        assert_git_success(&repo_path, &["add", "."]);
        assert_git_success(&repo_path, &["commit", "-m", "init"]);

        let worktree_path = base_dir.join("worktree");
        let branch_name = format!("luban-test-branch-{unique}");
        assert_git_success(
            &repo_path,
            &[
                "worktree",
                "add",
                "-b",
                &branch_name,
                worktree_path
                    .to_str()
                    .expect("worktree path should be utf-8"),
            ],
        );

        let dirty_file = worktree_path.join("tracked.txt");
        std::fs::write(&dirty_file, "hello\ndirty\n").expect("write should succeed");

        let no_force = run_git(
            &repo_path,
            &[
                "worktree",
                "remove",
                worktree_path
                    .to_str()
                    .expect("worktree path should be utf-8"),
            ],
        );
        assert!(
            !no_force.status.success(),
            "worktree remove without --force should fail for dirty worktree"
        );

        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            sqlite,
        };

        ProjectWorkspaceService::archive_workspace(
            &service,
            repo_path.clone(),
            worktree_path.clone(),
        )
        .expect("archive_workspace should remove dirty worktree with --force");
        assert!(!worktree_path.exists(), "worktree path should be removed");

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
    }
}
