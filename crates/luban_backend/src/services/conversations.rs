use super::GitWorkspaceService;
use anyhow::Context as _;
use luban_domain::{ConversationEntry, ConversationSnapshot, PersistedAppState, WorkspaceStatus};
use std::{
    io::{BufRead as _, BufReader},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn qualify_legacy_conversation_entries(entries: Vec<ConversationEntry>) -> Vec<ConversationEntry> {
    let mut next_turn_index: usize = 0;
    let mut current_scope: Option<String> = None;

    entries
        .into_iter()
        .map(|entry| match entry {
            ConversationEntry::UserMessage { .. } => {
                current_scope = Some(format!("legacy-turn-{next_turn_index}"));
                next_turn_index = next_turn_index.saturating_add(1);
                entry
            }
            ConversationEntry::CodexItem { item } => {
                let scope = current_scope
                    .as_deref()
                    .unwrap_or("legacy-preamble")
                    .to_owned();
                let item = *item;
                let raw_id = super::codex_item_id(&item);
                let item = if raw_id.contains('/') {
                    item
                } else {
                    super::qualify_codex_item(&scope, item)
                };
                ConversationEntry::CodexItem {
                    item: Box::new(item),
                }
            }
            other => other,
        })
        .collect()
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ConversationMeta {
    version: u32,
    thread_id: Option<String>,
    created_at: u64,
    updated_at: u64,
}

impl GitWorkspaceService {
    pub(super) fn conversation_dir(&self, project_slug: &str, workspace_name: &str) -> PathBuf {
        let mut path = self.conversations_root.clone();
        path.push(project_slug);
        path.push(workspace_name);
        path
    }

    pub(super) fn conversation_meta_path(
        &self,
        project_slug: &str,
        workspace_name: &str,
    ) -> PathBuf {
        self.conversation_dir(project_slug, workspace_name)
            .join("conversation.json")
    }

    pub(super) fn conversation_events_path(
        &self,
        project_slug: &str,
        workspace_name: &str,
    ) -> PathBuf {
        self.conversation_dir(project_slug, workspace_name)
            .join("events.jsonl")
    }

    pub(super) fn now_unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
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

    pub(super) fn load_conversation_legacy(
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
                runner: None,
                agent_model_id: None,
                thinking_effort: None,
                amp_mode: None,
                entries: Vec::new(),
                entries_total: 0,
                entries_start: 0,
                pending_prompts: Vec::new(),
                queue_paused: false,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
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
                ) => super::codex_item_id(item) == super::codex_item_id(prev),
                _ => false,
            };
            if !is_duplicate {
                entries.push(entry);
            }
        }

        let entries_total = entries.len() as u64;
        Ok(Some(ConversationSnapshot {
            thread_id: meta.thread_id,
            runner: None,
            agent_model_id: None,
            thinking_effort: None,
            amp_mode: None,
            entries,
            entries_total,
            entries_start: 0,
            pending_prompts: Vec::new(),
            queue_paused: false,
            run_started_at_unix_ms: None,
            run_finished_at_unix_ms: None,
        }))
    }

    pub(super) fn load_app_state_internal(&self) -> anyhow::Result<PersistedAppState> {
        let mut state = self.sqlite.load_app_state()?;
        let mut dirty = false;
        for project in &mut state.projects {
            if !project.is_git {
                continue;
            }
            for workspace in &mut project.workspaces {
                let should_auto_archive = workspace.status == WorkspaceStatus::Active
                    && workspace.workspace_name != "main";
                if should_auto_archive && !workspace.worktree_path.exists() {
                    workspace.status = WorkspaceStatus::Archived;
                    dirty = true;
                    continue;
                }

                let resolved = match self.run_git(
                    &workspace.worktree_path,
                    ["rev-parse", "--abbrev-ref", "HEAD"],
                ) {
                    Ok(resolved) => resolved,
                    Err(_) => {
                        if should_auto_archive {
                            workspace.status = WorkspaceStatus::Archived;
                            dirty = true;
                        }
                        continue;
                    }
                };
                let trimmed = resolved.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if workspace.branch_name != trimmed {
                    workspace.branch_name = trimmed.to_owned();
                    dirty = true;
                }
            }
        }

        if dirty {
            let _ = self.sqlite.save_app_state(state.clone());
        }
        Ok(state)
    }

    pub(super) fn save_app_state_internal(
        &self,
        snapshot: PersistedAppState,
    ) -> anyhow::Result<()> {
        self.sqlite.save_app_state(snapshot)
    }

    pub(super) fn ensure_conversation_internal(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
    ) -> anyhow::Result<()> {
        self.sqlite
            .ensure_conversation(project_slug, workspace_name, thread_id)
    }

    pub(super) fn load_conversation_internal(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_id: u64,
    ) -> anyhow::Result<ConversationSnapshot> {
        let snapshot = self.sqlite.load_conversation(
            project_slug.clone(),
            workspace_name.clone(),
            thread_id,
        )?;

        if thread_id != 1 {
            return Ok(snapshot);
        }

        let snapshot_user_count = snapshot
            .entries
            .iter()
            .filter(|e| matches!(e, ConversationEntry::UserMessage { .. }))
            .count();
        let snapshot_has_unscoped_codex_items = snapshot.entries.iter().any(|e| match e {
            ConversationEntry::CodexItem { item } => !super::codex_item_id(item).contains('/'),
            _ => false,
        });
        let snapshot_has_scoped_codex_items = snapshot.entries.iter().any(|e| match e {
            ConversationEntry::CodexItem { item } => super::codex_item_id(item).contains('/'),
            _ => false,
        });

        let should_attempt_legacy_repair = (!snapshot.entries.is_empty()
            || snapshot.thread_id.is_some())
            && snapshot_has_unscoped_codex_items
            && !snapshot_has_scoped_codex_items;
        let should_attempt_legacy_import =
            snapshot.entries.is_empty() && snapshot.thread_id.is_none();
        if !should_attempt_legacy_import && !should_attempt_legacy_repair {
            return Ok(snapshot);
        }

        let Some(legacy) = self.load_conversation_legacy(&project_slug, &workspace_name)? else {
            return Ok(snapshot);
        };

        if let Some(thread_id) = legacy.thread_id.as_deref() {
            let existing_thread_id = self.sqlite.get_conversation_thread_id(
                project_slug.clone(),
                workspace_name.clone(),
                1,
            )?;
            if existing_thread_id.is_none() {
                self.sqlite.set_conversation_thread_id(
                    project_slug.clone(),
                    workspace_name.clone(),
                    1,
                    thread_id.to_owned(),
                )?;
            }
        }

        if legacy.entries.is_empty() {
            return Ok(snapshot);
        }

        if should_attempt_legacy_repair {
            let legacy_user_count = legacy
                .entries
                .iter()
                .filter(|e| matches!(e, ConversationEntry::UserMessage { .. }))
                .count();
            if snapshot_user_count > legacy_user_count {
                return Ok(snapshot);
            }
        }

        let qualified_entries = qualify_legacy_conversation_entries(legacy.entries);
        self.sqlite.replace_conversation_entries(
            project_slug.clone(),
            workspace_name.clone(),
            1,
            qualified_entries,
        )?;

        self.sqlite
            .load_conversation(project_slug.clone(), workspace_name.clone(), 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite_store::SqliteStore;
    use luban_domain::CodexThreadItem;
    use std::path::{Path, PathBuf};

    fn temp_dir(test_name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push("luban-tests");
        dir.push("services");
        dir.push(format!(
            "{test_name}-{}-{}",
            std::process::id(),
            GitWorkspaceService::now_unix_seconds()
        ));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn write_legacy_events(
        conversations_root: &Path,
        project_slug: &str,
        workspace_name: &str,
        entries: &[ConversationEntry],
    ) {
        let dir = conversations_root.join(project_slug).join(workspace_name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let content = entries
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(path, format!("{content}\n")).unwrap();
    }

    fn agent_message(id: &str, text: &str) -> ConversationEntry {
        ConversationEntry::CodexItem {
            item: Box::new(CodexThreadItem::AgentMessage {
                id: id.to_owned(),
                text: text.to_owned(),
            }),
        }
    }

    #[test]
    fn legacy_import_scopes_repeated_codex_item_ids() {
        let root = temp_dir("legacy_import_scopes_repeated_codex_item_ids");
        let db_path = root.join("state.sqlite");
        let conversations_root = root.join("conversations");
        let worktrees_root = root.join("worktrees");

        let sqlite = SqliteStore::new(db_path).unwrap();
        let svc = GitWorkspaceService {
            worktrees_root,
            conversations_root: conversations_root.clone(),
            task_prompts_root: root.join("task-prompts"),
            sqlite,
            claude_processes: std::sync::Mutex::new(std::collections::HashMap::new()),
        };

        let legacy_entries = vec![
            ConversationEntry::UserMessage {
                text: "u1".to_owned(),
                attachments: Vec::new(),
            },
            agent_message("item_0", "A"),
            ConversationEntry::UserMessage {
                text: "u2".to_owned(),
                attachments: Vec::new(),
            },
            agent_message("item_0", "B"),
        ];
        write_legacy_events(&conversations_root, "p", "w", &legacy_entries);

        let snapshot = svc
            .load_conversation_internal("p".to_owned(), "w".to_owned(), 1)
            .unwrap();

        let agent_texts = snapshot
            .entries
            .iter()
            .filter_map(|e| match e {
                ConversationEntry::CodexItem { item } => match item.as_ref() {
                    CodexThreadItem::AgentMessage { text, .. } => Some(text.as_str()),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(agent_texts, vec!["A", "B"]);
    }

    #[test]
    fn legacy_repair_can_fix_broken_sqlite_import_without_scoped_items() {
        let root = temp_dir("legacy_repair_can_fix_broken_sqlite_import_without_scoped_items");
        let db_path = root.join("state.sqlite");
        let conversations_root = root.join("conversations");
        let worktrees_root = root.join("worktrees");

        let sqlite = SqliteStore::new(db_path).unwrap();
        let svc = GitWorkspaceService {
            worktrees_root,
            conversations_root: conversations_root.clone(),
            task_prompts_root: root.join("task-prompts"),
            sqlite: sqlite.clone(),
            claude_processes: std::sync::Mutex::new(std::collections::HashMap::new()),
        };

        let legacy_entries = vec![
            ConversationEntry::UserMessage {
                text: "u1".to_owned(),
                attachments: Vec::new(),
            },
            agent_message("item_0", "A"),
            ConversationEntry::UserMessage {
                text: "u2".to_owned(),
                attachments: Vec::new(),
            },
            agent_message("item_0", "B"),
        ];
        write_legacy_events(&conversations_root, "p", "w", &legacy_entries);

        sqlite
            .append_conversation_entries("p".to_owned(), "w".to_owned(), 1, legacy_entries.clone())
            .unwrap();
        let before = sqlite
            .load_conversation("p".to_owned(), "w".to_owned(), 1)
            .unwrap();
        let before_agent_count = before
            .entries
            .iter()
            .filter(|e| matches!(e, ConversationEntry::CodexItem { .. }))
            .count();
        assert_eq!(
            before_agent_count, 1,
            "expected broken import to drop duplicates"
        );

        let after = svc
            .load_conversation_internal("p".to_owned(), "w".to_owned(), 1)
            .unwrap();
        let after_agent_texts = after
            .entries
            .iter()
            .filter_map(|e| match e {
                ConversationEntry::CodexItem { item } => match item.as_ref() {
                    CodexThreadItem::AgentMessage { text, .. } => Some(text.as_str()),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(after_agent_texts, vec!["A", "B"]);
    }
}
