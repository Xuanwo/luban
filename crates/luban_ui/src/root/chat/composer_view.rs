use super::super::*;
use super::composer;

pub(in crate::root) struct ChatComposerViewProps<'a> {
    pub(in crate::root) debug_layout_enabled: bool,
    pub(in crate::root) workspace_id: WorkspaceId,
    pub(in crate::root) thread_id: WorkspaceThreadId,
    pub(in crate::root) input_state: gpui::Entity<InputState>,
    pub(in crate::root) draft_attachments: &'a [luban_domain::DraftAttachment],
    pub(in crate::root) model_id: &'a str,
    pub(in crate::root) thinking_effort: ThinkingEffort,
    pub(in crate::root) composed: &'a str,
    pub(in crate::root) send_disabled: bool,
    pub(in crate::root) is_running: bool,
    pub(in crate::root) queue_panel: AnyElement,
    pub(in crate::root) view_handle: &'a gpui::WeakEntity<LubanRootView>,
    pub(in crate::root) theme: &'a gpui_component::Theme,
}

pub(in crate::root) fn render_chat_composer(props: ChatComposerViewProps<'_>) -> AnyElement {
    let ChatComposerViewProps {
        debug_layout_enabled,
        workspace_id,
        thread_id,
        input_state,
        draft_attachments,
        model_id,
        thinking_effort,
        composed,
        send_disabled,
        is_running,
        queue_panel,
        view_handle,
        theme,
    } = props;
    let pending_drop_paths: Rc<RefCell<Option<Vec<PathBuf>>>> = Rc::new(RefCell::new(None));
    div()
        .debug_selector(|| "workspace-chat-composer".to_owned())
        .when(debug_layout_enabled, |s| {
            s.on_prepaint(move |bounds, window, _app| {
                debug_layout::record("workspace-chat-composer", window.viewport_size(), bounds);
            })
        })
        .w_full()
        .flex_shrink_0()
        .px_4()
        .pb_4()
        .child(
            div()
                .w_full()
                .debug_selector(|| "chat-composer-surface".to_owned())
                .capture_action({
                    let view_handle = view_handle.clone();
                    let input_state = input_state.clone();
                    move |_: &gpui_component::input::Paste, window: &mut Window, app: &mut gpui::App| {
                        let Some(clipboard) = app.read_from_clipboard() else {
                            return;
                        };

                        let (inline_text, imports) =
                            composer::context_import_plan_from_clipboard(&clipboard);
                        if imports.is_empty() {
                            return;
                        }

                        app.stop_propagation();

                        if let Some(text) = inline_text.as_deref()
                            && !text.is_empty()
                        {
                            let inline_insert = text.to_owned();
                            input_state.update(app, move |state, cx| {
                                state.replace(&inline_insert, window, cx);
                            });
                        }

                        let draft_text = input_state.read(app).value().to_owned();
                        let anchor = input_state.read(app).cursor();

                        let _ = view_handle.update(app, move |view, cx| {
                            view.dispatch(
                                Action::ChatDraftChanged {
                                    workspace_id,
                                    thread_id,
                                    text: draft_text.to_string(),
                                },
                                cx,
                            );

                            for spec in imports {
                                let id = next_pending_context_id();
                                let kind = match &spec {
                                    composer::ContextImportSpec::Image { .. } => {
                                        luban_domain::ContextTokenKind::Image
                                    }
                                    composer::ContextImportSpec::Text { .. } => {
                                        luban_domain::ContextTokenKind::Text
                                    }
                                    composer::ContextImportSpec::File { .. } => {
                                        luban_domain::ContextTokenKind::File
                                    }
                                };
                                view.dispatch(
                                    Action::ChatDraftAttachmentAdded {
                                        workspace_id,
                                        thread_id,
                                        id,
                                        kind,
                                        anchor,
                                    },
                                    cx,
                                );
                                view.enqueue_context_import(workspace_id, thread_id, id, kind, spec, cx);
                            }
                        });
                    }
                })
                .on_drop({
                    let view_handle = view_handle.clone();
                    let pending_drop_paths = pending_drop_paths.clone();
                    let input_state = input_state.clone();
                    move |event: &gpui::FileDropEvent, _window: &mut Window, app: &mut gpui::App| {
                        match event {
                            gpui::FileDropEvent::Entered { paths, .. } => {
                                *pending_drop_paths.borrow_mut() =
                                    Some(paths.paths().iter().map(|p| p.to_path_buf()).collect());
                            }
                            gpui::FileDropEvent::Exited => {
                                pending_drop_paths.borrow_mut().take();
                            }
                            gpui::FileDropEvent::Submit { .. } => {
                                let Some(paths) = pending_drop_paths.borrow_mut().take() else {
                                    return;
                                };

                                let mut imports = Vec::new();
                                for path in paths {
                                    if !composer::is_text_like_extension(&path) {
                                        continue;
                                    }
                                    imports.push(composer::ContextImportSpec::File { source_path: path });
                                }

                                if imports.is_empty() {
                                    return;
                                }

                                let draft_text = input_state.read(app).value().to_owned();
                                let anchor = input_state.read(app).cursor();
                                let _ = view_handle.update(app, move |view, cx| {
                                    view.dispatch(
                                        Action::ChatDraftChanged {
                                            workspace_id,
                                            thread_id,
                                            text: draft_text.to_string(),
                                        },
                                        cx,
                                    );
                                    for spec in imports {
                                        let id = next_pending_context_id();
                                        view.dispatch(
                                            Action::ChatDraftAttachmentAdded {
                                                workspace_id,
                                                thread_id,
                                                id,
                                                kind: luban_domain::ContextTokenKind::File,
                                                anchor,
                                            },
                                            cx,
                                        );
                                        view.enqueue_context_import(
                                            workspace_id,
                                            thread_id,
                                            id,
                                            luban_domain::ContextTokenKind::File,
                                            spec,
                                            cx,
                                        );
                                    }
                                });
                            }
                            gpui::FileDropEvent::Pending { .. } => {}
                        }
                    }
                })
                .p_1()
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
                                .flex_col()
                                .gap_2()
                                .when_some(
                                    attachments::chat_composer_attachments_row(
                                        workspace_id,
                                        thread_id,
                                        draft_attachments,
                                        view_handle,
                                        theme,
                                    ),
                                    |s, row| s.child(row),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .debug_selector(|| "chat-composer-input".to_owned())
                                        .child(
                                            Input::new(&input_state)
                                                .px_4()
                                                .appearance(false)
                                                .with_size(Size::Large),
                                        ),
                                )
                                .child({
                                    let view_handle = view_handle.clone();
                                    let current_model_id = model_id.to_owned();
                                    let current_thinking_effort = thinking_effort;
                                    let model_label = agent_model_label(&current_model_id)
                                        .unwrap_or(current_model_id.as_str())
                                        .to_owned();

                                    let model_selector = Popover::new("chat-model-popover")
                                        .appearance(true)
                                        .anchor(gpui::Corner::TopLeft)
                                        .trigger(
                                            Button::new("chat-model-selector")
                                                .outline()
                                                .compact()
                                                .with_size(Size::Small)
                                                .icon(Icon::new(IconName::Bot))
                                                .label(model_label),
                                        )
                                        .content({
                                            let view_handle = view_handle.clone();
                                            let current_model_id = current_model_id.clone();
                                            move |_popover_state, _window, cx| {
                                                let theme = cx.theme();
                                                let popover_handle = cx.entity();
                                                let items = agent_models().iter().map(|spec| {
                                                    let selected = spec.id == current_model_id;
                                                    let view_handle = view_handle.clone();
                                                    let model_id = spec.id.to_owned();
                                                    let popover_handle = popover_handle.clone();
                                                    div()
                                                        .h(px(32.0))
                                                        .w_full()
                                                        .px_2()
                                                        .rounded_md()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .cursor_pointer()
                                                        .hover(move |s| s.bg(theme.list_hover))
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            move |_, window, app| {
                                                                let _ = view_handle.update(
                                                                    app,
                                                                    |view, cx| {
                                                                        view.dispatch(
                                                                            Action::ChatModelChanged {
                                                                                workspace_id,
                                                                                thread_id,
                                                                                model_id: model_id.clone(),
                                                                            },
                                                                            cx,
                                                                        );
                                                                    },
                                                                );
                                                                popover_handle.update(app, |state, cx| {
                                                                    state.dismiss(window, cx);
                                                                });
                                                            },
                                                        )
                                                        .child(div().child(spec.label))
                                                        .when(selected, |s| {
                                                            s.child(
                                                                Icon::new(IconName::Check)
                                                                    .with_size(Size::Small)
                                                                    .text_color(theme.muted_foreground),
                                                            )
                                                        })
                                                        .into_any_element()
                                                });

                                                div()
                                                    .w(px(260.0))
                                                    .p_2()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .children(items)
                                                    .into_any_element()
                                            }
                                        });

                                    let effort_selector = Popover::new("chat-thinking-effort-popover")
                                        .appearance(true)
                                        .anchor(gpui::Corner::TopLeft)
                                        .trigger({
                                            let label = current_thinking_effort.label();
                                            Button::new("chat-thinking-effort-selector")
                                                .outline()
                                                .compact()
                                                .with_size(Size::Small)
                                                .icon(Icon::new(Icon::empty().path("icons/brain.svg")))
                                                .label(label)
                                        })
                                        .content({
                                            let view_handle = view_handle.clone();
                                            let current_model_id = current_model_id.clone();
                                            move |_popover_state, _window, cx| {
                                                let theme = cx.theme();
                                                let popover_handle = cx.entity();
                                                let items = ThinkingEffort::ALL.into_iter().map(|effort| {
                                                    let selected = effort == current_thinking_effort;
                                                    let enabled =
                                                        thinking_effort_supported(&current_model_id, effort);
                                                    let view_handle = view_handle.clone();
                                                    let popover_handle = popover_handle.clone();
                                                    div()
                                                        .h(px(32.0))
                                                        .w_full()
                                                        .px_2()
                                                        .rounded_md()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .when(enabled, |s| {
                                                            s.cursor_pointer()
                                                                .hover(move |s| s.bg(theme.list_hover))
                                                                .on_mouse_down(
                                                                    MouseButton::Left,
                                                                    move |_, window, app| {
                                                                        let _ = view_handle.update(
                                                                            app,
                                                                            |view, cx| {
                                                                                view.dispatch(
                                                                                    Action::ThinkingEffortChanged {
                                                                                        workspace_id,
                                                                                        thread_id,
                                                                                        thinking_effort: effort,
                                                                                    },
                                                                                    cx,
                                                                                );
                                                                            },
                                                                        );
                                                                        popover_handle.update(
                                                                            app,
                                                                            |state, cx| {
                                                                                state.dismiss(window, cx);
                                                                            },
                                                                        );
                                                                    },
                                                                )
                                                        })
                                                        .when(!enabled, |s| {
                                                            s.text_color(theme.muted_foreground)
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(theme.muted_foreground)
                                                                        .child("Not supported"),
                                                                )
                                                        })
                                                        .child(div().child(effort.label()))
                                                        .when(selected, |s| {
                                                            s.child(
                                                                Icon::new(IconName::Check)
                                                                    .with_size(Size::Small)
                                                                    .text_color(theme.muted_foreground),
                                                            )
                                                        })
                                                        .into_any_element()
                                                });

                                                div()
                                                    .w(px(220.0))
                                                    .p_2()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .children(items)
                                                    .into_any_element()
                                            }
                                        });

                                    let send_controls = div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .debug_selector(|| "chat-send-message".to_owned())
                                                .child({
                                                    let view_handle = view_handle.clone();
                                                    let input_state = input_state.clone();
                                                    let composed = composed.to_owned();
                                                    Button::new("chat-send-message")
                                                        .primary()
                                                        .compact()
                                                        .disabled(send_disabled)
                                                        .icon(Icon::new(IconName::ArrowUp))
                                                        .tooltip(if is_running { "Queue" } else { "Send" })
                                                        .on_click(move |_, window, app| {
                                                            if composed.trim().is_empty() {
                                                                return;
                                                            }

                                                            input_state.update(app, |state, cx| {
                                                                state.set_value("", window, cx);
                                                            });

                                                            let _ = view_handle.update(app, |view, cx| {
                                                                view.dispatch(
                                                                    Action::SendAgentMessage {
                                                                        workspace_id,
                                                                        thread_id,
                                                                        text: composed.clone(),
                                                                    },
                                                                    cx,
                                                                );
                                                            });
                                                        })
                                                        .into_any_element()
                                                }),
                                        )
                                        .when(is_running, |s| {
                                            let view_handle = view_handle.clone();
                                            s.child(
                                                Button::new("chat-cancel-turn")
                                                    .danger()
                                                    .compact()
                                                    .icon(Icon::new(IconName::CircleX))
                                                    .tooltip("Cancel")
                                                    .on_click(move |_, _, app| {
                                                        let _ = view_handle.update(app, |view, cx| {
                                                            view.dispatch(
                                                                Action::CancelAgentTurn {
                                                                    workspace_id,
                                                                    thread_id,
                                                                },
                                                                cx,
                                                            );
                                                        });
                                                    }),
                                            )
                                        });

                                    div()
                                        .w_full()
                                        .flex()
                                        .px_4()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .debug_selector(|| "chat-model-selector".to_owned())
                                                        .child(model_selector),
                                                )
                                                .child(
                                                    div()
                                                        .debug_selector(|| {
                                                            "chat-thinking-effort-selector".to_owned()
                                                        })
                                                        .child(effort_selector),
                                                ),
                                        )
                                        .child(send_controls)
                                        .into_any_element()
                                }),
                        ),
                ),
        )
        .into_any_element()
}
