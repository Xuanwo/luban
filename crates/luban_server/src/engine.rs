use crate::branch_watch::BranchWatchHandle;
use anyhow::Context as _;
use luban_api::{
    AppSnapshot, ConversationSnapshot, PullRequestCiState, PullRequestSnapshot, PullRequestState,
    ThreadsSnapshot, WorkspaceTabsSnapshot, WsServerMessage,
};
use luban_backend::{GitWorkspaceService, SqliteStoreOptions};
use luban_domain::{
    Action, AppState, AttachmentKind, AttachmentRef, CodexThreadEvent, CodexThreadItem,
    ConversationEntry, Effect, OpenTarget, OperationStatus, ProjectWorkspaceService,
    PullRequestCiState as DomainPullRequestCiState, PullRequestInfo,
    PullRequestState as DomainPullRequestState, ThinkingEffort, WorkspaceId, WorkspaceThreadId,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
#[cfg(target_os = "macos")]
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
        before: Option<u64>,
        limit: Option<u64>,
    ) -> anyhow::Result<ConversationSnapshot> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::GetConversationSnapshot {
                workspace_id,
                thread_id,
                before,
                limit,
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

    pub async fn starred_tasks_snapshot(
        &self,
    ) -> anyhow::Result<std::collections::HashSet<(u64, u64)>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::GetStarredTasks { reply: tx })
            .await
            .context("engine unavailable")?;
        rx.await.context("engine stopped")?
    }

    pub async fn apply_client_action(
        &self,
        request_id: String,
        action: luban_api::ClientAction,
    ) -> Result<u64, String> {
        let (tx, rx) = oneshot::channel();
        if self
            .tx
            .send(EngineCommand::ApplyClientAction {
                request_id,
                action,
                reply: tx,
            })
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
        before: Option<u64>,
        limit: Option<u64>,
        reply: oneshot::Sender<anyhow::Result<ConversationSnapshot>>,
    },
    GetWorkspaceWorktreePath {
        workspace_id: luban_api::WorkspaceId,
        reply: oneshot::Sender<anyhow::Result<Option<PathBuf>>>,
    },
    GetStarredTasks {
        reply: oneshot::Sender<anyhow::Result<std::collections::HashSet<(u64, u64)>>>,
    },
    ApplyClientAction {
        request_id: String,
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
    WorkspaceBranchObserved {
        workspace_id: WorkspaceId,
        branch_name: String,
    },
}

#[derive(Clone, Debug)]
struct PullRequestCacheEntry {
    info: Option<PullRequestInfo>,
    next_refresh_at: Instant,
    consecutive_empty: u32,
}

const PULL_REQUEST_REFRESH_TICK_INTERVAL: Duration = Duration::from_secs(30);
const PULL_REQUEST_REFRESH_MAX_PER_TICK: usize = 2;
const PULL_REQUEST_REFRESH_JITTER_WINDOW_SECS: u64 = 10;

const PULL_REQUEST_REFRESH_INTERVAL_CLOSED: Duration = Duration::from_secs(10 * 60);
const PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_PENDING: Duration = Duration::from_secs(30);
const PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_SUCCESS: Duration = Duration::from_secs(3 * 60);
const PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_FAILURE: Duration = Duration::from_secs(60);
const PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_UNKNOWN: Duration = Duration::from_secs(60);

const PULL_REQUEST_REFRESH_INTERVAL_EMPTY_INITIAL: Duration = Duration::from_secs(60);
const PULL_REQUEST_REFRESH_INTERVAL_EMPTY_MEDIUM: Duration = Duration::from_secs(3 * 60);
const PULL_REQUEST_REFRESH_INTERVAL_EMPTY_MAX: Duration = Duration::from_secs(10 * 60);

fn pull_request_refresh_jitter(workspace_id: WorkspaceId) -> Duration {
    let window = PULL_REQUEST_REFRESH_JITTER_WINDOW_SECS.max(1);
    Duration::from_secs(workspace_id.as_u64() % window)
}

fn pull_request_next_refresh_at(
    workspace_id: WorkspaceId,
    now: Instant,
    previous: Option<&PullRequestCacheEntry>,
    info: Option<&PullRequestInfo>,
) -> (Instant, u32) {
    let (interval, consecutive_empty) = match info {
        Some(pr) => {
            let interval = if pr.state != DomainPullRequestState::Open {
                PULL_REQUEST_REFRESH_INTERVAL_CLOSED
            } else {
                match pr.ci_state {
                    Some(DomainPullRequestCiState::Pending) => {
                        PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_PENDING
                    }
                    Some(DomainPullRequestCiState::Failure) => {
                        PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_FAILURE
                    }
                    Some(DomainPullRequestCiState::Success) => {
                        if pr.merge_ready {
                            PULL_REQUEST_REFRESH_INTERVAL_CLOSED
                        } else {
                            PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_SUCCESS
                        }
                    }
                    None => PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_UNKNOWN,
                }
            };
            (interval, 0)
        }
        None => {
            let prev = previous.map(|e| e.consecutive_empty).unwrap_or(0);
            let consecutive_empty = prev.saturating_add(1);
            let interval = match consecutive_empty {
                1 => PULL_REQUEST_REFRESH_INTERVAL_EMPTY_INITIAL,
                2 => PULL_REQUEST_REFRESH_INTERVAL_EMPTY_MEDIUM,
                _ => PULL_REQUEST_REFRESH_INTERVAL_EMPTY_MAX,
            };
            (interval, consecutive_empty)
        }
    };

    let next_refresh_at = now
        .checked_add(interval)
        .unwrap_or(now)
        .checked_add(pull_request_refresh_jitter(workspace_id))
        .unwrap_or(now);

    (next_refresh_at, consecutive_empty)
}

pub struct Engine {
    state: AppState,
    rev: u64,
    services: Arc<dyn ProjectWorkspaceService>,
    events: broadcast::Sender<WsServerMessage>,
    tx: mpsc::Sender<EngineCommand>,
    branch_watch: BranchWatchHandle,
    cancel_flags: HashMap<(WorkspaceId, WorkspaceThreadId), CancelFlagEntry>,
    pull_requests: HashMap<WorkspaceId, PullRequestCacheEntry>,
    pull_requests_in_flight: HashSet<WorkspaceId>,
}

#[derive(Clone)]
struct CancelFlagEntry {
    run_id: u64,
    flag: Arc<AtomicBool>,
}

impl Engine {
    pub fn start(
        services: Arc<dyn ProjectWorkspaceService>,
    ) -> (EngineHandle, broadcast::Sender<WsServerMessage>) {
        let (tx, mut rx) = mpsc::channel::<EngineCommand>(256);
        let (events, _) = broadcast::channel::<WsServerMessage>(256);

        let branch_watch = BranchWatchHandle::start(tx.clone());
        let mut engine = Self {
            state: AppState::new(),
            rev: 0,
            services,
            events: events.clone(),
            tx: tx.clone(),
            branch_watch,
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

    async fn execute_task_draft(
        &mut self,
        draft: luban_api::TaskDraft,
        mode: luban_api::TaskExecuteMode,
        workdir_id: Option<luban_api::WorkspaceId>,
    ) -> Result<luban_api::TaskExecuteResult, String> {
        let Some(workdir_id) = workdir_id else {
            return Err("workdir_id is required".to_owned());
        };

        let workspace_id = WorkspaceId::from_u64(workdir_id.0);
        let Some(workspace) = self.state.workspace(workspace_id) else {
            return Err("workdir not found".to_owned());
        };
        let worktree_path = workspace.worktree_path.to_string_lossy().to_string();

        let Some(project_path) = self
            .state
            .projects
            .iter()
            .find(|p| p.workspaces.iter().any(|w| w.id == workspace_id))
            .map(|p| p.path.to_string_lossy().to_string())
        else {
            return Err("failed to locate project for workdir".to_owned());
        };

        self.process_action_queue(Action::OpenWorkspace { workspace_id })
            .await;
        self.process_action_queue(Action::CreateWorkspaceThread { workspace_id })
            .await;

        let Some(thread_id) = self.state.active_thread_id(workspace_id) else {
            return Err("failed to determine created task id".to_owned());
        };

        let default_model_id = self.state.agent_default_model_id().to_owned();
        let default_effort = self.state.agent_default_thinking_effort();

        self.process_action_queue(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: default_model_id,
        })
        .await;
        self.process_action_queue(Action::ThinkingEffortChanged {
            workspace_id,
            thread_id,
            thinking_effort: default_effort,
        })
        .await;

        if mode == luban_api::TaskExecuteMode::Start {
            self.process_action_queue(Action::SendAgentMessage {
                workspace_id,
                thread_id,
                text: draft.prompt.clone(),
                attachments: Vec::new(),
                runner: None,
                amp_mode: None,
            })
            .await;
        }

        Ok(luban_api::TaskExecuteResult {
            project_id: luban_api::ProjectId(project_path),
            workspace_id: luban_api::WorkspaceId(workspace_id.as_u64()),
            thread_id: luban_api::WorkspaceThreadId(thread_id.as_u64()),
            worktree_path,
            prompt: draft.prompt,
            mode,
        })
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

                let tabs = self
                    .state
                    .workspace_tabs(wid)
                    .map(map_workspace_tabs_snapshot)
                    .unwrap_or_default();
                let snapshot = threads.map(|threads| ThreadsSnapshot {
                    rev: self.rev,
                    workspace_id,
                    tabs,
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
                before,
                limit,
                reply,
            } => {
                let snapshot = self
                    .get_conversation_snapshot(workspace_id, thread_id, before, limit)
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
            EngineCommand::GetStarredTasks { reply } => {
                let starred = self
                    .state
                    .starred_tasks
                    .iter()
                    .map(|(workspace_id, thread_id)| (workspace_id.as_u64(), thread_id.as_u64()))
                    .collect::<std::collections::HashSet<_>>();
                let _ = reply.send(Ok(starred));
            }
            EngineCommand::ApplyClientAction {
                request_id,
                action,
                reply,
            } => {
                if matches!(action, luban_api::ClientAction::PickProjectPath) {
                    let events = self.events.clone();
                    let rev = self.rev;
                    tokio::task::spawn_blocking(move || {
                        let picked = pick_project_folder();
                        let _ = events.send(WsServerMessage::Event {
                            rev,
                            event: Box::new(luban_api::ServerEvent::ProjectPathPicked {
                                request_id,
                                path: picked.as_ref().map(|p| p.to_string_lossy().to_string()),
                            }),
                        });
                    });
                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::AddProject { path } = &action {
                    enum AddProjectDecision {
                        ReuseExisting,
                        Add { root_path: PathBuf, is_git: bool },
                    }

                    let services = self.services.clone();
                    let requested_path = expand_user_path(path);
                    let existing_paths = self
                        .state
                        .projects
                        .iter()
                        .map(|p| p.path.clone())
                        .collect::<Vec<_>>();

                    let decision = tokio::task::spawn_blocking(move || {
                        let requested = services.project_identity(requested_path)?;
                        if let Some(github_repo) = requested.github_repo.as_deref() {
                            for existing_path in existing_paths {
                                let existing = match services.project_identity(existing_path) {
                                    Ok(v) => v,
                                    Err(_) => continue,
                                };
                                if existing.github_repo.as_deref() == Some(github_repo) {
                                    return Ok(AddProjectDecision::ReuseExisting);
                                }
                            }
                        }

                        Ok::<AddProjectDecision, String>(AddProjectDecision::Add {
                            root_path: requested.root_path,
                            is_git: requested.is_git,
                        })
                    })
                    .await
                    .ok()
                    .unwrap_or_else(|| Err("failed to join project identity task".to_owned()));

                    match decision {
                        Ok(AddProjectDecision::ReuseExisting) => {
                            let _ = reply.send(Ok(self.rev));
                            return;
                        }
                        Ok(AddProjectDecision::Add { root_path, is_git }) => {
                            self.process_action_queue(Action::AddProject {
                                path: root_path,
                                is_git,
                            })
                            .await;
                            let _ = reply.send(Ok(self.rev));
                            return;
                        }
                        Err(message) => {
                            let _ = reply.send(Err(message));
                            return;
                        }
                    }
                }

                if let luban_api::ClientAction::AddProjectAndOpen { path } = &action {
                    enum AddProjectDecision {
                        ReuseExisting { root_path: PathBuf, is_git: bool },
                        Add { root_path: PathBuf, is_git: bool },
                    }

                    let services = self.services.clone();
                    let requested_path = expand_user_path(path);
                    let existing_paths = self
                        .state
                        .projects
                        .iter()
                        .map(|p| p.path.clone())
                        .collect::<Vec<_>>();

                    let decision = tokio::task::spawn_blocking(move || {
                        let requested = services.project_identity(requested_path)?;
                        if let Some(github_repo) = requested.github_repo.as_deref() {
                            for existing_path in existing_paths {
                                let existing =
                                    match services.project_identity(existing_path.clone()) {
                                        Ok(v) => v,
                                        Err(_) => continue,
                                    };
                                if existing.github_repo.as_deref() == Some(github_repo) {
                                    return Ok(AddProjectDecision::ReuseExisting {
                                        root_path: existing.root_path,
                                        is_git: existing.is_git,
                                    });
                                }
                            }
                        }

                        Ok::<AddProjectDecision, String>(AddProjectDecision::Add {
                            root_path: requested.root_path,
                            is_git: requested.is_git,
                        })
                    })
                    .await
                    .ok()
                    .unwrap_or_else(|| Err("failed to join project identity task".to_owned()));

                    let (root_path, is_git) = match decision {
                        Ok(AddProjectDecision::ReuseExisting { root_path, is_git }) => {
                            (root_path, is_git)
                        }
                        Ok(AddProjectDecision::Add { root_path, is_git }) => (root_path, is_git),
                        Err(message) => {
                            let _ = reply.send(Err(message));
                            return;
                        }
                    };

                    self.process_action_queue(Action::AddProject {
                        path: root_path.clone(),
                        is_git,
                    })
                    .await;

                    let Some(project_id) = find_project_id_by_path(&self.state, &root_path) else {
                        let _ =
                            reply.send(Err("failed to locate project after adding it".to_owned()));
                        return;
                    };

                    self.process_action_queue(Action::EnsureMainWorkspace { project_id })
                        .await;

                    let main_workspace_id = self
                        .state
                        .projects
                        .iter()
                        .find(|p| p.id == project_id)
                        .and_then(|p| {
                            let active = p
                                .workspaces
                                .iter()
                                .filter(|w| w.status == luban_domain::WorkspaceStatus::Active);
                            active
                                .clone()
                                .find(|w| w.workspace_name == "main" && w.worktree_path == p.path)
                                .map(|w| w.id)
                                .or_else(|| active.clone().next().map(|w| w.id))
                        });

                    let Some(workspace_id) = main_workspace_id else {
                        let _ = reply.send(Err(
                            "failed to locate main workspace after ensuring it".to_owned(),
                        ));
                        return;
                    };

                    let _ = self.events.send(WsServerMessage::Event {
                        rev: self.rev,
                        event: Box::new(luban_api::ServerEvent::AddProjectAndOpenReady {
                            request_id: request_id.clone(),
                            project_id: luban_api::ProjectId(
                                self.state
                                    .projects
                                    .iter()
                                    .find(|p| p.id == project_id)
                                    .map(|p| p.path.to_string_lossy().to_string())
                                    .unwrap_or_else(|| root_path.to_string_lossy().to_string()),
                            ),
                            workspace_id: luban_api::WorkspaceId(workspace_id.as_u64()),
                        }),
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::TaskPreview { input } = &action {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let input = input.clone();
                    tokio::spawn(async move {
                        let result =
                            tokio::task::spawn_blocking(move || services.task_preview(input))
                                .await
                                .ok()
                                .unwrap_or_else(|| {
                                    Err("failed to join task preview task".to_owned())
                                });

                        match result {
                            Ok(draft) => {
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(luban_api::ServerEvent::TaskPreviewReady {
                                        request_id,
                                        draft: Box::new(map_task_draft(&draft)),
                                    }),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::TaskExecute {
                    draft,
                    mode,
                    workdir_id,
                } = &action
                {
                    let draft = draft.as_ref().clone();
                    let mode = *mode;
                    let workdir_id = *workdir_id;

                    match self.execute_task_draft(draft, mode, workdir_id).await {
                        Ok(result) => {
                            let _ = self.events.send(WsServerMessage::Event {
                                rev: self.rev,
                                event: Box::new(luban_api::ServerEvent::TaskExecuted {
                                    request_id: request_id.clone(),
                                    result,
                                }),
                            });
                            let _ = reply.send(Ok(self.rev));
                        }
                        Err(message) => {
                            let _ = reply.send(Err(message));
                        }
                    }
                    return;
                }

                if let luban_api::ClientAction::FeedbackSubmit {
                    title,
                    body,
                    labels,
                    feedback_type,
                    action: submit_action,
                } = &action
                {
                    let services = self.services.clone();
                    let title = title.clone();
                    let body = body.clone();
                    let labels = labels.clone();
                    let issue = match tokio::task::spawn_blocking(move || {
                        services.feedback_create_issue(title, body, labels)
                    })
                    .await
                    {
                        Ok(Ok(issue)) => issue,
                        Ok(Err(message)) => {
                            let _ = reply.send(Err(message));
                            return;
                        }
                        Err(_) => {
                            let _ = reply
                                .send(Err("failed to join feedback create issue task".to_owned()));
                            return;
                        }
                    };

                    let issue_snapshot = luban_api::TaskIssueInfo {
                        number: issue.number,
                        title: issue.title.clone(),
                        url: issue.url.clone(),
                    };

                    let task_result = match submit_action {
                        luban_api::FeedbackSubmitAction::CreateIssue => None,
                        luban_api::FeedbackSubmitAction::FixIt => {
                            let intent_kind = match feedback_type {
                                luban_api::FeedbackType::Bug => luban_domain::TaskIntentKind::Fix,
                                luban_api::FeedbackType::Feature => {
                                    luban_domain::TaskIntentKind::Implement
                                }
                                luban_api::FeedbackType::Question => {
                                    luban_domain::TaskIntentKind::Discuss
                                }
                            };
                            let services = self.services.clone();
                            let issue = issue.clone();
                            let draft = match tokio::task::spawn_blocking(move || {
                                services.feedback_task_draft(issue, intent_kind)
                            })
                            .await
                            {
                                Ok(Ok(draft)) => draft,
                                Ok(Err(message)) => {
                                    let _ = reply.send(Err(message));
                                    return;
                                }
                                Err(_) => {
                                    let _ =
                                        reply
                                            .send(Err("failed to join feedback task draft task"
                                                .to_owned()));
                                    return;
                                }
                            };

                            let api_draft = map_task_draft(&draft);
                            let workdir_id = self
                                .state
                                .last_open_workspace_id
                                .or_else(|| {
                                    self.state
                                        .projects
                                        .iter()
                                        .flat_map(|p| p.workspaces.iter())
                                        .find(|w| w.status == luban_domain::WorkspaceStatus::Active)
                                        .map(|w| w.id)
                                })
                                .map(|id| luban_api::WorkspaceId(id.as_u64()));
                            let result = match self
                                .execute_task_draft(
                                    api_draft,
                                    luban_api::TaskExecuteMode::Create,
                                    workdir_id,
                                )
                                .await
                            {
                                Ok(result) => result,
                                Err(message) => {
                                    let _ = reply.send(Err(message));
                                    return;
                                }
                            };
                            Some(result)
                        }
                    };

                    let _ = self.events.send(WsServerMessage::Event {
                        rev: self.rev,
                        event: Box::new(luban_api::ServerEvent::FeedbackSubmitted {
                            request_id: request_id.clone(),
                            result: luban_api::FeedbackSubmitResult {
                                issue: issue_snapshot,
                                task: task_result,
                            },
                        }),
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if matches!(action, luban_api::ClientAction::CodexCheck) {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    tokio::spawn(async move {
                        let result = tokio::task::spawn_blocking(move || services.codex_check())
                            .await
                            .ok()
                            .unwrap_or_else(|| Err("failed to join codex check task".to_owned()));

                        let (ok, message) = match result {
                            Ok(()) => (true, None),
                            Err(message) => (false, Some(message)),
                        };

                        let _ = events.send(WsServerMessage::Event {
                            rev,
                            event: Box::new(luban_api::ServerEvent::CodexCheckReady {
                                request_id,
                                ok,
                                message,
                            }),
                        });
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if matches!(action, luban_api::ClientAction::AmpCheck) {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    tokio::spawn(async move {
                        let result = tokio::task::spawn_blocking(move || services.amp_check())
                            .await
                            .ok()
                            .unwrap_or_else(|| Err("failed to join amp check task".to_owned()));

                        let (ok, message) = match result {
                            Ok(()) => (true, None),
                            Err(message) => (false, Some(message)),
                        };

                        let _ = events.send(WsServerMessage::Event {
                            rev,
                            event: Box::new(luban_api::ServerEvent::AmpCheckReady {
                                request_id,
                                ok,
                                message,
                            }),
                        });
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if matches!(action, luban_api::ClientAction::ClaudeCheck) {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    tokio::spawn(async move {
                        let result = tokio::task::spawn_blocking(move || services.claude_check())
                            .await
                            .ok()
                            .unwrap_or_else(|| Err("failed to join claude check task".to_owned()));

                        let (ok, message) = match result {
                            Ok(()) => (true, None),
                            Err(message) => (false, Some(message)),
                        };

                        let _ = events.send(WsServerMessage::Event {
                            rev,
                            event: Box::new(luban_api::ServerEvent::ClaudeCheckReady {
                                request_id,
                                ok,
                                message,
                            }),
                        });
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if matches!(action, luban_api::ClientAction::CodexConfigTree) {
                    fn map_entry(
                        entry: luban_domain::CodexConfigEntry,
                    ) -> luban_api::CodexConfigEntrySnapshot {
                        luban_api::CodexConfigEntrySnapshot {
                            path: entry.path,
                            name: entry.name,
                            kind: match entry.kind {
                                luban_domain::CodexConfigEntryKind::File => {
                                    luban_api::CodexConfigEntryKind::File
                                }
                                luban_domain::CodexConfigEntryKind::Folder => {
                                    luban_api::CodexConfigEntryKind::Folder
                                }
                            },
                            children: entry.children.into_iter().map(map_entry).collect(),
                        }
                    }

                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    tokio::spawn(async move {
                        let result =
                            tokio::task::spawn_blocking(move || services.codex_config_tree())
                                .await
                                .ok()
                                .unwrap_or_else(|| {
                                    Err("failed to join codex config tree task".to_owned())
                                });

                        match result {
                            Ok(tree) => {
                                let tree = tree.into_iter().map(map_entry).collect();
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(luban_api::ServerEvent::CodexConfigTreeReady {
                                        request_id,
                                        tree,
                                    }),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if matches!(action, luban_api::ClientAction::AmpConfigTree) {
                    fn map_entry(
                        entry: luban_domain::AmpConfigEntry,
                    ) -> luban_api::AmpConfigEntrySnapshot {
                        luban_api::AmpConfigEntrySnapshot {
                            path: entry.path,
                            name: entry.name,
                            kind: match entry.kind {
                                luban_domain::AmpConfigEntryKind::File => {
                                    luban_api::AmpConfigEntryKind::File
                                }
                                luban_domain::AmpConfigEntryKind::Folder => {
                                    luban_api::AmpConfigEntryKind::Folder
                                }
                            },
                            children: entry.children.into_iter().map(map_entry).collect(),
                        }
                    }

                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    tokio::spawn(async move {
                        let result =
                            tokio::task::spawn_blocking(move || services.amp_config_tree())
                                .await
                                .ok()
                                .unwrap_or_else(|| {
                                    Err("failed to join amp config tree task".to_owned())
                                });

                        match result {
                            Ok(tree) => {
                                let tree = tree.into_iter().map(map_entry).collect();
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(luban_api::ServerEvent::AmpConfigTreeReady {
                                        request_id,
                                        tree,
                                    }),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::CodexConfigListDir { path } = &action {
                    fn map_entry(
                        entry: luban_domain::CodexConfigEntry,
                    ) -> luban_api::CodexConfigEntrySnapshot {
                        luban_api::CodexConfigEntrySnapshot {
                            path: entry.path,
                            name: entry.name,
                            kind: match entry.kind {
                                luban_domain::CodexConfigEntryKind::File => {
                                    luban_api::CodexConfigEntryKind::File
                                }
                                luban_domain::CodexConfigEntryKind::Folder => {
                                    luban_api::CodexConfigEntryKind::Folder
                                }
                            },
                            children: entry.children.into_iter().map(map_entry).collect(),
                        }
                    }

                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let path = path.clone();
                    tokio::spawn(async move {
                        let path_for_task = path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            services.codex_config_list_dir(path_for_task)
                        })
                        .await
                        .ok()
                        .unwrap_or_else(|| {
                            Err("failed to join codex config list dir task".to_owned())
                        });

                        match result {
                            Ok(entries) => {
                                let entries = entries.into_iter().map(map_entry).collect();
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(
                                        luban_api::ServerEvent::CodexConfigListDirReady {
                                            request_id,
                                            path,
                                            entries,
                                        },
                                    ),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::AmpConfigListDir { path } = &action {
                    fn map_entry(
                        entry: luban_domain::AmpConfigEntry,
                    ) -> luban_api::AmpConfigEntrySnapshot {
                        luban_api::AmpConfigEntrySnapshot {
                            path: entry.path,
                            name: entry.name,
                            kind: match entry.kind {
                                luban_domain::AmpConfigEntryKind::File => {
                                    luban_api::AmpConfigEntryKind::File
                                }
                                luban_domain::AmpConfigEntryKind::Folder => {
                                    luban_api::AmpConfigEntryKind::Folder
                                }
                            },
                            children: entry.children.into_iter().map(map_entry).collect(),
                        }
                    }

                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let path = path.clone();
                    tokio::spawn(async move {
                        let path_for_task = path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            services.amp_config_list_dir(path_for_task)
                        })
                        .await
                        .ok()
                        .unwrap_or_else(|| Err("failed to join amp config list task".to_owned()));

                        match result {
                            Ok(entries) => {
                                let entries = entries.into_iter().map(map_entry).collect();
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(
                                        luban_api::ServerEvent::AmpConfigListDirReady {
                                            request_id,
                                            path,
                                            entries,
                                        },
                                    ),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::CodexConfigReadFile { path } = &action {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let path = path.clone();
                    tokio::spawn(async move {
                        let path_for_task = path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            services.codex_config_read_file(path_for_task)
                        })
                        .await
                        .ok()
                        .unwrap_or_else(|| Err("failed to join codex config read task".to_owned()));

                        match result {
                            Ok(contents) => {
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(luban_api::ServerEvent::CodexConfigFileReady {
                                        request_id,
                                        path,
                                        contents,
                                    }),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::AmpConfigReadFile { path } = &action {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let path = path.clone();
                    tokio::spawn(async move {
                        let path_for_task = path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            services.amp_config_read_file(path_for_task)
                        })
                        .await
                        .ok()
                        .unwrap_or_else(|| Err("failed to join amp config read task".to_owned()));

                        match result {
                            Ok(contents) => {
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(luban_api::ServerEvent::AmpConfigFileReady {
                                        request_id,
                                        path,
                                        contents,
                                    }),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::CodexConfigWriteFile { path, contents } = &action {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let path = path.clone();
                    let contents = contents.clone();
                    tokio::spawn(async move {
                        let path_for_task = path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            services.codex_config_write_file(path_for_task, contents)
                        })
                        .await
                        .ok()
                        .unwrap_or_else(|| {
                            Err("failed to join codex config write task".to_owned())
                        });

                        match result {
                            Ok(()) => {
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(luban_api::ServerEvent::CodexConfigFileSaved {
                                        request_id,
                                        path,
                                    }),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::AmpConfigWriteFile { path, contents } = &action {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let path = path.clone();
                    let contents = contents.clone();
                    tokio::spawn(async move {
                        let path_for_task = path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            services.amp_config_write_file(path_for_task, contents)
                        })
                        .await
                        .ok()
                        .unwrap_or_else(|| Err("failed to join amp config write task".to_owned()));

                        match result {
                            Ok(()) => {
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(luban_api::ServerEvent::AmpConfigFileSaved {
                                        request_id,
                                        path,
                                    }),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if matches!(action, luban_api::ClientAction::ClaudeConfigTree) {
                    fn map_entry(
                        entry: luban_domain::ClaudeConfigEntry,
                    ) -> luban_api::ClaudeConfigEntrySnapshot {
                        luban_api::ClaudeConfigEntrySnapshot {
                            path: entry.path,
                            name: entry.name,
                            kind: match entry.kind {
                                luban_domain::ClaudeConfigEntryKind::File => {
                                    luban_api::ClaudeConfigEntryKind::File
                                }
                                luban_domain::ClaudeConfigEntryKind::Folder => {
                                    luban_api::ClaudeConfigEntryKind::Folder
                                }
                            },
                            children: entry.children.into_iter().map(map_entry).collect(),
                        }
                    }

                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    tokio::spawn(async move {
                        let result =
                            tokio::task::spawn_blocking(move || services.claude_config_tree())
                                .await
                                .ok()
                                .unwrap_or_else(|| {
                                    Err("failed to join claude config tree task".to_owned())
                                });

                        match result {
                            Ok(tree) => {
                                let tree = tree.into_iter().map(map_entry).collect();
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(
                                        luban_api::ServerEvent::ClaudeConfigTreeReady {
                                            request_id,
                                            tree,
                                        },
                                    ),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::ClaudeConfigListDir { path } = &action {
                    fn map_entry(
                        entry: luban_domain::ClaudeConfigEntry,
                    ) -> luban_api::ClaudeConfigEntrySnapshot {
                        luban_api::ClaudeConfigEntrySnapshot {
                            path: entry.path,
                            name: entry.name,
                            kind: match entry.kind {
                                luban_domain::ClaudeConfigEntryKind::File => {
                                    luban_api::ClaudeConfigEntryKind::File
                                }
                                luban_domain::ClaudeConfigEntryKind::Folder => {
                                    luban_api::ClaudeConfigEntryKind::Folder
                                }
                            },
                            children: entry.children.into_iter().map(map_entry).collect(),
                        }
                    }

                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let path = path.clone();
                    tokio::spawn(async move {
                        let path_for_task = path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            services.claude_config_list_dir(path_for_task)
                        })
                        .await
                        .ok()
                        .unwrap_or_else(|| {
                            Err("failed to join claude config list dir task".to_owned())
                        });

                        match result {
                            Ok(entries) => {
                                let entries = entries.into_iter().map(map_entry).collect();
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(
                                        luban_api::ServerEvent::ClaudeConfigListDirReady {
                                            request_id,
                                            path,
                                            entries,
                                        },
                                    ),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::ClaudeConfigReadFile { path } = &action {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let path = path.clone();
                    tokio::spawn(async move {
                        let path_for_task = path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            services.claude_config_read_file(path_for_task)
                        })
                        .await
                        .ok()
                        .unwrap_or_else(|| {
                            Err("failed to join claude config read task".to_owned())
                        });

                        match result {
                            Ok(contents) => {
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(
                                        luban_api::ServerEvent::ClaudeConfigFileReady {
                                            request_id,
                                            path,
                                            contents,
                                        },
                                    ),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
                                });
                            }
                        }
                    });

                    let _ = reply.send(Ok(self.rev));
                    return;
                }

                if let luban_api::ClientAction::ClaudeConfigWriteFile { path, contents } = &action {
                    let services = self.services.clone();
                    let events = self.events.clone();
                    let request_id = request_id.clone();
                    let rev = self.rev;
                    let path = path.clone();
                    let contents = contents.clone();
                    tokio::spawn(async move {
                        let path_for_task = path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            services.claude_config_write_file(path_for_task, contents)
                        })
                        .await
                        .ok()
                        .unwrap_or_else(|| {
                            Err("failed to join claude config write task".to_owned())
                        });

                        match result {
                            Ok(()) => {
                                let _ = events.send(WsServerMessage::Event {
                                    rev,
                                    event: Box::new(
                                        luban_api::ServerEvent::ClaudeConfigFileSaved {
                                            request_id,
                                            path,
                                        },
                                    ),
                                });
                            }
                            Err(message) => {
                                let _ = events.send(WsServerMessage::Error {
                                    request_id: Some(request_id),
                                    message,
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

                match &action {
                    luban_api::ClientAction::DeleteProject { project_id } => {
                        let path = expand_user_path(&project_id.0);
                        let Some(id) = find_project_id_by_path(&self.state, &path) else {
                            let _ = reply.send(Err("project not found".to_owned()));
                            return;
                        };
                        self.process_action_queue(Action::DeleteProject { project_id: id })
                            .await;
                        let _ = reply.send(Ok(self.rev));
                        return;
                    }
                    luban_api::ClientAction::ToggleProjectExpanded { project_id } => {
                        let path = expand_user_path(&project_id.0);
                        let Some(id) = find_project_id_by_path(&self.state, &path) else {
                            let _ = reply.send(Err("project not found".to_owned()));
                            return;
                        };
                        self.process_action_queue(Action::ToggleProjectExpanded { project_id: id })
                            .await;
                        let _ = reply.send(Ok(self.rev));
                        return;
                    }
                    luban_api::ClientAction::CreateWorkspace { project_id } => {
                        let path = expand_user_path(&project_id.0);
                        let Some(id) = find_project_id_by_path(&self.state, &path) else {
                            let _ = reply.send(Err("project not found".to_owned()));
                            return;
                        };
                        self.process_action_queue(Action::CreateWorkspace {
                            project_id: id,
                            branch_name_hint: None,
                        })
                        .await;
                        let _ = reply.send(Ok(self.rev));
                        return;
                    }
                    luban_api::ClientAction::EnsureMainWorkspace { project_id } => {
                        let path = expand_user_path(&project_id.0);
                        let Some(id) = find_project_id_by_path(&self.state, &path) else {
                            let _ = reply.send(Err("project not found".to_owned()));
                            return;
                        };
                        self.process_action_queue(Action::EnsureMainWorkspace { project_id: id })
                            .await;
                        let _ = reply.send(Ok(self.rev));
                        return;
                    }
                    luban_api::ClientAction::CancelAndSendAgentMessage {
                        workspace_id,
                        thread_id,
                        text,
                        attachments,
                        runner,
                        amp_mode,
                    } => {
                        let wid = WorkspaceId::from_u64(workspace_id.0);
                        let tid = WorkspaceThreadId::from_u64(thread_id.0);
                        self.process_action_queue(Action::CancelAgentTurn {
                            workspace_id: wid,
                            thread_id: tid,
                        })
                        .await;
                        let runner = runner.map(map_api_agent_runner_kind);
                        let amp_mode = if runner == Some(luban_domain::AgentRunnerKind::Amp) {
                            amp_mode.clone()
                        } else {
                            None
                        };
                        self.process_action_queue(Action::SendAgentMessage {
                            workspace_id: wid,
                            thread_id: tid,
                            text: text.clone(),
                            attachments: attachments
                                .iter()
                                .cloned()
                                .map(map_api_attachment)
                                .collect(),
                            runner,
                            amp_mode,
                        })
                        .await;
                        let _ = reply.send(Ok(self.rev));
                        return;
                    }
                    _ => {}
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

                let now = Instant::now();
                let previous = self.pull_requests.get(&workspace_id);
                let (next_refresh_at, consecutive_empty) =
                    pull_request_next_refresh_at(workspace_id, now, previous, info.as_ref());

                let changed = self
                    .pull_requests
                    .get(&workspace_id)
                    .map(|e| e.info != info)
                    .unwrap_or(true);

                self.pull_requests.insert(
                    workspace_id,
                    PullRequestCacheEntry {
                        info,
                        next_refresh_at,
                        consecutive_empty,
                    },
                );

                if changed {
                    self.rev = self.rev.saturating_add(1);
                    self.publish_app_snapshot();
                }
            }
            EngineCommand::WorkspaceBranchObserved {
                workspace_id,
                branch_name,
            } => {
                self.process_action_queue(Action::WorkspaceBranchSynced {
                    workspace_id,
                    branch_name,
                })
                .await;
            }
        }
    }

    async fn get_conversation_snapshot(
        &self,
        workspace_id: luban_api::WorkspaceId,
        thread_id: luban_api::WorkspaceThreadId,
        before: Option<u64>,
        limit: Option<u64>,
    ) -> anyhow::Result<ConversationSnapshot> {
        if let Ok(snapshot) = self.conversation_snapshot(workspace_id, thread_id, before, limit) {
            return Ok(snapshot);
        }

        const DEFAULT_ENTRIES_LIMIT: usize = 2000;
        const MAX_ENTRIES_LIMIT: usize = 5000;

        let limit = limit
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(DEFAULT_ENTRIES_LIMIT)
            .clamp(1, MAX_ENTRIES_LIMIT);

        let wid = WorkspaceId::from_u64(workspace_id.0);
        let Some(scope) = workspace_scope(&self.state, wid) else {
            return Err(anyhow::anyhow!("workspace not found"));
        };

        let services = self.services.clone();
        let tid = thread_id.0;
        let loaded = tokio::task::spawn_blocking(move || {
            services.load_conversation_page(
                scope.project_slug,
                scope.workspace_name,
                tid,
                before,
                limit as u64,
            )
        })
        .await
        .ok()
        .unwrap_or_else(|| Err("failed to join load conversation task".to_owned()))
        .map_err(|e| anyhow::anyhow!(e))?;

        let entries_total = loaded.entries_total;
        let entries_start = loaded.entries_start;
        let entries_end = entries_start.saturating_add(loaded.entries.len() as u64);
        let entries_truncated = entries_start > 0 || entries_end < entries_total;

        let runner = loaded
            .runner
            .unwrap_or_else(|| self.state.agent_default_runner());
        let model_id = loaded
            .agent_model_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| self.state.agent_default_model_id())
            .to_owned();
        let thinking_effort = loaded
            .thinking_effort
            .unwrap_or_else(|| self.state.agent_default_thinking_effort());
        let amp_mode = if runner == luban_domain::AgentRunnerKind::Amp {
            loaded
                .amp_mode
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| Some(self.state.agent_amp_mode().to_owned()))
        } else {
            None
        };

        Ok(ConversationSnapshot {
            rev: self.rev,
            workspace_id,
            thread_id,
            agent_runner: match runner {
                luban_domain::AgentRunnerKind::Codex => luban_api::AgentRunnerKind::Codex,
                luban_domain::AgentRunnerKind::Amp => luban_api::AgentRunnerKind::Amp,
                luban_domain::AgentRunnerKind::Claude => luban_api::AgentRunnerKind::Claude,
            },
            agent_model_id: model_id.clone(),
            thinking_effort: match thinking_effort {
                ThinkingEffort::Minimal => luban_api::ThinkingEffort::Minimal,
                ThinkingEffort::Low => luban_api::ThinkingEffort::Low,
                ThinkingEffort::Medium => luban_api::ThinkingEffort::Medium,
                ThinkingEffort::High => luban_api::ThinkingEffort::High,
                ThinkingEffort::XHigh => luban_api::ThinkingEffort::XHigh,
            },
            amp_mode,
            run_status: luban_api::OperationStatus::Idle,
            run_started_at_unix_ms: loaded.run_started_at_unix_ms,
            run_finished_at_unix_ms: loaded.run_finished_at_unix_ms,
            entries: loaded.entries.iter().map(map_conversation_entry).collect(),
            entries_total,
            entries_start,
            entries_truncated,
            in_progress_items: Vec::new(),
            pending_prompts: loaded
                .pending_prompts
                .iter()
                .map(|prompt| luban_api::QueuedPromptSnapshot {
                    id: prompt.id,
                    text: prompt.text.clone(),
                    attachments: prompt.attachments.iter().map(map_attachment_ref).collect(),
                    run_config: luban_api::AgentRunConfigSnapshot {
                        runner: match prompt.run_config.runner {
                            luban_domain::AgentRunnerKind::Codex => {
                                luban_api::AgentRunnerKind::Codex
                            }
                            luban_domain::AgentRunnerKind::Amp => luban_api::AgentRunnerKind::Amp,
                            luban_domain::AgentRunnerKind::Claude => {
                                luban_api::AgentRunnerKind::Claude
                            }
                        },
                        model_id: prompt.run_config.model_id.clone(),
                        thinking_effort: match prompt.run_config.thinking_effort {
                            ThinkingEffort::Minimal => luban_api::ThinkingEffort::Minimal,
                            ThinkingEffort::Low => luban_api::ThinkingEffort::Low,
                            ThinkingEffort::Medium => luban_api::ThinkingEffort::Medium,
                            ThinkingEffort::High => luban_api::ThinkingEffort::High,
                            ThinkingEffort::XHigh => luban_api::ThinkingEffort::XHigh,
                        },
                        amp_mode: prompt.run_config.amp_mode.clone(),
                    },
                })
                .collect(),
            queue_paused: loaded.queue_paused,
            remote_thread_id: loaded.thread_id,
            title: format!("Thread {tid}"),
        })
    }

    async fn process_action_queue(&mut self, initial: Action) {
        let mut actions = VecDeque::from([initial]);
        let mut effects = VecDeque::<Effect>::new();

        while let Some(action) = actions.pop_front() {
            self.rev = self.rev.saturating_add(1);

            let should_sync_branch_watchers = should_sync_branch_watchers(&action);
            let conversation_key = conversation_key_for_action(&action);
            let queue_state_key = queue_state_key_for_action(&action);
            let threads_event = threads_event_for_action(&action);

            let new_effects = self.state.apply(action);
            if should_sync_branch_watchers {
                self.sync_branch_watchers();
            }
            self.publish_app_snapshot();

            if let Some((wid, tid)) = conversation_key {
                self.publish_conversation_snapshot(wid, tid);
            }
            if let Some((wid, threads)) = threads_event {
                self.publish_threads_event(wid, &threads);
            }
            if let Some((wid, tid)) = queue_state_key {
                self.persist_queue_state(wid, tid).await;
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

    fn sync_branch_watchers(&self) {
        let workspaces = self
            .state
            .projects
            .iter()
            .filter(|p| p.is_git)
            .flat_map(|p| {
                p.workspaces.iter().filter_map(|w| {
                    if w.status != luban_domain::WorkspaceStatus::Active {
                        return None;
                    }
                    Some((w.id, w.worktree_path.clone()))
                })
            })
            .collect::<Vec<_>>();
        self.branch_watch.sync_workspaces(workspaces);
    }

    async fn persist_queue_state(&self, workspace_id: WorkspaceId, thread_id: WorkspaceThreadId) {
        let Some(scope) = workspace_scope(&self.state, workspace_id) else {
            return;
        };
        let Some(conversation) = self
            .state
            .workspace_thread_conversation(workspace_id, thread_id)
        else {
            return;
        };

        let queue_paused = conversation.queue_paused;
        let run_started_at_unix_ms = conversation.run_started_at_unix_ms;
        let run_finished_at_unix_ms = conversation.run_finished_at_unix_ms;
        let pending_prompts = conversation
            .pending_prompts
            .iter()
            .cloned()
            .collect::<Vec<_>>();

        let services = self.services.clone();
        let project_slug = scope.project_slug;
        let workspace_name = scope.workspace_name;
        let thread_local_id = thread_id.as_u64();
        let result = tokio::task::spawn_blocking(move || {
            services.save_conversation_queue_state(
                project_slug,
                workspace_name,
                thread_local_id,
                queue_paused,
                run_started_at_unix_ms,
                run_finished_at_unix_ms,
                pending_prompts,
            )
        })
        .await;

        match result {
            Ok(Ok(())) => {}
            Ok(Err(message)) => {
                tracing::error!(message = %message, "failed to persist queued prompts");
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to join queued prompt persistence task");
            }
        }
    }

    fn refresh_pull_requests_for_all_workspaces(&mut self) {
        let now = Instant::now();
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

        let mut candidates = workspace_ids
            .into_iter()
            .filter(|workspace_id| self.should_start_pull_request_refresh(*workspace_id, now))
            .collect::<Vec<_>>();

        candidates.sort_by_key(|workspace_id| {
            self.pull_requests
                .get(workspace_id)
                .map(|e| e.next_refresh_at)
                .unwrap_or(now)
        });

        for workspace_id in candidates
            .into_iter()
            .take(PULL_REQUEST_REFRESH_MAX_PER_TICK)
        {
            self.start_pull_request_refresh(workspace_id);
        }
    }

    fn maybe_refresh_pull_request(&mut self, workspace_id: WorkspaceId) {
        let now = Instant::now();
        if !self.should_start_pull_request_refresh(workspace_id, now) {
            return;
        }
        self.start_pull_request_refresh(workspace_id);
    }

    fn should_start_pull_request_refresh(&self, workspace_id: WorkspaceId, now: Instant) -> bool {
        if self.pull_requests_in_flight.contains(&workspace_id) {
            return false;
        }
        if self.state.workspace(workspace_id).is_none() {
            return false;
        }
        if let Some(entry) = self.pull_requests.get(&workspace_id) {
            return now >= entry.next_refresh_at;
        }
        true
    }

    fn start_pull_request_refresh(&mut self, workspace_id: WorkspaceId) {
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
                    Ok(persisted) => Action::AppStateLoaded {
                        persisted: Box::new(persisted),
                    },
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
            Effect::LoadCodexDefaults => {
                let services = self.services.clone();
                let loaded = tokio::task::spawn_blocking(move || {
                    services.codex_config_read_file("config.toml".to_owned())
                })
                .await
                .ok()
                .unwrap_or_else(|| Err("failed to join codex config read task".to_owned()));

                let contents = match loaded {
                    Ok(contents) => contents,
                    Err(message) => {
                        tracing::debug!(message = %message, "codex defaults unavailable");
                        return Ok(VecDeque::new());
                    }
                };

                let (model_id, thinking_effort) = parse_codex_defaults_toml(&contents);
                if model_id.is_none() && thinking_effort.is_none() {
                    return Ok(VecDeque::new());
                }

                Ok(VecDeque::from([Action::CodexDefaultsLoaded {
                    model_id,
                    thinking_effort,
                }]))
            }
            Effect::LoadTaskPromptTemplates => {
                let services = self.services.clone();
                let loaded =
                    tokio::task::spawn_blocking(move || services.task_prompt_templates_load())
                        .await
                        .ok()
                        .unwrap_or_else(|| {
                            Err("failed to join task prompt templates load task".to_owned())
                        });
                match loaded {
                    Ok(templates) => Ok(VecDeque::from([Action::TaskPromptTemplatesLoaded {
                        templates,
                    }])),
                    Err(message) => {
                        tracing::warn!(message = %message, "failed to load task prompt templates");
                        Ok(VecDeque::new())
                    }
                }
            }
            Effect::LoadSystemPromptTemplates => {
                let services = self.services.clone();
                let loaded =
                    tokio::task::spawn_blocking(move || services.system_prompt_templates_load())
                        .await
                        .ok()
                        .unwrap_or_else(|| {
                            Err("failed to join system prompt templates load task".to_owned())
                        });
                match loaded {
                    Ok(templates) => Ok(VecDeque::from([Action::SystemPromptTemplatesLoaded {
                        templates,
                    }])),
                    Err(message) => {
                        tracing::warn!(message = %message, "failed to load system prompt templates");
                        Ok(VecDeque::new())
                    }
                }
            }
            Effect::MigrateLegacyTaskPromptTemplates { templates } => {
                if templates.is_empty() {
                    return Ok(VecDeque::new());
                }
                let services = self.services.clone();
                let migrated = tokio::task::spawn_blocking(move || {
                    let existing = services.task_prompt_templates_load().unwrap_or_default();
                    if !existing.is_empty() {
                        return Ok::<(), String>(());
                    }
                    for (kind, template) in templates {
                        services.task_prompt_template_store(kind, template)?;
                    }
                    Ok(())
                })
                .await
                .ok()
                .unwrap_or_else(|| {
                    Err("failed to join task prompt templates migrate task".to_owned())
                });
                if let Err(message) = migrated {
                    tracing::warn!(message = %message, "failed to migrate legacy task prompt templates");
                }
                Ok(VecDeque::new())
            }
            Effect::StoreTaskPromptTemplate {
                intent_kind,
                template,
            } => {
                let services = self.services.clone();
                let saved = tokio::task::spawn_blocking(move || {
                    services.task_prompt_template_store(intent_kind, template)
                })
                .await
                .ok()
                .unwrap_or_else(|| {
                    Err("failed to join task prompt template store task".to_owned())
                });
                if let Err(message) = saved {
                    tracing::warn!(message = %message, "failed to store task prompt template");
                }
                Ok(VecDeque::new())
            }
            Effect::DeleteTaskPromptTemplate { intent_kind } => {
                let services = self.services.clone();
                let deleted = tokio::task::spawn_blocking(move || {
                    services.task_prompt_template_delete(intent_kind)
                })
                .await
                .ok()
                .unwrap_or_else(|| {
                    Err("failed to join task prompt template delete task".to_owned())
                });
                if let Err(message) = deleted {
                    tracing::warn!(message = %message, "failed to delete task prompt template");
                }
                Ok(VecDeque::new())
            }
            Effect::StoreSystemPromptTemplate { kind, template } => {
                let services = self.services.clone();
                let saved = tokio::task::spawn_blocking(move || {
                    services.system_prompt_template_store(kind, template)
                })
                .await
                .ok()
                .unwrap_or_else(|| {
                    Err("failed to join system prompt template store task".to_owned())
                });
                if let Err(message) = saved {
                    tracing::warn!(message = %message, "failed to store system prompt template");
                }
                Ok(VecDeque::new())
            }
            Effect::DeleteSystemPromptTemplate { kind } => {
                let services = self.services.clone();
                let deleted = tokio::task::spawn_blocking(move || {
                    services.system_prompt_template_delete(kind)
                })
                .await
                .ok()
                .unwrap_or_else(|| {
                    Err("failed to join system prompt template delete task".to_owned())
                });
                if let Err(message) = deleted {
                    tracing::warn!(message = %message, "failed to delete system prompt template");
                }
                Ok(VecDeque::new())
            }
            Effect::CreateWorkspace {
                project_id,
                branch_name_hint,
            } => {
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
                    services.create_workspace(project_path, project_slug, branch_name_hint)
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
            Effect::RenameWorkspaceBranch {
                workspace_id,
                requested_branch_name,
            } => {
                let Some(workspace) = self.state.workspace(workspace_id) else {
                    return Ok(VecDeque::from([Action::WorkspaceBranchRenameFailed {
                        workspace_id,
                        message: "workspace not found".to_owned(),
                    }]));
                };

                let worktree_path = workspace.worktree_path.clone();
                let services = self.services.clone();
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        services.rename_workspace_branch(worktree_path, requested_branch_name)
                    })
                    .await
                    .ok()
                    .unwrap_or_else(|| {
                        Err("failed to join rename workspace branch task".to_owned())
                    });

                    let action = match result {
                        Ok(branch_name) => Action::WorkspaceBranchRenamed {
                            workspace_id,
                            branch_name,
                        },
                        Err(message) => Action::WorkspaceBranchRenameFailed {
                            workspace_id,
                            message,
                        },
                    };
                    let _ = tx
                        .send(EngineCommand::DispatchAction {
                            action: Box::new(action),
                        })
                        .await;
                });

                Ok(VecDeque::new())
            }
            Effect::AiRenameWorkspaceBranch {
                workspace_id,
                input,
            } => {
                if workspace_scope(&self.state, workspace_id).is_none() {
                    return Ok(VecDeque::from([Action::WorkspaceBranchRenameFailed {
                        workspace_id,
                        message: "workspace not found".to_owned(),
                    }]));
                };

                let worktree_path = self
                    .state
                    .workspace(workspace_id)
                    .map(|w| w.worktree_path.clone())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

                let services = self.services.clone();
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        let draft = luban_domain::TaskDraft {
                            input,
                            project: luban_domain::TaskProjectSpec::Unspecified,
                            intent_kind: luban_domain::TaskIntentKind::Other,
                            summary: String::new(),
                            prompt: String::new(),
                            repo: None,
                            issue: None,
                            pull_request: None,
                        };
                        let suggested = services.task_suggest_branch_name(draft)?;
                        services.rename_workspace_branch(worktree_path, suggested)
                    })
                    .await
                    .ok()
                    .unwrap_or_else(|| {
                        Err("failed to join ai rename workspace branch task".to_owned())
                    });

                    let action = match result {
                        Ok(branch_name) => Action::WorkspaceBranchRenamed {
                            workspace_id,
                            branch_name,
                        },
                        Err(message) => Action::WorkspaceBranchRenameFailed {
                            workspace_id,
                            message,
                        },
                    };
                    let _ = tx
                        .send(EngineCommand::DispatchAction {
                            action: Box::new(action),
                        })
                        .await;
                });

                Ok(VecDeque::new())
            }
            Effect::AiAutoTitleThread {
                workspace_id,
                thread_id,
                input,
                expected_current_title,
            } => {
                let Some(scope) = workspace_scope(&self.state, workspace_id) else {
                    return Ok(VecDeque::new());
                };

                let use_fake_agent = std::env::var_os("LUBAN_E2E_ROOT").is_some()
                    && std::env::var("LUBAN_CODEX_BIN")
                        .ok()
                        .is_some_and(|bin| bin == "/usr/bin/false");

                let services = self.services.clone();
                let tx = self.tx.clone();
                let project_slug = scope.project_slug;
                let workspace_name = scope.workspace_name;
                let thread_local_id = thread_id.as_u64();
                tokio::spawn(async move {
                    let services_for_suggest = services.clone();
                    let project_slug_for_update = project_slug.clone();
                    let workspace_name_for_update = workspace_name.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        let suggested = if use_fake_agent {
                            let derived = luban_domain::derive_thread_title(&input);
                            if derived.is_empty() {
                                "Thread".to_owned()
                            } else {
                                derived
                            }
                        } else {
                            services_for_suggest.task_suggest_thread_title(input)?
                        };

                        let suggested = luban_domain::derive_thread_title(&suggested);
                        if suggested.is_empty() {
                            return Ok::<_, String>(false);
                        }

                        services_for_suggest.conversation_update_title_if_matches(
                            project_slug_for_update,
                            workspace_name_for_update,
                            thread_local_id,
                            expected_current_title,
                            suggested,
                        )
                    })
                    .await
                    .ok()
                    .unwrap_or_else(|| Err("failed to join auto title thread task".to_owned()));

                    let Ok(updated) = result else {
                        return;
                    };
                    if !updated {
                        return;
                    }

                    let services_for_list = services.clone();
                    let project_slug_for_list = project_slug.clone();
                    let workspace_name_for_list = workspace_name.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        services_for_list.list_conversation_threads(
                            project_slug_for_list,
                            workspace_name_for_list,
                        )
                    })
                    .await
                    .ok()
                    .unwrap_or_else(|| Err("failed to join list threads task".to_owned()));

                    let Ok(threads) = result else {
                        return;
                    };

                    let _ = tx
                        .send(EngineCommand::DispatchAction {
                            action: Box::new(Action::WorkspaceThreadsLoaded {
                                workspace_id,
                                threads,
                            }),
                        })
                        .await;
                });

                Ok(VecDeque::new())
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
                    services.load_conversation_page(
                        scope.project_slug,
                        scope.workspace_name,
                        thread_local_id,
                        None,
                        5000,
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
            Effect::StoreConversationRunConfig {
                workspace_id,
                thread_id,
                runner,
                model_id,
                thinking_effort,
                amp_mode,
            } => {
                let Some(scope) = workspace_scope(&self.state, workspace_id) else {
                    return Ok(VecDeque::new());
                };
                let services = self.services.clone();
                let thread_local_id = thread_id.as_u64();
                let _ = tokio::task::spawn_blocking(move || {
                    services.save_conversation_run_config(
                        scope.project_slug,
                        scope.workspace_name,
                        thread_local_id,
                        runner,
                        model_id,
                        thinking_effort,
                        amp_mode,
                    )
                })
                .await;
                Ok(VecDeque::new())
            }
            Effect::RunAgentTurn {
                workspace_id,
                thread_id,
                run_id,
                text,
                attachments,
                run_config,
            } => {
                let started_at_unix_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .try_into()
                    .unwrap_or(0u64);

                let use_fake_agent = std::env::var_os("LUBAN_E2E_ROOT").is_some()
                    && std::env::var("LUBAN_CODEX_BIN")
                        .ok()
                        .is_some_and(|bin| bin == "/usr/bin/false");
                let fake_agent_delay = if use_fake_agent {
                    let prompt = text.as_str();
                    if prompt.contains("e2e-running-card")
                        || prompt.contains("e2e-streaming-message")
                    {
                        Duration::from_millis(3500)
                    } else if prompt.contains("e2e-ansi-output") {
                        Duration::from_millis(600)
                    } else if prompt.contains("e2e-cancel") {
                        Duration::from_millis(2500)
                    } else if prompt.contains("e2e-queued") {
                        Duration::from_millis(1500)
                    } else {
                        Duration::from_millis(50)
                    }
                } else {
                    Duration::from_millis(0)
                };

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
                    runner: run_config.runner,
                    amp_mode: run_config.amp_mode.clone(),
                    model: Some(run_config.model_id.clone()),
                    model_reasoning_effort: Some(run_config.thinking_effort.as_str().to_owned()),
                };

                let cancel = Arc::new(AtomicBool::new(false));
                self.cancel_flags.insert(
                    (workspace_id, thread_id),
                    CancelFlagEntry {
                        run_id,
                        flag: cancel.clone(),
                    },
                );

                if use_fake_agent {
                    let tx = self.tx.clone();
                    std::thread::spawn(move || {
                        let deadline = fake_agent_delay;
                        let start = Instant::now();
                        let prompt = request.prompt.clone();

                        let emit_many_steps = prompt.contains("e2e-many-steps");
                        let emit_pagination_steps = prompt.contains("e2e-pagination-steps");
                        let emit_markdown_reasoning = prompt.contains("e2e-thinking-markdown");
                        let emit_file_change = prompt.contains("e2e-file-change");
                        let emit_streaming_message = prompt.contains("e2e-streaming-message");
                        let emit_long_output = prompt.contains("e2e-long-output");

                        if emit_many_steps || emit_pagination_steps {
                            let count = if emit_pagination_steps {
                                2505u32
                            } else {
                                12_000u32
                            };
                            // Generate a large amount of completed items to stress the UI render/timing
                            // paths. This is used only in e2e mode (`LUBAN_E2E_ROOT` + fake codex bin).
                            // Keep the IDs simple and stable.
                            for i in 0..count {
                                if cancel.load(Ordering::SeqCst) {
                                    break;
                                }
                                let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                    action: Box::new(Action::AgentEventReceived {
                                        workspace_id,
                                        thread_id,
                                        run_id,
                                        event: luban_domain::CodexThreadEvent::ItemCompleted {
                                            item: luban_domain::CodexThreadItem::CommandExecution {
                                                id: format!("e2e_many_{i}"),
                                                command: format!("echo {i}"),
                                                aggregated_output: "ok".to_owned(),
                                                exit_code: Some(0),
                                                status: luban_domain::CodexCommandExecutionStatus::Completed,
                                            },
                                        },
                                    }),
                                });
                            }

                            if !cancel.load(Ordering::SeqCst) {
                                let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                    action: Box::new(Action::AgentEventReceived {
                                        workspace_id,
                                        thread_id,
                                        run_id,
                                        event: luban_domain::CodexThreadEvent::TurnFailed {
                                            error: luban_domain::CodexThreadError {
                                                message: "e2e agent stub".to_owned(),
                                            },
                                        },
                                    }),
                                });
                            }

                            let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                action: Box::new(Action::AgentTurnFinished {
                                    workspace_id,
                                    thread_id,
                                    run_id,
                                }),
                            });
                            return;
                        }

                        let mut sent_1_start = false;
                        let mut sent_1_done = false;
                        let mut sent_2_start = false;
                        let mut sent_2_done = false;
                        let mut sent_3_start = false;
                        let mut sent_ansi_output = false;
                        let mut streaming_started = false;
                        let mut streaming_completed = false;
                        let streaming_id = "e2e_stream_msg_1".to_owned();
                        let streaming_needle = "e2e-selection-needle";
                        let mut streaming_text = String::new();
                        let mut streaming_chunks_sent: u32 = 0;

                        while start.elapsed() < deadline && !cancel.load(Ordering::SeqCst) {
                            let elapsed = start.elapsed();

                            if emit_streaming_message && !streaming_completed {
                                if !streaming_started && elapsed >= Duration::from_millis(50) {
                                    streaming_started = true;
                                    streaming_text =
                                        format!("Streaming...\n\n{streaming_needle}\n\n");
                                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                        action: Box::new(Action::AgentEventReceived {
                                            workspace_id,
                                            thread_id,
                                            run_id,
                                            event: luban_domain::CodexThreadEvent::ItemStarted {
                                                item: luban_domain::CodexThreadItem::AgentMessage {
                                                    id: streaming_id.clone(),
                                                    text: streaming_text.clone(),
                                                },
                                            },
                                        }),
                                    });
                                }

                                if streaming_started {
                                    let chunk_every_ms = 120u64;
                                    let elapsed_ms = elapsed.as_millis() as u64;
                                    let expected_chunks =
                                        (elapsed_ms / chunk_every_ms).min(25) as u32;
                                    while streaming_chunks_sent < expected_chunks {
                                        streaming_chunks_sent += 1;
                                        streaming_text.push_str(&format!(
                                            "chunk-{:02}\n",
                                            streaming_chunks_sent
                                        ));
                                        let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                            action: Box::new(Action::AgentEventReceived {
                                                workspace_id,
                                                thread_id,
                                                run_id,
                                                event: luban_domain::CodexThreadEvent::ItemUpdated {
                                                    item: luban_domain::CodexThreadItem::AgentMessage {
                                                        id: streaming_id.clone(),
                                                        text: streaming_text.clone(),
                                                    },
                                                },
                                            }),
                                        });
                                    }
                                }

                                if elapsed >= Duration::from_millis(3000) {
                                    streaming_completed = true;
                                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                        action: Box::new(Action::AgentEventReceived {
                                            workspace_id,
                                            thread_id,
                                            run_id,
                                            event: luban_domain::CodexThreadEvent::ItemCompleted {
                                                item: luban_domain::CodexThreadItem::AgentMessage {
                                                    id: streaming_id.clone(),
                                                    text: streaming_text.clone(),
                                                },
                                            },
                                        }),
                                    });
                                }
                            }

                            if prompt.contains("e2e-ansi-output")
                                && !sent_ansi_output
                                && elapsed >= Duration::from_millis(75)
                            {
                                sent_ansi_output = true;
                                let aggregated_output = [
                                    "[[2m[WebServer] [[22m Finished 'dev' profile [unoptimized + debuginfo] target(s) in 0.33s",
                                    "[[2m[WebServer] [[22m Running 'target/debug/luban_server'",
                                    "",
                                    "(node:4596) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.",
                                    "",
                                    "[[1A[[2K[[0G [[32m√[[39m [[2mtests/e2e/chat-ui.spec.ts:334:5 › enter commits IME composition without sending[[22m",
                                    "[[32m  2 passed[[39m[[2m (14.1s)[[22m",
                                ]
                                .join("\n");
                                let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                    action: Box::new(Action::AgentEventReceived {
                                        workspace_id,
                                        thread_id,
                                        run_id,
                                        event: luban_domain::CodexThreadEvent::ItemCompleted {
                                            item: luban_domain::CodexThreadItem::CommandExecution {
                                                id: "e2e_ansi_cmd_1".to_owned(),
                                                command: "zsh -lc \"just test-ui\"".to_owned(),
                                                aggregated_output,
                                                exit_code: Some(0),
                                                status: luban_domain::CodexCommandExecutionStatus::Completed,
                                            },
                                        },
                                    }),
                                });
                            }

                            if prompt.contains("e2e-running-card") {
                                if !sent_1_start && elapsed >= Duration::from_millis(50) {
                                    sent_1_start = true;
                                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                        action: Box::new(Action::AgentEventReceived {
                                            workspace_id,
                                            thread_id,
                                            run_id,
                                            event: luban_domain::CodexThreadEvent::ItemStarted {
                                                item: luban_domain::CodexThreadItem::CommandExecution {
                                                    id: "e2e_cmd_1".to_owned(),
                                                    command: "echo 1".to_owned(),
                                                    aggregated_output: "".to_owned(),
                                                    exit_code: None,
                                                    status: luban_domain::CodexCommandExecutionStatus::InProgress,
                                                },
                                            },
                                        }),
                                    });
                                }
                                if !sent_1_done && elapsed >= Duration::from_millis(250) {
                                    sent_1_done = true;
                                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
	                                        action: Box::new(Action::AgentEventReceived {
	                                            workspace_id,
	                                            thread_id,
	                                            run_id,
	                                            event: luban_domain::CodexThreadEvent::ItemCompleted {
	                                                item: luban_domain::CodexThreadItem::CommandExecution {
	                                                    id: "e2e_cmd_1".to_owned(),
	                                                    command: "echo 1".to_owned(),
	                                                    aggregated_output: "".to_owned(),
	                                                    exit_code: Some(0),
	                                                    status: luban_domain::CodexCommandExecutionStatus::Completed,
	                                                },
	                                            },
	                                        }),
	                                    });
                                }
                                if !sent_2_start && elapsed >= Duration::from_millis(350) {
                                    sent_2_start = true;
                                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                        action: Box::new(Action::AgentEventReceived {
                                            workspace_id,
                                            thread_id,
                                            run_id,
                                            event: luban_domain::CodexThreadEvent::ItemStarted {
                                                item: luban_domain::CodexThreadItem::CommandExecution {
                                                    id: "e2e_cmd_2".to_owned(),
                                                    command: "echo 2".to_owned(),
                                                    aggregated_output: "".to_owned(),
                                                    exit_code: None,
                                                    status: luban_domain::CodexCommandExecutionStatus::InProgress,
                                                },
                                            },
                                        }),
                                    });
                                }
                                if !sent_2_done && elapsed >= Duration::from_millis(1750) {
                                    sent_2_done = true;
                                    let aggregated_output = if emit_long_output {
                                        [
                                            "test io::commit::conflict_resolver::tests::test_conflicting_rebase::ours_1__update_full__::other_1__update_full__ ... ok",
                                            "test io::commit::conflict_resolver::tests::test_conflicting_rebase::ours_1__update_full__::other_2__update_partial__ ... ok",
                                            "test io::commit::conflict_resolver::tests::test_conflicting_rebase::ours_2__update_partial__::other_4__delete_partial__ ... ok",
                                        ]
                                        .join("\n")
                                    } else {
                                        "ok".to_owned()
                                    };
                                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                        action: Box::new(Action::AgentEventReceived {
                                            workspace_id,
                                            thread_id,
                                            run_id,
                                            event: luban_domain::CodexThreadEvent::ItemCompleted {
                                                item: luban_domain::CodexThreadItem::CommandExecution {
                                                    id: "e2e_cmd_2".to_owned(),
                                                    command: "echo 2".to_owned(),
                                                    aggregated_output,
                                                    exit_code: Some(0),
                                                    status: luban_domain::CodexCommandExecutionStatus::Completed,
                                                },
                                            },
                                        }),
                                    });
                                }
                                if !sent_3_start && elapsed >= Duration::from_millis(1800) {
                                    sent_3_start = true;
                                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                        action: Box::new(Action::AgentEventReceived {
                                            workspace_id,
                                            thread_id,
                                            run_id,
                                            event: luban_domain::CodexThreadEvent::ItemStarted {
                                                item: luban_domain::CodexThreadItem::CommandExecution {
                                                    id: "e2e_cmd_3".to_owned(),
                                                    command: "echo 3".to_owned(),
                                                    aggregated_output: "".to_owned(),
                                                    exit_code: None,
                                                    status: luban_domain::CodexCommandExecutionStatus::InProgress,
                                                },
                                            },
                                        }),
                                    });
                                }
                            }

                            std::thread::sleep(Duration::from_millis(25));
                        }

                        if !cancel.load(Ordering::SeqCst) {
                            if emit_markdown_reasoning {
                                let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                    action: Box::new(Action::AgentEventReceived {
                                        workspace_id,
                                        thread_id,
                                        run_id,
                                        event: luban_domain::CodexThreadEvent::ItemStarted {
                                            item: luban_domain::CodexThreadItem::Reasoning {
                                                id: "e2e_reasoning_1".to_owned(),
                                                text:
                                                    "**Plan**: verify markdown summary stripping."
                                                        .to_owned(),
                                            },
                                        },
                                    }),
                                });

                                let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                    action: Box::new(Action::AgentEventReceived {
                                        workspace_id,
                                        thread_id,
                                        run_id,
                                        event: luban_domain::CodexThreadEvent::ItemCompleted {
                                            item: luban_domain::CodexThreadItem::Reasoning {
                                                id: "e2e_reasoning_1".to_owned(),
                                                text:
                                                    "**Plan**: verify markdown summary stripping."
                                                        .to_owned(),
                                            },
                                        },
                                    }),
                                });
                            }

                            if prompt.contains("e2e-mermaid") {
                                let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                    action: Box::new(Action::AgentEventReceived {
                                        workspace_id,
                                        thread_id,
                                        run_id,
                                        event: luban_domain::CodexThreadEvent::ItemCompleted {
                                            item: luban_domain::CodexThreadItem::AgentMessage {
                                                id: "e2e_mermaid_1".to_owned(),
                                                text: prompt.clone(),
                                            },
                                        },
                                    }),
                                });
                            }

                            if emit_file_change {
                                let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                    action: Box::new(Action::AgentEventReceived {
                                        workspace_id,
                                        thread_id,
                                        run_id,
                                        event: luban_domain::CodexThreadEvent::ItemCompleted {
                                            item: luban_domain::CodexThreadItem::FileChange {
                                                id: "e2e_file_change_1".to_owned(),
                                                changes: vec![
                                                    luban_domain::CodexFileUpdateChange {
                                                        path: "src/e2e-file-change/a.txt".to_owned(),
                                                        kind: luban_domain::CodexPatchChangeKind::Add,
                                                    },
                                                    luban_domain::CodexFileUpdateChange {
                                                        path: "web/e2e-file-change/b.ts".to_owned(),
                                                        kind: luban_domain::CodexPatchChangeKind::Update,
                                                    },
                                                    luban_domain::CodexFileUpdateChange {
                                                        path: "README.md".to_owned(),
                                                        kind: luban_domain::CodexPatchChangeKind::Delete,
                                                    },
                                                ],
                                                status: luban_domain::CodexPatchApplyStatus::Completed,
                                            },
                                        },
                                    }),
                                });
                            }

                            let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                action: Box::new(Action::AgentEventReceived {
                                    workspace_id,
                                    thread_id,
                                    run_id,
                                    event: luban_domain::CodexThreadEvent::TurnFailed {
                                        error: luban_domain::CodexThreadError {
                                            message: "e2e agent stub".to_owned(),
                                        },
                                    },
                                }),
                            });
                        }

                        if cancel.load(Ordering::SeqCst) {
                            return;
                        }

                        let _ = tx.blocking_send(EngineCommand::DispatchAction {
                            action: Box::new(Action::AgentRunFinishedAt {
                                workspace_id,
                                thread_id,
                                run_id,
                                finished_at_unix_ms: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis()
                                    .try_into()
                                    .unwrap_or(0u64),
                            }),
                        });

                        let _ = tx.blocking_send(EngineCommand::DispatchAction {
                            action: Box::new(Action::AgentTurnFinished {
                                workspace_id,
                                thread_id,
                                run_id,
                            }),
                        });
                    });

                    return Ok(VecDeque::from([Action::AgentRunStartedAt {
                        workspace_id,
                        thread_id,
                        run_id,
                        started_at_unix_ms,
                    }]));
                }

                let services = self.services.clone();
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let on_event: Arc<dyn Fn(luban_domain::AgentThreadEvent) + Send + Sync> = {
                        let tx = tx.clone();
                        Arc::new(move |event| {
                            let _ = tx.blocking_send(EngineCommand::DispatchAction {
                                action: Box::new(Action::AgentEventReceived {
                                    workspace_id,
                                    thread_id,
                                    run_id,
                                    event,
                                }),
                            });
                        })
                    };

                    let result =
                        services.run_agent_turn_streamed(request, cancel.clone(), on_event);
                    if let Err(message) = result
                        && !cancel.load(Ordering::SeqCst)
                    {
                        let _ = tx.blocking_send(EngineCommand::DispatchAction {
                            action: Box::new(Action::AgentEventReceived {
                                workspace_id,
                                thread_id,
                                run_id,
                                event: luban_domain::CodexThreadEvent::Error { message },
                            }),
                        });
                    }

                    if cancel.load(Ordering::SeqCst) {
                        return;
                    }

                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
                        action: Box::new(Action::AgentRunFinishedAt {
                            workspace_id,
                            thread_id,
                            run_id,
                            finished_at_unix_ms: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                                .try_into()
                                .unwrap_or(0u64),
                        }),
                    });

                    let _ = tx.blocking_send(EngineCommand::DispatchAction {
                        action: Box::new(Action::AgentTurnFinished {
                            workspace_id,
                            thread_id,
                            run_id,
                        }),
                    });
                });

                Ok(VecDeque::from([Action::AgentRunStartedAt {
                    workspace_id,
                    thread_id,
                    run_id,
                    started_at_unix_ms,
                }]))
            }
            Effect::CancelAgentTurn {
                workspace_id,
                thread_id,
                run_id,
            } => {
                if let Some(entry) = self.cancel_flags.get(&(workspace_id, thread_id))
                    && entry.run_id == run_id
                {
                    entry.flag.store(true, Ordering::SeqCst);
                }
                let finished_at_unix_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .try_into()
                    .unwrap_or(0u64);
                Ok(VecDeque::from([Action::AgentRunFinishedAt {
                    workspace_id,
                    thread_id,
                    run_id,
                    finished_at_unix_ms,
                }]))
            }
            Effect::CleanupClaudeProcess {
                workspace_id,
                thread_id,
            } => {
                // Clean up any persistent Claude process for this thread
                if let Some(scope) = workspace_scope(&self.state, workspace_id) {
                    self.services.cleanup_claude_process(
                        &scope.project_slug,
                        &scope.workspace_name,
                        thread_id.as_u64(),
                    );
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
                            event: Box::new(luban_api::ServerEvent::Toast {
                                message: message.clone(),
                            }),
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
                            event: Box::new(luban_api::ServerEvent::Toast {
                                message: message.clone(),
                            }),
                        });
                        Ok(VecDeque::from([
                            Action::OpenWorkspacePullRequestFailedActionFailed { message },
                        ]))
                    }
                }
            }
            Effect::OpenWorkspaceInIde { workspace_id } => {
                let Some(workspace) = self.state.workspace(workspace_id) else {
                    return Ok(VecDeque::new());
                };

                let services = self.services.clone();
                let worktree_path = workspace.worktree_path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    services.open_workspace_in_ide(worktree_path)
                })
                .await
                .ok()
                .unwrap_or_else(|| Err("failed to join open workspace in ide task".to_owned()));

                match result {
                    Ok(()) => Ok(VecDeque::new()),
                    Err(message) => {
                        let _ = self.events.send(WsServerMessage::Event {
                            rev: self.rev,
                            event: Box::new(luban_api::ServerEvent::Toast {
                                message: message.clone(),
                            }),
                        });
                        Ok(VecDeque::from([Action::OpenWorkspaceInIdeFailed {
                            message,
                        }]))
                    }
                }
            }
            Effect::OpenWorkspaceWith {
                workspace_id,
                target,
            } => {
                let Some(workspace) = self.state.workspace(workspace_id) else {
                    return Ok(VecDeque::new());
                };

                let services = self.services.clone();
                let worktree_path = workspace.worktree_path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    services.open_workspace_with(worktree_path, target)
                })
                .await
                .ok()
                .unwrap_or_else(|| Err("failed to join open workspace with task".to_owned()));

                match result {
                    Ok(()) => Ok(VecDeque::new()),
                    Err(message) => {
                        let _ = self.events.send(WsServerMessage::Event {
                            rev: self.rev,
                            event: Box::new(luban_api::ServerEvent::Toast {
                                message: message.clone(),
                            }),
                        });
                        Ok(VecDeque::from([Action::OpenWorkspaceWithFailed {
                            message,
                        }]))
                    }
                }
            }
            Effect::ArchiveWorkspace { workspace_id } => {
                if let Some(scope) = workspace_scope(&self.state, workspace_id) {
                    for (wid, thread_id) in self.state.conversations.keys() {
                        if *wid != workspace_id {
                            continue;
                        }
                        self.services.cleanup_claude_process(
                            &scope.project_slug,
                            &scope.workspace_name,
                            thread_id.as_u64(),
                        );
                    }
                }

                let mut project_path: Option<PathBuf> = None;
                let mut worktree_path: Option<PathBuf> = None;

                for project in &self.state.projects {
                    for workspace in &project.workspaces {
                        if workspace.id == workspace_id {
                            project_path = Some(project.path.clone());
                            worktree_path = Some(workspace.worktree_path.clone());
                            break;
                        }
                    }
                    if project_path.is_some() {
                        break;
                    }
                }

                let (Some(project_path), Some(worktree_path)) = (project_path, worktree_path)
                else {
                    return Ok(VecDeque::from([Action::WorkspaceArchiveFailed {
                        workspace_id,
                        message: "workspace not found".to_owned(),
                    }]));
                };

                let services = self.services.clone();
                let result = tokio::task::spawn_blocking(move || {
                    services.archive_workspace(project_path, worktree_path)
                })
                .await
                .ok()
                .unwrap_or_else(|| Err("failed to join archive workspace task".to_owned()));

                let action = match result {
                    Ok(()) => Action::WorkspaceArchived { workspace_id },
                    Err(message) => Action::WorkspaceArchiveFailed {
                        workspace_id,
                        message,
                    },
                };

                Ok(VecDeque::from([action]))
            }
        }
    }

    fn publish_app_snapshot(&self) {
        let _ = self.events.send(WsServerMessage::Event {
            rev: self.rev,
            event: Box::new(luban_api::ServerEvent::AppChanged {
                rev: self.rev,
                snapshot: Box::new(self.app_snapshot()),
            }),
        });
    }

    fn publish_threads_event(
        &self,
        workspace_id: WorkspaceId,
        threads: &[luban_domain::ConversationThreadMeta],
    ) {
        let api_id = luban_api::WorkspaceId(workspace_id.as_u64());
        let tabs = self
            .state
            .workspace_tabs(workspace_id)
            .map(map_workspace_tabs_snapshot)
            .unwrap_or_default();
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
            event: Box::new(luban_api::ServerEvent::WorkspaceThreadsChanged {
                workspace_id: api_id,
                tabs,
                threads,
            }),
        });
    }

    fn publish_conversation_snapshot(
        &self,
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    ) {
        let api_wid = luban_api::WorkspaceId(workspace_id.as_u64());
        let api_tid = luban_api::WorkspaceThreadId(thread_id.as_u64());
        if let Ok(snapshot) = self.conversation_snapshot(api_wid, api_tid, None, None) {
            let _ = self.events.send(WsServerMessage::Event {
                rev: self.rev,
                event: Box::new(luban_api::ServerEvent::ConversationChanged {
                    snapshot: Box::new(snapshot),
                }),
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
                .map(|p| {
                    let path = p.path.to_string_lossy().to_string();
                    luban_api::ProjectSnapshot {
                        id: luban_api::ProjectId(path.clone()),
                        name: p.name.clone(),
                        slug: p.slug.clone(),
                        path,
                        is_git: p.is_git,
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
                                archive_status: match w.archive_status {
                                    OperationStatus::Idle => luban_api::OperationStatus::Idle,
                                    OperationStatus::Running => luban_api::OperationStatus::Running,
                                },
                                branch_rename_status: match w.branch_rename_status {
                                    OperationStatus::Idle => luban_api::OperationStatus::Idle,
                                    OperationStatus::Running => luban_api::OperationStatus::Running,
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
                    }
                })
                .collect(),
            appearance: luban_api::AppearanceSnapshot {
                theme: match self.state.appearance_theme {
                    luban_domain::AppearanceTheme::Light => luban_api::AppearanceTheme::Light,
                    luban_domain::AppearanceTheme::Dark => luban_api::AppearanceTheme::Dark,
                    luban_domain::AppearanceTheme::System => luban_api::AppearanceTheme::System,
                },
                fonts: luban_api::AppearanceFontsSnapshot {
                    ui_font: self.state.appearance_fonts.ui_font.clone(),
                    chat_font: self.state.appearance_fonts.chat_font.clone(),
                    code_font: self.state.appearance_fonts.code_font.clone(),
                    terminal_font: self.state.appearance_fonts.terminal_font.clone(),
                },
                global_zoom: (self.state.global_zoom_percent as f64) / 100.0,
            },
            agent: luban_api::AgentSettingsSnapshot {
                codex_enabled: self.state.agent_codex_enabled(),
                amp_enabled: self.state.agent_amp_enabled(),
                claude_enabled: self.state.agent_claude_enabled(),
                default_model_id: Some(self.state.agent_default_model_id().to_owned()),
                default_thinking_effort: Some(match self.state.agent_default_thinking_effort() {
                    ThinkingEffort::Minimal => luban_api::ThinkingEffort::Minimal,
                    ThinkingEffort::Low => luban_api::ThinkingEffort::Low,
                    ThinkingEffort::Medium => luban_api::ThinkingEffort::Medium,
                    ThinkingEffort::High => luban_api::ThinkingEffort::High,
                    ThinkingEffort::XHigh => luban_api::ThinkingEffort::XHigh,
                }),
                default_runner: Some(match self.state.agent_default_runner() {
                    luban_domain::AgentRunnerKind::Codex => luban_api::AgentRunnerKind::Codex,
                    luban_domain::AgentRunnerKind::Amp => luban_api::AgentRunnerKind::Amp,
                    luban_domain::AgentRunnerKind::Claude => luban_api::AgentRunnerKind::Claude,
                }),
                amp_mode: Some(self.state.agent_amp_mode().to_owned()),
            },
            task: luban_api::TaskSettingsSnapshot {
                prompt_templates: luban_domain::TaskIntentKind::ALL
                    .iter()
                    .copied()
                    .filter_map(|kind| {
                        self.state.task_prompt_templates.get(&kind).map(|template| {
                            luban_api::TaskPromptTemplateSnapshot {
                                intent_kind: map_task_intent_kind(kind),
                                template: template.clone(),
                            }
                        })
                    })
                    .collect(),
                default_prompt_templates: luban_domain::TaskIntentKind::ALL
                    .iter()
                    .copied()
                    .map(|kind| luban_api::TaskPromptTemplateSnapshot {
                        intent_kind: map_task_intent_kind(kind),
                        template: luban_domain::default_task_prompt_template(kind),
                    })
                    .collect(),
                system_prompt_templates: luban_domain::SystemTaskKind::ALL
                    .iter()
                    .copied()
                    .filter_map(|kind| {
                        self.state
                            .system_prompt_templates
                            .get(&kind)
                            .map(|template| luban_api::SystemPromptTemplateSnapshot {
                                kind: map_system_task_kind(kind),
                                template: template.clone(),
                            })
                    })
                    .collect(),
                default_system_prompt_templates: luban_domain::SystemTaskKind::ALL
                    .iter()
                    .copied()
                    .map(|kind| luban_api::SystemPromptTemplateSnapshot {
                        kind: map_system_task_kind(kind),
                        template: luban_domain::default_system_prompt_template(kind),
                    })
                    .collect(),
            },
            ui: {
                let active_workspace_id = match self.state.main_pane {
                    luban_domain::MainPane::Workspace(id) => Some(id),
                    _ => self.state.last_open_workspace_id,
                };
                let active_thread_id =
                    active_workspace_id.and_then(|id| self.state.active_thread_id(id));
                luban_api::UiSnapshot {
                    active_workspace_id: active_workspace_id
                        .map(|id| luban_api::WorkspaceId(id.as_u64())),
                    active_thread_id: active_thread_id
                        .map(|id| luban_api::WorkspaceThreadId(id.as_u64())),
                    open_button_selection: self.state.open_button_selection.clone(),
                    sidebar_project_order: self
                        .state
                        .sidebar_project_order
                        .iter()
                        .cloned()
                        .map(luban_api::ProjectId)
                        .collect(),
                }
            },
        }
    }

    // Threads snapshots are served via `ProjectWorkspaceService::list_conversation_threads` in the command handler.

    fn conversation_snapshot(
        &self,
        workspace_id: luban_api::WorkspaceId,
        thread_id: luban_api::WorkspaceThreadId,
        before: Option<u64>,
        limit: Option<u64>,
    ) -> anyhow::Result<ConversationSnapshot> {
        const DEFAULT_ENTRIES_LIMIT: usize = 2000;
        const MAX_ENTRIES_LIMIT: usize = 5000;

        let limit = limit
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(DEFAULT_ENTRIES_LIMIT)
            .clamp(1, MAX_ENTRIES_LIMIT);

        let wid = WorkspaceId::from_u64(workspace_id.0);
        let tid = WorkspaceThreadId::from_u64(thread_id.0);
        let Some(conversation) = self.state.workspace_thread_conversation(wid, tid) else {
            return Err(anyhow::anyhow!("conversation not found"));
        };

        let window_start = usize::try_from(conversation.entries_start).unwrap_or(0);
        let window_end = window_start.saturating_add(conversation.entries.len());
        let total_entries = usize::try_from(conversation.entries_total).unwrap_or(window_end);

        let before = before
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(total_entries)
            .min(total_entries);
        let end = before;
        let start = end.saturating_sub(limit);
        let entries_truncated = start > 0 || end < total_entries;

        if start < window_start || end > window_end {
            return Err(anyhow::anyhow!("requested slice is not in memory"));
        }

        let local_start = start.saturating_sub(window_start);
        let local_end = end.saturating_sub(window_start);

        Ok(ConversationSnapshot {
            rev: self.rev,
            workspace_id,
            thread_id,
            agent_runner: match conversation.agent_runner {
                luban_domain::AgentRunnerKind::Codex => luban_api::AgentRunnerKind::Codex,
                luban_domain::AgentRunnerKind::Amp => luban_api::AgentRunnerKind::Amp,
                luban_domain::AgentRunnerKind::Claude => luban_api::AgentRunnerKind::Claude,
            },
            agent_model_id: conversation.agent_model_id.clone(),
            thinking_effort: match conversation.thinking_effort {
                ThinkingEffort::Minimal => luban_api::ThinkingEffort::Minimal,
                ThinkingEffort::Low => luban_api::ThinkingEffort::Low,
                ThinkingEffort::Medium => luban_api::ThinkingEffort::Medium,
                ThinkingEffort::High => luban_api::ThinkingEffort::High,
                ThinkingEffort::XHigh => luban_api::ThinkingEffort::XHigh,
            },
            amp_mode: if conversation.agent_runner == luban_domain::AgentRunnerKind::Amp {
                conversation
                    .amp_mode
                    .clone()
                    .or_else(|| Some(self.state.agent_amp_mode().to_owned()))
            } else {
                None
            },
            run_status: match conversation.run_status {
                OperationStatus::Idle => luban_api::OperationStatus::Idle,
                OperationStatus::Running => luban_api::OperationStatus::Running,
            },
            run_started_at_unix_ms: conversation.run_started_at_unix_ms,
            run_finished_at_unix_ms: conversation.run_finished_at_unix_ms,
            entries: conversation
                .entries
                .get(local_start..local_end)
                .unwrap_or_default()
                .iter()
                .map(map_conversation_entry)
                .collect(),
            entries_total: total_entries as u64,
            entries_start: start as u64,
            entries_truncated,
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
            pending_prompts: conversation
                .pending_prompts
                .iter()
                .map(|prompt| luban_api::QueuedPromptSnapshot {
                    id: prompt.id,
                    text: prompt.text.clone(),
                    attachments: prompt.attachments.iter().map(map_attachment_ref).collect(),
                    run_config: luban_api::AgentRunConfigSnapshot {
                        runner: match prompt.run_config.runner {
                            luban_domain::AgentRunnerKind::Codex => {
                                luban_api::AgentRunnerKind::Codex
                            }
                            luban_domain::AgentRunnerKind::Amp => luban_api::AgentRunnerKind::Amp,
                            luban_domain::AgentRunnerKind::Claude => {
                                luban_api::AgentRunnerKind::Claude
                            }
                        },
                        model_id: prompt.run_config.model_id.clone(),
                        thinking_effort: match prompt.run_config.thinking_effort {
                            ThinkingEffort::Minimal => luban_api::ThinkingEffort::Minimal,
                            ThinkingEffort::Low => luban_api::ThinkingEffort::Low,
                            ThinkingEffort::Medium => luban_api::ThinkingEffort::Medium,
                            ThinkingEffort::High => luban_api::ThinkingEffort::High,
                            ThinkingEffort::XHigh => luban_api::ThinkingEffort::XHigh,
                        },
                        amp_mode: prompt.run_config.amp_mode.clone(),
                    },
                })
                .collect(),
            queue_paused: conversation.queue_paused,
            remote_thread_id: conversation.thread_id.clone(),
            title: conversation.title.clone(),
        })
    }
}

fn normalize_project_path(path: &std::path::Path) -> PathBuf {
    use std::path::Component;

    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let popped = out.pop();
                if !popped {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn find_project_id_by_path(
    state: &AppState,
    path: &std::path::Path,
) -> Option<luban_domain::ProjectId> {
    let normalized_path = normalize_project_path(path);
    state
        .projects
        .iter()
        .find(|p| normalize_project_path(&p.path) == normalized_path)
        .map(|p| p.id)
}

fn map_task_intent_kind(kind: luban_domain::TaskIntentKind) -> luban_api::TaskIntentKind {
    match kind {
        luban_domain::TaskIntentKind::Fix => luban_api::TaskIntentKind::Fix,
        luban_domain::TaskIntentKind::Implement => luban_api::TaskIntentKind::Implement,
        luban_domain::TaskIntentKind::Review => luban_api::TaskIntentKind::Review,
        luban_domain::TaskIntentKind::Discuss => luban_api::TaskIntentKind::Discuss,
        luban_domain::TaskIntentKind::Other => luban_api::TaskIntentKind::Other,
    }
}

fn map_system_task_kind(kind: luban_domain::SystemTaskKind) -> luban_api::SystemTaskKind {
    match kind {
        luban_domain::SystemTaskKind::InferType => luban_api::SystemTaskKind::InferType,
        luban_domain::SystemTaskKind::RenameBranch => luban_api::SystemTaskKind::RenameBranch,
        luban_domain::SystemTaskKind::AutoTitleThread => luban_api::SystemTaskKind::AutoTitleThread,
    }
}

fn map_task_project_spec(spec: &luban_domain::TaskProjectSpec) -> luban_api::TaskProjectSpec {
    match spec {
        luban_domain::TaskProjectSpec::Unspecified => luban_api::TaskProjectSpec::Unspecified,
        luban_domain::TaskProjectSpec::LocalPath { path } => {
            luban_api::TaskProjectSpec::LocalPath {
                path: path.to_string_lossy().to_string(),
            }
        }
        luban_domain::TaskProjectSpec::GitHubRepo { full_name } => {
            luban_api::TaskProjectSpec::GitHubRepo {
                full_name: full_name.clone(),
            }
        }
    }
}

fn map_task_draft(draft: &luban_domain::TaskDraft) -> luban_api::TaskDraft {
    luban_api::TaskDraft {
        input: draft.input.clone(),
        project: map_task_project_spec(&draft.project),
        intent_kind: map_task_intent_kind(draft.intent_kind),
        summary: draft.summary.clone(),
        prompt: draft.prompt.clone(),
        repo: draft.repo.as_ref().map(|r| luban_api::TaskRepoInfo {
            full_name: r.full_name.clone(),
            url: r.url.clone(),
            default_branch: r.default_branch.clone(),
        }),
        issue: draft.issue.as_ref().map(|i| luban_api::TaskIssueInfo {
            number: i.number,
            title: i.title.clone(),
            url: i.url.clone(),
        }),
        pull_request: draft
            .pull_request
            .as_ref()
            .map(|pr| luban_api::TaskPullRequestInfo {
                number: pr.number,
                title: pr.title.clone(),
                url: pr.url.clone(),
                head_ref: pr.head_ref.clone(),
                base_ref: pr.base_ref.clone(),
                mergeable: pr.mergeable.clone(),
            }),
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

fn should_sync_branch_watchers(action: &Action) -> bool {
    matches!(
        action,
        Action::AppStateLoaded { .. }
            | Action::AddProject { .. }
            | Action::CreateWorkspace { .. }
            | Action::EnsureMainWorkspace { .. }
            | Action::WorkspaceCreated { .. }
            | Action::WorkspaceArchived { .. }
            | Action::DeleteProject { .. }
    )
}

fn conversation_key_for_action(action: &Action) -> Option<(WorkspaceId, WorkspaceThreadId)> {
    match action {
        Action::SendAgentMessage {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::QueueAgentMessage {
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
        Action::AgentRunStartedAt {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::AgentRunFinishedAt {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::AgentTurnFinished {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::CancelAgentTurn {
            workspace_id,
            thread_id,
        } => Some((*workspace_id, *thread_id)),
        Action::ChatModelChanged {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::ChatRunnerChanged {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::ChatAmpModeChanged {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::ThinkingEffortChanged {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::RemoveQueuedPrompt {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::ReorderQueuedPrompt {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::UpdateQueuedPrompt {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::ClearQueuedPrompts {
            workspace_id,
            thread_id,
        } => Some((*workspace_id, *thread_id)),
        Action::ResumeQueuedPrompts {
            workspace_id,
            thread_id,
        } => Some((*workspace_id, *thread_id)),
        _ => None,
    }
}

fn queue_state_key_for_action(action: &Action) -> Option<(WorkspaceId, WorkspaceThreadId)> {
    match action {
        Action::SendAgentMessage {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::QueueAgentMessage {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::RemoveQueuedPrompt {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::ReorderQueuedPrompt {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::UpdateQueuedPrompt {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::ClearQueuedPrompts {
            workspace_id,
            thread_id,
        } => Some((*workspace_id, *thread_id)),
        Action::ResumeQueuedPrompts {
            workspace_id,
            thread_id,
        } => Some((*workspace_id, *thread_id)),
        Action::CancelAgentTurn {
            workspace_id,
            thread_id,
        } => Some((*workspace_id, *thread_id)),
        Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id: _,
            event:
                CodexThreadEvent::TurnCompleted { .. }
                | CodexThreadEvent::TurnFailed { .. }
                | CodexThreadEvent::Error { .. },
        } => Some((*workspace_id, *thread_id)),
        Action::AgentRunStartedAt {
            workspace_id,
            thread_id,
            ..
        } => Some((*workspace_id, *thread_id)),
        Action::AgentRunFinishedAt {
            workspace_id,
            thread_id,
            ..
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

fn parse_codex_defaults_toml(contents: &str) -> (Option<String>, Option<ThinkingEffort>) {
    fn strip_comment(line: &str) -> &str {
        let mut in_single = false;
        let mut in_double = false;
        for (idx, ch) in line.char_indices() {
            match ch {
                '\'' if !in_double => in_single = !in_single,
                '"' if !in_single => in_double = !in_double,
                '#' if !in_single && !in_double => return &line[..idx],
                _ => {}
            }
        }
        line
    }

    fn parse_string_value(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(rest) = trimmed.strip_prefix('"') {
            let end = rest.find('"')?;
            return Some(rest[..end].to_owned());
        }
        if let Some(rest) = trimmed.strip_prefix('\'') {
            let end = rest.find('\'')?;
            return Some(rest[..end].to_owned());
        }
        None
    }

    fn parse_effort(raw: &str) -> Option<ThinkingEffort> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "minimal" => Some(ThinkingEffort::Minimal),
            "low" => Some(ThinkingEffort::Low),
            "medium" => Some(ThinkingEffort::Medium),
            "high" => Some(ThinkingEffort::High),
            "xhigh" => Some(ThinkingEffort::XHigh),
            _ => None,
        }
    }

    let mut in_root = true;
    let mut model_id: Option<String> = None;
    let mut effort: Option<ThinkingEffort> = None;

    for raw_line in contents.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_root = false;
            continue;
        }
        if !in_root {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        if key == "model" && model_id.is_none() {
            model_id = parse_string_value(value).map(|v| v.trim().to_owned());
            continue;
        }
        if key == "model_reasoning_effort" && effort.is_none() {
            if let Some(value) = parse_string_value(value) {
                effort = parse_effort(&value);
            }
            continue;
        }
    }

    (
        model_id.and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        }),
        effort,
    )
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

fn map_workspace_tabs_snapshot(tabs: &luban_domain::WorkspaceTabs) -> WorkspaceTabsSnapshot {
    WorkspaceTabsSnapshot {
        open_tabs: tabs
            .open_tabs
            .iter()
            .map(|id| luban_api::WorkspaceThreadId(id.as_u64()))
            .collect(),
        archived_tabs: tabs
            .archived_tabs
            .iter()
            .map(|id| luban_api::WorkspaceThreadId(id.as_u64()))
            .collect(),
        active_tab: luban_api::WorkspaceThreadId(tabs.active_tab.as_u64()),
    }
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
    let payload = serde_json::to_value(item).unwrap_or(serde_json::Value::Null);
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
            is_git: true,
        }),
        luban_api::ClientAction::AddProjectAndOpen { .. } => None,
        luban_api::ClientAction::TaskPreview { .. } => None,
        luban_api::ClientAction::TaskExecute { .. } => None,
        luban_api::ClientAction::TaskStarSet {
            workspace_id,
            thread_id,
            starred,
        } => Some(Action::TaskStarSet {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            starred,
        }),
        luban_api::ClientAction::FeedbackSubmit { .. } => None,
        luban_api::ClientAction::DeleteProject { .. } => None,
        luban_api::ClientAction::ToggleProjectExpanded { .. } => None,
        luban_api::ClientAction::CreateWorkspace { .. } => None,
        luban_api::ClientAction::OpenWorkspace { workspace_id } => Some(Action::OpenWorkspace {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
        }),
        luban_api::ClientAction::OpenWorkspaceInIde { workspace_id } => {
            Some(Action::OpenWorkspaceInIde {
                workspace_id: WorkspaceId::from_u64(workspace_id.0),
            })
        }
        luban_api::ClientAction::OpenWorkspaceWith {
            workspace_id,
            target,
        } => Some(Action::OpenWorkspaceWith {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            target: match target {
                luban_api::OpenTarget::Vscode => OpenTarget::Vscode,
                luban_api::OpenTarget::Cursor => OpenTarget::Cursor,
                luban_api::OpenTarget::Zed => OpenTarget::Zed,
                luban_api::OpenTarget::Ghostty => OpenTarget::Ghostty,
                luban_api::OpenTarget::Finder => OpenTarget::Finder,
            },
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
        luban_api::ClientAction::EnsureMainWorkspace { .. } => None,
        luban_api::ClientAction::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id,
        } => Some(Action::ChatModelChanged {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            model_id,
        }),
        luban_api::ClientAction::ChatRunnerChanged {
            workspace_id,
            thread_id,
            runner,
        } => Some(Action::ChatRunnerChanged {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            runner: map_api_agent_runner_kind(runner),
        }),
        luban_api::ClientAction::ChatAmpModeChanged {
            workspace_id,
            thread_id,
            amp_mode,
        } => Some(Action::ChatAmpModeChanged {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            amp_mode,
        }),
        luban_api::ClientAction::ThinkingEffortChanged {
            workspace_id,
            thread_id,
            thinking_effort,
        } => Some(Action::ThinkingEffortChanged {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            thinking_effort: match thinking_effort {
                luban_api::ThinkingEffort::Minimal => ThinkingEffort::Minimal,
                luban_api::ThinkingEffort::Low => ThinkingEffort::Low,
                luban_api::ThinkingEffort::Medium => ThinkingEffort::Medium,
                luban_api::ThinkingEffort::High => ThinkingEffort::High,
                luban_api::ThinkingEffort::XHigh => ThinkingEffort::XHigh,
            },
        }),
        luban_api::ClientAction::SendAgentMessage {
            workspace_id,
            thread_id,
            text,
            attachments,
            runner,
            amp_mode,
        } => Some(Action::SendAgentMessage {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            text,
            attachments: attachments.into_iter().map(map_api_attachment).collect(),
            runner: runner.map(map_api_agent_runner_kind),
            amp_mode,
        }),
        luban_api::ClientAction::CancelAndSendAgentMessage { .. } => None,
        luban_api::ClientAction::QueueAgentMessage {
            workspace_id,
            thread_id,
            text,
            attachments,
            runner,
            amp_mode,
        } => Some(Action::QueueAgentMessage {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            text,
            attachments: attachments.into_iter().map(map_api_attachment).collect(),
            runner: runner.map(map_api_agent_runner_kind),
            amp_mode,
        }),
        luban_api::ClientAction::RemoveQueuedPrompt {
            workspace_id,
            thread_id,
            prompt_id,
        } => Some(Action::RemoveQueuedPrompt {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            prompt_id,
        }),
        luban_api::ClientAction::ReorderQueuedPrompt {
            workspace_id,
            thread_id,
            active_id,
            over_id,
        } => Some(Action::ReorderQueuedPrompt {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            active_id,
            over_id,
        }),
        luban_api::ClientAction::UpdateQueuedPrompt {
            workspace_id,
            thread_id,
            prompt_id,
            text,
            attachments,
            model_id,
            thinking_effort,
        } => Some(Action::UpdateQueuedPrompt {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
            prompt_id,
            text,
            attachments: attachments.into_iter().map(map_api_attachment).collect(),
            model_id,
            thinking_effort: match thinking_effort {
                luban_api::ThinkingEffort::Minimal => ThinkingEffort::Minimal,
                luban_api::ThinkingEffort::Low => ThinkingEffort::Low,
                luban_api::ThinkingEffort::Medium => ThinkingEffort::Medium,
                luban_api::ThinkingEffort::High => ThinkingEffort::High,
                luban_api::ThinkingEffort::XHigh => ThinkingEffort::XHigh,
            },
        }),
        luban_api::ClientAction::WorkspaceRenameBranch {
            workspace_id,
            branch_name,
        } => Some(Action::WorkspaceBranchRenameRequested {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            requested_branch_name: branch_name,
        }),
        luban_api::ClientAction::WorkspaceAiRenameBranch {
            workspace_id,
            thread_id,
        } => Some(Action::WorkspaceBranchAiRenameRequested {
            workspace_id: WorkspaceId::from_u64(workspace_id.0),
            thread_id: WorkspaceThreadId::from_u64(thread_id.0),
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
        luban_api::ClientAction::OpenButtonSelectionChanged { selection } => {
            Some(Action::OpenButtonSelectionChanged { selection })
        }
        luban_api::ClientAction::SidebarProjectOrderChanged { project_ids } => {
            Some(Action::SidebarProjectOrderChanged {
                project_ids: project_ids.into_iter().map(|id| id.0).collect(),
            })
        }
        luban_api::ClientAction::AppearanceThemeChanged { theme } => {
            Some(Action::AppearanceThemeChanged {
                theme: match theme {
                    luban_api::AppearanceTheme::Light => luban_domain::AppearanceTheme::Light,
                    luban_api::AppearanceTheme::Dark => luban_domain::AppearanceTheme::Dark,
                    luban_api::AppearanceTheme::System => luban_domain::AppearanceTheme::System,
                },
            })
        }
        luban_api::ClientAction::AppearanceFontsChanged { fonts } => {
            Some(Action::AppearanceFontsChanged {
                ui_font: fonts.ui_font,
                chat_font: fonts.chat_font,
                code_font: fonts.code_font,
                terminal_font: fonts.terminal_font,
            })
        }
        luban_api::ClientAction::AppearanceGlobalZoomChanged { zoom } => {
            Some(Action::AppearanceGlobalZoomChanged { zoom })
        }
        luban_api::ClientAction::CodexEnabledChanged { enabled } => {
            Some(Action::AgentCodexEnabledChanged { enabled })
        }
        luban_api::ClientAction::AmpEnabledChanged { enabled } => {
            Some(Action::AgentAmpEnabledChanged { enabled })
        }
        luban_api::ClientAction::ClaudeEnabledChanged { enabled } => {
            Some(Action::AgentClaudeEnabledChanged { enabled })
        }
        luban_api::ClientAction::AgentRunnerChanged { runner } => {
            Some(Action::AgentRunnerChanged {
                runner: match runner {
                    luban_api::AgentRunnerKind::Codex => luban_domain::AgentRunnerKind::Codex,
                    luban_api::AgentRunnerKind::Amp => luban_domain::AgentRunnerKind::Amp,
                    luban_api::AgentRunnerKind::Claude => luban_domain::AgentRunnerKind::Claude,
                },
            })
        }
        luban_api::ClientAction::AgentAmpModeChanged { mode } => {
            Some(Action::AgentAmpModeChanged { mode })
        }
        luban_api::ClientAction::TaskPromptTemplateChanged {
            intent_kind,
            template,
        } => Some(Action::TaskPromptTemplateChanged {
            intent_kind: match intent_kind {
                luban_api::TaskIntentKind::Fix => luban_domain::TaskIntentKind::Fix,
                luban_api::TaskIntentKind::Implement => luban_domain::TaskIntentKind::Implement,
                luban_api::TaskIntentKind::Review => luban_domain::TaskIntentKind::Review,
                luban_api::TaskIntentKind::Discuss => luban_domain::TaskIntentKind::Discuss,
                luban_api::TaskIntentKind::Other => luban_domain::TaskIntentKind::Other,
            },
            template,
        }),
        luban_api::ClientAction::SystemPromptTemplateChanged { kind, template } => {
            Some(Action::SystemPromptTemplateChanged {
                kind: match kind {
                    luban_api::SystemTaskKind::InferType => luban_domain::SystemTaskKind::InferType,
                    luban_api::SystemTaskKind::RenameBranch => {
                        luban_domain::SystemTaskKind::RenameBranch
                    }
                    luban_api::SystemTaskKind::AutoTitleThread => {
                        luban_domain::SystemTaskKind::AutoTitleThread
                    }
                },
                template,
            })
        }
        luban_api::ClientAction::CodexCheck
        | luban_api::ClientAction::CodexConfigTree
        | luban_api::ClientAction::CodexConfigListDir { .. }
        | luban_api::ClientAction::CodexConfigReadFile { .. }
        | luban_api::ClientAction::CodexConfigWriteFile { .. }
        | luban_api::ClientAction::AmpCheck
        | luban_api::ClientAction::AmpConfigTree
        | luban_api::ClientAction::AmpConfigListDir { .. }
        | luban_api::ClientAction::AmpConfigReadFile { .. }
        | luban_api::ClientAction::AmpConfigWriteFile { .. }
        | luban_api::ClientAction::ClaudeCheck
        | luban_api::ClientAction::ClaudeConfigTree
        | luban_api::ClientAction::ClaudeConfigListDir { .. }
        | luban_api::ClientAction::ClaudeConfigReadFile { .. }
        | luban_api::ClientAction::ClaudeConfigWriteFile { .. } => None,
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

fn map_api_agent_runner_kind(kind: luban_api::AgentRunnerKind) -> luban_domain::AgentRunnerKind {
    match kind {
        luban_api::AgentRunnerKind::Codex => luban_domain::AgentRunnerKind::Codex,
        luban_api::AgentRunnerKind::Amp => luban_domain::AgentRunnerKind::Amp,
        luban_api::AgentRunnerKind::Claude => luban_domain::AgentRunnerKind::Claude,
    }
}

pub fn new_default_services() -> anyhow::Result<Arc<dyn ProjectWorkspaceService>> {
    Ok(GitWorkspaceService::new_with_options(SqliteStoreOptions {
        persist_ui_state: true,
    })
    .context("failed to init backend services")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use luban_domain::{
        CodexCommandExecutionStatus, ContextImage, ContextItem,
        ConversationSnapshot as DomainConversationSnapshot, ConversationThreadMeta,
        PersistedAppState, PersistedProject, PersistedWorkspace, WorkspaceStatus,
    };
    use std::collections::HashMap;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

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
            _branch_name_hint: Option<String>,
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

        fn rename_workspace_branch(
            &self,
            _worktree_path: PathBuf,
            _requested_branch_name: String,
        ) -> Result<String, String> {
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

        fn load_conversation_page(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _thread_id: u64,
            _before: Option<u64>,
            _limit: u64,
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

        fn record_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _attachment: AttachmentRef,
            _created_at_unix_ms: u64,
        ) -> Result<u64, String> {
            Err("unimplemented".to_owned())
        }

        fn list_context_items(
            &self,
            _project_slug: String,
            _workspace_name: String,
        ) -> Result<Vec<ContextItem>, String> {
            Err("unimplemented".to_owned())
        }

        fn delete_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _context_id: u64,
        ) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn run_agent_turn_streamed(
            &self,
            _request: luban_domain::RunAgentTurnRequest,
            _cancel: Arc<AtomicBool>,
            _on_event: Arc<dyn Fn(luban_domain::AgentThreadEvent) + Send + Sync>,
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

        fn task_preview(&self, _input: String) -> Result<luban_domain::TaskDraft, String> {
            Err("unimplemented".to_owned())
        }

        fn task_prepare_project(
            &self,
            _spec: luban_domain::TaskProjectSpec,
        ) -> Result<PathBuf, String> {
            Err("unimplemented".to_owned())
        }
    }

    struct IdentityServices;

    impl ProjectWorkspaceService for IdentityServices {
        fn load_app_state(&self) -> Result<PersistedAppState, String> {
            Ok(PersistedAppState {
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
                agent_default_thinking_effort: None,
                agent_default_runner: None,
                agent_amp_mode: None,
                agent_codex_enabled: Some(true),
                agent_amp_enabled: Some(true),
                agent_claude_enabled: Some(true),
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
            })
        }

        fn save_app_state(&self, _snapshot: PersistedAppState) -> Result<(), String> {
            Ok(())
        }

        fn create_workspace(
            &self,
            _project_path: PathBuf,
            _project_slug: String,
            _branch_name_hint: Option<String>,
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

        fn rename_workspace_branch(
            &self,
            _worktree_path: PathBuf,
            _requested_branch_name: String,
        ) -> Result<String, String> {
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

        fn load_conversation_page(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _thread_id: u64,
            _before: Option<u64>,
            _limit: u64,
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

        fn record_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _attachment: AttachmentRef,
            _created_at_unix_ms: u64,
        ) -> Result<u64, String> {
            Err("unimplemented".to_owned())
        }

        fn list_context_items(
            &self,
            _project_slug: String,
            _workspace_name: String,
        ) -> Result<Vec<ContextItem>, String> {
            Ok(Vec::new())
        }

        fn delete_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _context_id: u64,
        ) -> Result<(), String> {
            Ok(())
        }

        fn run_agent_turn_streamed(
            &self,
            _request: luban_domain::RunAgentTurnRequest,
            _cancel: Arc<AtomicBool>,
            _on_event: Arc<dyn Fn(luban_domain::AgentThreadEvent) + Send + Sync>,
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

        fn task_preview(&self, _input: String) -> Result<luban_domain::TaskDraft, String> {
            Err("unimplemented".to_owned())
        }

        fn task_prepare_project(
            &self,
            _spec: luban_domain::TaskProjectSpec,
        ) -> Result<PathBuf, String> {
            Err("unimplemented".to_owned())
        }

        fn project_identity(&self, path: PathBuf) -> Result<luban_domain::ProjectIdentity, String> {
            Ok(luban_domain::ProjectIdentity {
                root_path: path,
                github_repo: Some("github.com/example/repo".to_owned()),
                is_git: true,
            })
        }
    }

    #[test]
    fn app_snapshot_includes_pull_request_info() {
        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-test"),
            is_git: true,
        });

        let project_id = state.projects[0].id;
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "main".to_owned(),
            branch_name: "main".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban-server-test"),
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
            branch_watch: BranchWatchHandle::disabled(),
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
                next_refresh_at: Instant::now(),
                consecutive_empty: 0,
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

    #[test]
    fn app_snapshot_marks_merged_pull_requests() {
        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-test"),
            is_git: true,
        });

        let project_id = state.projects[0].id;
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "main".to_owned(),
            branch_name: "main".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban-server-test"),
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
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        engine.pull_requests.insert(
            workspace_id,
            PullRequestCacheEntry {
                info: Some(PullRequestInfo {
                    number: 7,
                    is_draft: false,
                    state: DomainPullRequestState::Merged,
                    ci_state: Some(DomainPullRequestCiState::Success),
                    merge_ready: false,
                }),
                next_refresh_at: Instant::now(),
                consecutive_empty: 0,
            },
        );

        let snapshot = engine.app_snapshot();
        let pr = snapshot.projects[0].workspaces[0].pull_request;
        assert_eq!(
            pr,
            Some(PullRequestSnapshot {
                number: 7,
                is_draft: false,
                state: PullRequestState::Merged,
                ci_state: Some(PullRequestCiState::Success),
                merge_ready: false,
            })
        );
    }

    #[test]
    fn pull_request_refresh_backoff_increases_on_empty_results() {
        let now = Instant::now();
        let workspace_id = WorkspaceId::from_u64(10);
        let previous = PullRequestCacheEntry {
            info: None,
            next_refresh_at: now,
            consecutive_empty: 1,
        };

        let (next, empty_count) =
            pull_request_next_refresh_at(workspace_id, now, Some(&previous), None);
        assert_eq!(empty_count, 2);
        let delta = next.duration_since(now);
        assert!(
            delta >= PULL_REQUEST_REFRESH_INTERVAL_EMPTY_MEDIUM,
            "expected at least {:?}, got {:?}",
            PULL_REQUEST_REFRESH_INTERVAL_EMPTY_MEDIUM,
            delta
        );
    }

    #[test]
    fn pull_request_refresh_pending_ci_is_frequently_refreshed() {
        let now = Instant::now();
        let workspace_id = WorkspaceId::from_u64(10);
        let info = PullRequestInfo {
            number: 1,
            is_draft: false,
            state: DomainPullRequestState::Open,
            ci_state: Some(DomainPullRequestCiState::Pending),
            merge_ready: false,
        };

        let (next, empty_count) =
            pull_request_next_refresh_at(workspace_id, now, None, Some(&info));
        assert_eq!(empty_count, 0);
        let delta = next.duration_since(now);
        assert!(
            delta >= PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_PENDING,
            "expected at least {:?}, got {:?}",
            PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_PENDING,
            delta
        );
        assert!(
            delta
                < PULL_REQUEST_REFRESH_INTERVAL_OPEN_CI_PENDING
                    + Duration::from_secs(PULL_REQUEST_REFRESH_JITTER_WINDOW_SECS + 1),
            "expected jitter window <= {:?}, got {:?}",
            Duration::from_secs(PULL_REQUEST_REFRESH_JITTER_WINDOW_SECS + 1),
            delta
        );
    }

    #[test]
    fn conversation_snapshots_are_truncated_to_tail() {
        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-test"),
            is_git: true,
        });

        let project_id = state.projects[0].id;
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "main".to_owned(),
            branch_name: "main".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban-server-test"),
        });

        let workspace_id = state.projects[0].workspaces[0].id;
        let thread_id = WorkspaceThreadId::from_u64(1);

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "seed".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        let key = (workspace_id, thread_id);
        let convo = state
            .conversations
            .get_mut(&key)
            .expect("conversation must exist");
        for i in 0..7000u32 {
            convo.entries.push(ConversationEntry::CodexItem {
                item: Box::new(CodexThreadItem::CommandExecution {
                    id: format!("cmd_{i}"),
                    command: format!("echo {i}"),
                    aggregated_output: String::new(),
                    exit_code: Some(0),
                    status: CodexCommandExecutionStatus::Completed,
                }),
            });
        }
        convo.entries_start = 0;
        convo.entries_total = convo.entries.len() as u64;
        convo.codex_item_ids = convo
            .entries
            .iter()
            .filter_map(|entry| match entry {
                ConversationEntry::CodexItem { item } => match item.as_ref() {
                    CodexThreadItem::CommandExecution { id, .. } => Some(id.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let total = convo.entries.len();

        let (events, _) = broadcast::channel::<WsServerMessage>(1);
        let (tx, _rx) = mpsc::channel::<EngineCommand>(1);
        let engine = Engine {
            state,
            rev: 1,
            services: Arc::new(TestServices),
            events,
            tx,
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        let api_wid = luban_api::WorkspaceId(workspace_id.as_u64());
        let api_tid = luban_api::WorkspaceThreadId(thread_id.as_u64());

        let snapshot = engine
            .conversation_snapshot(api_wid, api_tid, None, None)
            .expect("snapshot must build");
        assert!(
            snapshot.entries_truncated,
            "large conversations must be truncated"
        );
        assert_eq!(snapshot.entries_total, total as u64);
        assert_eq!(
            snapshot.entries_start + snapshot.entries.len() as u64,
            snapshot.entries_total
        );
        assert!(snapshot.entries.len() <= 2000);
    }

    #[test]
    fn default_services_persist_ui_state() {
        static ENV_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .expect("mutex poisoned");

        struct EnvGuard {
            prev_root: Option<std::ffi::OsString>,
            root: PathBuf,
        }

        impl Drop for EnvGuard {
            fn drop(&mut self) {
                if let Some(prev) = self.prev_root.take() {
                    unsafe {
                        std::env::set_var(luban_domain::paths::LUBAN_ROOT_ENV, prev);
                    }
                } else {
                    unsafe {
                        std::env::remove_var(luban_domain::paths::LUBAN_ROOT_ENV);
                    }
                }
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }

        let root = std::env::temp_dir().join(format!(
            "luban-tests-default-services-persist-ui-state-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");

        let env_guard = EnvGuard {
            prev_root: std::env::var_os(luban_domain::paths::LUBAN_ROOT_ENV),
            root: root.clone(),
        };
        unsafe {
            std::env::set_var(luban_domain::paths::LUBAN_ROOT_ENV, root.as_os_str());
        }

        let services = new_default_services().expect("init services");

        let snapshot = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                slug: "p".to_owned(),
                name: "P".to_owned(),
                path: PathBuf::from("/tmp/p"),
                is_git: true,
                expanded: false,
                workspaces: vec![PersistedWorkspace {
                    id: 10,
                    workspace_name: "main".to_owned(),
                    branch_name: "main".to_owned(),
                    worktree_path: PathBuf::from("/tmp/p"),
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
            agent_claude_enabled: Some(true),
            last_open_workspace_id: Some(10),
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            workspace_active_thread_id: HashMap::from([(10, 2)]),
            workspace_open_tabs: HashMap::from([(10, vec![1, 2])]),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::from([(10, 3)]),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            starred_tasks: HashMap::new(),
            task_prompt_templates: HashMap::new(),
        };

        services
            .save_app_state(snapshot.clone())
            .expect("save app state");
        let loaded = services.load_app_state().expect("load app state");

        assert_eq!(loaded.workspace_open_tabs.get(&10), Some(&vec![1, 2]));
        assert_eq!(loaded.workspace_next_thread_id.get(&10), Some(&3));
        assert_eq!(loaded.workspace_active_thread_id.get(&10), Some(&2));
        drop(env_guard);
    }

    #[test]
    fn workspace_threads_changed_includes_tabs_snapshot() {
        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-test"),
            is_git: true,
        });

        let project_id = state.projects[0].id;
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "main".to_owned(),
            branch_name: "main".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban-server-test"),
        });

        let workspace_id = state.projects[0].workspaces[0].id;
        state.apply(Action::OpenWorkspace { workspace_id });

        state.apply(Action::CreateWorkspaceThread { workspace_id });
        state.apply(Action::CreateWorkspaceThread { workspace_id });

        let open_tabs = state
            .workspace_tabs(workspace_id)
            .expect("workspace tabs exist after opening workspace")
            .open_tabs
            .clone();

        let archived_thread = open_tabs[0];
        state.apply(Action::CloseWorkspaceThreadTab {
            workspace_id,
            thread_id: archived_thread,
        });

        let tabs = state.workspace_tabs(workspace_id).unwrap();
        assert!(tabs.archived_tabs.contains(&archived_thread));

        let mut meta_ids = Vec::new();
        meta_ids.extend(tabs.open_tabs.iter().copied());
        meta_ids.extend(tabs.archived_tabs.iter().copied());
        let metas = meta_ids
            .iter()
            .map(|id| ConversationThreadMeta {
                thread_id: *id,
                remote_thread_id: None,
                title: format!("thread-{}", id.as_u64()),
                updated_at_unix_seconds: 0,
            })
            .collect::<Vec<_>>();

        let (events, _) = broadcast::channel::<WsServerMessage>(4);
        let mut rx = events.subscribe();
        let (tx, _rx_cmd) = mpsc::channel::<EngineCommand>(1);
        let engine = Engine {
            state,
            rev: 1,
            services: Arc::new(TestServices),
            events,
            tx,
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        engine.publish_threads_event(workspace_id, &metas);

        let message = rx.try_recv().expect("expected a threads event");
        let WsServerMessage::Event { event, .. } = message else {
            panic!("expected WsServerMessage::Event");
        };

        let luban_api::ServerEvent::WorkspaceThreadsChanged {
            workspace_id: wid,
            tabs,
            ..
        } = *event
        else {
            panic!("expected workspace_threads_changed");
        };

        assert_eq!(wid.0, workspace_id.as_u64());
        assert_eq!(
            tabs.open_tabs.len() + tabs.archived_tabs.len(),
            metas.len(),
            "tabs snapshot should match the set of known thread ids"
        );
        assert!(
            tabs.archived_tabs
                .iter()
                .any(|id| id.0 == archived_thread.as_u64())
        );
    }

    #[tokio::test]
    async fn add_project_reuses_existing_by_github_repo() {
        let (engine, _events) = Engine::start(Arc::new(IdentityServices));
        engine
            .apply_client_action(
                "req-1".to_owned(),
                luban_api::ClientAction::AddProject {
                    path: "/tmp/repo-a".to_owned(),
                },
            )
            .await
            .expect("add first project should succeed");
        engine
            .apply_client_action(
                "req-2".to_owned(),
                luban_api::ClientAction::AddProject {
                    path: "/tmp/repo-b".to_owned(),
                },
            )
            .await
            .expect("add second project should be reused");

        let snapshot = engine.app_snapshot().await.expect("snapshot should work");
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.projects[0].path, "/tmp/repo-a");
    }

    struct ArchiveOkServices {
        calls: Arc<std::sync::Mutex<Vec<(PathBuf, PathBuf)>>>,
        cancel_flag: Option<Arc<AtomicBool>>,
    }

    impl ProjectWorkspaceService for ArchiveOkServices {
        fn load_app_state(&self) -> Result<PersistedAppState, String> {
            Ok(PersistedAppState {
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
                agent_default_thinking_effort: None,
                agent_default_runner: None,
                agent_amp_mode: None,
                agent_codex_enabled: Some(true),
                agent_amp_enabled: Some(true),
                agent_claude_enabled: Some(true),
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
            })
        }

        fn save_app_state(&self, _snapshot: PersistedAppState) -> Result<(), String> {
            Ok(())
        }

        fn create_workspace(
            &self,
            _project_path: PathBuf,
            _project_slug: String,
            _branch_name_hint: Option<String>,
        ) -> Result<luban_domain::CreatedWorkspace, String> {
            Err("unimplemented".to_owned())
        }

        fn open_workspace_in_ide(&self, _worktree_path: PathBuf) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn archive_workspace(
            &self,
            project_path: PathBuf,
            worktree_path: PathBuf,
        ) -> Result<(), String> {
            if let Some(cancel_flag) = &self.cancel_flag
                && !cancel_flag.load(Ordering::SeqCst)
            {
                return Err("archive workspace called before agent cancel".to_owned());
            }
            self.calls
                .lock()
                .expect("mutex poisoned")
                .push((project_path, worktree_path));
            Ok(())
        }

        fn rename_workspace_branch(
            &self,
            _worktree_path: PathBuf,
            _requested_branch_name: String,
        ) -> Result<String, String> {
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

        fn load_conversation_page(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _thread_id: u64,
            _before: Option<u64>,
            _limit: u64,
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

        fn record_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _attachment: AttachmentRef,
            _created_at_unix_ms: u64,
        ) -> Result<u64, String> {
            Err("unimplemented".to_owned())
        }

        fn list_context_items(
            &self,
            _project_slug: String,
            _workspace_name: String,
        ) -> Result<Vec<ContextItem>, String> {
            Ok(Vec::new())
        }

        fn delete_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _context_id: u64,
        ) -> Result<(), String> {
            Ok(())
        }

        fn run_agent_turn_streamed(
            &self,
            _request: luban_domain::RunAgentTurnRequest,
            _cancel: Arc<AtomicBool>,
            _on_event: Arc<dyn Fn(luban_domain::AgentThreadEvent) + Send + Sync>,
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

        fn task_preview(&self, _input: String) -> Result<luban_domain::TaskDraft, String> {
            Err("unimplemented".to_owned())
        }

        fn task_prepare_project(
            &self,
            _spec: luban_domain::TaskProjectSpec,
        ) -> Result<PathBuf, String> {
            Err("unimplemented".to_owned())
        }

        fn project_identity(
            &self,
            _path: PathBuf,
        ) -> Result<luban_domain::ProjectIdentity, String> {
            Err("unimplemented".to_owned())
        }
    }

    #[tokio::test]
    async fn archive_workspace_runs_effect_and_marks_archived() {
        let calls = Arc::new(std::sync::Mutex::new(Vec::<(PathBuf, PathBuf)>::new()));
        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(ArchiveOkServices {
            calls: calls.clone(),
            cancel_flag: None,
        });

        let mut state = AppState::new();
        let project_path = PathBuf::from("/tmp/luban-server-archive-test");
        let _ = state.apply(Action::AddProject {
            path: project_path.clone(),
            is_git: true,
        });
        let project_id = state.projects[0].id;

        let worktree_path = PathBuf::from("/tmp/luban-server-archive-test-wt");
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "wt".to_owned(),
            branch_name: "feature".to_owned(),
            worktree_path: worktree_path.clone(),
        });

        let workspace_id = state
            .projects
            .iter()
            .flat_map(|p| p.workspaces.iter())
            .find(|w| w.worktree_path == worktree_path)
            .expect("workspace should exist")
            .id;

        let (events, _) = broadcast::channel::<WsServerMessage>(16);
        let (tx, _rx) = mpsc::channel::<EngineCommand>(16);
        let mut engine = Engine {
            state,
            rev: 1,
            services,
            events,
            tx,
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        engine
            .process_action_queue(Action::ArchiveWorkspace { workspace_id })
            .await;

        let workspace = engine
            .state
            .workspace(workspace_id)
            .expect("workspace should still exist after archive");
        assert_eq!(workspace.status, luban_domain::WorkspaceStatus::Archived);
        assert_eq!(engine.state.main_pane, luban_domain::MainPane::None);
        assert_eq!(engine.state.right_pane, luban_domain::RightPane::None);

        let calls = calls.lock().expect("mutex poisoned");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, project_path);
        assert_eq!(calls[0].1, worktree_path);
    }

    #[tokio::test]
    async fn archive_workspace_cancels_agent_turns_before_archiving() {
        let calls = Arc::new(std::sync::Mutex::new(Vec::<(PathBuf, PathBuf)>::new()));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(ArchiveOkServices {
            calls: calls.clone(),
            cancel_flag: Some(cancel_flag.clone()),
        });

        let mut state = AppState::new();
        let project_path = PathBuf::from("/tmp/luban-server-archive-cancel-test");
        let _ = state.apply(Action::AddProject {
            path: project_path.clone(),
            is_git: true,
        });
        let project_id = state.projects[0].id;

        let worktree_path = PathBuf::from("/tmp/luban-server-archive-cancel-test-wt");
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "wt".to_owned(),
            branch_name: "feature".to_owned(),
            worktree_path: worktree_path.clone(),
        });

        let workspace_id = state
            .projects
            .iter()
            .flat_map(|p| p.workspaces.iter())
            .find(|w| w.worktree_path == worktree_path)
            .expect("workspace should exist")
            .id;

        let thread_id = state
            .workspace_tabs
            .get(&workspace_id)
            .expect("workspace tabs should exist")
            .active_tab;

        let run_id = 7u64;
        {
            let conversation = state
                .conversations
                .get_mut(&(workspace_id, thread_id))
                .expect("conversation should exist");
            conversation.run_status = OperationStatus::Running;
            conversation.active_run_id = Some(run_id);
        }

        let (events, _) = broadcast::channel::<WsServerMessage>(16);
        let (tx, _rx) = mpsc::channel::<EngineCommand>(16);
        let mut engine = Engine {
            state,
            rev: 1,
            services,
            events,
            tx,
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::from([(
                (workspace_id, thread_id),
                CancelFlagEntry {
                    run_id,
                    flag: cancel_flag.clone(),
                },
            )]),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        engine
            .process_action_queue(Action::ArchiveWorkspace { workspace_id })
            .await;

        assert!(cancel_flag.load(Ordering::SeqCst));

        let calls = calls.lock().expect("mutex poisoned");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, project_path);
        assert_eq!(calls[0].1, worktree_path);
    }

    struct OpenInIdeServices {
        opened: Arc<std::sync::Mutex<Vec<PathBuf>>>,
        opened_with: Arc<std::sync::Mutex<Vec<(PathBuf, OpenTarget)>>>,
    }

    impl ProjectWorkspaceService for OpenInIdeServices {
        fn load_app_state(&self) -> Result<PersistedAppState, String> {
            Ok(PersistedAppState {
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
                agent_default_thinking_effort: None,
                agent_default_runner: None,
                agent_amp_mode: None,
                agent_codex_enabled: Some(true),
                agent_amp_enabled: Some(true),
                agent_claude_enabled: Some(true),
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
            })
        }

        fn save_app_state(&self, _snapshot: PersistedAppState) -> Result<(), String> {
            Ok(())
        }

        fn create_workspace(
            &self,
            _project_path: PathBuf,
            _project_slug: String,
            _branch_name_hint: Option<String>,
        ) -> Result<luban_domain::CreatedWorkspace, String> {
            Err("unimplemented".to_owned())
        }

        fn open_workspace_in_ide(&self, worktree_path: PathBuf) -> Result<(), String> {
            self.opened
                .lock()
                .expect("mutex poisoned")
                .push(worktree_path);
            Ok(())
        }

        fn open_workspace_with(
            &self,
            worktree_path: PathBuf,
            target: OpenTarget,
        ) -> Result<(), String> {
            self.opened_with
                .lock()
                .expect("mutex poisoned")
                .push((worktree_path, target));
            Ok(())
        }

        fn archive_workspace(
            &self,
            _project_path: PathBuf,
            _worktree_path: PathBuf,
        ) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn rename_workspace_branch(
            &self,
            _worktree_path: PathBuf,
            _requested_branch_name: String,
        ) -> Result<String, String> {
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

        fn load_conversation_page(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _thread_id: u64,
            _before: Option<u64>,
            _limit: u64,
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

        fn record_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _attachment: AttachmentRef,
            _created_at_unix_ms: u64,
        ) -> Result<u64, String> {
            Err("unimplemented".to_owned())
        }

        fn list_context_items(
            &self,
            _project_slug: String,
            _workspace_name: String,
        ) -> Result<Vec<ContextItem>, String> {
            Ok(Vec::new())
        }

        fn delete_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _context_id: u64,
        ) -> Result<(), String> {
            Ok(())
        }

        fn run_agent_turn_streamed(
            &self,
            _request: luban_domain::RunAgentTurnRequest,
            _cancel: Arc<AtomicBool>,
            _on_event: Arc<dyn Fn(luban_domain::AgentThreadEvent) + Send + Sync>,
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

        fn task_preview(&self, _input: String) -> Result<luban_domain::TaskDraft, String> {
            Err("unimplemented".to_owned())
        }

        fn task_prepare_project(
            &self,
            _spec: luban_domain::TaskProjectSpec,
        ) -> Result<PathBuf, String> {
            Err("unimplemented".to_owned())
        }

        fn project_identity(
            &self,
            _path: PathBuf,
        ) -> Result<luban_domain::ProjectIdentity, String> {
            Err("unimplemented".to_owned())
        }
    }

    #[tokio::test]
    async fn open_workspace_in_ide_runs_effect() {
        let opened = Arc::new(std::sync::Mutex::new(Vec::<PathBuf>::new()));
        let opened_with = Arc::new(std::sync::Mutex::new(Vec::<(PathBuf, OpenTarget)>::new()));
        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(OpenInIdeServices {
            opened: opened.clone(),
            opened_with: opened_with.clone(),
        });

        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-open-ide-test"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "main".to_owned(),
            branch_name: "main".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban-server-open-ide-test"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        let worktree_path = state.projects[0].workspaces[0].worktree_path.clone();

        let (events, _) = broadcast::channel::<WsServerMessage>(16);
        let (tx, _rx) = mpsc::channel::<EngineCommand>(16);
        let mut engine = Engine {
            state,
            rev: 1,
            services,
            events,
            tx,
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        engine
            .process_action_queue(Action::OpenWorkspaceInIde { workspace_id })
            .await;

        let opened = opened.lock().expect("mutex poisoned");
        assert_eq!(opened.as_slice(), &[worktree_path]);
    }

    #[tokio::test]
    async fn open_workspace_with_runs_effect() {
        let opened = Arc::new(std::sync::Mutex::new(Vec::<PathBuf>::new()));
        let opened_with = Arc::new(std::sync::Mutex::new(Vec::<(PathBuf, OpenTarget)>::new()));
        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(OpenInIdeServices {
            opened: opened.clone(),
            opened_with: opened_with.clone(),
        });

        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-open-with-test"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "main".to_owned(),
            branch_name: "main".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban-server-open-with-test"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        let worktree_path = state.projects[0].workspaces[0].worktree_path.clone();

        let (events, _) = broadcast::channel::<WsServerMessage>(16);
        let (tx, _rx) = mpsc::channel::<EngineCommand>(16);
        let mut engine = Engine {
            state,
            rev: 1,
            services,
            events,
            tx,
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        engine
            .process_action_queue(Action::OpenWorkspaceWith {
                workspace_id,
                target: OpenTarget::Vscode,
            })
            .await;

        let opened_with = opened_with.lock().expect("mutex poisoned");
        assert_eq!(
            opened_with.as_slice(),
            &[(worktree_path, OpenTarget::Vscode)]
        );
    }

    struct CaptureRunAgentTurnServices {
        sender: std::sync::mpsc::Sender<luban_domain::RunAgentTurnRequest>,
    }

    impl ProjectWorkspaceService for CaptureRunAgentTurnServices {
        fn load_app_state(&self) -> Result<PersistedAppState, String> {
            Ok(PersistedAppState {
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
                agent_default_thinking_effort: None,
                agent_default_runner: None,
                agent_amp_mode: None,
                agent_codex_enabled: Some(true),
                agent_amp_enabled: Some(true),
                agent_claude_enabled: Some(true),
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
            })
        }

        fn save_app_state(&self, _snapshot: PersistedAppState) -> Result<(), String> {
            Ok(())
        }

        fn create_workspace(
            &self,
            _project_path: PathBuf,
            _project_slug: String,
            _branch_name_hint: Option<String>,
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

        fn rename_workspace_branch(
            &self,
            _worktree_path: PathBuf,
            _requested_branch_name: String,
        ) -> Result<String, String> {
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

        fn load_conversation_page(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _thread_id: u64,
            _before: Option<u64>,
            _limit: u64,
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

        fn record_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _attachment: AttachmentRef,
            _created_at_unix_ms: u64,
        ) -> Result<u64, String> {
            Err("unimplemented".to_owned())
        }

        fn list_context_items(
            &self,
            _project_slug: String,
            _workspace_name: String,
        ) -> Result<Vec<ContextItem>, String> {
            Ok(Vec::new())
        }

        fn delete_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _context_id: u64,
        ) -> Result<(), String> {
            Ok(())
        }

        fn run_agent_turn_streamed(
            &self,
            request: luban_domain::RunAgentTurnRequest,
            _cancel: Arc<AtomicBool>,
            _on_event: Arc<dyn Fn(luban_domain::AgentThreadEvent) + Send + Sync>,
        ) -> Result<(), String> {
            let _ = self.sender.send(request);
            Ok(())
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

        fn task_preview(&self, _input: String) -> Result<luban_domain::TaskDraft, String> {
            Err("unimplemented".to_owned())
        }

        fn task_prepare_project(
            &self,
            _spec: luban_domain::TaskProjectSpec,
        ) -> Result<PathBuf, String> {
            Err("unimplemented".to_owned())
        }

        fn project_identity(
            &self,
            _path: PathBuf,
        ) -> Result<luban_domain::ProjectIdentity, String> {
            Err("unimplemented".to_owned())
        }
    }

    struct SlowRenameServices {
        delay: Duration,
    }

    impl ProjectWorkspaceService for SlowRenameServices {
        fn load_app_state(&self) -> Result<PersistedAppState, String> {
            Ok(PersistedAppState {
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
                agent_default_thinking_effort: None,
                agent_default_runner: None,
                agent_amp_mode: None,
                agent_codex_enabled: Some(true),
                agent_amp_enabled: Some(true),
                agent_claude_enabled: Some(true),
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
            })
        }

        fn save_app_state(&self, _snapshot: PersistedAppState) -> Result<(), String> {
            Ok(())
        }

        fn create_workspace(
            &self,
            _project_path: PathBuf,
            _project_slug: String,
            _branch_name_hint: Option<String>,
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

        fn rename_workspace_branch(
            &self,
            _worktree_path: PathBuf,
            requested_branch_name: String,
        ) -> Result<String, String> {
            std::thread::sleep(self.delay);
            Ok(requested_branch_name)
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

        fn load_conversation_page(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _thread_id: u64,
            _before: Option<u64>,
            _limit: u64,
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

        fn record_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _attachment: AttachmentRef,
            _created_at_unix_ms: u64,
        ) -> Result<u64, String> {
            Err("unimplemented".to_owned())
        }

        fn list_context_items(
            &self,
            _project_slug: String,
            _workspace_name: String,
        ) -> Result<Vec<ContextItem>, String> {
            Err("unimplemented".to_owned())
        }

        fn delete_context_item(
            &self,
            _project_slug: String,
            _workspace_name: String,
            _context_id: u64,
        ) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn run_agent_turn_streamed(
            &self,
            _request: luban_domain::RunAgentTurnRequest,
            _cancel: Arc<AtomicBool>,
            _on_event: Arc<dyn Fn(luban_domain::AgentThreadEvent) + Send + Sync>,
        ) -> Result<(), String> {
            Err("unimplemented".to_owned())
        }

        fn task_preview(&self, _input: String) -> Result<luban_domain::TaskDraft, String> {
            Err("unimplemented".to_owned())
        }

        fn task_prepare_project(
            &self,
            _spec: luban_domain::TaskProjectSpec,
        ) -> Result<PathBuf, String> {
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

        fn project_identity(
            &self,
            _path: PathBuf,
        ) -> Result<luban_domain::ProjectIdentity, String> {
            Err("unimplemented".to_owned())
        }
    }

    #[tokio::test]
    async fn workspace_branch_rename_does_not_block_engine() {
        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(SlowRenameServices {
            delay: Duration::from_secs(2),
        });

        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-rename-test"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "luban/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban-server-rename-test"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;

        let (events, _) = broadcast::channel::<WsServerMessage>(16);
        let (tx, mut rx) = mpsc::channel::<EngineCommand>(16);
        let mut engine = Engine {
            state,
            rev: 1,
            services,
            events,
            tx: tx.clone(),
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        let rename = tokio::time::timeout(
            Duration::from_millis(200),
            engine.process_action_queue(Action::WorkspaceBranchRenameRequested {
                workspace_id,
                requested_branch_name: "luban/rename-test".to_owned(),
            }),
        )
        .await;
        assert!(rename.is_ok(), "rename action should not block");

        // Drain the dispatch action so the spawned task does not leak.
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(cmd) = rx.recv().await {
                if let EngineCommand::DispatchAction { action } = cmd {
                    engine.process_action_queue(*action).await;
                    break;
                }
            }
        })
        .await;
    }

    #[tokio::test]
    async fn agent_turn_does_not_override_codex_defaults() {
        let (sender, receiver) = std::sync::mpsc::channel::<luban_domain::RunAgentTurnRequest>();
        let services: Arc<dyn ProjectWorkspaceService> =
            Arc::new(CaptureRunAgentTurnServices { sender });

        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-agent-turn-test"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "main".to_owned(),
            branch_name: "main".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban-server-agent-turn-test"),
        });

        let workspace_id = state.projects[0].workspaces[0].id;
        let thread_id = WorkspaceThreadId::from_u64(1);

        let _ = state.apply(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: "not-a-real-model".to_owned(),
        });

        let (events, _) = broadcast::channel::<WsServerMessage>(16);
        let (tx, _rx) = mpsc::channel::<EngineCommand>(16);
        let mut engine = Engine {
            state,
            rev: 1,
            services,
            events,
            tx,
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        engine
            .process_action_queue(Action::SendAgentMessage {
                workspace_id,
                thread_id,
                text: "hello".to_owned(),
                attachments: Vec::new(),
                runner: None,
                amp_mode: None,
            })
            .await;

        let request = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("expected agent turn request");

        assert_eq!(request.runner, luban_domain::AgentRunnerKind::Codex);
        assert!(request.amp_mode.is_none());
        assert_eq!(request.model.as_deref(), Some("not-a-real-model"));
        assert_eq!(request.model_reasoning_effort.as_deref(), Some("medium"));
    }

    #[tokio::test]
    async fn agent_turn_uses_pinned_chat_runner_and_amp_mode() {
        let (sender, receiver) = std::sync::mpsc::channel::<luban_domain::RunAgentTurnRequest>();
        let services: Arc<dyn ProjectWorkspaceService> =
            Arc::new(CaptureRunAgentTurnServices { sender });

        let mut state = AppState::new();
        let _ = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/luban-server-pinned-run-config-test"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        let _ = state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "main".to_owned(),
            branch_name: "main".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban-server-pinned-run-config-test"),
        });

        let workspace_id = state.projects[0].workspaces[0].id;
        let thread_id = WorkspaceThreadId::from_u64(1);

        let (events, _) = broadcast::channel::<WsServerMessage>(16);
        let (tx, _rx) = mpsc::channel::<EngineCommand>(16);
        let mut engine = Engine {
            state,
            rev: 1,
            services,
            events,
            tx,
            branch_watch: BranchWatchHandle::disabled(),
            cancel_flags: HashMap::new(),
            pull_requests: HashMap::new(),
            pull_requests_in_flight: HashSet::new(),
        };

        engine
            .process_action_queue(Action::ChatRunnerChanged {
                workspace_id,
                thread_id,
                runner: luban_domain::AgentRunnerKind::Amp,
            })
            .await;

        engine
            .process_action_queue(Action::ChatAmpModeChanged {
                workspace_id,
                thread_id,
                amp_mode: "rush".to_owned(),
            })
            .await;

        engine
            .process_action_queue(Action::SendAgentMessage {
                workspace_id,
                thread_id,
                text: "hello".to_owned(),
                attachments: Vec::new(),
                runner: None,
                amp_mode: None,
            })
            .await;

        let request = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("expected agent turn request");

        assert_eq!(request.runner, luban_domain::AgentRunnerKind::Amp);
        assert_eq!(request.amp_mode.as_deref(), Some("rush"));
    }
}
