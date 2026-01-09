use anyhow::Context as _;
use luban_api::{
    AppSnapshot, ConversationSnapshot, PullRequestCiState, PullRequestSnapshot, PullRequestState,
    ThreadsSnapshot, WsServerMessage,
};
use luban_backend::{GitWorkspaceService, SqliteStoreOptions};
use luban_domain::{
    Action, AppState, AttachmentKind, AttachmentRef, CodexThreadItem, ConversationEntry, Effect,
    OperationStatus, ProjectWorkspaceService, PullRequestCiState as DomainPullRequestCiState,
    PullRequestInfo, PullRequestState as DomainPullRequestState, ThinkingEffort, WorkspaceId,
    WorkspaceThreadId, default_agent_model_id, default_thinking_effort,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, oneshot};

#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::Sender<EngineCommand>,
}

impl EngineHandle {
    pub async fn current_rev(&self) -> anyhow::Result<u64> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::GetRev { reply: tx })
            .await
            .context("engine unavailable")?;
        rx.await.context("engine stopped")?
    }

    pub async fn app_snapshot(&self) -> anyhow::Result<AppSnapshot> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::GetAppSnapshot { reply: tx })
            .await
            .context("engine unavailable")?;
        rx.await.context("engine stopped")?
    }

    pub async fn threads_snapshot(
        &self,
        workspace_id: luban_api::WorkspaceId,
    ) -> anyhow::Result<ThreadsSnapshot> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::GetThreadsSnapshot {
                workspace_id,
                reply: tx,
            })
            .await
            .context("engine unavailable")?;
        rx.await.context("engine stopped")?
    }

    pub async fn conversation_snapshot(
        &self,
        workspace_id: luban_api::WorkspaceId,
        thread_id: luban_api::WorkspaceThreadId,
    ) -> anyhow::Result<ConversationSnapshot> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::GetConversationSnapshot {
                workspace_id,
                thread_id,
                reply: tx,
            })
            .await
            .context("engine unavailable")?;
        rx.await.context("engine stopped")?
    }

    pub async fn workspace_worktree_path(
        &self,
        workspace_id: luban_api::WorkspaceId,
    ) -> anyhow::Result<Option<PathBuf>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::GetWorkspaceWorktreePath {
                workspace_id,
                reply: tx,
            })
            .await
            .context("engine unavailable")?;
        rx.await.context("engine stopped")?
    }

    pub async fn apply_client_action(
        &self,
        action: luban_api::ClientAction,
    ) -> Result<u64, String> {
        let (tx, rx) = oneshot::channel();
        if self
            .tx
            .send(EngineCommand::ApplyClientAction { action, reply: tx })
            .await
            .is_err()
        {
            return Err("engine unavailable".to_owned());
        }
        rx.await
            .unwrap_or_else(|_| Err("engine stopped".to_owned()))
    }
}

pub enum EngineCommand {
    GetRev {
        reply: oneshot::Sender<anyhow::Result<u64>>,
    },
    GetAppSnapshot {
        reply: oneshot::Sender<anyhow::Result<AppSnapshot>>,
    },
    GetThreadsSnapshot {
        workspace_id: luban_api::WorkspaceId,
        reply: oneshot::Sender<anyhow::Result<ThreadsSnapshot>>,
    },
    GetConversationSnapshot {
        workspace_id: luban_api::WorkspaceId,
        thread_id: luban_api::WorkspaceThreadId,
        reply: oneshot::Sender<anyhow::Result<ConversationSnapshot>>,
    },
    GetWorkspaceWorktreePath {
        workspace_id: luban_api::WorkspaceId,
        reply: oneshot::Sender<anyhow::Result<Option<PathBuf>>>,
    },
    ApplyClientAction {
        action: luban_api::ClientAction,
        reply: oneshot::Sender<Result<u64, String>>,
    },
    DispatchAction {
        action: Box<Action>,
    },
    RefreshPullRequests {
        workspace_id: Option<WorkspaceId>,
    },
    PullRequestInfoUpdated {
        workspace_id: WorkspaceId,
        info: Option<PullRequestInfo>,
    },
}

#[derive(Clone, Debug)]
struct PullRequestCacheEntry {
    info: Option<PullRequestInfo>,
    refreshed_at: Instant,
}

const PULL_REQUEST_REFRESH_MIN_INTERVAL: Duration = Duration::from_secs(15);
const PULL_REQUEST_REFRESH_TICK_INTERVAL: Duration = Duration::from_secs(30);

pub struct Engine {
    state: AppState,
    rev: u64,
    services: Arc<dyn ProjectWorkspaceService>,
    events: broadcast::Sender<WsServerMessage>,
    tx: mpsc::Sender<EngineCommand>,
    cancel_flags: HashMap<(WorkspaceId, WorkspaceThreadId), Arc<AtomicBool>>,
    pull_requests: HashMap<WorkspaceId, PullRequestCacheEntry>,
    pull_requests_in_flight: HashSet<WorkspaceId>,
}

impl Engine {
    pub fn start(
        services: Arc<dyn ProjectWorkspaceService>,
    ) -> (EngineHandle, broadcast::Sender<WsServerMessage>) {
        let (tx, mut rx) = mpsc::channel::<EngineCommand>(256);
        let (events, _) = broadcast::channel::<WsServerMessage>(256);

        let mut engine = Self {
            state: AppState::new(),
            rev: 0,
            services,
            events: events.clone(),
            tx: tx.clone(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        let refresh_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(PULL_REQUEST_REFRESH_TICK_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let _ = refresh_tx
                    .send(EngineCommand::RefreshPullRequests { workspace_id: None })
                    .await;
            }
        });

        tokio::spawn(async move {
            engine.bootstrap().await;
            while let Some(cmd) = rx.recv().await {
                engine.handle(cmd).await;
            }
        });

        (EngineHandle { tx }, events)
    }

    async fn bootstrap(&mut self) {
        self.process_action_queue(Action::AppStarted).await;
    }

    async fn handle(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::GetRev { reply } => {
                let _ = reply.send(Ok(self.rev));
            }
            EngineCommand::GetAppSnapshot { reply } => {
                self.refresh_pull_requests_for_all_workspaces();
                let _ = reply.send(Ok(self.app_snapshot()));
            }
            EngineCommand::GetThreadsSnapshot {
                workspace_id,
                reply,
            } => {
                let wid = WorkspaceId::from_u64(workspace_id.0);
                let Some(scope) = workspace_scope(&self.state, wid) else {
                    let _ = reply.send(Err(anyhow::anyhow!("workspace not found")));
                    return;
                };

                let services = self.services.clone();
                let threads = tokio::task::spawn_blocking(move || {
                    services.list_conversation_threads(scope.project_slug, scope.workspace_name)
                })
                .await
                .ok()
                .unwrap_or_else(|| Err("failed to join list threads task".to_owned()));

                let snapshot = threads.map(|threads| ThreadsSnapshot {
                    rev: self.rev,
                    workspace_id,
                    threads: threads
                        .into_iter()
                        .map(|t| luban_api::ThreadMeta {
                            thread_id: luban_api::WorkspaceThreadId(t.thread_id.as_u64()),
                            remote_thread_id: t.remote_thread_id,
                            title: t.title,
                            updated_at_unix_seconds: t.updated_at_unix_seconds,
                        })
                        .collect(),
                });

                let _ = reply.send(snapshot.map_err(|e| anyhow::anyhow!(e)));
            }
            EngineCommand::GetConversationSnapshot {
                workspace_id,
                thread_id,
                reply,
            } => {
                let snapshot = self
                    .get_conversation_snapshot(workspace_id, thread_id)
                    .await;
                let _ = reply.send(snapshot);
            }
            EngineCommand::GetWorkspaceWorktreePath {
                workspace_id,
                reply,
            } => {
                let id = WorkspaceId::from_u64(workspace_id.0);
                let path = self.state.workspace(id).map(|w| w.worktree_path.clone());
                let _ = reply.send(Ok(path));
            }
            EngineCommand::ApplyClientAction { action, reply } => {
                if matches!(action, luban_api::ClientAction::PickProjectPath) {
                    let tx = self.tx.clone();
                    let events = self.events.clone();
                    let rev = self.rev;
                    tokio::task::spawn_blocking(move || {
                        let picked = pick_project_folder();
                        match picked {
                            Some(path) => {
                                let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                    action: Box::new(Action::AddProject { path }),
                                });
                            }
                            None => {
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: luban_api::ServerEvent::Toast {
                                        message: "No folder selected".to_owned(),
                                    },
                                });
                            }
                        }
                    });
                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::OpenWorkspace { workspace_id } = &action {
                    self.maybe_refresh_pull_request(WorkspaceId::from_u64(workspace_id.0));
                }

                let mapped = map_client_action(action);
                let Some(action) = mapped else {
                    let _ = reply.send(Err("unsupported action".to_owned()));
                    return;
                };

                self.process_action_queue(action).await;
                let _ = reply.send(Ok(self.rev));
            }
            EngineCommand::DispatchAction { action } => {
                self.process_action_queue(*action).await;
            }
            EngineCommand::RefreshPullRequests { workspace_id } => match workspace_id {
                Some(id) => self.maybe_refresh_pull_request(id),
                None => self.refresh_pull_requests_for_all_workspaces(),
            },
            EngineCommand::PullRequestInfoUpdated { workspace_id, info } => {
                self.pull_requests_in_flight.remove(&workspace_id);

                let changed = self
                    .pull_requests
                    .get(&workspace_id)
                    .map(|e| e.info != info)
                    .unwrap_or(true);

                self.pull_requests.insert(
                    workspace_id,
                    PullRequestCacheEntry {
                        info,
                        refreshed_at: Instant::now(),
                    },
                );

                if changed {
                    self.rev = self.rev.saturating_add(1);
                    self.publish_app_snapshot();
                }
            }
        }
    }

    async fn get_conversation_snapshot(
        &self,
        workspace_id: luban_api::WorkspaceId,
        thread_id: luban_api::WorkspaceThreadId,
    ) -> anyhow::Result<ConversationSnapshot> {
        if let Ok(snapshot) = self.conversation_snapshot(workspace_id, thread_id) {
            return Ok(snapshot);
        }

        let wid = WorkspaceId::from_u64(workspace_id.0);
        let Some(scope) = workspace_scope(&self.state, wid) else {
            return Err(anyhow::anyhow!("workspace not found"));
        };

        let services = self.services.clone();
        let tid = thread_id.0;
        let loaded = tokio::task::spawn_blocking(move || {
            services.load_conversation(scope.project_slug, scope.workspace_name, tid)
        })
        .await
        .ok()
        .unwrap_or_else(|| Err("failed to join load conversation task".to_owned()))
        .map_err(|e| anyhow::anyhow!(e))?;

        Ok(ConversationSnapshot {
            rev: self.rev,
            workspace_id,
            thread_id,
            agent_model_id: default_agent_model_id().to_owned(),
            thinking_effort: match default_thinking_effort() {
                ThinkingEffort::Low => luban_api::ThinkingEffort::Low,
                ThinkingEffort::Medium => luban_api::ThinkingEffort::Medium,
                ThinkingEffort::High => luban_api::ThinkingEffort::High,
                ThinkingEffort::XHigh => luban_api::ThinkingEffort::XHigh,
            },
            run_status: luban_api::OperationStatus::Idle,
            entries: loaded.entries.iter().map(map_conversation_entry).collect(),
            in_progress_items: Vec::new(),
            remote_thread_id: loaded.thread_id,
            title: format!("Thread {tid}"),
        })
    }

    async fn process_action_queue(&mut self, initial: Action) {
        let mut actions = VecDeque::from([initial]);
        let mut effects = VecDeque::<Effect>::new();

        while let Some(action) = actions.pop_front() {
            self.rev = self.rev.saturating_add(1);

            let conversation_key = conversation_key_for_action(&action);
            let threads_event = threads_event_for_action(&action);

            let new_effects = self.state.apply(action);
            self.publish_app_snapshot();

            if let Some((wid, tid)) = conversation_key {
                self.publish_conversation_snapshot(wid, tid);
            }
            if let Some((wid, threads)) = threads_event {
                self.publish_threads_event(wid, &threads);
            }

            effects.extend(new_effects);

            while let Some(effect) = effects.pop_front() {
                match self.run_effect(effect).await {
                    Ok(mut followups) => actions.append(&mut followups),
                    Err(err) => {
                        tracing::error!(error = %err, "effect failed");
                    }
                }
            }
        }
    }

    fn refresh_pull_requests_for_all_workspaces(&mut self) {
        let workspace_ids = self
            .state
            .projects
            .iter()
            .flat_map(|project| {
                project.workspaces.iter().filter_map(|workspace| {
                    if workspace.status != luban_domain::WorkspaceStatus::Active {
                        return None;
                    }
                    Some(workspace.id)
                })
            })
            .collect::<Vec<_>>();

        for workspace_id in workspace_ids {
            self.maybe_refresh_pull_request(workspace_id);
        }
    }

    fn maybe_refresh_pull_request(&mut self, workspace_id: WorkspaceId) {
        if self.pull_requests_in_flight.contains(&workspace_id) {
            return;
        }

        if let Some(entry) = self.pull_requests.get(&workspace_id)
            && entry.refreshed_at.elapsed() < PULL_REQUEST_REFRESH_MIN_INTERVAL
        {
            return;
        }

        let Some(workspace) = self.state.workspace(workspace_id) else {
            return;
        };

        self.pull_requests_in_flight.insert(workspace_id);

        let services = self.services.clone();
        let tx = self.tx.clone();
        let worktree_path = workspace.worktree_path.clone();

        std::thread::spawn(move || {
            let info = services.gh_pull_request_info(worktree_path).ok().flatten();
            let _ = tx.blocking_send(EngineCommand::PullRequestInfoUpdated { workspace_id, info });
        });
    }

    async fn run_effect(&mut self, effect: Effect) -> anyhow::Result<VecDeque<Action>> {
        match effect {
            Effect::LoadAppState => {
                let services = self.services.clone();
                let loaded = tokio::task::spawn_blocking(move || services.load_app_state())
                    .await
                    .ok()
                    .unwrap_or_else(|| Err("failed to join load task".to_owned()));
                let action = match loaded {
                    Ok(persisted) => Action::AppStateLoaded { persisted },
                    Err(message) => Action::AppStateLoadFailed { message },
                };
                Ok(VecDeque::from([action]))
            }
            Effect::SaveAppState => {
                let services = self.services.clone();
                let snapshot = self.state.to_persisted();
                let saved = tokio::task::spawn_blocking(move || services.save_app_state(snapshot))
                    .await
                    .ok()
                    .unwrap_or_else(|| Err("failed to join save task".to_owned()));
                let action = match saved {
                    Ok(()) => Action::AppStateSaved,
                    Err(message) => Action::AppStateSaveFailed { message },
                };
                Ok(VecDeque::from([action]))
            }
            Effect::CreateWorkspace { project_id } => {
                let Some(project) = self.state.projects.iter().find(|p| p.id == project_id) else {
                    return Ok(VecDeque::from([Action::WorkspaceCreateFailed {
                        project_id,
                        message: "project not found".to_owned(),
                    }]));
                };
                let project_path = project.path.clone();
                let project_slug = project.slug.clone();
                let services = self.services.clone();

                let created = tokio::task::spawn_blocking(move || {
                    services.create_workspace(project_path, project_slug)
                })
                .await
                .ok()
                .unwrap_or_else(|| Err("failed to join create workspace task".to_owned()));

                let action = match created {
                    Ok(created) => Action::WorkspaceCreated {
                        project_id,
                        workspace_name: created.workspace_name,
                        branch_name: created.branch_name,
                        worktree_path: created.worktree_path,
                    },
                    Err(message) => Action::WorkspaceCreateFailed {
                        project_id,
                        message,
                    },
                };
                Ok(VecDeque::from([action]))
            }
            Effect::LoadWorkspaceThreads { workspace_id } => {
                let Some(scope) = workspace_scope(&self.state, workspace_id) else {
                    return Ok(VecDeque::new());
                };
                let services = self.services.clone();
                let result = tokio::task::spawn_blocking(move || {
                    services.list_conversation_threads(scope.project_slug, scope.workspace_name)
                })
                .await
                .ok()
                .unwrap_or_else(|| Err("failed to join list threads task".to_owned()));
                let action = match result {
                    Ok(threads) => Action::WorkspaceThreadsLoaded {
                        workspace_id,
                        threads,
                    },
                    Err(message) => Action::WorkspaceThreadsLoadFailed {
                        workspace_id,
                        message,
                    },
                };
                Ok(VecDeque::from([action]))
            }
            Effect::LoadConversation {
                workspace_id,
                thread_id,
            } => {
                let Some(scope) = workspace_scope(&self.state, workspace_id) else {
                    return Ok(VecDeque::new());
                };
                let services = self.services.clone();
                let thread_local_id = thread_id.as_u64();
                let result = tokio::task::spawn_blocking(move || {
                    services.load_conversation(
                        scope.project_slug,
                        scope.workspace_name,
                        thread_local_id,
                    )
                })
                .await
                .ok()
                .unwrap_or_else(|| Err("failed to join load conversation task".to_owned()));
                let action = match result {
                    Ok(snapshot) => Action::ConversationLoaded {
                        workspace_id,
                        thread_id,
                        snapshot,
                    },
                    Err(message) => Action::ConversationLoadFailed {
                        workspace_id,
                        thread_id,
                        message,
                    },
                };
                Ok(VecDeque::from([action]))
            }
            Effect::EnsureConversation {
                workspace_id,
                thread_id,
            } => {
                let Some(scope) = workspace_scope(&self.state, workspace_id) else {
                    return Ok(VecDeque::new());
                };
                let services = self.services.clone();
                let thread_local_id = thread_id.as_u64();
                let _ = tokio::task::spawn_blocking(move || {
                    services.ensure_conversation(
                        scope.project_slug,
                        scope.workspace_name,
                        thread_local_id,
                    )
                })
                .await;
                Ok(VecDeque::new())
            }
            Effect::RunAgentTurn {
                workspace_id,
                thread_id,
                text,
                attachments,
                run_config,
            } => {
                let Some(scope) = workspace_scope(&self.state, workspace_id) else {
                    return Ok(VecDeque::new());
                };

                let worktree_path = self
                    .state
                    .workspace(workspace_id)
                    .map(|w| w.worktree_path.clone())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

                let remote_thread_id = self
                    .state
                    .workspace_thread_conversation(workspace_id, thread_id)
                    .and_then(|c| c.thread_id.clone());

                let request = luban_domain::RunAgentTurnRequest {
                    project_slug: scope.project_slug,
                    workspace_name: scope.workspace_name,
                    worktree_path,
                    thread_local_id: thread_id.as_u64(),
                    thread_id: remote_thread_id,
                    prompt: text,
                    attachments,
                    model: Some(run_config.model_id),
                    model_reasoning_effort: Some(run_config.thinking_effort.as_str().to_owned()),
                };

                let cancel = Arc::new(AtomicBool::new(false));
                self.cancel_flags
                    .insert((workspace_id, thread_id), cancel.clone());

                let services = self.services.clone();
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let on_event: Arc<dyn Fn(luban_domain::CodexThreadEvent) + Send + Sync> = {
                        let tx = tx.clone();
                        Arc::new(move |event| {
                            let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                action: Box::new(Action::AgentEventReceived {
                                    workspace_id,
                                    thread_id,
                                    event,
                                }),
                            });
                        })
                    };

                    let result = services.run_agent_turn_streamed(request, cancel, on_event);
                    if let Err(message) = result {
                        let _ = tx.blocking_send(EngineCommand::DispatchAction {
                            action: Box::new(Action::AgentEventReceived {
                                workspace_id,
                                thread_id,
                                event: luban_domain::CodexThreadEvent::Error { message },
                            }),
                        });
                    }

                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
                        action: Box::new(Action::AgentTurnFinished {
                            workspace_id,
                            thread_id,
                        }),
                    });
                });

                Ok(VecDeque::new())
            }
            Effect::CancelAgentTurn {
                workspace_id,
                thread_id,
            } => {
                if let Some(flag) = self.cancel_flags.get(&(workspace_id, thread_id)) {
                    flag.store(true, Ordering::SeqCst);
                }
                Ok(VecDeque::new())
            }
            Effect::OpenWorkspacePullRequest { workspace_id } => {
                let Some(workspace) = self.state.workspace(workspace_id) else {
                    return Ok(VecDeque::new());
                };
                let worktree_path = workspace.worktree_path.clone();
                let services = self.services.clone();
                let result = tokio::task::spawn_blocking(move || {
                    services.gh_open_pull_request(worktree_path)
                })
                .await
                .ok()
                .unwrap_or_else(|| Err("failed to join open pull request task".to_owned()));
                match result {
                    Ok(()) => Ok(VecDeque::new()),
                    Err(message) => {
                        let _ = self.events.send(WsServerMessage::Event {
                            rev: self.rev,
                            event: luban_api::ServerEvent::Toast {
                                message: message.clone(),
                            },
                        });
                        Ok(VecDeque::from([Action::OpenWorkspacePullRequestFailed {
                            message,
                        }]))
                    }
                }
            }
            Effect::OpenWorkspacePullRequestFailedAction { workspace_id } => {
                let Some(workspace) = self.state.workspace(workspace_id) else {
                    return Ok(VecDeque::new());
                };
                let worktree_path = workspace.worktree_path.clone();
                let services = self.services.clone();
                let result = tokio::task::spawn_blocking(move || {
                    services.gh_open_pull_request_failed_action(worktree_path)
                })
                .await
                .ok()
                .unwrap_or_else(|| {
                    Err("failed to join open pull request failed action task".to_owned())
                });
                match result {
                    Ok(()) => Ok(VecDeque::new()),
                    Err(message) => {
                        let _ = self.events.send(WsServerMessage::Event {
                            rev: self.rev,
                            event: luban_api::ServerEvent::Toast {
                                message: message.clone(),
                            },
                        });
                        Ok(VecDeque::from([
                            Action::OpenWorkspacePullRequestFailedActionFailed { message },
                        ]))
                    }
                }
            }
            _ => Ok(VecDeque::new()),
        }
    }

    fn publish_app_snapshot(&self) {
        let _ = self.events.send(WsServerMessage::Event {
            rev: self.rev,
            event: luban_api::ServerEvent::AppChanged {
                rev: self.rev,
                snapshot: self.app_snapshot(),
            },
        });
    }

    fn publish_threads_event(
        &self,
        workspace_id: WorkspaceId,
        threads: &[luban_domain::ConversationThreadMeta],
    ) {
        let api_id = luban_api::WorkspaceId(workspace_id.as_u64());
        let threads = threads
            .iter()
            .map(|t| luban_api::ThreadMeta {
                thread_id: luban_api::WorkspaceThreadId(t.thread_id.as_u64()),
                remote_thread_id: t.remote_thread_id.clone(),
                title: t.title.clone(),
                updated_at_unix_seconds: t.updated_at_unix_seconds,
            })
            .collect::<Vec<_>>();

        let _ = self.events.send(WsServerMessage::Event {
            rev: self.rev,
            event: luban_api::ServerEvent::WorkspaceThreadsChanged {
                workspace_id: api_id,
                threads,
            },
        });
    }

    fn publish_conversation_snapshot(
        &self,
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    ) {
        let api_wid = luban_api::WorkspaceId(workspace_id.as_u64());
        let api_tid = luban_api::WorkspaceThreadId(thread_id.as_u64());
        if let Ok(snapshot) = self.conversation_snapshot(api_wid, api_tid) {
            let _ = self.events.send(WsServerMessage::Event {
                rev: self.rev,
                event: luban_api::ServerEvent::ConversationChanged { snapshot },
            });
        }
    }

    fn app_snapshot(&self) -> AppSnapshot {
        let mut running_workspaces = std::collections::HashSet::<WorkspaceId>::new();
        for ((workspace_id, _), conversation) in &self.state.conversations {
            if conversation.run_status == OperationStatus::Running {
                running_workspaces.insert(*workspace_id);
            }
        }

        AppSnapshot {
            rev: self.rev,
            projects: self
                .state
                .projects
                .iter()
                .map(|p| luban_api::ProjectSnapshot {
                    // Keep a stable, human-sized identifier for each workspace, derived from
                    // `(project.slug, workspace_id)`.
                    id: luban_api::ProjectId(p.id.as_u64()),
                    name: p.name.clone(),
                    slug: p.slug.clone(),
                    path: p.path.to_string_lossy().to_string(),
                    expanded: p.expanded,
                    create_workspace_status: match p.create_workspace_status {
                        OperationStatus::Idle => luban_api::OperationStatus::Idle,
                        OperationStatus::Running => luban_api::OperationStatus::Running,
                    },
                    workspaces: p
                        .workspaces
                        .iter()
                        .map(|w| luban_api::WorkspaceSnapshot {
                            id: luban_api::WorkspaceId(w.id.as_u64()),
                            short_id: workspace_short_id(&p.slug, w.id.as_u64()),
                            workspace_name: w.workspace_name.clone(),
                            branch_name: w.branch_name.clone(),
                            worktree_path: w.worktree_path.to_string_lossy().to_string(),
                            status: match w.status {
                                luban_domain::WorkspaceStatus::Active => {
                                    luban_api::WorkspaceStatus::Active
                                }
                                luban_domain::WorkspaceStatus::Archived => {
                                    luban_api::WorkspaceStatus::Archived
                                }
                            },
                            agent_run_status: if running_workspaces.contains(&w.id) {
                                luban_api::OperationStatus::Running
                            } else {
                                luban_api::OperationStatus::Idle
                            },
                            has_unread_completion: self
                                .state
                                .workspace_unread_completions
                                .contains(&w.id),
                            pull_request: self
                                .pull_requests
                                .get(&w.id)
                                .and_then(|entry| entry.info)
                                .map(map_pull_request_info),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    // Threads snapshots are served via `ProjectWorkspaceService::list_conversation_threads` in the command handler.

    fn conversation_snapshot(
        &self,
        workspace_id: luban_api::WorkspaceId,
        thread_id: luban_api::WorkspaceThreadId,
    ) -> anyhow::Result<ConversationSnapshot> {
        let wid = WorkspaceId::from_u64(workspace_id.0);
        let tid = WorkspaceThreadId::from_u64(thread_id.0);
        let Some(conversation) = self.state.workspace_thread_conversation(wid, tid) else {
            return Err(anyhow::anyhow!("conversation not found"));
        };

        Ok(ConversationSnapshot {
            rev: self.rev,
            workspace_id,
            thread_id,
            agent_model_id: conversation.agent_model_id.clone(),
            thinking_effort: match conversation.thinking_effort {
                ThinkingEffort::Low => luban_api::ThinkingEffort::Low,
                ThinkingEffort::Medium => luban_api::ThinkingEffort::Medium,
                ThinkingEffort::High => luban_api::ThinkingEffort::High,
                ThinkingEffort::XHigh => luban_api::ThinkingEffort::XHigh,
            },
            run_status: match conversation.run_status {
                OperationStatus::Idle => luban_api::OperationStatus::Idle,
                OperationStatus::Running => luban_api::OperationStatus::Running,
            },
            entries: conversation
                .entries
                .iter()
                .map(map_conversation_entry)
                .collect(),
            in_progress_items: conversation
                .in_progress_order
                .iter()
                .filter_map(|id| conversation.in_progress_items.get(id))
                .map(|item| {
                    let id = codex_item_id(item).to_owned();
                    let (kind, payload) = map_agent_item(item);
                    luban_api::AgentItem { id, kind, payload }
                })
                .collect(),
            remote_thread_id: conversation.thread_id.clone(),
            title: conversation.title.clone(),
        })
    }
}

fn pick_project_folder() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        // `rfd` requires a windowed environment and a main-thread call on macOS. In our
        // localhost server process we may run in a non-windowed environment, so use the
        // system dialog via AppleScript instead.
        let output = Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose folder with prompt \"Select project folder\")",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let raw = String::from_utf8_lossy(&output.stdout);
        let path = raw.trim().trim_end_matches('/').trim();
        if path.is_empty() {
            return None;
        }
        Some(PathBuf::from(path))
    }

    #[cfg(not(target_os = "macos"))]
    {
        rfd::FileDialog::new()
            .set_title("Select project folder")
            .pick_folder()
    }
}

#[derive(Clone)]
struct WorkspaceScope {
    project_slug: String,
    workspace_name: String,
}

fn workspace_scope(state: &AppState, workspace_id: WorkspaceId) -> Option<WorkspaceScope> {
    for project in &state.projects {
        for workspace in &project.workspaces {
            if workspace.id == workspace_id {
                return Some(WorkspaceScope {
                    project_slug: project.slug.clone(),
                    workspace_name: workspace.workspace_name.clone(),
                });
            }
        }
    }
    None
}

fn conversation_key_for_action(action: &Action) -> Option<(WorkspaceId, WorkspaceThreadId)> {
    match action {
        Action::SendAgentMessage {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::ConversationLoaded {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::ConversationLoadFailed {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::AgentEventReceived {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::AgentTurnFinished {
            workspace_id,
            thread_id,
        } => Some((*workspace_id, *thread_id)),
        Action::CancelAgentTurn {
            workspace_id,
            thread_id,
        } => Some((*workspace_id, *thread_id)),
        _ => None,
    }
}

fn threads_event_for_action(
    action: &Action,
) -> Option<(WorkspaceId, Vec<luban_domain::ConversationThreadMeta>)> {
    match action {
        Action::WorkspaceThreadsLoaded {
            workspace_id,
            threads,
        } => Some((*workspace_id, threads.clone())),
        _ => None,
    }
}

fn map_pull_request_info(info: PullRequestInfo) -> PullRequestSnapshot {
    let state = match info.state {
        DomainPullRequestState::Open => PullRequestState::Open,
        DomainPullRequestState::Closed => PullRequestState::Closed,
        DomainPullRequestState::Merged => PullRequestState::Merged,
    };
    let ci_state = info.ci_state.map(|s| match s {
        DomainPullRequestCiState::Pending => PullRequestCiState::Pending,
        DomainPullRequestCiState::Success => PullRequestCiState::Success,
        DomainPullRequestCiState::Failure => PullRequestCiState::Failure,
    });
    PullRequestSnapshot {
        number: info.number,
        is_draft: info.is_draft,
        state,
        ci_state,
        merge_ready: info.merge_ready,
    }
}

fn workspace_short_id(project_slug: &str, workspace_id: u64) -> String {
    let mut prefix = project_slug
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .take(2)
        .collect::<String>();
    while prefix.len() < 2 {
        prefix.push('x');
    }

    let mut suffix = to_base36(workspace_id);
    if suffix.len() < 2 {
        suffix.insert(0, '0');
    }

    format!("{prefix}{suffix}")
}

fn to_base36(mut n: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_owned();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_else(|_| "0".to_owned())
}

fn map_conversation_entry(entry: &ConversationEntry) -> luban_api::ConversationEntry {
    match entry {
        ConversationEntry::UserMessage { text, attachments } => {
            luban_api::ConversationEntry::UserMessage(luban_api::UserMessage {
                text: text.clone(),
                attachments: attachments.iter().map(map_attachment_ref).collect(),
            })
        }
        ConversationEntry::CodexItem { item } => {
            let id = codex_item_id(item.as_ref()).to_owned();
            let (kind, payload) = map_agent_item(item.as_ref());
            luban_api::ConversationEntry::AgentItem(luban_api::AgentItem { id, kind, payload })
        }
        ConversationEntry::TurnUsage { usage } => {
            let usage_json = usage.as_ref().and_then(|u| serde_json::to_value(u).ok());
            luban_api::ConversationEntry::TurnUsage { usage_json }
        }
        ConversationEntry::TurnDuration { duration_ms } => {
            luban_api::ConversationEntry::TurnDuration {
                duration_ms: *duration_ms,
            }
        }
        ConversationEntry::TurnCanceled => luban_api::ConversationEntry::TurnCanceled,
        ConversationEntry::TurnError { message } => luban_api::ConversationEntry::TurnError {
            message: message.clone(),
        },
    }
}

fn map_attachment_ref(att: &AttachmentRef) -> luban_api::AttachmentRef {
    luban_api::AttachmentRef {
        id: att.id.clone(),
        kind: match att.kind {
            AttachmentKind::Image => luban_api::AttachmentKind::Image,
            AttachmentKind::Text => luban_api::AttachmentKind::Text,
            AttachmentKind::File => luban_api::AttachmentKind::File,
        },
        name: att.name.clone(),
        extension: att.extension.clone(),
        mime: att.mime.clone(),
        byte_len: att.byte_len,
    }
}

fn map_agent_item(item: &CodexThreadItem) -> (luban_api::AgentItemKind, serde_json::Value) {
    let kind = match item {
        CodexThreadItem::AgentMessage { .. } => luban_api::AgentItemKind::AgentMessage,
        CodexThreadItem::Reasoning { .. } => luban_api::AgentItemKind::Reasoning,
        CodexThreadItem::CommandExecution { .. } => luban_api::AgentItemKind::CommandExecution,
        CodexThreadItem::FileChange { .. } => luban_api::AgentItemKind::FileChange,
        CodexThreadItem::McpToolCall { .. } => luban_api::AgentItemKind::McpToolCall,
        CodexThreadItem::WebSearch { .. } => luban_api::AgentItemKind::WebSearch,
        CodexThreadItem::TodoList { .. } => luban_api::AgentItemKind::TodoList,
        CodexThreadItem::Error { .. } => luban_api::AgentItemKind::Error,
    };
    let payload = serde_json::to_value(item).unwrap_or_else(|_| serde_json::Value::Null);
    (kind, payload)
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

fn map_client_action(action: luban_api::ClientAction) -> Option<Action> {
    match action {
        luban_api::ClientAction::PickProjectPath => None,
        luban_api::ClientAction::AddProject { path } => Some(Action::AddProject {
            path: expand_user_path(&path),
        }),
        luban_api::ClientAction::DeleteProject { project_id } => Some(Action::DeleteProject {
            project_id: luban_domain::ProjectId::from_u64(project_id.0),
        }),
        luban_api::ClientAction::ToggleProjectExpanded { project_id } => {
            Some(Action::ToggleProjectExpanded {
                project_id: luban_domain::ProjectId::from_u64(project_id.0),
            })
        }
        luban_api::ClientAction::CreateWorkspace { project_id } => Some(Action::CreateWorkspace {
            project_id: luban_domain::ProjectId::from_u64(project_id.0),
        }),
        luban_api::ClientAction::OpenWorkspace { workspace_id } => Some(Action::OpenWorkspace {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
        }),
        luban_api::ClientAction::OpenWorkspacePullRequest { workspace_id } => {
            Some(Action::OpenWorkspacePullRequest {
                workspace_id: WorkspaceId::from_u64(workspace_id.0),
            })
        }
        luban_api::ClientAction::OpenWorkspacePullRequestFailedAction { workspace_id } => {
            Some(Action::OpenWorkspacePullRequestFailedAction {
                workspace_id: WorkspaceId::from_u64(workspace_id.0),
            })
        }
        luban_api::ClientAction::ArchiveWorkspace { workspace_id } => {
            Some(Action::ArchiveWorkspace {
                workspace_id: WorkspaceId::from_u64(workspace_id.0),
            })
        }
        luban_api::ClientAction::SendAgentMessage {
            workspace_id,
            thread_id,
            text,
            attachments,
        } => Some(Action::SendAgentMessage {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            text,
            attachments: attachments.into_iter().map(map_api_attachment).collect(),
        }),
        luban_api::ClientAction::CancelAgentTurn {
            workspace_id,
            thread_id,
        } => Some(Action::CancelAgentTurn {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
        }),
        luban_api::ClientAction::CreateWorkspaceThread { workspace_id } => {
            Some(Action::CreateWorkspaceThread {
                workspace_id: WorkspaceId::from_u64(workspace_id.0),
            })
        }
        luban_api::ClientAction::ActivateWorkspaceThread {
            workspace_id,
            thread_id,
        } => Some(Action::ActivateWorkspaceThread {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
        }),
        luban_api::ClientAction::CloseWorkspaceThreadTab {
            workspace_id,
            thread_id,
        } => Some(Action::CloseWorkspaceThreadTab {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
        }),
        luban_api::ClientAction::RestoreWorkspaceThreadTab {
            workspace_id,
            thread_id,
        } => Some(Action::RestoreWorkspaceThreadTab {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
        }),
        luban_api::ClientAction::ReorderWorkspaceThreadTab {
            workspace_id,
            thread_id,
            to_index,
        } => Some(Action::ReorderWorkspaceThreadTab {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            to_index,
        }),
    }
}

fn expand_user_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
        return PathBuf::from(trimmed);
    }

    if let Some(suffix) = trimmed.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(suffix);
    }

    PathBuf::from(trimmed)
}

fn map_api_attachment(att: luban_api::AttachmentRef) -> AttachmentRef {
    AttachmentRef {
        id: att.id,
        kind: match att.kind {
            luban_api::AttachmentKind::Image => AttachmentKind::Image,
            luban_api::AttachmentKind::Text => AttachmentKind::Text,
            luban_api::AttachmentKind::File => AttachmentKind::File,
        },
        name: att.name,
        extension: att.extension,
        mime: att.mime,
        byte_len: att.byte_len,
    }
}

pub fn new_default_services() -> anyhow::Result<Arc<dyn ProjectWorkspaceService>> {
    Ok(GitWorkspaceService::new_with_options(SqliteStoreOptions {
        persist_ui_state: false,
    })
    .context("failed to init backend services")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use luban_domain::{
        CodexThreadEvent, ContextImage, ConversationSnapshot as DomainConversationSnapshot,
        ConversationThreadMeta, PersistedAppState,
    };
    use std::sync::atomic::AtomicBool;

    struct TestServices;

    impl ProjectWorkspaceService for TestServices {
        fn load_app_state(&self) -> Result<PersistedAppState, String> {
            Err("unimplemented".to_owned())
        }

        fn save_app_state(&self, _snapshot: PersistedAppState) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn create_workspace(
            &self,
            _project_path: PathBuf,
            _project_slug: String,
        ) -> Result<luban_domain::CreatedWorkspace, String> {
            Err("unimplemented".to_owned())
        }

        fn open_workspace_in_ide(&self, _worktree_path: PathBuf) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn archive_workspace(
            &self,
            _project_path: PathBuf,
            _worktree_path: PathBuf,
        ) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn ensure_conversation(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _thread_id: u64,
        ) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn list_conversation_threads(
            &self,
            _project_slug: String,
            _workspace_name: String,
        ) -> Result<Vec<ConversationThreadMeta>, String> {
            Err("unimplemented".to_owned())
        }

        fn load_conversation(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _thread_id: u64,
        ) -> Result<DomainConversationSnapshot, String> {
            Err("unimplemented".to_owned())
        }

        fn store_context_image(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _image: ContextImage,
        ) -> Result<AttachmentRef, String> {
            Err("unimplemented".to_owned())
        }

        fn store_context_text(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _text: String,
            _extension: String,
        ) -> Result<AttachmentRef, String> {
            Err("unimplemented".to_owned())
        }

        fn store_context_file(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _source_path: PathBuf,
        ) -> Result<AttachmentRef, String> {
            Err("unimplemented".to_owned())
        }

        fn run_agent_turn_streamed(
            &self,
            _request: luban_domain::RunAgentTurnRequest,
            _cancel: Arc<AtomicBool>,
            _on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync>,
        ) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn gh_is_authorized(&self) -> Result<bool, String> {
            Err("unimplemented".to_owned())
        }

        fn gh_pull_request_info(
            &self,
            _worktree_path: PathBuf,
        ) -> Result<Option<PullRequestInfo>, String> {
            Err("unimplemented".to_owned())
        }

        fn gh_open_pull_request(&self, _worktree_path: PathBuf) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn gh_open_pull_request_failed_action(
            &self,
            _worktree_path: PathBuf,
        ) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }
    }

    #[test]
    fn app_snapshot_includes_pull_request_info() {
        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-test"),
        });

        let workspace_id = state.projects[0].workspaces[0].id;

        let (events, _) = broadcast::channel::<WsServerMessage>(1);
        let (tx, _rx) = mpsc::channel::<EngineCommand>(1);
        let mut engine = Engine {
            state,
            rev: 1,
            services: Arc::new(TestServices),
            events,
            tx,
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        engine.pull_requests.insert(
            workspace_id,
            PullRequestCacheEntry {
                info: Some(PullRequestInfo {
                    number: 42,
                    is_draft: false,
                    state: DomainPullRequestState::Open,
                    ci_state: Some(DomainPullRequestCiState::Pending),
                    merge_ready: false,
                }),
                refreshed_at: Instant::now(),
            },
        );

        let snapshot = engine.app_snapshot();
        let pr = snapshot.projects[0].workspaces[0].pull_request;
        assert_eq!(
            pr,
            Some(PullRequestSnapshot {
                number: 42,
                is_draft: false,
                state: PullRequestState::Open,
                ci_state: Some(PullRequestCiState::Pending),
                merge_ready: false,
            })
        );
    }
}
