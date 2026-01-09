use super::*;

pub(super) fn render_sidebar(
    cx: &mut Context<LubanRootView>,
    state: &AppState,
    sidebar_width: gpui::Pixels,
    workspace_pull_request_numbers: &HashMap<WorkspaceId, Option<PullRequestInfo>>,
    projects_scroll_handle: &gpui::ScrollHandle,
    debug_scrollbar_enabled: bool,
) -> impl IntoElement {
    let theme = cx.theme();
    let projects_scroll_handle = projects_scroll_handle.clone();
    let debug_scroll_handle = projects_scroll_handle.clone();
    let view_handle = cx.entity().downgrade();

    div()
        .w(sidebar_width)
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .debug_selector(|| "sidebar".to_owned())
        .bg(theme.sidebar)
        .text_color(theme.sidebar_foreground)
        .border_r_1()
        .border_color(theme.sidebar_border)
        .child(min_height_zero(
            div()
                .flex_1()
                .relative()
                .flex()
                .flex_col()
                .child(min_height_zero(
                    div()
                        .flex_1()
                        .id("projects-scroll")
                        .debug_selector(|| "projects-scroll".to_owned())
                        .overflow_y_scroll()
                        .track_scroll(&projects_scroll_handle)
                        .py_2()
                        .when(debug_scrollbar_enabled, move |s| {
                            s.on_prepaint(move |bounds, window, _app| {
                                debug_scrollbar::record(
                                    "projects-scroll",
                                    window.viewport_size(),
                                    bounds,
                                    &debug_scroll_handle,
                                );
                            })
                        })
                        .children(state.projects.iter().enumerate().map(|(i, project)| {
                            render_project(
                                cx,
                                state,
                                i,
                                project,
                                state.main_pane,
                                workspace_pull_request_numbers,
                            )
                        })),
                ))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .w(px(16.0))
                        .debug_selector(|| "projects-scrollbar".to_owned())
                        .child(
                            Scrollbar::vertical(&projects_scroll_handle).id("projects-scrollbar"),
                        ),
                ),
        ))
        .child(render_sidebar_footer(cx, state, view_handle))
}

fn render_project(
    cx: &mut Context<LubanRootView>,
    state: &AppState,
    project_index: usize,
    project: &luban_domain::Project,
    main_pane: MainPane,
    workspace_pull_request_numbers: &HashMap<WorkspaceId, Option<PullRequestInfo>>,
) -> AnyElement {
    let (border, muted_foreground, sidebar_accent, primary, transparent) = {
        let theme = cx.theme();
        (
            theme.border,
            theme.muted_foreground,
            theme.sidebar_accent,
            theme.primary,
            theme.transparent,
        )
    };
    let is_selected = matches!(main_pane, MainPane::ProjectSettings(id) if id == project.id);
    let is_active = match main_pane {
        MainPane::Workspace(workspace_id) => project
            .workspaces
            .iter()
            .any(|w| w.status == WorkspaceStatus::Active && w.id == workspace_id),
        _ => false,
    };
    let is_highlighted = is_selected || is_active;
    let view_handle = cx.entity().downgrade();
    let project_id = project.id;

    let disclosure_icon_path = if project.expanded {
        "icons/chevron-down.svg"
    } else {
        "icons/chevron-right.svg"
    };

    let mut highlighted_bg = primary;
    highlighted_bg.a = 0.1;
    let mut hover_bg = sidebar_accent;
    hover_bg.a = 0.5;

    let header = div()
        .px(px(12.0))
        .py(px(6.0))
        .flex()
        .items_center()
        .text_color(muted_foreground)
        .bg(if is_highlighted {
            highlighted_bg
        } else {
            transparent
        })
        .hover(move |s| s.bg(hover_bg))
        .cursor_pointer()
        .debug_selector(move || format!("project-header-{project_index}"))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.dispatch(Action::ToggleProjectExpanded { project_id }, cx)
            }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_shrink_0()
                        .debug_selector(move || format!("project-toggle-{project_index}"))
                        .child(
                            Icon::empty()
                                .path(disclosure_icon_path)
                                .with_size(Size::Size(px(12.0))),
                        ),
                )
                .child(min_width_zero(
                    div()
                        .flex_1()
                        .truncate()
                        .debug_selector(move || format!("project-title-{project_index}"))
                        .text_size(px(13.0))
                        .line_height(px(19.5))
                        .child(project.name.clone()),
                )),
        );

    let main_workspace = project
        .workspaces
        .iter()
        .find(|w| {
            w.status == WorkspaceStatus::Active
                && w.workspace_name == "main"
                && w.worktree_path == project.path
        })
        .map(|workspace| render_main_workspace_row(cx, state, project_index, workspace, main_pane));

    let workspace_rows: Vec<AnyElement> = project
        .workspaces
        .iter()
        .filter(|w| w.status == WorkspaceStatus::Active && w.worktree_path != project.path)
        .enumerate()
        .map(|(workspace_index, workspace)| {
            let pr_info = workspace_pull_request_numbers
                .get(&workspace.id)
                .copied()
                .flatten();
            render_workspace_row(
                cx,
                view_handle.clone(),
                state,
                project_index,
                workspace_index,
                workspace,
                main_pane,
                pr_info,
            )
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .child(header)
        .when(project.expanded || is_highlighted, |s| {
            s.child(
                div()
                    .flex()
                    .flex_col()
                    .ml_4()
                    .pl_3()
                    .border_l_1()
                    .border_color(border)
                    .when_some(main_workspace, |s, row| s.child(row))
                    .child(div().flex().flex_col().children(workspace_rows)),
            )
        })
        .into_any_element()
}

#[cfg(test)]
fn format_relative_age(when: Option<std::time::SystemTime>) -> Option<String> {
    let when = when?;
    let elapsed = std::time::SystemTime::now().duration_since(when).ok()?;
    let seconds = elapsed.as_secs();

    Some(if seconds < 60 {
        "just now".to_owned()
    } else if seconds < 60 * 60 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 60 * 60 * 24 {
        format!("{}h ago", seconds / (60 * 60))
    } else {
        format!("{}d ago", seconds / (60 * 60 * 24))
    })
}

#[allow(clippy::too_many_arguments)]
fn render_workspace_row(
    cx: &mut Context<LubanRootView>,
    view_handle: gpui::WeakEntity<LubanRootView>,
    state: &AppState,
    project_index: usize,
    workspace_index: usize,
    workspace: &luban_domain::Workspace,
    main_pane: MainPane,
    pr_info: Option<PullRequestInfo>,
) -> AnyElement {
    let workspace_id = workspace.id;
    let is_running = state.workspace_has_running_turn(workspace_id);
    let has_unread = state.workspace_has_unread_completion(workspace_id);
    let pr_open = pr_info.is_some_and(|info| info.state == luban_domain::PullRequestState::Open);
    let theme = cx.theme();
    let archive_disabled = workspace.archive_status == OperationStatus::Running;

    let title = sidebar_workspace_title(workspace);
    let pr_label = pr_info.map(|info| format!("#{}", info.number));
    let ci_state = pr_info.and_then(|info| info.ci_state);
    let merge_ready = pr_info.map(|info| info.merge_ready).unwrap_or(false);
    let status = if is_running {
        WorkspaceRowStatus::AgentRunning
    } else if has_unread {
        WorkspaceRowStatus::ReplyNeeded
    } else if pr_open {
        match ci_state.unwrap_or(luban_domain::PullRequestCiState::Success) {
            luban_domain::PullRequestCiState::Pending => WorkspaceRowStatus::PullRequestRunning,
            luban_domain::PullRequestCiState::Failure => WorkspaceRowStatus::PullRequestFailed,
            luban_domain::PullRequestCiState::Success => {
                if merge_ready {
                    WorkspaceRowStatus::PullRequestMergeReady
                } else {
                    WorkspaceRowStatus::PullRequestReviewing
                }
            }
        }
    } else {
        WorkspaceRowStatus::Idle
    };

    let mut hover_bg = theme.sidebar_accent;
    hover_bg.a = 0.3;

    let mut selected_bg = theme.primary;
    selected_bg.a = 0.1;
    let mut unread_bg = theme.warning_foreground;
    unread_bg.a = 0.1;
    let mut failure_bg = theme.danger_foreground;
    failure_bg.a = 0.05;

    let mut row_bg = theme.transparent;
    if matches!(main_pane, MainPane::Workspace(id) if id == workspace_id) {
        row_bg = selected_bg;
    } else if status == WorkspaceRowStatus::ReplyNeeded {
        row_bg = unread_bg;
    } else if status == WorkspaceRowStatus::PullRequestFailed {
        row_bg = failure_bg;
    }

    div()
        .px(px(8.0))
        .py(px(6.0))
        .mx(px(4.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .rounded(px(2.0))
        .cursor_pointer()
        .bg(row_bg)
        .hover(move |s| s.bg(hover_bg))
        .group("worktree")
        .debug_selector(move || format!("workspace-row-{project_index}-{workspace_index}"))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.dispatch(Action::OpenWorkspace { workspace_id }, cx)
            }),
        )
        .child(
            div()
                .debug_selector(move || format!("workspace-icon-{project_index}-{workspace_index}"))
                .child(
                    Icon::empty()
                        .path("icons/git-branch.svg")
                        .with_size(Size::Size(px(12.0)))
                        .text_color(theme.muted_foreground),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(12.0))
                .line_height(px(16.0))
                .text_color(theme.muted_foreground)
                .child(title),
        )
        .child(render_workspace_row_right(
            cx,
            view_handle,
            project_index,
            workspace_index,
            workspace_id,
            status,
            pr_label,
            archive_disabled,
        ))
        .into_any_element()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceRowStatus {
    Idle,
    AgentRunning,
    ReplyNeeded,
    PullRequestRunning,
    PullRequestReviewing,
    PullRequestMergeReady,
    PullRequestFailed,
}

fn render_main_workspace_row(
    cx: &mut Context<LubanRootView>,
    _state: &AppState,
    project_index: usize,
    workspace: &luban_domain::Workspace,
    _main_pane: MainPane,
) -> AnyElement {
    let theme = cx.theme();
    let workspace_id = workspace.id;

    let title = sidebar_workspace_title(workspace);
    let mut hover_bg = theme.sidebar_accent;
    hover_bg.a = 0.3;

    let mut muted_50 = theme.muted_foreground;
    muted_50.a = 0.5;

    div()
        .px(px(8.0))
        .py(px(6.0))
        .mx(px(4.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .rounded(px(2.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .group("worktree")
        .debug_selector(move || format!("workspace-main-row-{project_index}"))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.dispatch(Action::OpenWorkspace { workspace_id }, cx)
            }),
        )
        .child(
            div()
                .debug_selector(move || format!("workspace-main-icon-{project_index}"))
                .child(
                    Icon::empty()
                        .path("icons/git-branch.svg")
                        .with_size(Size::Size(px(12.0)))
                        .text_color(theme.muted_foreground),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(12.0))
                .line_height(px(16.0))
                .text_color(theme.muted_foreground)
                .child(title),
        )
        .child(
            Icon::empty()
                .path("icons/circle.svg")
                .with_size(Size::Size(px(12.0)))
                .text_color(muted_50),
        )
        .child(
            div()
                .p(px(2.0))
                .text_color(muted_50)
                .debug_selector(move || format!("workspace-main-home-{project_index}"))
                .child(
                    Icon::empty()
                        .path("icons/house.svg")
                        .with_size(Size::Size(px(12.0))),
                ),
        )
        .into_any_element()
}

pub(super) fn sidebar_workspace_title(workspace: &luban_domain::Workspace) -> String {
    workspace.workspace_name.clone()
}

#[cfg(test)]
pub(super) fn sidebar_workspace_metadata(workspace: &luban_domain::Workspace) -> String {
    let age = format_relative_age(workspace.last_activity_at);
    match age {
        Some(age) => format!("{} · {}", workspace.workspace_name, age),
        None => workspace.workspace_name.clone(),
    }
}

fn active_project_id(state: &AppState) -> Option<ProjectId> {
    match state.main_pane {
        MainPane::ProjectSettings(project_id) => Some(project_id),
        MainPane::Workspace(workspace_id) => state.projects.iter().find_map(|project| {
            project
                .workspaces
                .iter()
                .any(|w| w.status == WorkspaceStatus::Active && w.id == workspace_id)
                .then_some(project.id)
        }),
        MainPane::Dashboard | MainPane::None => state.projects.first().map(|p| p.id),
    }
}

fn render_sidebar_footer(
    cx: &mut Context<LubanRootView>,
    state: &AppState,
    view_handle: gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    let theme = cx.theme();
    let project_id = active_project_id(state);

    let settings_button = div()
        .w_full()
        .h(px(36.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(8.0))
        .text_size(px(14.0))
        .line_height(px(20.0))
        .text_color(theme.muted_foreground)
        .rounded(px(4.0))
        .hover(move |s| {
            s.bg(theme.sidebar_accent)
                .text_color(theme.sidebar_foreground)
        })
        .cursor_pointer()
        .debug_selector(|| "sidebar-settings".to_owned())
        .child(
            Icon::empty()
                .path("icons/settings.svg")
                .with_size(Size::Size(px(16.0))),
        )
        .child("Settings")
        .on_mouse_down(MouseButton::Left, move |_, _, app| {
            let Some(project_id) = project_id else {
                return;
            };
            let _ = view_handle.update(app, |view, cx| {
                view.dispatch(Action::OpenProjectSettings { project_id }, cx);
            });
        });

    div()
        .border_t_1()
        .border_color(theme.border)
        .p(px(8.0))
        .child(settings_button)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_workspace_row_right(
    cx: &mut Context<LubanRootView>,
    view_handle: gpui::WeakEntity<LubanRootView>,
    project_index: usize,
    workspace_index: usize,
    workspace_id: WorkspaceId,
    status: WorkspaceRowStatus,
    pr_label: Option<String>,
    archive_disabled: bool,
) -> AnyElement {
    let theme = cx.theme();

    let mut container = div().flex().items_center().gap(px(8.0)).flex_shrink_0();

    match status {
        WorkspaceRowStatus::AgentRunning => {
            container = container.child(
                div()
                    .id(format!(
                        "workspace-status-running-{project_index}-{workspace_index}"
                    ))
                    .debug_selector(move || {
                        format!("workspace-status-running-{project_index}-{workspace_index}")
                    })
                    .child(
                        SpinningIcon::new("icons/loader-circle.svg")
                            .with_size(px(12.0))
                            .color(theme.primary),
                    ),
            );
        }
        WorkspaceRowStatus::ReplyNeeded => {
            container = container.child(
                div()
                    .id(format!(
                        "workspace-status-unread-{project_index}-{workspace_index}"
                    ))
                    .debug_selector(move || {
                        format!("workspace-status-unread-{project_index}-{workspace_index}")
                    })
                    .relative()
                    .child(
                        Icon::empty()
                            .path("icons/message-circle.svg")
                            .with_size(Size::Size(px(12.0)))
                            .text_color(theme.warning_foreground),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(2.0))
                            .right(px(2.0))
                            .w(px(6.0))
                            .h(px(6.0))
                            .rounded(px(3.0))
                            .bg(theme.warning_foreground),
                    ),
            );
        }
        WorkspaceRowStatus::PullRequestRunning
        | WorkspaceRowStatus::PullRequestReviewing
        | WorkspaceRowStatus::PullRequestMergeReady
        | WorkspaceRowStatus::PullRequestFailed => {
            if let Some(pr_label) = pr_label {
                let (link_color, link_hover) =
                    if status == WorkspaceRowStatus::PullRequestMergeReady {
                        (theme.success_foreground, theme.success_foreground)
                    } else {
                        (theme.link, theme.link_hover)
                    };

                let pr_id = format!("workspace-pr-{project_index}-{workspace_index}");
                let view_handle_for_pr = view_handle.clone();
                let pr_button = div()
                    .debug_selector(move || pr_id.clone())
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .text_size(px(10.0))
                    .line_height(px(15.0))
                    .text_color(link_color)
                    .cursor_pointer()
                    .hover(move |s| s.text_color(link_hover))
                    .on_mouse_down(MouseButton::Left, move |_, _, app| {
                        let _ = view_handle_for_pr.update(app, |view, cx| {
                            view.dispatch(Action::OpenWorkspacePullRequest { workspace_id }, cx);
                        });
                    })
                    .child(
                        Icon::empty()
                            .path("icons/git-pull-request.svg")
                            .with_size(Size::Size(px(12.0))),
                    )
                    .child(pr_label);

                let status_icon: AnyElement = match status {
                    WorkspaceRowStatus::PullRequestRunning => div()
                        .id(format!(
                            "workspace-status-pr-running-{project_index}-{workspace_index}"
                        ))
                        .debug_selector(move || {
                            format!("workspace-status-pr-running-{project_index}-{workspace_index}")
                        })
                        .child(
                            SpinningIcon::new("icons/loader-circle.svg")
                                .with_size(px(10.0))
                                .color(theme.warning_foreground),
                        )
                        .into_any_element(),
                    WorkspaceRowStatus::PullRequestReviewing => div()
                        .id(format!(
                            "workspace-status-pr-reviewing-{project_index}-{workspace_index}"
                        ))
                        .debug_selector(move || {
                            format!(
                                "workspace-status-pr-reviewing-{project_index}-{workspace_index}"
                            )
                        })
                        .child(
                            Icon::empty()
                                .path("icons/clock.svg")
                                .with_size(Size::Size(px(10.0)))
                                .text_color(theme.warning_foreground),
                        )
                        .into_any_element(),
                    WorkspaceRowStatus::PullRequestMergeReady => div()
                        .id(format!(
                            "workspace-status-pr-merge-ready-{project_index}-{workspace_index}"
                        ))
                        .debug_selector(move || {
                            format!(
                                "workspace-status-pr-merge-ready-{project_index}-{workspace_index}"
                            )
                        })
                        .child(
                            Icon::empty()
                                .path("icons/circle-check.svg")
                                .with_size(Size::Size(px(10.0)))
                                .text_color(theme.success_foreground),
                        )
                        .into_any_element(),
                    WorkspaceRowStatus::PullRequestFailed => {
                        let id = format!(
                            "workspace-status-pr-failure-{project_index}-{workspace_index}"
                        );
                        let view_handle_for_failed_action = view_handle.clone();
                        div()
                            .debug_selector(move || id.clone())
                            .cursor_pointer()
                            .text_color(theme.danger_foreground)
                            .hover(move |s| s.text_color(theme.danger_foreground))
                            .on_mouse_down(MouseButton::Left, move |_, _, app| {
                                let _ = view_handle_for_failed_action.update(app, |view, cx| {
                                    view.dispatch(
                                        Action::OpenWorkspacePullRequestFailedAction {
                                            workspace_id,
                                        },
                                        cx,
                                    );
                                });
                            })
                            .child(
                                Icon::empty()
                                    .path("icons/circle-x.svg")
                                    .with_size(Size::Size(px(10.0))),
                            )
                            .into_any_element()
                    }
                    _ => div().into_any_element(),
                };

                container = container.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .flex_shrink_0()
                        .child(pr_button)
                        .child(status_icon),
                );
            }
        }
        WorkspaceRowStatus::Idle => {}
    }

    let archive_id = format!("workspace-archive-{project_index}-{workspace_index}");
    let archive_icon: AnyElement = if archive_disabled {
        SpinningIcon::new("icons/loader-circle.svg")
            .with_size(px(12.0))
            .color(theme.muted_foreground)
            .into_any_element()
    } else {
        Icon::empty()
            .path("icons/archive.svg")
            .with_size(Size::Size(px(12.0)))
            .into_any_element()
    };
    let archive_button = div()
        .debug_selector(move || archive_id.clone())
        .p(px(2.0))
        .rounded(px(4.0))
        .text_color(theme.muted_foreground)
        .hover(move |s| s.text_color(theme.sidebar_foreground))
        .cursor_pointer()
        .when(!archive_disabled, |s| s.invisible())
        .group_hover("worktree", |s| s.visible())
        .on_mouse_down(MouseButton::Left, {
            let view_handle_for_archive = view_handle.clone();
            move |_, window, app| {
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

                let view_handle = view_handle_for_archive.clone();
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
                                view.dispatch(Action::ArchiveWorkspace { workspace_id }, view_cx);
                            },
                        );
                    }
                })
                .detach();
            }
        })
        .child(archive_icon)
        .into_any_element();

    container.child(archive_button).into_any_element()
}

#[derive(IntoElement)]
struct SpinningIcon {
    path: SharedString,
    size: Pixels,
    color: gpui::Hsla,
}

impl SpinningIcon {
    fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            size: px(12.0),
            color: gpui::Hsla::default(),
        }
    }

    fn with_size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    fn color(mut self, color: gpui::Hsla) -> Self {
        self.color = color;
        self
    }
}

impl gpui::RenderOnce for SpinningIcon {
    fn render(self, window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        use std::time::{SystemTime, UNIX_EPOCH};

        window.request_animation_frame();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f32();
        let angle = now * std::f32::consts::TAU;

        Icon::empty()
            .path(self.path)
            .with_size(Size::Size(self.size))
            .text_color(self.color)
            .rotate(gpui::radians(angle))
    }
}
