use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, MouseButton, PromptButton, PromptLevel, Window, div, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, Size,
    button::*,
    collapsible::Collapsible,
    input::{Input, InputEvent, InputState},
};
use luban_domain::{
    Action, AppState, CodexThreadEvent, CodexThreadItem, ConversationSnapshot, Effect, MainPane,
    OperationStatus, ProjectId, WorkspaceId, WorkspaceStatus,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

pub struct CreatedWorkspace {
    pub workspace_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
}

pub trait ProjectWorkspaceService: Send + Sync {
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
        project_slug: String,
        workspace_name: String,
        worktree_path: PathBuf,
        thread_id: Option<String>,
        prompt: String,
        on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync>,
    ) -> Result<(), String>;
}

pub struct LubanRootView {
    state: AppState,
    services: Arc<dyn ProjectWorkspaceService>,
    chat_input: Option<gpui::Entity<InputState>>,
    expanded_agent_items: HashSet<String>,
    expanded_agent_turns: HashSet<String>,
    running_turn_started_at: HashMap<WorkspaceId, Instant>,
    running_turn_tickers: HashSet<WorkspaceId>,
    chat_scroll_handle: gpui::ScrollHandle,
    last_chat_workspace_id: Option<WorkspaceId>,
    last_chat_item_count: usize,
    _subscriptions: Vec<gpui::Subscription>,
}

impl LubanRootView {
    pub fn new(services: Arc<dyn ProjectWorkspaceService>, _cx: &mut Context<Self>) -> Self {
        Self {
            state: AppState::new(),
            services,
            chat_input: None,
            expanded_agent_items: HashSet::new(),
            expanded_agent_turns: HashSet::new(),
            running_turn_started_at: HashMap::new(),
            running_turn_tickers: HashSet::new(),
            chat_scroll_handle: gpui::ScrollHandle::new(),
            last_chat_workspace_id: None,
            last_chat_item_count: 0,
            _subscriptions: Vec::new(),
        }
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
            running_turn_started_at: HashMap::new(),
            running_turn_tickers: HashSet::new(),
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

    fn toggle_agent_turn_expanded(&mut self, id: &str) {
        if self.expanded_agent_turns.contains(id) {
            self.expanded_agent_turns.remove(id);
        } else {
            self.expanded_agent_turns.insert(id.to_owned());
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
        }
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

        let thread_id = self
            .state
            .workspace_conversation(workspace_id)
            .and_then(|c| c.thread_id.clone());
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
                        let result = services.run_agent_turn_streamed(
                            agent_context.project_slug,
                            agent_context.workspace_name,
                            agent_context.worktree_path,
                            thread_id,
                            text,
                            on_event,
                        );

                        if let Err(message) = result {
                            let _ = tx_for_error.send_blocking(CodexThreadEvent::Error { message });
                        }
                    });

                    drop(tx);

                    while let Ok(event) = rx.recv().await {
                        let _ = this.update(
                            &mut async_cx,
                            |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                                view.dispatch(
                                    Action::AgentEventReceived {
                                        workspace_id,
                                        event,
                                    },
                                    view_cx,
                                )
                            },
                        );
                    }

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(Action::AgentTurnFinished { workspace_id }, view_cx)
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
        let sidebar_width = px(340.0);

        div()
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_sm()
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
                .h(px(44.0))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(theme.sidebar_border)
                .child(div().child("Projects"))
                .child(
                    div()
                        .debug_selector(|| "add-project".to_owned())
                        .child(
                            Button::new("add-project")
                                .ghost()
                                .compact()
                                .label("+")
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
                                }),
                        ),
                ),
        )
        .child(
            div()
                .flex_1()
                .id("projects-scroll")
                .overflow_scroll()
                .py_2()
                .children(state.projects.iter().enumerate().map(|(i, project)| {
                    render_project(cx, i, project, state.main_pane)
                })),
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

    let disclosure = if project.expanded { "▾" } else { "▸" };
    let create_label = match project.create_workspace_status {
        OperationStatus::Idle => "+",
        OperationStatus::Running => "…",
    };

    let header = div()
        .px_2()
        .py_2()
        .flex()
        .items_center()
        .gap_2()
        .bg(if is_selected {
            theme.sidebar_accent
        } else {
            theme.sidebar
        })
        .hover(move |s| s.bg(theme.sidebar_accent))
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
                        let project_id = project.id;
                        move |this, _, _, cx| {
                            this.dispatch(Action::ToggleProjectExpanded { project_id }, cx)
                        }
                    }),
                )
                .child(
                    div()
                        .w(px(16.0))
                        .text_color(theme.muted_foreground)
                        .debug_selector(move || format!("project-toggle-{project_index}"))
                        .child(disclosure),
                )
                .child(div().child(project.name.clone())),
        )
        .child(
            div()
                .debug_selector(move || format!("project-create-workspace-{project_index}"))
                .child(
                    Button::new(format!("project-create-workspace-{project_index}"))
                        .ghost()
                        .compact()
                        .disabled(matches!(
                            project.create_workspace_status,
                            OperationStatus::Running
                        ))
                        .label(create_label)
                        .on_click({
                            let view_handle = view_handle.clone();
                            let project_id = project.id;
                            move |_, _, app| {
                                let _ = view_handle.update(app, |view, cx| {
                                    view.dispatch(Action::CreateWorkspace { project_id }, cx);
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
                        .label("⋯")
                        .on_click({
                            let view_handle = view_handle.clone();
                            let project_id = project.id;
                            move |_, _, app| {
                                let _ = view_handle.update(app, |view, cx| {
                                    view.dispatch(Action::OpenProjectSettings { project_id }, cx);
                                });
                            }
                        }),
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

    let row = div()
        .px_2()
        .py_1()
        .flex()
        .items_center()
        .gap_2()
        .bg(if is_selected {
            theme.sidebar_accent
        } else {
            theme.sidebar
        })
        .hover(move |s| s.bg(theme.sidebar_accent))
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
                .child(div().flex_1().child(workspace.workspace_name.clone()))
                .child(div().text_color(theme.muted_foreground).child("—")),
        )
        .child(
            div()
                .debug_selector(move || {
                    format!("workspace-archive-{project_index}-{workspace_index}")
                })
                .child(
                    Button::new(format!("workspace-archive-{project_index}-{workspace_index}"))
                        .danger()
                        .compact()
                        .disabled(archive_disabled)
                        .label(if archive_disabled { "…" } else { "Archive" })
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

        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Message..."));

        let subscription = cx.subscribe_in(&input_state, window, {
            let input_state = input_state.clone();
            move |this: &mut LubanRootView, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary: false } = ev {
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
            }
        });

        self._subscriptions.push(subscription);
        self.chat_input = Some(input_state.clone());
        input_state
    }

    fn render_main(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let view_handle = cx.entity().downgrade();

        let content = match self.state.main_pane {
            MainPane::None => {
                self.last_chat_workspace_id = None;
                self.last_chat_item_count = 0;

                div()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().child("Welcome"))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("Select a workspace to begin."),
                    )
                    .into_any_element()
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
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().child(title))
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
                let theme = cx.theme();

                let conversation = self.state.workspace_conversation(workspace_id);
                let entries: &[luban_domain::ConversationEntry] =
                    conversation.map(|c| c.entries.as_slice()).unwrap_or(&[]);
                let in_progress_items: Vec<&CodexThreadItem> = conversation
                    .map(|c| c.in_progress_items.values().collect())
                    .unwrap_or_default();
                let run_status = conversation
                    .map(|c| c.run_status)
                    .unwrap_or(OperationStatus::Idle);
                let _thread_id = conversation.and_then(|c| c.thread_id.as_deref());

                let is_running = run_status == OperationStatus::Running;
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

                let workspace_changed = self.last_chat_workspace_id != Some(workspace_id);
                let item_count_increased =
                    !workspace_changed && rendered_item_count > self.last_chat_item_count;
                if workspace_changed || (item_count_increased && pinned_to_bottom) {
                    self.chat_scroll_handle.scroll_to_bottom();
                }
                self.last_chat_workspace_id = Some(workspace_id);
                self.last_chat_item_count = rendered_item_count;

                let history = div()
                    .flex_1()
                    .id("workspace-chat-scroll")
                    .overflow_scroll()
                    .track_scroll(&self.chat_scroll_handle)
                    .overflow_x_hidden()
                    .w_full()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when_some(running_elapsed, |s, elapsed| {
                        s.child(render_running_status_row(theme, elapsed))
                    })
                    .children(history_children)
                    .children(in_progress_items.into_iter().map({
                        let view_handle = view_handle.clone();
                        let expanded = expanded.clone();
                        move |item| render_codex_item(item, theme, true, &expanded, &view_handle)
                    }))
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
                                    Icon::new(IconName::LoaderCircle)
                                        .with_size(Size::Small)
                                        .text_color(theme.muted_foreground),
                                )
                                .child(div().text_color(theme.muted_foreground).child("Thinking:"))
                                .child(
                                    div()
                                        .flex_1()
                                        .truncate()
                                        .text_color(theme.muted_foreground)
                                        .child("…"),
                                ),
                        )
                    });

                let composer = div()
                    .p_3()
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&input_state)))
                    .child(
                        Button::new("agent-send")
                            .compact()
                            .label("Send")
                            .disabled(is_running)
                            .on_click({
                                let input_state = input_state.clone();
                                move |_, window, app| {
                                    let text = input_state.read(app).value().trim().to_owned();
                                    if text.is_empty() {
                                        return;
                                    }
                                    input_state
                                        .update(app, |state, cx| state.set_value("", window, cx));
                                    let _ = view_handle.update(app, |view, cx| {
                                        view.dispatch(
                                            Action::SendAgentMessage { workspace_id, text },
                                            cx,
                                        );
                                    });
                                }
                            }),
                    );

                div()
                    .flex()
                    .flex_col()
                    .h_full()
                    .child(history)
                    .child(composer)
                    .into_any_element()
            }
        };

        let theme = cx.theme();
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .h(px(44.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.title_bar_border)
                    .bg(theme.title_bar)
                    .child(div().child("Workspace")),
            )
            .when_some(self.state.last_error.clone(), |s, message| {
                let theme = cx.theme();
                let view_handle = cx.entity().downgrade();
                s.child(
                    div()
                        .mx_3()
                        .mt_3()
                        .p_2()
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
            .child(content)
            .into_any_element()
    }
}

fn render_conversation_entry(
    entry: &luban_domain::ConversationEntry,
    theme: &gpui_component::Theme,
    expanded_items: &HashSet<String>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    match entry {
        luban_domain::ConversationEntry::UserMessage { text } => {
            let bubble = div()
                .p_2()
                .rounded_md()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .child(
                    div()
                        .text_color(theme.secondary_foreground)
                        .child(text.clone()),
                );

            div()
                .w_full()
                .flex()
                .justify_end()
                .child(bubble)
                .into_any_element()
        }
        luban_domain::ConversationEntry::CodexItem { item } => {
            render_codex_item(item.as_ref(), theme, false, expanded_items, view_handle)
        }
        luban_domain::ConversationEntry::TurnUsage { usage: _ } => {
            div().hidden().into_any_element()
        }
        luban_domain::ConversationEntry::TurnDuration { duration_ms } => div()
            .text_color(theme.muted_foreground)
            .child(format!(
                "Time: {}",
                format_duration_compact(Duration::from_millis(*duration_ms))
            ))
            .into_any_element(),
        luban_domain::ConversationEntry::TurnError { message } => div()
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

fn build_workspace_history_children(
    entries: &[luban_domain::ConversationEntry],
    theme: &gpui_component::Theme,
    expanded_items: &HashSet<String>,
    expanded_turns: &HashSet<String>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> Vec<AnyElement> {
    struct TurnAccumulator<'a> {
        id: String,
        tool_calls: usize,
        messages: usize,
        ops: Vec<&'a CodexThreadItem>,
        agent_messages: Vec<&'a CodexThreadItem>,
    }

    let mut children = Vec::new();
    let mut turn_index = 0usize;
    let mut current_turn: Option<TurnAccumulator<'_>> = None;

    let flush_turn = |turn: TurnAccumulator<'_>, children: &mut Vec<AnyElement>| {
        if turn.ops.is_empty() && turn.agent_messages.is_empty() {
            return;
        }

        let expanded = expanded_turns.contains(&turn.id);
        let header = render_agent_turn_summary_row(
            &turn.id,
            turn.tool_calls,
            turn.messages,
            !turn.ops.is_empty(),
            expanded,
            theme,
            view_handle,
        );
        let content = div().pl_4().flex().flex_col().gap_2().children(
            turn.ops
                .into_iter()
                .map(|item| render_codex_item(item, theme, false, expanded_items, view_handle)),
        );

        children.push(
            Collapsible::new()
                .open(expanded)
                .w_full()
                .child(header)
                .content(content)
                .into_any_element(),
        );

        for item in turn.agent_messages {
            children.push(render_codex_item(
                item,
                theme,
                false,
                expanded_items,
                view_handle,
            ));
        }
    };

    for entry in entries {
        match entry {
            luban_domain::ConversationEntry::UserMessage { text: _ } => {
                if let Some(turn) = current_turn.take() {
                    flush_turn(turn, &mut children);
                }

                children.push(render_conversation_entry(
                    entry,
                    theme,
                    expanded_items,
                    view_handle,
                ));
                current_turn = Some(TurnAccumulator {
                    id: format!("agent-turn-{turn_index}"),
                    tool_calls: 0,
                    messages: 0,
                    ops: Vec::new(),
                    agent_messages: Vec::new(),
                });
                turn_index += 1;
            }
            luban_domain::ConversationEntry::CodexItem { item } => {
                let item = item.as_ref();
                if let Some(turn) = &mut current_turn {
                    if matches!(item, CodexThreadItem::AgentMessage { .. }) {
                        turn.messages += 1;
                        turn.agent_messages.push(item);
                        continue;
                    }

                    if codex_item_is_tool_call(item) {
                        turn.tool_calls += 1;
                    }
                    turn.ops.push(item);
                    continue;
                }

                children.push(render_codex_item(
                    item,
                    theme,
                    false,
                    expanded_items,
                    view_handle,
                ));
            }
            luban_domain::ConversationEntry::TurnUsage { .. } => {
                if let Some(turn) = current_turn.take() {
                    flush_turn(turn, &mut children);
                }
            }
            luban_domain::ConversationEntry::TurnDuration { .. }
            | luban_domain::ConversationEntry::TurnError { .. } => {
                if let Some(turn) = current_turn.take() {
                    flush_turn(turn, &mut children);
                }
                children.push(render_conversation_entry(
                    entry,
                    theme,
                    expanded_items,
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
    tool_calls: usize,
    messages: usize,
    has_ops: bool,
    expanded: bool,
    theme: &gpui_component::Theme,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    let row = div()
        .h(px(28.0))
        .w_full()
        .px_1()
        .flex()
        .items_center()
        .gap_2();

    let row = if has_ops {
        let icon = if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        let tooltip = if expanded { "Hide" } else { "Show" };
        let id = id.to_owned();
        let view_handle = view_handle.clone();
        row.child(
            Button::new(format!("agent-turn-toggle-{id}"))
                .ghost()
                .compact()
                .icon(icon)
                .tooltip(tooltip)
                .on_click(move |_, _, app| {
                    let _ = view_handle.update(app, |view, cx| {
                        view.toggle_agent_turn_expanded(&id);
                        cx.notify();
                    });
                }),
        )
    } else {
        row.child(div().w(px(16.0)))
    };

    row.child(
        div()
            .flex_1()
            .truncate()
            .text_color(theme.muted_foreground)
            .child(format!("{tool_calls} tool calls, {messages} messages")),
    )
    .into_any_element()
}

fn render_codex_item(
    item: &CodexThreadItem,
    theme: &gpui_component::Theme,
    in_progress: bool,
    expanded_items: &HashSet<String>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    if !in_progress && let CodexThreadItem::AgentMessage { text, .. } = item {
        return div()
            .w_full()
            .px_1()
            .py_1()
            .flex()
            .flex_col()
            .child(div().text_color(theme.foreground).child(text.clone()))
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
        let item_icon = Icon::new(codex_item_icon_name(item))
            .with_size(Size::Small)
            .text_color(theme.muted_foreground);
        return div()
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
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .text_color(theme.muted_foreground)
                    .child(text),
            )
            .when_some(toggle_button, |s, b| s.child(b))
            .into_any_element();
    }

    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_color(theme.muted_foreground).child(title))
                .child(div().child(summary)),
        )
        .when_some(toggle_button, |s, b| s.child(b));

    Collapsible::new()
        .open(expanded)
        .p_2()
        .rounded_md()
        .bg(theme.muted)
        .border_1()
        .border_color(theme.border)
        .child(header)
        .content(render_codex_item_details(item, theme))
        .into_any_element()
}

fn render_codex_item_details(item: &CodexThreadItem, theme: &gpui_component::Theme) -> AnyElement {
    match item {
        CodexThreadItem::AgentMessage { text, .. } => div()
            .mt_2()
            .child(div().child(text.clone()))
            .into_any_element(),
        CodexThreadItem::Reasoning { text, .. } => div()
            .mt_2()
            .text_color(theme.muted_foreground)
            .child(text.clone())
            .into_any_element(),
        CodexThreadItem::CommandExecution {
            command,
            aggregated_output,
            exit_code,
            ..
        } => div()
            .mt_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().child(command.clone()))
            .when(!aggregated_output.trim().is_empty(), |s| {
                s.child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(aggregated_output.clone()),
                )
            })
            .when_some(*exit_code, |s, code| {
                s.child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(format!("Exit: {code}")),
                )
            })
            .into_any_element(),
        CodexThreadItem::FileChange { changes, .. } => div()
            .mt_2()
            .flex()
            .flex_col()
            .gap_1()
            .children(changes.iter().map(|c| {
                div()
                    .text_color(theme.muted_foreground)
                    .child(format!("{:?}: {}", c.kind, c.path))
            }))
            .into_any_element(),
        CodexThreadItem::TodoList { items, .. } => div()
            .mt_2()
            .flex()
            .flex_col()
            .gap_1()
            .children(items.iter().map(|i| {
                let prefix = if i.completed { "[x]" } else { "[ ]" };
                div()
                    .text_color(theme.muted_foreground)
                    .child(format!("{prefix} {}", i.text))
            }))
            .into_any_element(),
        CodexThreadItem::WebSearch { query, .. } => div()
            .mt_2()
            .child(div().child(query.clone()))
            .into_any_element(),
        CodexThreadItem::McpToolCall {
            server,
            tool,
            status,
            ..
        } => div()
            .mt_2()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().child(format!("{server}::{tool}")))
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child(format!("{status:?}")),
            )
            .into_any_element(),
        CodexThreadItem::Error { message, .. } => div()
            .mt_2()
            .text_color(theme.danger_foreground)
            .child(message.clone())
            .into_any_element(),
    }
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
                text.lines().next().unwrap_or("").to_owned()
            },
        ),
        CodexThreadItem::CommandExecution {
            command, status, ..
        } => ("Command", format!("{status:?}{progress_suffix}: {command}")),
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

fn codex_item_compact_summary(item: &CodexThreadItem) -> (&'static str, String) {
    match item {
        CodexThreadItem::AgentMessage { text, .. } => {
            ("Agent", text.lines().next().unwrap_or("").to_owned())
        }
        CodexThreadItem::Reasoning { text, .. } => {
            let summary = if text.trim().is_empty() {
                "…".to_owned()
            } else {
                text.lines().next().unwrap_or("").to_owned()
            };
            ("Thinking", summary)
        }
        CodexThreadItem::CommandExecution { command, .. } => ("Bash", command.clone()),
        CodexThreadItem::FileChange { changes, .. } => {
            ("Patch", format!("{} file(s)", changes.len()))
        }
        CodexThreadItem::McpToolCall { server, tool, .. } => ("MCP", format!("{server}::{tool}")),
        CodexThreadItem::WebSearch { query, .. } => ("Search", query.clone()),
        CodexThreadItem::TodoList { items, .. } => ("Todo", format!("{} item(s)", items.len())),
        CodexThreadItem::Error { message, .. } => ("Error", message.clone()),
    }
}

fn codex_item_icon_name(item: &CodexThreadItem) -> IconName {
    match item {
        CodexThreadItem::AgentMessage { .. } => IconName::Bot,
        CodexThreadItem::Reasoning { .. } => IconName::LoaderCircle,
        CodexThreadItem::CommandExecution { .. } => IconName::SquareTerminal,
        CodexThreadItem::FileChange { .. } => IconName::File,
        CodexThreadItem::McpToolCall { .. } => IconName::Settings2,
        CodexThreadItem::WebSearch { .. } => IconName::Globe,
        CodexThreadItem::TodoList { .. } => IconName::Check,
        CodexThreadItem::Error { .. } => IconName::TriangleAlert,
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

fn render_running_status_row(theme: &gpui_component::Theme, elapsed: Duration) -> AnyElement {
    div()
        .h(px(28.0))
        .w_full()
        .px_1()
        .flex()
        .items_center()
        .gap_2()
        .child(
            Icon::new(IconName::LoaderCircle)
                .with_size(Size::Small)
                .text_color(theme.muted_foreground),
        )
        .child(
            div()
                .flex_1()
                .truncate()
                .text_color(theme.muted_foreground)
                .child(format!("Running: {}", format_duration_compact(elapsed))),
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
    use gpui::Modifiers;
    use std::sync::Arc;

    #[derive(Default)]
    struct FakeService;

    impl ProjectWorkspaceService for FakeService {
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
            _project_slug: String,
            _workspace_name: String,
            _worktree_path: PathBuf,
            thread_id: Option<String>,
            prompt: String,
            on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync>,
        ) -> Result<(), String> {
            let thread_id = thread_id.unwrap_or_else(|| "thread-1".to_owned());
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
                    text: format!("Echo: {prompt}"),
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
            command: "echo hello".to_owned(),
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
}
