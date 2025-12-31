use anyhow::{Context as _, anyhow};
use bip39::Language;
use luban_domain::{
    CodexThreadEvent, CodexThreadItem, ConversationEntry, ConversationSnapshot, PersistedAppState,
};
use rand::{Rng as _, rngs::OsRng};
use std::{
    collections::HashSet,
    ffi::OsStr,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::sqlite_store::SqliteStore;
use luban_ui::{CreatedWorkspace, ProjectWorkspaceService, RunAgentTurnRequest};

const SIDECAR_EVENT_PREFIX: &str = "__LUBAN_EVENT__ ";

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

enum SidecarStdoutLine {
    Event(Box<CodexThreadEvent>),
    Ignored { message: String },
    Noise { message: String },
}

fn parse_sidecar_stdout_line(line: &str) -> anyhow::Result<SidecarStdoutLine> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(SidecarStdoutLine::Noise {
            message: String::new(),
        });
    }

    let (payload, is_protocol) = if let Some(payload) = trimmed.strip_prefix(SIDECAR_EVENT_PREFIX) {
        (payload.trim_start(), true)
    } else {
        (trimmed, false)
    };

    let looks_like_json = payload.starts_with('{') || payload.starts_with('[');
    if !looks_like_json {
        return Ok(SidecarStdoutLine::Noise {
            message: payload.to_owned(),
        });
    }

    match serde_json::from_str::<CodexThreadEvent>(payload) {
        Ok(event) => Ok(SidecarStdoutLine::Event(Box::new(event))),
        Err(err) => {
            let value = match serde_json::from_str::<serde_json::Value>(payload) {
                Ok(value) => value,
                Err(_) if is_protocol => {
                    return Err(err).context("failed to parse sidecar protocol JSON");
                }
                Err(_) => {
                    return Ok(SidecarStdoutLine::Noise {
                        message: payload.to_owned(),
                    });
                }
            };

            let type_name = value
                .as_object()
                .and_then(|obj| obj.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("<missing type>");

            Ok(SidecarStdoutLine::Ignored {
                message: format!("ignored sidecar event: {type_name}"),
            })
        }
    }
}

#[derive(Clone)]
pub struct GitWorkspaceService {
    worktrees_root: PathBuf,
    conversations_root: PathBuf,
    agent_sidecar_dir: PathBuf,
    sqlite: SqliteStore,
}

impl GitWorkspaceService {
    pub fn new() -> anyhow::Result<Arc<Self>> {
        let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
        let mut luban_root = PathBuf::from(home);
        luban_root.push("luban");

        std::fs::create_dir_all(&luban_root)
            .with_context(|| format!("failed to create {}", luban_root.display()))?;

        let worktrees_root = luban_root.join("worktrees");
        let conversations_root = luban_root.join("conversations");
        let sqlite_path = luban_root.join("luban.db");
        let agent_sidecar_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("agent_sidecar");
        let sqlite = SqliteStore::new(sqlite_path).context("failed to init sqlite store")?;

        Ok(Arc::new(Self {
            worktrees_root,
            conversations_root,
            agent_sidecar_dir,
            sqlite,
        }))
    }

    fn run_git<I, S>(&self, repo_path: &Path, args: I) -> anyhow::Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .context("failed to spawn git")?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "git failed ({}):\nstdout:\n{}\nstderr:\n{}",
                output.status,
                stdout.trim(),
                stderr.trim()
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn select_remote(&self, repo_path: &Path) -> anyhow::Result<String> {
        let out = self.run_git(repo_path, ["remote"])?;
        let remotes = out
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        if remotes.contains(&"origin") {
            return Ok("origin".to_owned());
        }

        if remotes.len() == 1 {
            return Ok(remotes[0].to_owned());
        }

        Err(anyhow!("cannot select remote: found {:?}", remotes))
    }

    fn resolve_default_upstream_ref(
        &self,
        repo_path: &Path,
        remote: &str,
    ) -> anyhow::Result<String> {
        let head_ref = self
            .run_git(
                repo_path,
                [
                    "symbolic-ref",
                    "--quiet",
                    &format!("refs/remotes/{remote}/HEAD"),
                ],
            )
            .context("failed to resolve remote HEAD ref (missing refs/remotes/<remote>/HEAD?)")?;

        let prefix = format!("refs/remotes/{remote}/");
        let Some(branch) = head_ref.strip_prefix(&prefix) else {
            return Err(anyhow!("unexpected remote HEAD ref: {head_ref}"));
        };

        let verify_ref = format!("refs/remotes/{remote}/{branch}");
        self.run_git(repo_path, ["show-ref", "--verify", "--quiet", &verify_ref])
            .with_context(|| format!("remote default branch ref not found: {verify_ref}"))?;

        Ok(format!("{remote}/{branch}"))
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

    fn conversation_dir(&self, project_slug: &str, workspace_name: &str) -> PathBuf {
        let mut path = self.conversations_root.clone();
        path.push(project_slug);
        path.push(workspace_name);
        path
    }

    fn conversation_meta_path(&self, project_slug: &str, workspace_name: &str) -> PathBuf {
        self.conversation_dir(project_slug, workspace_name)
            .join("conversation.json")
    }

    fn conversation_events_path(&self, project_slug: &str, workspace_name: &str) -> PathBuf {
        self.conversation_dir(project_slug, workspace_name)
            .join("events.jsonl")
    }

    fn now_unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn ensure_sidecar_installed(&self) -> anyhow::Result<()> {
        let node_modules = self.agent_sidecar_dir.join("node_modules");
        if node_modules.is_dir() {
            return Ok(());
        }

        Err(anyhow!(
            "missing Codex sidecar dependencies: run 'npm install' in {}",
            self.agent_sidecar_dir.display()
        ))
    }

    fn read_conversation_meta_legacy(
        &self,
        project_slug: &str,
        workspace_name: &str,
    ) -> anyhow::Result<ConversationMeta> {
        let path = self.conversation_meta_path(project_slug, workspace_name);
        let content =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_slice(&content).context("failed to parse conversation meta")
    }

    fn load_conversation_legacy(
        &self,
        project_slug: &str,
        workspace_name: &str,
    ) -> anyhow::Result<Option<ConversationSnapshot>> {
        let meta_path = self.conversation_meta_path(project_slug, workspace_name);
        let events_path = self.conversation_events_path(project_slug, workspace_name);
        if !meta_path.exists() && !events_path.exists() {
            return Ok(None);
        }

        let meta = if meta_path.exists() {
            self.read_conversation_meta_legacy(project_slug, workspace_name)?
        } else {
            ConversationMeta {
                version: 1,
                thread_id: None,
                created_at: Self::now_unix_seconds(),
                updated_at: Self::now_unix_seconds(),
            }
        };

        if !events_path.exists() {
            return Ok(Some(ConversationSnapshot {
                thread_id: meta.thread_id,
                entries: Vec::new(),
            }));
        }

        let file = std::fs::File::open(&events_path)
            .with_context(|| format!("failed to open {}", events_path.display()))?;
        let reader = BufReader::new(file);

        let mut entries = Vec::new();
        for line in reader.lines() {
            let line = line.context("failed to read line")?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let entry: ConversationEntry =
                serde_json::from_str(trimmed).context("failed to parse entry")?;
            if matches!(entry, ConversationEntry::TurnUsage { .. }) {
                continue;
            }
            let is_duplicate = match (&entry, entries.last()) {
                (
                    ConversationEntry::CodexItem { item },
                    Some(ConversationEntry::CodexItem { item: prev }),
                ) => codex_item_id(item) == codex_item_id(prev),
                _ => false,
            };
            if !is_duplicate {
                entries.push(entry);
            }
        }

        Ok(Some(ConversationSnapshot {
            thread_id: meta.thread_id,
            entries,
        }))
    }

    fn load_app_state_internal(&self) -> anyhow::Result<PersistedAppState> {
        self.sqlite.load_app_state()
    }

    fn save_app_state_internal(&self, snapshot: PersistedAppState) -> anyhow::Result<()> {
        self.sqlite.save_app_state(snapshot)
    }

    fn ensure_conversation_internal(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> anyhow::Result<()> {
        self.sqlite
            .ensure_conversation(project_slug, workspace_name)
    }

    fn load_conversation_internal(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> anyhow::Result<ConversationSnapshot> {
        let snapshot = self
            .sqlite
            .load_conversation(project_slug.clone(), workspace_name.clone())?;

        if !snapshot.entries.is_empty() || snapshot.thread_id.is_some() {
            return Ok(snapshot);
        }

        let Some(legacy) = self.load_conversation_legacy(&project_slug, &workspace_name)? else {
            return Ok(snapshot);
        };

        if legacy.entries.is_empty() && legacy.thread_id.is_none() {
            return Ok(snapshot);
        }

        if let Some(thread_id) = legacy.thread_id.as_deref() {
            let existing_thread_id = self
                .sqlite
                .get_conversation_thread_id(project_slug.clone(), workspace_name.clone())?;
            if existing_thread_id.is_none() {
                self.sqlite.set_conversation_thread_id(
                    project_slug.clone(),
                    workspace_name.clone(),
                    thread_id.to_owned(),
                )?;
            }
        }

        if !legacy.entries.is_empty() {
            self.sqlite.append_conversation_entries(
                project_slug.clone(),
                workspace_name.clone(),
                legacy.entries,
            )?;
        }

        self.sqlite
            .load_conversation(project_slug.clone(), workspace_name.clone())
    }

    fn run_codex_turn_streamed_via_sidecar(
        &self,
        thread_id: Option<String>,
        worktree_path: &Path,
        prompt: &str,
        cancel: Arc<AtomicBool>,
        mut on_event: impl FnMut(CodexThreadEvent) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.ensure_sidecar_installed()?;

        let script = self.agent_sidecar_dir.join("run.mjs");
        let request = SidecarTurnRequest {
            thread_id,
            working_directory: worktree_path
                .to_str()
                .ok_or_else(|| anyhow!("invalid worktree path"))?
                .to_owned(),
            prompt: prompt.to_owned(),
            sandbox_mode: "danger-full-access".to_owned(),
            approval_policy: "never".to_owned(),
            network_access_enabled: true,
            web_search_enabled: true,
            skip_git_repo_check: false,
        };

        let mut child = Command::new("node")
            .arg(&script)
            .current_dir(&self.agent_sidecar_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn node for {}", script.display()))?;

        let input = serde_json::to_vec(&request).context("failed to serialize request")?;
        child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("missing stdin"))?
            .write_all(&input)
            .context("failed to write stdin")?;
        drop(child.stdin.take());

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("missing stderr"))?;

        let finished = Arc::new(AtomicBool::new(false));
        let child = Arc::new(std::sync::Mutex::new(child));
        let killer = {
            let child = child.clone();
            let cancel = cancel.clone();
            let finished = finished.clone();
            std::thread::spawn(move || {
                while !finished.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(25));
                }
                if cancel.load(Ordering::SeqCst)
                    && let Ok(mut child) = child.lock()
                {
                    let _ = child.kill();
                }
            })
        };

        let stderr_handle = std::thread::spawn(move || -> String {
            let mut buf = Vec::new();
            let mut reader = BufReader::new(stderr);
            let _ = reader.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).to_string()
        });

        let stdout_reader = BufReader::new(stdout);
        let mut stdout_noise: Vec<String> = Vec::new();
        for line in stdout_reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    if cancel.load(Ordering::SeqCst) {
                        break;
                    }
                    return Err(err).context("failed to read stdout line");
                }
            };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if cancel.load(Ordering::SeqCst) {
                break;
            }

            match parse_sidecar_stdout_line(trimmed) {
                Ok(SidecarStdoutLine::Event(event)) => on_event(*event)?,
                Ok(
                    SidecarStdoutLine::Ignored { message } | SidecarStdoutLine::Noise { message },
                ) => {
                    if message.is_empty() {
                        continue;
                    }
                    if stdout_noise.len() < 64 {
                        stdout_noise.push(message);
                    }
                }
                Err(err) => {
                    if cancel.load(Ordering::SeqCst) {
                        break;
                    }
                    return Err(err).context("failed to parse codex sidecar stdout");
                }
            }
        }

        let status = child
            .lock()
            .map_err(|_| anyhow!("failed to lock node child"))?
            .wait()
            .context("failed to wait for node")?;
        finished.store(true, Ordering::SeqCst);
        let _ = killer.join();
        let stderr_text = stderr_handle.join().unwrap_or_default();

        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }

        if !status.success() {
            let sidecar_noise = if stdout_noise.is_empty() {
                String::new()
            } else {
                format!("\nstdout (non-protocol):\n{}\n", stdout_noise.join("\n"))
            };
            return Err(anyhow!(
                "codex sidecar failed ({}):\nstderr:\n{}{}",
                status,
                stderr_text.trim(),
                sidecar_noise
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn sidecar_stdout_parsing_accepts_prefixed_events() {
        let parsed = parse_sidecar_stdout_line("__LUBAN_EVENT__ {\"type\":\"turn.started\"}")
            .expect("parse should succeed");
        assert!(matches!(
            parsed,
            SidecarStdoutLine::Event(event) if matches!(*event, CodexThreadEvent::TurnStarted)
        ));
    }

    #[test]
    fn sidecar_stdout_parsing_accepts_legacy_json_events() {
        let parsed =
            parse_sidecar_stdout_line("{\"type\":\"turn.started\"}").expect("parse should succeed");
        assert!(matches!(
            parsed,
            SidecarStdoutLine::Event(event) if matches!(*event, CodexThreadEvent::TurnStarted)
        ));
    }

    #[test]
    fn sidecar_stdout_parsing_ignores_unknown_events() {
        let parsed = parse_sidecar_stdout_line(
            "__LUBAN_EVENT__ {\"type\":\"turn.reconnect\",\"detail\":\"x\"}",
        )
        .expect("parse should succeed");
        assert!(matches!(parsed, SidecarStdoutLine::Ignored { .. }));
    }

    #[test]
    fn sidecar_stdout_parsing_treats_plain_text_as_noise() {
        let parsed = parse_sidecar_stdout_line("retry/reconnect").expect("parse should succeed");
        assert!(matches!(parsed, SidecarStdoutLine::Noise { .. }));
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

        let sqlite = SqliteStore::new(base_dir.join("luban.db")).expect("sqlite init should work");
        let service = GitWorkspaceService {
            worktrees_root: base_dir.join("worktrees"),
            conversations_root: base_dir.join("conversations"),
            agent_sidecar_dir: base_dir.join("sidecar"),
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
    ) -> Result<(), String> {
        self.ensure_conversation_internal(project_slug, workspace_name)
            .map_err(|e| format!("{e:#}"))
    }

    fn load_conversation(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> Result<ConversationSnapshot, String> {
        self.load_conversation_internal(project_slug, workspace_name)
            .map_err(|e| format!("{e:#}"))
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
            thread_id,
            prompt,
        } = request;

        let turn_started_at = Instant::now();
        let duration_appended = Arc::new(AtomicBool::new(false));
        let mut appended_item_ids = HashSet::<String>::new();

        let result: anyhow::Result<()> = (|| {
            self.ensure_conversation_internal(project_slug.clone(), workspace_name.clone())?;

            let mut existing_thread_id = self
                .sqlite
                .get_conversation_thread_id(project_slug.clone(), workspace_name.clone())?;
            if existing_thread_id.is_none()
                && let Some(legacy) =
                    self.load_conversation_legacy(&project_slug, &workspace_name)?
                && let Some(legacy_thread_id) = legacy.thread_id
            {
                self.sqlite.set_conversation_thread_id(
                    project_slug.clone(),
                    workspace_name.clone(),
                    legacy_thread_id.clone(),
                )?;
                existing_thread_id = Some(legacy_thread_id);
            }

            self.sqlite.append_conversation_entries(
                project_slug.clone(),
                workspace_name.clone(),
                vec![ConversationEntry::UserMessage {
                    text: prompt.clone(),
                }],
            )?;

            let resolved_thread_id = thread_id.or(existing_thread_id);

            let mut turn_error: Option<String> = None;
            let mut transient_error_seq: u64 = 0;
            let duration_appended_for_events = duration_appended.clone();

            self.run_codex_turn_streamed_via_sidecar(
                resolved_thread_id,
                &worktree_path,
                &prompt,
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
                        on_event(event.clone());

                        match &event {
                            CodexThreadEvent::ThreadStarted { thread_id } => {
                                self.sqlite.set_conversation_thread_id(
                                    project_slug.clone(),
                                    workspace_name.clone(),
                                    thread_id.clone(),
                                )?;
                            }
                            CodexThreadEvent::ItemCompleted { item } => {
                                let id = codex_item_id(item).to_owned();
                                if appended_item_ids.insert(id) {
                                    self.sqlite.append_conversation_entries(
                                        project_slug.clone(),
                                        workspace_name.clone(),
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
                        vec![ConversationEntry::TurnDuration { duration_ms }],
                    )?;
                    on_event(CodexThreadEvent::TurnDuration { duration_ms });
                }
                self.sqlite.append_conversation_entries(
                    project_slug.clone(),
                    workspace_name.clone(),
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
                    vec![ConversationEntry::TurnDuration { duration_ms }],
                );
                on_event(CodexThreadEvent::TurnDuration { duration_ms });
            }
            let _ = self.sqlite.append_conversation_entries(
                project_slug.clone(),
                workspace_name.clone(),
                vec![ConversationEntry::TurnError {
                    message: format!("{err:#}"),
                }],
            );
        }

        result.map_err(|e| format!("{e:#}"))
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ConversationMeta {
    version: u32,
    thread_id: Option<String>,
    created_at: u64,
    updated_at: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SidecarTurnRequest {
    thread_id: Option<String>,
    working_directory: String,
    prompt: String,
    sandbox_mode: String,
    approval_policy: String,
    network_access_enabled: bool,
    web_search_enabled: bool,
    skip_git_repo_check: bool,
}
