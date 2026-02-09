use anyhow::{Context as _, anyhow};
use luban_domain::{
    AttachmentKind, AttachmentRef, ChatScrollAnchor, ContextItem, ConversationEntry,
    ConversationSnapshot, ConversationThreadMeta, PersistedAppState, QueuedPrompt,
    TaskDocumentEvent, TaskDocumentEventType, TaskDocumentIndex, TaskDocumentKind, ThinkingEffort,
    WorkspaceStatus, WorkspaceThreadId,
};
use rand::{RngCore as _, rngs::OsRng};
use rusqlite::{Connection, OptionalExtension as _, params, params_from_iter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SqliteStoreError {
    ConversationNotFound,
}

impl std::fmt::Display for SqliteStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqliteStoreError::ConversationNotFound => write!(f, "conversation not found"),
        }
    }
}

impl std::error::Error for SqliteStoreError {}

const LATEST_SCHEMA_VERSION: u32 = 23;
const WORKSPACE_CHAT_SCROLL_PREFIX: &str = "workspace_chat_scroll_y10_";
const WORKSPACE_CHAT_SCROLL_ANCHOR_PREFIX: &str = "workspace_chat_scroll_anchor_";
const WORKSPACE_ACTIVE_THREAD_PREFIX: &str = "workspace_active_thread_id_";
const WORKSPACE_OPEN_TAB_PREFIX: &str = "workspace_open_tab_";
const WORKSPACE_ARCHIVED_TAB_PREFIX: &str = "workspace_archived_tab_";
const WORKSPACE_NEXT_THREAD_ID_PREFIX: &str = "workspace_next_thread_id_";
const WORKSPACE_UNREAD_COMPLETION_PREFIX: &str = "workspace_unread_completion_";
const WORKSPACE_THREAD_RUN_CONFIG_PREFIX: &str = "workspace_thread_run_config_";
const TASK_STARRED_PREFIX: &str = "task_starred_";
const LAST_OPEN_WORKSPACE_ID_KEY: &str = "last_open_workspace_id";
const OPEN_BUTTON_SELECTION_KEY: &str = "open_button_selection";
const SIDEBAR_PROJECT_ORDER_KEY: &str = "sidebar_project_order";
const GLOBAL_ZOOM_PERCENT_KEY: &str = "global_zoom_percent";
const AGENT_DEFAULT_MODEL_ID_KEY: &str = "agent_default_model_id";
const AGENT_RUNNER_DEFAULT_MODELS_KEY: &str = "agent_runner_default_models";
const AGENT_DEFAULT_THINKING_EFFORT_KEY: &str = "agent_default_thinking_effort";
const AGENT_DEFAULT_RUNNER_KEY: &str = "agent_default_runner";
const AGENT_AMP_MODE_KEY: &str = "agent_amp_mode";
const AGENT_CODEX_ENABLED_KEY: &str = "agent_codex_enabled";
const AGENT_AMP_ENABLED_KEY: &str = "agent_amp_enabled";
const AGENT_CLAUDE_ENABLED_KEY: &str = "agent_claude_enabled";
const AGENT_DROID_ENABLED_KEY: &str = "agent_droid_enabled";
const TASK_PROMPT_TEMPLATE_PREFIX: &str = "task_prompt_template_";
const APPEARANCE_THEME_KEY: &str = "appearance_theme";
const APPEARANCE_UI_FONT_KEY: &str = "appearance_ui_font";
const APPEARANCE_CHAT_FONT_KEY: &str = "appearance_chat_font";
const APPEARANCE_CODE_FONT_KEY: &str = "appearance_code_font";
const APPEARANCE_TERMINAL_FONT_KEY: &str = "appearance_terminal_font";
const TELEGRAM_ENABLED_KEY: &str = "telegram_enabled";
const TELEGRAM_BOT_TOKEN_KEY: &str = "telegram_bot_token";
const TELEGRAM_BOT_USERNAME_KEY: &str = "telegram_bot_username";
const TELEGRAM_PAIRED_CHAT_ID_KEY: &str = "telegram_paired_chat_id";
const TELEGRAM_TOPIC_BINDINGS_KEY: &str = "telegram_topic_bindings";

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
    (
        7,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0007_project_archived.sql"
        )),
    ),
    (
        8,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0008_context_items.sql"
        )),
    ),
    (
        9,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0009_project_is_git.sql"
        )),
    ),
    (
        10,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0010_workspace_branch_renamed.sql"
        )),
    ),
    (
        11,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0011_drop_workspace_branch_fields.sql"
        )),
    ),
    (
        12,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0012_conversation_queue.sql"
        )),
    ),
    (
        13,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0013_conversation_run_timing.sql"
        )),
    ),
    (
        14,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0014_conversation_run_config.sql"
        )),
    ),
    (
        15,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0015_conversation_agent_runner.sql"
        )),
    ),
    (
        16,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0016_conversation_task_status.sql"
        )),
    ),
    (
        17,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0017_conversation_events_v2.sql"
        )),
    ),
    (
        18,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0018_conversation_entry_id.sql"
        )),
    ),
    (
        19,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0019_conversation_task_status_auto_update.sql"
        )),
    ),
    (
        20,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0020_conversation_task_validation_pr.sql"
        )),
    ),
    (
        21,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0021_cleanup_autocreated_thread1.sql"
        )),
    ),
    (
        22,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0022_new_task_drafts.sql"
        )),
    ),
    (
        23,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0023_task_documents.sql"
        )),
    ),
];

#[derive(Clone)]
pub struct SqliteStore {
    tx: mpsc::Sender<DbCommand>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SqliteStoreOptions {
    pub persist_ui_state: bool,
}

impl Default for SqliteStoreOptions {
    fn default() -> Self {
        Self {
            persist_ui_state: true,
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
    GetAppSettingText {
        key: String,
        reply: mpsc::Sender<anyhow::Result<Option<String>>>,
    },
    SetAppSettingText {
        key: String,
        value: Option<String>,
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
    ReplaceConversationEntries {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        entries: Vec<ConversationEntry>,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    UpdateConversationTitleIfMatches {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        expected_current_title: String,
        new_title: String,
        reply: mpsc::Sender<anyhow::Result<bool>>,
    },
    LoadConversation {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        reply: mpsc::Sender<anyhow::Result<ConversationSnapshot>>,
    },
    LoadConversationPage {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        before: Option<u64>,
        limit: u64,
        reply: mpsc::Sender<anyhow::Result<ConversationSnapshot>>,
    },
    DeleteConversationThread {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    SaveConversationQueueState {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        queue_paused: bool,
        run_started_at_unix_ms: Option<u64>,
        run_finished_at_unix_ms: Option<u64>,
        pending_prompts: Vec<QueuedPrompt>,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    SaveConversationRunConfig {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        runner: luban_domain::AgentRunnerKind,
        model_id: String,
        thinking_effort: ThinkingEffort,
        amp_mode: Option<String>,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    SaveConversationTaskStatus {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        task_status: luban_domain::TaskStatus,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    SaveConversationTaskStatusLastAnalyzed {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    SaveConversationTaskValidationPr {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        pr_number: u64,
        pr_url: Option<String>,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    ListConversationTasksForMergedPr {
        project_slug: String,
        workspace_name: String,
        pr_number: u64,
        reply: mpsc::Sender<anyhow::Result<Vec<u64>>>,
    },
    UpsertTaskDocumentIndex {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        index: TaskDocumentIndex,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    ListTaskDocumentIndices {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        reply: mpsc::Sender<anyhow::Result<Vec<TaskDocumentIndex>>>,
    },
    InsertTaskDocumentEvent {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        kind: TaskDocumentKind,
        event_type: TaskDocumentEventType,
        rel_path: String,
        content_hash: String,
        byte_len: u64,
        created_at_unix_ms: u64,
        reply: mpsc::Sender<anyhow::Result<u64>>,
    },
    ListTaskDocumentEvents {
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        reply: mpsc::Sender<anyhow::Result<Vec<TaskDocumentEvent>>>,
    },
    InsertContextItem {
        project_slug: String,
        workspace_name: String,
        attachment: AttachmentRef,
        created_at_unix_ms: u64,
        reply: mpsc::Sender<anyhow::Result<u64>>,
    },
    ListContextItems {
        project_slug: String,
        workspace_name: String,
        reply: mpsc::Sender<anyhow::Result<Vec<ContextItem>>>,
    },
    DeleteContextItem {
        project_slug: String,
        workspace_name: String,
        context_id: u64,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    ListNewTaskDrafts {
        reply: mpsc::Sender<anyhow::Result<Vec<luban_domain::NewTaskDraft>>>,
    },
    CreateNewTaskDraft {
        text: String,
        project_id: Option<String>,
        workspace_id: Option<u64>,
        reply: mpsc::Sender<anyhow::Result<luban_domain::NewTaskDraft>>,
    },
    UpdateNewTaskDraft {
        draft_id: String,
        text: String,
        project_id: Option<String>,
        workspace_id: Option<u64>,
        reply: mpsc::Sender<anyhow::Result<luban_domain::NewTaskDraft>>,
    },
    DeleteNewTaskDraft {
        draft_id: String,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    LoadNewTaskStash {
        reply: mpsc::Sender<anyhow::Result<Option<luban_domain::NewTaskStash>>>,
    },
    SaveNewTaskStash {
        stash: luban_domain::NewTaskStash,
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
    ClearNewTaskStash {
        reply: mpsc::Sender<anyhow::Result<()>>,
    },
}

impl SqliteStore {
    pub fn new(db_path: PathBuf) -> anyhow::Result<Self> {
        Self::new_with_options(db_path, SqliteStoreOptions::default())
    }

    pub fn new_with_options(db_path: PathBuf, options: SqliteStoreOptions) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel::<DbCommand>();

        std::thread::Builder::new()
            .name("luban-sqlite".to_owned())
            .spawn(move || {
                let mut db = SqliteDatabase::open(&db_path, options);
                while let Ok(cmd) = rx.recv() {
                    match (&mut db, cmd) {
                        (Ok(db), DbCommand::LoadAppState { reply }) => {
                            let _ = reply.send(db.load_app_state());
                        }
                        (Ok(db), DbCommand::SaveAppState { snapshot, reply }) => {
                            let _ = reply.send(db.save_app_state(&snapshot));
                        }
                        (Ok(db), DbCommand::GetAppSettingText { key, reply }) => {
                            let _ = reply.send(db.get_app_setting_text(&key));
                        }
                        (Ok(db), DbCommand::SetAppSettingText { key, value, reply }) => {
                            let _ = reply.send(db.set_app_setting_text(&key, value.as_deref()));
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
                            DbCommand::ReplaceConversationEntries {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                entries,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.replace_conversation_entries(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                &entries,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::UpdateConversationTitleIfMatches {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                expected_current_title,
                                new_title,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.update_conversation_title_if_matches(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                &expected_current_title,
                                &new_title,
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
                        (
                            Ok(db),
                            DbCommand::LoadConversationPage {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                before,
                                limit,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.load_conversation_page(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                before,
                                limit,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::DeleteConversationThread {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.delete_conversation_thread(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::SaveConversationQueueState {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                queue_paused,
                                run_started_at_unix_ms,
                                run_finished_at_unix_ms,
                                pending_prompts,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.save_conversation_queue_state(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                queue_paused,
                                run_started_at_unix_ms,
                                run_finished_at_unix_ms,
                                &pending_prompts,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::SaveConversationRunConfig {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                runner,
                                model_id,
                                thinking_effort,
                                amp_mode,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.save_conversation_run_config(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                runner,
                                &model_id,
                                thinking_effort,
                                amp_mode.as_deref(),
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::SaveConversationTaskStatus {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                task_status,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.save_conversation_task_status(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                task_status,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::SaveConversationTaskStatusLastAnalyzed {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.save_conversation_task_status_last_analyzed(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::SaveConversationTaskValidationPr {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                pr_number,
                                pr_url,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.save_conversation_task_validation_pr(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                pr_number,
                                pr_url.as_deref(),
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::ListConversationTasksForMergedPr {
                                project_slug,
                                workspace_name,
                                pr_number,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.list_conversation_tasks_for_merged_pr(
                                &project_slug,
                                &workspace_name,
                                pr_number,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::UpsertTaskDocumentIndex {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                index,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.upsert_task_document_index(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                &index,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::ListTaskDocumentIndices {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.list_task_document_indices(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::InsertTaskDocumentEvent {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                kind,
                                event_type,
                                rel_path,
                                content_hash,
                                byte_len,
                                created_at_unix_ms,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.insert_task_document_event(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                                kind,
                                event_type,
                                &rel_path,
                                &content_hash,
                                byte_len,
                                created_at_unix_ms,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::ListTaskDocumentEvents {
                                project_slug,
                                workspace_name,
                                thread_local_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.list_task_document_events(
                                &project_slug,
                                &workspace_name,
                                thread_local_id,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::InsertContextItem {
                                project_slug,
                                workspace_name,
                                attachment,
                                created_at_unix_ms,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.insert_context_item(
                                &project_slug,
                                &workspace_name,
                                &attachment,
                                created_at_unix_ms,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::ListContextItems {
                                project_slug,
                                workspace_name,
                                reply,
                            },
                        ) => {
                            let _ =
                                reply.send(db.list_context_items(&project_slug, &workspace_name));
                        }
                        (
                            Ok(db),
                            DbCommand::DeleteContextItem {
                                project_slug,
                                workspace_name,
                                context_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.delete_context_item(
                                &project_slug,
                                &workspace_name,
                                context_id,
                            ));
                        }
                        (Ok(db), DbCommand::ListNewTaskDrafts { reply }) => {
                            let _ = reply.send(db.list_new_task_drafts());
                        }
                        (
                            Ok(db),
                            DbCommand::CreateNewTaskDraft {
                                text,
                                project_id,
                                workspace_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.create_new_task_draft(
                                &text,
                                project_id.as_deref(),
                                workspace_id,
                            ));
                        }
                        (
                            Ok(db),
                            DbCommand::UpdateNewTaskDraft {
                                draft_id,
                                text,
                                project_id,
                                workspace_id,
                                reply,
                            },
                        ) => {
                            let _ = reply.send(db.update_new_task_draft(
                                &draft_id,
                                &text,
                                project_id.as_deref(),
                                workspace_id,
                            ));
                        }
                        (Ok(db), DbCommand::DeleteNewTaskDraft { draft_id, reply }) => {
                            let _ = reply.send(db.delete_new_task_draft(&draft_id));
                        }
                        (Ok(db), DbCommand::LoadNewTaskStash { reply }) => {
                            let _ = reply.send(db.load_new_task_stash());
                        }
                        (Ok(db), DbCommand::SaveNewTaskStash { stash, reply }) => {
                            let _ = reply.send(db.save_new_task_stash(&stash));
                        }
                        (Ok(db), DbCommand::ClearNewTaskStash { reply }) => {
                            let _ = reply.send(db.clear_new_task_stash());
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

    pub fn get_app_setting_text(&self, key: impl Into<String>) -> anyhow::Result<Option<String>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::GetAppSettingText {
                key: key.into(),
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn set_app_setting_text(
        &self,
        key: impl Into<String>,
        value: Option<String>,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::SetAppSettingText {
                key: key.into(),
                value,
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

    pub fn replace_conversation_entries(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        entries: Vec<ConversationEntry>,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::ReplaceConversationEntries {
                project_slug,
                workspace_name,
                thread_local_id,
                entries,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn update_conversation_title_if_matches(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        expected_current_title: String,
        new_title: String,
    ) -> anyhow::Result<bool> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::UpdateConversationTitleIfMatches {
                project_slug,
                workspace_name,
                thread_local_id,
                expected_current_title,
                new_title,
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

    pub fn load_conversation_page(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        before: Option<u64>,
        limit: u64,
    ) -> anyhow::Result<ConversationSnapshot> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::LoadConversationPage {
                project_slug,
                workspace_name,
                thread_local_id,
                before,
                limit,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn delete_conversation_thread(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::DeleteConversationThread {
                project_slug,
                workspace_name,
                thread_local_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_conversation_queue_state(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        queue_paused: bool,
        run_started_at_unix_ms: Option<u64>,
        run_finished_at_unix_ms: Option<u64>,
        pending_prompts: Vec<QueuedPrompt>,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::SaveConversationQueueState {
                project_slug,
                workspace_name,
                thread_local_id,
                queue_paused,
                run_started_at_unix_ms,
                run_finished_at_unix_ms,
                pending_prompts,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_conversation_run_config(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        runner: luban_domain::AgentRunnerKind,
        model_id: String,
        thinking_effort: ThinkingEffort,
        amp_mode: Option<String>,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::SaveConversationRunConfig {
                project_slug,
                workspace_name,
                thread_local_id,
                runner,
                model_id,
                thinking_effort,
                amp_mode,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn save_conversation_task_status(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        task_status: luban_domain::TaskStatus,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::SaveConversationTaskStatus {
                project_slug,
                workspace_name,
                thread_local_id,
                task_status,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn save_conversation_task_status_last_analyzed(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::SaveConversationTaskStatusLastAnalyzed {
                project_slug,
                workspace_name,
                thread_local_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn save_conversation_task_validation_pr(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        pr_number: u64,
        pr_url: Option<String>,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::SaveConversationTaskValidationPr {
                project_slug,
                workspace_name,
                thread_local_id,
                pr_number,
                pr_url,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn list_conversation_tasks_for_merged_pr(
        &self,
        project_slug: String,
        workspace_name: String,
        pr_number: u64,
    ) -> anyhow::Result<Vec<u64>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::ListConversationTasksForMergedPr {
                project_slug,
                workspace_name,
                pr_number,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn upsert_task_document_index(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        index: TaskDocumentIndex,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::UpsertTaskDocumentIndex {
                project_slug,
                workspace_name,
                thread_local_id,
                index,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn list_task_document_indices(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
    ) -> anyhow::Result<Vec<TaskDocumentIndex>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::ListTaskDocumentIndices {
                project_slug,
                workspace_name,
                thread_local_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_task_document_event(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
        kind: TaskDocumentKind,
        event_type: TaskDocumentEventType,
        rel_path: String,
        content_hash: String,
        byte_len: u64,
        created_at_unix_ms: u64,
    ) -> anyhow::Result<u64> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::InsertTaskDocumentEvent {
                project_slug,
                workspace_name,
                thread_local_id,
                kind,
                event_type,
                rel_path,
                content_hash,
                byte_len,
                created_at_unix_ms,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn list_task_document_events(
        &self,
        project_slug: String,
        workspace_name: String,
        thread_local_id: u64,
    ) -> anyhow::Result<Vec<TaskDocumentEvent>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::ListTaskDocumentEvents {
                project_slug,
                workspace_name,
                thread_local_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn insert_context_item(
        &self,
        project_slug: String,
        workspace_name: String,
        attachment: AttachmentRef,
        created_at_unix_ms: u64,
    ) -> anyhow::Result<u64> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::InsertContextItem {
                project_slug,
                workspace_name,
                attachment,
                created_at_unix_ms,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn list_context_items(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> anyhow::Result<Vec<ContextItem>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::ListContextItems {
                project_slug,
                workspace_name,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn delete_context_item(
        &self,
        project_slug: String,
        workspace_name: String,
        context_id: u64,
    ) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::DeleteContextItem {
                project_slug,
                workspace_name,
                context_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn list_new_task_drafts(&self) -> anyhow::Result<Vec<luban_domain::NewTaskDraft>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::ListNewTaskDrafts { reply: reply_tx })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn create_new_task_draft(
        &self,
        text: String,
        project_id: Option<String>,
        workspace_id: Option<u64>,
    ) -> anyhow::Result<luban_domain::NewTaskDraft> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::CreateNewTaskDraft {
                text,
                project_id,
                workspace_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn update_new_task_draft(
        &self,
        draft_id: String,
        text: String,
        project_id: Option<String>,
        workspace_id: Option<u64>,
    ) -> anyhow::Result<luban_domain::NewTaskDraft> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::UpdateNewTaskDraft {
                draft_id,
                text,
                project_id,
                workspace_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn delete_new_task_draft(&self, draft_id: String) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::DeleteNewTaskDraft {
                draft_id,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn load_new_task_stash(&self) -> anyhow::Result<Option<luban_domain::NewTaskStash>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::LoadNewTaskStash { reply: reply_tx })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn save_new_task_stash(&self, stash: luban_domain::NewTaskStash) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::SaveNewTaskStash {
                stash,
                reply: reply_tx,
            })
            .context("sqlite worker is not running")?;
        reply_rx.recv().context("sqlite worker terminated")?
    }

    pub fn clear_new_task_stash(&self) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(DbCommand::ClearNewTaskStash { reply: reply_tx })
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
        DbCommand::GetAppSettingText { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::SetAppSettingText { reply, .. } => {
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
        DbCommand::ReplaceConversationEntries { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::UpdateConversationTitleIfMatches { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::LoadConversation { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::LoadConversationPage { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::DeleteConversationThread { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::SaveConversationQueueState { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::SaveConversationRunConfig { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::SaveConversationTaskStatus { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::SaveConversationTaskStatusLastAnalyzed { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::SaveConversationTaskValidationPr { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::ListConversationTasksForMergedPr { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::UpsertTaskDocumentIndex { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::ListTaskDocumentIndices { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::InsertTaskDocumentEvent { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::ListTaskDocumentEvents { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::InsertContextItem { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::ListContextItems { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::DeleteContextItem { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::ListNewTaskDrafts { reply } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::CreateNewTaskDraft { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::UpdateNewTaskDraft { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::DeleteNewTaskDraft { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::LoadNewTaskStash { reply } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::SaveNewTaskStash { reply, .. } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
        DbCommand::ClearNewTaskStash { reply } => {
            let _ = reply.send(Err(anyhow!(message)));
        }
    }
}

struct SqliteDatabase {
    conn: Connection,
    persist_ui_state: bool,
}

impl SqliteDatabase {
    fn open(db_path: &Path, options: SqliteStoreOptions) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let mut conn = Connection::open(db_path)
            .with_context(|| format!("failed to open sqlite db {}", db_path.display()))?;

        configure_connection(&mut conn).context("failed to configure sqlite connection")?;
        apply_migrations(&mut conn).context("failed to apply sqlite migrations")?;

        Ok(Self {
            conn,
            persist_ui_state: options.persist_ui_state,
        })
    }

    fn load_app_state(&mut self) -> anyhow::Result<PersistedAppState> {
        let mut projects = Vec::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT id, slug, name, path, expanded, is_git FROM projects ORDER BY id ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?;
            for row in rows {
                let (id, slug, name, path, expanded, is_git) = row?;
                projects.push(luban_domain::PersistedProject {
                    id,
                    slug,
                    name,
                    path: PathBuf::from(path),
                    is_git: is_git != 0,
                    expanded: expanded != 0,
                    workspaces: Vec::new(),
                });
            }
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, workspace_name, worktree_path, status, last_activity_at
             FROM workspaces ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<i64>>(5)?,
            ))
        })?;

        for row in rows {
            let (id, project_id, workspace_name, worktree_path, status, last_activity_at) = row?;
            let status = workspace_status_from_i64(status)?;
            let last_activity_at_unix_seconds = last_activity_at.map(|v| v as u64);

            let Some(project) = projects.iter_mut().find(|p| p.id == project_id) else {
                continue;
            };

            let branch_name = workspace_name.clone();
            project.workspaces.push(luban_domain::PersistedWorkspace {
                id,
                workspace_name,
                branch_name,
                worktree_path: PathBuf::from(worktree_path),
                status,
                last_activity_at_unix_seconds,
            });
        }

        let agent_default_model_id = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![AGENT_DEFAULT_MODEL_ID_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load agent default model id")?;

        let agent_runner_default_models: HashMap<String, String> = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![AGENT_RUNNER_DEFAULT_MODELS_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load agent runner default models")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        let agent_default_thinking_effort = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![AGENT_DEFAULT_THINKING_EFFORT_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load agent default thinking effort")?;

        let agent_default_runner = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![AGENT_DEFAULT_RUNNER_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load agent default runner")?;

        let agent_amp_mode = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![AGENT_AMP_MODE_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load agent amp mode")?;

        let agent_codex_enabled = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![AGENT_CODEX_ENABLED_KEY],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load agent codex enabled flag")?
            .map(|value| value != 0);

        let agent_amp_enabled = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![AGENT_AMP_ENABLED_KEY],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load agent amp enabled flag")?
            .map(|value| value != 0);

        let agent_claude_enabled = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![AGENT_CLAUDE_ENABLED_KEY],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load agent claude enabled flag")?
            .map(|value| value != 0);

        let agent_droid_enabled = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![AGENT_DROID_ENABLED_KEY],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load agent droid enabled flag")?
            .map(|value| value != 0);

        let telegram_enabled = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![TELEGRAM_ENABLED_KEY],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load telegram enabled flag")?
            .map(|value| value != 0);

        let telegram_bot_token = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![TELEGRAM_BOT_TOKEN_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load telegram bot token")?;

        let telegram_bot_username = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![TELEGRAM_BOT_USERNAME_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load telegram bot username")?;

        let telegram_paired_chat_id = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![TELEGRAM_PAIRED_CHAT_ID_KEY],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load telegram paired chat id")?;

        let telegram_topic_bindings = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![TELEGRAM_TOPIC_BINDINGS_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load telegram topic bindings")?;

        let mut task_prompt_templates = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT key, value FROM app_settings_text WHERE key LIKE 'task_prompt_template_%'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(kind) = key.strip_prefix(TASK_PROMPT_TEMPLATE_PREFIX) else {
                continue;
            };
            if kind.trim().is_empty() || value.trim().is_empty() {
                continue;
            }
            task_prompt_templates.insert(kind.to_owned(), value);
        }

        let mut workspace_thread_run_config_overrides = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT key, value FROM app_settings_text WHERE key LIKE 'workspace_thread_run_config_%'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(raw) = key.strip_prefix(WORKSPACE_THREAD_RUN_CONFIG_PREFIX) else {
                continue;
            };
            let mut parts = raw.split('_');
            let workspace_id = match parts.next() {
                Some(workspace_id_str) => match workspace_id_str.parse::<u64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                None => continue,
            };
            let thread_id = match parts.next() {
                Some(thread_id_str) => match thread_id_str.parse::<u64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                None => continue,
            };
            if parts.next().is_some() {
                continue;
            }
            let Ok(run_config) = serde_json::from_str::<
                luban_domain::PersistedWorkspaceThreadRunConfigOverride,
            >(&value) else {
                continue;
            };
            workspace_thread_run_config_overrides.insert((workspace_id, thread_id), run_config);
        }

        if !self.persist_ui_state {
            return Ok(PersistedAppState {
                projects,
                sidebar_width: None,
                terminal_pane_width: None,
                global_zoom_percent: None,
                appearance_theme: None,
                appearance_ui_font: None,
                appearance_chat_font: None,
                appearance_code_font: None,
                appearance_terminal_font: None,
                agent_default_model_id,
                agent_runner_default_models,
                agent_default_thinking_effort,
                agent_default_runner,
                agent_amp_mode,
                agent_codex_enabled,
                agent_amp_enabled,
                agent_claude_enabled,
                agent_droid_enabled,
                last_open_workspace_id: None,
                open_button_selection: None,
                sidebar_project_order: Vec::new(),
                workspace_active_thread_id: HashMap::new(),
                workspace_open_tabs: HashMap::new(),
                workspace_archived_tabs: HashMap::new(),
                workspace_next_thread_id: HashMap::new(),
                workspace_chat_scroll_y10: HashMap::new(),
                workspace_chat_scroll_anchor: HashMap::new(),
                workspace_unread_completions: HashMap::new(),
                workspace_thread_run_config_overrides,
                starred_tasks: HashMap::new(),
                task_prompt_templates,
                telegram_enabled,
                telegram_bot_token,
                telegram_bot_username,
                telegram_paired_chat_id,
                telegram_topic_bindings,
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

        let global_zoom_percent = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![GLOBAL_ZOOM_PERCENT_KEY],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed to load global zoom")?
            .and_then(|value| u16::try_from(value).ok());

        let appearance_theme = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![APPEARANCE_THEME_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load appearance theme")?;

        let appearance_ui_font = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![APPEARANCE_UI_FONT_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load appearance ui font")?;

        let appearance_chat_font = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![APPEARANCE_CHAT_FONT_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load appearance chat font")?;

        let appearance_code_font = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![APPEARANCE_CODE_FONT_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load appearance code font")?;

        let appearance_terminal_font = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![APPEARANCE_TERMINAL_FONT_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load appearance terminal font")?;

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

        let open_button_selection = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![OPEN_BUTTON_SELECTION_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load open button selection")?;

        let sidebar_project_order = self
            .conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![SIDEBAR_PROJECT_ORDER_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load sidebar project order")?
            .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
            .unwrap_or_default();

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

        let mut workspace_chat_scroll_anchor = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT key, value FROM app_settings_text WHERE key LIKE 'workspace_chat_scroll_anchor_%'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(rest) = key.strip_prefix(WORKSPACE_CHAT_SCROLL_ANCHOR_PREFIX) else {
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
            let Ok(anchor) = serde_json::from_str::<ChatScrollAnchor>(&value) else {
                continue;
            };
            workspace_chat_scroll_anchor.insert((workspace_id, thread_id), anchor);
        }

        let mut workspace_unread_completions = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT key, value FROM app_settings WHERE key LIKE 'workspace_unread_completion_%'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(workspace_id) = key.strip_prefix(WORKSPACE_UNREAD_COMPLETION_PREFIX) else {
                continue;
            };
            let Ok(workspace_id) = workspace_id.parse::<u64>() else {
                continue;
            };
            let unread = value != 0;
            if unread {
                workspace_unread_completions.insert(workspace_id, true);
            }
        }

        let mut starred_tasks = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM app_settings WHERE key LIKE 'task_starred_%'")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let Some(raw) = key.strip_prefix(TASK_STARRED_PREFIX) else {
                continue;
            };
            let mut parts = raw.split('_');
            let workspace_id = match parts.next() {
                Some(workspace_id_str) => match workspace_id_str.parse::<u64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                None => continue,
            };
            let thread_id = match parts.next() {
                Some(thread_id_str) => match thread_id_str.parse::<u64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                None => continue,
            };
            if parts.next().is_some() {
                continue;
            }
            let starred = value != 0;
            if starred {
                starred_tasks.insert((workspace_id, thread_id), true);
            }
        }

        Ok(PersistedAppState {
            projects,
            sidebar_width,
            terminal_pane_width,
            global_zoom_percent,
            appearance_theme,
            appearance_ui_font,
            appearance_chat_font,
            appearance_code_font,
            appearance_terminal_font,
            agent_default_model_id,
            agent_runner_default_models,
            agent_default_thinking_effort,
            agent_default_runner,
            agent_amp_mode,
            agent_codex_enabled,
            agent_amp_enabled,
            agent_claude_enabled,
            agent_droid_enabled,
            last_open_workspace_id,
            open_button_selection,
            sidebar_project_order,
            workspace_active_thread_id,
            workspace_open_tabs,
            workspace_archived_tabs,
            workspace_next_thread_id,
            workspace_chat_scroll_y10,
            workspace_chat_scroll_anchor,
            workspace_unread_completions,
            workspace_thread_run_config_overrides,
            starred_tasks,
            task_prompt_templates,
            telegram_enabled,
            telegram_bot_token,
            telegram_bot_username,
            telegram_paired_chat_id,
            telegram_topic_bindings,
        })
    }

    fn get_app_setting_text(&mut self, key: &str) -> anyhow::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM app_settings_text WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .with_context(|| format!("failed to load app setting text {key}"))
    }

    fn set_app_setting_text(&mut self, key: &str, value: Option<&str>) -> anyhow::Result<()> {
        let now = now_unix_seconds();
        let tx = self.conn.transaction()?;
        if let Some(value) = value {
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![key, value, now],
            )?;
        } else {
            tx.execute("DELETE FROM app_settings_text WHERE key = ?1", params![key])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn save_app_state(&mut self, snapshot: &PersistedAppState) -> anyhow::Result<()> {
        let now = now_unix_seconds();
        let tx = self.conn.transaction()?;

        let mut existing_workspace_keys: HashMap<u64, (String, String)> = HashMap::new();
        {
            let mut stmt = tx.prepare(
                "SELECT w.id, p.slug, w.workspace_name
                 FROM workspaces w
                 JOIN projects p ON p.id = w.project_id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            for row in rows {
                let (workspace_id, project_slug, workspace_name) = row?;
                existing_workspace_keys.insert(workspace_id, (project_slug, workspace_name));
            }
        }

        for project in &snapshot.projects {
            let path = project.path.to_string_lossy().into_owned();
            tx.execute(
                "INSERT INTO projects (id, slug, name, path, expanded, is_git, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE((SELECT created_at FROM projects WHERE id = ?1), ?7), ?7)
                 ON CONFLICT(id) DO UPDATE SET
                   slug = excluded.slug,
                   name = excluded.name,
                   path = excluded.path,
                   expanded = excluded.expanded,
                   is_git = excluded.is_git,
                   updated_at = excluded.updated_at",
                params![
                    project.id as i64,
                    project.slug,
                    project.name,
                    path,
                    if project.expanded { 1i64 } else { 0i64 },
                    if project.is_git { 1i64 } else { 0i64 },
                    now,
                ],
            )?;
        }

        let mut workspace_ids = Vec::new();
        for project in &snapshot.projects {
            for workspace in &project.workspaces {
                workspace_ids.push(workspace.id);
                let worktree_path = workspace.worktree_path.to_string_lossy().into_owned();
                tx.execute(
                    "INSERT INTO workspaces (id, project_id, workspace_name, worktree_path, status, last_activity_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE((SELECT created_at FROM workspaces WHERE id = ?1), ?7), ?7)
                     ON CONFLICT(id) DO UPDATE SET
                       project_id = excluded.project_id,
                       workspace_name = excluded.workspace_name,
                       worktree_path = excluded.worktree_path,
                       status = excluded.status,
                       last_activity_at = excluded.last_activity_at,
                       updated_at = excluded.updated_at",
                    params![
                        workspace.id as i64,
                        project.id as i64,
                        workspace.workspace_name,
                        worktree_path,
                        workspace_status_to_i64(workspace.status),
                        workspace.last_activity_at_unix_seconds.map(|v| v as i64),
                        now,
                    ],
                )?;

                if let Some((old_slug, old_workspace_name)) =
                    existing_workspace_keys.get(&workspace.id)
                    && (old_slug != &project.slug
                        || old_workspace_name != &workspace.workspace_name)
                {
                    tx.execute(
                        "UPDATE conversations
                         SET project_slug = ?1, workspace_name = ?2
                         WHERE project_slug = ?3 AND workspace_name = ?4",
                        params![
                            project.slug,
                            workspace.workspace_name,
                            old_slug,
                            old_workspace_name
                        ],
                    )?;
                    tx.execute(
                        "UPDATE conversation_entries
                         SET project_slug = ?1, workspace_name = ?2
                         WHERE project_slug = ?3 AND workspace_name = ?4",
                        params![
                            project.slug,
                            workspace.workspace_name,
                            old_slug,
                            old_workspace_name
                        ],
                    )?;
                    tx.execute(
                        "UPDATE task_documents
                         SET project_slug = ?1, workspace_name = ?2
                         WHERE project_slug = ?3 AND workspace_name = ?4",
                        params![
                            project.slug,
                            workspace.workspace_name,
                            old_slug,
                            old_workspace_name
                        ],
                    )?;
                    tx.execute(
                        "UPDATE task_document_events
                         SET project_slug = ?1, workspace_name = ?2
                         WHERE project_slug = ?3 AND workspace_name = ?4",
                        params![
                            project.slug,
                            workspace.workspace_name,
                            old_slug,
                            old_workspace_name
                        ],
                    )?;
                }
            }
        }

        {
            use std::collections::HashSet;
            let snapshot_workspace_ids: HashSet<u64> = workspace_ids.iter().copied().collect();
            for (workspace_id, (project_slug, workspace_name)) in existing_workspace_keys {
                if snapshot_workspace_ids.contains(&workspace_id) {
                    continue;
                }

                tx.execute(
                    "DELETE FROM conversation_entries
                     WHERE project_slug = ?1 AND workspace_name = ?2",
                    params![project_slug, workspace_name],
                )?;
                tx.execute(
                    "DELETE FROM conversations
                     WHERE project_slug = ?1 AND workspace_name = ?2",
                    params![project_slug, workspace_name],
                )?;
                tx.execute(
                    "DELETE FROM task_documents
                     WHERE project_slug = ?1 AND workspace_name = ?2",
                    params![project_slug, workspace_name],
                )?;
                tx.execute(
                    "DELETE FROM task_document_events
                     WHERE project_slug = ?1 AND workspace_name = ?2",
                    params![project_slug, workspace_name],
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

        if self.persist_ui_state {
            let upsert_text = |tx: &rusqlite::Transaction<'_>, key: &str, value: Option<&str>| {
                if let Some(value) = value {
                    tx.execute(
                        "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                         VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                         ON CONFLICT(key) DO UPDATE SET
                           value = excluded.value,
                           updated_at = excluded.updated_at",
                        params![key, value, now],
                    )?;
                } else {
                    tx.execute("DELETE FROM app_settings_text WHERE key = ?1", params![key])?;
                }
                Ok::<(), rusqlite::Error>(())
            };

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

            upsert_text(
                &tx,
                APPEARANCE_THEME_KEY,
                snapshot.appearance_theme.as_deref(),
            )?;
            upsert_text(
                &tx,
                APPEARANCE_UI_FONT_KEY,
                snapshot.appearance_ui_font.as_deref(),
            )?;
            upsert_text(
                &tx,
                APPEARANCE_CHAT_FONT_KEY,
                snapshot.appearance_chat_font.as_deref(),
            )?;
            upsert_text(
                &tx,
                APPEARANCE_CODE_FONT_KEY,
                snapshot.appearance_code_font.as_deref(),
            )?;
            upsert_text(
                &tx,
                APPEARANCE_TERMINAL_FONT_KEY,
                snapshot.appearance_terminal_font.as_deref(),
            )?;
            upsert_text(
                &tx,
                OPEN_BUTTON_SELECTION_KEY,
                snapshot.open_button_selection.as_deref(),
            )?;
            let sidebar_project_order = (!snapshot.sidebar_project_order.is_empty())
                .then(|| serde_json::to_string(&snapshot.sidebar_project_order).ok())
                .flatten();
            upsert_text(
                &tx,
                SIDEBAR_PROJECT_ORDER_KEY,
                sidebar_project_order.as_deref(),
            )?;

            if let Some(value) = snapshot.global_zoom_percent {
                tx.execute(
                    "INSERT INTO app_settings (key, value, created_at, updated_at)
                     VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                     ON CONFLICT(key) DO UPDATE SET
                       value = excluded.value,
                       updated_at = excluded.updated_at",
                    params![GLOBAL_ZOOM_PERCENT_KEY, value as i64, now],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM app_settings WHERE key = ?1",
                    params![GLOBAL_ZOOM_PERCENT_KEY],
                )?;
            }
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

        if !snapshot.agent_runner_default_models.is_empty() {
            let json =
                serde_json::to_string(&snapshot.agent_runner_default_models).unwrap_or_default();
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![AGENT_RUNNER_DEFAULT_MODELS_KEY, json, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings_text WHERE key = ?1",
                params![AGENT_RUNNER_DEFAULT_MODELS_KEY],
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

        if let Some(value) = snapshot.agent_default_runner.as_deref() {
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![AGENT_DEFAULT_RUNNER_KEY, value, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings_text WHERE key = ?1",
                params![AGENT_DEFAULT_RUNNER_KEY],
            )?;
        }

        if let Some(value) = snapshot.agent_amp_mode.as_deref() {
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![AGENT_AMP_MODE_KEY, value, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings_text WHERE key = ?1",
                params![AGENT_AMP_MODE_KEY],
            )?;
        }

        if let Some(enabled) = snapshot.agent_codex_enabled {
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![
                    AGENT_CODEX_ENABLED_KEY,
                    if enabled { 1i64 } else { 0i64 },
                    now
                ],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings WHERE key = ?1",
                params![AGENT_CODEX_ENABLED_KEY],
            )?;
        }

        if let Some(enabled) = snapshot.agent_amp_enabled {
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![
                    AGENT_AMP_ENABLED_KEY,
                    if enabled { 1i64 } else { 0i64 },
                    now
                ],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings WHERE key = ?1",
                params![AGENT_AMP_ENABLED_KEY],
            )?;
        }

        if let Some(enabled) = snapshot.agent_claude_enabled {
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![
                    AGENT_CLAUDE_ENABLED_KEY,
                    if enabled { 1i64 } else { 0i64 },
                    now
                ],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings WHERE key = ?1",
                params![AGENT_CLAUDE_ENABLED_KEY],
            )?;
        }

        if let Some(enabled) = snapshot.agent_droid_enabled {
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![
                    AGENT_DROID_ENABLED_KEY,
                    if enabled { 1i64 } else { 0i64 },
                    now
                ],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings WHERE key = ?1",
                params![AGENT_DROID_ENABLED_KEY],
            )?;
        }

        if let Some(enabled) = snapshot.telegram_enabled {
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![
                    TELEGRAM_ENABLED_KEY,
                    if enabled { 1i64 } else { 0i64 },
                    now
                ],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings WHERE key = ?1",
                params![TELEGRAM_ENABLED_KEY],
            )?;
        }

        if let Some(value) = snapshot.telegram_bot_token.as_deref() {
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![TELEGRAM_BOT_TOKEN_KEY, value, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings_text WHERE key = ?1",
                params![TELEGRAM_BOT_TOKEN_KEY],
            )?;
        }

        if let Some(value) = snapshot.telegram_bot_username.as_deref() {
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![TELEGRAM_BOT_USERNAME_KEY, value, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings_text WHERE key = ?1",
                params![TELEGRAM_BOT_USERNAME_KEY],
            )?;
        }

        if let Some(chat_id) = snapshot.telegram_paired_chat_id {
            tx.execute(
                "INSERT INTO app_settings (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![TELEGRAM_PAIRED_CHAT_ID_KEY, chat_id, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings WHERE key = ?1",
                params![TELEGRAM_PAIRED_CHAT_ID_KEY],
            )?;
        }

        if let Some(value) = snapshot.telegram_topic_bindings.as_deref() {
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![TELEGRAM_TOPIC_BINDINGS_KEY, value, now],
            )?;
        } else {
            tx.execute(
                "DELETE FROM app_settings_text WHERE key = ?1",
                params![TELEGRAM_TOPIC_BINDINGS_KEY],
            )?;
        }

        tx.execute(
            "DELETE FROM app_settings_text WHERE key LIKE 'task_prompt_template_%'",
            [],
        )?;
        for (kind, template) in &snapshot.task_prompt_templates {
            let key = format!("{TASK_PROMPT_TEMPLATE_PREFIX}{kind}");
            let trimmed = template.trim();
            if trimmed.is_empty() {
                continue;
            }
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![key, trimmed, now],
            )?;
        }

        tx.execute(
            "DELETE FROM app_settings_text WHERE key LIKE 'workspace_thread_run_config_%'",
            [],
        )?;
        for ((workspace_id, thread_id), run_config) in
            &snapshot.workspace_thread_run_config_overrides
        {
            let key = format!("{WORKSPACE_THREAD_RUN_CONFIG_PREFIX}{workspace_id}_{thread_id}");
            let model_id = run_config.model_id.trim();
            let effort = run_config.thinking_effort.trim();
            if model_id.is_empty() || effort.is_empty() {
                continue;
            }
            let value = serde_json::to_string(run_config).unwrap_or_default();
            if value.trim().is_empty() {
                continue;
            }
            tx.execute(
                "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                 VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![key, value, now],
            )?;
        }

        if self.persist_ui_state {
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

            tx.execute(
                "DELETE FROM app_settings_text WHERE key LIKE 'workspace_chat_scroll_anchor_%'",
                [],
            )?;
            for ((workspace_id, thread_id), anchor) in &snapshot.workspace_chat_scroll_anchor {
                let key =
                    format!("{WORKSPACE_CHAT_SCROLL_ANCHOR_PREFIX}{workspace_id}_{thread_id}");
                let value = serde_json::to_string(anchor)
                    .context("failed to serialize chat scroll anchor")?;
                tx.execute(
                    "INSERT INTO app_settings_text (key, value, created_at, updated_at)
                     VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings_text WHERE key = ?1), ?3), ?3)
                     ON CONFLICT(key) DO UPDATE SET
                       value = excluded.value,
                       updated_at = excluded.updated_at",
                    params![key, value, now],
                )?;
            }

            tx.execute(
                "DELETE FROM app_settings WHERE key LIKE 'workspace_unread_completion_%'",
                [],
            )?;
            for (workspace_id, unread) in &snapshot.workspace_unread_completions {
                if !*unread {
                    continue;
                }
                let key = format!("{WORKSPACE_UNREAD_COMPLETION_PREFIX}{workspace_id}");
                tx.execute(
                    "INSERT INTO app_settings (key, value, created_at, updated_at)
                     VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                     ON CONFLICT(key) DO UPDATE SET
                       value = excluded.value,
                       updated_at = excluded.updated_at",
                    params![key, 1i64, now],
                )?;
            }

            tx.execute(
                "DELETE FROM app_settings WHERE key LIKE 'task_starred_%'",
                [],
            )?;
            for ((workspace_id, thread_id), starred) in &snapshot.starred_tasks {
                if !*starred {
                    continue;
                }
                let key = format!("{TASK_STARRED_PREFIX}{workspace_id}_{thread_id}");
                tx.execute(
                    "INSERT INTO app_settings (key, value, created_at, updated_at)
                     VALUES (?1, ?2, COALESCE((SELECT created_at FROM app_settings WHERE key = ?1), ?3), ?3)
                     ON CONFLICT(key) DO UPDATE SET
                       value = excluded.value,
                       updated_at = excluded.updated_at",
                    params![key, 1i64, now],
                )?;
            }
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
        let inserted = self.conn.execute(
            "INSERT INTO conversations (project_slug, workspace_name, thread_local_id, thread_id, title, task_status, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, 'backlog', ?5, ?5)
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

        if inserted > 0 {
            let entry_id = "sys_1".to_owned();
            let entry = ConversationEntry::SystemEvent {
                entry_id: entry_id.clone(),
                created_at_unix_ms: now_unix_millis(),
                event: luban_domain::ConversationSystemEvent::TaskCreated,
            };
            let payload_json =
                serde_json::to_string(&entry).context("failed to serialize conversation entry")?;
            self.conn.execute(
                    "INSERT OR IGNORE INTO conversation_entries
                     (project_slug, workspace_name, thread_local_id, seq, entry_id, kind, codex_item_id, payload_json, created_at)
                     VALUES (?1, ?2, ?3, 1, ?4, 'system_event', NULL, ?5, ?6)",
                    params![
                        project_slug,
                        workspace_name,
                        thread_local_id as i64,
                        entry_id,
                        payload_json,
                        now,
                    ],
                )?;
        }

        Ok(())
    }

    fn get_conversation_thread_id(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) -> anyhow::Result<Option<String>> {
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
        self.repair_conversation_rows_for_entries(project_slug, workspace_name)?;
        let mut stmt = self.conn.prepare(
            "SELECT c.thread_local_id,
                    c.thread_id,
                    c.title,
                    c.created_at,
                    c.updated_at,
                    c.task_status,
                    c.task_status_last_analyzed_message_seq,
                    (SELECT COALESCE(MAX(e2.seq), 0)
                     FROM conversation_entries e2
                     WHERE e2.project_slug = c.project_slug
                       AND e2.workspace_name = c.workspace_name
                       AND e2.thread_local_id = c.thread_local_id
                       AND e2.kind IN ('user_message', 'codex_item')) AS last_message_seq,
                    c.queue_paused,
                    c.run_started_at_unix_ms,
                    c.run_finished_at_unix_ms,
                    (SELECT COUNT(*)
                     FROM conversation_queued_prompts qp
                     WHERE qp.project_slug = c.project_slug
                       AND qp.workspace_name = c.workspace_name
                       AND qp.thread_local_id = c.thread_local_id) AS pending_prompt_count,
                    (SELECT e.kind
                     FROM conversation_entries e
                     WHERE e.project_slug = c.project_slug
                       AND e.workspace_name = c.workspace_name
                       AND e.thread_local_id = c.thread_local_id
                       AND e.kind IN ('turn_error', 'turn_canceled', 'turn_duration')
                     ORDER BY e.seq DESC
                     LIMIT 1) AS last_turn_kind
             FROM conversations c
             WHERE c.project_slug = ?1 AND c.workspace_name = ?2
             ORDER BY c.updated_at DESC, c.thread_local_id DESC",
        )?;
        let rows = stmt.query_map(params![project_slug, workspace_name], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, Option<i64>>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, Option<String>>(12)?,
            ))
        })?;

        let mut threads = Vec::new();
        for row in rows {
            let (
                thread_local_id,
                remote_thread_id,
                title,
                created_at,
                updated_at,
                task_status,
                task_status_last_analyzed_message_seq,
                last_message_seq,
                queue_paused,
                run_started_at_unix_ms,
                run_finished_at_unix_ms,
                pending_prompt_count,
                last_turn_kind,
            ) = row?;
            let Some(thread_local_id) = u64::try_from(thread_local_id).ok() else {
                continue;
            };
            let Some(created_at) = u64::try_from(created_at).ok() else {
                continue;
            };
            let Some(updated_at) = u64::try_from(updated_at).ok() else {
                continue;
            };
            let title = title.unwrap_or_else(|| format!("Thread {thread_local_id}"));
            let task_status = luban_domain::parse_task_status(&task_status)
                .unwrap_or(luban_domain::TaskStatus::Todo);
            let last_message_seq = u64::try_from(last_message_seq).unwrap_or_default();
            let task_status_last_analyzed_message_seq =
                u64::try_from(task_status_last_analyzed_message_seq).unwrap_or_default();
            let running = run_started_at_unix_ms.is_some() && run_finished_at_unix_ms.is_none();
            let pending_prompt_count = u64::try_from(pending_prompt_count).unwrap_or(0);
            let turn_status = if running {
                luban_domain::TurnStatus::Running
            } else if pending_prompt_count > 0 {
                if queue_paused != 0 {
                    luban_domain::TurnStatus::Paused
                } else {
                    luban_domain::TurnStatus::Awaiting
                }
            } else {
                luban_domain::TurnStatus::Idle
            };
            let last_turn_result = match last_turn_kind.as_deref() {
                Some("turn_duration") => Some(luban_domain::TurnResult::Completed),
                Some("turn_error") | Some("turn_canceled") => {
                    Some(luban_domain::TurnResult::Failed)
                }
                _ => None,
            };
            threads.push(ConversationThreadMeta {
                thread_id: WorkspaceThreadId::from_u64(thread_local_id),
                remote_thread_id,
                title,
                created_at_unix_seconds: created_at,
                updated_at_unix_seconds: updated_at,
                task_status,
                last_message_seq,
                task_status_last_analyzed_message_seq,
                turn_status,
                last_turn_result,
            });
        }

        Ok(threads)
    }

    fn repair_conversation_rows_for_entries(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
    ) -> anyhow::Result<()> {
        let thread_ids = {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT thread_local_id
                 FROM conversation_entries
                 WHERE project_slug = ?1 AND workspace_name = ?2",
            )?;
            let rows = stmt.query_map(params![project_slug, workspace_name], |row| {
                row.get::<_, i64>(0)
            })?;
            let mut thread_ids = Vec::new();
            for row in rows {
                let thread_local_id = row?;
                let Some(thread_local_id) = u64::try_from(thread_local_id).ok() else {
                    continue;
                };
                thread_ids.push(thread_local_id);
            }
            thread_ids
        };

        for thread_local_id in thread_ids {
            self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;
            self.conn.execute(
                "UPDATE conversations
                 SET updated_at = MAX(
                   updated_at,
                   COALESCE(
                     (SELECT MAX(created_at)
                      FROM conversation_entries
                      WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3),
                     updated_at
                   )
                 )
                 WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
                params![project_slug, workspace_name, thread_local_id as i64],
            )?;
        }

        Ok(())
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
            ConversationEntry::UserEvent {
                event: luban_domain::UserEvent::Message { text, .. },
                ..
            } => {
                let title = luban_domain::derive_thread_title(text);
                if title.is_empty() { None } else { Some(title) }
            }
            _ => None,
        });

        let now = now_unix_seconds();
        let tx = self.conn.transaction()?;
        let mut next_seq: i64 = tx.query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1
             FROM conversation_entries
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64],
            |row| row.get(0),
        )?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO conversation_entries
                 (project_slug, workspace_name, thread_local_id, seq, entry_id, kind, codex_item_id, payload_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for entry in entries {
                let (kind, codex_item_id, entry_id) = conversation_entry_index_fields(entry);
                let entry_id = if entry_id.is_empty() {
                    format!("e_{next_seq}")
                } else {
                    entry_id.to_owned()
                };
                let mut stored_entry = entry.clone();
                set_conversation_entry_id(&mut stored_entry, entry_id.clone());
                ensure_conversation_entry_created_at(&mut stored_entry, now_unix_millis());
                let payload_json =
                    serde_json::to_string(&stored_entry).context("failed to serialize entry")?;
                stmt.execute(params![
                    project_slug,
                    workspace_name,
                    thread_local_id as i64,
                    next_seq,
                    entry_id,
                    kind,
                    codex_item_id,
                    payload_json,
                    now
                ])?;
                next_seq += 1;
            }
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

    fn replace_conversation_entries(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        entries: &[ConversationEntry],
    ) -> anyhow::Result<()> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;

        let now = now_unix_seconds();
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM conversation_entries
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64],
        )?;

        let derived_title = entries.iter().find_map(|entry| match entry {
            ConversationEntry::UserEvent {
                event: luban_domain::UserEvent::Message { text, .. },
                ..
            } => {
                let title = luban_domain::derive_thread_title(text);
                if title.is_empty() { None } else { Some(title) }
            }
            _ => None,
        });

        if !entries.is_empty() {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO conversation_entries
                 (project_slug, workspace_name, thread_local_id, seq, entry_id, kind, codex_item_id, payload_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for (idx, entry) in entries.iter().enumerate() {
                let seq = (idx.saturating_add(1)) as i64;
                let (kind, codex_item_id, entry_id) = conversation_entry_index_fields(entry);
                let entry_id = if entry_id.is_empty() {
                    format!("e_{seq}")
                } else {
                    entry_id.to_owned()
                };
                let mut stored_entry = entry.clone();
                set_conversation_entry_id(&mut stored_entry, entry_id.clone());
                ensure_conversation_entry_created_at(&mut stored_entry, now_unix_millis());
                let payload_json =
                    serde_json::to_string(&stored_entry).context("failed to serialize entry")?;
                stmt.execute(params![
                    project_slug,
                    workspace_name,
                    thread_local_id as i64,
                    seq,
                    entry_id,
                    kind,
                    codex_item_id,
                    payload_json,
                    now
                ])?;
            }
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

    fn update_conversation_title_if_matches(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        expected_current_title: &str,
        new_title: &str,
    ) -> anyhow::Result<bool> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;
        let new_title = new_title.trim();
        if new_title.is_empty() {
            return Ok(false);
        }

        let updated = self.conn.execute(
            "UPDATE conversations
             SET title = ?5
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
               AND (title IS NULL OR title LIKE 'Thread %' OR title = ?4)
               AND COALESCE(title, '') <> ?5",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                expected_current_title,
                new_title
            ],
        )?;
        Ok(updated > 0)
    }

    fn load_conversation(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) -> anyhow::Result<ConversationSnapshot> {
        let row = self
            .conn
            .query_row(
                "SELECT title, thread_id, task_status, queue_paused, run_started_at_unix_ms, run_finished_at_unix_ms, agent_runner, agent_model_id, thinking_effort, amp_mode FROM conversations
                 WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
                params![project_slug, workspace_name, thread_local_id as i64],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                    ))
                },
            )
            .optional()
            .context("failed to load conversation meta")?;
        let Some((
            title,
            thread_id,
            task_status,
            queue_paused,
            started,
            finished,
            agent_runner,
            model_id,
            thinking_effort,
            amp_mode,
        )) = row
        else {
            return Err(SqliteStoreError::ConversationNotFound.into());
        };

        let title = title
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned);
        let thinking_effort = thinking_effort
            .as_deref()
            .and_then(luban_domain::parse_thinking_effort);
        let agent_runner = agent_runner
            .as_deref()
            .and_then(luban_domain::parse_agent_runner_kind);
        let task_status =
            luban_domain::parse_task_status(&task_status).unwrap_or(luban_domain::TaskStatus::Todo);
        let queue_paused = queue_paused != 0;
        let run_started_at_unix_ms = started.and_then(|v| u64::try_from(v).ok());
        let run_finished_at_unix_ms = finished.and_then(|v| u64::try_from(v).ok());

        let mut stmt = self.conn.prepare(
            "SELECT entry_id, payload_json
             FROM conversation_entries
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
             ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(
            params![project_slug, workspace_name, thread_local_id as i64],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?;

        let mut entries = Vec::new();
        for row in rows {
            let (entry_id, json) = row?;
            let mut entry: ConversationEntry =
                serde_json::from_str(&json).context("failed to parse entry")?;
            set_conversation_entry_id(&mut entry, entry_id);
            entries.push(entry);
        }

        let mut stmt = self.conn.prepare(
            "SELECT payload_json
             FROM conversation_queued_prompts
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
             ORDER BY seq ASC, prompt_id ASC",
        )?;
        let rows = stmt.query_map(
            params![project_slug, workspace_name, thread_local_id as i64],
            |row| row.get::<_, String>(0),
        )?;

        let mut pending_prompts = Vec::new();
        for row in rows {
            let json = row?;
            let prompt: QueuedPrompt =
                serde_json::from_str(&json).context("failed to parse queued prompt")?;
            pending_prompts.push(prompt);
        }

        let entries_total = entries.len() as u64;
        Ok(ConversationSnapshot {
            title,
            thread_id,
            task_status,
            runner: agent_runner,
            agent_model_id: model_id,
            thinking_effort,
            amp_mode,
            entries,
            entries_total,
            entries_start: 0,
            pending_prompts,
            queue_paused,
            run_started_at_unix_ms,
            run_finished_at_unix_ms,
        })
    }

    fn load_conversation_page(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        before: Option<u64>,
        limit: u64,
    ) -> anyhow::Result<ConversationSnapshot> {
        let row = self
            .conn
            .query_row(
                "SELECT title, thread_id, task_status, queue_paused, run_started_at_unix_ms, run_finished_at_unix_ms, agent_runner, agent_model_id, thinking_effort, amp_mode FROM conversations
                 WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
                params![project_slug, workspace_name, thread_local_id as i64],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                    ))
                },
            )
            .optional()
            .context("failed to load conversation meta")?;
        let Some((
            title,
            thread_id,
            task_status,
            queue_paused,
            started,
            finished,
            agent_runner,
            model_id,
            thinking_effort,
            amp_mode,
        )) = row
        else {
            return Err(SqliteStoreError::ConversationNotFound.into());
        };

        let title = title
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned);
        let thinking_effort = thinking_effort
            .as_deref()
            .and_then(luban_domain::parse_thinking_effort);
        let agent_runner = agent_runner
            .as_deref()
            .and_then(luban_domain::parse_agent_runner_kind);
        let task_status =
            luban_domain::parse_task_status(&task_status).unwrap_or(luban_domain::TaskStatus::Todo);
        let queue_paused = queue_paused != 0;
        let run_started_at_unix_ms = started.and_then(|v| u64::try_from(v).ok());
        let run_finished_at_unix_ms = finished.and_then(|v| u64::try_from(v).ok());

        let total_entries: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM conversation_entries
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64],
            |row| row.get::<_, i64>(0),
        )? as u64;

        let total_entries_usize = usize::try_from(total_entries).unwrap_or(0);
        let before_usize = before
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(total_entries_usize)
            .min(total_entries_usize);

        let limit_usize = usize::try_from(limit).unwrap_or(0);
        let end = before_usize;
        let start = end.saturating_sub(limit_usize);

        let mut entries = Vec::new();
        if end > start {
            let start_exclusive = i64::try_from(start).unwrap_or(i64::MAX);
            let end_inclusive = i64::try_from(end).unwrap_or(i64::MAX);
            let mut stmt = self.conn.prepare(
                "SELECT entry_id, payload_json
                 FROM conversation_entries
                 WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
                   AND seq > ?4 AND seq <= ?5
                 ORDER BY seq ASC",
            )?;
            let rows = stmt.query_map(
                params![
                    project_slug,
                    workspace_name,
                    thread_local_id as i64,
                    start_exclusive,
                    end_inclusive
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;

            for row in rows {
                let (entry_id, json) = row?;
                let mut entry: ConversationEntry =
                    serde_json::from_str(&json).context("failed to parse entry")?;
                set_conversation_entry_id(&mut entry, entry_id);
                entries.push(entry);
            }
        }

        let mut stmt = self.conn.prepare(
            "SELECT payload_json
             FROM conversation_queued_prompts
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
             ORDER BY seq ASC, prompt_id ASC",
        )?;
        let rows = stmt.query_map(
            params![project_slug, workspace_name, thread_local_id as i64],
            |row| row.get::<_, String>(0),
        )?;

        let mut pending_prompts = Vec::new();
        for row in rows {
            let json = row?;
            let prompt: QueuedPrompt =
                serde_json::from_str(&json).context("failed to parse queued prompt")?;
            pending_prompts.push(prompt);
        }

        Ok(ConversationSnapshot {
            title,
            thread_id,
            task_status,
            runner: agent_runner,
            agent_model_id: model_id,
            thinking_effort,
            amp_mode,
            entries,
            entries_total: total_entries,
            entries_start: start as u64,
            pending_prompts,
            queue_paused,
            run_started_at_unix_ms,
            run_finished_at_unix_ms,
        })
    }

    fn delete_conversation_thread(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM conversation_queued_prompts
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64],
        )?;
        tx.execute(
            "DELETE FROM conversation_entries
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64],
        )?;
        tx.execute(
            "DELETE FROM conversations
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64],
        )?;
        tx.execute(
            "DELETE FROM task_documents
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64],
        )?;
        tx.execute(
            "DELETE FROM task_document_events
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn save_conversation_queue_state(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        queue_paused: bool,
        run_started_at_unix_ms: Option<u64>,
        run_finished_at_unix_ms: Option<u64>,
        pending_prompts: &[QueuedPrompt],
    ) -> anyhow::Result<()> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;

        let now = now_unix_seconds();
        let next_queued_prompt_id = pending_prompts
            .iter()
            .map(|prompt| prompt.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM conversation_queued_prompts
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![project_slug, workspace_name, thread_local_id as i64],
        )?;

        if !pending_prompts.is_empty() {
            let mut stmt = tx.prepare(
                "INSERT INTO conversation_queued_prompts
                 (project_slug, workspace_name, thread_local_id, prompt_id, seq, payload_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (idx, prompt) in pending_prompts.iter().enumerate() {
                let seq = (idx.saturating_add(1)) as i64;
                let payload_json =
                    serde_json::to_string(prompt).context("failed to serialize queued prompt")?;
                stmt.execute(params![
                    project_slug,
                    workspace_name,
                    thread_local_id as i64,
                    prompt.id as i64,
                    seq,
                    payload_json,
                    now
                ])?;
            }
        }

        tx.execute(
            "UPDATE conversations
             SET queue_paused = ?4,
                 run_started_at_unix_ms = ?5,
                 run_finished_at_unix_ms = ?6,
                 next_queued_prompt_id = ?7,
                 updated_at = ?8
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                if queue_paused { 1 } else { 0 },
                run_started_at_unix_ms.map(|v| v as i64),
                run_finished_at_unix_ms.map(|v| v as i64),
                next_queued_prompt_id as i64,
                now
            ],
        )?;
        tx.commit()?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn save_conversation_run_config(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        runner: luban_domain::AgentRunnerKind,
        model_id: &str,
        thinking_effort: ThinkingEffort,
        amp_mode: Option<&str>,
    ) -> anyhow::Result<()> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;
        let now = now_unix_seconds();
        self.conn.execute(
            "UPDATE conversations
             SET agent_runner = ?4,
                 agent_model_id = ?5,
                 thinking_effort = ?6,
                 amp_mode = ?7,
                 updated_at = ?8
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                runner.as_str(),
                model_id,
                thinking_effort.as_str(),
                amp_mode,
                now
            ],
        )?;
        Ok(())
    }

    fn save_conversation_task_status(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        task_status: luban_domain::TaskStatus,
    ) -> anyhow::Result<()> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;

        let previous_status = self
            .conn
            .query_row(
                "SELECT task_status FROM conversations
                 WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
                params![project_slug, workspace_name, thread_local_id as i64],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to load existing conversation task status")?
            .and_then(|raw| luban_domain::parse_task_status(&raw));

        if previous_status.is_some_and(|status| status == task_status) {
            return Ok(());
        }

        let now = now_unix_seconds();
        self.conn.execute(
            "UPDATE conversations
             SET task_status = ?4,
                 updated_at = ?5
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                task_status.as_str(),
                now
            ],
        )?;

        if let Some(from) = previous_status {
            let id = format!(
                "sys_{}",
                self.conn
                    .query_row(
                        "SELECT COALESCE(MAX(seq), 0) + 1 FROM conversation_entries
                         WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
                        params![project_slug, workspace_name, thread_local_id as i64],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap_or(1)
            );

            let entry = ConversationEntry::SystemEvent {
                entry_id: id,
                created_at_unix_ms: now_unix_millis(),
                event: luban_domain::ConversationSystemEvent::TaskStatusChanged {
                    from,
                    to: task_status,
                },
            };
            self.append_conversation_entries(
                project_slug,
                workspace_name,
                thread_local_id,
                &[entry],
            )?;
        }

        Ok(())
    }

    fn save_conversation_task_status_last_analyzed(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) -> anyhow::Result<()> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;

        let last_message_seq: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(seq), 0)
             FROM conversation_entries
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
               AND kind IN ('user_message', 'codex_item')",
            params![project_slug, workspace_name, thread_local_id as i64],
            |row| row.get(0),
        )?;

        self.conn.execute(
            "UPDATE conversations
             SET task_status_last_analyzed_message_seq = ?4
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                last_message_seq
            ],
        )?;

        Ok(())
    }

    fn save_conversation_task_validation_pr(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        pr_number: u64,
        pr_url: Option<&str>,
    ) -> anyhow::Result<()> {
        self.ensure_conversation(project_slug, workspace_name, thread_local_id)?;

        self.conn.execute(
            "UPDATE conversations
             SET task_validation_pr_number = ?4,
                 task_validation_pr_url = ?5
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                pr_number as i64,
                pr_url
            ],
        )?;

        Ok(())
    }

    fn list_conversation_tasks_for_merged_pr(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        pr_number: u64,
    ) -> anyhow::Result<Vec<u64>> {
        let thread_ids = {
            let mut stmt = self.conn.prepare(
                "SELECT thread_local_id
                 FROM conversations
                 WHERE project_slug = ?1
                   AND workspace_name = ?2
                   AND task_status = 'validating'
                   AND task_validation_pr_number = ?3",
            )?;
            let rows = stmt.query_map(
                params![project_slug, workspace_name, pr_number as i64],
                |row| row.get::<_, i64>(0),
            )?;
            let mut out = Vec::new();
            for row in rows {
                let id = row?;
                let Some(id) = u64::try_from(id).ok() else {
                    continue;
                };
                out.push(id);
            }
            out
        };

        Ok(thread_ids)
    }

    fn upsert_task_document_index(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        index: &TaskDocumentIndex,
    ) -> anyhow::Result<()> {
        let now = now_unix_seconds();
        self.conn.execute(
            "INSERT INTO task_documents
             (project_slug, workspace_name, thread_local_id, kind, rel_path, content_hash, byte_len, updated_at_unix_ms, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT(project_slug, workspace_name, thread_local_id, kind)
             DO UPDATE SET
               rel_path = excluded.rel_path,
               content_hash = excluded.content_hash,
               byte_len = excluded.byte_len,
               updated_at_unix_ms = excluded.updated_at_unix_ms,
               updated_at = excluded.updated_at",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                index.kind.as_key(),
                index.rel_path,
                index.content_hash,
                index.byte_len as i64,
                index.updated_at_unix_ms as i64,
                now,
            ],
        )?;
        Ok(())
    }

    fn list_task_document_indices(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) -> anyhow::Result<Vec<TaskDocumentIndex>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, rel_path, content_hash, byte_len, updated_at_unix_ms
             FROM task_documents
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
             ORDER BY CASE kind WHEN 'task' THEN 1 WHEN 'plan' THEN 2 WHEN 'memory' THEN 3 ELSE 4 END ASC",
        )?;
        let rows = stmt.query_map(
            params![project_slug, workspace_name, thread_local_id as i64],
            |row| {
                let kind = row.get::<_, String>(0)?;
                let rel_path = row.get::<_, String>(1)?;
                let content_hash = row.get::<_, String>(2)?;
                let byte_len = row.get::<_, i64>(3)?;
                let updated_at_unix_ms = row.get::<_, i64>(4)?;
                Ok((kind, rel_path, content_hash, byte_len, updated_at_unix_ms))
            },
        )?;

        let mut out = Vec::new();
        for row in rows {
            let (kind, rel_path, content_hash, byte_len, updated_at_unix_ms) = row?;
            let Some(kind) = TaskDocumentKind::parse_key(&kind) else {
                continue;
            };
            let Some(byte_len) = u64::try_from(byte_len).ok() else {
                continue;
            };
            let Some(updated_at_unix_ms) = u64::try_from(updated_at_unix_ms).ok() else {
                continue;
            };
            out.push(TaskDocumentIndex {
                kind,
                rel_path,
                content_hash,
                byte_len,
                updated_at_unix_ms,
            });
        }
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_task_document_event(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
        kind: TaskDocumentKind,
        event_type: TaskDocumentEventType,
        rel_path: &str,
        content_hash: &str,
        byte_len: u64,
        created_at_unix_ms: u64,
    ) -> anyhow::Result<u64> {
        let now = now_unix_seconds();
        self.conn.execute(
            "INSERT INTO task_document_events
             (project_slug, workspace_name, thread_local_id, kind, event_type, rel_path, content_hash, byte_len, created_at_unix_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                project_slug,
                workspace_name,
                thread_local_id as i64,
                kind.as_key(),
                event_type.as_key(),
                rel_path,
                content_hash,
                byte_len as i64,
                created_at_unix_ms as i64,
                now,
            ],
        )?;
        Ok(self.conn.last_insert_rowid() as u64)
    }

    fn list_task_document_events(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        thread_local_id: u64,
    ) -> anyhow::Result<Vec<TaskDocumentEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT event_id, kind, event_type, rel_path, content_hash, byte_len, created_at_unix_ms
             FROM task_document_events
             WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3
             ORDER BY event_id ASC",
        )?;
        let rows = stmt.query_map(
            params![project_slug, workspace_name, thread_local_id as i64],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )?;

        let mut out = Vec::new();
        for row in rows {
            let (event_id, kind, event_type, rel_path, content_hash, byte_len, created_at_unix_ms) =
                row?;
            let Some(event_id) = u64::try_from(event_id).ok() else {
                continue;
            };
            let Some(kind) = TaskDocumentKind::parse_key(&kind) else {
                continue;
            };
            let Some(event_type) = TaskDocumentEventType::parse_key(&event_type) else {
                continue;
            };
            let Some(byte_len) = u64::try_from(byte_len).ok() else {
                continue;
            };
            let Some(created_at_unix_ms) = u64::try_from(created_at_unix_ms).ok() else {
                continue;
            };
            out.push(TaskDocumentEvent {
                event_id,
                kind,
                event_type,
                rel_path,
                content_hash,
                byte_len,
                created_at_unix_ms,
            });
        }
        Ok(out)
    }

    fn insert_context_item(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        attachment: &AttachmentRef,
        created_at_unix_ms: u64,
    ) -> anyhow::Result<u64> {
        let kind = match attachment.kind {
            AttachmentKind::Image => "image",
            AttachmentKind::Text => "text",
            AttachmentKind::File => "file",
        };

        self.conn.execute(
            "INSERT INTO context_items
             (project_slug, workspace_name, attachment_id, kind, name, extension, mime, byte_len, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                project_slug,
                workspace_name,
                attachment.id,
                kind,
                attachment.name,
                attachment.extension,
                attachment.mime,
                attachment.byte_len as i64,
                created_at_unix_ms as i64,
            ],
        )?;

        Ok(self.conn.last_insert_rowid() as u64)
    }

    fn list_context_items(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
    ) -> anyhow::Result<Vec<ContextItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, attachment_id, kind, name, extension, mime, byte_len, created_at_ms
             FROM context_items
             WHERE project_slug = ?1 AND workspace_name = ?2
             ORDER BY created_at_ms DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![project_slug, workspace_name], |row| {
            let id = row.get::<_, i64>(0)? as u64;
            let attachment_id = row.get::<_, String>(1)?;
            let kind_str = row.get::<_, String>(2)?;
            let name = row.get::<_, String>(3)?;
            let extension = row.get::<_, String>(4)?;
            let mime = row.get::<_, Option<String>>(5)?;
            let byte_len = row.get::<_, i64>(6)? as u64;
            let created_at_unix_ms = row.get::<_, i64>(7)? as u64;

            let kind = match kind_str.as_str() {
                "image" => AttachmentKind::Image,
                "text" => AttachmentKind::Text,
                "file" => AttachmentKind::File,
                _ => AttachmentKind::File,
            };

            Ok(ContextItem {
                id,
                attachment: AttachmentRef {
                    id: attachment_id,
                    kind,
                    name,
                    extension,
                    mime,
                    byte_len,
                },
                created_at_unix_ms,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn delete_context_item(
        &mut self,
        project_slug: &str,
        workspace_name: &str,
        context_id: u64,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM context_items
             WHERE project_slug = ?1 AND workspace_name = ?2 AND id = ?3",
            params![project_slug, workspace_name, context_id as i64],
        )?;
        Ok(())
    }

    fn list_new_task_drafts(&mut self) -> anyhow::Result<Vec<luban_domain::NewTaskDraft>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, project_id, workspace_id, created_at_ms, updated_at_ms
             FROM new_task_drafts
             ORDER BY updated_at_ms DESC, created_at_ms DESC, id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id = row.get::<_, String>(0)?;
            let text = row.get::<_, String>(1)?;
            let project_id = row.get::<_, Option<String>>(2)?;
            let workspace_id = row
                .get::<_, Option<i64>>(3)?
                .and_then(|v| u64::try_from(v).ok());
            let created_at_unix_ms = row.get::<_, i64>(4)? as u64;
            let updated_at_unix_ms = row.get::<_, i64>(5)? as u64;
            Ok(luban_domain::NewTaskDraft {
                id,
                text,
                project_id,
                workspace_id,
                created_at_unix_ms,
                updated_at_unix_ms,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn create_new_task_draft(
        &mut self,
        text: &str,
        project_id: Option<&str>,
        workspace_id: Option<u64>,
    ) -> anyhow::Result<luban_domain::NewTaskDraft> {
        let now = now_unix_millis();
        let mut bytes = [0u8; 16];
        OsRng.fill_bytes(&mut bytes);
        let id = hex_lower(&bytes);

        self.conn.execute(
            "INSERT INTO new_task_drafts
             (id, text, project_id, workspace_id, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                text,
                project_id,
                workspace_id.map(|v| v as i64),
                now as i64,
                now as i64
            ],
        )?;

        Ok(luban_domain::NewTaskDraft {
            id,
            text: text.to_owned(),
            project_id: project_id.map(str::to_owned),
            workspace_id,
            created_at_unix_ms: now,
            updated_at_unix_ms: now,
        })
    }

    fn update_new_task_draft(
        &mut self,
        draft_id: &str,
        text: &str,
        project_id: Option<&str>,
        workspace_id: Option<u64>,
    ) -> anyhow::Result<luban_domain::NewTaskDraft> {
        let (created_at_unix_ms,): (i64,) = self
            .conn
            .query_row(
                "SELECT created_at_ms FROM new_task_drafts WHERE id = ?1",
                params![draft_id],
                |row| Ok((row.get::<_, i64>(0)?,)),
            )
            .optional()
            .context("failed to load new task draft")?
            .ok_or_else(|| anyhow!("draft not found"))?;

        let now = now_unix_millis();
        let updated = self.conn.execute(
            "UPDATE new_task_drafts
             SET text = ?2,
                 project_id = ?3,
                 workspace_id = ?4,
                 updated_at_ms = ?5
             WHERE id = ?1",
            params![
                draft_id,
                text,
                project_id,
                workspace_id.map(|v| v as i64),
                now as i64
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("draft not found"));
        }

        Ok(luban_domain::NewTaskDraft {
            id: draft_id.to_owned(),
            text: text.to_owned(),
            project_id: project_id.map(str::to_owned),
            workspace_id,
            created_at_unix_ms: created_at_unix_ms as u64,
            updated_at_unix_ms: now,
        })
    }

    fn delete_new_task_draft(&mut self, draft_id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM new_task_drafts WHERE id = ?1",
            params![draft_id],
        )?;
        Ok(())
    }

    fn load_new_task_stash(&mut self) -> anyhow::Result<Option<luban_domain::NewTaskStash>> {
        self.conn
            .query_row(
                "SELECT text, project_id, workspace_id, editing_draft_id, updated_at_ms
                 FROM new_task_stash
                 WHERE id = 1",
                [],
                |row| {
                    let text = row.get::<_, String>(0)?;
                    let project_id = row.get::<_, Option<String>>(1)?;
                    let workspace_id = row
                        .get::<_, Option<i64>>(2)?
                        .and_then(|v| u64::try_from(v).ok());
                    let editing_draft_id = row.get::<_, Option<String>>(3)?;
                    let updated_at_unix_ms = row.get::<_, i64>(4)? as u64;
                    Ok(luban_domain::NewTaskStash {
                        text,
                        project_id,
                        workspace_id,
                        editing_draft_id,
                        updated_at_unix_ms,
                    })
                },
            )
            .optional()
            .context("failed to load new task stash")
    }

    fn save_new_task_stash(&mut self, stash: &luban_domain::NewTaskStash) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO new_task_stash
             (id, text, project_id, workspace_id, editing_draft_id, updated_at_ms)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                text = excluded.text,
                project_id = excluded.project_id,
                workspace_id = excluded.workspace_id,
                editing_draft_id = excluded.editing_draft_id,
                updated_at_ms = excluded.updated_at_ms",
            params![
                stash.text,
                stash.project_id,
                stash.workspace_id.map(|v| v as i64),
                stash.editing_draft_id,
                stash.updated_at_unix_ms as i64
            ],
        )?;
        Ok(())
    }

    fn clear_new_task_stash(&mut self) -> anyhow::Result<()> {
        self.conn
            .execute("DELETE FROM new_task_stash WHERE id = 1", [])?;
        Ok(())
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
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
        if *version == 17 {
            migrate_conversation_entries_v17(conn)
                .with_context(|| "failed to migrate conversation entry payloads to v2")?;
        }
        conn.pragma_update(None, "user_version", *version as i64)
            .context("failed to update user_version")?;
        current = *version;
    }

    conn.execute_batch("COMMIT;")
        .context("failed to commit migration transaction")?;
    Ok(())
}

fn migrate_conversation_entries_v17(conn: &mut Connection) -> anyhow::Result<()> {
    #[derive(Debug, serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum LegacyConversationEntry {
        SystemEvent {
            id: String,
            created_at_unix_ms: u64,
            event: luban_domain::ConversationSystemEvent,
        },
        UserMessage {
            text: String,
            #[serde(default)]
            attachments: Vec<AttachmentRef>,
        },
        CodexItem {
            item: Box<luban_domain::CodexThreadItem>,
        },
        TurnUsage {
            usage: Option<luban_domain::CodexUsage>,
        },
        TurnDuration {
            duration_ms: u64,
        },
        TurnCanceled,
        TurnError {
            message: String,
        },
    }

    fn legacy_to_v2(entry: LegacyConversationEntry) -> ConversationEntry {
        match entry {
            LegacyConversationEntry::SystemEvent {
                id,
                created_at_unix_ms,
                event,
            } => ConversationEntry::SystemEvent {
                entry_id: id,
                created_at_unix_ms,
                event,
            },
            LegacyConversationEntry::UserMessage { text, attachments } => {
                ConversationEntry::UserEvent {
                    entry_id: String::new(),
                    created_at_unix_ms: 0,
                    event: luban_domain::UserEvent::Message { text, attachments },
                }
            }
            LegacyConversationEntry::CodexItem { item } => match *item {
                luban_domain::CodexThreadItem::AgentMessage { id, text } => {
                    ConversationEntry::AgentEvent {
                        entry_id: String::new(),
                        created_at_unix_ms: 0,
                        runner: None,
                        event: luban_domain::AgentEvent::Message { id, text },
                    }
                }
                other => ConversationEntry::AgentEvent {
                    entry_id: String::new(),
                    created_at_unix_ms: 0,
                    runner: None,
                    event: luban_domain::AgentEvent::Item {
                        item: Box::new(other),
                    },
                },
            },
            LegacyConversationEntry::TurnUsage { usage } => ConversationEntry::AgentEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                runner: None,
                event: luban_domain::AgentEvent::TurnUsage { usage },
            },
            LegacyConversationEntry::TurnDuration { duration_ms } => {
                ConversationEntry::AgentEvent {
                    entry_id: String::new(),
                    created_at_unix_ms: 0,
                    runner: None,
                    event: luban_domain::AgentEvent::TurnDuration { duration_ms },
                }
            }
            LegacyConversationEntry::TurnCanceled => ConversationEntry::AgentEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                runner: None,
                event: luban_domain::AgentEvent::TurnCanceled,
            },
            LegacyConversationEntry::TurnError { message } => ConversationEntry::AgentEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                runner: None,
                event: luban_domain::AgentEvent::TurnError { message },
            },
        }
    }

    let mut select = conn.prepare("SELECT rowid, payload_json FROM conversation_entries")?;
    let mut rows = select.query([])?;

    let mut updates: Vec<(i64, String)> = Vec::new();
    while let Some(row) = rows.next()? {
        let row_id: i64 = row.get(0)?;
        let payload_json: String = row.get(1)?;

        let parsed: serde_json::Value = serde_json::from_str(&payload_json)
            .with_context(|| format!("invalid conversation entry json (rowid={row_id})"))?;
        let Some(kind) = parsed
            .get("type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
        else {
            return Err(anyhow!(
                "conversation entry missing type tag (rowid={row_id})"
            ));
        };
        if matches!(kind.as_str(), "system_event" | "user_event" | "agent_event") {
            continue;
        }

        let legacy: LegacyConversationEntry =
            serde_json::from_value(parsed).with_context(|| {
                format!("unknown conversation entry type '{kind}' (rowid={row_id})")
            })?;
        let migrated = legacy_to_v2(legacy);
        let out = serde_json::to_string(&migrated).context("failed to serialize migrated entry")?;
        updates.push((row_id, out));
    }

    if updates.is_empty() {
        return Ok(());
    }

    let mut stmt =
        conn.prepare("UPDATE conversation_entries SET payload_json = ?1 WHERE rowid = ?2")?;
    for (row_id, payload_json) in updates {
        stmt.execute(params![payload_json, row_id])?;
    }

    Ok(())
}

fn now_unix_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn now_unix_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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

fn conversation_entry_index_fields(
    entry: &ConversationEntry,
) -> (&'static str, Option<&str>, &str) {
    match entry {
        ConversationEntry::SystemEvent { entry_id, .. } => {
            ("system_event", None, entry_id.as_str())
        }
        ConversationEntry::UserEvent {
            entry_id, event, ..
        } => match event {
            luban_domain::UserEvent::Message { .. } => ("user_message", None, entry_id.as_str()),
            luban_domain::UserEvent::TerminalCommandStarted { .. } => {
                ("terminal_command_started", None, entry_id.as_str())
            }
            luban_domain::UserEvent::TerminalCommandFinished { .. } => {
                ("terminal_command_finished", None, entry_id.as_str())
            }
        },
        ConversationEntry::AgentEvent {
            entry_id, event, ..
        } => match event {
            luban_domain::AgentEvent::Message { id, .. } => {
                ("codex_item", Some(id.as_str()), entry_id.as_str())
            }
            luban_domain::AgentEvent::Item { item } => (
                "codex_item",
                Some(codex_item_id(item.as_ref())),
                entry_id.as_str(),
            ),
            luban_domain::AgentEvent::TurnUsage { .. } => ("turn_usage", None, entry_id.as_str()),
            luban_domain::AgentEvent::TurnDuration { .. } => {
                ("turn_duration", None, entry_id.as_str())
            }
            luban_domain::AgentEvent::TurnCanceled => ("turn_canceled", None, entry_id.as_str()),
            luban_domain::AgentEvent::TurnError { .. } => ("turn_error", None, entry_id.as_str()),
        },
    }
}

fn set_conversation_entry_id(entry: &mut ConversationEntry, entry_id: String) {
    match entry {
        ConversationEntry::SystemEvent { entry_id: slot, .. } => *slot = entry_id,
        ConversationEntry::UserEvent { entry_id: slot, .. } => *slot = entry_id,
        ConversationEntry::AgentEvent { entry_id: slot, .. } => *slot = entry_id,
    }
}

fn ensure_conversation_entry_created_at(entry: &mut ConversationEntry, created_at_unix_ms: u64) {
    match entry {
        ConversationEntry::SystemEvent { .. } => {}
        ConversationEntry::UserEvent {
            created_at_unix_ms: slot,
            ..
        } => {
            if *slot == 0 {
                *slot = created_at_unix_ms;
            }
        }
        ConversationEntry::AgentEvent {
            created_at_unix_ms: slot,
            ..
        } => {
            if *slot == 0 {
                *slot = created_at_unix_ms;
            }
        }
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
    use luban_domain::{
        AgentRunConfig, ChatScrollAnchor, CodexThreadItem, PersistedProject, PersistedWorkspace,
        QueuedPrompt, ThinkingEffort,
    };
    use std::path::Path;

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

    fn open_db(path: &Path) -> SqliteDatabase {
        SqliteDatabase::open(path, SqliteStoreOptions::default()).unwrap()
    }

    #[test]
    fn migrations_create_schema() {
        let path = temp_db_path("migrations_create_schema");
        let db = open_db(&path);

        let count: i64 = db
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('projects','workspaces','conversations','conversation_entries','conversation_queued_prompts','app_settings','app_settings_text','context_items')",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
        assert_eq!(count, 8);
    }

    #[test]
    fn migrations_reopen_does_not_fail() {
        let path = temp_db_path("migrations_reopen_does_not_fail");
        {
            let _db = open_db(&path);
        }

        let db = open_db(&path);
        let version: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version as u32, LATEST_SCHEMA_VERSION);
    }

    #[test]
    fn task_status_last_analyzed_tracks_last_message_seq() {
        let path = temp_db_path("task_status_last_analyzed_tracks_last_message_seq");
        let mut db = open_db(&path);

        db.ensure_conversation("p", "w", 1).unwrap();
        db.append_conversation_entries(
            "p",
            "w",
            1,
            &[ConversationEntry::UserEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                event: luban_domain::UserEvent::Message {
                    text: "hello".to_owned(),
                    attachments: Vec::new(),
                },
            }],
        )
        .unwrap();

        let threads = db.list_conversation_threads("p", "w").unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].last_message_seq, 2);
        assert_eq!(threads[0].task_status_last_analyzed_message_seq, 0);

        db.save_conversation_task_status_last_analyzed("p", "w", 1)
            .unwrap();

        let threads = db.list_conversation_threads("p", "w").unwrap();
        assert_eq!(threads[0].task_status_last_analyzed_message_seq, 2);

        db.append_conversation_entries(
            "p",
            "w",
            1,
            &[ConversationEntry::AgentEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                runner: None,
                event: luban_domain::AgentEvent::Message {
                    id: "m1".to_owned(),
                    text: "hi".to_owned(),
                },
            }],
        )
        .unwrap();

        let threads = db.list_conversation_threads("p", "w").unwrap();
        assert_eq!(threads[0].last_message_seq, 3);
        assert_eq!(threads[0].task_status_last_analyzed_message_seq, 2);

        db.save_conversation_task_status_last_analyzed("p", "w", 1)
            .unwrap();

        let threads = db.list_conversation_threads("p", "w").unwrap();
        assert_eq!(threads[0].task_status_last_analyzed_message_seq, 3);
    }

    #[test]
    fn task_validation_pr_can_be_listed_on_merge() {
        let path = temp_db_path("task_validation_pr_can_be_listed_on_merge");
        let mut db = open_db(&path);

        db.ensure_conversation("p", "w", 1).unwrap();
        db.save_conversation_task_status("p", "w", 1, luban_domain::TaskStatus::Validating)
            .unwrap();
        db.save_conversation_task_validation_pr(
            "p",
            "w",
            1,
            123,
            Some("https://github.com/acme/repo/pull/123"),
        )
        .unwrap();

        let matching = db
            .list_conversation_tasks_for_merged_pr("p", "w", 124)
            .unwrap();
        assert!(matching.is_empty());

        let matching = db
            .list_conversation_tasks_for_merged_pr("p", "w", 123)
            .unwrap();
        assert_eq!(matching, vec![1]);

        let threads = db.list_conversation_threads("p", "w").unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].task_status, luban_domain::TaskStatus::Validating);
    }

    #[test]
    fn list_conversation_threads_does_not_autocreate_threads() {
        let path = temp_db_path("list_conversation_threads_does_not_autocreate_threads");
        let mut db = open_db(&path);

        let threads = db.list_conversation_threads("p", "w").unwrap();
        assert!(
            threads.is_empty(),
            "expected no threads for a new workspace"
        );

        let err = db
            .load_conversation_page("p", "w", 1, None, 10)
            .unwrap_err();
        assert!(
            err.downcast_ref::<SqliteStoreError>() == Some(&SqliteStoreError::ConversationNotFound),
            "expected missing conversation to return ConversationNotFound"
        );
    }

    #[test]
    fn delete_conversation_thread_removes_conversation_and_entries() {
        let path = temp_db_path("delete_conversation_thread_removes_conversation_and_entries");
        let mut db = open_db(&path);

        db.ensure_conversation("p", "w", 1).unwrap();
        db.append_conversation_entries(
            "p",
            "w",
            1,
            &[ConversationEntry::UserEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                event: luban_domain::UserEvent::Message {
                    text: "hello".to_owned(),
                    attachments: Vec::new(),
                },
            }],
        )
        .unwrap();

        db.save_conversation_queue_state("p", "w", 1, false, None, None, &[])
            .unwrap();

        let threads = db.list_conversation_threads("p", "w").unwrap();
        assert_eq!(threads.len(), 1);

        db.delete_conversation_thread("p", "w", 1).unwrap();

        let threads = db.list_conversation_threads("p", "w").unwrap();
        assert!(threads.is_empty());

        let err = db
            .load_conversation_page("p", "w", 1, None, 10)
            .unwrap_err();
        assert!(
            err.downcast_ref::<SqliteStoreError>() == Some(&SqliteStoreError::ConversationNotFound),
            "expected deleted conversation to return ConversationNotFound"
        );

        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversation_entries WHERE project_slug = 'p' AND workspace_name = 'w' AND thread_local_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn load_conversation_page_includes_title() {
        let path = temp_db_path("load_conversation_page_includes_title");
        let mut db = open_db(&path);

        db.ensure_conversation("p", "w", 1).unwrap();

        let snapshot = db.load_conversation_page("p", "w", 1, None, 100).unwrap();
        assert_eq!(snapshot.title.as_deref(), Some("Thread 1"));

        db.append_conversation_entries(
            "p",
            "w",
            1,
            &[ConversationEntry::UserEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                event: luban_domain::UserEvent::Message {
                    text: "Hello world".to_owned(),
                    attachments: Vec::new(),
                },
            }],
        )
        .unwrap();

        let snapshot = db.load_conversation_page("p", "w", 1, None, 100).unwrap();
        assert_eq!(snapshot.title.as_deref(), Some("Hello world"));
    }

    fn create_db_at_schema_version(path: &Path, target_version: u32) {
        let mut conn = Connection::open(path).unwrap();
        configure_connection(&mut conn).unwrap();

        let current: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(current as u32, 0);

        conn.execute_batch("BEGIN IMMEDIATE;").unwrap();
        for (version, sql) in MIGRATIONS {
            if *version > target_version {
                break;
            }
            conn.execute_batch(sql).unwrap();
            conn.pragma_update(None, "user_version", *version as i64)
                .unwrap();
        }
        conn.execute_batch("COMMIT;").unwrap();

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version as u32, target_version);
    }

    #[test]
    fn migrations_upgrade_v10_database_in_place() {
        let path = temp_db_path("migrations_upgrade_v10_database_in_place");
        create_db_at_schema_version(&path, 10);

        let mut db = open_db(&path);
        let version: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version as u32, LATEST_SCHEMA_VERSION);

        let columns = db
            .conn
            .prepare("PRAGMA table_info(workspaces)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(!columns.iter().any(|c| c == "branch_name"));
        assert!(!columns.iter().any(|c| c == "branch_renamed"));

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "p".to_owned(),
                name: "P".to_owned(),
                path: PathBuf::from("/tmp/p"),
                is_git: true,
                expanded: false,
                workspaces: vec![PersistedWorkspace {
                    id: 2,
                    workspace_name: "w".to_owned(),
                    branch_name: "w".to_owned(),
                    worktree_path: PathBuf::from("/tmp/p/worktrees/w"),
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
            agent_runner_default_models: HashMap::new(),
            agent_default_thinking_effort: None,
            agent_default_runner: None,
            agent_amp_mode: None,
            agent_codex_enabled: Some(true),
            agent_amp_enabled: Some(true),
            agent_claude_enabled: Some(true),
            agent_droid_enabled: Some(true),
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            starred_tasks: HashMap::new(),
            task_prompt_templates: HashMap::new(),
            telegram_enabled: None,
            telegram_bot_token: None,
            telegram_bot_username: None,
            telegram_paired_chat_id: None,
            telegram_topic_bindings: None,
        };

        db.save_app_state(&snapshot).unwrap();
        let loaded = db.load_app_state().unwrap();
        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn migrations_upgrade_v16_migrates_conversation_entry_payloads_v17() {
        let path = temp_db_path("migrations_upgrade_v16_migrates_conversation_entry_payloads_v17");
        create_db_at_schema_version(&path, 16);

        let mut conn = Connection::open(&path).unwrap();
        configure_connection(&mut conn).unwrap();

        let now = now_unix_seconds();
        conn.execute(
            "INSERT INTO conversations (project_slug, workspace_name, thread_local_id, thread_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, 't', ?4, ?4)",
            params!["p", "w", 1i64, now],
        )
        .unwrap();

        let legacy_payload = serde_json::json!({
            "type": "user_message",
            "text": "Hello",
            "attachments": [],
        })
        .to_string();
        conn.execute(
            "INSERT INTO conversation_entries (project_slug, workspace_name, thread_local_id, seq, kind, codex_item_id, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, 'user_message', NULL, ?5, ?6)",
            params!["p", "w", 1i64, 1i64, legacy_payload, now],
        )
        .unwrap();
        drop(conn);

        let mut db = open_db(&path);
        let migrated_payload: String = db
            .conn
            .query_row(
                "SELECT payload_json FROM conversation_entries
                 WHERE project_slug = ?1 AND workspace_name = ?2 AND thread_local_id = ?3 AND seq = 1",
                params!["p", "w", 1i64],
                |row| row.get(0),
            )
            .unwrap();
        let migrated_json: serde_json::Value = serde_json::from_str(&migrated_payload).unwrap();
        assert_eq!(
            migrated_json.get("type").and_then(|v| v.as_str()),
            Some("user_event")
        );

        let snapshot = db.load_conversation("p", "w", 1).unwrap();
        assert!(matches!(
            snapshot.entries.as_slice(),
            [ConversationEntry::UserEvent {
                event: luban_domain::UserEvent::Message { text, .. },
                ..
            }, ..] if text == "Hello"
        ));
    }

    #[test]
    fn save_and_load_app_state_roundtrips() {
        let path = temp_db_path("save_and_load_app_state_roundtrips");
        let mut db = open_db(&path);

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "my-project".to_owned(),
                name: "My Project".to_owned(),
                path: PathBuf::from("/tmp/my-project"),
                is_git: true,
                expanded: true,
                workspaces: vec![PersistedWorkspace {
                    id: 10,
                    workspace_name: "alpha".to_owned(),
                    branch_name: "alpha".to_owned(),
                    worktree_path: PathBuf::from("/tmp/my-project/worktrees/alpha"),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                }],
            }],
            sidebar_width: Some(280),
            terminal_pane_width: Some(360),
            global_zoom_percent: Some(110),
            appearance_theme: Some("dark".to_owned()),
            appearance_ui_font: Some("Inter".to_owned()),
            appearance_chat_font: Some("Inter".to_owned()),
            appearance_code_font: Some("Geist Mono".to_owned()),
            appearance_terminal_font: Some("Geist Mono".to_owned()),
            agent_default_model_id: Some("gpt-5.2".to_owned()),
            agent_runner_default_models: HashMap::new(),
            agent_default_thinking_effort: Some("high".to_owned()),
            agent_default_runner: Some("amp".to_owned()),
            agent_amp_mode: Some("rush".to_owned()),
            agent_codex_enabled: Some(true),
            agent_amp_enabled: Some(true),
            agent_claude_enabled: Some(true),
            agent_droid_enabled: Some(true),
            last_open_workspace_id: Some(10),
            open_button_selection: None,
            sidebar_project_order: vec!["/tmp/my-project".to_owned()],
            workspace_active_thread_id: HashMap::from([(10, 1)]),
            workspace_open_tabs: HashMap::from([(10, vec![1, 2, 3])]),
            workspace_archived_tabs: HashMap::from([(10, vec![9, 8])]),
            workspace_next_thread_id: HashMap::from([(10, 4)]),
            workspace_chat_scroll_y10: HashMap::from([((10, 1), -1234)]),
            workspace_chat_scroll_anchor: HashMap::from([(
                (10, 1),
                ChatScrollAnchor::Block {
                    block_id: "history-block-agent-turn-3".to_owned(),
                    block_index: 3,
                    offset_in_block_y10: 420,
                },
            )]),
            workspace_unread_completions: HashMap::from([(10, true)]),
            workspace_thread_run_config_overrides: HashMap::from([(
                (10, 2),
                luban_domain::PersistedWorkspaceThreadRunConfigOverride {
                    runner: Some("amp".to_owned()),
                    amp_mode: Some("rush".to_owned()),
                    model_id: "gpt-5.3-codex".to_owned(),
                    thinking_effort: "high".to_owned(),
                },
            )]),
            starred_tasks: HashMap::from([((10, 2), true)]),
            task_prompt_templates: HashMap::from([(
                "fix".to_owned(),
                "Fix issue template override".to_owned(),
            )]),
            telegram_enabled: None,
            telegram_bot_token: None,
            telegram_bot_username: None,
            telegram_paired_chat_id: None,
            telegram_topic_bindings: None,
        };

        db.save_app_state(&snapshot).unwrap();
        let loaded = db.load_app_state().unwrap();
        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn conversation_append_is_idempotent_by_codex_item_id() {
        let path = temp_db_path("conversation_append_is_idempotent_by_codex_item_id");
        let mut db = open_db(&path);

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "p".to_owned(),
                name: "P".to_owned(),
                path: PathBuf::from("/tmp/p"),
                is_git: true,
                expanded: false,
                workspaces: vec![PersistedWorkspace {
                    id: 2,
                    workspace_name: "w".to_owned(),
                    branch_name: "w".to_owned(),
                    worktree_path: PathBuf::from("/tmp/p/worktrees/w"),
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
            agent_runner_default_models: HashMap::new(),
            agent_default_thinking_effort: None,
            agent_default_runner: None,
            agent_amp_mode: None,
            agent_codex_enabled: Some(true),
            agent_amp_enabled: Some(true),
            agent_claude_enabled: Some(true),
            agent_droid_enabled: Some(true),
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            starred_tasks: HashMap::new(),
            task_prompt_templates: HashMap::new(),
            telegram_enabled: None,
            telegram_bot_token: None,
            telegram_bot_username: None,
            telegram_paired_chat_id: None,
            telegram_topic_bindings: None,
        };
        db.save_app_state(&snapshot).unwrap();

        db.ensure_conversation("p", "w", 1).unwrap();

        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };
        let entry = match item {
            CodexThreadItem::AgentMessage { id, text } => ConversationEntry::AgentEvent {
                entry_id: "e_1".to_owned(),
                created_at_unix_ms: 0,
                runner: None,
                event: luban_domain::AgentEvent::Message { id, text },
            },
            other => ConversationEntry::AgentEvent {
                entry_id: "e_1".to_owned(),
                created_at_unix_ms: 0,
                runner: None,
                event: luban_domain::AgentEvent::Item {
                    item: Box::new(other),
                },
            },
        };

        db.append_conversation_entries("p", "w", 1, std::slice::from_ref(&entry))
            .unwrap();
        db.append_conversation_entries("p", "w", 1, std::slice::from_ref(&entry))
            .unwrap();

        let snapshot = db.load_conversation("p", "w", 1).unwrap();
        let count = snapshot
            .entries
            .iter()
            .filter(|e| matches!(e, ConversationEntry::AgentEvent { .. }))
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn conversation_title_update_is_conditionally_applied() {
        let path = temp_db_path("conversation_title_update_is_conditionally_applied");
        let mut db = open_db(&path);

        db.ensure_conversation("p", "w", 1).unwrap();

        let updated = db
            .update_conversation_title_if_matches("p", "w", 1, "Derived", "AI Title")
            .unwrap();
        assert!(updated);

        let updated = db
            .update_conversation_title_if_matches("p", "w", 1, "Derived", "Other")
            .unwrap();
        assert!(!updated);

        let updated = db
            .update_conversation_title_if_matches("p", "w", 1, "AI Title", "Better")
            .unwrap();
        assert!(updated);

        let updated = db
            .update_conversation_title_if_matches("p", "w", 1, "Better", "Better")
            .unwrap();
        assert!(!updated);
    }

    #[test]
    fn conversation_queue_state_round_trip() {
        let path = temp_db_path("conversation_queue_state_round_trip");
        let mut db = open_db(&path);

        db.ensure_conversation("p", "w", 1).unwrap();

        let prompts = vec![
            QueuedPrompt {
                id: 2,
                text: "queued-a".to_owned(),
                attachments: Vec::new(),
                run_config: AgentRunConfig {
                    runner: luban_domain::AgentRunnerKind::Codex,
                    model_id: "gpt-5.3-codex".to_owned(),
                    thinking_effort: ThinkingEffort::Minimal,
                    amp_mode: None,
                },
            },
            QueuedPrompt {
                id: 7,
                text: "queued-b".to_owned(),
                attachments: Vec::new(),
                run_config: AgentRunConfig {
                    runner: luban_domain::AgentRunnerKind::Codex,
                    model_id: "gpt-5.3-codex".to_owned(),
                    thinking_effort: ThinkingEffort::Minimal,
                    amp_mode: None,
                },
            },
        ];

        db.save_conversation_queue_state("p", "w", 1, true, None, None, &prompts)
            .unwrap();

        let snapshot = db.load_conversation("p", "w", 1).unwrap();
        assert!(snapshot.queue_paused);
        assert_eq!(snapshot.pending_prompts.len(), 2);
        assert_eq!(snapshot.pending_prompts[0].id, 2);
        assert_eq!(snapshot.pending_prompts[0].text, "queued-a");
        assert_eq!(snapshot.pending_prompts[1].id, 7);
        assert_eq!(snapshot.pending_prompts[1].text, "queued-b");
    }

    #[test]
    fn conversation_run_timing_round_trip() {
        let path = temp_db_path("conversation_run_timing_round_trip");
        let mut db = open_db(&path);

        db.ensure_conversation("p", "w", 1).unwrap();

        db.save_conversation_queue_state("p", "w", 1, false, Some(10), None, &[])
            .unwrap();

        let snapshot = db.load_conversation("p", "w", 1).unwrap();
        assert_eq!(snapshot.run_started_at_unix_ms, Some(10));
        assert_eq!(snapshot.run_finished_at_unix_ms, None);

        db.save_conversation_queue_state("p", "w", 1, false, Some(10), Some(42), &[])
            .unwrap();
        let snapshot = db.load_conversation_page("p", "w", 1, None, 10).unwrap();
        assert_eq!(snapshot.run_started_at_unix_ms, Some(10));
        assert_eq!(snapshot.run_finished_at_unix_ms, Some(42));
    }

    #[test]
    fn conversation_run_config_round_trip() {
        let path = temp_db_path("conversation_run_config_round_trip");
        let mut db = open_db(&path);

        db.ensure_conversation("p", "w", 1).unwrap();

        db.save_conversation_run_config(
            "p",
            "w",
            1,
            luban_domain::AgentRunnerKind::Codex,
            "gpt-5.3-codex",
            ThinkingEffort::High,
            None,
        )
        .unwrap();

        let snapshot = db.load_conversation("p", "w", 1).unwrap();
        assert_eq!(snapshot.runner, Some(luban_domain::AgentRunnerKind::Codex));
        assert_eq!(snapshot.agent_model_id.as_deref(), Some("gpt-5.3-codex"));
        assert_eq!(snapshot.thinking_effort, Some(ThinkingEffort::High));
        assert_eq!(snapshot.amp_mode, None);

        let snapshot = db.load_conversation_page("p", "w", 1, None, 10).unwrap();
        assert_eq!(snapshot.runner, Some(luban_domain::AgentRunnerKind::Codex));
        assert_eq!(snapshot.agent_model_id.as_deref(), Some("gpt-5.3-codex"));
        assert_eq!(snapshot.thinking_effort, Some(ThinkingEffort::High));
        assert_eq!(snapshot.amp_mode, None);
    }

    #[test]
    fn conversation_load_page_returns_slice_and_totals() {
        let path = temp_db_path("conversation_load_page_returns_slice_and_totals");
        let mut db = open_db(&path);

        db.ensure_conversation("p", "w", 1).unwrap();

        for idx in 0..10_u64 {
            let entry = ConversationEntry::AgentEvent {
                entry_id: String::new(),
                created_at_unix_ms: 0,
                runner: None,
                event: luban_domain::AgentEvent::TurnDuration { duration_ms: idx },
            };
            db.append_conversation_entries("p", "w", 1, std::slice::from_ref(&entry))
                .unwrap();
        }

        let snapshot = db.load_conversation_page("p", "w", 1, Some(8), 3).unwrap();
        assert_eq!(snapshot.entries_total, 11);
        assert_eq!(snapshot.entries_start, 5);
        assert_eq!(snapshot.entries.len(), 3);
        assert!(matches!(
            &snapshot.entries[..],
            [
                ConversationEntry::AgentEvent {
                    event: luban_domain::AgentEvent::TurnDuration { duration_ms: 4 },
                    ..
                },
                ConversationEntry::AgentEvent {
                    event: luban_domain::AgentEvent::TurnDuration { duration_ms: 5 },
                    ..
                },
                ConversationEntry::AgentEvent {
                    event: luban_domain::AgentEvent::TurnDuration { duration_ms: 6 },
                    ..
                }
            ]
        ));
    }

    #[test]
    fn conversation_append_allows_same_raw_id_across_turns_when_scoped() {
        let path = temp_db_path("conversation_append_allows_same_raw_id_across_turns_when_scoped");
        let mut db = open_db(&path);

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "p".to_owned(),
                name: "P".to_owned(),
                path: PathBuf::from("/tmp/p"),
                is_git: true,
                expanded: false,
                workspaces: vec![PersistedWorkspace {
                    id: 2,
                    workspace_name: "w".to_owned(),
                    branch_name: "w".to_owned(),
                    worktree_path: PathBuf::from("/tmp/p/worktrees/w"),
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
            agent_runner_default_models: HashMap::new(),
            agent_default_thinking_effort: None,
            agent_default_runner: None,
            agent_amp_mode: None,
            agent_codex_enabled: Some(true),
            agent_amp_enabled: Some(true),
            agent_claude_enabled: Some(true),
            agent_droid_enabled: Some(true),
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            starred_tasks: HashMap::new(),
            task_prompt_templates: HashMap::new(),
            telegram_enabled: None,
            telegram_bot_token: None,
            telegram_bot_username: None,
            telegram_paired_chat_id: None,
            telegram_topic_bindings: None,
        };
        db.save_app_state(&snapshot).unwrap();

        db.ensure_conversation("p", "w", 1).unwrap();

        let entry_a = ConversationEntry::AgentEvent {
            entry_id: String::new(),
            created_at_unix_ms: 0,
            runner: None,
            event: luban_domain::AgentEvent::Message {
                id: "turn-a/item_0".to_owned(),
                text: "A".to_owned(),
            },
        };
        let entry_b = ConversationEntry::AgentEvent {
            entry_id: String::new(),
            created_at_unix_ms: 0,
            runner: None,
            event: luban_domain::AgentEvent::Message {
                id: "turn-b/item_0".to_owned(),
                text: "B".to_owned(),
            },
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
                ConversationEntry::AgentEvent {
                    event: luban_domain::AgentEvent::Message { id, text },
                    ..
                } => Some((id.as_str(), text.as_str())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            vec![("turn-a/item_0", "A"), ("turn-b/item_0", "B")]
        );
    }

    #[test]
    fn save_app_state_can_move_workspaces_without_losing_conversations() {
        let path = temp_db_path("save_app_state_can_move_workspaces_without_losing_conversations");
        let mut db = open_db(&path);

        let snapshot_before = PersistedAppState {
            projects: vec![
                PersistedProject {
                    id: 1,
                    slug: "p1".to_owned(),
                    name: "P1".to_owned(),
                    path: PathBuf::from("/tmp/p1"),
                    is_git: true,
                    expanded: false,
                    workspaces: vec![PersistedWorkspace {
                        id: 10,
                        workspace_name: "w1".to_owned(),
                        branch_name: "w1".to_owned(),
                        worktree_path: PathBuf::from("/tmp/p1/worktrees/w1"),
                        status: WorkspaceStatus::Active,
                        last_activity_at_unix_seconds: None,
                    }],
                },
                PersistedProject {
                    id: 2,
                    slug: "p2".to_owned(),
                    name: "P2".to_owned(),
                    path: PathBuf::from("/tmp/p2"),
                    is_git: true,
                    expanded: false,
                    workspaces: vec![PersistedWorkspace {
                        id: 20,
                        workspace_name: "w".to_owned(),
                        branch_name: "w".to_owned(),
                        worktree_path: PathBuf::from("/tmp/p2/worktrees/w"),
                        status: WorkspaceStatus::Active,
                        last_activity_at_unix_seconds: None,
                    }],
                },
            ],
            sidebar_width: None,
            terminal_pane_width: None,
            global_zoom_percent: None,
            appearance_theme: None,
            appearance_ui_font: None,
            appearance_chat_font: None,
            appearance_code_font: None,
            appearance_terminal_font: None,
            agent_default_model_id: None,
            agent_runner_default_models: HashMap::new(),
            agent_default_thinking_effort: None,
            agent_default_runner: None,
            agent_amp_mode: None,
            agent_codex_enabled: Some(true),
            agent_amp_enabled: Some(true),
            agent_claude_enabled: Some(true),
            agent_droid_enabled: Some(true),
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            starred_tasks: HashMap::new(),
            task_prompt_templates: HashMap::new(),
            telegram_enabled: None,
            telegram_bot_token: None,
            telegram_bot_username: None,
            telegram_paired_chat_id: None,
            telegram_topic_bindings: None,
        };

        db.save_app_state(&snapshot_before).unwrap();

        db.ensure_conversation("p2", "w", 1).unwrap();
        let entry = ConversationEntry::UserEvent {
            entry_id: String::new(),
            created_at_unix_ms: 0,
            event: luban_domain::UserEvent::Message {
                text: "hello".to_owned(),
                attachments: Vec::new(),
            },
        };
        db.append_conversation_entries("p2", "w", 1, std::slice::from_ref(&entry))
            .unwrap();

        let snapshot_after = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "p1".to_owned(),
                name: "P1".to_owned(),
                path: PathBuf::from("/tmp/p1"),
                is_git: true,
                expanded: false,
                workspaces: vec![
                    PersistedWorkspace {
                        id: 10,
                        workspace_name: "w1".to_owned(),
                        branch_name: "w1".to_owned(),
                        worktree_path: PathBuf::from("/tmp/p1/worktrees/w1"),
                        status: WorkspaceStatus::Active,
                        last_activity_at_unix_seconds: None,
                    },
                    PersistedWorkspace {
                        id: 20,
                        workspace_name: "w".to_owned(),
                        branch_name: "w".to_owned(),
                        worktree_path: PathBuf::from("/tmp/p2/worktrees/w"),
                        status: WorkspaceStatus::Active,
                        last_activity_at_unix_seconds: None,
                    },
                ],
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
            agent_runner_default_models: HashMap::new(),
            agent_default_thinking_effort: None,
            agent_default_runner: None,
            agent_amp_mode: None,
            agent_codex_enabled: Some(true),
            agent_amp_enabled: Some(true),
            agent_claude_enabled: Some(true),
            agent_droid_enabled: Some(true),
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            starred_tasks: HashMap::new(),
            task_prompt_templates: HashMap::new(),
            telegram_enabled: None,
            telegram_bot_token: None,
            telegram_bot_username: None,
            telegram_paired_chat_id: None,
            telegram_topic_bindings: None,
        };

        db.save_app_state(&snapshot_after).unwrap();

        let loaded = db.load_app_state().unwrap();
        assert_eq!(loaded, snapshot_after);

        let conv = db.load_conversation("p1", "w", 1).unwrap();
        assert!(
            conv.entries.iter().any(
                |e| matches!(e, ConversationEntry::UserEvent { event: luban_domain::UserEvent::Message { text, .. }, .. } if text == "hello")
            ),
            "expected conversation entries to be preserved across workspace move"
        );
    }

    #[test]
    fn save_app_state_deletes_conversations_for_removed_workspaces() {
        let path = temp_db_path("save_app_state_deletes_conversations_for_removed_workspaces");
        let mut db = open_db(&path);

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "p".to_owned(),
                name: "P".to_owned(),
                path: PathBuf::from("/tmp/p"),
                is_git: true,
                expanded: false,
                workspaces: vec![PersistedWorkspace {
                    id: 2,
                    workspace_name: "w".to_owned(),
                    branch_name: "w".to_owned(),
                    worktree_path: PathBuf::from("/tmp/p/worktrees/w"),
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
            agent_runner_default_models: HashMap::new(),
            agent_default_thinking_effort: None,
            agent_default_runner: None,
            agent_amp_mode: None,
            agent_codex_enabled: Some(true),
            agent_amp_enabled: Some(true),
            agent_claude_enabled: Some(true),
            agent_droid_enabled: Some(true),
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            starred_tasks: HashMap::new(),
            task_prompt_templates: HashMap::new(),
            telegram_enabled: None,
            telegram_bot_token: None,
            telegram_bot_username: None,
            telegram_paired_chat_id: None,
            telegram_topic_bindings: None,
        };

        db.save_app_state(&snapshot).unwrap();
        db.ensure_conversation("p", "w", 1).unwrap();
        let entry = ConversationEntry::UserEvent {
            entry_id: String::new(),
            created_at_unix_ms: 0,
            event: luban_domain::UserEvent::Message {
                text: "hello".to_owned(),
                attachments: Vec::new(),
            },
        };
        db.append_conversation_entries("p", "w", 1, std::slice::from_ref(&entry))
            .unwrap();

        let entries_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversation_entries WHERE project_slug = ?1 AND workspace_name = ?2",
                params!["p", "w"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(entries_count, 2);

        let empty = PersistedAppState {
            projects: Vec::new(),
            sidebar_width: None,
            terminal_pane_width: None,
            global_zoom_percent: None,
            appearance_theme: None,
            appearance_ui_font: None,
            appearance_chat_font: None,
            appearance_code_font: None,
            appearance_terminal_font: None,
            agent_default_model_id: None,
            agent_runner_default_models: HashMap::new(),
            agent_default_thinking_effort: None,
            agent_default_runner: None,
            agent_amp_mode: None,
            agent_codex_enabled: Some(true),
            agent_amp_enabled: Some(true),
            agent_claude_enabled: Some(true),
            agent_droid_enabled: Some(true),
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            starred_tasks: HashMap::new(),
            task_prompt_templates: HashMap::new(),
            telegram_enabled: None,
            telegram_bot_token: None,
            telegram_bot_username: None,
            telegram_paired_chat_id: None,
            telegram_topic_bindings: None,
        };
        db.save_app_state(&empty).unwrap();

        let conversations_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE project_slug = ?1 AND workspace_name = ?2",
                params!["p", "w"],
                |row| row.get(0),
            )
            .unwrap();
        let entries_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversation_entries WHERE project_slug = ?1 AND workspace_name = ?2",
                params!["p", "w"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(conversations_count, 0);
        assert_eq!(entries_count, 0);
    }
}
