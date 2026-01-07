use super::*;

pub(super) fn render_titlebar(
    cx: &mut Context<LubanRootView>,
    state: &AppState,
    sidebar_width: gpui::Pixels,
    right_pane_width: gpui::Pixels,
    terminal_enabled: bool,
) -> AnyElement {
    fn workspace_to_return_to(view: &LubanRootView) -> Option<WorkspaceId> {
        let last_active = view
            .last_workspace_before_dashboard
            .and_then(|workspace_id| {
                view.state
                    .workspace(workspace_id)
                    .filter(|w| w.status == WorkspaceStatus::Active)
                    .map(|_| workspace_id)
            });
        if last_active.is_some() {
            return last_active;
        }

        for project in &view.state.projects {
            for workspace in &project.workspaces {
                if workspace.status != WorkspaceStatus::Active {
                    continue;
                }
                if workspace.workspace_name == "main" && workspace.worktree_path == project.path {
                    return Some(workspace.id);
                }
            }
        }

        for project in &view.state.projects {
            for workspace in &project.workspaces {
                if workspace.status == WorkspaceStatus::Active {
                    return Some(workspace.id);
                }
            }
        }

        None
    }

    fn handle_titlebar_double_click(window: &Window) {
        #[cfg(test)]
        {
            window.toggle_fullscreen();
        }

        #[cfg(all(not(test), target_os = "macos"))]
        {
            window.titlebar_double_click();
        }

        #[cfg(all(not(test), not(target_os = "macos")))]
        {
            window.zoom_window();
        }
    }

    let theme = cx.theme();
    let titlebar_height = px(TITLEBAR_HEIGHT);

    let titlebar_background = if state.main_pane == MainPane::Dashboard {
        theme.sidebar
    } else {
        theme.title_bar
    };
    let titlebar_border = if state.main_pane == MainPane::Dashboard {
        theme.sidebar_border
    } else {
        theme.title_bar_border
    };

    let TitlebarContext {
        branch_label,
        ide_workspace_id,
    } = titlebar_context(state);

    let terminal_toggle_enabled = terminal_enabled && ide_workspace_id.is_some();
    let terminal_toggle_icon = if state.right_pane == RightPane::Terminal {
        IconName::PanelRightClose
    } else {
        IconName::PanelRightOpen
    };
    let terminal_toggle_tooltip = if state.right_pane == RightPane::Terminal {
        "Hide terminal"
    } else {
        "Show terminal"
    };
    let terminal_toggle_button = {
        let view_handle = cx.entity().downgrade();
        Button::new("titlebar-toggle-terminal")
            .ghost()
            .compact()
            .disabled(!terminal_toggle_enabled)
            .icon(terminal_toggle_icon)
            .tooltip(terminal_toggle_tooltip)
            .on_click(move |_, _, app| {
                if !terminal_toggle_enabled {
                    return;
                }
                let _ = view_handle.update(app, |view, cx| {
                    view.dispatch(Action::ToggleTerminalPane, cx);
                });
            })
    };

    let open_in_zed_button = ide_workspace_id.map(|workspace_id| {
        let view_handle = cx.entity().downgrade();
        Button::new("workspace-open-in-zed")
            .outline()
            .compact()
            .icon(Icon::new(Icon::empty().path("icons/user-pen.svg")))
            .label("Open")
            .tooltip("Open in Zed")
            .on_click(move |_, _, app| {
                let _ = view_handle.update(app, |view, cx| {
                    view.dispatch(Action::OpenWorkspaceInIde { workspace_id }, cx);
                });
            })
    });

    let view_handle = cx.entity().downgrade();
    let add_project_button = {
        let view_handle = view_handle.clone();
        move || {
            let view_handle = view_handle.clone();
            Button::new("add-project")
                .ghost()
                .compact()
                .debug_selector(|| "add-project-button".to_owned())
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
                })
        }
    };

    let is_dashboard_selected = state.main_pane == MainPane::Dashboard;
    let dashboard_preview_open =
        is_dashboard_selected && state.dashboard_preview_workspace_id.is_some();

    let sidebar_titlebar = if sidebar_width <= px(0.0) {
        div()
            .w(px(0.0))
            .h(titlebar_height)
            .hidden()
            .into_any_element()
    } else {
        let (toggle_label, toggle_label_debug, toggle_icon_path) = if is_dashboard_selected {
            (
                "Dashboard",
                "titlebar-dashboard-label",
                "icons/square-kanban.svg",
            )
        } else {
            (
                "Workspace",
                "titlebar-workspace-label",
                "icons/notebook-text.svg",
            )
        };
        let toggle_color = theme.muted_foreground;

        div()
            .w(sidebar_width)
            .h(titlebar_height)
            .flex_shrink_0()
            .flex()
            .items_center()
            .bg(theme.sidebar)
            .text_color(theme.sidebar_foreground)
            .border_b_1()
            .border_color(theme.sidebar_border)
            .when(!is_dashboard_selected, |s| {
                s.border_r_1().border_color(theme.sidebar_border)
            })
            .debug_selector(|| "titlebar-sidebar".to_owned())
            .child(
                div()
                    .h_full()
                    .mx_3()
                    .w_full()
                    .flex()
                    .items_center()
                    .child(div().flex_1().when(dashboard_preview_open, |s| {
                        let view_handle = view_handle.clone();
                        s.cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, app| {
                                let _ = view_handle.update(app, |view, cx| {
                                    view.dispatch(Action::DashboardPreviewClosed, cx);
                                });
                            })
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.sidebar_border)
                            .debug_selector(|| "titlebar-dashboard-title".to_owned())
                            .cursor_pointer()
                            .hover(move |s| s.bg(theme.sidebar_accent))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    if this.state.main_pane == MainPane::Dashboard {
                                        if let Some(workspace_id) = workspace_to_return_to(this) {
                                            this.dispatch(
                                                Action::OpenWorkspace { workspace_id },
                                                cx,
                                            );
                                        }
                                    } else {
                                        this.dispatch(Action::OpenDashboard, cx);
                                    }
                                }),
                            )
                            .child(
                                Icon::new(Icon::empty().path(toggle_icon_path))
                                    .with_size(Size::Small)
                                    .text_color(toggle_color),
                            )
                            .child(
                                div()
                                    .debug_selector(move || toggle_label_debug.to_owned())
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(toggle_color)
                                    .child(toggle_label),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .justify_end()
                            .when(!is_dashboard_selected, |s| s.child(add_project_button())),
                    ),
            )
            .into_any_element()
    };

    let branch_indicator = div()
        .flex()
        .items_center()
        .debug_selector(|| "titlebar-branch-indicator".to_owned())
        .child(div().text_sm().child(branch_label));

    let titlebar_zoom_area = div()
        .flex_1()
        .h(titlebar_height)
        .flex()
        .items_center()
        .gap_3()
        .debug_selector(|| "titlebar-zoom-area".to_owned())
        .on_mouse_down(MouseButton::Left, move |event, window, _| {
            if event.click_count != 2 {
                return;
            }
            handle_titlebar_double_click(window);
        })
        .child(branch_indicator)
        .when_some(open_in_zed_button, |s, button| {
            s.child(
                div()
                    .debug_selector(|| "titlebar-open-in-zed".to_owned())
                    .child(button)
                    .flex_shrink_0(),
            )
        })
        .child(div().flex_1());

    let main_titlebar = if is_dashboard_selected {
        let view_handle = view_handle.clone();
        div()
            .flex_1()
            .h(titlebar_height)
            .px_4()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(titlebar_border)
            .bg(titlebar_background)
            .debug_selector(|| "titlebar-main".to_owned())
            .on_mouse_down(MouseButton::Left, move |event, window, app| {
                if event.click_count == 2 {
                    handle_titlebar_double_click(window);
                    return;
                }
                if dashboard_preview_open {
                    let _ = view_handle.update(app, |view, cx| {
                        view.dispatch(Action::DashboardPreviewClosed, cx);
                    });
                }
            })
            .child(div().flex_1())
            .into_any_element()
    } else {
        div()
            .flex_1()
            .h(titlebar_height)
            .px_4()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(titlebar_border)
            .bg(titlebar_background)
            .debug_selector(|| "titlebar-main".to_owned())
            .child(min_width_zero(titlebar_zoom_area))
            .into_any_element()
    };

    let terminal_titlebar = {
        let right_width = if state.right_pane == RightPane::Terminal && terminal_toggle_enabled {
            right_pane_width
        } else if terminal_toggle_enabled {
            px(44.0)
        } else {
            px(0.0)
        };

        let show_divider = state.right_pane == RightPane::Terminal && terminal_toggle_enabled;
        let divider = div()
            .id("titlebar-terminal-divider")
            .w(if show_divider { px(1.0) } else { px(0.0) })
            .h_full()
            .bg(theme.border)
            .flex_shrink_0()
            .debug_selector(|| "titlebar-terminal-divider".to_owned());

        let content = div()
            .id("titlebar-terminal-content")
            .flex_1()
            .h_full()
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .when(
                state.right_pane == RightPane::Terminal && terminal_toggle_enabled,
                |s| s.child(div().text_sm().font_semibold().child("Terminal")),
            )
            .child(
                div()
                    .debug_selector(|| "titlebar-toggle-terminal".to_owned())
                    .child(terminal_toggle_button),
            );

        div()
            .w(right_width)
            .h(titlebar_height)
            .flex_shrink_0()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(titlebar_border)
            .bg(titlebar_background)
            .debug_selector(|| "titlebar-terminal".to_owned())
            .child(divider)
            .child(content)
    };

    div()
        .w_full()
        .flex()
        .child(sidebar_titlebar)
        .child(main_titlebar)
        .child(terminal_titlebar)
        .into_any_element()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TitlebarContext {
    pub(super) branch_label: String,
    pub(super) ide_workspace_id: Option<WorkspaceId>,
}

pub(super) fn titlebar_context(state: &AppState) -> TitlebarContext {
    let active_workspace = match state.main_pane {
        MainPane::Workspace(workspace_id) => state.workspace(workspace_id),
        MainPane::Dashboard | MainPane::ProjectSettings(_) | MainPane::None => None,
    };
    let fallback_title = main_pane_title(state, state.main_pane);

    TitlebarContext {
        branch_label: active_workspace
            .map(|workspace| workspace.branch_name.clone())
            .unwrap_or(fallback_title),
        ide_workspace_id: active_workspace.map(|workspace| workspace.id),
    }
}
