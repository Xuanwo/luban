use crate::engine::EngineHandle;
use crate::engine::TelegramRuntimeConfig;
use anyhow::Context as _;
use luban_api::{ConversationEntry, ServerEvent, TaskStatus, WsServerMessage};
use luban_domain::Action;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

const TELEGRAM_DISABLED_ENV: &str = "LUBAN_TELEGRAM_DISABLED";
const TELEGRAM_API_BASE_URL_ENV: &str = "LUBAN_TELEGRAM_API_BASE_URL";
const TELEGRAM_API_BASE_URL_DEFAULT: &str = "https://api.telegram.org";

const TELEGRAM_LONG_POLL_TIMEOUT_SECS: u64 = 20;
const TELEGRAM_REQUEST_TIMEOUT_SECS: u64 = 30;
const TELEGRAM_MAX_MESSAGE_CHARS: usize = 3800;
const TELEGRAM_REPLY_ROUTE_TTL_SECS: u64 = 6 * 60 * 60;
const TELEGRAM_REPLY_ROUTE_MAX_ROUTES: usize = 256;
const TELEGRAM_PARSE_MODE_MARKDOWN_V2: &str = "MarkdownV2";

const KB_HOME: &str = "Home";
const KB_PROJECTS: &str = "Projects";
const KB_RECENT_TASKS: &str = "Recent tasks";
const KB_ACTIVE_TASK: &str = "Active task";
const KB_NEW_TASK: &str = "New task";
const KB_BACK: &str = "Back";
const KB_CANCEL: &str = "Cancel";

pub(crate) fn telegram_disabled() -> bool {
    std::env::var(TELEGRAM_DISABLED_ENV)
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .is_some_and(|v| v == "1" || v == "true" || v == "yes")
}

fn telegram_api_base_url() -> String {
    std::env::var(TELEGRAM_API_BASE_URL_ENV)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| TELEGRAM_API_BASE_URL_DEFAULT.to_owned())
}

pub(crate) fn start_gateway(engine: EngineHandle, events: broadcast::Sender<WsServerMessage>) {
    if telegram_disabled() {
        tracing::info!("telegram gateway disabled by env");
        return;
    }

    tokio::spawn(async move {
        let mut gateway = TelegramGateway::new(engine, events.subscribe());
        if let Err(err) = gateway.run().await {
            tracing::warn!(error = %err, "telegram gateway stopped");
        }
    });
}

pub(crate) async fn telegram_get_me_username(token: &str) -> Result<String, String> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(TELEGRAM_REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|err| format!("failed to build http client: {err}"))?;

    let base = telegram_api_base_url();
    let url = format!("{base}/bot{token}/getMe");
    let res = http
        .get(url)
        .send()
        .await
        .map_err(|err| format!("telegram getMe request failed: {err}"))?;

    let parsed = res
        .json::<TelegramApiResponse<TelegramGetMeResult>>()
        .await
        .map_err(|err| format!("telegram getMe response parse failed: {err}"))?;

    if !parsed.ok {
        return Err(parsed
            .description
            .unwrap_or_else(|| "telegram getMe failed".to_owned()));
    }

    let username = parsed
        .result
        .username
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "telegram bot username is missing".to_owned())?;

    Ok(username.to_owned())
}

#[derive(Clone, Debug, Deserialize)]
struct TelegramApiResponse<T> {
    ok: bool,
    #[serde(default)]
    result: T,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TelegramGetMeResult {
    #[serde(default)]
    username: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TelegramSendMessageResult {
    #[serde(default)]
    message_id: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TelegramUpdate {
    update_id: i64,
    #[serde(default)]
    message: Option<TelegramMessage>,
    #[serde(default)]
    callback_query: Option<TelegramCallbackQuery>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TelegramCallbackQuery {
    id: String,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    message: Option<TelegramMessage>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TelegramMessage {
    #[serde(default)]
    message_id: i64,
    chat: TelegramChat,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    message_thread_id: Option<i64>,
    #[serde(default)]
    is_topic_message: Option<bool>,
    #[serde(default)]
    reply_to_message: Option<Box<TelegramMessage>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TelegramChat {
    id: i64,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

struct TelegramGateway {
    engine: EngineHandle,
    events: broadcast::Receiver<WsServerMessage>,
    http: reqwest::Client,
    api_base: String,
    last_config_rev: u64,
    runtime: TelegramRuntimeConfig,
    session: TelegramSession,
    last_seen_entry_index: HashMap<(i64, Option<i64>, u64, u64), u64>,
    reply_routes: HashMap<i64, ReplyRoute>,
    topic_bindings: HashMap<i64, TopicBinding>,
    inbox_initialized_workspaces: HashSet<u64>,
    inbox_task_status: HashMap<(u64, u64), TaskStatus>,
}

#[derive(Clone, Debug, Default)]
struct TelegramSession {
    active_workspace_id: Option<u64>,
    active_thread_id: Option<u64>,
    ui_state: TelegramUiState,
    keyboard_routes: HashMap<String, KeyboardRoute>,
    pending_comment_target: Option<(u64, u64)>,
}

#[derive(Clone, Debug, Default)]
enum TelegramUiState {
    #[default]
    Home,
    SelectingProject,
    SelectingTask,
    SelectingWorktree {
        project_slug: String,
    },
}

#[derive(Clone, Debug)]
enum KeyboardRoute {
    SelectProject { project_slug: String },
    SelectTask { workspace_id: u64, thread_id: u64 },
    NewTask { project_slug: String },
    SelectWorktree { workspace_id: u64 },
}

#[derive(Clone, Debug)]
struct ReplyRoute {
    workspace_id: u64,
    thread_id: u64,
    created_at: Instant,
}

#[derive(Clone, Debug)]
struct TopicBinding {
    workspace_id: u64,
    thread_id: u64,
    replayed_up_to: Option<u64>,
}

impl TelegramGateway {
    fn new(engine: EngineHandle, events: broadcast::Receiver<WsServerMessage>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(TELEGRAM_REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            engine,
            events,
            http,
            api_base: telegram_api_base_url(),
            last_config_rev: 0,
            runtime: TelegramRuntimeConfig {
                enabled: false,
                bot_token: None,
                paired_chat_id: None,
                topic_bindings: Vec::new(),
            },
            session: TelegramSession::default(),
            last_seen_entry_index: HashMap::new(),
            reply_routes: HashMap::new(),
            topic_bindings: HashMap::new(),
            inbox_initialized_workspaces: HashSet::new(),
            inbox_task_status: HashMap::new(),
        }
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        self.refresh_runtime_config().await;

        let mut update_offset = 0i64;

        loop {
            let token = self.runtime.bot_token.clone();
            let enabled = self.runtime.enabled;
            let api_base = self.api_base.clone();
            let http = self.http.clone();
            let offset = update_offset;

            let poll_fut = async move {
                if !enabled {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    return Ok(None);
                }
                let Some(token) = token else {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    return Ok(None);
                };
                poll_updates(http, api_base, token, offset).await.map(Some)
            };

            tokio::select! {
                msg = self.events.recv() => {
                    match msg {
                        Ok(msg) => self.handle_server_message(msg).await,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                polled = poll_fut => {
                    match polled {
                        Ok(Some((updates, next_offset))) => {
                            update_offset = next_offset;
                            self.handle_updates(updates).await;
                        }
                        Ok(None) => {}
                        Err(err) => {
                            self.set_last_error(err).await;
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_server_message(&mut self, msg: WsServerMessage) {
        let WsServerMessage::Event { event, .. } = msg else {
            return;
        };

        match *event {
            ServerEvent::AppChanged { snapshot, .. } => {
                let next_rev = snapshot.integrations.telegram.config_rev;
                if next_rev != self.last_config_rev {
                    self.last_config_rev = next_rev;
                    self.refresh_runtime_config().await;
                }
            }
            ServerEvent::TaskSummariesChanged { tasks, .. } => {
                self.handle_task_summaries_changed(tasks).await;
            }
            ServerEvent::ConversationChanged { snapshot } => {
                let Some(chat_id) = self.runtime.paired_chat_id else {
                    return;
                };
                self.forward_conversation_updates(chat_id, None, &snapshot)
                    .await;
            }
            _ => {}
        }
    }

    async fn handle_task_summaries_changed(&mut self, tasks: Vec<luban_api::TaskSummarySnapshot>) {
        let Some(chat_id) = self.runtime.paired_chat_id else {
            return;
        };

        if tasks.is_empty() {
            return;
        }

        let workspace_id = tasks[0].workspace_id.0;
        let initialized = self.inbox_initialized_workspaces.contains(&workspace_id);
        if !initialized {
            for t in tasks {
                self.inbox_task_status
                    .insert((t.workspace_id.0, t.thread_id.0), t.task_status);
            }
            self.inbox_initialized_workspaces.insert(workspace_id);
            return;
        }

        for t in tasks {
            let key = (t.workspace_id.0, t.thread_id.0);
            match self.inbox_task_status.get(&key).copied() {
                None => {
                    self.inbox_task_status.insert(key, t.task_status);
                    let text = format!("Task created: {}", t.title.trim());
                    let kb = inline_keyboard(vec![vec![InlineButton::new(
                        "Comment",
                        &format!("comment:{}:{}", t.workspace_id.0, t.thread_id.0),
                    )]]);
                    let _ = self.send_message(chat_id, None, &text, Some(kb)).await;
                }
                Some(prev) => {
                    self.inbox_task_status.insert(key, t.task_status);
                    if prev != TaskStatus::Done && t.task_status == TaskStatus::Done {
                        let text = format!("Task completed: {}", t.title.trim());
                        let kb = inline_keyboard(vec![vec![InlineButton::new(
                            "Comment",
                            &format!("comment:{}:{}", t.workspace_id.0, t.thread_id.0),
                        )]]);
                        let _ = self.send_message(chat_id, None, &text, Some(kb)).await;
                    }
                }
            }
        }
    }

    async fn refresh_runtime_config(&mut self) {
        match self.engine.telegram_runtime_config().await {
            Ok(cfg) => {
                let token_changed = cfg.bot_token != self.runtime.bot_token;
                let paired_chat_changed = cfg.paired_chat_id != self.runtime.paired_chat_id;
                self.runtime = cfg;
                if token_changed {
                    self.session = TelegramSession::default();
                    self.last_seen_entry_index.clear();
                    self.reply_routes.clear();
                }
                if paired_chat_changed {
                    self.session = TelegramSession::default();
                    self.last_seen_entry_index.clear();
                    self.reply_routes.clear();
                }

                self.topic_bindings = self
                    .runtime
                    .topic_bindings
                    .iter()
                    .map(|b| {
                        (
                            b.message_thread_id,
                            TopicBinding {
                                workspace_id: b.workspace_id,
                                thread_id: b.thread_id,
                                replayed_up_to: b.replayed_up_to,
                            },
                        )
                    })
                    .collect();

                if let Some(chat_id) = self.runtime.paired_chat_id {
                    for (topic_id, binding) in &self.topic_bindings {
                        let Some(replayed_up_to) = binding.replayed_up_to else {
                            continue;
                        };
                        let key = (
                            chat_id,
                            Some(*topic_id),
                            binding.workspace_id,
                            binding.thread_id,
                        );
                        self.last_seen_entry_index
                            .entry(key)
                            .or_insert(replayed_up_to);
                    }
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "failed to refresh telegram runtime config");
            }
        }
    }

    fn prune_reply_routes(&mut self) {
        let now = Instant::now();
        let ttl = Duration::from_secs(TELEGRAM_REPLY_ROUTE_TTL_SECS);
        self.reply_routes
            .retain(|_, route| now.duration_since(route.created_at) <= ttl);
    }

    fn insert_reply_route(&mut self, message_id: i64, workspace_id: u64, thread_id: u64) {
        self.prune_reply_routes();

        if self.reply_routes.len() >= TELEGRAM_REPLY_ROUTE_MAX_ROUTES {
            if let Some(oldest) = self
                .reply_routes
                .iter()
                .min_by_key(|(_, route)| route.created_at)
                .map(|(k, _)| *k)
            {
                self.reply_routes.remove(&oldest);
            } else {
                self.reply_routes.clear();
            }
        }

        self.reply_routes.insert(
            message_id,
            ReplyRoute {
                workspace_id,
                thread_id,
                created_at: Instant::now(),
            },
        );
    }

    async fn handle_updates(&mut self, updates: Vec<TelegramUpdate>) {
        for update in updates {
            if let Some(cb) = update.callback_query {
                if let Err(err) = self.handle_callback_query(cb).await {
                    tracing::debug!(error = %err, "telegram callback query failed");
                }
                continue;
            }

            if let Some(msg) = update.message {
                if let Err(err) = self.handle_message(msg).await {
                    tracing::debug!(error = %err, "telegram message failed");
                }
                continue;
            }
        }
    }

    async fn handle_message(&mut self, msg: TelegramMessage) -> anyhow::Result<()> {
        let chat_id = msg.chat.id;
        if msg.chat.kind.as_deref() != Some("private") {
            return Ok(());
        }

        let text = msg.text.as_deref().unwrap_or_default().trim();
        if text.is_empty() {
            return Ok(());
        }

        if text.starts_with("/start") {
            let code = text
                .split_whitespace()
                .nth(1)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or_default()
                .to_owned();
            if code.is_empty() {
                self.send_message(chat_id, None, "Missing pairing code.", None)
                    .await?;
                return Ok(());
            }

            match self
                .engine
                .consume_telegram_pairing_code(code, chat_id)
                .await
            {
                Ok(()) => {
                    self.set_last_error("".to_owned()).await;
                    self.send_home(chat_id).await?;
                }
                Err(message) => {
                    self.send_message(chat_id, None, &message, None).await?;
                }
            }
            return Ok(());
        }

        let Some(paired_chat_id) = self.runtime.paired_chat_id else {
            self.send_message(
                chat_id,
                None,
                "Telegram is not paired. Open Settings -> Integrations -> Telegram.",
                None,
            )
            .await?;
            return Ok(());
        };
        if paired_chat_id != chat_id {
            return Ok(());
        }

        if msg.is_topic_message.unwrap_or(false) || msg.message_thread_id.is_some() {
            self.send_message(
                chat_id,
                None,
                "Topics are not supported. Please use the main chat.",
                None,
            )
            .await?;
            return Ok(());
        }

        if self.handle_keyboard_input(chat_id, text).await? {
            return Ok(());
        }

        if let Some((wid, tid)) = self.session.pending_comment_target.take() {
            self.session.active_workspace_id = Some(wid);
            self.session.active_thread_id = Some(tid);
            self.send_agent_message(wid, tid, text).await;
            self.send_home(chat_id).await?;
            return Ok(());
        }

        self.prune_reply_routes();
        let now = Instant::now();
        let Some((wid, tid)) = resolve_message_target(
            &msg,
            &self.session,
            &self.reply_routes,
            &self.topic_bindings,
            now,
        ) else {
            self.send_home(chat_id).await?;
            return Ok(());
        };
        self.session.active_workspace_id = Some(wid);
        self.session.active_thread_id = Some(tid);

        self.send_agent_message(wid, tid, text).await;
        Ok(())
    }

    async fn send_agent_message(&self, wid: u64, tid: u64, text: &str) {
        let action = luban_api::ClientAction::SendAgentMessage {
            workspace_id: luban_api::WorkspaceId(wid),
            thread_id: luban_api::WorkspaceThreadId(tid),
            text: text.to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        };
        let _ = self
            .engine
            .apply_client_action("telegram_send".to_owned(), action)
            .await;
    }

    async fn handle_callback_query(&mut self, cb: TelegramCallbackQuery) -> anyhow::Result<()> {
        let Some(chat_id) = cb.message.as_ref().map(|m| m.chat.id) else {
            return Ok(());
        };
        if cb.message.as_ref().and_then(|m| m.chat.kind.as_deref()) != Some("private") {
            return Ok(());
        }

        let data = cb.data.as_deref().unwrap_or_default().trim();
        let topic_action = parse_topic_callback_action(data);
        let action = parse_callback_action(data);
        self.answer_callback_query(&cb.id).await?;

        let Some(paired_chat_id) = self.runtime.paired_chat_id else {
            self.send_message(
                chat_id,
                None,
                "Telegram is not paired. Open Settings -> Integrations -> Telegram.",
                None,
            )
            .await?;
            return Ok(());
        };
        if paired_chat_id != chat_id {
            return Ok(());
        }

        if let Some(topic_action) = topic_action {
            self.handle_topic_callback_action(chat_id, topic_action)
                .await?;
            return Ok(());
        }

        match action {
            CallbackAction::Home => self.send_home(chat_id).await?,
            CallbackAction::Workspaces => self.send_workspaces(chat_id).await?,
            CallbackAction::SelectWorkspace { workspace_id } => {
                self.session.active_workspace_id = Some(workspace_id);
                self.session.active_thread_id = None;
                self.send_tasks(chat_id, workspace_id).await?;
            }
            CallbackAction::SelectTask {
                workspace_id,
                thread_id,
            } => {
                self.session.active_workspace_id = Some(workspace_id);
                self.session.active_thread_id = Some(thread_id);
                self.send_task_selected(chat_id, workspace_id, thread_id)
                    .await?;
            }
            CallbackAction::NewTask { workspace_id } => {
                self.create_task_and_select(chat_id, workspace_id).await?;
            }
            CallbackAction::Comment {
                workspace_id,
                thread_id,
            } => {
                self.session.pending_comment_target = Some((workspace_id, thread_id));
                let title = self.task_title_or_default(workspace_id, thread_id).await;
                let text = format!("Comment on: {title}\n\nSend your message now.");
                self.send_message(chat_id, None, &text, Some(home_reply_keyboard()))
                    .await?;
            }
        }

        Ok(())
    }

    async fn create_task_and_select(
        &mut self,
        chat_id: i64,
        workspace_id: u64,
    ) -> anyhow::Result<()> {
        let action = luban_api::ClientAction::CreateWorkspaceThread {
            workspace_id: luban_api::WorkspaceId(workspace_id),
        };
        let _ = self
            .engine
            .apply_client_action("telegram_new_task".to_owned(), action)
            .await;

        let snapshot = self
            .engine
            .threads_snapshot(luban_api::WorkspaceId(workspace_id))
            .await
            .context("threads snapshot")?;

        let new_tid = snapshot
            .threads
            .iter()
            .map(|t| t.thread_id.0)
            .max()
            .unwrap_or(1);
        self.session.active_workspace_id = Some(workspace_id);
        self.session.active_thread_id = Some(new_tid);
        self.send_task_selected(chat_id, workspace_id, new_tid)
            .await?;
        Ok(())
    }

    async fn handle_topic_callback_action(
        &mut self,
        chat_id: i64,
        action: TopicCallbackAction,
    ) -> anyhow::Result<()> {
        match action {
            TopicCallbackAction::ShowProjectMenu { topic_id } => {
                self.send_topic_project_menu(chat_id, topic_id).await?;
            }
            TopicCallbackAction::SelectProject {
                topic_id,
                project_slug,
            } => {
                self.send_topic_task_menu(chat_id, topic_id, &project_slug)
                    .await?;
            }
            TopicCallbackAction::BindTask {
                topic_id,
                workspace_id,
                thread_id,
            } => {
                self.bind_topic_to_task(chat_id, topic_id, workspace_id, thread_id, true)
                    .await?;
            }
            TopicCallbackAction::NewTask {
                topic_id,
                project_slug,
            } => {
                self.send_topic_worktree_menu(chat_id, topic_id, &project_slug)
                    .await?;
            }
            TopicCallbackAction::CreateTask {
                topic_id,
                workspace_id,
            } => {
                let thread_id = self.create_task(workspace_id).await?;
                self.bind_topic_to_task(chat_id, topic_id, workspace_id, thread_id, false)
                    .await?;
            }
            TopicCallbackAction::Unbind { topic_id } => {
                self.unbind_topic(chat_id, topic_id).await?;
            }
        }
        Ok(())
    }

    async fn send_topic_project_menu(&mut self, chat_id: i64, topic_id: i64) -> anyhow::Result<()> {
        let app = self.engine.app_snapshot().await.context("app snapshot")?;
        let mut rows = Vec::new();
        for project in &app.projects {
            rows.push(vec![InlineButton::new(
                &project.name,
                &format!("tproj:{topic_id}:{}", project.slug),
            )]);
        }

        let text = "Select a project for this topic.";
        let kb = inline_keyboard(rows);
        self.send_message(chat_id, Some(topic_id), text, Some(kb))
            .await?;
        Ok(())
    }

    async fn send_topic_task_menu(
        &mut self,
        chat_id: i64,
        topic_id: i64,
        project_slug: &str,
    ) -> anyhow::Result<()> {
        let tasks = self.project_recent_tasks(project_slug).await?;

        let mut rows = Vec::new();
        for t in tasks.into_iter().take(8) {
            let label = format!("{} · {}", truncate_label(&t.title, 32), t.workspace_name);
            let data = format!("tbind:{topic_id}:{}:{}", t.workspace_id, t.thread_id);
            rows.push(vec![InlineButton::new(&label, &data)]);
        }

        rows.push(vec![InlineButton::new(
            "New Task",
            &format!("tnew:{topic_id}:{project_slug}"),
        )]);
        rows.push(vec![InlineButton::new(
            "Change Project",
            &format!("tproj:{topic_id}"),
        )]);

        let text = format!("Project: {project_slug}\n\nSelect a task for this topic.");
        let kb = inline_keyboard(rows);
        self.send_message(chat_id, Some(topic_id), &text, Some(kb))
            .await?;
        Ok(())
    }

    async fn send_topic_worktree_menu(
        &mut self,
        chat_id: i64,
        topic_id: i64,
        project_slug: &str,
    ) -> anyhow::Result<()> {
        let app = self.engine.app_snapshot().await.context("app snapshot")?;
        let Some(project) = app.projects.iter().find(|p| p.slug == project_slug) else {
            self.send_topic_project_menu(chat_id, topic_id).await?;
            return Ok(());
        };

        let mut rows = Vec::new();
        for ws in &project.workspaces {
            let label = format!("{} ({})", ws.workspace_name, ws.branch_name);
            let data = format!("tcreate:{topic_id}:{}", ws.id.0);
            rows.push(vec![InlineButton::new(&label, &data)]);
        }
        rows.push(vec![InlineButton::new(
            "Back",
            &format!("tproj:{topic_id}:{project_slug}"),
        )]);

        let text = "Select a worktree for the new task.";
        let kb = inline_keyboard(rows);
        self.send_message(chat_id, Some(topic_id), text, Some(kb))
            .await?;
        Ok(())
    }

    async fn create_task(&mut self, workspace_id: u64) -> anyhow::Result<u64> {
        let action = luban_api::ClientAction::CreateWorkspaceThread {
            workspace_id: luban_api::WorkspaceId(workspace_id),
        };
        let _ = self
            .engine
            .apply_client_action("telegram_topic_new_task".to_owned(), action)
            .await;

        let snapshot = self
            .engine
            .threads_snapshot(luban_api::WorkspaceId(workspace_id))
            .await
            .context("threads snapshot")?;

        Ok(snapshot
            .threads
            .iter()
            .map(|t| t.thread_id.0)
            .max()
            .unwrap_or(1))
    }

    async fn bind_topic_to_task(
        &mut self,
        chat_id: i64,
        topic_id: i64,
        workspace_id: u64,
        thread_id: u64,
        replay_history: bool,
    ) -> anyhow::Result<()> {
        self.last_seen_entry_index
            .retain(|(cid, tid, _, _), _| *cid != chat_id || *tid != Some(topic_id));

        let snapshot = self
            .engine
            .conversation_snapshot(
                luban_api::WorkspaceId(workspace_id),
                luban_api::WorkspaceThreadId(thread_id),
                None,
                Some(1),
            )
            .await
            .context("conversation snapshot")?;

        let title = snapshot.title.trim();
        let title = if title.is_empty() { "Task" } else { title };
        let topic_name = sanitize_telegram_topic_name(title);
        let _ = self.edit_forum_topic(chat_id, topic_id, &topic_name).await;

        let mut replayed_up_to = None;
        if replay_history {
            replayed_up_to = self
                .latest_global_entry_index(workspace_id, thread_id)
                .await?;
            if let Some(replayed_up_to) = replayed_up_to {
                self.last_seen_entry_index.insert(
                    (chat_id, Some(topic_id), workspace_id, thread_id),
                    replayed_up_to,
                );
            }
        }

        self.topic_bindings.insert(
            topic_id,
            TopicBinding {
                workspace_id,
                thread_id,
                replayed_up_to,
            },
        );

        let kb = inline_keyboard(vec![vec![InlineButton::new(
            "Unlink",
            &format!("tunbind:{topic_id}"),
        )]]);
        let text = format!("Linked to: {title}\n\nSend a message in this topic to continue.");
        self.send_message(chat_id, Some(topic_id), &text, Some(kb))
            .await?;

        if replay_history && replayed_up_to.is_some() {
            self.replay_task_dynamics(chat_id, topic_id, workspace_id, thread_id)
                .await?;
        }

        let _ = self
            .engine
            .dispatch_domain_action(Action::TelegramTopicBound {
                message_thread_id: topic_id,
                workspace_id,
                thread_id,
                replayed_up_to,
            })
            .await;

        Ok(())
    }

    async fn unbind_topic(&mut self, chat_id: i64, topic_id: i64) -> anyhow::Result<()> {
        self.topic_bindings.remove(&topic_id);
        self.last_seen_entry_index
            .retain(|(cid, tid, _, _), _| *cid != chat_id || *tid != Some(topic_id));

        let _ = self
            .engine
            .dispatch_domain_action(Action::TelegramTopicUnbound {
                message_thread_id: topic_id,
            })
            .await;

        self.send_topic_project_menu(chat_id, topic_id).await?;
        Ok(())
    }

    async fn latest_global_entry_index(
        &mut self,
        workspace_id: u64,
        thread_id: u64,
    ) -> anyhow::Result<Option<u64>> {
        let snapshot = self
            .engine
            .conversation_snapshot(
                luban_api::WorkspaceId(workspace_id),
                luban_api::WorkspaceThreadId(thread_id),
                None,
                Some(1),
            )
            .await
            .context("conversation snapshot")?;

        if snapshot.entries_total == 0 || snapshot.entries.is_empty() {
            return Ok(None);
        }

        Ok(Some(snapshot.entries_start.saturating_add(
            snapshot.entries.len().saturating_sub(1) as u64,
        )))
    }

    async fn replay_task_dynamics(
        &mut self,
        chat_id: i64,
        topic_id: i64,
        workspace_id: u64,
        thread_id: u64,
    ) -> anyhow::Result<()> {
        const PAGE_LIMIT: u64 = 5000;

        let mut before = None;
        let mut pages = Vec::new();
        loop {
            let snapshot = self
                .engine
                .conversation_snapshot(
                    luban_api::WorkspaceId(workspace_id),
                    luban_api::WorkspaceThreadId(thread_id),
                    before,
                    Some(PAGE_LIMIT),
                )
                .await
                .context("conversation snapshot")?;
            before = if snapshot.entries_start == 0 {
                None
            } else {
                Some(snapshot.entries_start)
            };
            pages.push(snapshot);
            if before.is_none() {
                break;
            }
        }

        let mut items = Vec::new();
        for page in pages.into_iter().rev() {
            for entry in &page.entries {
                if let Some(text) = format_conversation_entry_for_telegram(entry) {
                    items.push(truncate_message(&text));
                }
            }
        }

        if items.is_empty() {
            return Ok(());
        }

        self.send_message(chat_id, Some(topic_id), "Previous activity:", None)
            .await?;

        let mut buf = String::new();
        for item in items {
            let candidate = if buf.is_empty() {
                item.clone()
            } else {
                format!("{buf}\n\n{item}")
            };

            if candidate.chars().count() > TELEGRAM_MAX_MESSAGE_CHARS {
                self.send_message(chat_id, Some(topic_id), &buf, None)
                    .await?;
                buf = item;
            } else {
                buf = candidate;
            }
        }
        if !buf.trim().is_empty() {
            self.send_message(chat_id, Some(topic_id), &buf, None)
                .await?;
        }

        Ok(())
    }

    async fn project_recent_tasks(
        &mut self,
        project_slug: &str,
    ) -> anyhow::Result<Vec<ProjectTask>> {
        let app = self.engine.app_snapshot().await.context("app snapshot")?;
        let Some(project) = app.projects.iter().find(|p| p.slug == project_slug) else {
            return Ok(Vec::new());
        };

        let mut out = Vec::new();
        for ws in &project.workspaces {
            let snapshot = self
                .engine
                .threads_snapshot(luban_api::WorkspaceId(ws.id.0))
                .await
                .context("threads snapshot")?;
            for t in snapshot.threads {
                out.push(ProjectTask {
                    workspace_id: ws.id.0,
                    workspace_name: ws.workspace_name.clone(),
                    thread_id: t.thread_id.0,
                    title: t.title,
                    updated_at_unix_seconds: t.updated_at_unix_seconds,
                });
            }
        }

        out.sort_by_key(|t| std::cmp::Reverse(t.updated_at_unix_seconds));
        Ok(out)
    }

    async fn send_home(&mut self, chat_id: i64) -> anyhow::Result<()> {
        self.session.ui_state = TelegramUiState::Home;
        self.session.keyboard_routes.clear();

        let mut text = "Luban (Telegram)\n\nUse the keyboard to select a task.\nIf you don't see it, tap the keyboard icon next to the input field.\n\nMessages are sent to the active task.".to_owned();
        if let (Some(wid), Some(tid)) = (
            self.session.active_workspace_id,
            self.session.active_thread_id,
        ) {
            let title = self.task_title_or_default(wid, tid).await;
            text.push_str(&format!("\n\nActive task: {title}"));
        }
        if self.session.pending_comment_target.is_some() {
            text.push_str("\n\nComment mode is active.");
        }

        self.send_message(chat_id, None, &text, Some(home_reply_keyboard()))
            .await?;
        Ok(())
    }

    async fn task_title_or_default(&mut self, workspace_id: u64, thread_id: u64) -> String {
        let snapshot = self
            .engine
            .conversation_snapshot(
                luban_api::WorkspaceId(workspace_id),
                luban_api::WorkspaceThreadId(thread_id),
                None,
                Some(1),
            )
            .await;
        match snapshot {
            Ok(snapshot) => {
                let title = snapshot.title.trim();
                if title.is_empty() {
                    "Task".to_owned()
                } else {
                    truncate_label(title, 48)
                }
            }
            Err(_) => "Task".to_owned(),
        }
    }

    async fn handle_keyboard_input(&mut self, chat_id: i64, text: &str) -> anyhow::Result<bool> {
        match text {
            KB_HOME => {
                self.session.pending_comment_target = None;
                self.send_home(chat_id).await?;
                return Ok(true);
            }
            KB_CANCEL => {
                self.session.pending_comment_target = None;
                self.send_home(chat_id).await?;
                return Ok(true);
            }
            KB_PROJECTS => {
                self.send_project_menu(chat_id).await?;
                return Ok(true);
            }
            KB_RECENT_TASKS => {
                self.send_recent_tasks_menu(chat_id).await?;
                return Ok(true);
            }
            KB_ACTIVE_TASK => {
                self.send_active_task(chat_id).await?;
                return Ok(true);
            }
            KB_BACK => {
                self.handle_keyboard_back(chat_id).await?;
                return Ok(true);
            }
            _ => {}
        }

        if let Some(route) = self.session.keyboard_routes.get(text).cloned() {
            self.session.pending_comment_target = None;
            self.handle_keyboard_route(chat_id, route).await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn handle_keyboard_back(&mut self, chat_id: i64) -> anyhow::Result<()> {
        match self.session.ui_state.clone() {
            TelegramUiState::SelectingProject | TelegramUiState::Home => {
                self.send_home(chat_id).await
            }
            TelegramUiState::SelectingTask => self.send_project_menu(chat_id).await,
            TelegramUiState::SelectingWorktree { project_slug } => {
                self.send_project_task_menu(chat_id, &project_slug).await
            }
        }
    }

    async fn handle_keyboard_route(
        &mut self,
        chat_id: i64,
        route: KeyboardRoute,
    ) -> anyhow::Result<()> {
        match route {
            KeyboardRoute::SelectProject { project_slug } => {
                self.send_project_task_menu(chat_id, &project_slug).await?;
            }
            KeyboardRoute::SelectTask {
                workspace_id,
                thread_id,
            } => {
                self.session.active_workspace_id = Some(workspace_id);
                self.session.active_thread_id = Some(thread_id);
                self.session.ui_state = TelegramUiState::Home;
                self.session.keyboard_routes.clear();
                let title = self.task_title_or_default(workspace_id, thread_id).await;
                let text = format!("Selected: {title}");
                self.send_message(chat_id, None, &text, Some(home_reply_keyboard()))
                    .await?;
            }
            KeyboardRoute::NewTask { project_slug } => {
                self.send_project_worktree_menu(chat_id, &project_slug)
                    .await?;
            }
            KeyboardRoute::SelectWorktree { workspace_id } => {
                let thread_id = self.create_task(workspace_id).await?;
                self.session.active_workspace_id = Some(workspace_id);
                self.session.active_thread_id = Some(thread_id);
                self.session.ui_state = TelegramUiState::Home;
                self.session.keyboard_routes.clear();
                let title = self.task_title_or_default(workspace_id, thread_id).await;
                let text = format!("Created and selected: {title}");
                self.send_message(chat_id, None, &text, Some(home_reply_keyboard()))
                    .await?;
            }
        }
        Ok(())
    }

    async fn send_active_task(&mut self, chat_id: i64) -> anyhow::Result<()> {
        if let (Some(wid), Some(tid)) = (
            self.session.active_workspace_id,
            self.session.active_thread_id,
        ) {
            let title = self.task_title_or_default(wid, tid).await;
            let text = format!("Active task: {title}");
            self.send_message(chat_id, None, &text, Some(home_reply_keyboard()))
                .await?;
            return Ok(());
        }

        self.send_message(
            chat_id,
            None,
            "No active task. Use Projects or Recent tasks to select one.",
            Some(home_reply_keyboard()),
        )
        .await?;
        Ok(())
    }

    async fn send_project_menu(&mut self, chat_id: i64) -> anyhow::Result<()> {
        let app = self.engine.app_snapshot().await.context("app snapshot")?;

        self.session.ui_state = TelegramUiState::SelectingProject;
        self.session.keyboard_routes.clear();

        let mut rows = Vec::new();
        for project in &app.projects {
            let label = truncate_label(&project.name, 48);
            let label = ensure_unique_label(&self.session.keyboard_routes, label);
            self.session.keyboard_routes.insert(
                label.clone(),
                KeyboardRoute::SelectProject {
                    project_slug: project.slug.clone(),
                },
            );
            rows.push(vec![label]);
        }
        rows.push(vec![KB_HOME.to_owned()]);

        let text = "Select a project.";
        let kb = reply_keyboard(rows);
        self.send_message(chat_id, None, text, Some(kb)).await?;
        Ok(())
    }

    async fn send_project_task_menu(
        &mut self,
        chat_id: i64,
        project_slug: &str,
    ) -> anyhow::Result<()> {
        let tasks = self.project_recent_tasks(project_slug).await?;

        self.session.ui_state = TelegramUiState::SelectingTask;
        self.session.keyboard_routes.clear();

        let mut rows = Vec::new();
        for t in tasks.into_iter().take(12) {
            let label = format!(
                "{} · {}",
                truncate_label(&t.title, 32),
                truncate_label(&t.workspace_name, 16)
            );
            let label = ensure_unique_label(&self.session.keyboard_routes, label);
            self.session.keyboard_routes.insert(
                label.clone(),
                KeyboardRoute::SelectTask {
                    workspace_id: t.workspace_id,
                    thread_id: t.thread_id,
                },
            );
            rows.push(vec![label]);
        }

        let new_task_label =
            ensure_unique_label(&self.session.keyboard_routes, KB_NEW_TASK.to_owned());
        self.session.keyboard_routes.insert(
            new_task_label.clone(),
            KeyboardRoute::NewTask {
                project_slug: project_slug.to_owned(),
            },
        );
        rows.push(vec![new_task_label]);

        rows.push(vec![KB_BACK.to_owned(), KB_HOME.to_owned()]);

        let text = format!("Project: {project_slug}\n\nSelect a task.");
        let kb = reply_keyboard(rows);
        self.send_message(chat_id, None, &text, Some(kb)).await?;
        Ok(())
    }

    async fn send_project_worktree_menu(
        &mut self,
        chat_id: i64,
        project_slug: &str,
    ) -> anyhow::Result<()> {
        let app = self.engine.app_snapshot().await.context("app snapshot")?;
        let Some(project) = app.projects.iter().find(|p| p.slug == project_slug) else {
            self.send_project_menu(chat_id).await?;
            return Ok(());
        };

        self.session.ui_state = TelegramUiState::SelectingWorktree {
            project_slug: project_slug.to_owned(),
        };
        self.session.keyboard_routes.clear();

        let mut rows = Vec::new();
        for ws in &project.workspaces {
            let label = format!("{} ({})", ws.workspace_name, ws.branch_name);
            let label =
                ensure_unique_label(&self.session.keyboard_routes, truncate_label(&label, 48));
            self.session.keyboard_routes.insert(
                label.clone(),
                KeyboardRoute::SelectWorktree {
                    workspace_id: ws.id.0,
                },
            );
            rows.push(vec![label]);
        }
        rows.push(vec![KB_BACK.to_owned(), KB_HOME.to_owned()]);

        let text = "Select a worktree for the new task.";
        let kb = reply_keyboard(rows);
        self.send_message(chat_id, None, text, Some(kb)).await?;
        Ok(())
    }

    async fn send_recent_tasks_menu(&mut self, chat_id: i64) -> anyhow::Result<()> {
        let app = self.engine.app_snapshot().await.context("app snapshot")?;

        let mut out = Vec::new();
        for project in &app.projects {
            for ws in &project.workspaces {
                let snapshot = self
                    .engine
                    .threads_snapshot(luban_api::WorkspaceId(ws.id.0))
                    .await
                    .context("threads snapshot")?;
                for t in snapshot.threads {
                    out.push(ProjectTask {
                        workspace_id: ws.id.0,
                        workspace_name: ws.workspace_name.clone(),
                        thread_id: t.thread_id.0,
                        title: t.title,
                        updated_at_unix_seconds: t.updated_at_unix_seconds,
                    });
                }
            }
        }
        out.sort_by_key(|t| std::cmp::Reverse(t.updated_at_unix_seconds));

        self.session.ui_state = TelegramUiState::Home;
        self.session.keyboard_routes.clear();

        let mut rows = Vec::new();
        for t in out.into_iter().take(12) {
            let label = format!(
                "{} · {}",
                truncate_label(&t.title, 32),
                truncate_label(&t.workspace_name, 16)
            );
            let label = ensure_unique_label(&self.session.keyboard_routes, label);
            self.session.keyboard_routes.insert(
                label.clone(),
                KeyboardRoute::SelectTask {
                    workspace_id: t.workspace_id,
                    thread_id: t.thread_id,
                },
            );
            rows.push(vec![label]);
        }
        rows.push(vec![KB_HOME.to_owned()]);

        let text = "Select a recent task.";
        let kb = reply_keyboard(rows);
        self.send_message(chat_id, None, text, Some(kb)).await?;
        Ok(())
    }

    async fn send_workspaces(&mut self, chat_id: i64) -> anyhow::Result<()> {
        let app = self.engine.app_snapshot().await.context("app snapshot")?;
        let mut rows = Vec::new();
        for project in &app.projects {
            for ws in &project.workspaces {
                let label = format!("{}/{}", project.slug, ws.workspace_name);
                let data = format!("ws:{}", ws.id.0);
                rows.push(vec![InlineButton::new(&label, &data)]);
            }
        }

        let text = "Workspaces";
        let kb = inline_keyboard(rows);
        self.send_message(chat_id, None, text, Some(kb)).await?;
        Ok(())
    }

    async fn send_tasks(&mut self, chat_id: i64, workspace_id: u64) -> anyhow::Result<()> {
        let snapshot = self
            .engine
            .threads_snapshot(luban_api::WorkspaceId(workspace_id))
            .await
            .context("threads snapshot")?;

        let mut rows = Vec::new();
        for t in snapshot.threads.iter().take(8) {
            let label = truncate_label(&t.title, 32);
            let data = format!("task:{}:{}", workspace_id, t.thread_id.0);
            rows.push(vec![InlineButton::new(&label, &data)]);
        }
        rows.push(vec![InlineButton::new(
            "New Task",
            &format!("new:{workspace_id}"),
        )]);
        rows.push(vec![InlineButton::new("Workspaces", "workspaces")]);

        let text = "Tasks";
        let kb = inline_keyboard(rows);
        self.send_message(chat_id, None, text, Some(kb)).await?;
        Ok(())
    }

    async fn send_task_selected(
        &mut self,
        chat_id: i64,
        workspace_id: u64,
        thread_id: u64,
    ) -> anyhow::Result<()> {
        let snapshot = self
            .engine
            .conversation_snapshot(
                luban_api::WorkspaceId(workspace_id),
                luban_api::WorkspaceThreadId(thread_id),
                None,
                Some(25),
            )
            .await
            .context("conversation snapshot")?;

        let title = snapshot.title.trim();
        let title = if title.is_empty() { "Task" } else { title };

        let text = format!("Selected: {title}\n\nSend a message to continue this task.");
        let kb = inline_keyboard(vec![vec![
            InlineButton::new("Tasks", &format!("ws:{workspace_id}")),
            InlineButton::new("Workspaces", "workspaces"),
        ]]);
        if let Some(message_id) = self
            .send_message_with_id(chat_id, None, &text, Some(kb), None)
            .await?
        {
            self.insert_reply_route(message_id, workspace_id, thread_id);
        }
        Ok(())
    }

    async fn forward_conversation_updates(
        &mut self,
        chat_id: i64,
        message_thread_id: Option<i64>,
        snapshot: &luban_api::ConversationSnapshot,
    ) {
        let key = (
            chat_id,
            message_thread_id,
            snapshot.workspace_id.0,
            snapshot.thread_id.0,
        );
        let start = snapshot.entries_start;
        let last_seen = self.last_seen_entry_index.get(&key).copied();

        let mut candidate = None::<(u64, String)>;
        for (idx, entry) in snapshot.entries.iter().enumerate() {
            let global_idx = start.saturating_add(idx as u64);
            if let Some(last_seen) = last_seen
                && global_idx <= last_seen
            {
                continue;
            }

            if let Some(text) = format_conversation_entry_for_telegram(entry) {
                candidate = Some((global_idx, text));
            }
        }

        let Some((global_idx, text)) = candidate else {
            return;
        };

        let title = snapshot.title.trim();
        let title = if title.is_empty() { "Task" } else { title };

        let formatted = format_task_push_markdown(title, &text);
        let kb = inline_keyboard(vec![vec![InlineButton::new(
            "Comment",
            &format!(
                "comment:{}:{}",
                snapshot.workspace_id.0, snapshot.thread_id.0
            ),
        )]]);
        let sent = self
            .send_message_with_id(
                chat_id,
                message_thread_id,
                &formatted,
                Some(kb),
                Some(TELEGRAM_PARSE_MODE_MARKDOWN_V2),
            )
            .await;
        let Ok(Some(message_id)) = sent else {
            return;
        };

        self.insert_reply_route(message_id, snapshot.workspace_id.0, snapshot.thread_id.0);
        self.last_seen_entry_index.insert(key, global_idx);
    }

    async fn send_message(
        &self,
        chat_id: i64,
        message_thread_id: Option<i64>,
        text: &str,
        reply_markup: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        let _ = self
            .send_message_with_id(chat_id, message_thread_id, text, reply_markup, None)
            .await?;
        Ok(())
    }

    async fn send_message_with_id(
        &self,
        chat_id: i64,
        message_thread_id: Option<i64>,
        text: &str,
        reply_markup: Option<serde_json::Value>,
        parse_mode: Option<&str>,
    ) -> anyhow::Result<Option<i64>> {
        let Some(token) = self.runtime.bot_token.as_deref() else {
            return Ok(None);
        };
        let url = format!(
            "{}/bot{}/sendMessage",
            self.api_base.trim_end_matches('/'),
            token
        );

        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "disable_web_page_preview": true,
        });
        if let Some(topic_id) = message_thread_id {
            body["message_thread_id"] = serde_json::json!(topic_id);
        }
        if let Some(markup) = reply_markup {
            body["reply_markup"] = markup;
        }
        if let Some(parse_mode) = parse_mode {
            body["parse_mode"] = serde_json::json!(parse_mode);
        }

        let res = self.http.post(url).json(&body).send().await;
        let res = match res {
            Ok(res) => res,
            Err(err) => {
                self.set_last_error(format!("telegram sendMessage failed: {err}"))
                    .await;
                return Ok(None);
            }
        };

        let parsed = res
            .json::<TelegramApiResponse<TelegramSendMessageResult>>()
            .await;
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(err) => {
                self.set_last_error(format!("telegram sendMessage response parse failed: {err}"))
                    .await;
                return Ok(None);
            }
        };

        if !parsed.ok {
            let message = parsed
                .description
                .unwrap_or_else(|| "telegram sendMessage failed".to_owned());
            self.set_last_error(message).await;
            return Ok(None);
        }

        Ok(Some(parsed.result.message_id))
    }

    async fn edit_forum_topic(
        &self,
        chat_id: i64,
        topic_id: i64,
        name: &str,
    ) -> anyhow::Result<()> {
        let Some(token) = self.runtime.bot_token.as_deref() else {
            return Ok(());
        };
        let url = format!(
            "{}/bot{}/editForumTopic",
            self.api_base.trim_end_matches('/'),
            token
        );

        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_thread_id": topic_id,
            "name": name,
        });

        let res = self.http.post(url).json(&body).send().await;
        let res = match res {
            Ok(res) => res,
            Err(err) => {
                self.set_last_error(format!("telegram editForumTopic failed: {err}"))
                    .await;
                return Ok(());
            }
        };

        let parsed = res.json::<TelegramApiResponse<bool>>().await;
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(err) => {
                self.set_last_error(format!(
                    "telegram editForumTopic response parse failed: {err}"
                ))
                .await;
                return Ok(());
            }
        };

        if !parsed.ok {
            let message = parsed
                .description
                .unwrap_or_else(|| "telegram editForumTopic failed".to_owned());
            self.set_last_error(message).await;
        }

        Ok(())
    }

    async fn answer_callback_query(&self, id: &str) -> anyhow::Result<()> {
        let Some(token) = self.runtime.bot_token.as_deref() else {
            return Ok(());
        };
        let url = format!(
            "{}/bot{}/answerCallbackQuery",
            self.api_base.trim_end_matches('/'),
            token
        );
        let body = serde_json::json!({ "callback_query_id": id });
        let _ = self.http.post(url).json(&body).send().await;
        Ok(())
    }

    async fn set_last_error(&self, message: String) {
        let next = message.trim();
        let message = if next.is_empty() {
            None
        } else if next.len() <= 1024 {
            Some(next.to_owned())
        } else {
            Some(next.chars().take(1024).collect::<String>())
        };

        let _ = self
            .engine
            .dispatch_domain_action(Action::TelegramLastErrorSet { message })
            .await;
    }
}

async fn poll_updates(
    http: reqwest::Client,
    api_base: String,
    token: String,
    offset: i64,
) -> Result<(Vec<TelegramUpdate>, i64), String> {
    let url = format!(
        "{}/bot{}/getUpdates?timeout={}&offset={}",
        api_base.trim_end_matches('/'),
        token,
        TELEGRAM_LONG_POLL_TIMEOUT_SECS,
        offset
    );
    let res = http
        .get(url)
        .send()
        .await
        .map_err(|err| format!("telegram getUpdates failed: {err}"))?;

    let parsed = res
        .json::<TelegramApiResponse<Vec<TelegramUpdate>>>()
        .await
        .map_err(|err| format!("telegram getUpdates parse failed: {err}"))?;

    if !parsed.ok {
        return Err(parsed
            .description
            .unwrap_or_else(|| "telegram getUpdates failed".to_owned()));
    }

    let mut next_offset = offset;
    for update in &parsed.result {
        next_offset = next_offset.max(update.update_id.saturating_add(1));
    }

    Ok((parsed.result, next_offset))
}

#[derive(Clone, Debug)]
enum CallbackAction {
    Home,
    Workspaces,
    SelectWorkspace { workspace_id: u64 },
    SelectTask { workspace_id: u64, thread_id: u64 },
    NewTask { workspace_id: u64 },
    Comment { workspace_id: u64, thread_id: u64 },
}

#[derive(Clone, Debug)]
enum TopicCallbackAction {
    ShowProjectMenu {
        topic_id: i64,
    },
    SelectProject {
        topic_id: i64,
        project_slug: String,
    },
    BindTask {
        topic_id: i64,
        workspace_id: u64,
        thread_id: u64,
    },
    NewTask {
        topic_id: i64,
        project_slug: String,
    },
    CreateTask {
        topic_id: i64,
        workspace_id: u64,
    },
    Unbind {
        topic_id: i64,
    },
}

#[derive(Clone, Debug)]
struct ProjectTask {
    workspace_id: u64,
    workspace_name: String,
    thread_id: u64,
    title: String,
    updated_at_unix_seconds: u64,
}

fn parse_callback_action(raw: &str) -> CallbackAction {
    if raw == "home" {
        return CallbackAction::Home;
    }
    if raw == "workspaces" {
        return CallbackAction::Workspaces;
    }
    if let Some(rest) = raw.strip_prefix("ws:")
        && let Ok(workspace_id) = rest.parse::<u64>()
    {
        return CallbackAction::SelectWorkspace { workspace_id };
    }
    if let Some(rest) = raw.strip_prefix("task:") {
        let mut parts = rest.split(':');
        if let (Some(wid), Some(tid)) = (parts.next(), parts.next())
            && let (Ok(workspace_id), Ok(thread_id)) = (wid.parse::<u64>(), tid.parse::<u64>())
        {
            return CallbackAction::SelectTask {
                workspace_id,
                thread_id,
            };
        }
    }
    if let Some(rest) = raw.strip_prefix("new:")
        && let Ok(workspace_id) = rest.parse::<u64>()
    {
        return CallbackAction::NewTask { workspace_id };
    }
    if let Some(rest) = raw.strip_prefix("comment:") {
        let mut parts = rest.split(':');
        if let (Some(wid), Some(tid)) = (parts.next(), parts.next())
            && let (Ok(workspace_id), Ok(thread_id)) = (wid.parse::<u64>(), tid.parse::<u64>())
        {
            return CallbackAction::Comment {
                workspace_id,
                thread_id,
            };
        }
    }
    CallbackAction::Home
}

fn parse_topic_callback_action(raw: &str) -> Option<TopicCallbackAction> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    if let Some(rest) = raw.strip_prefix("tproj:") {
        let mut parts = rest.split(':');
        let topic_id = parts.next()?.parse::<i64>().ok()?;
        let slug = parts.next().map(str::trim).unwrap_or_default();
        if slug.is_empty() {
            return Some(TopicCallbackAction::ShowProjectMenu { topic_id });
        }
        return Some(TopicCallbackAction::SelectProject {
            topic_id,
            project_slug: slug.to_owned(),
        });
    }

    if let Some(rest) = raw.strip_prefix("tbind:") {
        let mut parts = rest.split(':');
        let topic_id = parts.next()?.parse::<i64>().ok()?;
        let workspace_id = parts.next()?.parse::<u64>().ok()?;
        let thread_id = parts.next()?.parse::<u64>().ok()?;
        return Some(TopicCallbackAction::BindTask {
            topic_id,
            workspace_id,
            thread_id,
        });
    }

    if let Some(rest) = raw.strip_prefix("tnew:") {
        let mut parts = rest.split(':');
        let topic_id = parts.next()?.parse::<i64>().ok()?;
        let slug = parts.next()?.trim();
        if slug.is_empty() {
            return None;
        }
        return Some(TopicCallbackAction::NewTask {
            topic_id,
            project_slug: slug.to_owned(),
        });
    }

    if let Some(rest) = raw.strip_prefix("tcreate:") {
        let mut parts = rest.split(':');
        let topic_id = parts.next()?.parse::<i64>().ok()?;
        let workspace_id = parts.next()?.parse::<u64>().ok()?;
        return Some(TopicCallbackAction::CreateTask {
            topic_id,
            workspace_id,
        });
    }

    if let Some(rest) = raw.strip_prefix("tunbind:")
        && let Ok(topic_id) = rest.trim().parse::<i64>()
    {
        return Some(TopicCallbackAction::Unbind { topic_id });
    }

    None
}

#[derive(Clone, Debug)]
struct InlineButton {
    text: String,
    callback_data: String,
}

impl InlineButton {
    fn new(text: &str, callback_data: &str) -> Self {
        Self {
            text: truncate_label(text, 64),
            callback_data: callback_data.to_owned(),
        }
    }
}

fn inline_keyboard(rows: Vec<Vec<InlineButton>>) -> serde_json::Value {
    let inline_keyboard = rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|b| {
                    serde_json::json!({
                        "text": b.text,
                        "callback_data": b.callback_data,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    serde_json::json!({ "inline_keyboard": inline_keyboard })
}

fn reply_keyboard(rows: Vec<Vec<String>>) -> serde_json::Value {
    let keyboard = rows
        .into_iter()
        .map(|row| row.into_iter().map(serde_json::Value::String).collect())
        .collect::<Vec<Vec<_>>>();
    serde_json::json!({
        "keyboard": keyboard,
        "resize_keyboard": true,
    })
}

fn home_reply_keyboard() -> serde_json::Value {
    reply_keyboard(vec![
        vec![KB_PROJECTS.to_owned(), KB_RECENT_TASKS.to_owned()],
        vec![KB_ACTIVE_TASK.to_owned(), KB_HOME.to_owned()],
        vec![KB_CANCEL.to_owned()],
    ])
}

fn ensure_unique_label(routes: &HashMap<String, KeyboardRoute>, raw: String) -> String {
    let trimmed = raw.trim();
    let trimmed = if trimmed.is_empty() { "Item" } else { trimmed };

    for suffix in 0..32 {
        let candidate = if suffix == 0 {
            trimmed.to_owned()
        } else {
            format!("{trimmed} ({suffix})")
        };

        if is_reserved_keyboard_label(&candidate) {
            continue;
        }
        if !routes.contains_key(&candidate) {
            return candidate;
        }
    }

    truncate_label(trimmed, 48)
}

fn is_reserved_keyboard_label(raw: &str) -> bool {
    matches!(
        raw,
        KB_HOME | KB_PROJECTS | KB_RECENT_TASKS | KB_ACTIVE_TASK | KB_BACK | KB_CANCEL
    )
}

fn truncate_label(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_owned();
    }
    let mut out = trimmed
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}

fn truncate_message(raw: &str) -> String {
    if raw.chars().count() <= TELEGRAM_MAX_MESSAGE_CHARS {
        return raw.to_owned();
    }
    let mut out = raw
        .chars()
        .take(TELEGRAM_MAX_MESSAGE_CHARS.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}

fn format_task_push_markdown(task_title: &str, raw: &str) -> String {
    let title = truncate_label(task_title, 48);
    let title = escape_markdown_v2(&title);

    let body_budget = TELEGRAM_MAX_MESSAGE_CHARS
        .saturating_sub(title.chars().count())
        .saturating_sub(3);
    let body = escape_markdown_v2(raw);
    let body = truncate_markdown_v2_body(&body, body_budget);

    format!("*{title}*\n{body}")
}

fn truncate_markdown_v2_body(raw: &str, max_chars: usize) -> String {
    if raw.chars().count() <= max_chars {
        return raw.to_owned();
    }
    let mut out = raw
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    while out.ends_with('\\') {
        out.pop();
    }
    out.push('…');
    out
}

fn escape_markdown_v2(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().saturating_add(8));
    for ch in raw.chars() {
        if matches!(
            ch,
            '_' | '*'
                | '['
                | ']'
                | '('
                | ')'
                | '~'
                | '`'
                | '>'
                | '#'
                | '+'
                | '-'
                | '='
                | '|'
                | '{'
                | '}'
                | '.'
                | '!'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn sanitize_telegram_topic_name(raw: &str) -> String {
    const LIMIT: usize = 128;
    let collapsed = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = collapsed.trim();
    let trimmed = if trimmed.is_empty() { "Task" } else { trimmed };
    trimmed.chars().take(LIMIT).collect()
}

fn format_conversation_entry_for_telegram(entry: &ConversationEntry) -> Option<String> {
    match entry {
        ConversationEntry::AgentEvent(v) => match &v.event {
            luban_api::AgentEvent::Message(msg) => Some(msg.text.clone()),
            luban_api::AgentEvent::TurnDuration { duration_ms } => {
                Some(format!("Turn completed ({duration_ms} ms)"))
            }
            luban_api::AgentEvent::TurnError { message } => Some(format!("Turn failed: {message}")),
            luban_api::AgentEvent::TurnCanceled => Some("Turn canceled.".to_owned()),
            _ => None,
        },
        _ => None,
    }
}

fn resolve_message_target(
    msg: &TelegramMessage,
    session: &TelegramSession,
    routes: &HashMap<i64, ReplyRoute>,
    topic_bindings: &HashMap<i64, TopicBinding>,
    now: Instant,
) -> Option<(u64, u64)> {
    let ttl = Duration::from_secs(TELEGRAM_REPLY_ROUTE_TTL_SECS);
    if let Some(reply_to) = msg.reply_to_message.as_deref()
        && let Some(route) = routes.get(&reply_to.message_id)
        && now.duration_since(route.created_at) <= ttl
    {
        return Some((route.workspace_id, route.thread_id));
    }

    if let Some(topic_id) = msg.message_thread_id
        && let Some(binding) = topic_bindings.get(&topic_id)
    {
        return Some((binding.workspace_id, binding.thread_id));
    }

    match (session.active_workspace_id, session.active_thread_id) {
        (Some(wid), Some(tid)) => Some((wid, tid)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg_with_reply(reply_to_message_id: i64) -> TelegramMessage {
        TelegramMessage {
            message_id: 100,
            chat: TelegramChat {
                id: 1,
                kind: Some("private".to_owned()),
            },
            text: Some("hello".to_owned()),
            message_thread_id: None,
            is_topic_message: None,
            reply_to_message: Some(Box::new(TelegramMessage {
                message_id: reply_to_message_id,
                chat: TelegramChat {
                    id: 1,
                    kind: Some("private".to_owned()),
                },
                text: None,
                message_thread_id: None,
                is_topic_message: None,
                reply_to_message: None,
            })),
        }
    }

    #[test]
    fn resolve_message_target_prefers_reply_route() {
        let now = Instant::now();
        let mut routes = HashMap::new();
        routes.insert(
            10,
            ReplyRoute {
                workspace_id: 1,
                thread_id: 2,
                created_at: now,
            },
        );
        let session = TelegramSession {
            active_workspace_id: Some(9),
            active_thread_id: Some(9),
            ..Default::default()
        };
        let topic_bindings = HashMap::new();
        assert_eq!(
            resolve_message_target(&msg_with_reply(10), &session, &routes, &topic_bindings, now),
            Some((1, 2))
        );
    }

    #[test]
    fn resolve_message_target_ignores_expired_reply_route() {
        let now = Instant::now();
        let mut routes = HashMap::new();
        routes.insert(
            10,
            ReplyRoute {
                workspace_id: 1,
                thread_id: 2,
                created_at: now - Duration::from_secs(TELEGRAM_REPLY_ROUTE_TTL_SECS + 1),
            },
        );
        let session = TelegramSession {
            active_workspace_id: Some(3),
            active_thread_id: Some(4),
            ..Default::default()
        };
        let topic_bindings = HashMap::new();
        assert_eq!(
            resolve_message_target(&msg_with_reply(10), &session, &routes, &topic_bindings, now),
            Some((3, 4))
        );
    }

    #[test]
    fn resolve_message_target_falls_back_to_session() {
        let now = Instant::now();
        let routes = HashMap::new();
        let topic_bindings = HashMap::new();
        let session = TelegramSession {
            active_workspace_id: Some(3),
            active_thread_id: Some(4),
            ..Default::default()
        };
        let msg = TelegramMessage {
            message_id: 1,
            chat: TelegramChat {
                id: 1,
                kind: Some("private".to_owned()),
            },
            text: Some("hello".to_owned()),
            message_thread_id: None,
            is_topic_message: None,
            reply_to_message: None,
        };
        assert_eq!(
            resolve_message_target(&msg, &session, &routes, &topic_bindings, now),
            Some((3, 4))
        );
    }

    #[test]
    fn resolve_message_target_returns_none_without_route_or_session() {
        let now = Instant::now();
        let routes = HashMap::new();
        let topic_bindings = HashMap::new();
        let session = TelegramSession::default();
        let msg = TelegramMessage {
            message_id: 1,
            chat: TelegramChat {
                id: 1,
                kind: Some("private".to_owned()),
            },
            text: Some("hello".to_owned()),
            message_thread_id: None,
            is_topic_message: None,
            reply_to_message: None,
        };
        assert_eq!(
            resolve_message_target(&msg, &session, &routes, &topic_bindings, now),
            None
        );
    }

    #[test]
    fn resolve_message_target_uses_topic_binding() {
        let now = Instant::now();
        let routes = HashMap::new();
        let session = TelegramSession::default();
        let mut topic_bindings = HashMap::new();
        topic_bindings.insert(
            55,
            TopicBinding {
                workspace_id: 7,
                thread_id: 8,
                replayed_up_to: None,
            },
        );
        let msg = TelegramMessage {
            message_id: 1,
            chat: TelegramChat {
                id: 1,
                kind: Some("private".to_owned()),
            },
            text: Some("hello".to_owned()),
            message_thread_id: Some(55),
            is_topic_message: Some(true),
            reply_to_message: None,
        };
        assert_eq!(
            resolve_message_target(&msg, &session, &routes, &topic_bindings, now),
            Some((7, 8))
        );
    }
}
