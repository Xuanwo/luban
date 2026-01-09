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
    let titlebar_border = theme.border;

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
        let add_project_button = {
            let view_handle = view_handle.clone();
            div()
                .w(px(28.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .p(px(6.0))
                .rounded(px(4.0))
                .text_color(theme.muted_foreground)
                .hover(move |s| {
                    s.bg(theme.sidebar_accent)
                        .text_color(theme.sidebar_foreground)
                })
                .cursor_pointer()
                .debug_selector(|| "add-project-button".to_owned())
                .child(
                    Icon::empty()
                        .path("icons/plus.svg")
                        .with_size(Size::Size(px(16.0))),
                )
                .on_mouse_down(MouseButton::Left, move |_, _window, app| {
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
        };

        let workspace_menu = {
            let view_handle_for_menu = view_handle.clone();

            let trigger = WorkspaceMenuTrigger::new(
                theme.sidebar_accent,
                theme.primary,
                theme.muted_foreground,
                theme.sidebar_foreground,
            );

            Popover::new("sidebar-workspace-menu-popover")
                .appearance(true)
                .anchor(gpui::Corner::TopLeft)
                .trigger(trigger)
                .content(move |_popover_state, _window, cx| {
                    let theme = cx.theme();
                    let popover_handle = cx.entity();

                    let view_handle_for_close_preview = view_handle_for_menu.clone();
                    let popover_handle_for_close_preview = popover_handle.clone();
                    let close_preview = div()
                        .h(px(32.0))
                        .w_full()
                        .px(px(8.0))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .hover(move |s| s.bg(theme.list_hover))
                        .debug_selector(|| "sidebar-workspace-menu-close-preview".to_owned())
                        .child(div().text_sm().child("Close Preview"))
                        .on_mouse_down(MouseButton::Left, move |_, window, app| {
                            let _ = view_handle_for_close_preview.update(app, |view, cx| {
                                view.dispatch(Action::DashboardPreviewClosed, cx);
                            });
                            popover_handle_for_close_preview
                                .update(app, |state, cx| state.dismiss(window, cx));
                        });

                    let view_handle_for_create = view_handle_for_menu.clone();
                    let popover_handle_for_create = popover_handle.clone();
                    let create_worktree = div()
                        .h(px(32.0))
                        .w_full()
                        .px(px(8.0))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .hover(move |s| s.bg(theme.list_hover))
                        .debug_selector(|| "sidebar-workspace-menu-new-worktree".to_owned())
                        .child(div().text_sm().child("New Worktree"))
                        .on_mouse_down(MouseButton::Left, move |_, window, app| {
                            let _ = view_handle_for_create.update(app, |view, cx| {
                                let Some(project_id) = active_project_id(&view.state) else {
                                    return;
                                };
                                view.dispatch(Action::CreateWorkspace { project_id }, cx);
                            });
                            popover_handle_for_create
                                .update(app, |state, cx| state.dismiss(window, cx));
                        });

                    let view_handle_for_toggle = view_handle_for_menu.clone();
                    let popover_handle_for_toggle = popover_handle.clone();
                    let dashboard_label = if is_dashboard_selected {
                        "Return to Workspace"
                    } else {
                        "Open Dashboard"
                    };
                    let toggle_dashboard = div()
                        .h(px(32.0))
                        .w_full()
                        .px(px(8.0))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .hover(move |s| s.bg(theme.list_hover))
                        .debug_selector(|| "sidebar-workspace-menu-toggle-dashboard".to_owned())
                        .child(div().text_sm().child(dashboard_label))
                        .on_mouse_down(MouseButton::Left, move |_, window, app| {
                            let _ = view_handle_for_toggle.update(app, |view, cx| {
                                if view.state.main_pane == MainPane::Dashboard {
                                    if let Some(workspace_id) = workspace_to_return_to(view) {
                                        view.dispatch(Action::OpenWorkspace { workspace_id }, cx);
                                    }
                                } else {
                                    view.dispatch(Action::OpenDashboard, cx);
                                }
                            });
                            popover_handle_for_toggle
                                .update(app, |state, cx| state.dismiss(window, cx));
                        });

                    div()
                        .w(px(220.0))
                        .p(px(4.0))
                        .bg(theme.popover)
                        .border_1()
                        .border_color(theme.border)
                        .rounded_md()
                        .shadow_sm()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .when(dashboard_preview_open, |s| s.child(close_preview))
                        .child(create_worktree)
                        .child(toggle_dashboard)
                })
        };

        div()
            .w(sidebar_width)
            .h(titlebar_height)
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .pl(if cfg!(target_os = "macos") {
                px(84.0)
            } else {
                px(12.0)
            })
            .pr(px(12.0))
            .bg(theme.sidebar)
            .text_color(theme.sidebar_foreground)
            .when(!is_dashboard_selected, |s| {
                s.border_r_1().border_color(titlebar_border)
            })
            .debug_selector(|| "titlebar-sidebar".to_owned())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .when(dashboard_preview_open, |s| {
                        let view_handle = view_handle.clone();
                        s.cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, app| {
                                let _ = view_handle.update(app, |view, cx| {
                                    view.dispatch(Action::DashboardPreviewClosed, cx);
                                });
                            })
                    })
                    .child(workspace_menu),
            )
            .when(!is_dashboard_selected, |s| s.child(add_project_button))
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
            .bg(titlebar_background)
            .debug_selector(|| "titlebar-terminal".to_owned())
            .child(divider)
            .child(content)
    };

    div()
        .w_full()
        .flex()
        .border_b_1()
        .border_color(titlebar_border)
        .debug_selector(|| "titlebar".to_owned())
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

#[derive(IntoElement)]
struct WorkspaceMenuTrigger {
    sidebar_accent: gpui::Hsla,
    primary: gpui::Hsla,
    muted_foreground: gpui::Hsla,
    sidebar_foreground: gpui::Hsla,
    selected: bool,
}

impl WorkspaceMenuTrigger {
    fn new(
        sidebar_accent: gpui::Hsla,
        primary: gpui::Hsla,
        muted_foreground: gpui::Hsla,
        sidebar_foreground: gpui::Hsla,
    ) -> Self {
        Self {
            sidebar_accent,
            primary,
            muted_foreground,
            sidebar_foreground,
            selected: false,
        }
    }
}

impl gpui_component::Selectable for WorkspaceMenuTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl gpui::RenderOnce for WorkspaceMenuTrigger {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        let mut icon_bg = self.primary;
        icon_bg.a = 0.15;

        div()
            .h(px(32.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(6.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .hover(move |s| s.bg(self.sidebar_accent))
            .debug_selector(|| "sidebar-workspace-menu".to_owned())
            .child(
                div()
                    .w(px(24.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .bg(icon_bg)
                    .child(
                        Icon::empty()
                            .path("icons/sparkles.svg")
                            .with_size(Size::Size(px(14.0)))
                            .text_color(self.primary),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(self.sidebar_foreground)
                    .child("Workspace"),
            )
            .child(
                Icon::empty()
                    .path("icons/chevron-down.svg")
                    .with_size(Size::Size(px(12.0)))
                    .text_color(self.muted_foreground),
            )
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
