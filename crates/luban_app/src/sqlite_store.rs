use anyhow::{Context as _, anyhow};
use luban_domain::{
    ConversationEntry, ConversationSnapshot, ConversationThreadMeta, PersistedAppState,
    WorkspaceStatus, WorkspaceThreadId,
};
use rusqlite::{Connection, OptionalExtension as _, params, params_from_iter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

const LATEST_SCHEMA_VERSION: u32 = 6;
const WORKSPACE_CHAT_SCROLL_PREFIX: &str = "workspace_chat_scroll_y10_";
const WORKSPACE_ACTIVE_THREAD_PREFIX: &str = "workspace_active_thread_id_";
const WORKSPACE_OPEN_TAB_PREFIX: &str = "workspace_open_tab_";
const WORKSPACE_ARCHIVED_TAB_PREFIX: &str = "workspace_archived_tab_";
const WORKSPACE_NEXT_THREAD_ID_PREFIX: &str = "workspace_next_thread_id_";
const LAST_OPEN_WORKSPACE_ID_KEY: &str = "last_open_workspace_id";
const AGENT_DEFAULT_MODEL_ID_KEY: &str = "agent_default_model_id";
const AGENT_DEFAULT_THINKING_EFFORT_KEY: &str = "agent_default_thinking_effort";

const MIGRATIONS: &[(u32, &str)] = &[
    (
        1,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0001_init.sql"
        )),
    ),
    (
        2,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0002_conversation_keys.sql"
        )),
    ),
    (
        3,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0003_app_settings.sql"
        )),
    ),
    (
        4,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0004_project_expanded.sql"
        )),
    ),
    (
        5,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0005_threaded_conversations.sql"
        )),
    ),
    (
        6,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0006_app_settings_text.sql"
        )),
    ),
];

pub struct SqliteStore {
    tx: mpsc::Sender<DbCommand>,
}

impl Clone for SqliteStore {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

enum DbCommand {
    LoadAppState {
        reply: mpsc::Sender<anyhow::Result<PersistedAppState>>,
    },
    SaveAppState {
        snapshot: Box<PersistedAppState>,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    EnsureConversation {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    GetConversationThreadId {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        reply: mpsc::Sender<anyhow::Result<Option<String>>>,
    },
    SetConversationThreadId {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        thread_id: String,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    ListConversationThreads {
        project_slug: String,
        workspace_name: String,
        reply: mpsc::Sender<anyhow::Result<Vec<ConversationThreadMeta>>>,
    },
    AppendConversationEntries {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        entries: Vec<ConversationEntry>,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    LoadConversation {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        reply: mpsc::Sender<anyhow::Result<ConversationSnapshot>>,
    },
}

impl SqliteStore {
    pub fn new(db_path: PathBuf) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel::<DbCommand>();

        std::thread::Builder::new()
            .name("luban-sqlite".to_owned())
            .spawn(move || {
                let mut db = SqliteDatabase::open(&db_path);
                while let Ok(cmd) = rx.recv() {
                    match (&mut db, cmd) {
                        (Ok(db), DbCommand::LoadAppState { reply }) => {
                            let _ = reply.send(db.load_app_state());
                        }
                        (Ok(db), DbCommand::SaveAppState { snapshot, reply }) => {
                            let _ = reply.send(db.save_app_state(&snapshot));
                        }
                        (
                            Ok(db),
                            DbCommand::EnsureConversation {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.ensure_conversation(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::GetConversationThreadId {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.get_conversation_thread_id(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::SetConversationThreadId {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                thread_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.set_conversation_thread_id(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                &thread_id,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::ListConversationThreads {
                                project_slug,
                                workspace_name,
                                reply,
                            },
                        ) => {
                            let _ = reply
                                .send(db.list_conversation_threads(&project_slug, &workspace_name));
                        }
                        (
                            Ok(db),
                            DbCommand::AppendConversationEntries {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                entries,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.append_conversation_entries(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                &entries,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::LoadConversation {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.load_conversation(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                            ));
                        }
                        (Err(err), cmd) => {
                            respond_db_open_error(err, cmd);
                        }
                    }
                }
            })
            .context("failed to spawn sqlite worker thread")?;

        Ok(Self { tx })
    }

    pub fn load_app_state(&self) -> anyhow::Result<PersistedAppState> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::LoadAppState { reply: reply_tx })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn save_app_state(&self, snapshot: PersistedAppState) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::SaveAppState {
                snapshot: Box::new(snapshot),
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn ensure_conversation(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::EnsureConversation {
                project_slug,
                workspace_name,
                thread_local_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn get_conversation_thread_id(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
    ) -> anyhow::Result<Option<String>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::GetConversationThreadId {
                project_slug,
                workspace_name,
                thread_local_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn set_conversation_thread_id(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        thread_id: String,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::SetConversationThreadId {
                project_slug,
                workspace_name,
                thread_local_id,
                thread_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn list_conversation_threads(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> anyhow::Result<Vec<ConversationThreadMeta>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::ListConversationThreads {
                project_slug,
                workspace_name,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn append_conversation_entries(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        entries: Vec<ConversationEntry>,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::AppendConversationEntries {
                project_slug,
                workspace_name,
                thread_local_id,
                entries,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn load_conversation(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
    ) -> anyhow::Result<ConversationSnapshot> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::LoadConversation {
                project_slug,
                workspace_name,
                thread_local_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }
}

fn respond_db_open_error(err: &anyhow::Error, cmd: DbCommand) {
    let message = format!("{err:#}");
    match cmd {
        DbCommand::LoadAppState { reply } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::SaveAppState { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::EnsureConversation { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::GetConversationThreadId { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::SetConversationThreadId { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::ListConversationThreads { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::AppendConversationEntries { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::LoadConversation { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
    }
}

struct SqliteDatabase {
    conn: Connection,
}

impl SqliteDatabase {
    fn open(db_path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let mut conn = Connection::open(db_path)
            .with_context(|| format!("failed to open sqlite db {}", db_path.display()))?;

        configure_connection(&mut conn).context("failed to configure sqlite connection")?;
        apply_migrations(&mut conn).context("failed to apply sqlite migrations")?;

        Ok(Self { conn })
    }

    fn load_app_state(&mut self) -> anyhow::Result<PersistedAppState> {
        let mut projects = Vec::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT id, slug, name, path, expanded FROM projects ORDER BY id ASC")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?;
            for row in rows {
                let (id, slug, name, path, expanded) = row?;
                projects.push(luban_domain::PersistedProject {
                    id,
                    slug,
                    name,
                    path: PathBuf::from(path),
                    expanded: expanded != 0,
                    workspaces: Vec::new(),
                });
            }
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, workspace_name, branch_name, worktree_path, status, last_activity_at
             FROM workspaces ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<i64>>(6)?,
            ))
        })?;

        for row in rows {
            let (
                id,
                project_id,
                workspace_name,
                branch_name,
                worktree_path,
                status,
                last_activity_at,
            ) = row?;
            let status = workspace_status_from_i64(status)?;
            let last_activity_at_unix_seconds = last_activity_at.map(|v| v as u64);

            let Some(project) = projects.iter_mut().find(|p| p.id == project_id) else {
                continue;
            };

            project.workspaces.push(luban_domain::PersistedWorkspace {
                id,
                workspace_name,
                branch_name,
                worktree_path: PathBuf::from(worktree_path),
                status,
                last_activity_at_unix_seconds,
            });
        }

        let sidebar_width = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'sidebar_width'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load sidebar width")?
            .and_then(|value| u16::try_from(value).ok());

        let terminal_pane_width = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'terminal_pane_width'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load terminal pane width")?
            .and_then(|value| u16::try_from(value).ok());

        let last_open_workspace_id = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![LAST_OPEN_WORKSPACE_ID_KEY],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load last open workspace id")?
            .and_then(|value| u64::try_from(value).ok());

        let agent_default_model_id = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![AGENT_DEFAULT_MODEL_ID_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load agent default model id")?;

        let agent_default_thinking_effort = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![AGENT_DEFAULT_THINKING_EFFORT_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load agent default thinking effort")?;

        let mut workspace_active_thread_id = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT key, value FROM app_settings WHERE key LIKE 'workspace_active_thread_id_%'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(workspace_id) = key.strip_prefix(WORKSPACE_ACTIVE_THREAD_PREFIX) else {
                continue;
            };
            let Ok(workspace_id) = workspace_id.parse::<u64>() else {
                continue;
            };
            let Some(thread_id) = u64::try_from(value).ok() else {
                continue;
            };
            workspace_active_thread_id.insert(workspace_id, thread_id);
        }

        let mut workspace_next_thread_id = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT key, value FROM app_settings WHERE key LIKE 'workspace_next_thread_id_%'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(workspace_id) = key.strip_prefix(WORKSPACE_NEXT_THREAD_ID_PREFIX) else {
                continue;
            };
            let Ok(workspace_id) = workspace_id.parse::<u64>() else {
                continue;
            };
            let Some(thread_id) = u64::try_from(value).ok() else {
                continue;
            };
            workspace_next_thread_id.insert(workspace_id, thread_id);
        }

        let mut workspace_open_tabs_indexed: HashMap<u64, Vec<(u32, u64)>> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM app_settings WHERE key LIKE 'workspace_open_tab_%'")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(rest) = key.strip_prefix(WORKSPACE_OPEN_TAB_PREFIX) else {
                continue;
            };
            let mut parts = rest.split('_');
            let Some(index_str) = parts.next() else {
                continue;
            };
            let Some(workspace_id_str) = parts.next() else {
                continue;
            };
            if parts.next().is_some() {
                continue;
            }
            let Ok(index) = index_str.parse::<u32>() else {
                continue;
            };
            let Ok(workspace_id) = workspace_id_str.parse::<u64>() else {
                continue;
            };
            let Some(thread_id) = u64::try_from(value).ok() else {
                continue;
            };
            workspace_open_tabs_indexed
                .entry(workspace_id)
                .or_default()
                .push((index, thread_id));
        }
        let mut workspace_open_tabs: HashMap<u64, Vec<u64>> = HashMap::new();
        for (workspace_id, mut tabs) in workspace_open_tabs_indexed {
            tabs.sort_by_key(|(index, _)| *index);
            let ids = tabs.into_iter().map(|(_, id)| id).collect::<Vec<_>>();
            if !ids.is_empty() {
                workspace_open_tabs.insert(workspace_id, ids);
            }
        }

        let mut workspace_archived_tabs_indexed: HashMap<u64, Vec<(u32, u64)>> = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT key, value FROM app_settings WHERE key LIKE 'workspace_archived_tab_%'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(rest) = key.strip_prefix(WORKSPACE_ARCHIVED_TAB_PREFIX) else {
                continue;
            };
            let mut parts = rest.split('_');
            let Some(index_str) = parts.next() else {
                continue;
            };
            let Some(workspace_id_str) = parts.next() else {
                continue;
            };
            if parts.next().is_some() {
                continue;
            }
            let Ok(index) = index_str.parse::<u32>() else {
                continue;
            };
            let Ok(workspace_id) = workspace_id_str.parse::<u64>() else {
                continue;
            };
            let Some(thread_id) = u64::try_from(value).ok() else {
                continue;
            };
            workspace_archived_tabs_indexed
                .entry(workspace_id)
                .or_default()
                .push((index, thread_id));
        }
        let mut workspace_archived_tabs: HashMap<u64, Vec<u64>> = HashMap::new();
        for (workspace_id, mut tabs) in workspace_archived_tabs_indexed {
            tabs.sort_by_key(|(index, _)| *index);
            let ids = tabs.into_iter().map(|(_, id)| id).collect::<Vec<_>>();
            if !ids.is_empty() {
                workspace_archived_tabs.insert(workspace_id, ids);
            }
        }

        let mut workspace_chat_scroll_y10 = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT key, value FROM app_settings WHERE key LIKE 'workspace_chat_scroll_y10_%'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(rest) = key.strip_prefix(WORKSPACE_CHAT_SCROLL_PREFIX) else {
                continue;
            };
            let mut parts = rest.split('_');
            let Some(workspace_id_str) = parts.next() else {
                continue;
            };
            let Ok(workspace_id) = workspace_id_str.parse::<u64>() else {
                continue;
            };
            let thread_id = match parts.next() {
                Some(thread_id_str) => match thread_id_str.parse::<u64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                None => 1,
            };
            if parts.next().is_some() {
                continue;
            }
            let Some(offset_y10) = i32::try_from(value).ok() else {
                continue;
            };
            workspace_chat_scroll_y10.insert((workspace_id, thread_id), offset_y10);
        }

        Ok(PersistedAppState {
            projects,
            sidebar_width,
            terminal_pane_width,
            agent_default_model_id,
            agent_default_thinking_effort,
            last_open_workspace_id,
            workspace_active_thread_id,
            workspace_open_tabs,
            workspace_archived_tabs,
            workspace_next_thread_id,
            workspace_chat_scroll_y10,
        })
    }

    fn save_app_state(&mut self, snapshot: &PersistedAppState) -> anyhow::Result<()> {
        let now = now_unix_seconds();
        let tx = self.conn.transaction()?;

        for project in &snapshot.projects {
            let path = project.path.to_string_lossy().into_owned();
            tx.execute(
                "INSERT INTO projects (id, slug, name, path, expanded, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, COALESCE((SELECT created_at FROM projects WHERE id = ?1), ?6), ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   slug = excluded.slug,
                   name = excluded.name,
                   path = excluded.path,
                   expanded = excluded.expanded,
                   updated_at = excluded.updated_at",
                params![
                    project.id as i64,
                    project.slug,
                    project.name,
                    path,
                    if project.expanded { 1i64 } else { 0i64 },
                    now,
                ],
            )?;
        }

        if snapshot.projects.is_empty() {
            tx.execute("DELETE FROM projects", [])?;
        } else {
            let placeholders = std::iter::repeat_n("?", snapshot.projects.len())
                .collect::<Vec<_>>()
                .join(",");
            tx.execute(
                &format!("DELETE FROM projects WHERE id NOT IN ({placeholders})"),
                params_from_iter(snapshot.projects.iter().map(|p| p.id as i64)),
            )?;
        }

        let mut workspace_ids = Vec::new();
        for project in &snapshot.projects {
            for workspace in &project.workspaces {
                workspace_ids.push(workspace.id);
                let worktree_path = workspace.worktree_path.to_string_lossy().into_owned();
                tx.execute(
                    "INSERT INTO workspaces (id, project_id, workspace_name, branch_name, worktree_path, status, last_activity_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE((SELECT created_at FROM workspaces WHERE id = ?1), ?8), ?8)
                     ON CONFLICT(id) DO UPDATE SET
                       project_id = excluded.project_id,
                       workspace_name = excluded.workspace_name,
                       branch_name = excluded.branch_name,
                       worktree_path = excluded.worktree_path,
                       status = excluded.status,
                       last_activity_at = excluded.last_activity_at,
                       updated_at = excluded.updated_at",
                    params![
                        workspace.id as i64,
                        project.id as i64,
                        workspace.workspace_name,
                        workspace.branch_name,
                        worktree_path,
                        workspace_status_to_i64(workspace.status),
                        workspace.last_activity_at_unix_seconds.map(|v| v as i64),
                        now,
                    ],
                )?;
            }
        }

        if workspace_ids.is_empty() {
            tx.execute("DELETE FROM workspaces", [])?;
        } else {
            let placeholders = std::iter::repeat_n("?", workspace_ids.len())
                .collect::<Vec<_>>()
                .join(",");
            tx.execute(
                &format!("DELETE FROM workspaces WHERE id NOT IN ({placeholders})"),
                params_from_iter(workspace_ids.iter().copied().map(|id| id as i64)),
            )?;
        }

        if let Some(value) = snapshot.sidebar_width {
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params!["sidebar_width", value as i64, now],
            )?;
        } else {
            tx.execute("DELETE FROM app_settings WHERE key = 'sidebar_width'", [])?;
        }

        if let Some(value) = snapshot.terminal_pane_width {
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params!["terminal_pane_width", value as i64, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings WHERE key = 'terminal_pane_width'",
                [],
            )?;
        }

        if let Some(value) = snapshot.agent_default_model_id.as_deref() {
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![AGENT_DEFAULT_MODEL_ID_KEY, value, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings_text WHERE key = ?1",
                params![AGENT_DEFAULT_MODEL_ID_KEY],
            )?;
        }

        if let Some(value) = snapshot.agent_default_thinking_effort.as_deref() {
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![AGENT_DEFAULT_THINKING_EFFORT_KEY, value, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings_text WHERE key = ?1",
                params![AGENT_DEFAULT_THINKING_EFFORT_KEY],
            )?;
        }

        if let Some(value) = snapshot.last_open_workspace_id {
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![LAST_OPEN_WORKSPACE_ID_KEY, value as i64, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings WHERE key = ?1",
                params![LAST_OPEN_WORKSPACE_ID_KEY],
            )?;
        }

        tx.execute(
            "DELETE FROM app_settings WHERE key LIKE 'workspace_active_thread_id_%'",
            [],
        )?;
        for (workspace_id, thread_id) in &snapshot.workspace_active_thread_id {
            let key = format!("{WORKSPACE_ACTIVE_THREAD_PREFIX}{workspace_id}");
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![key, *thread_id as i64, now],
            )?;
        }

        tx.execute(
            "DELETE FROM app_settings WHERE key LIKE 'workspace_open_tab_%'",
            [],
        )?;
        for (workspace_id, tabs) in &snapshot.workspace_open_tabs {
            for (idx, thread_id) in tabs.iter().copied().enumerate() {
                let key = format!("{WORKSPACE_OPEN_TAB_PREFIX}{idx}_{workspace_id}");
                tx.execute(
                    "INSERT INTO app_settings (key, value, created_at, updated_at)
                     VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                     ON CONFLICT(key) DO UPDATE SET
                       value = excluded.value,
                       updated_at = excluded.updated_at",
                    params![key, thread_id as i64, now],
                )?;
            }
        }

        tx.execute(
            "DELETE FROM app_settings WHERE key LIKE 'workspace_archived_tab_%'",
            [],
        )?;
        for (workspace_id, tabs) in &snapshot.workspace_archived_tabs {
            for (idx, thread_id) in tabs.iter().copied().enumerate() {
                let key = format!("{WORKSPACE_ARCHIVED_TAB_PREFIX}{idx}_{workspace_id}");
                tx.execute(
                    "INSERT INTO app_settings (key, value, created_at, updated_at)
                     VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                     ON CONFLICT(key) DO UPDATE SET
                       value = excluded.value,
                       updated_at = excluded.updated_at",
                    params![key, thread_id as i64, now],
                )?;
            }
        }

        tx.execute(
            "DELETE FROM app_settings WHERE key LIKE 'workspace_next_thread_id_%'",
            [],
        )?;
        for (workspace_id, next_id) in &snapshot.workspace_next_thread_id {
            let key = format!("{WORKSPACE_NEXT_THREAD_ID_PREFIX}{workspace_id}");
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![key, *next_id as i64, now],
            )?;
        }

        tx.execute(
            "DELETE FROM app_settings WHERE key LIKE 'workspace_chat_scroll_y10_%'",
            [],
        )?;
        for ((workspace_id, thread_id), offset_y10) in &snapshot.workspace_chat_scroll_y10 {
            let key = format!("{WORKSPACE_CHAT_SCROLL_PREFIX}{workspace_id}_{thread_id}");
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![key, *offset_y10 as i64, now],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    fn ensure_conversation(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) -> anyhow::Result<()> {
        let now = now_unix_seconds();
        let default_title = format!("Thread {thread_local_id}");
        self.conn.execute(
            "INSERT INTO conversations (project_slug, workspace_name, thread_local_id, thread_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?5)
             ON CONFLICT(project_slug, workspace_name, thread_local_id) DO NOTHING",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                default_title,
                now
            ],
        )?;
        self.conn.execute(
            "UPDATE conversations
             SET title = COALESCE(title, ?4)
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                default_title
            ],
        )?;

        Ok(())
    }

    fn get_conversation_thread_id(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) -> anyhow::Result<Option<String>> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;
        self.conn
            .query_row(
                "SELECT thread_id FROM conversations
                 WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
                params![project_slug, workspace_name, thread_local_id as i64],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|opt| opt.flatten())
            .context("failed to load thread id")
    }

    fn set_conversation_thread_id(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        thread_id: &str,
    ) -> anyhow::Result<()> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;
        let now = now_unix_seconds();
        self.conn.execute(
            "UPDATE conversations
             SET thread_id = ?3, updated_at = ?4
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?5",
            params![
                project_slug,
                workspace_name,
                thread_id,
                now,
                thread_local_id as i64
            ],
        )?;

        Ok(())
    }

    fn list_conversation_threads(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
    ) -> anyhow::Result<Vec<ConversationThreadMeta>> {
        self.ensure_conversation(project_slug, workspace_name, 1)?;
        let mut stmt = self.conn.prepare(
            "SELECT thread_local_id, thread_id, title, updated_at
             FROM conversations
             WHERE project_slug = ?1 AND workspace_name = ?2
             ORDER BY updated_at DESC, thread_local_id DESC",
        )?;
        let rows = stmt.query_map(params![project_slug, workspace_name], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;

        let mut threads = Vec::new();
        for row in rows {
            let (thread_local_id, remote_thread_id, title, updated_at) = row?;
            let Some(thread_local_id) = u64::try_from(thread_local_id).ok() else {
                continue;
            };
            let Some(updated_at) = u64::try_from(updated_at).ok() else {
                continue;
            };
            let title = title.unwrap_or_else(|| format!("Thread {thread_local_id}"));
            threads.push(ConversationThreadMeta {
                thread_id: WorkspaceThreadId::from_u64(thread_local_id),
                remote_thread_id,
                title,
                updated_at_unix_seconds: updated_at,
            });
        }

        Ok(threads)
    }

    fn append_conversation_entries(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        entries: &[ConversationEntry],
    ) -> anyhow::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;

        let derived_title = entries.iter().find_map(|entry| match entry {
            ConversationEntry::UserMessage { text } => {
                let title = luban_domain::derive_thread_title(text);
                if title.is_empty() { None } else { Some(title) }
            }
            _ => None,
        });

        let now = now_unix_seconds();
        let tx = self.conn.transaction()?;
        for entry in entries {
            let (kind, codex_item_id) = conversation_entry_index_fields(entry);
            let payload_json = serde_json::to_string(entry).context("failed to serialize entry")?;
            tx.execute(
                "INSERT OR IGNORE INTO conversation_entries
                 (project_slug, workspace_name, thread_local_id, seq, kind, codex_item_id, payload_json, created_at)
                 VALUES (?1, ?2, ?3,
                   (SELECT COALESCE(MAX(seq), 0) + 1
                    FROM conversation_entries
                    WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3),
                   ?4, ?5, ?6, ?7)",
                params![
                    project_slug,
                    workspace_name,
                    thread_local_id as i64,
                    kind,
                    codex_item_id,
                    payload_json,
                    now
                ],
            )?;
        }
        tx.execute(
            "UPDATE conversations
             SET updated_at = ?4
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64, now],
        )?;
        if let Some(title) = derived_title {
            tx.execute(
                "UPDATE conversations
                 SET title = ?4
                 WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
                   AND (title IS NULL OR title LIKE 'Thread %')",
                params![project_slug, workspace_name, thread_local_id as i64, title],
            )?;
        }
        tx.commit()?;

        Ok(())
    }

    fn load_conversation(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) -> anyhow::Result<ConversationSnapshot> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;
        let thread_id = self
            .conn
            .query_row(
                "SELECT thread_id FROM conversations
                 WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
                params![project_slug, workspace_name, thread_local_id as i64],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .context("failed to load conversation meta")?
            .flatten();

        let mut stmt = self.conn.prepare(
            "SELECT payload_json
             FROM conversation_entries
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
             ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(
            params![project_slug, workspace_name, thread_local_id as i64],
            |row| row.get::<_, String>(0),
        )?;

        let mut entries = Vec::new();
        for row in rows {
            let json = row?;
            let entry: ConversationEntry =
                serde_json::from_str(&json).context("failed to parse entry")?;
            entries.push(entry);
        }

        Ok(ConversationSnapshot { thread_id, entries })
    }
}

fn configure_connection(conn: &mut Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;",
    )
    .context("failed to apply sqlite PRAGMAs")?;
    Ok(())
}

fn apply_migrations(conn: &mut Connection) -> anyhow::Result<()> {
    let mut current: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .context("failed to read user_version")? as u32;

    if current > LATEST_SCHEMA_VERSION {
        return Err(anyhow!(
            "sqlite schema version is newer than this build: db={}, app={}",
            current,
            LATEST_SCHEMA_VERSION
        ));
    }

    if current == LATEST_SCHEMA_VERSION {
        return Ok(());
    }

    conn.execute_batch("BEGIN IMMEDIATE;")
        .context("failed to begin migration transaction")?;

    for (version, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }
        conn.execute_batch(sql)
            .with_context(|| format!("failed to apply migration v{version:04}"))?;
        conn.pragma_update(None, "user_version", *version as i64)
            .context("failed to update user_version")?;
        current = *version;
    }

    conn.execute_batch("COMMIT;")
        .context("failed to commit migration transaction")?;
    Ok(())
}

fn now_unix_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn workspace_status_to_i64(status: WorkspaceStatus) -> i64 {
    match status {
        WorkspaceStatus::Active => 0,
        WorkspaceStatus::Archived => 1,
    }
}

fn workspace_status_from_i64(v: i64) -> anyhow::Result<WorkspaceStatus> {
    match v {
        0 => Ok(WorkspaceStatus::Active),
        1 => Ok(WorkspaceStatus::Archived),
        _ => Err(anyhow!("invalid workspace status: {v}")),
    }
}

fn conversation_entry_index_fields(entry: &ConversationEntry) -> (&'static str, Option<&str>) {
    match entry {
        ConversationEntry::UserMessage { .. } => ("user_message", None),
        ConversationEntry::CodexItem { item } => ("codex_item", Some(codex_item_id(item.as_ref()))),
        ConversationEntry::TurnUsage { .. } => ("turn_usage", None),
        ConversationEntry::TurnDuration { .. } => ("turn_duration", None),
        ConversationEntry::TurnCanceled => ("turn_canceled", None),
        ConversationEntry::TurnError { .. } => ("turn_error", None),
    }
}

fn codex_item_id(item: &luban_domain::CodexThreadItem) -> &str {
    match item {
        luban_domain::CodexThreadItem::AgentMessage { id, .. } => id,
        luban_domain::CodexThreadItem::Reasoning { id, .. } => id,
        luban_domain::CodexThreadItem::CommandExecution { id, .. } => id,
        luban_domain::CodexThreadItem::FileChange { id, .. } => id,
        luban_domain::CodexThreadItem::McpToolCall { id, .. } => id,
        luban_domain::CodexThreadItem::WebSearch { id, .. } => id,
        luban_domain::CodexThreadItem::TodoList { id, .. } => id,
        luban_domain::CodexThreadItem::Error { id, .. } => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luban_domain::{CodexThreadItem, PersistedProject, PersistedWorkspace};

    fn temp_db_path(test_name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push("luban-tests");
        let _ = std::fs::create_dir_all(&dir);
        dir.push(format!(
            "{test_name}-{}-{}.db",
            std::process::id(),
            now_unix_seconds()
        ));
        dir
    }

    #[test]
    fn migrations_create_schema() {
        let path = temp_db_path("migrations_create_schema");
        let db = SqliteDatabase::open(&path).unwrap();

        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('projects','workspaces','conversations','conversation_entries','app_settings','app_settings_text')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 6);
    }

    #[test]
    fn save_and_load_app_state_roundtrips() {
        let path = temp_db_path("save_and_load_app_state_roundtrips");
        let mut db = SqliteDatabase::open(&path).unwrap();

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "my-project".to_owned(),
                name: "My Project".to_owned(),
                path: PathBuf::from("/tmp/my-project"),
                expanded: true,
                workspaces: vec![PersistedWorkspace {
                    id: 10,
                    workspace_name: "alpha".to_owned(),
                    branch_name: "luban/alpha".to_owned(),
                    worktree_path: PathBuf::from("/tmp/my-project/worktrees/alpha"),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                }],
            }],
            sidebar_width: Some(280),
            terminal_pane_width: Some(360),
            agent_default_model_id: Some("gpt-5.2-codex".to_owned()),
            agent_default_thinking_effort: Some("high".to_owned()),
            last_open_workspace_id: Some(10),
            workspace_active_thread_id: HashMap::from([(10, 1)]),
            workspace_open_tabs: HashMap::from([(10, vec![1, 2, 3])]),
            workspace_archived_tabs: HashMap::from([(10, vec![9, 8])]),
            workspace_next_thread_id: HashMap::from([(10, 4)]),
            workspace_chat_scroll_y10: HashMap::from([((10, 1), -1234)]),
        };

        db.save_app_state(&snapshot).unwrap();
        let loaded = db.load_app_state().unwrap();
        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn conversation_append_is_idempotent_by_codex_item_id() {
        let path = temp_db_path("conversation_append_is_idempotent_by_codex_item_id");
        let mut db = SqliteDatabase::open(&path).unwrap();

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "p".to_owned(),
                name: "P".to_owned(),
                path: PathBuf::from("/tmp/p"),
                expanded: false,
                workspaces: vec![PersistedWorkspace {
                    id: 2,
                    workspace_name: "w".to_owned(),
                    branch_name: "luban/w".to_owned(),
                    worktree_path: PathBuf::from("/tmp/p/worktrees/w"),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                }],
            }],
            sidebar_width: None,
            terminal_pane_width: None,
            agent_default_model_id: None,
            agent_default_thinking_effort: None,
            last_open_workspace_id: None,
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
        };
        db.save_app_state(&snapshot).unwrap();

        db.ensure_conversation("p", "w", 1).unwrap();

        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };
        let entry = ConversationEntry::CodexItem {
            item: Box::new(item),
        };

        db.append_conversation_entries("p", "w", 1, std::slice::from_ref(&entry))
            .unwrap();
        db.append_conversation_entries("p", "w", 1, std::slice::from_ref(&entry))
            .unwrap();

        let snapshot = db.load_conversation("p", "w", 1).unwrap();
        let count = snapshot
            .entries
            .iter()
            .filter(|e| matches!(e, ConversationEntry::CodexItem { .. }))
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn conversation_append_allows_same_raw_id_across_turns_when_scoped() {
        let path = temp_db_path("conversation_append_allows_same_raw_id_across_turns_when_scoped");
        let mut db = SqliteDatabase::open(&path).unwrap();

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "p".to_owned(),
                name: "P".to_owned(),
                path: PathBuf::from("/tmp/p"),
                expanded: false,
                workspaces: vec![PersistedWorkspace {
                    id: 2,
                    workspace_name: "w".to_owned(),
                    branch_name: "luban/w".to_owned(),
                    worktree_path: PathBuf::from("/tmp/p/worktrees/w"),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                }],
            }],
            sidebar_width: None,
            terminal_pane_width: None,
            agent_default_model_id: None,
            agent_default_thinking_effort: None,
            last_open_workspace_id: None,
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
        };
        db.save_app_state(&snapshot).unwrap();

        db.ensure_conversation("p", "w", 1).unwrap();

        let entry_a = ConversationEntry::CodexItem {
            item: Box::new(CodexThreadItem::AgentMessage {
                id: "turn-a/item_0".to_owned(),
                text: "A".to_owned(),
            }),
        };
        let entry_b = ConversationEntry::CodexItem {
            item: Box::new(CodexThreadItem::AgentMessage {
                id: "turn-b/item_0".to_owned(),
                text: "B".to_owned(),
            }),
        };

        db.append_conversation_entries("p", "w", 1, std::slice::from_ref(&entry_a))
            .unwrap();
        db.append_conversation_entries("p", "w", 1, std::slice::from_ref(&entry_b))
            .unwrap();

        let snapshot = db.load_conversation("p", "w", 1).unwrap();
        let messages = snapshot
            .entries
            .iter()
            .filter_map(|e| match e {
                ConversationEntry::CodexItem { item } => match item.as_ref() {
                    CodexThreadItem::AgentMessage { id, text } => {
                        Some((id.as_str(), text.as_str()))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            vec![("turn-a/item_0", "A"), ("turn-b/item_0", "B")]
        );
    }
}
