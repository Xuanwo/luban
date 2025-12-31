use gpui::prelude::*;
use gpui::{
    AnyElement, Context, ElementId, IntoElement, MouseButton, Pixels, PromptButton, PromptLevel,
    SharedString, Window, div, px, rems,
};
use gpui_component::input::RopeExt as _;
use gpui_component::{
    ActiveTheme as _, Disableable as _, ElementExt as _, Icon, IconName, IconNamed as _,
    Sizable as _, Size, StyledExt as _,
    button::*,
    collapsible::Collapsible,
    input::{Input, InputEvent, InputState},
    spinner::Spinner,
    text::{TextView, TextViewStyle},
};
use luban_domain::{
    Action, AppState, CodexThreadEvent, CodexThreadItem, ConversationSnapshot, Effect, MainPane,
    OperationStatus, PersistedAppState, ProjectId, WorkspaceId, WorkspaceStatus,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub struct CreatedWorkspace {
    pub workspace_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct RunAgentTurnRequest {
    pub project_slug: String,
    pub workspace_name: String,
    pub worktree_path: PathBuf,
    pub thread_id: Option<String>,
    pub prompt: String,
}

pub trait ProjectWorkspaceService: Send + Sync {
    fn load_app_state(&self) -> Result<PersistedAppState, String>;

    fn save_app_state(&self, snapshot: PersistedAppState) -> Result<(), String>;

    fn create_workspace(
        &self,
        project_path: PathBuf,
        project_slug: String,
    ) -> Result<CreatedWorkspace, String>;

    fn archive_workspace(
        &self,
        project_path: PathBuf,
        worktree_path: PathBuf,
    ) -> Result<(), String>;

    fn ensure_conversation(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> Result<(), String>;

    fn load_conversation(
        &self,
        project_slug: String,
        workspace_name: String,
    ) -> Result<ConversationSnapshot, String>;

    fn run_agent_turn_streamed(
        &self,
        request: RunAgentTurnRequest,
        cancel: Arc<AtomicBool>,
        on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync>,
    ) -> Result<(), String>;
}

pub struct LubanRootView {
    state: AppState,
    services: Arc<dyn ProjectWorkspaceService>,
    chat_input: Option<gpui::Entity<InputState>>,
    expanded_agent_items: HashSet<String>,
    expanded_agent_turns: HashSet<String>,
    chat_column_width: Option<Pixels>,
    running_turn_started_at: HashMap<WorkspaceId, Instant>,
    running_turn_tickers: HashSet<WorkspaceId>,
    turn_generation: HashMap<WorkspaceId, u64>,
    turn_cancel_flags: HashMap<WorkspaceId, Arc<AtomicBool>>,
    chat_scroll_handle: gpui::ScrollHandle,
    last_chat_workspace_id: Option<WorkspaceId>,
    last_chat_item_count: usize,
    _subscriptions: Vec<gpui::Subscription>,
}

impl LubanRootView {
    pub fn new(services: Arc<dyn ProjectWorkspaceService>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            state: AppState::new(),
            services,
            chat_input: None,
            expanded_agent_items: HashSet::new(),
            expanded_agent_turns: HashSet::new(),
            chat_column_width: None,
            running_turn_started_at: HashMap::new(),
            running_turn_tickers: HashSet::new(),
            turn_generation: HashMap::new(),
            turn_cancel_flags: HashMap::new(),
            chat_scroll_handle: gpui::ScrollHandle::new(),
            last_chat_workspace_id: None,
            last_chat_item_count: 0,
            _subscriptions: Vec::new(),
        };

        this.dispatch(Action::AppStarted, cx);
        this
    }

    #[cfg(test)]
    pub fn with_state(
        services: Arc<dyn ProjectWorkspaceService>,
        state: AppState,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            state,
            services,
            chat_input: None,
            expanded_agent_items: HashSet::new(),
            expanded_agent_turns: HashSet::new(),
            chat_column_width: None,
            running_turn_started_at: HashMap::new(),
            running_turn_tickers: HashSet::new(),
            turn_generation: HashMap::new(),
            turn_cancel_flags: HashMap::new(),
            chat_scroll_handle: gpui::ScrollHandle::new(),
            last_chat_workspace_id: None,
            last_chat_item_count: 0,
            _subscriptions: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn debug_state(&self) -> &AppState {
        &self.state
    }

    fn dispatch(&mut self, action: Action, cx: &mut Context<Self>) {
        let start_timer_workspace = match &action {
            Action::SendAgentMessage { workspace_id, .. } => Some(*workspace_id),
            _ => None,
        };
        let stop_timer_workspace = match &action {
            Action::AgentEventReceived {
                workspace_id,
                event:
                    CodexThreadEvent::TurnCompleted { .. }
                    | CodexThreadEvent::TurnFailed { .. }
                    | CodexThreadEvent::Error { .. },
            }
            | Action::AgentTurnFinished { workspace_id } => Some(*workspace_id),
            Action::CancelAgentTurn { workspace_id } => Some(*workspace_id),
            _ => None,
        };

        let effects = self.state.apply(action);
        cx.notify();

        if let Some(workspace_id) = start_timer_workspace {
            let is_running = self
                .state
                .workspace_conversation(workspace_id)
                .map(|c| c.run_status == OperationStatus::Running)
                .unwrap_or(false);
            if is_running {
                self.ensure_running_turn_timer(workspace_id, cx);
            }
        }

        if let Some(workspace_id) = stop_timer_workspace {
            self.running_turn_started_at.remove(&workspace_id);
            self.running_turn_tickers.remove(&workspace_id);
        }

        for effect in effects {
            self.run_effect(effect, cx);
        }
    }

    fn bump_turn_generation(&mut self, workspace_id: WorkspaceId) -> u64 {
        let entry = self.turn_generation.entry(workspace_id).or_insert(0);
        *entry += 1;
        *entry
    }

    fn toggle_agent_turn_expanded(&mut self, id: &str) {
        if self.expanded_agent_turns.contains(id) {
            self.expanded_agent_turns.remove(id);
        } else {
            self.expanded_agent_turns.insert(id.to_owned());
            let prefix = format!("{id}::");
            self.expanded_agent_items
                .retain(|item_id| !item_id.starts_with(&prefix));
        }
    }

    fn ensure_running_turn_timer(&mut self, workspace_id: WorkspaceId, cx: &mut Context<Self>) {
        self.running_turn_started_at
            .entry(workspace_id)
            .or_insert_with(Instant::now);
        if !self.running_turn_tickers.insert(workspace_id) {
            return;
        }

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    loop {
                        gpui::Timer::after(Duration::from_secs(1)).await;

                        let still_running = this
                            .update(&mut async_cx, |view: &mut LubanRootView, view_cx| {
                                let running = view
                                    .state
                                    .workspace_conversation(workspace_id)
                                    .map(|c| c.run_status == OperationStatus::Running)
                                    .unwrap_or(false);
                                if running {
                                    view_cx.notify();
                                } else {
                                    view.running_turn_started_at.remove(&workspace_id);
                                    view.running_turn_tickers.remove(&workspace_id);
                                }
                                running
                            })
                            .unwrap_or(false);

                        if !still_running {
                            break;
                        }
                    }
                }
            },
        )
        .detach();
    }

    fn run_effect(&mut self, effect: Effect, cx: &mut Context<Self>) {
        match effect {
            Effect::LoadAppState => self.run_load_app_state(cx),
            Effect::SaveAppState => self.run_save_app_state(cx),
            Effect::CreateWorkspace { project_id } => self.run_create_workspace(project_id, cx),
            Effect::ArchiveWorkspace { workspace_id } => {
                self.run_archive_workspace(workspace_id, cx)
            }
            Effect::EnsureConversation { workspace_id } => {
                self.run_ensure_conversation(workspace_id, cx)
            }
            Effect::LoadConversation { workspace_id } => {
                self.run_load_conversation(workspace_id, cx)
            }
            Effect::RunAgentTurn { workspace_id, text } => {
                self.run_agent_turn(workspace_id, text, cx)
            }
            Effect::CancelAgentTurn { workspace_id } => {
                self.bump_turn_generation(workspace_id);
                if let Some(flag) = self.turn_cancel_flags.get(&workspace_id) {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        }
    }

    fn run_load_app_state(&mut self, cx: &mut Context<Self>) {
        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move { services.load_app_state() })
                        .await;

                    let action = match result {
                        Ok(persisted) => Action::AppStateLoaded { persisted },
                        Err(message) => Action::AppStateLoadFailed { message },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_save_app_state(&mut self, cx: &mut Context<Self>) {
        let services = self.services.clone();
        let snapshot = self.state.to_persisted();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move { services.save_app_state(snapshot) })
                        .await;

                    let action = match result {
                        Ok(()) => Action::AppStateSaved,
                        Err(message) => Action::AppStateSaveFailed { message },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn toggle_agent_item_expanded(&mut self, id: &str) {
        if !self.expanded_agent_items.insert(id.to_owned()) {
            self.expanded_agent_items.remove(id);
        }
    }

    fn run_create_workspace(&mut self, project_id: ProjectId, cx: &mut Context<Self>) {
        let Some(project) = self.state.project(project_id) else {
            self.dispatch(
                Action::WorkspaceCreateFailed {
                    project_id,
                    message: "Project not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let project_path = project.path.clone();
        let project_slug = project.slug.clone();
        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move {
                            services.create_workspace(project_path, project_slug)
                        })
                        .await;

                    let action = match result {
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

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_archive_workspace(&mut self, workspace_id: WorkspaceId, cx: &mut Context<Self>) {
        let Some((project_path, worktree_path)) = workspace_context(&self.state, workspace_id)
        else {
            self.dispatch(
                Action::WorkspaceArchiveFailed {
                    workspace_id,
                    message: "Workspace not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move {
                            services.archive_workspace(project_path, worktree_path)
                        })
                        .await;

                    let action = match result {
                        Ok(()) => Action::WorkspaceArchived { workspace_id },
                        Err(message) => Action::WorkspaceArchiveFailed {
                            workspace_id,
                            message,
                        },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_ensure_conversation(&mut self, workspace_id: WorkspaceId, cx: &mut Context<Self>) {
        let Some(agent_context) = workspace_agent_context(&self.state, workspace_id) else {
            self.dispatch(
                Action::ConversationLoadFailed {
                    workspace_id,
                    message: "Workspace not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move {
                            services.ensure_conversation(
                                agent_context.project_slug,
                                agent_context.workspace_name,
                            )
                        })
                        .await;

                    if let Err(message) = result {
                        let _ = this.update(
                            &mut async_cx,
                            |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                                view.dispatch(
                                    Action::ConversationLoadFailed {
                                        workspace_id,
                                        message,
                                    },
                                    view_cx,
                                )
                            },
                        );
                    }
                }
            },
        )
        .detach();
    }

    fn run_load_conversation(&mut self, workspace_id: WorkspaceId, cx: &mut Context<Self>) {
        let Some(agent_context) = workspace_agent_context(&self.state, workspace_id) else {
            self.dispatch(
                Action::ConversationLoadFailed {
                    workspace_id,
                    message: "Workspace not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move {
                            services.load_conversation(
                                agent_context.project_slug,
                                agent_context.workspace_name,
                            )
                        })
                        .await;

                    let action = match result {
                        Ok(snapshot) => Action::ConversationLoaded {
                            workspace_id,
                            snapshot,
                        },
                        Err(message) => Action::ConversationLoadFailed {
                            workspace_id,
                            message,
                        },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_agent_turn(&mut self, workspace_id: WorkspaceId, text: String, cx: &mut Context<Self>) {
        let Some(agent_context) = workspace_agent_context(&self.state, workspace_id) else {
            self.dispatch(Action::AgentTurnFinished { workspace_id }, cx);
            return;
        };

        let generation = self.bump_turn_generation(workspace_id);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.turn_cancel_flags
            .insert(workspace_id, cancel_flag.clone());

        let thread_id = self
            .state
            .workspace_conversation(workspace_id)
            .and_then(|c| c.thread_id.clone());
        let request = RunAgentTurnRequest {
            project_slug: agent_context.project_slug,
            workspace_name: agent_context.workspace_name,
            worktree_path: agent_context.worktree_path,
            thread_id,
            prompt: text,
        };
        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let (tx, rx) = async_channel::unbounded::<CodexThreadEvent>();

                    let tx_for_events = tx.clone();
                    let tx_for_error = tx.clone();
                    let on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync> =
                        Arc::new(move |e| {
                            let _ = tx_for_events.send_blocking(e);
                        });

                    std::thread::spawn(move || {
                        let result =
                            services.run_agent_turn_streamed(request, cancel_flag, on_event);

                        if let Err(message) = result {
                            let _ = tx_for_error.send_blocking(CodexThreadEvent::Error { message });
                        }
                    });

                    drop(tx);

                    while let Ok(event) = rx.recv().await {
                        let _ = this.update(
                            &mut async_cx,
                            |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                                let current_generation = view
                                    .turn_generation
                                    .get(&workspace_id)
                                    .copied()
                                    .unwrap_or(0);
                                if current_generation != generation {
                                    return;
                                }

                                let still_running = view
                                    .state
                                    .workspace_conversation(workspace_id)
                                    .map(|c| c.run_status == OperationStatus::Running)
                                    .unwrap_or(false);
                                if !still_running {
                                    return;
                                }

                                view.dispatch(
                                    Action::AgentEventReceived {
                                        workspace_id,
                                        event,
                                    },
                                    view_cx,
                                );
                            },
                        );
                    }

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            let current_generation = view
                                .turn_generation
                                .get(&workspace_id)
                                .copied()
                                .unwrap_or(0);
                            if current_generation != generation {
                                return;
                            }
                            view.dispatch(Action::AgentTurnFinished { workspace_id }, view_cx);
                        },
                    );
                }
            },
        )
        .detach();
    }
}

impl gpui::Render for LubanRootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let sidebar_width = px(300.0);

        div()
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(render_sidebar(cx, &self.state, sidebar_width))
            .child(self.render_main(window, cx))
    }
}

fn render_sidebar(
    cx: &mut Context<LubanRootView>,
    state: &AppState,
    sidebar_width: gpui::Pixels,
) -> impl IntoElement {
    let theme = cx.theme();
    let view_handle = cx.entity().downgrade();

    let add_project_button = Button::new("add-project")
        .ghost()
        .compact()
        .icon(Icon::new(IconName::Plus).text_color(theme.muted_foreground))
        .tooltip("Add project")
        .on_click(move |_, _window, app| {
            let view_handle = view_handle.clone();
            let options = gpui::PathPromptOptions {
                files: false,
                directories: true,
                multiple: false,
                prompt: Some("Add Project".into()),
            };

            let receiver = app.prompt_for_paths(options);
            app.spawn(move |cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let Ok(result) = receiver.await else {
                        return;
                    };
                    let Ok(Some(mut paths)) = result else {
                        return;
                    };
                    let Some(path) = paths.pop() else {
                        return;
                    };

                    let _ = view_handle.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(Action::AddProject { path }, view_cx);
                        },
                    );
                }
            })
            .detach();
        });

    div()
        .w(sidebar_width)
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .bg(theme.sidebar)
        .text_color(theme.sidebar_foreground)
        .border_r_1()
        .border_color(theme.sidebar_border)
        .child(
            div()
                .h(px(40.0))
                .px_2()
                .flex()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(theme.sidebar_border)
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .text_xs()
                        .child("PROJECTS"),
                )
                .child(
                    div()
                        .debug_selector(|| "add-project".to_owned())
                        .child(add_project_button),
                ),
        )
        .child(
            div()
                .flex_1()
                .id("projects-scroll")
                .overflow_scroll()
                .py_2()
                .children(
                    state
                        .projects
                        .iter()
                        .enumerate()
                        .map(|(i, project)| render_project(cx, i, project, state.main_pane)),
                ),
        )
}

fn render_project(
    cx: &mut Context<LubanRootView>,
    project_index: usize,
    project: &luban_domain::Project,
    main_pane: MainPane,
) -> AnyElement {
    let theme = cx.theme();
    let is_selected = matches!(main_pane, MainPane::ProjectSettings(id) if id == project.id);
    let view_handle = cx.entity().downgrade();
    let project_id = project.id;

    let disclosure_icon = if project.expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let create_loading = matches!(project.create_workspace_status, OperationStatus::Running);

    let selection_border = if is_selected {
        theme.primary
    } else {
        theme.transparent
    };
    let header = div()
        .mx_2()
        .mt_1()
        .h(px(32.0))
        .px_2()
        .flex()
        .items_center()
        .justify_between()
        .rounded_md()
        .border_l_2()
        .border_color(selection_border)
        .bg(if is_selected {
            theme.sidebar_accent
        } else {
            theme.transparent
        })
        .hover(move |s| s.bg(theme.sidebar_accent))
        .group("")
        .text_color(if is_selected {
            theme.sidebar_accent_foreground
        } else {
            theme.sidebar_foreground
        })
        .debug_selector(move || format!("project-header-{project_index}"))
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener({
                        move |this, _, _, cx| {
                            this.dispatch(Action::ToggleProjectExpanded { project_id }, cx)
                        }
                    }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .max_w(px(220.0))
                                .truncate()
                                .text_lg()
                                .font_semibold()
                                .child(project.name.clone()),
                        )
                        .child(
                            div()
                                .w(px(16.0))
                                .debug_selector(move || format!("project-toggle-{project_index}"))
                                .child(
                                    Icon::new(disclosure_icon)
                                        .with_size(Size::Small)
                                        .text_color(theme.muted_foreground),
                                ),
                        ),
                ),
        )
        .child(
            div().flex().items_center().gap_1().child(
                div()
                    .when(!create_loading, |s| s.invisible())
                    .group_hover("", |s| s.visible())
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .debug_selector(move || {
                                format!("project-create-workspace-{project_index}")
                            })
                            .child(
                                Button::new(format!("project-create-workspace-{project_index}"))
                                    .ghost()
                                    .compact()
                                    .disabled(create_loading)
                                    .loading(create_loading)
                                    .icon(
                                        Icon::new(IconName::Plus)
                                            .text_color(theme.muted_foreground),
                                    )
                                    .tooltip("Create workspace")
                                    .on_click({
                                        let view_handle = view_handle.clone();
                                        move |_, _, app| {
                                            let _ = view_handle.update(app, |view, cx| {
                                                view.dispatch(
                                                    Action::CreateWorkspace { project_id },
                                                    cx,
                                                );
                                            });
                                        }
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(move || format!("project-settings-{project_index}"))
                            .child(
                                Button::new(format!("project-settings-{project_index}"))
                                    .ghost()
                                    .compact()
                                    .icon(
                                        Icon::new(IconName::Ellipsis)
                                            .text_color(theme.muted_foreground),
                                    )
                                    .tooltip("Project settings")
                                    .on_click({
                                        let view_handle = view_handle.clone();
                                        move |_, _, app| {
                                            let _ = view_handle.update(app, |view, cx| {
                                                view.dispatch(
                                                    Action::OpenProjectSettings { project_id },
                                                    cx,
                                                );
                                            });
                                        }
                                    }),
                            ),
                    ),
            ),
        );

    let children = project
        .workspaces
        .iter()
        .enumerate()
        .filter(|(_, w)| w.status == WorkspaceStatus::Active)
        .map(|(workspace_index, workspace)| {
            render_workspace_row(
                cx,
                view_handle.clone(),
                project_index,
                workspace_index,
                project.id,
                workspace,
                main_pane,
            )
        });

    div()
        .flex()
        .flex_col()
        .child(header)
        .when(project.expanded, |s| {
            s.child(div().pl(px(22.0)).flex().flex_col().children(children))
        })
        .into_any_element()
}

fn render_workspace_row(
    cx: &mut Context<LubanRootView>,
    view_handle: gpui::WeakEntity<LubanRootView>,
    project_index: usize,
    workspace_index: usize,
    _project_id: ProjectId,
    workspace: &luban_domain::Workspace,
    main_pane: MainPane,
) -> AnyElement {
    let theme = cx.theme();
    let is_selected = matches!(main_pane, MainPane::Workspace(id) if id == workspace.id);
    let workspace_id = workspace.id;
    let archive_disabled = workspace.archive_status == OperationStatus::Running;
    let archive_icon = if archive_disabled {
        IconName::LoaderCircle
    } else {
        IconName::Inbox
    };

    let selection_border = if is_selected {
        theme.primary
    } else {
        theme.transparent
    };
    let row = div()
        .mx_2()
        .h(px(30.0))
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .rounded_md()
        .border_l_2()
        .border_color(selection_border)
        .bg(if is_selected {
            theme.sidebar_accent
        } else {
            theme.transparent
        })
        .hover(move |s| s.bg(theme.sidebar_accent))
        .group("")
        .text_color(if is_selected {
            theme.sidebar_accent_foreground
        } else {
            theme.sidebar_foreground
        })
        .debug_selector(move || format!("workspace-row-{project_index}-{workspace_index}"))
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.dispatch(Action::OpenWorkspace { workspace_id }, cx)
                    }),
                )
                .child(
                    div()
                        .flex_1()
                        .truncate()
                        .text_base()
                        .child(workspace.workspace_name.clone()),
                ),
        )
        .child(
            div()
                .debug_selector(move || {
                    format!("workspace-archive-{project_index}-{workspace_index}")
                })
                .when(!archive_disabled, |s| s.invisible())
                .group_hover("", |s| s.visible())
                .child(
                    Button::new(format!("workspace-archive-{project_index}-{workspace_index}"))
                        .ghost()
                        .compact()
                        .disabled(archive_disabled)
                        .icon(Icon::new(archive_icon).text_color(theme.muted_foreground))
                        .tooltip("Archive workspace")
                        .on_click(move |_, window, app| {
                            if archive_disabled {
                                return;
                            }

                            let receiver = window.prompt(
                                PromptLevel::Warning,
                                "Archive workspace?",
                                Some("This will remove the git worktree on disk."),
                                &[PromptButton::ok("Archive"), PromptButton::cancel("Cancel")],
                                app,
                            );

                            let view_handle = view_handle.clone();
                            app.spawn(move |cx: &mut gpui::AsyncApp| {
                                let mut async_cx = cx.clone();
                                async move {
                                    let Ok(choice) = receiver.await else {
                                        return;
                                    };
                                    if choice != 0 {
                                        return;
                                    }
                                    let _ = view_handle.update(
                                        &mut async_cx,
                                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                                            view.dispatch(
                                                Action::ArchiveWorkspace { workspace_id },
                                                view_cx,
                                            );
                                        },
                                    );
                                }
                            })
                            .detach();
                        }),
                ),
        );

    row.into_any_element()
}

impl LubanRootView {
    fn ensure_chat_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<InputState> {
        if let Some(input) = self.chat_input.clone() {
            return input;
        }

        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(4, 12)
                .placeholder("Message... (\u{2318}\u{21a9} to send)")
        });

        let subscription = cx.subscribe_in(&input_state, window, {
            let input_state = input_state.clone();
            move |this: &mut LubanRootView, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => {
                    if let MainPane::Workspace(workspace_id) = this.state.main_pane {
                        let text = input_state.read(cx).value().to_owned();
                        let existing = this
                            .state
                            .workspace_conversation(workspace_id)
                            .map(|c| c.draft.as_str())
                            .unwrap_or("");
                        if text != existing {
                            this.dispatch(
                                Action::ChatDraftChanged {
                                    workspace_id,
                                    text: text.to_string(),
                                },
                                cx,
                            );
                        }
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { secondary: true } => {
                    let text = input_state.read(cx).value().trim().to_owned();
                    if text.is_empty() {
                        return;
                    }
                    let MainPane::Workspace(workspace_id) = this.state.main_pane else {
                        return;
                    };
                    input_state.update(cx, |state, cx| state.set_value("", window, cx));
                    this.dispatch(Action::SendAgentMessage { workspace_id, text }, cx);
                }
                InputEvent::PressEnter { .. } | InputEvent::Focus | InputEvent::Blur => {}
            }
        });

        self._subscriptions.push(subscription);
        self.chat_input = Some(input_state.clone());
        input_state
    }

    fn render_main(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let view_handle = cx.entity().downgrade();
        let title = main_pane_title(&self.state, self.state.main_pane);
        let show_title_bar = matches!(self.state.main_pane, MainPane::ProjectSettings(_));

        let content = match self.state.main_pane {
            MainPane::None => {
                self.last_chat_workspace_id = None;
                self.last_chat_item_count = 0;

                div().flex_1().into_any_element()
            }
            MainPane::ProjectSettings(project_id) => {
                self.last_chat_workspace_id = None;
                self.last_chat_item_count = 0;

                let title = self
                    .state
                    .project(project_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "Project".to_owned());

                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_w(px(900.0))
                    .mx_auto()
                    .child(div().text_lg().child(title))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("No settings yet."),
                    )
                    .into_any_element()
            }
            MainPane::Workspace(workspace_id) => {
                if self.state.workspace(workspace_id).is_none() {
                    return div()
                        .p_3()
                        .child(
                            div()
                                .text_color(cx.theme().danger_foreground)
                                .child("Workspace not found"),
                        )
                        .into_any_element();
                }

                let input_state = self.ensure_chat_input(window, cx);

                let conversation = self.state.workspace_conversation(workspace_id);
                let entries: &[luban_domain::ConversationEntry] =
                    conversation.map(|c| c.entries.as_slice()).unwrap_or(&[]);
                let in_progress_items: Vec<&CodexThreadItem> = conversation
                    .map(|c| c.in_progress_items.values().collect())
                    .unwrap_or_default();
                let run_status = conversation
                    .map(|c| c.run_status)
                    .unwrap_or(OperationStatus::Idle);
                let queued_prompts: Vec<String> = conversation
                    .map(|c| c.pending_prompts.iter().cloned().collect())
                    .unwrap_or_default();
                let queue_paused = conversation.map(|c| c.queue_paused).unwrap_or(false);
                let _thread_id = conversation.and_then(|c| c.thread_id.as_deref());

                let is_running = run_status == OperationStatus::Running;
                let workspace_changed = self.last_chat_workspace_id != Some(workspace_id);
                if workspace_changed {
                    let saved_draft = conversation.map(|c| c.draft.clone()).unwrap_or_default();
                    let current_value = input_state.read(cx).value().to_owned();
                    let should_move_cursor = !saved_draft.is_empty();
                    if current_value != saved_draft.as_str() || should_move_cursor {
                        input_state.update(cx, move |state, cx| {
                            if current_value != saved_draft.as_str() {
                                state.set_value(&saved_draft, window, cx);
                            }

                            if should_move_cursor {
                                let end = state.text().offset_to_position(state.text().len());
                                if state.cursor_position() != end {
                                    state.set_cursor_position(end, window, cx);
                                }
                            }
                        });
                    }
                }

                let theme = cx.theme();

                let draft = input_state.read(cx).value().trim().to_owned();
                let send_disabled = draft.is_empty();
                let running_elapsed = if is_running {
                    self.running_turn_started_at
                        .get(&workspace_id)
                        .map(|t| t.elapsed())
                } else {
                    None
                };

                let expanded = self.expanded_agent_items.clone();
                let expanded_turns = self.expanded_agent_turns.clone();
                let has_in_progress_items = !in_progress_items.is_empty();

                let history_children = build_workspace_history_children(
                    entries,
                    theme,
                    &expanded,
                    &expanded_turns,
                    self.chat_column_width,
                    &view_handle,
                );
                let rendered_item_count = history_children.len()
                    + in_progress_items.len()
                    + usize::from(running_elapsed.is_some())
                    + usize::from(is_running && !has_in_progress_items);

                let pinned_to_bottom = {
                    let offset = self.chat_scroll_handle.offset();
                    let max_offset = self.chat_scroll_handle.max_offset();
                    let threshold = if max_offset.height > px(24.0) {
                        max_offset.height - px(24.0)
                    } else {
                        px(0.0)
                    };
                    (-offset.y) >= threshold
                };

                let item_count_increased =
                    !workspace_changed && rendered_item_count > self.last_chat_item_count;
                if workspace_changed || (item_count_increased && pinned_to_bottom) {
                    self.chat_scroll_handle.scroll_to_bottom();
                }
                self.last_chat_workspace_id = Some(workspace_id);
                self.last_chat_item_count = rendered_item_count;

                let mut in_progress_children = Vec::with_capacity(in_progress_items.len());
                for item in in_progress_items {
                    in_progress_children.push(render_codex_item(
                        item,
                        theme,
                        true,
                        &expanded,
                        self.chat_column_width,
                        &view_handle,
                    ));
                }

                let history = div()
                    .flex_1()
                    .id("workspace-chat-scroll")
                    .overflow_scroll()
                    .track_scroll(&self.chat_scroll_handle)
                    .overflow_x_hidden()
                    .w_full()
                    .px_4()
                    .py_3()
                    .child(min_width_zero(
                        div()
                            .debug_selector(|| "workspace-chat-column".to_owned())
                            .on_prepaint({
                                let view_handle = view_handle.clone();
                                move |bounds, _window, app| {
                                    let width = bounds.size.width;
                                    let _ = view_handle.update(app, |view, cx| {
                                        let should_update = match view.chat_column_width {
                                            Some(prev) => (prev - width).abs() > px(0.5),
                                            None => true,
                                        };
                                        if should_update {
                                            view.chat_column_width = Some(width);
                                            cx.notify();
                                        }
                                    });
                                }
                            })
                            .w_full()
                            .max_w(px(900.0))
                            .mx_auto()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .whitespace_normal()
                            .pb(px(160.0))
                            .children(history_children)
                            .children(in_progress_children)
                            .when_some(running_elapsed, |s, elapsed| {
                                s.child(render_turn_duration_row(theme, elapsed, true))
                            })
                            .when(is_running && !has_in_progress_items, |s| {
                                s.child(
                                    div()
                                        .h(px(28.0))
                                        .w_full()
                                        .px_1()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Spinner::new()
                                                .with_size(Size::Small)
                                                .color(theme.muted_foreground),
                                        )
                                        .child(
                                            div()
                                                .text_color(theme.muted_foreground)
                                                .child("Thinking:"),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .truncate()
                                                .text_color(theme.muted_foreground)
                                                .child("…"),
                                        ),
                                )
                            }),
                    ));

                let queue_panel = if !queued_prompts.is_empty() {
                    let theme = cx.theme();
                    let view_handle = view_handle.clone();
                    let input_state = input_state.clone();

                    let toolbar = div()
                        .h(px(24.0))
                        .w_full()
                        .px_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(div().text_xs().text_color(theme.muted_foreground).child(
                            if queue_paused {
                                "Queued • Paused"
                            } else {
                                "Queued"
                            },
                        ))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .when(queue_paused && !is_running, |s| {
                                    let view_handle = view_handle.clone();
                                    s.child(
                                        Button::new("queued-resume")
                                            .primary()
                                            .compact()
                                            .icon(IconName::Redo2)
                                            .tooltip("Resume queued messages")
                                            .on_click(move |_, _, app| {
                                                let _ = view_handle.update(app, |view, cx| {
                                                    view.dispatch(
                                                        Action::ResumeQueuedPrompts {
                                                            workspace_id,
                                                        },
                                                        cx,
                                                    );
                                                });
                                            }),
                                    )
                                })
                                .child(
                                    Button::new("queued-clear-all")
                                        .ghost()
                                        .compact()
                                        .icon(IconName::Delete)
                                        .tooltip("Clear queued messages")
                                        .on_click({
                                            let view_handle = view_handle.clone();
                                            move |_, window, app| {
                                                let receiver = window.prompt(
                                                    PromptLevel::Warning,
                                                    "Clear queued messages?",
                                                    Some("This will remove all queued messages."),
                                                    &[
                                                        PromptButton::ok("Clear"),
                                                        PromptButton::cancel("Cancel"),
                                                    ],
                                                    app,
                                                );

                                                let view_handle = view_handle.clone();
                                                app.spawn(move |cx: &mut gpui::AsyncApp| {
                                                    let mut async_cx = cx.clone();
                                                    async move {
                                                        let Ok(choice) = receiver.await else {
                                                            return;
                                                        };
                                                        if choice != 0 {
                                                            return;
                                                        }
                                                        let _ = view_handle.update(
                                                            &mut async_cx,
                                                            |view: &mut LubanRootView, view_cx| {
                                                                view.dispatch(
                                                                    Action::ClearQueuedPrompts {
                                                                        workspace_id,
                                                                    },
                                                                    view_cx,
                                                                );
                                                            },
                                                        );
                                                    }
                                                })
                                                .detach();
                                            }
                                        }),
                                ),
                        );

                    let content = div().pt_2().px_2().flex().flex_col().gap_1().children(
                        queued_prompts.iter().enumerate().map(|(idx, text)| {
                            let view_handle_for_edit = view_handle.clone();
                            let view_handle_for_remove = view_handle.clone();
                            let input_state = input_state.clone();
                            let text = text.clone();
                            div()
                                .h(px(28.0))
                                .w_full()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .truncate()
                                        .text_color(theme.muted_foreground)
                                        .child(text.clone()),
                                )
                                .child(
                                    Button::new(format!("queued-edit-{idx}"))
                                        .ghost()
                                        .compact()
                                        .icon(IconName::Replace)
                                        .tooltip("Move to input and remove from queue")
                                        .on_click(move |_, window, app| {
                                            input_state.update(app, |state, cx| {
                                                state.set_value(&text, window, cx);
                                            });
                                            let _ = view_handle_for_edit.update(app, |view, cx| {
                                                view.dispatch(
                                                    Action::RemoveQueuedPrompt {
                                                        workspace_id,
                                                        index: idx,
                                                    },
                                                    cx,
                                                );
                                            });
                                        }),
                                )
                                .child({
                                    Button::new(format!("queued-remove-{idx}"))
                                        .ghost()
                                        .compact()
                                        .icon(IconName::Close)
                                        .tooltip("Remove from queue")
                                        .on_click(move |_, _, app| {
                                            let _ =
                                                view_handle_for_remove.update(app, |view, cx| {
                                                    view.dispatch(
                                                        Action::RemoveQueuedPrompt {
                                                            workspace_id,
                                                            index: idx,
                                                        },
                                                        cx,
                                                    );
                                                });
                                        })
                                })
                                .into_any_element()
                        }),
                    );

                    div()
                        .w_full()
                        .child(toolbar)
                        .child(content)
                        .into_any_element()
                } else {
                    div().hidden().into_any_element()
                };

                let composer = div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .px_4()
                    .pb_4()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(900.0))
                            .mx_auto()
                            .p_2()
                            .rounded_lg()
                            .bg(theme.background)
                            .border_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(queue_panel)
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .items_end()
                                            .gap_2()
                                            .child(
                                                div().flex_1().child(
                                                    Input::new(&input_state)
                                                        .appearance(false)
                                                        .with_size(Size::Large),
                                                ),
                                            )
                                            .child({
                                                let view_handle = view_handle.clone();
                                                let input_state = input_state.clone();
                                                let draft = draft.clone();
                                                Button::new("chat-send-message")
                                                    .primary()
                                                    .compact()
                                                    .disabled(send_disabled)
                                                    .icon(Icon::new(IconName::ArrowUp))
                                                    .tooltip(if is_running {
                                                        "Queue"
                                                    } else {
                                                        "Send"
                                                    })
                                                    .on_click(move |_, window, app| {
                                                        if draft.trim().is_empty() {
                                                            return;
                                                        }

                                                        input_state.update(app, |state, cx| {
                                                            state.set_value("", window, cx);
                                                        });

                                                        let _ =
                                                            view_handle.update(app, |view, cx| {
                                                                view.dispatch(
                                                                    Action::SendAgentMessage {
                                                                        workspace_id,
                                                                        text: draft.clone(),
                                                                    },
                                                                    cx,
                                                                );
                                                            });
                                                    })
                                                    .into_any_element()
                                            })
                                            .when(is_running, |s| {
                                                let view_handle = view_handle.clone();
                                                s.child(
                                                    Button::new("chat-cancel-turn")
                                                        .danger()
                                                        .compact()
                                                        .icon(Icon::new(IconName::CircleX))
                                                        .tooltip("Cancel")
                                                        .on_click(move |_, _, app| {
                                                            let _ = view_handle.update(
                                                                app,
                                                                |view, cx| {
                                                                    view.dispatch(
                                                                        Action::CancelAgentTurn {
                                                                            workspace_id,
                                                                        },
                                                                        cx,
                                                                    );
                                                                },
                                                            );
                                                        }),
                                                )
                                            }),
                                    ),
                            ),
                    );

                div()
                    .flex()
                    .flex_col()
                    .h_full()
                    .relative()
                    .child(history)
                    .child(composer)
                    .into_any_element()
            }
        };

        let theme = cx.theme();
        let title_bar = div()
            .h(px(44.0))
            .px_4()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(theme.title_bar_border)
            .bg(theme.title_bar)
            .child(div().text_sm().child(title));

        min_width_zero(
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .bg(theme.background)
                .when(show_title_bar, |s| s.child(title_bar))
                .when_some(self.state.last_error.clone(), |s, message| {
                    let theme = cx.theme();
                    let view_handle = cx.entity().downgrade();
                    s.child(
                        div()
                            .mx_4()
                            .mt_3()
                            .p_3()
                            .rounded_md()
                            .bg(theme.danger)
                            .border_1()
                            .border_color(theme.danger_hover)
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(theme.danger_foreground)
                            .child(div().child(message))
                            .child(
                                div().debug_selector(|| "error-dismiss".to_owned()).child(
                                    Button::new("error-dismiss")
                                        .ghost()
                                        .compact()
                                        .label("Dismiss")
                                        .on_click(move |_, _, app| {
                                            let _ = view_handle.update(app, |view, cx| {
                                                view.dispatch(Action::ClearError, cx);
                                            });
                                        }),
                                ),
                            ),
                    )
                })
                .child(content),
        )
        .into_any_element()
    }
}

fn main_pane_title(state: &AppState, pane: MainPane) -> String {
    match pane {
        MainPane::None => String::new(),
        MainPane::ProjectSettings(project_id) => state
            .project(project_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Project Settings".to_owned()),
        MainPane::Workspace(workspace_id) => state
            .workspace(workspace_id)
            .map(|w| w.workspace_name.clone())
            .unwrap_or_else(|| "Workspace".to_owned()),
    }
}

fn render_conversation_entry(
    entry_index: usize,
    entry: &luban_domain::ConversationEntry,
    theme: &gpui_component::Theme,
    expanded_items: &HashSet<String>,
    chat_column_width: Option<Pixels>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    match entry {
        luban_domain::ConversationEntry::UserMessage { text } => {
            let is_short_single_line = text.lines().nth(1).is_none() && text.chars().count() <= 80;
            let wrap_width = if is_short_single_line {
                None
            } else {
                chat_column_width
                    .map(|w| w.min(px(680.0)))
                    .map(|w| (w - px(32.0)).max(px(0.0)))
            };
            let message = chat_message_view(
                &format!("user-message-{entry_index}"),
                text,
                wrap_width,
                theme.foreground,
            );
            let bubble = min_width_zero(
                div()
                    .max_w(px(680.0))
                    .overflow_x_hidden()
                    .p_2()
                    .rounded_md()
                    .bg(theme.accent)
                    .border_1()
                    .border_color(theme.border)
                    .child(min_width_zero(
                        div().w_full().whitespace_normal().child(message),
                    )),
            );

            div()
                .debug_selector(move || format!("conversation-user-row-{entry_index}"))
                .id(format!("conversation-user-{entry_index}"))
                .w_full()
                .overflow_x_hidden()
                .flex()
                .flex_row()
                .justify_end()
                .child(
                    bubble
                        .debug_selector(move || format!("conversation-user-bubble-{entry_index}")),
                )
                .into_any_element()
        }
        luban_domain::ConversationEntry::CodexItem { item } => div()
            .id(format!(
                "conversation-codex-{}-{entry_index}",
                codex_item_id(item)
            ))
            .w_full()
            .child(render_codex_item(
                item.as_ref(),
                theme,
                false,
                expanded_items,
                chat_column_width,
                view_handle,
            ))
            .into_any_element(),
        luban_domain::ConversationEntry::TurnUsage { usage: _ } => div()
            .id(format!("conversation-usage-{entry_index}"))
            .hidden()
            .into_any_element(),
        luban_domain::ConversationEntry::TurnDuration { duration_ms } => div()
            .debug_selector(move || format!("turn-duration-{entry_index}"))
            .id(format!("conversation-duration-{entry_index}"))
            .child(render_turn_duration_row(
                theme,
                Duration::from_millis(*duration_ms),
                false,
            ))
            .into_any_element(),
        luban_domain::ConversationEntry::TurnCanceled => div()
            .id(format!("conversation-canceled-{entry_index}"))
            .p_2()
            .rounded_md()
            .bg(theme.muted)
            .border_1()
            .border_color(theme.border)
            .text_color(theme.muted_foreground)
            .child(div().child("Canceled"))
            .into_any_element(),
        luban_domain::ConversationEntry::TurnError { message } => div()
            .id(format!("conversation-error-{entry_index}"))
            .p_2()
            .rounded_md()
            .bg(theme.danger)
            .border_1()
            .border_color(theme.danger_hover)
            .text_color(theme.danger_foreground)
            .child(div().child(message.clone()))
            .into_any_element(),
    }
}

fn min_width_zero(mut element: gpui::Div) -> gpui::Div {
    element.style().min_size.width = Some(px(0.0).into());
    element
}

#[derive(Clone, Copy)]
struct TurnSummaryCounts {
    tool_calls: usize,
    reasonings: usize,
}

fn format_agent_turn_summary(counts: TurnSummaryCounts) -> String {
    format!(
        "{} tool calls, {} thinking",
        counts.tool_calls, counts.reasonings
    )
}

fn build_workspace_history_children(
    entries: &[luban_domain::ConversationEntry],
    theme: &gpui_component::Theme,
    expanded_items: &HashSet<String>,
    expanded_turns: &HashSet<String>,
    chat_column_width: Option<Pixels>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> Vec<AnyElement> {
    struct TurnAccumulator<'a> {
        id: String,
        tool_calls: usize,
        reasonings: usize,
        summary_items: Vec<&'a CodexThreadItem>,
        agent_messages: Vec<&'a CodexThreadItem>,
    }

    let mut children = Vec::new();
    let mut turn_index = 0usize;
    let mut current_turn: Option<TurnAccumulator<'_>> = None;

    let flush_turn = |turn: TurnAccumulator<'_>, children: &mut Vec<AnyElement>| {
        if turn.summary_items.is_empty() && turn.agent_messages.is_empty() {
            return;
        }

        let turn_container_id = turn.id.clone();
        let turn_id = turn.id.clone();
        let expanded = expanded_turns.contains(&turn.id);
        let header = render_agent_turn_summary_row(
            &turn.id,
            TurnSummaryCounts {
                tool_calls: turn.tool_calls,
                reasonings: turn.reasonings,
            },
            !turn.summary_items.is_empty(),
            expanded,
            theme,
            view_handle,
        );
        let mut summary_children = Vec::with_capacity(turn.summary_items.len());
        for item in turn.summary_items {
            summary_children.push(render_tool_summary_item(
                &turn_id,
                item,
                theme,
                expanded_items,
                chat_column_width,
                view_handle,
            ));
        }
        let content = div()
            .pl_4()
            .flex()
            .flex_col()
            .gap_2()
            .children(summary_children);

        children.push(
            div()
                .id(format!("conversation-turn-{turn_container_id}"))
                .w_full()
                .child(
                    Collapsible::new()
                        .open(expanded)
                        .w_full()
                        .child(header)
                        .content(content),
                )
                .into_any_element(),
        );

        for item in turn.agent_messages {
            children.push(render_codex_item(
                item,
                theme,
                false,
                expanded_items,
                chat_column_width,
                view_handle,
            ));
        }
    };

    for (entry_index, entry) in entries.iter().enumerate() {
        match entry {
            luban_domain::ConversationEntry::UserMessage { text: _ } => {
                if let Some(turn) = current_turn.take() {
                    flush_turn(turn, &mut children);
                }

                children.push(render_conversation_entry(
                    entry_index,
                    entry,
                    theme,
                    expanded_items,
                    chat_column_width,
                    view_handle,
                ));
                current_turn = Some(TurnAccumulator {
                    id: format!("agent-turn-{turn_index}"),
                    tool_calls: 0,
                    reasonings: 0,
                    summary_items: Vec::new(),
                    agent_messages: Vec::new(),
                });
                turn_index += 1;
            }
            luban_domain::ConversationEntry::CodexItem { item } => {
                let item = item.as_ref();
                if let Some(turn) = &mut current_turn {
                    if matches!(item, CodexThreadItem::AgentMessage { .. }) {
                        turn.agent_messages.push(item);
                        continue;
                    }

                    if matches!(item, CodexThreadItem::Reasoning { .. }) {
                        turn.reasonings += 1;
                        turn.summary_items.push(item);
                        continue;
                    }

                    if matches!(item, CodexThreadItem::Error { .. }) {
                        turn.summary_items.push(item);
                        continue;
                    }

                    if codex_item_is_tool_call(item) {
                        turn.tool_calls += 1;
                        turn.summary_items.push(item);
                    }
                    continue;
                }

                children.push(render_codex_item(
                    item,
                    theme,
                    false,
                    expanded_items,
                    chat_column_width,
                    view_handle,
                ));
            }
            luban_domain::ConversationEntry::TurnUsage { .. } => {
                if let Some(turn) = current_turn.take() {
                    flush_turn(turn, &mut children);
                }
            }
            luban_domain::ConversationEntry::TurnDuration { .. }
            | luban_domain::ConversationEntry::TurnCanceled
            | luban_domain::ConversationEntry::TurnError { .. } => {
                if let Some(turn) = current_turn.take() {
                    flush_turn(turn, &mut children);
                }
                children.push(render_conversation_entry(
                    entry_index,
                    entry,
                    theme,
                    expanded_items,
                    chat_column_width,
                    view_handle,
                ));
            }
        }
    }

    if let Some(turn) = current_turn.take() {
        flush_turn(turn, &mut children);
    }

    children
}

fn render_agent_turn_summary_row(
    id: &str,
    counts: TurnSummaryCounts,
    has_ops: bool,
    expanded: bool,
    theme: &gpui_component::Theme,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    let debug_id = format!("agent-turn-summary-{id}");
    let view_handle_for_click = view_handle.clone();
    let id_for_click = id.to_owned();

    let row = div()
        .debug_selector(move || debug_id.clone())
        .h(px(28.0))
        .w_full()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .group("")
        .when(has_ops, move |s| {
            let view_handle = view_handle_for_click.clone();
            let id = id_for_click.clone();
            s.cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, _, app| {
                    let _ = view_handle.update(app, |view, cx| {
                        view.toggle_agent_turn_expanded(&id);
                        cx.notify();
                    });
                })
        });

    let disclosure_icon = if expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    row.child(
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(min_width_zero(
                div()
                    .flex_1()
                    .truncate()
                    .text_left()
                    .text_color(theme.muted_foreground)
                    .child(format_agent_turn_summary(counts)),
            ))
            .child(div().w(px(16.0)).when(has_ops, |s| {
                let debug_id = format!("agent-turn-toggle-{id}");
                s.debug_selector(move || debug_id.clone())
                    .invisible()
                    .when(expanded, |s| s.visible())
                    .group_hover("", |s| s.visible())
                    .child(
                        Icon::new(disclosure_icon)
                            .with_size(Size::Small)
                            .text_color(theme.muted_foreground),
                    )
            })),
    )
    .into_any_element()
}

fn render_tool_summary_item(
    turn_id: &str,
    item: &CodexThreadItem,
    theme: &gpui_component::Theme,
    expanded_items: &HashSet<String>,
    chat_column_width: Option<Pixels>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    let item_id = codex_item_id(item);
    let item_key = format!("{turn_id}::{item_id}");
    let expanded = expanded_items.contains(&item_key);
    let element_id = format!("conversation-turn-item-{}", item_key.replace("::", "-"));
    let debug_id = format!("agent-turn-item-summary-{turn_id}-{item_id}");

    let (title, summary) = codex_item_summary(item, false);
    let icon = Icon::empty()
        .path(codex_item_icon_path(item))
        .with_size(Size::Small)
        .text_color(theme.muted_foreground);

    let disclosure_icon = if expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    let view_handle_for_click = view_handle.clone();
    let item_key_for_click = item_key.clone();
    let header = div()
        .debug_selector(move || debug_id.clone())
        .h(px(28.0))
        .w_full()
        .px_2()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .group("")
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_, _, app| {
            let _ = view_handle_for_click.update(app, |view, cx| {
                view.toggle_agent_item_expanded(&item_key_for_click);
                cx.notify();
            });
        })
        .child(icon)
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(format!("{title}:")),
        )
        .child(min_width_zero(
            div()
                .flex_1()
                .truncate()
                .text_color(theme.muted_foreground)
                .child(summary),
        ))
        .child(
            div()
                .w(px(16.0))
                .invisible()
                .when(expanded, |s| s.visible())
                .group_hover("", |s| s.visible())
                .child(
                    Icon::new(disclosure_icon)
                        .with_size(Size::Small)
                        .text_color(theme.muted_foreground),
                ),
        );

    let details = div()
        .w_full()
        .overflow_x_hidden()
        .whitespace_normal()
        .pl_6()
        .child(render_codex_item_details(
            item,
            theme,
            chat_column_width,
            view_handle,
        ));

    div()
        .id(element_id)
        .w_full()
        .child(
            Collapsible::new()
                .open(expanded)
                .w_full()
                .child(header)
                .content(details),
        )
        .into_any_element()
}

fn render_codex_item(
    item: &CodexThreadItem,
    theme: &gpui_component::Theme,
    in_progress: bool,
    expanded_items: &HashSet<String>,
    chat_column_width: Option<Pixels>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    if !in_progress && let CodexThreadItem::AgentMessage { id, text } = item {
        let wrap_width = chat_column_width.map(|w| (w - px(32.0)).max(px(0.0)));
        let message = chat_message_view(
            &format!("agent-message-{id}"),
            text,
            wrap_width,
            theme.foreground,
        );
        let debug_id = format!("conversation-agent-message-{id}");
        return div()
            .debug_selector(move || debug_id.clone())
            .id(format!("codex-agent-message-{id}"))
            .w_full()
            .overflow_x_hidden()
            .px_2()
            .py_1()
            .flex()
            .flex_col()
            .child(min_width_zero(
                div().w_full().whitespace_normal().child(message),
            ))
            .into_any_element();
    }

    let id = codex_item_id(item);
    let always_expanded = matches!(item, CodexThreadItem::AgentMessage { .. });
    let expanded = always_expanded || expanded_items.contains(id);

    let (title, summary) = codex_item_summary(item, in_progress);

    let toggle_button = if always_expanded {
        None
    } else {
        let view_handle = view_handle.clone();
        let id = id.to_owned();
        let icon = if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        let tooltip = if expanded { "Hide" } else { "Show" };
        Some(
            Button::new(format!("agent-item-toggle-{id}"))
                .ghost()
                .compact()
                .icon(icon)
                .tooltip(tooltip)
                .on_click(move |_, _, app| {
                    let _ = view_handle.update(app, |view, cx| {
                        view.toggle_agent_item_expanded(&id);
                        cx.notify();
                    });
                }),
        )
    };

    if in_progress && !expanded && !always_expanded {
        let (label, text) = codex_item_compact_summary(item);
        let item_icon = if matches!(item, CodexThreadItem::Reasoning { .. }) {
            Spinner::new()
                .with_size(Size::Small)
                .color(theme.muted_foreground)
                .into_any_element()
        } else {
            Icon::empty()
                .path(codex_item_icon_path(item))
                .with_size(Size::Small)
                .text_color(theme.muted_foreground)
                .into_any_element()
        };
        return div()
            .id(format!("codex-compact-{id}"))
            .h(px(28.0))
            .w_full()
            .px_1()
            .flex()
            .items_center()
            .gap_2()
            .child(item_icon)
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child(format!("{label}:")),
            )
            .child(min_width_zero(
                div()
                    .flex_1()
                    .truncate()
                    .text_color(theme.muted_foreground)
                    .child(text),
            ))
            .when_some(toggle_button, |s, b| s.child(b))
            .into_any_element();
    }

    let header = div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .child(min_width_zero(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_color(theme.muted_foreground).child(title))
                .child(min_width_zero(div().truncate().child(summary))),
        ))
        .when_some(toggle_button, |s, b| s.child(b));

    div()
        .id(format!("codex-item-{id}"))
        .w_full()
        .child(
            Collapsible::new()
                .open(expanded)
                .w_full()
                .p_2()
                .rounded_md()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .child(header)
                .content(render_codex_item_details(
                    item,
                    theme,
                    chat_column_width,
                    view_handle,
                )),
        )
        .into_any_element()
}

fn render_codex_item_details(
    item: &CodexThreadItem,
    theme: &gpui_component::Theme,
    chat_column_width: Option<Pixels>,
    _view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    match item {
        CodexThreadItem::AgentMessage { id, text } => {
            let wrap_width = chat_column_width.map(|w| (w - px(80.0)).max(px(0.0)));
            let message = chat_message_view(
                &format!("agent-message-{id}-details"),
                text,
                wrap_width,
                theme.foreground,
            );
            div()
                .mt_2()
                .w_full()
                .overflow_x_hidden()
                .child(min_width_zero(
                    div().w_full().whitespace_normal().child(message),
                ))
                .into_any_element()
        }
        CodexThreadItem::Reasoning { id, text } => {
            let wrap_width = chat_column_width.map(|w| (w - px(80.0)).max(px(0.0)));
            let message = chat_message_view(
                &format!("reasoning-{id}-details"),
                text,
                wrap_width,
                theme.muted_foreground,
            );
            div()
                .mt_2()
                .w_full()
                .overflow_x_hidden()
                .child(min_width_zero(
                    div().w_full().whitespace_normal().child(message),
                ))
                .into_any_element()
        }
        CodexThreadItem::CommandExecution {
            id,
            command,
            aggregated_output,
            exit_code,
            ..
        } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .flex()
            .flex_col()
            .gap_2()
            .child(min_width_zero(
                div()
                    .w_full()
                    .overflow_x_hidden()
                    .whitespace_normal()
                    .child(
                        chat_markdown_view(
                            &format!("command-{id}-details"),
                            &fenced_code_block("sh", command),
                            chat_column_width.map(|w| (w - px(80.0)).max(px(0.0))),
                        )
                        .text_color(theme.foreground),
                    ),
            ))
            .when(!aggregated_output.trim().is_empty(), |s| {
                s.child(min_width_zero(
                    div()
                        .w_full()
                        .overflow_x_hidden()
                        .whitespace_normal()
                        .child(
                            chat_markdown_view(
                                &format!("command-{id}-output"),
                                &fenced_code_block("", aggregated_output),
                                chat_column_width.map(|w| (w - px(80.0)).max(px(0.0))),
                            )
                            .text_color(theme.muted_foreground),
                        ),
                ))
            })
            .when_some(*exit_code, |s, code| {
                s.child(
                    div()
                        .whitespace_normal()
                        .text_color(theme.muted_foreground)
                        .child(format!("Exit: {code}")),
                )
            })
            .into_any_element(),
        CodexThreadItem::FileChange { changes, .. } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .flex()
            .flex_col()
            .gap_1()
            .children(changes.iter().map(|c| {
                div()
                    .w_full()
                    .overflow_x_hidden()
                    .whitespace_normal()
                    .text_color(theme.muted_foreground)
                    .child(format!("{:?}: {}", c.kind, c.path))
            }))
            .into_any_element(),
        CodexThreadItem::TodoList { items, .. } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .flex()
            .flex_col()
            .gap_1()
            .children(items.iter().map(|i| {
                let prefix = if i.completed { "[x]" } else { "[ ]" };
                div()
                    .w_full()
                    .overflow_x_hidden()
                    .whitespace_normal()
                    .text_color(theme.muted_foreground)
                    .child(format!("{prefix} {}", i.text))
            }))
            .into_any_element(),
        CodexThreadItem::WebSearch { query, .. } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .child(div().whitespace_normal().child(query.clone()))
            .into_any_element(),
        CodexThreadItem::McpToolCall {
            server,
            tool,
            status,
            ..
        } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().whitespace_normal().child(format!("{server}::{tool}")))
            .child(
                div()
                    .whitespace_normal()
                    .text_color(theme.muted_foreground)
                    .child(format!("{status:?}")),
            )
            .into_any_element(),
        CodexThreadItem::Error { message, .. } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .text_color(theme.danger_foreground)
            .child(message.clone())
            .into_any_element(),
    }
}

fn chat_markdown_view(id: &str, source: &str, wrap_width: Option<Pixels>) -> TextView {
    let mut code_block_style = gpui::StyleRefinement::default();
    code_block_style.size.width = Some(gpui::relative(1.).into());
    code_block_style.max_size.width = Some(gpui::relative(1.).into());
    code_block_style.min_size.width = Some(px(0.0).into());

    let mut view = TextView::markdown(
        ElementId::Name(SharedString::from(format!("{id}-markdown"))),
        source.to_owned(),
    )
    .style(
        TextViewStyle::default()
            .paragraph_gap(rems(0.5))
            .code_block(code_block_style),
    )
    .text_size(px(16.0))
    .whitespace_normal()
    .flex()
    .flex_col();

    gpui::Styled::style(&mut view).align_items = Some(gpui::AlignItems::Stretch);

    if let Some(wrap_width) = wrap_width {
        view.w(wrap_width)
    } else {
        view
    }
}

fn fenced_code_block(lang: &str, code: &str) -> String {
    let mut max_ticks = 0usize;
    let mut current = 0usize;
    for ch in code.chars() {
        if ch == '`' {
            current += 1;
            max_ticks = max_ticks.max(current);
        } else {
            current = 0;
        }
    }

    let fence_len = (max_ticks + 1).max(3);
    let fence = "`".repeat(fence_len);

    if lang.is_empty() {
        format!("{fence}\n{code}\n{fence}")
    } else {
        format!("{fence}{lang}\n{code}\n{fence}")
    }
}

fn chat_message_view(
    id: &str,
    source: &str,
    wrap_width: Option<Pixels>,
    text_color: gpui::Hsla,
) -> AnyElement {
    let markdown_like = source.contains("```")
        || source.contains("**")
        || source.contains('`')
        || source.contains("](")
        || source
            .lines()
            .any(|line| line.starts_with("# ") || line.starts_with("- ") || line.starts_with("* "));

    if markdown_like {
        return chat_markdown_view(id, source, wrap_width)
            .text_color(text_color)
            .into_any_element();
    }

    let mut container = div()
        .id(ElementId::Name(SharedString::from(format!("{id}-text"))))
        .text_size(px(16.0))
        .whitespace_normal()
        .text_color(text_color)
        .child(source.to_owned());

    if let Some(wrap_width) = wrap_width {
        container = container.w(wrap_width);
    }

    container.into_any_element()
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

fn codex_item_summary(item: &CodexThreadItem, in_progress: bool) -> (&'static str, String) {
    let progress_suffix = if in_progress { " (in progress)" } else { "" };
    match item {
        CodexThreadItem::AgentMessage { text, .. } => {
            ("Agent", text.lines().next().unwrap_or("").to_owned())
        }
        CodexThreadItem::Reasoning { text, .. } => (
            if in_progress {
                "Reasoning (in progress)"
            } else {
                "Reasoning"
            },
            if text.trim().is_empty() {
                "…".to_owned()
            } else {
                collapse_inline_markdown_for_summary(text.lines().next().unwrap_or(""))
            },
        ),
        CodexThreadItem::CommandExecution {
            command, status, ..
        } => (
            "Command",
            format!(
                "{status:?}{progress_suffix}: {}",
                command.lines().next().unwrap_or("")
            ),
        ),
        CodexThreadItem::FileChange {
            changes, status, ..
        } => (
            "File change",
            format!("{status:?}{progress_suffix}: {} file(s)", changes.len()),
        ),
        CodexThreadItem::McpToolCall {
            server,
            tool,
            status,
            ..
        } => (
            "MCP tool call",
            format!("{status:?}{progress_suffix}: {server}::{tool}"),
        ),
        CodexThreadItem::WebSearch { query, .. } => (
            "Web search",
            format!(
                "{}{}",
                progress_suffix,
                if query.is_empty() { "" } else { ": " }
            ) + query,
        ),
        CodexThreadItem::TodoList { items, .. } => (
            "Todo list",
            format!("{progress_suffix}: {} item(s)", items.len()),
        ),
        CodexThreadItem::Error { message, .. } => ("Error", message.clone()),
    }
}

fn collapse_inline_markdown_for_summary(text: &str) -> String {
    text.replace("**", "")
        .replace("__", "")
        .replace("`", "")
        .replace('*', "")
        .trim()
        .to_owned()
}

fn codex_item_compact_summary(item: &CodexThreadItem) -> (&'static str, String) {
    match item {
        CodexThreadItem::AgentMessage { text, .. } => {
            ("Agent", text.lines().next().unwrap_or("").to_owned())
        }
        CodexThreadItem::Reasoning { text, .. } => {
            let summary = if text.trim().is_empty() {
                "…".to_owned()
            } else {
                collapse_inline_markdown_for_summary(text.lines().next().unwrap_or(""))
            };
            ("Thinking", summary)
        }
        CodexThreadItem::CommandExecution { command, .. } => {
            ("Bash", command.lines().next().unwrap_or("").to_owned())
        }
        CodexThreadItem::FileChange { changes, .. } => {
            ("Patch", format!("{} file(s)", changes.len()))
        }
        CodexThreadItem::McpToolCall { server, tool, .. } => ("MCP", format!("{server}::{tool}")),
        CodexThreadItem::WebSearch { query, .. } => ("Search", query.clone()),
        CodexThreadItem::TodoList { items, .. } => ("Todo", format!("{} item(s)", items.len())),
        CodexThreadItem::Error { message, .. } => ("Error", message.clone()),
    }
}

fn codex_item_icon_path(item: &CodexThreadItem) -> SharedString {
    match item {
        CodexThreadItem::AgentMessage { .. } => IconName::Bot.path(),
        CodexThreadItem::Reasoning { .. } => "icons/brain.svg".into(),
        CodexThreadItem::CommandExecution { .. } => IconName::SquareTerminal.path(),
        CodexThreadItem::FileChange { .. } => IconName::File.path(),
        CodexThreadItem::McpToolCall { .. } => IconName::Settings2.path(),
        CodexThreadItem::WebSearch { .. } => IconName::Globe.path(),
        CodexThreadItem::TodoList { .. } => IconName::Check.path(),
        CodexThreadItem::Error { .. } => IconName::TriangleAlert.path(),
    }
}

fn codex_item_is_tool_call(item: &CodexThreadItem) -> bool {
    matches!(
        item,
        CodexThreadItem::CommandExecution { .. }
            | CodexThreadItem::FileChange { .. }
            | CodexThreadItem::McpToolCall { .. }
            | CodexThreadItem::WebSearch { .. }
    )
}

fn render_turn_duration_row(
    theme: &gpui_component::Theme,
    elapsed: Duration,
    in_progress: bool,
) -> AnyElement {
    let icon = if in_progress {
        Spinner::new()
            .with_size(Size::Small)
            .color(theme.muted_foreground)
            .into_any_element()
    } else {
        Icon::empty()
            .path("icons/timer.svg")
            .with_size(Size::Small)
            .text_color(theme.muted_foreground)
            .into_any_element()
    };
    div()
        .h(px(24.0))
        .w_full()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .text_color(theme.muted_foreground)
        .child(icon)
        .child(
            div()
                .flex_1()
                .truncate()
                .child(format_duration_compact(elapsed)),
        )
        .into_any_element()
}

fn format_duration_compact(duration: Duration) -> String {
    let ms = duration.as_millis() as u64;
    let secs = ms / 1000;

    if secs < 60 {
        let tenths = (ms % 1000) / 100;
        if secs == 0 && tenths == 0 {
            return "0.0s".to_owned();
        }
        return format!("{secs}.{tenths}s");
    }

    let mins = secs / 60;
    let rem_secs = secs % 60;
    if mins < 60 {
        return format!("{mins}m{rem_secs:02}s");
    }

    let hours = mins / 60;
    let rem_mins = mins % 60;
    format!("{hours}h{rem_mins:02}m")
}

fn workspace_context(state: &AppState, workspace_id: WorkspaceId) -> Option<(PathBuf, PathBuf)> {
    for project in &state.projects {
        for workspace in &project.workspaces {
            if workspace.id == workspace_id && workspace.status == WorkspaceStatus::Active {
                return Some((project.path.clone(), workspace.worktree_path.clone()));
            }
        }
    }
    None
}

struct WorkspaceAgentContext {
    project_slug: String,
    workspace_name: String,
    worktree_path: PathBuf,
}

fn workspace_agent_context(
    state: &AppState,
    workspace_id: WorkspaceId,
) -> Option<WorkspaceAgentContext> {
    for project in &state.projects {
        for workspace in &project.workspaces {
            if workspace.id == workspace_id && workspace.status == WorkspaceStatus::Active {
                return Some(WorkspaceAgentContext {
                    project_slug: project.slug.clone(),
                    workspace_name: workspace.workspace_name.clone(),
                    worktree_path: workspace.worktree_path.clone(),
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Modifiers, px, size};
    use luban_domain::ConversationEntry;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn agent_turn_summary_uses_thinking_label_and_omits_messages() {
        let summary = format_agent_turn_summary(TurnSummaryCounts {
            tool_calls: 2,
            reasonings: 3,
        });
        assert_eq!(summary, "2 tool calls, 3 thinking");
        assert!(!summary.contains("message"));
        assert!(!summary.contains("reasoning"));
    }

    #[derive(Default)]
    struct FakeService;

    impl ProjectWorkspaceService for FakeService {
        fn load_app_state(&self) -> Result<PersistedAppState, String> {
            Ok(PersistedAppState {
                projects: Vec::new(),
            })
        }

        fn save_app_state(&self, _snapshot: PersistedAppState) -> Result<(), String> {
            Ok(())
        }

        fn create_workspace(
            &self,
            _project_path: PathBuf,
            _project_slug: String,
        ) -> Result<CreatedWorkspace, String> {
            Ok(CreatedWorkspace {
                workspace_name: "abandon-about".to_owned(),
                branch_name: "luban/abandon-about".to_owned(),
                worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
            })
        }

        fn archive_workspace(
            &self,
            _project_path: PathBuf,
            _worktree_path: PathBuf,
        ) -> Result<(), String> {
            Ok(())
        }

        fn ensure_conversation(
            &self,
            _project_slug: String,
            _workspace_name: String,
        ) -> Result<(), String> {
            Ok(())
        }

        fn load_conversation(
            &self,
            _project_slug: String,
            _workspace_name: String,
        ) -> Result<ConversationSnapshot, String> {
            Ok(ConversationSnapshot {
                thread_id: None,
                entries: Vec::new(),
            })
        }

        fn run_agent_turn_streamed(
            &self,
            request: RunAgentTurnRequest,
            _cancel: Arc<AtomicBool>,
            on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync>,
        ) -> Result<(), String> {
            let thread_id = request.thread_id.unwrap_or_else(|| "thread-1".to_owned());
            on_event(CodexThreadEvent::ThreadStarted {
                thread_id: thread_id.clone(),
            });
            on_event(CodexThreadEvent::ItemStarted {
                item: CodexThreadItem::CommandExecution {
                    id: "cmd-1".to_owned(),
                    command: "echo hello".to_owned(),
                    aggregated_output: "".to_owned(),
                    exit_code: None,
                    status: luban_domain::CodexCommandExecutionStatus::InProgress,
                },
            });
            on_event(CodexThreadEvent::ItemCompleted {
                item: CodexThreadItem::AgentMessage {
                    id: "item-1".to_owned(),
                    text: format!("Echo: {}", request.prompt),
                },
            });
            on_event(CodexThreadEvent::TurnCompleted {
                usage: luban_domain::CodexUsage {
                    input_tokens: 1,
                    cached_input_tokens: 0,
                    output_tokens: 1,
                },
            });
            Ok(())
        }
    }

    #[test]
    fn compact_item_summary_is_stable() {
        let item = CodexThreadItem::CommandExecution {
            id: "cmd-1".to_owned(),
            command: "echo hello\necho world".to_owned(),
            aggregated_output: String::new(),
            exit_code: None,
            status: luban_domain::CodexCommandExecutionStatus::InProgress,
        };
        assert_eq!(
            codex_item_compact_summary(&item),
            ("Bash", "echo hello".to_owned())
        );

        let item = CodexThreadItem::Reasoning {
            id: "r-1".to_owned(),
            text: "\n".to_owned(),
        };
        assert_eq!(
            codex_item_compact_summary(&item),
            ("Thinking", "…".to_owned())
        );
    }

    #[test]
    fn codex_item_icon_paths_are_stable() {
        let item = CodexThreadItem::Reasoning {
            id: "r-1".to_owned(),
            text: "x".to_owned(),
        };
        assert_eq!(codex_item_icon_path(&item).as_ref(), "icons/brain.svg");
    }

    #[test]
    fn duration_format_is_compact() {
        assert_eq!(
            format_duration_compact(Duration::from_millis(1234)),
            "1.2s".to_owned()
        );
        assert_eq!(
            format_duration_compact(Duration::from_secs(62)),
            "1m02s".to_owned()
        );
    }

    #[test]
    fn main_pane_title_tracks_selected_context() {
        let mut state = AppState::new();
        assert_eq!(main_pane_title(&state, MainPane::None), String::new());

        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        let project_name = state.projects[0].name.clone();

        assert_eq!(
            main_pane_title(&state, MainPane::ProjectSettings(project_id)),
            project_name
        );

        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;

        assert_eq!(
            main_pane_title(&state, MainPane::Workspace(workspace_id)),
            "abandon-about".to_owned()
        );
    }

    #[gpui::test]
    async fn clicking_project_header_toggles_expanded(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::ToggleProjectExpanded { project_id });

        let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        cx.refresh().unwrap();

        let bounds = cx
            .debug_bounds("project-header-0")
            .expect("missing debug bounds for project-header-0");
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.refresh().unwrap();

        let expanded = view.read_with(cx, |v, _| v.debug_state().projects[0].expanded);
        assert!(!expanded);
    }

    #[gpui::test]
    async fn archiving_workspace_shows_prompt_and_updates_state(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::ToggleProjectExpanded { project_id });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });

        let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        cx.refresh().unwrap();

        let row_bounds = cx
            .debug_bounds("workspace-row-0-0")
            .expect("missing debug bounds for workspace-row-0-0");
        cx.simulate_mouse_move(row_bounds.center(), None, Modifiers::none());
        cx.refresh().unwrap();

        let bounds = cx
            .debug_bounds("workspace-archive-0-0")
            .expect("missing debug bounds for workspace-archive-0-0");
        cx.simulate_click(bounds.center(), Modifiers::none());
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        cx.refresh().unwrap();

        let status = view.read_with(cx, |v, _| v.debug_state().projects[0].workspaces[0].status);
        assert_eq!(status, WorkspaceStatus::Active);

        let row_bounds = cx
            .debug_bounds("workspace-row-0-0")
            .expect("missing debug bounds for workspace-row-0-0");
        cx.simulate_mouse_move(row_bounds.center(), None, Modifiers::none());
        cx.refresh().unwrap();

        let bounds = cx
            .debug_bounds("workspace-archive-0-0")
            .expect("missing debug bounds for workspace-archive-0-0");
        cx.simulate_click(bounds.center(), Modifiers::none());
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Archive");
        cx.run_until_parked();
        cx.refresh().unwrap();

        let status = view.read_with(cx, |v, _| v.debug_state().projects[0].workspaces[0].status);
        assert_eq!(status, WorkspaceStatus::Archived);
    }

    #[gpui::test]
    async fn markdown_messages_render_in_workspace(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        state.main_pane = MainPane::Workspace(workspace_id);
        state.apply(Action::ConversationLoaded {
            workspace_id,
            snapshot: ConversationSnapshot {
                thread_id: Some("thread-1".to_owned()),
                entries: vec![
                    ConversationEntry::UserMessage {
                        text: "Hello **world**\n\n- a\n- b\n\n`inline`".to_owned(),
                    },
                    ConversationEntry::CodexItem {
                        item: Box::new(CodexThreadItem::AgentMessage {
                            id: "item-1".to_owned(),
                            text: "Reply:\n\n- one\n- two\n\n[gpui](https://example.com)"
                                .to_owned(),
                        }),
                    },
                ],
            },
        });

        let (_, window_cx) =
            cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        window_cx.refresh().unwrap();

        let bounds = window_cx
            .debug_bounds("conversation-agent-message-item-1")
            .expect("missing debug bounds for conversation-agent-message-item-1");
        assert!(bounds.size.height > px(0.0));
    }

    #[gpui::test]
    async fn clicking_turn_summary_row_toggles_expanded(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        state.main_pane = MainPane::Workspace(workspace_id);
        state.apply(Action::ConversationLoaded {
            workspace_id,
            snapshot: ConversationSnapshot {
                thread_id: Some("thread-1".to_owned()),
                entries: vec![
                    ConversationEntry::UserMessage {
                        text: "Test".to_owned(),
                    },
                    ConversationEntry::CodexItem {
                        item: Box::new(CodexThreadItem::CommandExecution {
                            id: "item-1".to_owned(),
                            command: "echo hello".to_owned(),
                            aggregated_output: "hello".to_owned(),
                            exit_code: Some(0),
                            status: luban_domain::CodexCommandExecutionStatus::Completed,
                        }),
                    },
                    ConversationEntry::CodexItem {
                        item: Box::new(CodexThreadItem::AgentMessage {
                            id: "item-2".to_owned(),
                            text: "Reply".to_owned(),
                        }),
                    },
                ],
            },
        });

        let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        cx.refresh().unwrap();

        let expanded = view.read_with(cx, |v, _| v.expanded_agent_turns.contains("agent-turn-0"));
        assert!(!expanded);

        let row_bounds = cx
            .debug_bounds("agent-turn-summary-agent-turn-0")
            .expect("missing debug bounds for agent-turn-summary-agent-turn-0");
        cx.simulate_click(row_bounds.center(), Modifiers::none());
        cx.refresh().unwrap();

        let expanded = view.read_with(cx, |v, _| v.expanded_agent_turns.contains("agent-turn-0"));
        assert!(expanded);
    }

    #[gpui::test]
    async fn clicking_turn_item_summary_row_toggles_expanded(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        state.main_pane = MainPane::Workspace(workspace_id);
        state.apply(Action::ConversationLoaded {
            workspace_id,
            snapshot: ConversationSnapshot {
                thread_id: Some("thread-1".to_owned()),
                entries: vec![
                    ConversationEntry::UserMessage {
                        text: "Test".to_owned(),
                    },
                    ConversationEntry::CodexItem {
                        item: Box::new(CodexThreadItem::Reasoning {
                            id: "item-1".to_owned(),
                            text: "Reasoning details".to_owned(),
                        }),
                    },
                    ConversationEntry::CodexItem {
                        item: Box::new(CodexThreadItem::AgentMessage {
                            id: "item-2".to_owned(),
                            text: "Reply".to_owned(),
                        }),
                    },
                ],
            },
        });

        let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        cx.refresh().unwrap();

        let row_bounds = cx
            .debug_bounds("agent-turn-summary-agent-turn-0")
            .expect("missing debug bounds for agent-turn-summary-agent-turn-0");
        cx.simulate_click(row_bounds.center(), Modifiers::none());
        cx.refresh().unwrap();

        let expanded = view.read_with(cx, |v, _| v.expanded_agent_turns.contains("agent-turn-0"));
        assert!(expanded);

        let item_bounds = cx
            .debug_bounds("agent-turn-item-summary-agent-turn-0-item-1")
            .expect("missing debug bounds for agent-turn-item-summary-agent-turn-0-item-1");
        cx.simulate_click(item_bounds.center(), Modifiers::none());
        cx.refresh().unwrap();

        let expanded = view.read_with(cx, |v, _| {
            v.expanded_agent_items.contains("agent-turn-0::item-1")
        });
        assert!(expanded);
    }

    #[gpui::test]
    async fn turn_summary_includes_error_items(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        state.main_pane = MainPane::Workspace(workspace_id);
        state.apply(Action::ConversationLoaded {
            workspace_id,
            snapshot: ConversationSnapshot {
                thread_id: Some("thread-1".to_owned()),
                entries: vec![
                    ConversationEntry::UserMessage {
                        text: "Test".to_owned(),
                    },
                    ConversationEntry::CodexItem {
                        item: Box::new(CodexThreadItem::Error {
                            id: "err-1".to_owned(),
                            message: "reconnecting ...1/5".to_owned(),
                        }),
                    },
                ],
            },
        });

        let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        cx.refresh().unwrap();

        let row_bounds = cx
            .debug_bounds("agent-turn-summary-agent-turn-0")
            .expect("missing debug bounds for agent-turn-summary-agent-turn-0");
        cx.simulate_click(row_bounds.center(), Modifiers::none());
        cx.refresh().unwrap();

        let expanded = view.read_with(cx, |v, _| v.expanded_agent_turns.contains("agent-turn-0"));
        assert!(expanded);

        let _ = cx
            .debug_bounds("agent-turn-item-summary-agent-turn-0-err-1")
            .expect("missing debug bounds for agent-turn-item-summary-agent-turn-0-err-1");
    }

    #[gpui::test]
    async fn user_message_reflows_on_window_resize(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        state.main_pane = MainPane::Workspace(workspace_id);

        let long_text = std::iter::repeat_n("word", 200)
            .collect::<Vec<_>>()
            .join(" ");
        state.apply(Action::ConversationLoaded {
            workspace_id,
            snapshot: ConversationSnapshot {
                thread_id: Some("thread-1".to_owned()),
                entries: vec![ConversationEntry::UserMessage { text: long_text }],
            },
        });

        let (_view, window_cx) =
            cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));

        window_cx.simulate_resize(size(px(1200.0), px(800.0)));
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
        let wide_column = window_cx
            .debug_bounds("workspace-chat-column")
            .expect("missing debug bounds for workspace-chat-column");
        let wide_bubble = window_cx
            .debug_bounds("conversation-user-bubble-0")
            .expect("missing debug bounds for conversation-user-bubble-0");

        window_cx.simulate_resize(size(px(520.0), px(800.0)));
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
        let narrow_column = window_cx
            .debug_bounds("workspace-chat-column")
            .expect("missing debug bounds for workspace-chat-column");
        let narrow_bubble = window_cx
            .debug_bounds("conversation-user-bubble-0")
            .expect("missing debug bounds for conversation-user-bubble-0");

        assert!(narrow_column.size.width < wide_column.size.width);
        assert!(
            narrow_bubble.size.height > wide_bubble.size.height,
            "wide={:?} narrow={:?}",
            wide_bubble.size,
            narrow_bubble.size
        );
        assert!(narrow_bubble.right() <= narrow_column.right() + px(2.0));
        assert!(narrow_bubble.right() >= narrow_column.right() - px(8.0));
    }

    #[gpui::test]
    async fn short_user_message_does_not_fill_max_width(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        state.main_pane = MainPane::Workspace(workspace_id);

        state.apply(Action::ConversationLoaded {
            workspace_id,
            snapshot: ConversationSnapshot {
                thread_id: Some("thread-1".to_owned()),
                entries: vec![ConversationEntry::UserMessage {
                    text: "Test".to_owned(),
                }],
            },
        });

        let (_, window_cx) =
            cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        window_cx.simulate_resize(size(px(1200.0), px(800.0)));
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();

        let bubble = window_cx
            .debug_bounds("conversation-user-bubble-0")
            .expect("missing debug bounds for conversation-user-bubble-0");
        assert!(bubble.size.width < px(300.0), "bubble={:?}", bubble.size);
    }

    #[gpui::test]
    async fn long_in_progress_reasoning_does_not_expand_chat_column(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        state.main_pane = MainPane::Workspace(workspace_id);

        state.apply(Action::SendAgentMessage {
            workspace_id,
            text: "Test".to_owned(),
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            event: CodexThreadEvent::ItemStarted {
                item: CodexThreadItem::Reasoning {
                    id: "item-1".to_owned(),
                    text: "a".repeat(16_384),
                },
            },
        });

        let (view, window_cx) =
            cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        window_cx.simulate_resize(size(px(720.0), px(800.0)));
        window_cx.refresh().unwrap();

        window_cx.update(|_, app| {
            view.update(app, |view, cx| {
                view.expanded_agent_items.insert("item-1".to_owned());
                cx.notify();
            });
        });
        window_cx.refresh().unwrap();

        let column = window_cx
            .debug_bounds("workspace-chat-column")
            .expect("missing debug bounds for workspace-chat-column");
        assert!(column.size.width <= px(720.0));

        let bubble = window_cx
            .debug_bounds("conversation-user-bubble-0")
            .expect("missing debug bounds for conversation-user-bubble-0");
        assert!(bubble.right() <= column.right() + px(2.0));
        assert!(bubble.right() >= column.right() - px(8.0));
    }

    #[gpui::test]
    async fn long_command_execution_does_not_expand_chat_column(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        state.main_pane = MainPane::Workspace(workspace_id);

        let long_command = format!(
            "bash -lc 'echo {} && echo \"{}\" && printf \"%s\" {}'",
            "a".repeat(4096),
            "b".repeat(4096),
            "c".repeat(4096)
        );
        let long_output = format!("{}\n{}", "x".repeat(4096), "y".repeat(4096));

        state.apply(Action::ConversationLoaded {
            workspace_id,
            snapshot: ConversationSnapshot {
                thread_id: Some("thread-1".to_owned()),
                entries: vec![
                    ConversationEntry::UserMessage {
                        text: "Test".to_owned(),
                    },
                    ConversationEntry::CodexItem {
                        item: Box::new(CodexThreadItem::CommandExecution {
                            id: "item-1".to_owned(),
                            command: long_command,
                            aggregated_output: long_output,
                            exit_code: Some(0),
                            status: luban_domain::CodexCommandExecutionStatus::Completed,
                        }),
                    },
                    ConversationEntry::CodexItem {
                        item: Box::new(CodexThreadItem::AgentMessage {
                            id: "item-2".to_owned(),
                            text: "Reply".to_owned(),
                        }),
                    },
                ],
            },
        });

        let (view, window_cx) =
            cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        window_cx.simulate_resize(size(px(720.0), px(800.0)));
        window_cx.refresh().unwrap();

        let turn_bounds = window_cx
            .debug_bounds("agent-turn-summary-agent-turn-0")
            .expect("missing debug bounds for agent-turn-summary-agent-turn-0");
        window_cx.simulate_click(turn_bounds.center(), Modifiers::none());
        window_cx.refresh().unwrap();

        let row_bounds = window_cx
            .debug_bounds("agent-turn-item-summary-agent-turn-0-item-1")
            .expect("missing debug bounds for agent-turn-item-summary-agent-turn-0-item-1");
        window_cx.simulate_click(row_bounds.center(), Modifiers::none());
        window_cx.refresh().unwrap();

        let expanded = view.read_with(window_cx, |v, _| {
            v.expanded_agent_items.contains("agent-turn-0::item-1")
        });
        assert!(expanded);

        let column = window_cx
            .debug_bounds("workspace-chat-column")
            .expect("missing debug bounds for workspace-chat-column");
        assert!(column.size.width <= px(720.0));

        let bubble = window_cx
            .debug_bounds("conversation-user-bubble-0")
            .expect("missing debug bounds for conversation-user-bubble-0");
        assert!(bubble.right() <= column.right() + px(2.0));
        assert!(bubble.right() >= column.right() - px(8.0));
    }

    #[gpui::test]
    async fn turn_duration_renders_below_messages(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = state.projects[0].workspaces[0].id;
        state.main_pane = MainPane::Workspace(workspace_id);
        state.apply(Action::ConversationLoaded {
            workspace_id,
            snapshot: ConversationSnapshot {
                thread_id: Some("thread-1".to_owned()),
                entries: vec![
                    ConversationEntry::UserMessage {
                        text: "Test".to_owned(),
                    },
                    ConversationEntry::CodexItem {
                        item: Box::new(CodexThreadItem::AgentMessage {
                            id: "item-1".to_owned(),
                            text: "Reply".to_owned(),
                        }),
                    },
                    ConversationEntry::TurnDuration { duration_ms: 6300 },
                ],
            },
        });

        let (_, window_cx) =
            cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
        window_cx.refresh().unwrap();

        let bounds = window_cx
            .debug_bounds("turn-duration-2")
            .expect("missing debug bounds for turn-duration-2");
        assert!(bounds.size.width > px(0.0));
    }

    #[gpui::test]
    async fn chat_input_draft_is_isolated_per_workspace(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w2".to_owned(),
            branch_name: "repo/w2".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w2"),
        });
        let w1 = state.projects[0].workspaces[0].id;
        let w2 = state.projects[0].workspaces[1].id;

        state.apply(Action::ChatDraftChanged {
            workspace_id: w1,
            text: "draft-1".to_owned(),
        });
        state.apply(Action::ChatDraftChanged {
            workspace_id: w2,
            text: "draft-2".to_owned(),
        });
        state.main_pane = MainPane::Workspace(w1);

        let view_slot: Arc<std::sync::Mutex<Option<gpui::Entity<LubanRootView>>>> =
            Arc::new(std::sync::Mutex::new(None));
        let view_slot_for_window = view_slot.clone();

        let (_, window_cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
            *view_slot_for_window.lock().expect("poisoned mutex") = Some(view.clone());
            gpui_component::Root::new(view, window, cx)
        });
        let view = view_slot
            .lock()
            .expect("poisoned mutex")
            .clone()
            .expect("missing view handle");
        window_cx.refresh().unwrap();

        let value = view.read_with(window_cx, |v, cx| {
            v.chat_input
                .as_ref()
                .map(|input| input.read(cx).value().to_string())
        });
        assert_eq!(value, Some("draft-1".to_owned()));

        window_cx.update(|_, app| {
            view.update(app, |view, cx| {
                view.dispatch(Action::OpenWorkspace { workspace_id: w2 }, cx);
            });
        });
        window_cx.refresh().unwrap();

        let value = view.read_with(window_cx, |v, cx| {
            v.chat_input
                .as_ref()
                .map(|input| input.read(cx).value().to_string())
        });
        assert_eq!(value, Some("draft-2".to_owned()));

        window_cx.update(|_, app| {
            view.update(app, |view, cx| {
                view.dispatch(Action::OpenWorkspace { workspace_id: w1 }, cx);
            });
        });
        window_cx.refresh().unwrap();

        let value = view.read_with(window_cx, |v, cx| {
            v.chat_input
                .as_ref()
                .map(|input| input.read(cx).value().to_string())
        });
        assert_eq!(value, Some("draft-1".to_owned()));
    }

    #[gpui::test]
    async fn chat_input_cursor_moves_to_end_on_workspace_switch(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);

        let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w2".to_owned(),
            branch_name: "repo/w2".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w2"),
        });
        let w1 = state.projects[0].workspaces[0].id;
        let w2 = state.projects[0].workspaces[1].id;

        state.apply(Action::ChatDraftChanged {
            workspace_id: w1,
            text: "draft-1".to_owned(),
        });
        state.apply(Action::ChatDraftChanged {
            workspace_id: w2,
            text: "draft-2".to_owned(),
        });
        state.main_pane = MainPane::Workspace(w1);

        let view_slot: Arc<std::sync::Mutex<Option<gpui::Entity<LubanRootView>>>> =
            Arc::new(std::sync::Mutex::new(None));
        let view_slot_for_window = view_slot.clone();

        let (_, window_cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
            *view_slot_for_window.lock().expect("poisoned mutex") = Some(view.clone());
            gpui_component::Root::new(view, window, cx)
        });
        let view = view_slot
            .lock()
            .expect("poisoned mutex")
            .clone()
            .expect("missing view handle");
        window_cx.refresh().unwrap();

        window_cx.update(|_, app| {
            view.update(app, |view, cx| {
                view.dispatch(Action::OpenWorkspace { workspace_id: w2 }, cx);
            });
        });
        window_cx.refresh().unwrap();

        let (value, cursor_at_end) = view.read_with(window_cx, |v, cx| {
            let Some(input) = v.chat_input.as_ref() else {
                return (None, false);
            };
            let state = input.read(cx);
            let value = state.value().to_string();
            let end = state.text().offset_to_position(state.text().len());
            (Some(value), state.cursor_position() == end)
        });
        assert_eq!(value, Some("draft-2".to_owned()));
        assert!(cursor_at_end);
    }
}
