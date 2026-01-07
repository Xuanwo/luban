use super::*;

impl LubanRootView {
    pub(super) fn render_dashboard(
        &mut self,
        view_handle: gpui::WeakEntity<LubanRootView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cards = dashboard_cards(&self.state, &self.workspace_pull_request_numbers);
        let preview_open = self.state.dashboard_preview_workspace_id.is_some();

        let board = {
            let theme = cx.theme();
            if cards.is_empty() {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme.muted_foreground)
                    .child("No workspaces yet.")
                    .into_any_element()
            } else {
                let columns = DashboardStage::ALL
                    .iter()
                    .copied()
                    .map(|stage| {
                        let mut stage_cards: Vec<DashboardCardModel> =
                            cards.iter().filter(|c| c.stage == stage).cloned().collect();
                        stage_cards.sort_by(|a, b| b.sort_key.cmp(&a.sort_key));
                        render_dashboard_column(
                            stage,
                            stage_cards,
                            &view_handle,
                            theme,
                            preview_open,
                        )
                    })
                    .collect::<Vec<_>>();

                min_width_zero(min_height_zero(
                    div()
                        .flex_1()
                        .relative()
                        .debug_selector(|| "dashboard-board".to_owned())
                        .p_4()
                        .overflow_x_scrollbar()
                        .child(div().flex().flex_row().gap_3().children(columns)),
                ))
                .into_any_element()
            }
        };

        let theme = cx.theme();
        let preview_panel_bg = theme.background;
        let preview_panel_border = theme.border;
        let preview_resizer_hover = theme.muted;
        let preview_overlay_tint = gpui::rgba(0x00000010);

        let preview_panel = self
            .state
            .dashboard_preview_workspace_id
            .and_then(|workspace_id| {
                let pr_info = self
                    .workspace_pull_request_numbers
                    .get(&workspace_id)
                    .copied()
                    .flatten();
                let model = dashboard_preview(&self.state, workspace_id, pr_info)?;
                Some(self.render_dashboard_preview_panel(model, view_handle.clone(), window, cx))
            });

        let preview_overlay = preview_panel.map(|panel| {
            let preview_width = self.dashboard_preview_width(window);
            let transparent = gpui::rgba(0x00000000);
            let hover = preview_resizer_hover;
            let close_handle = view_handle.clone();
            div()
                .absolute()
                .inset_0()
                .occlude()
                .on_scroll_wheel(|_, _, app| {
                    app.stop_propagation();
                })
                .flex()
                .flex_row()
                .child(
                    div()
                        .flex_1()
                        .cursor_pointer()
                        .bg(preview_overlay_tint)
                        .debug_selector(|| "dashboard-preview-backdrop".to_owned())
                        .id("dashboard-preview-backdrop")
                        .on_mouse_down(MouseButton::Left, move |_, _, app| {
                            let _ = close_handle.update(app, |view, cx| {
                                view.dispatch(Action::DashboardPreviewClosed, cx);
                            });
                        }),
                )
                .child(
                    div()
                        .w(px(DASHBOARD_PREVIEW_RESIZER_WIDTH))
                        .h_full()
                        .flex_shrink_0()
                        .cursor(CursorStyle::ResizeLeftRight)
                        .id("dashboard-preview-resizer")
                        .debug_selector(|| "dashboard-preview-resizer".to_owned())
                        .bg(transparent)
                        .hover(move |s| s.bg(hover))
                        .on_prepaint({
                            let view_handle = view_handle.clone();
                            move |_bounds, _window, app| {
                                let _ = view_handle.update(app, |_view, cx| {
                                    #[cfg(test)]
                                    _view.record_inspector_bounds(
                                        "dashboard-preview-resizer",
                                        _bounds,
                                    );
                                    cx.notify();
                                });
                            }
                        })
                        .on_drag(DashboardPreviewResizeDrag, {
                            let view_handle = view_handle.clone();
                            move |_, _offset, window, app| {
                                let start_mouse_x = window.mouse_position().x;
                                let start_width = preview_width;
                                let _ = view_handle.update(app, |view, cx| {
                                    view.dashboard_preview_resize =
                                        Some(DashboardPreviewResizeState {
                                            start_mouse_x,
                                            start_width,
                                        });
                                    view.dashboard_preview_width_preview = Some(start_width);
                                    cx.notify();
                                });
                                app.new(|_| DashboardPreviewResizeGhost)
                            }
                        })
                        .on_drag_move::<DashboardPreviewResizeDrag>({
                            let view_handle = view_handle.clone();
                            move |event, window, app| {
                                let mouse_x = event.event.position.x;
                                let viewport_width = window.viewport_size().width;
                                let _ = view_handle.update(app, |view, cx| {
                                    let Some(state) = view.dashboard_preview_resize else {
                                        return;
                                    };
                                    let desired =
                                        state.start_width - (mouse_x - state.start_mouse_x);
                                    let clamped =
                                        view.clamp_dashboard_preview_width(desired, viewport_width);
                                    view.dashboard_preview_width_preview = Some(clamped);
                                    cx.notify();
                                });
                            }
                        })
                        .on_mouse_up(MouseButton::Left, {
                            let view_handle = view_handle.clone();
                            move |_, window, app| {
                                let viewport_width = window.viewport_size().width;
                                let _ = view_handle.update(app, |view, cx| {
                                    view.finish_dashboard_preview_resize(viewport_width, cx);
                                });
                            }
                        })
                        .on_mouse_up_out(MouseButton::Left, {
                            let view_handle = view_handle.clone();
                            move |_, window, app| {
                                let viewport_width = window.viewport_size().width;
                                let _ = view_handle.update(app, |view, cx| {
                                    view.finish_dashboard_preview_resize(viewport_width, cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .w(preview_width)
                        .h_full()
                        .flex_shrink_0()
                        .bg(preview_panel_bg)
                        .border_l_1()
                        .border_color(preview_panel_border)
                        .id("dashboard-preview-panel")
                        .debug_selector(|| "dashboard-preview-panel".to_owned())
                        .on_prepaint({
                            let view_handle = view_handle.clone();
                            move |_bounds, _window, app| {
                                let _ = view_handle.update(app, |_view, cx| {
                                    #[cfg(test)]
                                    _view.record_inspector_bounds(
                                        "dashboard-preview-panel",
                                        _bounds,
                                    );
                                    cx.notify();
                                });
                            }
                        })
                        .child(panel),
                )
                .into_any_element()
        });

        let container = div()
            .flex_1()
            .w_full()
            .h_full()
            .relative()
            .debug_selector(|| "dashboard-root".to_owned())
            .child(board)
            .when_some(preview_overlay, |s, overlay| s.child(overlay));
        #[cfg(test)]
        let container = {
            let view_handle = view_handle.clone();
            container.on_scroll_wheel(move |_, _, app| {
                let _ = view_handle.update(app, |view, _cx| {
                    view.dashboard_scroll_wheel_events =
                        view.dashboard_scroll_wheel_events.saturating_add(1);
                });
            })
        };
        container.into_any_element()
    }

    fn render_dashboard_preview_panel(
        &mut self,
        model: DashboardPreviewModel,
        view_handle: gpui::WeakEntity<LubanRootView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let background = theme.background;
        let border = theme.border;
        let muted_foreground = theme.muted_foreground;
        let workspace_id = model.workspace_id;
        let stage_label = model.stage.title();
        let project_line = match model.pr_info {
            Some(pr) => format!("{} · {} · #{}", model.project_name, stage_label, pr.number),
            None => format!("{} · {}", model.project_name, stage_label),
        };

        let open_task_button = {
            let view_handle = view_handle.clone();
            Button::new("dashboard-preview-open-task")
                .outline()
                .compact()
                .label("View")
                .tooltip("Open task view")
                .on_click(move |_, _, app| {
                    let _ = view_handle.update(app, |view, cx| {
                        view.dispatch(Action::OpenWorkspace { workspace_id }, cx);
                    });
                })
        };

        let open_in_zed_button = {
            let view_handle = view_handle.clone();
            Button::new("dashboard-preview-open-zed")
                .outline()
                .compact()
                .icon(IconName::ExternalLink)
                .label("Open in Zed")
                .tooltip("Open in Zed")
                .on_click(move |_, _, app| {
                    let _ = view_handle.update(app, |view, cx| {
                        view.dispatch(Action::OpenWorkspaceInIde { workspace_id }, cx);
                    });
                })
        };

        let close_button = {
            let view_handle = view_handle.clone();
            Button::new("dashboard-preview-close")
                .ghost()
                .compact()
                .icon(IconName::Close)
                .tooltip("Close preview")
                .on_click(move |_, _, app| {
                    let _ = view_handle.update(app, |view, cx| {
                        view.dispatch(Action::DashboardPreviewClosed, cx);
                    });
                })
        };

        let editor = self.render_dashboard_preview_editor(workspace_id, view_handle, window, cx);

        div()
            .h_full()
            .w_full()
            .bg(background)
            .flex()
            .flex_col()
            .debug_selector(|| "dashboard-preview".to_owned())
            .child(
                div()
                    .h(px(44.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .truncate()
                                    .child(model.workspace_name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_foreground)
                                    .truncate()
                                    .child(project_line),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .debug_selector(|| "dashboard-preview-open-task".to_owned())
                                    .child(open_task_button),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "dashboard-preview-open-zed".to_owned())
                                    .child(open_in_zed_button),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "dashboard-preview-close".to_owned())
                                    .child(close_button),
                            ),
                    ),
            )
            .child(editor)
            .into_any_element()
    }

    fn render_dashboard_preview_editor(
        &mut self,
        workspace_id: WorkspaceId,
        view_handle: gpui::WeakEntity<LubanRootView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.state.workspace(workspace_id).is_none() {
            return div()
                .flex_1()
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
        let thread_id = self.active_thread_id_for_workspace(workspace_id);
        let chat_key = (workspace_id, thread_id);
        let entries: &[luban_domain::ConversationEntry] =
            conversation.map(|c| c.entries.as_slice()).unwrap_or(&[]);
        let entries_len = conversation.map(|c| c.entries.len()).unwrap_or(0);
        let ordered_in_progress_items: Vec<&CodexThreadItem> = conversation
            .map(|c| {
                c.in_progress_order
                    .iter()
                    .filter_map(|id| c.in_progress_items.get(id))
                    .collect()
            })
            .unwrap_or_default();
        let run_status = conversation
            .map(|c| c.run_status)
            .unwrap_or(OperationStatus::Idle);
        let queued_prompts: Vec<luban_domain::QueuedPrompt> = conversation
            .map(|c| c.pending_prompts.iter().cloned().collect())
            .unwrap_or_default();
        let queue_paused = conversation.map(|c| c.queue_paused).unwrap_or(false);
        let _thread_id = conversation.and_then(|c| c.thread_id.as_deref());
        let (model_id, thinking_effort) = match (conversation, run_status) {
            (Some(conversation), OperationStatus::Running) => conversation
                .current_run_config
                .as_ref()
                .map(|cfg| (cfg.model_id.clone(), cfg.thinking_effort))
                .unwrap_or_else(|| {
                    (
                        conversation.agent_model_id.clone(),
                        conversation.thinking_effort,
                    )
                }),
            (Some(conversation), _) => (
                conversation.agent_model_id.clone(),
                conversation.thinking_effort,
            ),
            (None, _) => (
                default_agent_model_id().to_owned(),
                default_thinking_effort(),
            ),
        };

        let is_running = run_status == OperationStatus::Running;
        let chat_target_changed = self.last_chat_workspace_id != Some(chat_key);
        if chat_target_changed {
            let offset_y10 = self
                .state
                .workspace_chat_scroll_y10
                .get(&(workspace_id, thread_id))
                .copied()
                .unwrap_or(0);
            self.chat_scroll_handle
                .set_offset(point(px(0.0), px(offset_y10 as f32 / 10.0)));

            let saved_draft = conversation.map(|c| c.draft.clone()).unwrap_or_default();
            let current_value = input_state.read(cx).value().to_owned();
            let should_move_cursor = !saved_draft.is_empty();
            if current_value != saved_draft.as_str() || should_move_cursor {
                input_state.update(cx, |state, cx| {
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

        if chat_target_changed {
            let offset_y10 = quantize_pixels_y10(self.chat_scroll_handle.offset().y);
            self.chat_last_observed_scroll_offset_y10
                .insert(chat_key, offset_y10);
        }
        update_chat_follow_state(
            chat_key,
            &self.chat_scroll_handle,
            &mut self.chat_follow_tail,
            &mut self.chat_last_observed_scroll_offset_y10,
        );

        let theme = cx.theme();

        let draft_text = conversation.map(|c| c.draft.clone()).unwrap_or_default();
        let draft_attachments: Vec<luban_domain::DraftAttachment> = conversation
            .map(|c| c.draft_attachments.clone())
            .unwrap_or_default();
        let composed = compose_user_message_text(&draft_text, &draft_attachments);
        let pending_context_imports = self
            .pending_context_imports
            .get(&chat_key)
            .copied()
            .unwrap_or(0);
        let send_disabled = pending_context_imports > 0 || composed.trim().is_empty();
        let running_elapsed = if is_running {
            self.running_turn_started_at
                .get(&chat_key)
                .map(|t| t.elapsed())
        } else {
            None
        };
        let tail_duration = running_elapsed.map(|elapsed| (elapsed, true)).or_else(|| {
            self.pending_turn_durations
                .get(&chat_key)
                .copied()
                .map(|elapsed| (elapsed, false))
        });

        let expanded = self.expanded_agent_items.clone();
        let expanded_turns = self.expanded_agent_turns.clone();
        let has_in_progress_items = !ordered_in_progress_items.is_empty();
        let force_expand_current_turn = is_running || has_in_progress_items;

        let running_turn_summary_items: Vec<&CodexThreadItem> = if force_expand_current_turn {
            let turn_count = agent_turn_count(entries);
            if self.running_turn_user_message_count.get(&chat_key).copied() != Some(turn_count) {
                self.running_turn_user_message_count
                    .insert(chat_key, turn_count);
                self.running_turn_summary_order.insert(chat_key, Vec::new());
            }

            let order = self.running_turn_summary_order.entry(chat_key).or_default();

            if let Some(conversation) = conversation {
                for id in conversation.in_progress_order.iter() {
                    let Some(item) = conversation.in_progress_items.get(id) else {
                        continue;
                    };
                    if !codex_item_is_summary_item(item) {
                        continue;
                    }
                    if order.iter().any(|v| v == id) {
                        continue;
                    }
                    order.push(id.clone());
                }
            }

            if let Some(last_user_message_index) = entries
                .iter()
                .rposition(|e| matches!(e, luban_domain::ConversationEntry::UserMessage { .. }))
            {
                for entry in &entries[(last_user_message_index + 1)..] {
                    let luban_domain::ConversationEntry::CodexItem { item } = entry else {
                        continue;
                    };
                    let item = item.as_ref();
                    if !codex_item_is_summary_item(item) {
                        continue;
                    }
                    let id = codex_item_id(item);
                    if order.iter().any(|v| v == id) {
                        continue;
                    }
                    order.push(id.to_owned());
                }
            }

            let order_snapshot = order.clone();
            let mut items = Vec::new();
            if let Some(conversation) = conversation {
                for id in &order_snapshot {
                    if let Some(item) = conversation.in_progress_items.get(id) {
                        if codex_item_is_summary_item(item) {
                            items.push(item);
                        }
                        continue;
                    }

                    if let Some(item) = find_summary_item_in_current_turn(entries, id) {
                        items.push(item);
                    }
                }
            }
            items
        } else {
            self.running_turn_user_message_count.remove(&chat_key);
            self.running_turn_summary_order.remove(&chat_key);
            Vec::new()
        };

        let history_children = build_workspace_history_children(
            entries,
            theme,
            &expanded,
            &expanded_turns,
            self.chat_column_width,
            &view_handle,
            &running_turn_summary_items,
            force_expand_current_turn,
        );

        if self.should_chat_follow_tail(chat_key) {
            self.chat_scroll_handle.scroll_to_bottom();
        }
        self.last_chat_workspace_id = Some(chat_key);
        self.last_chat_item_count = entries_len;

        let debug_layout_enabled = self.debug_layout_enabled;
        let history = min_height_zero(
            div()
                .flex_1()
                .id("workspace-chat-scroll")
                .overflow_scroll()
                .track_scroll(&self.chat_scroll_handle)
                .overflow_x_hidden()
                .when(debug_layout_enabled, |s| {
                    s.on_prepaint(move |bounds, window, _app| {
                        debug_layout::record(
                            "workspace-chat-scroll",
                            window.viewport_size(),
                            bounds,
                        );
                    })
                })
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
                        .flex()
                        .flex_col()
                        .gap_3()
                        .whitespace_normal()
                        .pb_2()
                        .children(history_children)
                        .when_some(tail_duration, |s, (elapsed, running)| {
                            s.child(
                                div()
                                    .debug_selector(|| "chat-tail-turn-duration".to_owned())
                                    .child(render_turn_duration_row(theme, elapsed, running)),
                            )
                        }),
                )),
        );

        let queue_panel = if !queued_prompts.is_empty() {
            let theme = cx.theme();
            let view_handle = view_handle.clone();
            let input_state = input_state.clone();

            let toolbar =
                div()
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
                                                        thread_id,
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
                                                                    thread_id,
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
                queued_prompts.iter().enumerate().map(|(idx, queued)| {
                    let view_handle_for_edit = view_handle.clone();
                    let view_handle_for_remove = view_handle.clone();
                    let input_state = input_state.clone();
                    let text = queued.text.clone();
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
                                    let (draft_text, attachments) =
                                        draft_text_and_attachments_from_message_text(&text);
                                    input_state.update(app, |state, cx| {
                                        state.set_value(&draft_text, window, cx);
                                        let end =
                                            state.text().offset_to_position(state.text().len());
                                        state.set_cursor_position(end, window, cx);
                                    });
                                    let _ = view_handle_for_edit.update(app, |view, cx| {
                                        let existing_ids = view
                                            .state
                                            .workspace_conversation(workspace_id)
                                            .map(|c| {
                                                c.draft_attachments
                                                    .iter()
                                                    .map(|a| a.id)
                                                    .collect::<Vec<_>>()
                                            })
                                            .unwrap_or_default();
                                        for id in existing_ids {
                                            view.dispatch(
                                                Action::ChatDraftAttachmentRemoved {
                                                    workspace_id,
                                                    thread_id,
                                                    id,
                                                },
                                                cx,
                                            );
                                        }
                                        view.dispatch(
                                            Action::ChatDraftChanged {
                                                workspace_id,
                                                thread_id,
                                                text: draft_text.clone(),
                                            },
                                            cx,
                                        );
                                        for (kind, anchor, path) in attachments {
                                            let id = next_pending_context_id();
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
                                            view.dispatch(
                                                Action::ChatDraftAttachmentResolved {
                                                    workspace_id,
                                                    thread_id,
                                                    id,
                                                    path,
                                                },
                                                cx,
                                            );
                                        }
                                        view.dispatch(
                                            Action::RemoveQueuedPrompt {
                                                workspace_id,
                                                thread_id,
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
                                    let _ = view_handle_for_remove.update(app, |view, cx| {
                                        view.dispatch(
                                            Action::RemoveQueuedPrompt {
                                                workspace_id,
                                                thread_id,
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

        let debug_layout_enabled = self.debug_layout_enabled;
        let pending_drop_paths: std::rc::Rc<std::cell::RefCell<Option<Vec<std::path::PathBuf>>>> =
            std::rc::Rc::new(std::cell::RefCell::new(None));
        let composer = div()
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
                            move |_: &gpui_component::input::Paste,
                                  window: &mut Window,
                                  app: &mut gpui::App| {
                                let Some(clipboard) = app.read_from_clipboard() else {
                                    return;
                                };

	                                let (inline_text, imports) =
	                                    chat::composer::context_import_plan_from_clipboard(&clipboard);
	                                if imports.is_empty() {
	                                    return;
	                                }

                                app.stop_propagation();

	                                if let Some(text) = inline_text.as_deref() && !text.is_empty() {
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
                                                chat::composer::ContextImportSpec::Image { .. } => {
                                                    luban_domain::ContextTokenKind::Image
                                                }
                                                chat::composer::ContextImportSpec::Text { .. } => {
                                                    luban_domain::ContextTokenKind::Text
                                                }
                                                chat::composer::ContextImportSpec::File { .. } => {
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
	                                            view.enqueue_context_import(
	                                                workspace_id,
	                                                thread_id,
	                                                id,
	                                                kind,
	                                                spec,
	                                                cx,
	                                            );
	                                        }
                                    });
	                            }
	                        })
	                        .on_drop({
	                            let view_handle = view_handle.clone();
	                            let pending_drop_paths = pending_drop_paths.clone();
                                let input_state = input_state.clone();
	                            move |event: &gpui::FileDropEvent,
	                                  _window: &mut Window,
	                                  app: &mut gpui::App| {
	                                match event {
                                    gpui::FileDropEvent::Entered { paths, .. } => {
                                        *pending_drop_paths.borrow_mut() = Some(
                                            paths.paths()
                                                .iter()
                                                .map(|p| p.to_path_buf())
                                                .collect(),
                                        );
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
	                                            if !chat::composer::is_text_like_extension(&path) {
	                                                continue;
	                                            }
	                                            imports.push(chat::composer::ContextImportSpec::File { source_path: path });
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
                                            &draft_attachments,
                                            &view_handle,
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
                                        let current_model_id = model_id.clone();
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
                                                            .on_mouse_down(MouseButton::Left, move |_, window, app| {
                                                                let _ = view_handle.update(app, |view, cx| {
                                                                    view.dispatch(
                                                                        Action::ChatModelChanged {
                                                                            workspace_id,
                                                                            thread_id,
                                                                            model_id: model_id.clone(),
                                                                        },
                                                                        cx,
                                                                    );
                                                                });
                                                                popover_handle.update(app, |state, cx| {
                                                                    state.dismiss(window, cx);
                                                                });
                                                            })
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

                                        let effort_selector =
                                            Popover::new("chat-thinking-effort-popover")
                                                .appearance(true)
                                                .anchor(gpui::Corner::TopLeft)
                                                .trigger({
                                                    let label = current_thinking_effort.label();
                                                    Button::new("chat-thinking-effort-selector")
                                                        .outline()
                                                        .compact()
                                                        .with_size(Size::Small)
                                                        .icon(Icon::new(Icon::empty().path(
                                                            "icons/brain.svg",
                                                        )))
                                                        .label(label)
                                                })
                                                .content({
                                                    let view_handle = view_handle.clone();
                                                    let current_model_id = current_model_id.clone();
                                                    move |_popover_state, _window, cx| {
                                                        let theme = cx.theme();
                                                        let popover_handle = cx.entity();
                                                        let items = ThinkingEffort::ALL
                                                            .into_iter()
                                                            .map(|effort| {
                                                                let selected =
                                                                    effort == current_thinking_effort;
                                                                let enabled = thinking_effort_supported(
                                                                    &current_model_id,
                                                                    effort,
                                                                );
                                                                let view_handle = view_handle.clone();
                                                                let popover_handle =
                                                                    popover_handle.clone();
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
                                                                            .on_mouse_down(MouseButton::Left, move |_, window, app| {
                                                                                let _ = view_handle.update(app, |view, cx| {
                                                                                    view.dispatch(
                                                                                        Action::ThinkingEffortChanged {
                                                                                            workspace_id,
                                                                                            thread_id,
                                                                                            thinking_effort: effort,
                                                                                        },
                                                                                        cx,
                                                                                    );
                                                                                });
                                                                                popover_handle.update(app, |state, cx| {
                                                                                    state.dismiss(window, cx);
                                                                                });
                                                                            })
                                                                    })
                                                                    .when(!enabled, |s| {
                                                                        s.text_color(
                                                                            theme.muted_foreground,
                                                                        )
                                                                    })
                                                                    .child(div().child(effort.label()))
                                                                    .when(selected, |s| {
                                                                        s.child(
                                                                            Icon::new(IconName::Check)
                                                                                .with_size(Size::Small)
                                                                                .text_color(
                                                                                    theme.muted_foreground,
                                                                                ),
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
	                                                        let composed = composed.clone();
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
                                                            .debug_selector(|| "chat-thinking-effort-selector".to_owned())
                                                            .child(effort_selector),
                                                    ),
                                            )
                                            .child(send_controls)
                                            .into_any_element()
                                    }),
                            ),
                    ),
            );

        min_height_zero(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .child(self.render_workspace_thread_tabs(workspace_id, thread_id, &view_handle, cx))
                .child(history)
                .child(composer),
        )
        .into_any_element()
    }
}

fn stage_indicator_icon(stage: DashboardStage, theme: &gpui_component::Theme) -> Icon {
    let icon = match stage {
        DashboardStage::Start => Icon::new(Icon::empty().path("icons/circle-dot.svg")),
        DashboardStage::Running => Icon::new(Icon::empty().path("icons/play.svg")),
        DashboardStage::Pending => Icon::new(Icon::empty().path("icons/message-square-more.svg")),
        DashboardStage::Reviewing => Icon::new(IconName::Eye),
        DashboardStage::Finished => Icon::new(Icon::empty().path("icons/book-check.svg")),
    };

    icon.with_size(Size::Small)
        .text_color(theme.muted_foreground)
}

fn render_dashboard_column(
    stage: DashboardStage,
    cards: Vec<DashboardCardModel>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
    theme: &gpui_component::Theme,
    preview_open: bool,
) -> AnyElement {
    let indicator = stage_indicator_icon(stage, theme);

    let header = div()
        .h(px(44.0))
        .px_3()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(indicator)
                .child(div().text_sm().font_semibold().child(stage.title())),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(theme.muted)
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!("{}", cards.len())),
        );

    let body = min_height_zero(
        div().flex_1().overflow_y_scrollbar().child(
            div().p_3().flex().flex_col().gap_4().children(
                cards
                    .into_iter()
                    .map(|card| render_dashboard_card(card, view_handle, theme, preview_open)),
            ),
        ),
    );

    div()
        .w(px(320.0))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .rounded_xl()
        .bg(theme.muted)
        .border_1()
        .border_color(theme.border)
        .debug_selector(move || format!("dashboard-column-{}", stage.debug_id()))
        .child(div().px_3().pt_3().child(header))
        .child(body)
        .into_any_element()
}

fn render_dashboard_card(
    card: DashboardCardModel,
    view_handle: &gpui::WeakEntity<LubanRootView>,
    theme: &gpui_component::Theme,
    preview_open: bool,
) -> AnyElement {
    let DashboardCardModel {
        project_index,
        project_name,
        workspace_name,
        branch_name,
        workspace_id,
        pr_info,
        snippet,
        ..
    } = card;
    let pr_label = pr_info.map(|info| format!("#{}", info.number));
    let snippet = snippet.unwrap_or_else(|| "—".to_owned());
    let debug_selector = format!("dashboard-card-{project_index}-{workspace_name}");

    div()
        .px_3()
        .py_3()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .debug_selector(move || debug_selector.clone())
        .when(!preview_open, move |s| {
            s.hover(move |s| {
                s.bg(theme.list_hover)
                    .border_color(theme.scrollbar_thumb_hover)
            })
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, {
                let view_handle = view_handle.clone();
                move |_, _window, app| {
                    let _ = view_handle.update(app, |view, cx| {
                        view.dispatch(Action::DashboardPreviewOpened { workspace_id }, cx);
                    });
                }
            })
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(branch_name),
                )
                .when_some(pr_label, |s, label| {
                    s.child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme.muted)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .hover({
                                let hover = theme.scrollbar_thumb_hover;
                                move |s| s.bg(hover)
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
                }),
        )
        .child(
            div()
                .mt_2()
                .text_sm()
                .font_semibold()
                .truncate()
                .child(workspace_name),
        )
        .child(
            div()
                .mt_3()
                .text_xs()
                .text_color(theme.muted_foreground)
                .truncate()
                .child(snippet),
        )
        .child(
            div().mt_3().flex().items_center().justify_between().child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(project_name),
            ),
        )
        .into_any_element()
}

// (preview panel rendering moved into `LubanRootView` impl above)
