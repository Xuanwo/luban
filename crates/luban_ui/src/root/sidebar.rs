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
                        .w(px(12.0))
                        .debug_selector(|| "projects-scrollbar".to_owned())
                        .child(
                            Scrollbar::vertical(&projects_scroll_handle)
                                .id("projects-scrollbar")
                                .scrollbar_show(ScrollbarShow::Always),
                        ),
                ),
        ))
}

fn render_project(
    cx: &mut Context<LubanRootView>,
    state: &AppState,
    project_index: usize,
    project: &luban_domain::Project,
    main_pane: MainPane,
    workspace_pull_request_numbers: &HashMap<WorkspaceId, Option<PullRequestInfo>>,
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
    let create_disabled = create_loading;

    let create_button = {
        let view_handle = view_handle.clone();
        let create_icon = if create_loading {
            IconName::LoaderCircle
        } else {
            IconName::Plus
        };

        Button::new(format!("project-create-workspace-{project_index}"))
            .ghost()
            .compact()
            .disabled(create_disabled)
            .icon(Icon::new(create_icon).text_color(theme.muted_foreground))
            .tooltip("New workspace")
            .on_click(move |_, _, app| {
                if create_disabled {
                    return;
                }
                let _ = view_handle.update(app, |view, cx| {
                    view.dispatch(Action::CreateWorkspace { project_id }, cx);
                });
            })
    };

    let settings_menu = {
        let view_handle = view_handle.clone();
        Popover::new(format!("project-settings-menu-{project_index}"))
            .appearance(true)
            .anchor(gpui::Corner::TopLeft)
            .trigger(
                Button::new(format!("project-settings-{project_index}"))
                    .ghost()
                    .compact()
                    .icon(Icon::new(IconName::Settings2).text_color(theme.muted_foreground))
                    .tooltip("Project settings"),
            )
            .content(move |_popover_state, _window, cx| {
                let theme = cx.theme();
                let popover_handle = cx.entity();
                let popover_handle_for_settings = popover_handle.clone();
                let popover_handle_for_delete = popover_handle.clone();
                let view_handle_for_settings = view_handle.clone();
                let view_handle_for_delete = view_handle.clone();

                let settings_row = div()
                    .h(px(32.0))
                    .w_full()
                    .px_2()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .hover(move |s| s.bg(theme.list_hover))
                    .on_mouse_down(MouseButton::Left, move |_, window, app| {
                        let _ = view_handle_for_settings.update(app, |view, cx| {
                            view.dispatch(Action::OpenProjectSettings { project_id }, cx);
                        });
                        popover_handle_for_settings.update(app, |state, cx| {
                            state.dismiss(window, cx);
                        });
                    })
                    .child(div().text_sm().child("Settings"));

                let delete_row = div()
                    .h(px(32.0))
                    .w_full()
                    .px_2()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .hover(move |s| s.bg(theme.list_hover))
                    .on_mouse_down(MouseButton::Left, move |_, window, app| {
                        popover_handle_for_delete.update(app, |state, cx| {
                            state.dismiss(window, cx);
                        });
                        let receiver = window.prompt(
                            PromptLevel::Warning,
                            "Delete project?",
                            Some("This will remove the project and all workspaces from Luban. Worktrees on disk are not deleted."),
                            &[PromptButton::ok("Delete"), PromptButton::cancel("Cancel")],
                            app,
                        );

                        let view_handle = view_handle_for_delete.clone();
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
                                    |view: &mut LubanRootView,
                                     view_cx: &mut Context<LubanRootView>| {
                                        view.dispatch(Action::DeleteProject { project_id }, view_cx);
                                    },
                                );
                            }
                        })
                        .detach();
                    })
                    .child(div().text_sm().text_color(theme.danger_foreground).child("Delete"));

                div()
                    .w(px(220.0))
                    .p_1()
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .shadow_sm()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(settings_row)
                    .child(delete_row)
            })
    };

    let action_row = div()
        .flex()
        .items_center()
        .gap_1()
        .invisible()
        .group_hover("", |s| s.visible())
        .child(
            div()
                .debug_selector(move || format!("project-create-workspace-{project_index}"))
                .child(create_button),
        )
        .child(
            div()
                .debug_selector(move || format!("project-settings-{project_index}"))
                .child(settings_menu),
        );

    let header = div()
        .mx_3()
        .mt_2()
        .mb_2()
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_between()
        .text_color(if is_selected {
            theme.sidebar_accent_foreground
        } else {
            theme.sidebar_foreground
        })
        .group("")
        .debug_selector(move || format!("project-header-{project_index}"))
        .child(min_width_zero(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap_1()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener({
                        move |this, _, _, cx| {
                            this.dispatch(Action::ToggleProjectExpanded { project_id }, cx)
                        }
                    }),
                )
                .child(min_width_zero(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(min_width_zero(
                            div()
                                .debug_selector(move || format!("project-title-{project_index}"))
                                .truncate()
                                .text_lg()
                                .font_semibold()
                                .child(project.name.clone()),
                        ))
                        .child(
                            div()
                                .flex_shrink_0()
                                .debug_selector(move || format!("project-toggle-{project_index}"))
                                .child(
                                    Icon::new(disclosure_icon)
                                        .with_size(Size::Small)
                                        .text_color(theme.muted_foreground),
                                ),
                        ),
                )),
        ))
        .child(
            div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .debug_selector(move || format!("project-actions-{project_index}"))
                .child(action_row),
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
        .when(project.expanded, |s| {
            s.child(
                div()
                    .flex()
                    .flex_col()
                    .when_some(main_workspace, |s, row| s.child(row))
                    .child(div().mt_1().flex().flex_col().children(workspace_rows)),
            )
        })
        .into_any_element()
}

fn format_relative_age(when: Option<SystemTime>) -> Option<String> {
    let when = when?;
    let elapsed = SystemTime::now().duration_since(when).ok()?;
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
    let theme = cx.theme();
    let is_selected = matches!(main_pane, MainPane::Workspace(id) if id == workspace.id);
    let workspace_id = workspace.id;
    let is_running = state.workspace_has_running_turn(workspace_id);
    let has_unread = state.workspace_has_unread_completion(workspace_id);
    let archive_disabled = workspace.archive_status == OperationStatus::Running;
    let archive_icon = if archive_disabled {
        IconName::LoaderCircle
    } else {
        IconName::Inbox
    };

    let title = sidebar_workspace_title(workspace);
    let metadata = sidebar_workspace_metadata(workspace);
    let pr_label = pr_info.map(|info| format!("#{}", info.number));
    let git_icon = if pr_info.is_some() {
        "icons/git-pull-request-arrow.svg"
    } else {
        "icons/git-branch.svg"
    };

    let row = div()
        .mx_3()
        .px_2()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .rounded_md()
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
                .w(px(16.0))
                .h(px(16.0))
                .flex_shrink_0()
                .debug_selector(move || {
                    format!("workspace-status-container-{project_index}-{workspace_index}")
                })
                .when(is_running, |s| {
                    s.child(
                        div()
                            .id(format!(
                                "workspace-status-running-{project_index}-{workspace_index}"
                            ))
                            .debug_selector(move || {
                                format!(
                                    "workspace-status-running-{project_index}-{workspace_index}"
                                )
                            })
                            .child(
                                Spinner::new()
                                    .with_size(Size::Small)
                                    .color(theme.muted_foreground),
                            ),
                    )
                })
                .when(!is_running && has_unread, |s| {
                    s.child(
                        div()
                            .id(format!(
                                "workspace-status-unread-{project_index}-{workspace_index}"
                            ))
                            .debug_selector(move || {
                                format!("workspace-status-unread-{project_index}-{workspace_index}")
                            })
                            .child(
                                Icon::empty()
                                    .path("icons/message-square-dot.svg")
                                    .with_size(Size::Small)
                                    .text_color(theme.sidebar_accent_foreground),
                            ),
                    )
                }),
        )
        .child(
            div()
                .debug_selector(move || {
                    if pr_info.is_some() {
                        format!("workspace-git-icon-pr-{project_index}-{workspace_index}")
                    } else {
                        format!("workspace-git-icon-branch-{project_index}-{workspace_index}")
                    }
                })
                .child(
                    Icon::empty()
                        .path(git_icon)
                        .with_size(Size::Small)
                        .text_color(theme.muted_foreground),
                ),
        )
        .child(min_width_zero(
            div()
                .flex_1()
                .flex()
                .flex_col()
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
                        .w_full()
                        .truncate()
                        .text_sm()
                        .font_semibold()
                        .child(title),
                )
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(metadata),
                ),
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_shrink_0()
                .when_some(pr_label, |s, label| {
                    s.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .debug_selector(move || {
                                format!("workspace-pr-{project_index}-{workspace_index}")
                            })
                            .cursor_pointer()
                            .hover({
                                let hover = theme.link;
                                move |s| s.text_color(hover)
                            })
                            .on_mouse_down(MouseButton::Left, {
                                let view_handle = view_handle.clone();
                                move |_, _window, app| {
                                    app.stop_propagation();
                                    let _ = view_handle.update(app, |view, cx| {
                                        view.dispatch(
                                            Action::OpenWorkspacePullRequest { workspace_id },
                                            cx,
                                        );
                                    });
                                }
                            })
                            .child(label),
                    )
                })
                .child(
                    div()
                        .debug_selector(move || {
                            format!("workspace-archive-{project_index}-{workspace_index}")
                        })
                        .when(!archive_disabled, |s| s.invisible())
                        .group_hover("", |s| s.visible())
                        .child(
                            Button::new(format!(
                                "workspace-archive-{project_index}-{workspace_index}"
                            ))
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
                                            |view: &mut LubanRootView,
                                             view_cx: &mut Context<LubanRootView>| {
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
                ),
        );

    row.into_any_element()
}

fn render_main_workspace_row(
    cx: &mut Context<LubanRootView>,
    state: &AppState,
    project_index: usize,
    workspace: &luban_domain::Workspace,
    main_pane: MainPane,
) -> AnyElement {
    let theme = cx.theme();
    let is_selected = matches!(main_pane, MainPane::Workspace(id) if id == workspace.id);
    let workspace_id = workspace.id;
    let is_running = state.workspace_has_running_turn(workspace_id);
    let has_unread = state.workspace_has_unread_completion(workspace_id);

    let title = sidebar_workspace_title(workspace);
    let metadata = sidebar_workspace_metadata(workspace);

    div()
        .mx_3()
        .px_2()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .rounded_md()
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
        .debug_selector(move || format!("workspace-main-row-{project_index}"))
        .child(
            div()
                .w(px(16.0))
                .h(px(16.0))
                .flex_shrink_0()
                .debug_selector(move || format!("workspace-main-status-container-{project_index}"))
                .when(is_running, |s| {
                    s.child(
                        div()
                            .id(format!("workspace-main-status-running-{project_index}"))
                            .debug_selector(move || {
                                format!("workspace-main-status-running-{project_index}")
                            })
                            .child(
                                Spinner::new()
                                    .with_size(Size::Small)
                                    .color(theme.muted_foreground),
                            ),
                    )
                })
                .when(!is_running && has_unread, |s| {
                    s.child(
                        div()
                            .id(format!("workspace-main-status-unread-{project_index}"))
                            .debug_selector(move || {
                                format!("workspace-main-status-unread-{project_index}")
                            })
                            .child(
                                Icon::empty()
                                    .path("icons/message-square-dot.svg")
                                    .with_size(Size::Small)
                                    .text_color(theme.sidebar_accent_foreground),
                            ),
                    )
                }),
        )
        .child(
            div()
                .debug_selector(move || format!("workspace-main-icon-{project_index}"))
                .child(
                    Icon::empty()
                        .path("icons/house.svg")
                        .with_size(Size::Small)
                        .text_color(theme.muted_foreground),
                ),
        )
        .child(min_width_zero(
            div()
                .flex_1()
                .flex()
                .flex_col()
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
                        .w_full()
                        .truncate()
                        .text_sm()
                        .font_semibold()
                        .child(title),
                )
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(metadata),
                ),
        ))
        .into_any_element()
}

pub(super) fn sidebar_workspace_title(workspace: &luban_domain::Workspace) -> String {
    workspace.branch_name.clone()
}

pub(super) fn sidebar_workspace_metadata(workspace: &luban_domain::Workspace) -> String {
    let age = format_relative_age(workspace.last_activity_at);
    match age {
        Some(age) => format!("{} · {}", workspace.workspace_name, age),
        None => workspace.workspace_name.clone(),
    }
}
