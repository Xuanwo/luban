use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, MouseButton, PromptButton, PromptLevel, Window, div, px,
};
use gpui_component::{ActiveTheme as _, Disableable as _, button::*};
use luban_domain::{
    Action, AppState, Effect, MainPane, OperationStatus, ProjectId, WorkspaceId, WorkspaceStatus,
};
use std::{path::PathBuf, sync::Arc};

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
}

pub struct LubanRootView {
    state: AppState,
    services: Arc<dyn ProjectWorkspaceService>,
}

impl LubanRootView {
    pub fn new(services: Arc<dyn ProjectWorkspaceService>, _cx: &mut Context<Self>) -> Self {
        Self {
            state: AppState::new(),
            services,
        }
    }

    #[cfg(test)]
    pub fn with_state(
        services: Arc<dyn ProjectWorkspaceService>,
        state: AppState,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self { state, services }
    }

    #[cfg(test)]
    pub fn debug_state(&self) -> &AppState {
        &self.state
    }

    fn dispatch(&mut self, action: Action, cx: &mut Context<Self>) {
        let effects = self.state.apply(action);
        cx.notify();

        for effect in effects {
            self.run_effect(effect, cx);
        }
    }

    fn run_effect(&mut self, effect: Effect, cx: &mut Context<Self>) {
        match effect {
            Effect::CreateWorkspace { project_id } => self.run_create_workspace(project_id, cx),
            Effect::ArchiveWorkspace { workspace_id } => {
                self.run_archive_workspace(workspace_id, cx)
            }
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
}

impl gpui::Render for LubanRootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let sidebar_width = px(340.0);

        div()
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_sm()
            .child(render_sidebar(cx, &self.state, sidebar_width))
            .child(render_main(cx, &self.state))
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

fn render_main(cx: &mut Context<LubanRootView>, state: &AppState) -> AnyElement {
    let theme = cx.theme();
    let content = match state.main_pane {
        MainPane::None => div()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().child("Welcome"))
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("Select a workspace to begin."),
            )
            .into_any_element(),
        MainPane::ProjectSettings(project_id) => {
            let title = state
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
                        .text_color(theme.muted_foreground)
                        .child("No settings yet."),
                )
                .into_any_element()
        }
        MainPane::Workspace(workspace_id) => {
            let Some(workspace) = state.workspace(workspace_id) else {
                return div()
                    .p_3()
                    .child(
                        div()
                            .text_color(theme.danger_foreground)
                            .child("Workspace not found"),
                    )
                    .into_any_element();
            };

            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().child(workspace.workspace_name.clone()))
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(format!("Branch: {}", workspace.branch_name)),
                )
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(format!("Worktree: {}", workspace.worktree_path.display())),
                )
                .child(
                    div()
                        .mt_3()
                        .p_2()
                        .rounded_md()
                        .bg(theme.muted)
                        .border_1()
                        .border_color(theme.border)
                        .child("Agent interaction placeholder"),
                )
                .into_any_element()
        }
    };

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
        .when_some(state.last_error.clone(), |s, message| {
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
