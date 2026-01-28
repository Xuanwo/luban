use anyhow::{Context as _, anyhow};
use luban_domain::paths;
use luban_domain::{
    AgentThreadEvent, AttachmentKind, AttachmentRef, ClaudeConfigEntry, ClaudeConfigEntryKind,
    CodexConfigEntry, CodexConfigEntryKind, CodexThreadEvent, CodexThreadItem, ContextImage,
    ConversationEntry, ConversationSnapshot, CreatedWorkspace, OpenTarget, PersistedAppState,
    ProjectWorkspaceService, PullRequestCiState, PullRequestInfo, PullRequestState,
    RunAgentTurnRequest, SystemTaskKind, TaskIntentKind,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    time::Instant,
};

use claude_process::{ClaudeProcessKey, ClaudeThreadProcess};

use crate::sqlite_store::{SqliteStore, SqliteStoreOptions};
use crate::time::unix_epoch_nanos_now;

mod amp_cli;
mod amp_mode;
mod ansi;
mod claude_cli;
pub mod claude_process;
mod cli_check;
mod codex_bin;
mod codex_cli;
mod codex_thread;
mod config_file_io;
mod config_path;
mod config_tree;
mod context_blobs;
mod conversations;
mod feedback;
mod git;
mod prompt;
mod pull_request;
mod reconnect_notice;
mod roots;
mod stream_json;
mod task;
mod workspace_name;
use amp_cli::AmpTurnParams;
use amp_mode::detect_amp_mode_from_config_root;
use claude_cli::ClaudeTurnParams;
use codex_cli::CodexTurnParams;
use codex_thread::{codex_item_id, generate_turn_scope_id, qualify_codex_item, qualify_event};
use prompt::{format_amp_prompt, format_codex_prompt, resolve_prompt_attachments};
use pull_request::pull_request_ci_state_from_check_buckets;
use reconnect_notice::is_transient_reconnect_notice;
use roots::{resolve_amp_root, resolve_claude_root, resolve_codex_root, resolve_luban_root};

fn codex_entries_from_shallow(entries: Vec<config_tree::ShallowEntry>) -> Vec<CodexConfigEntry> {
    entries
        .into_iter()
        .map(|entry| CodexConfigEntry {
            path: entry.path,
            name: entry.name,
            kind: match entry.kind {
                config_tree::ShallowEntryKind::Folder => CodexConfigEntryKind::Folder,
                config_tree::ShallowEntryKind::File => CodexConfigEntryKind::File,
            },
            children: Vec::new(),
        })
        .collect()
}

fn amp_entries_from_shallow(
    entries: Vec<config_tree::ShallowEntry>,
) -> Vec<luban_domain::AmpConfigEntry> {
    entries
        .into_iter()
        .map(|entry| luban_domain::AmpConfigEntry {
            path: entry.path,
            name: entry.name,
            kind: match entry.kind {
                config_tree::ShallowEntryKind::Folder => luban_domain::AmpConfigEntryKind::Folder,
                config_tree::ShallowEntryKind::File => luban_domain::AmpConfigEntryKind::File,
            },
            children: Vec::new(),
        })
        .collect()
}

fn claude_entries_from_shallow(entries: Vec<config_tree::ShallowEntry>) -> Vec<ClaudeConfigEntry> {
    entries
        .into_iter()
        .map(|entry| ClaudeConfigEntry {
            path: entry.path,
            name: entry.name,
            kind: match entry.kind {
                config_tree::ShallowEntryKind::Folder => ClaudeConfigEntryKind::Folder,
                config_tree::ShallowEntryKind::File => ClaudeConfigEntryKind::File,
            },
            children: Vec::new(),
        })
        .collect()
}

/// Git workspace service with persistent Claude process management.
///
/// Each thread/tab can have its own Claude process that maintains MCP connections
/// across multiple turns, avoiding the overhead of reconnecting MCP for each turn.
pub struct GitWorkspaceService {
    worktrees_root: PathBuf,
    conversations_root: PathBuf,
    task_prompts_root: PathBuf,
    sqlite: SqliteStore,

    /// Persistent Claude processes mapped by (project_slug, workspace_name, thread_local_id).
    /// Each thread can have at most one active Claude process.
    claude_processes: Mutex<HashMap<ClaudeProcessKey, ClaudeThreadProcess>>,
}

impl GitWorkspaceService {
    pub fn new() -> anyhow::Result<Arc<Self>> {
        Self::new_with_options(SqliteStoreOptions::default())
    }

    pub fn new_with_options(options: SqliteStoreOptions) -> anyhow::Result<Arc<Self>> {
        let luban_root = resolve_luban_root()?;

        std::fs::create_dir_all(&luban_root)
            .with_context(|| format!("failed to create {}", luban_root.display()))?;

        let worktrees_root = paths::worktrees_root(&luban_root);
        let conversations_root = paths::conversations_root(&luban_root);
        let task_prompts_root = paths::task_prompts_root(&luban_root);
        let sqlite_path = paths::sqlite_path(&luban_root);
        let sqlite = SqliteStore::new_with_options(sqlite_path, options)
            .context("failed to init sqlite store")?;
        std::fs::create_dir_all(&task_prompts_root).with_context(|| {
            format!(
                "failed to create task prompts dir {}",
                task_prompts_root.display()
            )
        })?;

        Ok(Arc::new(Self {
            worktrees_root,
            conversations_root,
            task_prompts_root,
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        }))
    }

    fn generate_workspace_name(&self) -> anyhow::Result<String> {
        workspace_name::generate_workspace_name()
    }

    fn worktree_path(&self, project_slug: &str, workspace_name: &str) -> PathBuf {
        let mut path = self.worktrees_root.clone();
        path.push(project_slug);
        path.push(workspace_name);
        path
    }

    fn task_prompt_template_path(&self, kind: TaskIntentKind) -> PathBuf {
        self.task_prompts_root.join(format!("{}.md", kind.as_key()))
    }

    fn system_prompt_template_path(&self, kind: SystemTaskKind) -> PathBuf {
        self.task_prompts_root.join(format!("{}.md", kind.as_key()))
    }

    fn codex_executable(&self) -> PathBuf {
        codex_bin::codex_executable()
    }

    // ========================================================================
    // Claude Process Management
    // ========================================================================

    /// Get an existing Claude process for the given thread, or create a new one.
    ///
    /// If the existing process is not alive, it will be cleaned up and a new one created.
    fn get_or_create_claude_process(
        &self,
        key: ClaudeProcessKey,
        worktree_path: &Path,
        thread_id: Option<&str>,
        add_dirs: &[PathBuf],
    ) -> anyhow::Result<()> {
        let mut processes = self
            .claude_processes
            .lock()
            .map_err(|_| anyhow!("failed to lock claude_processes"))?;

        // Check if we have an existing process
        if let Some(process) = processes.get(&key) {
            if process.is_alive() {
                // Existing process is alive, nothing to do
                return Ok(());
            }
            // Process is dead, remove it
            processes.remove(&key);
        }

        // Create a new process
        let process = ClaudeThreadProcess::spawn_and_warmup(worktree_path, thread_id, add_dirs)?;
        processes.insert(key, process);

        Ok(())
    }

    /// Clean up the Claude process for the given thread.
    ///
    /// This should be called when a thread/tab is closed to free resources.
    pub fn cleanup_claude_process(
        &self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) {
        let key = ClaudeProcessKey::new(project_slug, workspace_name, thread_local_id);

        if let Ok(mut processes) = self.claude_processes.lock()
            && let Some(mut process) = processes.remove(&key)
        {
            process.shutdown();
        }
    }

    /// Clean up all Claude processes for a given workspace.
    ///
    /// This should be called when a workspace is closed.
    pub fn cleanup_workspace_claude_processes(&self, project_slug: &str, workspace_name: &str) {
        if let Ok(mut processes) = self.claude_processes.lock() {
            let keys_to_remove: Vec<_> = processes
                .keys()
                .filter(|k| k.project_slug == project_slug && k.workspace_name == workspace_name)
                .cloned()
                .collect();

            for key in keys_to_remove {
                if let Some(mut process) = processes.remove(&key) {
                    process.shutdown();
                }
            }
        }
    }

    /// Check if a Claude process exists and is alive for the given thread.
    #[allow(dead_code)]
    fn has_alive_claude_process(&self, key: &ClaudeProcessKey) -> bool {
        if let Ok(processes) = self.claude_processes.lock() {
            processes.get(key).is_some_and(|p| p.is_alive())
        } else {
            false
        }
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

    fn run_amp_turn_streamed_via_cli(
        &self,
        params: AmpTurnParams,
        cancel: Arc<AtomicBool>,
        on_event: impl FnMut(CodexThreadEvent) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        amp_cli::run_amp_turn_streamed_via_cli(params, cancel, on_event)
    }

    #[allow(dead_code)]
    fn run_claude_turn_streamed_via_cli(
        &self,
        params: ClaudeTurnParams,
        cancel: Arc<AtomicBool>,
        on_event: impl FnMut(CodexThreadEvent) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        claude_cli::run_claude_turn_streamed_via_cli(params, cancel, on_event)
    }

    /// Run a Claude turn with process reuse.
    ///
    /// This uses persistent processes that stay alive across turns, avoiding
    /// MCP reconnection overhead. The process uses `--input-format stream-json`
    /// to accept prompts from stdin.
    fn run_claude_turn_with_process_reuse(
        &self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        params: ClaudeTurnParams,
        cancel: Arc<AtomicBool>,
        mut on_event: impl FnMut(CodexThreadEvent) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let key = ClaudeProcessKey::new(project_slug, workspace_name, thread_local_id);

        // Get or create the persistent process
        // This will create a new process if none exists or if the existing one died
        self.get_or_create_claude_process(
            key.clone(),
            &params.worktree_path,
            params.thread_id.as_deref(),
            &params.add_dirs,
        )?;

        // Send the prompt via stdin
        {
            let processes = self
                .claude_processes
                .lock()
                .map_err(|_| anyhow!("failed to lock claude_processes"))?;

            let process = processes
                .get(&key)
                .ok_or_else(|| anyhow!("process not found after creation"))?;

            if !process.is_alive() {
                // Process died unexpectedly after creation - this shouldn't happen
                // but if it does, recreate it
                drop(processes);
                self.get_or_create_claude_process(
                    key.clone(),
                    &params.worktree_path,
                    params.thread_id.as_deref(),
                    &params.add_dirs,
                )?;

                // Try again with the new process
                let processes = self
                    .claude_processes
                    .lock()
                    .map_err(|_| anyhow!("failed to lock claude_processes"))?;
                let process = processes
                    .get(&key)
                    .ok_or_else(|| anyhow!("process not found after recreation"))?;

                on_event(CodexThreadEvent::TurnStarted)?;
                process.send_prompt(&params.prompt)?;
            } else {
                // Emit TurnStarted
                on_event(CodexThreadEvent::TurnStarted)?;

                // Send the prompt
                process.send_prompt(&params.prompt)?;
            }
        }

        // Poll for events until turn completes or cancelled
        let timeout = std::time::Duration::from_secs(600); // 10 minute timeout
        let start = std::time::Instant::now();

        loop {
            if cancel.load(Ordering::SeqCst) {
                break;
            }

            if start.elapsed() > timeout {
                return Err(anyhow!("claude turn timed out"));
            }

            // Poll events from the process
            let (events, is_turn_completed, is_alive) = {
                let processes = self
                    .claude_processes
                    .lock()
                    .map_err(|_| anyhow!("failed to lock claude_processes"))?;

                if let Some(process) = processes.get(&key) {
                    let events = process.poll_events();
                    let completed = process.is_turn_completed();
                    let alive = process.is_alive();
                    (events, completed, alive)
                } else {
                    (Vec::new(), true, false)
                }
            };

            // Forward events to the callback
            for event in events {
                on_event(event)?;
            }

            if is_turn_completed || !is_alive {
                break;
            }

            // Small sleep to avoid busy-waiting
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(())
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
        branch_name_hint: Option<String>,
    ) -> Result<CreatedWorkspace, String> {
        let result: anyhow::Result<CreatedWorkspace> = (|| {
            let remote = "origin";
            self.run_git(&project_path, ["remote", "get-url", remote])
                .with_context(|| format!("remote '{remote}' not found"))?;

            self.run_git(&project_path, ["fetch", "--prune", remote, "main"])
                .with_context(|| format!("failed to fetch '{remote}/main'"))?;

            let upstream_commit = self
                .run_git(
                    &project_path,
                    ["rev-parse", "--verify", "origin/main^{commit}"],
                )
                .context("failed to resolve origin/main commit")?;

            std::fs::create_dir_all(self.worktrees_root.join(&project_slug))
                .context("failed to create worktrees root")?;

            fn normalize_branch_suffix(raw: &str) -> Option<String> {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return None;
                }

                let mut rest = trimmed;
                if let Some(stripped) = rest.strip_prefix("refs/heads/") {
                    rest = stripped;
                }
                if let Some(stripped) = rest.strip_prefix("luban/") {
                    rest = stripped;
                }

                let mut out = String::new();
                let mut prev_hyphen = false;
                for ch in rest.chars() {
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
                let limited = trimmed.chars().take(MAX_SUFFIX_LEN).collect::<String>();
                let limited = limited.trim_matches('-').to_owned();
                if limited.is_empty() {
                    return None;
                }
                Some(limited)
            }

            fn branch_exists(project_path: &Path, branch_name: &str) -> bool {
                let branch_ref = format!("refs/heads/{branch_name}");
                Command::new("git")
                    .args(["show-ref", "--verify", "--quiet", &branch_ref])
                    .current_dir(project_path)
                    .status()
                    .ok()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }

            if let Some(hint) = branch_name_hint
                .as_deref()
                .and_then(normalize_branch_suffix)
            {
                for attempt in 0..64 {
                    let workspace_name = if attempt == 0 {
                        hint.clone()
                    } else {
                        format!("{hint}-v{}", attempt + 1)
                    };

                    let branch_name = format!("luban/{workspace_name}");
                    let worktree_path = self.worktree_path(&project_slug, &workspace_name);

                    if worktree_path.exists() {
                        continue;
                    }
                    if branch_exists(&project_path, &branch_name) {
                        continue;
                    }

                    self.run_git(
                        &project_path,
                        [
                            "worktree",
                            "add",
                            "-b",
                            &branch_name,
                            worktree_path
                                .to_str()
                                .ok_or_else(|| anyhow!("invalid worktree path"))?,
                            upstream_commit.trim(),
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
            }

            for _ in 0..64 {
                let workspace_name = self.generate_workspace_name()?;
                let branch_name = format!("luban/{workspace_name}");
                let worktree_path = self.worktree_path(&project_slug, &workspace_name);

                if worktree_path.exists() {
                    continue;
                }

                if branch_exists(&project_path, &branch_name) {
                    continue;
                }

                self.run_git(
                    &project_path,
                    [
                        "worktree",
                        "add",
                        "-b",
                        &branch_name,
                        worktree_path
                            .to_str()
                            .ok_or_else(|| anyhow!("invalid worktree path"))?,
                        upstream_commit.trim(),
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
        self.open_workspace_with(worktree_path, OpenTarget::Zed)
    }

    fn open_workspace_with(
        &self,
        worktree_path: PathBuf,
        target: OpenTarget,
    ) -> Result<(), String> {
        let result: anyhow::Result<()> = (|| {
            if !worktree_path.exists() {
                return Err(anyhow!(
                    "workspace path does not exist: {}",
                    worktree_path.display()
                ));
            }

            #[cfg(target_os = "macos")]
            {
                let mut cmd = Command::new("open");
                let cmd_label: &'static str = match target {
                    OpenTarget::Vscode => "open -a Visual Studio Code",
                    OpenTarget::Cursor => "open -a Cursor",
                    OpenTarget::Zed => "open -a Zed",
                    OpenTarget::Ghostty => "open -a Ghostty",
                    OpenTarget::Finder => "open -R",
                };

                match target {
                    OpenTarget::Vscode => {
                        cmd.args(["-a", "Visual Studio Code"]).arg(&worktree_path);
                    }
                    OpenTarget::Cursor => {
                        cmd.args(["-a", "Cursor"]).arg(&worktree_path);
                    }
                    OpenTarget::Zed => {
                        cmd.args(["-a", "Zed"]).arg(&worktree_path);
                    }
                    OpenTarget::Ghostty => {
                        cmd.args(["-a", "Ghostty"]);
                    }
                    OpenTarget::Finder => {
                        cmd.arg("-R").arg(&worktree_path);
                    }
                }

                let status = cmd
                    .status()
                    .with_context(|| format!("failed to spawn '{cmd_label}'"))?;
                if !status.success() {
                    return Err(anyhow!("'{cmd_label}' exited with status: {status}"));
                }
                Ok(())
            }

            #[cfg(target_os = "linux")]
            {
                let command = linux_open_command(target, &worktree_path)?;
                let status = Command::new(command.program)
                    .args(&command.args)
                    .status()
                    .with_context(|| format!("failed to spawn '{}'", command.label))?;
                if !status.success() {
                    return Err(anyhow!("'{}' exited with status: {status}", command.label));
                }
                Ok(())
            }

            #[cfg(all(not(target_os = "macos"), not(target_os = "linux")))]
            {
                let _ = worktree_path;
                let _ = target;
                Err(anyhow!(
                    "opening external apps is only supported on macOS and Linux for now"
                ))
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

    fn rename_workspace_branch(
        &self,
        worktree_path: PathBuf,
        requested_branch_name: String,
    ) -> Result<String, String> {
        let result: anyhow::Result<String> = (|| {
            if !worktree_path.exists() {
                return Err(anyhow!(
                    "workspace path does not exist: {}",
                    worktree_path.display()
                ));
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

            fn branch_exists(worktree_path: &Path, branch_name: &str) -> bool {
                let branch_ref = format!("refs/heads/{branch_name}");
                Command::new("git")
                    .args(["show-ref", "--verify", "--quiet", &branch_ref])
                    .current_dir(worktree_path)
                    .status()
                    .ok()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }

            let current_branch = self
                .run_git(&worktree_path, ["rev-parse", "--abbrev-ref", "HEAD"])
                .context("failed to resolve current branch")?;
            let current_branch = current_branch.trim();
            if current_branch.is_empty() || current_branch == "HEAD" {
                return Err(anyhow!(
                    "workspace is not on a branch (current branch is '{}')",
                    current_branch
                ));
            }
            if current_branch == "main" {
                return Err(anyhow!("refusing to rename main branch"));
            }

            let normalized = normalize_branch_name(&requested_branch_name)
                .ok_or_else(|| anyhow!("invalid branch name"))?;
            if normalized == current_branch {
                return Ok(normalized);
            }

            for attempt in 1..=64 {
                let candidate = if attempt == 1 {
                    normalized.clone()
                } else {
                    format!("{normalized}-v{attempt}")
                };

                if candidate != current_branch && branch_exists(&worktree_path, &candidate) {
                    continue;
                }

                let rename_result = self.run_git(&worktree_path, ["branch", "-m", &candidate]);
                if let Err(err) = rename_result {
                    let message = err.to_string();
                    let exists = message.contains("already exists")
                        || message.contains("a branch named")
                        || message.contains("a branch with that name")
                        || message.contains("is not a valid branch name");
                    let locked = message.contains("cannot lock ref")
                        || message.contains("cannot lock")
                        || message.contains("unable to lock");
                    if (exists || locked) && candidate != current_branch {
                        continue;
                    }
                    return Err(err)
                        .with_context(|| format!("failed to rename branch to {candidate}"));
                }

                let updated = self
                    .run_git(&worktree_path, ["rev-parse", "--abbrev-ref", "HEAD"])
                    .context("failed to resolve renamed branch")?;
                let updated = updated.trim().to_owned();
                if updated.is_empty() || updated == "HEAD" {
                    return Err(anyhow!("failed to resolve renamed branch"));
                }
                return Ok(updated);
            }

            Err(anyhow!("failed to find a free branch name after retries"))
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

    fn load_conversation_page(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
        before: Option<u64>,
        limit: u64,
    ) -> Result<ConversationSnapshot, String> {
        let snapshot = self
            .sqlite
            .load_conversation_page(
                project_slug.clone(),
                workspace_name.clone(),
                thread_id,
                before,
                limit,
            )
            .map_err(|e| format!("{e:#}"))?;

        if thread_id == 1 && snapshot.entries_total == 0 && snapshot.thread_id.is_none() {
            return self
                .load_conversation_internal(project_slug, workspace_name, thread_id)
                .map(|mut repaired| {
                    let total = repaired.entries.len();
                    let before = before
                        .and_then(|v| usize::try_from(v).ok())
                        .unwrap_or(total)
                        .min(total);
                    let limit = usize::try_from(limit).unwrap_or(0);
                    let end = before;
                    let start = end.saturating_sub(limit);
                    repaired.entries = repaired
                        .entries
                        .get(start..end)
                        .unwrap_or_default()
                        .to_vec();
                    repaired.entries_total = total as u64;
                    repaired.entries_start = start as u64;
                    repaired
                })
                .map_err(|e| format!("{e:#}"));
        }

        Ok(snapshot)
    }

    fn save_conversation_queue_state(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
        queue_paused: bool,
        run_started_at_unix_ms: Option<u64>,
        run_finished_at_unix_ms: Option<u64>,
        pending_prompts: Vec<luban_domain::QueuedPrompt>,
    ) -> Result<(), String> {
        self.sqlite
            .save_conversation_queue_state(
                project_slug,
                workspace_name,
                thread_id,
                queue_paused,
                run_started_at_unix_ms,
                run_finished_at_unix_ms,
                pending_prompts,
            )
            .map_err(|e| format!("{e:#}"))
    }

    fn save_conversation_run_config(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
        model_id: String,
        thinking_effort: luban_domain::ThinkingEffort,
    ) -> Result<(), String> {
        self.sqlite
            .save_conversation_run_config(
                project_slug,
                workspace_name,
                thread_id,
                model_id,
                thinking_effort,
            )
            .map_err(|e| format!("{e:#}"))
    }

    fn store_context_image(
        &self,
        project_slug: String,
        workspace_name: String,
        image: ContextImage,
    ) -> Result<AttachmentRef, String> {
        let byte_len = image.bytes.len() as u64;
        let stored: anyhow::Result<(String, PathBuf)> = self.store_context_bytes(
            &project_slug,
            &workspace_name,
            &image.bytes,
            &image.extension,
        );
        let (id, stored_path) = stored.map_err(|e| format!("{e:#}"))?;
        let _ = self.maybe_store_context_image_thumbnail(
            &project_slug,
            &workspace_name,
            &stored_path,
            &image.bytes,
        );
        let extension = stored_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("png")
            .to_owned();
        Ok(AttachmentRef {
            id,
            kind: AttachmentKind::Image,
            name: format!("image.{extension}"),
            extension,
            mime: None,
            byte_len,
        })
    }

    fn store_context_text(
        &self,
        project_slug: String,
        workspace_name: String,
        text: String,
        extension: String,
    ) -> Result<AttachmentRef, String> {
        let bytes = text.into_bytes();
        let byte_len = bytes.len() as u64;
        let result: anyhow::Result<(String, PathBuf)> =
            self.store_context_bytes(&project_slug, &workspace_name, &bytes, &extension);
        let (id, stored_path) = result.map_err(|e| format!("{e:#}"))?;
        let extension = stored_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("txt")
            .to_owned();
        Ok(AttachmentRef {
            id,
            kind: AttachmentKind::Text,
            name: format!("text.{extension}"),
            extension,
            mime: None,
            byte_len,
        })
    }

    fn store_context_file(
        &self,
        project_slug: String,
        workspace_name: String,
        source_path: PathBuf,
    ) -> Result<AttachmentRef, String> {
        let name = source_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file")
            .to_owned();
        let result: anyhow::Result<(String, String, u64, PathBuf)> =
            self.store_context_file_internal(&project_slug, &workspace_name, &source_path);
        let (id, extension, byte_len, _path) = result.map_err(|e| format!("{e:#}"))?;
        Ok(AttachmentRef {
            id,
            kind: AttachmentKind::File,
            name,
            extension,
            mime: None,
            byte_len,
        })
    }

    fn record_context_item(
        &self,
        project_slug: String,
        workspace_name: String,
        attachment: AttachmentRef,
        created_at_unix_ms: u64,
    ) -> Result<u64, String> {
        self.sqlite
            .insert_context_item(project_slug, workspace_name, attachment, created_at_unix_ms)
            .map_err(|e| format!("{e:#}"))
    }

    fn list_context_items(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> Result<Vec<luban_domain::ContextItem>, String> {
        self.sqlite
            .list_context_items(project_slug, workspace_name)
            .map_err(|e| format!("{e:#}"))
    }

    fn delete_context_item(
        &self,
        project_slug: String,
        workspace_name: String,
        context_id: u64,
    ) -> Result<(), String> {
        self.sqlite
            .delete_context_item(project_slug, workspace_name, context_id)
            .map_err(|e| format!("{e:#}"))
    }

    fn run_agent_turn_streamed(
        &self,
        request: RunAgentTurnRequest,
        cancel: Arc<AtomicBool>,
        on_event: Arc<dyn Fn(AgentThreadEvent) + Send + Sync>,
    ) -> Result<(), String> {
        let RunAgentTurnRequest {
            project_slug,
            workspace_name,
            worktree_path,
            thread_local_id,
            thread_id,
            prompt,
            attachments,
            runner,
            amp_mode,
            model,
            model_reasoning_effort,
        } = request;

        let turn_started_at = Instant::now();
        let turn_scope_id = generate_turn_scope_id();
        let duration_appended = Arc::new(AtomicBool::new(false));
        let mut appended_item_ids = HashSet::<String>::new();
        let mut saw_agent_message = false;

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
                    attachments: attachments.clone(),
                }],
            )?;

            let resolved_thread_id = thread_id.or(existing_thread_id);
            let blobs_dir = self.context_blobs_dir(&project_slug, &workspace_name);
            let prompt_attachments = resolve_prompt_attachments(&blobs_dir, &attachments);
            let image_paths = prompt_attachments
                .iter()
                .filter(|a| a.kind == AttachmentKind::Image)
                .map(|a| a.path.clone())
                .collect::<Vec<_>>();

            let runner = std::env::var("LUBAN_AGENT_RUNNER")
                .ok()
                .as_deref()
                .and_then(luban_domain::parse_agent_runner_kind)
                .unwrap_or(runner);
            let use_amp = runner == luban_domain::AgentRunnerKind::Amp;
            let amp_prompt = if use_amp {
                format_amp_prompt(&prompt, &prompt_attachments)
            } else {
                prompt.clone()
            };
            let codex_prompt = format_codex_prompt(&prompt, &prompt_attachments);

            let env_amp_mode = std::env::var("LUBAN_AMP_MODE")
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty());
            let resolved_amp_mode = if use_amp {
                let amp_root = resolve_amp_root().ok();
                env_amp_mode.or_else(|| amp_mode.clone()).or_else(|| {
                    amp_root
                        .as_deref()
                        .and_then(detect_amp_mode_from_config_root)
                })
            } else {
                None
            };

            let mut turn_error: Option<String> = None;
            let mut transient_error_seq: u64 = 0;
            let duration_appended_for_events = duration_appended.clone();

            let run_result = if use_amp {
                self.run_amp_turn_streamed_via_cli(
                    AmpTurnParams {
                        thread_id: resolved_thread_id,
                        worktree_path: worktree_path.clone(),
                        prompt: amp_prompt,
                        mode: resolved_amp_mode.clone(),
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

                            if let CodexThreadEvent::ItemStarted { item }
                            | CodexThreadEvent::ItemUpdated { item }
                            | CodexThreadEvent::ItemCompleted { item } = &event
                                && matches!(item, CodexThreadItem::AgentMessage { .. })
                            {
                                saw_agent_message = true;
                            }

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
                                        let duration_ms =
                                            turn_started_at.elapsed().as_millis() as u64;
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
                                        let duration_ms =
                                            turn_started_at.elapsed().as_millis() as u64;
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
                                        let duration_ms =
                                            turn_started_at.elapsed().as_millis() as u64;
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
                )
            } else if runner == luban_domain::AgentRunnerKind::Claude {
                self.run_claude_turn_with_process_reuse(
                    &project_slug,
                    &workspace_name,
                    thread_local_id,
                    ClaudeTurnParams {
                        thread_id: resolved_thread_id,
                        worktree_path: worktree_path.clone(),
                        prompt: codex_prompt.clone(),
                        add_dirs: if prompt_attachments.is_empty() {
                            Vec::new()
                        } else {
                            vec![blobs_dir.clone()]
                        },
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

                            if let CodexThreadEvent::ItemStarted { item }
                            | CodexThreadEvent::ItemUpdated { item }
                            | CodexThreadEvent::ItemCompleted { item } = &event
                                && matches!(item, CodexThreadItem::AgentMessage { .. })
                            {
                                saw_agent_message = true;
                            }

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
                                        let duration_ms =
                                            turn_started_at.elapsed().as_millis() as u64;
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
                                        let duration_ms =
                                            turn_started_at.elapsed().as_millis() as u64;
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
                                        let duration_ms =
                                            turn_started_at.elapsed().as_millis() as u64;
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
                )
            } else {
                self.run_codex_turn_streamed_via_cli(
                    CodexTurnParams {
                        thread_id: resolved_thread_id,
                        worktree_path: worktree_path.clone(),
                        prompt: codex_prompt,
                        image_paths,
                        model: model.clone(),
                        model_reasoning_effort: model_reasoning_effort.clone(),
                        sandbox_mode: None,
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

                            if let CodexThreadEvent::ItemStarted { item }
                            | CodexThreadEvent::ItemUpdated { item }
                            | CodexThreadEvent::ItemCompleted { item } = &event
                                && matches!(item, CodexThreadItem::AgentMessage { .. })
                            {
                                saw_agent_message = true;
                            }

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
                                        let duration_ms =
                                            turn_started_at.elapsed().as_millis() as u64;
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
                                        let duration_ms =
                                            turn_started_at.elapsed().as_millis() as u64;
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
                                        let duration_ms =
                                            turn_started_at.elapsed().as_millis() as u64;
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
                )
            };

            run_result?;

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

            if !saw_agent_message {
                return Err(anyhow!("agent finished without a final message"));
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
        #[derive(Clone, serde::Deserialize)]
        struct GhPullRequestCheck {
            #[serde(default)]
            bucket: String,
        }

        #[derive(serde::Deserialize)]
        struct GhPullRequestView {
            number: u64,
            #[serde(default, rename = "isDraft")]
            is_draft: bool,
            #[serde(default)]
            state: String,
            #[serde(default, rename = "mergeStateStatus")]
            merge_state_status: String,
            #[serde(default, rename = "reviewDecision")]
            review_decision: String,
        }

        let output = Command::new("gh")
            .args([
                "pr",
                "view",
                "--json",
                "number,isDraft,state,mergeStateStatus,reviewDecision",
            ])
            .current_dir(&worktree_path)
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

        if state != PullRequestState::Open {
            return Ok(Some(PullRequestInfo {
                number: value.number,
                is_draft: value.is_draft,
                state,
                ci_state: None,
                merge_ready: false,
            }));
        }

        fn parse_checks(output: &std::process::Output) -> Option<Vec<GhPullRequestCheck>> {
            serde_json::from_slice::<Vec<GhPullRequestCheck>>(&output.stdout).ok()
        }

        let required_checks_output = Command::new("gh")
            .args(["pr", "checks", "--required", "--json", "bucket"])
            .current_dir(&worktree_path)
            .output();
        let required_checks_parsed = required_checks_output.as_ref().ok().and_then(parse_checks);

        let mut all_checks_parsed: Option<Vec<GhPullRequestCheck>> = None;
        let checks = if required_checks_parsed
            .as_ref()
            .is_some_and(|checks| !checks.is_empty())
        {
            required_checks_parsed.clone().unwrap_or_default()
        } else {
            let all_checks_output = Command::new("gh")
                .args(["pr", "checks", "--json", "bucket"])
                .current_dir(&worktree_path)
                .output();
            all_checks_parsed = all_checks_output.as_ref().ok().and_then(parse_checks);
            all_checks_parsed.clone().unwrap_or_default()
        };

        let checks_known = required_checks_parsed.is_some() || all_checks_parsed.is_some();

        let ci_state = if !checks_known {
            None
        } else if checks.is_empty() {
            Some(PullRequestCiState::Success)
        } else {
            pull_request_ci_state_from_check_buckets(
                checks.iter().map(|check| check.bucket.as_str()),
            )
        };
        let merge_ready = pull_request::is_merge_ready(
            state,
            value.is_draft,
            &value.merge_state_status,
            &value.review_decision,
            ci_state,
        );

        Ok(Some(PullRequestInfo {
            number: value.number,
            is_draft: value.is_draft,
            state,
            ci_state,
            merge_ready,
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

    fn gh_open_pull_request_failed_action(&self, worktree_path: PathBuf) -> Result<(), String> {
        #[derive(serde::Deserialize)]
        struct GhPullRequestCheck {
            #[serde(default)]
            bucket: String,
            #[serde(default)]
            link: String,
        }

        let result: anyhow::Result<()> = (|| {
            let output = Command::new("gh")
                .args(["pr", "checks", "--required", "--json", "bucket,link"])
                .current_dir(&worktree_path)
                .output()
                .context("failed to run 'gh pr checks'")?;

            fn failing_check_url(checks: &[GhPullRequestCheck]) -> Option<&str> {
                checks
                    .iter()
                    .find(|check| {
                        (check.bucket == "fail" || check.bucket == "cancel")
                            && !check.link.is_empty()
                    })
                    .map(|check| check.link.as_str())
            }

            let required_checks = serde_json::from_slice::<Vec<GhPullRequestCheck>>(&output.stdout)
                .unwrap_or_default();
            if let Some(url) = failing_check_url(&required_checks) {
                return self.open_url(url);
            }

            if required_checks.is_empty() {
                let output = Command::new("gh")
                    .args(["pr", "checks", "--json", "bucket,link"])
                    .current_dir(&worktree_path)
                    .output()
                    .context("failed to run 'gh pr checks'")?;
                let checks = serde_json::from_slice::<Vec<GhPullRequestCheck>>(&output.stdout)
                    .unwrap_or_default();
                if let Some(url) = failing_check_url(&checks) {
                    return self.open_url(url);
                }
            }

            let output = Command::new("gh")
                .args(["pr", "checks", "--web"])
                .current_dir(&worktree_path)
                .output()
                .context("failed to run 'gh pr checks --web'")?;
            if output.status.success() {
                return Ok(());
            }

            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if stderr.is_empty() {
                Err(anyhow!("failed to open pull request checks"))
            } else {
                Err(anyhow!("{stderr}"))
            }
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn task_preview(&self, input: String) -> Result<luban_domain::TaskDraft, String> {
        task::task_preview(self, input).map_err(|e| format!("{e:#}"))
    }

    fn task_prepare_project(&self, spec: luban_domain::TaskProjectSpec) -> Result<PathBuf, String> {
        task::task_prepare_project(self, spec).map_err(|e| format!("{e:#}"))
    }

    fn feedback_create_issue(
        &self,
        title: String,
        body: String,
        labels: Vec<String>,
    ) -> Result<luban_domain::TaskIssueInfo, String> {
        feedback::feedback_create_issue(title, body, labels).map_err(|e| format!("{e:#}"))
    }

    fn feedback_task_draft(
        &self,
        issue: luban_domain::TaskIssueInfo,
        intent_kind: TaskIntentKind,
    ) -> Result<luban_domain::TaskDraft, String> {
        feedback::feedback_task_draft(self, issue, intent_kind).map_err(|e| format!("{e:#}"))
    }

    fn task_prompt_templates_load(
        &self,
    ) -> Result<std::collections::HashMap<TaskIntentKind, String>, String> {
        fn inner(
            service: &GitWorkspaceService,
        ) -> anyhow::Result<std::collections::HashMap<TaskIntentKind, String>> {
            let mut out = std::collections::HashMap::new();
            for kind in TaskIntentKind::ALL {
                let path = service.task_prompt_template_path(kind);
                let contents = match std::fs::read_to_string(&path) {
                    Ok(contents) => contents,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(err) => {
                        return Err(anyhow!(err).context(format!(
                            "failed to read task prompt template {}",
                            path.display()
                        )));
                    }
                };
                let trimmed = contents.trim();
                if trimmed.is_empty() {
                    continue;
                }
                out.insert(kind, trimmed.to_owned());
            }
            Ok(out)
        }

        inner(self).map_err(|e| format!("{e:#}"))
    }

    fn task_prompt_template_store(
        &self,
        intent_kind: TaskIntentKind,
        template: String,
    ) -> Result<(), String> {
        fn inner(
            service: &GitWorkspaceService,
            intent_kind: TaskIntentKind,
            template: String,
        ) -> anyhow::Result<()> {
            std::fs::create_dir_all(&service.task_prompts_root).with_context(|| {
                format!(
                    "failed to create task prompts dir {}",
                    service.task_prompts_root.display()
                )
            })?;

            let path = service.task_prompt_template_path(intent_kind);
            let nanos = unix_epoch_nanos_now();
            let pid = std::process::id();
            let tmp = service.task_prompts_root.join(format!(
                ".{}.{}.{}.tmp",
                intent_kind.as_key(),
                pid,
                nanos
            ));

            let mut normalized = template;
            if !normalized.ends_with('\n') {
                normalized.push('\n');
            }

            std::fs::write(&tmp, normalized.as_bytes())
                .with_context(|| format!("failed to write {}", tmp.display()))?;

            if std::fs::rename(&tmp, &path).is_err() {
                if path.exists() {
                    let _ = std::fs::remove_file(&path);
                }
                std::fs::rename(&tmp, &path).with_context(|| {
                    format!("failed to replace task prompt template {}", path.display())
                })?;
            }

            Ok(())
        }

        inner(self, intent_kind, template).map_err(|e| format!("{e:#}"))
    }

    fn task_prompt_template_delete(&self, intent_kind: TaskIntentKind) -> Result<(), String> {
        let path = self.task_prompt_template_path(intent_kind);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!(
                "{:#}",
                anyhow!(err).context(format!("failed to remove {}", path.display()))
            )),
        }
    }

    fn system_prompt_templates_load(
        &self,
    ) -> Result<std::collections::HashMap<SystemTaskKind, String>, String> {
        fn inner(
            service: &GitWorkspaceService,
        ) -> anyhow::Result<std::collections::HashMap<SystemTaskKind, String>> {
            let mut out = std::collections::HashMap::new();
            for kind in SystemTaskKind::ALL {
                let path = service.system_prompt_template_path(kind);
                let contents = match std::fs::read_to_string(&path) {
                    Ok(contents) => contents,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(err) => {
                        return Err(anyhow!(err).context(format!(
                            "failed to read system prompt template {}",
                            path.display()
                        )));
                    }
                };
                let trimmed = contents.trim();
                if trimmed.is_empty() {
                    continue;
                }
                out.insert(kind, trimmed.to_owned());
            }
            Ok(out)
        }

        inner(self).map_err(|e| format!("{e:#}"))
    }

    fn system_prompt_template_store(
        &self,
        kind: SystemTaskKind,
        template: String,
    ) -> Result<(), String> {
        fn inner(
            service: &GitWorkspaceService,
            kind: SystemTaskKind,
            template: String,
        ) -> anyhow::Result<()> {
            std::fs::create_dir_all(&service.task_prompts_root).with_context(|| {
                format!(
                    "failed to create task prompts dir {}",
                    service.task_prompts_root.display()
                )
            })?;

            let path = service.system_prompt_template_path(kind);
            let nanos = unix_epoch_nanos_now();
            let pid = std::process::id();
            let tmp =
                service
                    .task_prompts_root
                    .join(format!(".{}.{}.{}.tmp", kind.as_key(), pid, nanos));

            let mut normalized = template;
            if !normalized.ends_with('\n') {
                normalized.push('\n');
            }

            std::fs::write(&tmp, normalized.as_bytes())
                .with_context(|| format!("failed to write {}", tmp.display()))?;

            if std::fs::rename(&tmp, &path).is_err() {
                if path.exists() {
                    let _ = std::fs::remove_file(&path);
                }
                std::fs::rename(&tmp, &path).with_context(|| {
                    format!(
                        "failed to replace system prompt template {}",
                        path.display()
                    )
                })?;
            }

            Ok(())
        }

        inner(self, kind, template).map_err(|e| format!("{e:#}"))
    }

    fn system_prompt_template_delete(&self, kind: SystemTaskKind) -> Result<(), String> {
        let path = self.system_prompt_template_path(kind);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!(
                "{:#}",
                anyhow!(err).context(format!("failed to remove {}", path.display()))
            )),
        }
    }

    fn task_suggest_branch_name(&self, draft: luban_domain::TaskDraft) -> Result<String, String> {
        task::task_suggest_branch_name(self, draft).map_err(|e| format!("{e:#}"))
    }

    fn task_suggest_thread_title(&self, input: String) -> Result<String, String> {
        task::task_suggest_thread_title(self, input).map_err(|e| format!("{e:#}"))
    }

    fn conversation_update_title_if_matches(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
        expected_current_title: String,
        new_title: String,
    ) -> Result<bool, String> {
        self.sqlite
            .update_conversation_title_if_matches(
                project_slug,
                workspace_name,
                thread_id,
                expected_current_title,
                new_title,
            )
            .map_err(|e| format!("{e:#}"))
    }

    fn codex_check(&self) -> Result<(), String> {
        let result: anyhow::Result<()> = {
            let codex = self.codex_executable();
            cli_check::check_cli_version(&codex, "codex")
        };

        result.map_err(|e| format!("{e:#}"))
    }

    fn codex_config_tree(&self) -> Result<Vec<CodexConfigEntry>, String> {
        let result: anyhow::Result<Vec<CodexConfigEntry>> = (|| {
            let root = resolve_codex_root()?;

            let entries = config_tree::read_optional_root_shallow_entries(
                &root,
                "failed to stat codex config root",
                "codex config root",
            )?;

            Ok(codex_entries_from_shallow(entries))
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn codex_config_list_dir(&self, path: String) -> Result<Vec<CodexConfigEntry>, String> {
        let result: anyhow::Result<Vec<CodexConfigEntry>> = (|| {
            let root = resolve_codex_root()?;

            if !root.exists() {
                return Ok(Vec::new());
            }

            let rel_path = config_path::parse_strict_relative_list_dir_path(&path)?;

            let abs = root.join(&rel_path);
            let meta = std::fs::metadata(&abs)
                .with_context(|| format!("failed to stat {}", abs.display()))?;
            if !meta.is_dir() {
                return Err(anyhow!("not a directory: {}", abs.display()));
            }

            let entries = config_tree::read_shallow_entries_in_dir(&abs, &rel_path)?;

            Ok(codex_entries_from_shallow(entries))
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn codex_config_read_file(&self, path: String) -> Result<String, String> {
        let result: anyhow::Result<String> = (|| {
            let root = resolve_codex_root()?;

            let rel_path = config_path::parse_strict_relative_file_path(&path)?;

            let abs = root.join(rel_path);
            config_file_io::read_small_utf8_file(&abs)
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn codex_config_write_file(&self, path: String, contents: String) -> Result<(), String> {
        let result: anyhow::Result<()> = (|| {
            let root = resolve_codex_root()?;

            let rel_path = config_path::parse_strict_relative_file_path(&path)?;

            let abs = root.join(rel_path);
            config_file_io::write_file_creating_parent_dirs(&abs, &contents)
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn amp_check(&self) -> Result<(), String> {
        let result: anyhow::Result<()> = {
            let amp = std::env::var_os("LUBAN_AMP_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("amp"));
            cli_check::check_cli_version(&amp, "amp")
        };

        result.map_err(|e| format!("{e:#}"))
    }

    fn amp_config_tree(&self) -> Result<Vec<luban_domain::AmpConfigEntry>, String> {
        let result: anyhow::Result<Vec<luban_domain::AmpConfigEntry>> = (|| {
            let root = resolve_amp_root()?;

            let entries = config_tree::read_optional_root_shallow_entries(
                &root,
                "failed to stat amp config root",
                "amp config root",
            )?;

            Ok(amp_entries_from_shallow(entries))
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn amp_config_list_dir(
        &self,
        path: String,
    ) -> Result<Vec<luban_domain::AmpConfigEntry>, String> {
        let result: anyhow::Result<Vec<luban_domain::AmpConfigEntry>> = (|| {
            let root = resolve_amp_root()?;

            if !root.exists() {
                return Ok(Vec::new());
            }

            let rel_path = config_path::parse_strict_relative_list_dir_path(&path)?;

            let abs = root.join(&rel_path);
            let meta = std::fs::metadata(&abs)
                .with_context(|| format!("failed to stat {}", abs.display()))?;
            if !meta.is_dir() {
                return Err(anyhow!("not a directory: {}", abs.display()));
            }

            let entries = config_tree::read_shallow_entries_in_dir(&abs, &rel_path)?;

            Ok(amp_entries_from_shallow(entries))
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn amp_config_read_file(&self, path: String) -> Result<String, String> {
        let result: anyhow::Result<String> = (|| {
            let root = resolve_amp_root()?;

            let rel_path = config_path::parse_strict_relative_file_path(&path)?;

            let abs = root.join(rel_path);
            config_file_io::read_small_utf8_file(&abs)
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn amp_config_write_file(&self, path: String, contents: String) -> Result<(), String> {
        let result: anyhow::Result<()> = (|| {
            let root = resolve_amp_root()?;

            let rel_path = config_path::parse_strict_relative_file_path(&path)?;

            let abs = root.join(rel_path);
            config_file_io::write_file_creating_parent_dirs(&abs, &contents)
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn claude_check(&self) -> Result<(), String> {
        let result: anyhow::Result<()> = {
            let claude = std::env::var_os(paths::LUBAN_CLAUDE_BIN_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("claude"));
            cli_check::check_cli_version(&claude, "claude")
        };

        result.map_err(|e| format!("{e:#}"))
    }

    fn claude_config_tree(&self) -> Result<Vec<ClaudeConfigEntry>, String> {
        let result: anyhow::Result<Vec<ClaudeConfigEntry>> = (|| {
            let root = resolve_claude_root()?;

            let entries = config_tree::read_optional_root_shallow_entries(
                &root,
                "failed to stat claude config root",
                "claude config root",
            )?;

            Ok(claude_entries_from_shallow(entries))
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn claude_config_list_dir(&self, path: String) -> Result<Vec<ClaudeConfigEntry>, String> {
        let result: anyhow::Result<Vec<ClaudeConfigEntry>> = (|| {
            let root = resolve_claude_root()?;

            if !root.exists() {
                return Ok(Vec::new());
            }

            let rel_path = config_path::parse_lenient_relative_list_dir_path(&path)?;

            let abs = root.join(&rel_path);
            let meta = std::fs::metadata(&abs)
                .with_context(|| format!("failed to stat {}", abs.display()))?;
            if !meta.is_dir() {
                return Err(anyhow!("not a directory: {}", abs.display()));
            }

            let entries = config_tree::read_shallow_entries_in_dir(&abs, &rel_path)?;

            Ok(claude_entries_from_shallow(entries))
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn claude_config_read_file(&self, path: String) -> Result<String, String> {
        let result: anyhow::Result<String> = (|| {
            let root = resolve_claude_root()?;

            let rel_path = config_path::parse_strict_relative_file_path(&path)?;

            let abs = root.join(rel_path);
            config_file_io::read_small_utf8_file(&abs)
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn claude_config_write_file(&self, path: String, contents: String) -> Result<(), String> {
        let result: anyhow::Result<()> = (|| {
            let root = resolve_claude_root()?;

            let rel_path = config_path::parse_strict_relative_file_path(&path)?;

            let abs = root.join(rel_path);
            config_file_io::write_file_creating_parent_dirs(&abs, &contents)
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn project_identity(&self, path: PathBuf) -> Result<luban_domain::ProjectIdentity, String> {
        let result: anyhow::Result<luban_domain::ProjectIdentity> = (|| {
            if !path.exists() {
                return Ok(luban_domain::ProjectIdentity {
                    root_path: path,
                    github_repo: None,
                    is_git: false,
                });
            }

            let root = self.repo_root(&path);
            let (root_path, is_git) = match root {
                Ok(root_path) => (root_path, true),
                Err(_) => (path, false),
            };

            let remote = if is_git {
                self.select_remote_best_effort(&root_path).unwrap_or(None)
            } else {
                None
            };
            let github_repo = if let Some(remote) = remote {
                let url = self
                    .run_git(&root_path, ["remote", "get-url", &remote])
                    .ok();
                url.and_then(|u| Self::github_repo_id_from_remote_url(&u))
            } else {
                None
            };

            Ok(luban_domain::ProjectIdentity {
                root_path,
                github_repo,
                is_git,
            })
        })();

        result.map_err(|e| format!("{e:#}"))
    }
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenCommand {
    program: &'static str,
    args: Vec<std::ffi::OsString>,
    label: &'static str,
}

#[cfg(target_os = "linux")]
fn linux_open_command(target: OpenTarget, worktree_path: &Path) -> anyhow::Result<OpenCommand> {
    let mut args = Vec::new();
    let (program, label) = match target {
        OpenTarget::Vscode => {
            args.push(worktree_path.as_os_str().to_os_string());
            ("code", "code")
        }
        OpenTarget::Zed => {
            args.push(worktree_path.as_os_str().to_os_string());
            ("zed", "zed")
        }
        OpenTarget::Finder => {
            args.push(worktree_path.as_os_str().to_os_string());
            ("xdg-open", "xdg-open")
        }
        OpenTarget::Cursor => {
            return Err(anyhow!("opening Cursor is not supported on Linux"));
        }
        OpenTarget::Ghostty => {
            return Err(anyhow!("opening Ghostty is not supported on Linux"));
        }
    };

    Ok(OpenCommand {
        program,
        args,
        label,
    })
}

impl GitWorkspaceService {
    fn open_url(&self, url: &str) -> anyhow::Result<()> {
        #[cfg(target_os = "macos")]
        {
            let status = Command::new("open")
                .arg(url)
                .status()
                .context("failed to spawn 'open'")?;
            if !status.success() {
                return Err(anyhow!("'open' exited with status: {status}"));
            }
            Ok(())
        }

        #[cfg(target_os = "windows")]
        {
            let status = Command::new("cmd")
                .args(["/C", "start", "", url])
                .status()
                .context("failed to spawn 'cmd /C start'")?;
            if !status.success() {
                return Err(anyhow!("'cmd /C start' exited with status: {status}"));
            }
            Ok(())
        }

        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        {
            let status = Command::new("xdg-open")
                .arg(url)
                .status()
                .context("failed to spawn 'xdg-open'")?;
            if !status.success() {
                return Err(anyhow!("'xdg-open' exited with status: {status}"));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::prompt::PromptAttachment;
    use super::pull_request::is_merge_ready;
    use super::*;
    use luban_domain::{PersistedProject, PersistedWorkspace, WorkspaceStatus};
    use std::path::{Path, PathBuf};

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner())
    }

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
    fn amp_prompt_includes_image_paths() {
        let prompt = "Hello";
        let attachments = vec![
            PromptAttachment {
                kind: AttachmentKind::Image,
                name: "a.png".to_owned(),
                path: PathBuf::from("images/a.png"),
            },
            PromptAttachment {
                kind: AttachmentKind::File,
                name: "b.bin".to_owned(),
                path: PathBuf::from("/tmp/b.bin"),
            },
        ];
        let formatted = format_amp_prompt(prompt, &attachments);
        assert!(formatted.starts_with("Hello\n\nAttached files:\n"));
        assert!(formatted.contains("- a.png: @images/a.png\n"));
        assert!(formatted.contains("- b.bin: @/tmp/b.bin\n"));
    }

    #[test]
    fn codex_prompt_includes_attachment_paths_without_amp_marker() {
        let prompt = "Hello";
        let attachments = vec![
            PromptAttachment {
                kind: AttachmentKind::Text,
                name: "notes.txt".to_owned(),
                path: PathBuf::from("/tmp/notes.txt"),
            },
            PromptAttachment {
                kind: AttachmentKind::Image,
                name: "image.png".to_owned(),
                path: PathBuf::from("/tmp/image.png"),
            },
        ];
        let formatted = format_codex_prompt(prompt, &attachments);
        assert!(formatted.starts_with("Hello\n\nAttached files:\n"));
        assert!(formatted.contains("- notes.txt: /tmp/notes.txt\n"));
        assert!(formatted.contains("- image.png: /tmp/image.png\n"));
        assert!(!formatted.contains("@/tmp/notes.txt"));
        assert!(!formatted.contains("@/tmp/image.png"));
    }

    #[test]
    fn gh_pr_check_bucket_ci_state_mapping() {
        assert_eq!(
            pull_request_ci_state_from_check_buckets(["pass"]),
            Some(PullRequestCiState::Success)
        );
        assert_eq!(
            pull_request_ci_state_from_check_buckets(["skipping"]),
            Some(PullRequestCiState::Success)
        );
        assert_eq!(
            pull_request_ci_state_from_check_buckets(["pending", "pass"]),
            Some(PullRequestCiState::Pending)
        );
        assert_eq!(
            pull_request_ci_state_from_check_buckets(["cancel"]),
            Some(PullRequestCiState::Failure)
        );
        assert_eq!(
            pull_request_ci_state_from_check_buckets(["fail", "pending", "pass"]),
            Some(PullRequestCiState::Failure)
        );
        assert_eq!(pull_request_ci_state_from_check_buckets(["unknown"]), None);
    }

    #[test]
    fn gh_pr_merge_ready_logic_is_stable() {
        assert!(is_merge_ready(
            PullRequestState::Open,
            false,
            "CLEAN",
            "APPROVED",
            Some(PullRequestCiState::Success),
        ));
        assert!(is_merge_ready(
            PullRequestState::Open,
            false,
            "HAS_HOOKS",
            "APPROVED",
            Some(PullRequestCiState::Success),
        ));

        assert!(!is_merge_ready(
            PullRequestState::Merged,
            false,
            "CLEAN",
            "APPROVED",
            Some(PullRequestCiState::Success),
        ));
        assert!(!is_merge_ready(
            PullRequestState::Open,
            true,
            "CLEAN",
            "APPROVED",
            Some(PullRequestCiState::Success),
        ));
        assert!(!is_merge_ready(
            PullRequestState::Open,
            false,
            "DIRTY",
            "APPROVED",
            Some(PullRequestCiState::Success),
        ));
        assert!(!is_merge_ready(
            PullRequestState::Open,
            false,
            "CLEAN",
            "CHANGES_REQUESTED",
            Some(PullRequestCiState::Success),
        ));
        assert!(!is_merge_ready(
            PullRequestState::Open,
            false,
            "CLEAN",
            "APPROVED",
            Some(PullRequestCiState::Pending),
        ));
    }

    #[test]
    fn task_prompt_templates_roundtrip_via_files() {
        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let root = std::env::temp_dir().join(format!(
            "luban-task-prompts-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp dir should be created");
        unsafe {
            std::env::set_var(paths::LUBAN_ROOT_ENV, root.as_os_str());
        }

        let service = GitWorkspaceService::new().expect("service should init");
        service
            .task_prompt_template_store(TaskIntentKind::Fix, "hello".to_owned())
            .expect("store should succeed");

        let loaded = service
            .task_prompt_templates_load()
            .expect("load should succeed");
        assert_eq!(
            loaded.get(&TaskIntentKind::Fix).map(String::as_str),
            Some("hello")
        );

        service
            .task_prompt_template_delete(TaskIntentKind::Fix)
            .expect("delete should succeed");
        let loaded = service
            .task_prompt_templates_load()
            .expect("load should succeed");
        assert!(!loaded.contains_key(&TaskIntentKind::Fix));

        unsafe {
            std::env::remove_var(paths::LUBAN_ROOT_ENV);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn codex_config_tree_is_shallow_and_codex_config_list_dir_pages() {
        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let root = std::env::temp_dir().join(format!(
            "luban-codex-config-tree-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp dir should be created");

        let cache_dir = root.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache dir should be created");
        for i in 0..3000 {
            std::fs::write(cache_dir.join(format!("blob-{i}.bin")), b"x")
                .expect("write cache file");
        }

        std::fs::write(root.join("config.toml"), "model = \"gpt-5.1-codex-mini\"")
            .expect("write config.toml");
        let prompts_dir = root.join("prompts");
        std::fs::create_dir_all(&prompts_dir).expect("prompts dir should be created");
        std::fs::write(prompts_dir.join("hello.md"), "# Hello\n").expect("write prompt");

        let prev = std::env::var_os(paths::LUBAN_CODEX_ROOT_ENV);
        unsafe {
            std::env::set_var(paths::LUBAN_CODEX_ROOT_ENV, &root);
        }

        let base_dir =
            std::env::temp_dir().join(format!("luban-services-{}-{}", std::process::id(), unique));
        std::fs::create_dir_all(&base_dir).expect("luban root should exist");
        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let tree = ProjectWorkspaceService::codex_config_tree(&service)
            .expect("codex_config_tree should succeed");

        let entries = ProjectWorkspaceService::codex_config_list_dir(&service, "cache".to_owned())
            .expect("codex_config_list_dir should succeed");
        assert!(
            entries.len() >= 3000,
            "expected list_dir to include all files (got {})",
            entries.len()
        );

        if let Some(value) = prev {
            unsafe {
                std::env::set_var(paths::LUBAN_CODEX_ROOT_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(paths::LUBAN_CODEX_ROOT_ENV);
            }
        }

        let mut paths = Vec::new();
        fn collect(out: &mut Vec<String>, entries: &[CodexConfigEntry]) {
            for entry in entries {
                out.push(entry.path.clone());
                collect(out, &entry.children);
            }
        }
        collect(&mut paths, &tree);

        assert!(
            paths.iter().any(|p| p == "config.toml"),
            "tree should include config.toml"
        );
        assert!(
            paths.iter().any(|p| p == "prompts"),
            "tree should include prompts dir"
        );
        assert!(
            paths.iter().any(|p| p == "cache"),
            "tree should include cache directory (shallow listing)"
        );

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[cfg(unix)]
    fn codex_config_tree_includes_symlink_files() {
        use std::os::unix::fs::symlink;

        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let root = std::env::temp_dir().join(format!(
            "luban-codex-config-symlink-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp dir should be created");

        std::fs::write(root.join("AGENTS-target.md"), "hello").expect("write target");
        symlink("AGENTS-target.md", root.join("AGENTS.md")).expect("symlink should be created");

        let prev = std::env::var_os(paths::LUBAN_CODEX_ROOT_ENV);
        unsafe {
            std::env::set_var(paths::LUBAN_CODEX_ROOT_ENV, &root);
        }

        let base_dir =
            std::env::temp_dir().join(format!("luban-services-{}-{}", std::process::id(), unique));
        std::fs::create_dir_all(&base_dir).expect("luban root should exist");
        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let tree = ProjectWorkspaceService::codex_config_tree(&service)
            .expect("codex_config_tree should succeed");

        let mut paths = Vec::new();
        fn collect(out: &mut Vec<String>, entries: &[CodexConfigEntry]) {
            for entry in entries {
                out.push(entry.path.clone());
                collect(out, &entry.children);
            }
        }
        collect(&mut paths, &tree);
        assert!(
            paths.iter().any(|p| p == "AGENTS.md"),
            "tree should include AGENTS.md symlink"
        );

        let contents =
            ProjectWorkspaceService::codex_config_read_file(&service, "AGENTS.md".to_owned())
                .expect("read should succeed");
        assert_eq!(contents, "hello");

        if let Some(value) = prev {
            unsafe {
                std::env::set_var(paths::LUBAN_CODEX_ROOT_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(paths::LUBAN_CODEX_ROOT_ENV);
            }
        }

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn amp_config_tree_is_shallow_and_amp_config_list_dir_supports_root_listing() {
        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let root = std::env::temp_dir().join(format!(
            "luban-amp-config-tree-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp dir should be created");

        let cache_dir = root.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache dir should be created");
        for i in 0..3000 {
            std::fs::write(cache_dir.join(format!("blob-{i}.bin")), b"x")
                .expect("write cache file");
        }

        std::fs::write(root.join("config.yaml"), "model: claude\n").expect("write config.yaml");
        let prompts_dir = root.join("prompts");
        std::fs::create_dir_all(&prompts_dir).expect("prompts dir should be created");
        std::fs::write(prompts_dir.join("hello.md"), "# Hello\n").expect("write prompt");

        let prev = std::env::var_os(paths::LUBAN_AMP_ROOT_ENV);
        unsafe {
            std::env::set_var(paths::LUBAN_AMP_ROOT_ENV, &root);
        }

        let base_dir =
            std::env::temp_dir().join(format!("luban-services-{}-{}", std::process::id(), unique));
        std::fs::create_dir_all(&base_dir).expect("luban root should exist");
        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let tree = ProjectWorkspaceService::amp_config_tree(&service)
            .expect("amp_config_tree should succeed");

        let entries = ProjectWorkspaceService::amp_config_list_dir(&service, "cache".to_owned())
            .expect("amp_config_list_dir should succeed");
        assert!(
            entries.len() >= 3000,
            "expected list_dir to include all files (got {})",
            entries.len()
        );

        let root_entries = ProjectWorkspaceService::amp_config_list_dir(&service, "".to_owned())
            .expect("amp_config_list_dir root should succeed");
        assert!(
            root_entries.iter().any(|e| e.path == "config.yaml"),
            "root list_dir should include config.yaml"
        );

        ProjectWorkspaceService::amp_config_write_file(
            &service,
            "nested/example.txt".to_owned(),
            "hello".to_owned(),
        )
        .expect("amp_config_write_file should succeed");
        let loaded = ProjectWorkspaceService::amp_config_read_file(
            &service,
            "nested/example.txt".to_owned(),
        )
        .expect("amp_config_read_file should succeed");
        assert_eq!(loaded, "hello");

        if let Some(value) = prev {
            unsafe {
                std::env::set_var(paths::LUBAN_AMP_ROOT_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(paths::LUBAN_AMP_ROOT_ENV);
            }
        }

        let mut paths = Vec::new();
        fn collect(out: &mut Vec<String>, entries: &[luban_domain::AmpConfigEntry]) {
            for entry in entries {
                out.push(entry.path.clone());
                collect(out, &entry.children);
            }
        }
        collect(&mut paths, &tree);

        assert!(
            paths.iter().any(|p| p == "config.yaml"),
            "tree should include config.yaml"
        );
        assert!(
            paths.iter().any(|p| p == "prompts"),
            "tree should include prompts dir"
        );
        assert!(
            paths.iter().any(|p| p == "cache"),
            "tree should include cache directory (shallow listing)"
        );

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[cfg(unix)]
    fn amp_config_tree_includes_symlink_files() {
        use std::os::unix::fs::symlink;

        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let root = std::env::temp_dir().join(format!(
            "luban-amp-config-symlink-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp dir should be created");

        std::fs::write(root.join("config-target.yaml"), "model: amp\n").expect("write target");
        symlink("config-target.yaml", root.join("config.yaml")).expect("symlink should be created");

        let prev = std::env::var_os(paths::LUBAN_AMP_ROOT_ENV);
        unsafe {
            std::env::set_var(paths::LUBAN_AMP_ROOT_ENV, &root);
        }

        let base_dir =
            std::env::temp_dir().join(format!("luban-services-{}-{}", std::process::id(), unique));
        std::fs::create_dir_all(&base_dir).expect("luban root should exist");
        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let tree = ProjectWorkspaceService::amp_config_tree(&service)
            .expect("amp_config_tree should succeed");

        let mut paths = Vec::new();
        fn collect(out: &mut Vec<String>, entries: &[luban_domain::AmpConfigEntry]) {
            for entry in entries {
                out.push(entry.path.clone());
                collect(out, &entry.children);
            }
        }
        collect(&mut paths, &tree);
        assert!(
            paths.iter().any(|p| p == "config.yaml"),
            "tree should include config.yaml symlink"
        );

        let contents =
            ProjectWorkspaceService::amp_config_read_file(&service, "config.yaml".to_owned())
                .expect("read should succeed");
        assert_eq!(contents, "model: amp\n");

        if let Some(value) = prev {
            unsafe {
                std::env::set_var(paths::LUBAN_AMP_ROOT_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(paths::LUBAN_AMP_ROOT_ENV);
            }
        }

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn claude_config_tree_is_shallow_and_claude_config_list_dir_pages() {
        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let root = std::env::temp_dir().join(format!(
            "luban-claude-config-tree-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp dir should be created");

        let cache_dir = root.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache dir should be created");
        for i in 0..512 {
            std::fs::write(cache_dir.join(format!("blob-{i}.bin")), b"x")
                .expect("write cache file");
        }

        std::fs::write(
            root.join("settings.json"),
            "{ \"permissions\": { \"allow\": [] } }\n",
        )
        .expect("write settings.json");

        let prev = std::env::var_os(paths::LUBAN_CLAUDE_ROOT_ENV);
        unsafe {
            std::env::set_var(paths::LUBAN_CLAUDE_ROOT_ENV, &root);
        }

        let base_dir =
            std::env::temp_dir().join(format!("luban-services-{}-{}", std::process::id(), unique));
        std::fs::create_dir_all(&base_dir).expect("luban root should exist");
        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let tree = ProjectWorkspaceService::claude_config_tree(&service)
            .expect("claude_config_tree should succeed");

        let entries = ProjectWorkspaceService::claude_config_list_dir(&service, "cache".to_owned())
            .expect("claude_config_list_dir should succeed");
        assert!(
            entries.len() >= 512,
            "expected list_dir to include all files (got {})",
            entries.len()
        );

        if let Some(value) = prev {
            unsafe {
                std::env::set_var(paths::LUBAN_CLAUDE_ROOT_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(paths::LUBAN_CLAUDE_ROOT_ENV);
            }
        }

        let mut paths = Vec::new();
        fn collect(out: &mut Vec<String>, entries: &[ClaudeConfigEntry]) {
            for entry in entries {
                out.push(entry.path.clone());
                collect(out, &entry.children);
            }
        }
        collect(&mut paths, &tree);

        assert!(
            paths.iter().any(|p| p == "settings.json"),
            "tree should include settings.json"
        );
        assert!(
            paths.iter().any(|p| p == "cache"),
            "tree should include cache directory (shallow listing)"
        );

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[cfg(unix)]
    fn claude_config_tree_includes_symlink_files() {
        use std::os::unix::fs::symlink;

        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let root = std::env::temp_dir().join(format!(
            "luban-claude-config-symlink-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp dir should be created");

        std::fs::write(root.join("settings-target.json"), "{ \"ok\": true }\n")
            .expect("write target");
        symlink("settings-target.json", root.join("settings.json"))
            .expect("symlink should be created");

        let prev = std::env::var_os(paths::LUBAN_CLAUDE_ROOT_ENV);
        unsafe {
            std::env::set_var(paths::LUBAN_CLAUDE_ROOT_ENV, &root);
        }

        let base_dir =
            std::env::temp_dir().join(format!("luban-services-{}-{}", std::process::id(), unique));
        std::fs::create_dir_all(&base_dir).expect("luban root should exist");
        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let tree = ProjectWorkspaceService::claude_config_tree(&service)
            .expect("claude_config_tree should succeed");

        let mut paths = Vec::new();
        fn collect(out: &mut Vec<String>, entries: &[ClaudeConfigEntry]) {
            for entry in entries {
                out.push(entry.path.clone());
                collect(out, &entry.children);
            }
        }
        collect(&mut paths, &tree);
        assert!(
            paths.iter().any(|p| p == "settings.json"),
            "tree should include settings.json symlink"
        );

        let contents =
            ProjectWorkspaceService::claude_config_read_file(&service, "settings.json".to_owned())
                .expect("read should succeed");
        assert_eq!(contents, "{ \"ok\": true }\n");

        if let Some(value) = prev {
            unsafe {
                std::env::set_var(paths::LUBAN_CLAUDE_ROOT_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(paths::LUBAN_CLAUDE_ROOT_ENV);
            }
        }

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn amp_mode_is_detected_from_config_files() {
        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let root = std::env::temp_dir().join(format!(
            "luban-amp-mode-config-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp dir should be created");

        std::fs::write(root.join("config.toml"), "mode = \"rush\"\n").expect("write config");
        assert_eq!(
            detect_amp_mode_from_config_root(&root).as_deref(),
            Some("rush")
        );

        std::fs::write(root.join("config.toml"), "mode = \"smart\"\n").expect("write config");
        assert_eq!(
            detect_amp_mode_from_config_root(&root).as_deref(),
            Some("smart")
        );

        std::fs::write(root.join("config.yaml"), "mode: rush\n").expect("write config");
        assert_eq!(
            detect_amp_mode_from_config_root(&root).as_deref(),
            Some("smart"),
            "config.toml takes precedence when present"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn system_prompt_templates_roundtrip_via_files() {
        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let root = std::env::temp_dir().join(format!(
            "luban-system-prompts-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp dir should be created");
        unsafe {
            std::env::set_var(paths::LUBAN_ROOT_ENV, root.as_os_str());
        }

        let service = GitWorkspaceService::new().expect("service should init");
        service
            .system_prompt_template_store(SystemTaskKind::InferType, "hello".to_owned())
            .expect("store should succeed");

        let loaded = service
            .system_prompt_templates_load()
            .expect("load should succeed");
        assert_eq!(
            loaded.get(&SystemTaskKind::InferType).map(String::as_str),
            Some("hello")
        );

        service
            .system_prompt_template_delete(SystemTaskKind::InferType)
            .expect("delete should succeed");
        let loaded = service
            .system_prompt_templates_load()
            .expect("load should succeed");
        assert!(!loaded.contains_key(&SystemTaskKind::InferType));

        unsafe {
            std::env::remove_var(paths::LUBAN_ROOT_ENV);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn codex_runner_reports_missing_executable() {
        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
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
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
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
                    sandbox_mode: None,
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
    fn codex_turn_errors_when_no_final_message_is_produced() {
        let _guard = lock_env();

        let unique = unix_epoch_nanos_now();
        let base_dir = std::env::temp_dir().join(format!(
            "luban-no-final-message-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&base_dir).expect("temp dir should be created");

        let fake_codex = base_dir.join("fake-codex");
        std::fs::write(
            &fake_codex,
            [
                "#!/bin/sh",
                "cat >/dev/null &",
                "stdin_pid=$!",
                "echo '{\"type\":\"turn.started\"}'",
                "echo '{\"type\":\"item.completed\",\"item\":{\"type\":\"command_execution\",\"id\":\"item_0\",\"command\":\"echo hi\",\"aggregated_output\":\"\",\"exit_code\":0,\"status\":\"completed\"}}'",
                "echo '{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":0,\"cached_input_tokens\":0,\"output_tokens\":0}}'",
                "wait \"$stdin_pid\"",
                "exit 0",
                "",
            ]
            .join("\n"),
        )
        .expect("fake codex should be written");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&fake_codex)
                .expect("fake codex should exist")
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&fake_codex, perms).expect("fake codex should be executable");
        }

        unsafe {
            std::env::set_var(paths::LUBAN_CODEX_BIN_ENV, fake_codex.as_os_str());
        }

        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let err = service
            .run_agent_turn_streamed(
                RunAgentTurnRequest {
                    project_slug: "p".to_owned(),
                    workspace_name: "w".to_owned(),
                    worktree_path: base_dir.clone(),
                    thread_local_id: 1,
                    thread_id: None,
                    prompt: "Hello".to_owned(),
                    attachments: Vec::new(),
                    runner: luban_domain::AgentRunnerKind::Codex,
                    amp_mode: None,
                    model: None,
                    model_reasoning_effort: None,
                },
                Arc::new(AtomicBool::new(false)),
                Arc::new(|_event| {}),
            )
            .expect_err("missing final message should be treated as an error");

        unsafe {
            std::env::remove_var(paths::LUBAN_CODEX_BIN_ENV);
        }

        assert!(
            err.contains("without a final message"),
            "unexpected error: {err}"
        );

        let snapshot = service
            .sqlite
            .load_conversation("p".to_owned(), "w".to_owned(), 1)
            .expect("conversation should be persisted");
        assert!(
            snapshot
                .entries
                .iter()
                .any(|e| matches!(e, ConversationEntry::TurnError { .. })),
            "expected TurnError entry to be persisted"
        );

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn tests_do_not_use_production_db_by_default() {
        let _guard = lock_env();

        let prev = std::env::var_os(paths::LUBAN_ROOT_ENV);
        unsafe {
            std::env::remove_var(paths::LUBAN_ROOT_ENV);
        }

        let root = resolve_luban_root().expect("test root should resolve");
        assert!(
            root.to_string_lossy().contains("luban-test-"),
            "expected test root under temp dir, got {}",
            root.display()
        );

        if let Some(prev) = prev {
            unsafe {
                std::env::set_var(paths::LUBAN_ROOT_ENV, prev);
            }
        }
    }

    #[test]
    fn luban_root_env_overrides_default_root() {
        let _guard = lock_env();

        let prev = std::env::var_os(paths::LUBAN_ROOT_ENV);
        let unique = unix_epoch_nanos_now();
        let base_dir =
            std::env::temp_dir().join(format!("luban-root-env-{}-{}", std::process::id(), unique));
        std::fs::create_dir_all(&base_dir).expect("temp dir should be created");

        unsafe {
            std::env::set_var(paths::LUBAN_ROOT_ENV, base_dir.as_os_str());
        }

        let service = GitWorkspaceService::new_with_options(SqliteStoreOptions::default())
            .expect("service should init");
        service
            .sqlite
            .load_app_state()
            .expect("sqlite queries should work");

        let expected_db = paths::sqlite_path(&base_dir);
        assert!(
            expected_db.exists(),
            "expected sqlite db at {}, but it was not created",
            expected_db.display()
        );

        drop(service);
        if let Some(prev) = prev {
            unsafe {
                std::env::set_var(paths::LUBAN_ROOT_ENV, prev);
            }
        } else {
            unsafe {
                std::env::remove_var(paths::LUBAN_ROOT_ENV);
            }
        }
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
        let unique = unix_epoch_nanos_now();
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
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
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

    fn git_rev_parse(repo_path: &Path, rev: &str) -> String {
        let out = run_git(repo_path, &["rev-parse", "--verify", rev]);
        assert!(
            out.status.success(),
            "git rev-parse {rev} failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    #[test]
    fn load_app_state_archives_missing_worktrees() {
        let unique = unix_epoch_nanos_now();
        let base_dir = std::env::temp_dir().join(format!(
            "luban-load-archives-missing-worktree-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir_all(&base_dir).expect("temp dir should be created");

        let repo_path = base_dir.join("repo");
        std::fs::create_dir_all(&repo_path).expect("repo dir should be created");

        assert_git_success(&repo_path, &["init"]);
        assert_git_success(&repo_path, &["config", "user.name", "Test User"]);
        assert_git_success(&repo_path, &["config", "user.email", "test@example.com"]);
        assert_git_success(&repo_path, &["checkout", "-b", "main"]);

        std::fs::write(repo_path.join("README.md"), "init\n").expect("write should succeed");
        assert_git_success(&repo_path, &["add", "."]);
        assert_git_success(&repo_path, &["commit", "-m", "init"]);

        let worktree_path = base_dir.join("worktree");
        let branch_name = format!("luban/review-lance-{}", unique % 10_000);
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
        assert!(worktree_path.exists(), "worktree path should exist");

        assert_git_success(
            &repo_path,
            &[
                "worktree",
                "remove",
                "--force",
                worktree_path
                    .to_str()
                    .expect("worktree path should be utf-8"),
            ],
        );
        assert!(!worktree_path.exists(), "worktree path should be removed");

        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                name: "repo".to_owned(),
                path: repo_path.clone(),
                slug: "repo".to_owned(),
                is_git: true,
                expanded: true,
                workspaces: vec![PersistedWorkspace {
                    id: 1,
                    workspace_name: "review-lance-5713".to_owned(),
                    branch_name: branch_name.clone(),
                    worktree_path: worktree_path.clone(),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                }],
            }],
            sidebar_width: None,
            terminal_pane_width: None,
            global_zoom_percent: None,
            appearance_theme: None,
            appearance_ui_font: None,
            appearance_chat_font: None,
            appearance_code_font: None,
            appearance_terminal_font: None,
            agent_default_model_id: None,
            agent_default_thinking_effort: None,
            agent_default_runner: None,
            agent_amp_mode: None,
            agent_codex_enabled: Some(true),
            agent_amp_enabled: Some(true),
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            sidebar_worktree_order: std::collections::HashMap::new(),
            workspace_active_thread_id: std::collections::HashMap::new(),
            workspace_open_tabs: std::collections::HashMap::new(),
            workspace_archived_tabs: std::collections::HashMap::new(),
            workspace_next_thread_id: std::collections::HashMap::new(),
            workspace_chat_scroll_y10: std::collections::HashMap::new(),
            workspace_chat_scroll_anchor: std::collections::HashMap::new(),
            workspace_unread_completions: std::collections::HashMap::new(),
            task_prompt_templates: std::collections::HashMap::new(),
        };

        service
            .sqlite
            .save_app_state(snapshot)
            .expect("sqlite save should succeed");

        let loaded = service
            .load_app_state_internal()
            .expect("load_app_state_internal should succeed");
        assert_eq!(
            loaded.projects[0].workspaces[0].status,
            WorkspaceStatus::Archived
        );

        let persisted = service
            .sqlite
            .load_app_state()
            .expect("sqlite load should succeed");
        assert_eq!(
            persisted.projects[0].workspaces[0].status,
            WorkspaceStatus::Archived
        );

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn create_workspace_bases_on_origin_main_and_does_not_track_upstream() {
        let unique = unix_epoch_nanos_now();
        let base_dir = std::env::temp_dir().join(format!(
            "luban-create-workspace-origin-main-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir_all(&base_dir).expect("temp dir should be created");

        let remote_dir = base_dir.join("remote.git");
        std::fs::create_dir_all(&remote_dir).expect("remote dir should be created");
        assert_git_success(&remote_dir, &["init", "--bare"]);
        assert_git_success(&remote_dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);

        let project_dir = base_dir.join("repo");
        std::fs::create_dir_all(&project_dir).expect("repo dir should be created");
        assert_git_success(&project_dir, &["init"]);
        assert_git_success(&project_dir, &["config", "user.name", "Test User"]);
        assert_git_success(&project_dir, &["config", "user.email", "test@example.com"]);
        assert_git_success(&project_dir, &["checkout", "-b", "main"]);

        std::fs::write(project_dir.join("README.md"), "init\n").expect("write should succeed");
        assert_git_success(&project_dir, &["add", "."]);
        assert_git_success(&project_dir, &["commit", "-m", "init"]);
        assert_git_success(
            &project_dir,
            &[
                "remote",
                "add",
                "origin",
                remote_dir.to_str().expect("remote path should be utf-8"),
            ],
        );
        assert_git_success(&project_dir, &["push", "-u", "origin", "main"]);
        assert_git_success(&project_dir, &["fetch", "--prune", "origin", "main"]);

        let upstream_clone = base_dir.join("upstream");
        assert_git_success(
            &base_dir,
            &[
                "clone",
                remote_dir.to_str().expect("remote path should be utf-8"),
                upstream_clone
                    .to_str()
                    .expect("upstream clone path should be utf-8"),
            ],
        );
        assert_git_success(&upstream_clone, &["config", "user.name", "Upstream User"]);
        assert_git_success(
            &upstream_clone,
            &["config", "user.email", "upstream@example.com"],
        );
        std::fs::write(upstream_clone.join("CHANGELOG.md"), "upstream\n")
            .expect("write should succeed");
        assert_git_success(&upstream_clone, &["add", "."]);
        assert_git_success(&upstream_clone, &["commit", "-m", "upstream"]);
        assert_git_success(&upstream_clone, &["push", "origin", "main"]);
        let upstream_head = git_rev_parse(&upstream_clone, "HEAD^{commit}");

        std::fs::write(project_dir.join("LOCAL.md"), "local only\n").expect("write should succeed");
        assert_git_success(&project_dir, &["add", "."]);
        assert_git_success(&project_dir, &["commit", "-m", "local"]);

        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let created = ProjectWorkspaceService::create_workspace(
            &service,
            project_dir.clone(),
            "proj".to_owned(),
            None,
        )
        .expect("create_workspace should succeed");

        let head = git_rev_parse(&created.worktree_path, "HEAD^{commit}");
        assert_eq!(
            head, upstream_head,
            "expected workspace to be created from origin/main (after fetch)"
        );

        let upstream_config_key = format!("branch.{}.remote", created.branch_name);
        let config = Command::new("git")
            .args(["config", "--get", &upstream_config_key])
            .current_dir(&project_dir)
            .output()
            .expect("git config should spawn");
        assert!(
            !config.status.success(),
            "expected branch to not track upstream, but {} is set to {:?}",
            upstream_config_key,
            String::from_utf8_lossy(&config.stdout).trim()
        );

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    fn stored_blob_path(
        service: &GitWorkspaceService,
        project_slug: &str,
        workspace_name: &str,
        attachment: &AttachmentRef,
    ) -> PathBuf {
        service
            .context_blobs_dir(project_slug, workspace_name)
            .join(format!("{}.{}", attachment.id, attachment.extension))
    }

    #[test]
    fn context_files_are_content_addressed_and_preserve_display_name() {
        let unique = unix_epoch_nanos_now();
        let base_dir = std::env::temp_dir().join(format!(
            "luban-context-file-name-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir_all(&base_dir).expect("temp dir should be created");

        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let source = base_dir.join("abc.png");
        let bytes = b"not-a-real-png";
        std::fs::write(&source, bytes).expect("write should succeed");

        let stored = ProjectWorkspaceService::store_context_file(
            &service,
            "proj".to_owned(),
            "main".to_owned(),
            source,
        )
        .expect("store_context_file should succeed");

        assert_eq!(stored.name, "abc.png");
        assert_eq!(stored.extension, "png");
        assert_eq!(stored.id, blake3::hash(bytes).to_hex().to_string());
        assert!(
            stored_blob_path(&service, "proj", "main", &stored).exists(),
            "stored blob should exist"
        );

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn context_images_are_content_addressed() {
        let unique = unix_epoch_nanos_now();
        let base_dir = std::env::temp_dir().join(format!(
            "luban-context-image-name-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir_all(&base_dir).expect("temp dir should be created");

        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let stored = ProjectWorkspaceService::store_context_image(
            &service,
            "proj".to_owned(),
            "main".to_owned(),
            ContextImage {
                extension: "png".to_owned(),
                bytes: b"not-a-real-png".to_vec(),
            },
        )
        .expect("store_context_image should succeed");

        assert_eq!(stored.name, "image.png");
        assert_eq!(stored.extension, "png");
        assert_eq!(
            stored.id,
            blake3::hash(b"not-a-real-png").to_hex().to_string()
        );
        assert!(
            stored_blob_path(&service, "proj", "main", &stored).exists(),
            "stored blob should exist"
        );

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn context_images_store_thumbnail_alongside_original() {
        let unique = unix_epoch_nanos_now();
        let base_dir = std::env::temp_dir().join(format!(
            "luban-context-image-thumb-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir_all(&base_dir).expect("temp dir should be created");

        let sqlite =
            SqliteStore::new(paths::sqlite_path(&base_dir)).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: paths::worktrees_root(&base_dir),
            conversations_root: paths::conversations_root(&base_dir),
            task_prompts_root: paths::task_prompts_root(&base_dir),
            sqlite,
            claude_processes: Mutex::new(HashMap::new()),
        };

        let img = image::RgbImage::from_fn(1200, 800, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        });
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode png");

        let stored = ProjectWorkspaceService::store_context_image(
            &service,
            "proj".to_owned(),
            "main".to_owned(),
            ContextImage {
                extension: "png".to_owned(),
                bytes: png,
            },
        )
        .expect("store_context_image should succeed");

        let stored_path = stored_blob_path(&service, "proj", "main", &stored);
        let thumb = stored_path.with_file_name(format!("{}-thumb.png", stored.id));

        assert!(stored_path.exists(), "stored path should exist");
        assert!(thumb.exists(), "thumbnail path should exist");

        let thumb_bytes = std::fs::read(&thumb).expect("read thumbnail");
        let thumb_img = image::load_from_memory(&thumb_bytes).expect("decode thumbnail");
        assert!(
            thumb_img.width() <= 360 && thumb_img.height() <= 220,
            "expected thumbnail to be constrained: {}x{}",
            thumb_img.width(),
            thumb_img.height()
        );

        let stored_len = std::fs::metadata(&stored_path).expect("stat stored").len();
        let thumb_len = std::fs::metadata(&thumb).expect("stat thumb").len();
        assert!(
            thumb_len <= stored_len,
            "expected thumbnail to not exceed original size (thumb={thumb_len}, stored={stored_len})"
        );

        drop(service);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_open_command_uses_code_for_vscode() {
        let worktree_path = Path::new("/tmp/luban-worktree");
        let command =
            linux_open_command(OpenTarget::Vscode, worktree_path).expect("vscode open command");
        assert_eq!(command.program, "code");
        assert_eq!(command.label, "code");
        assert_eq!(
            command.args,
            vec![std::ffi::OsString::from("/tmp/luban-worktree")]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_open_command_uses_zed_for_zed() {
        let worktree_path = Path::new("/tmp/luban-worktree");
        let command = linux_open_command(OpenTarget::Zed, worktree_path).expect("zed open command");
        assert_eq!(command.program, "zed");
        assert_eq!(command.label, "zed");
        assert_eq!(
            command.args,
            vec![std::ffi::OsString::from("/tmp/luban-worktree")]
        );
    }
}
