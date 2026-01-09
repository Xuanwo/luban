use super::*;

fn attachment_title_and_icon(
    kind: luban_domain::ContextTokenKind,
    attachment: Option<&luban_domain::AttachmentRef>,
) -> (String, IconName, &'static str) {
    let filename = attachment
        .map(|a| a.name.clone())
        .unwrap_or_else(|| "attachment".to_owned());
    match kind {
        luban_domain::ContextTokenKind::Image => (filename, IconName::GalleryVerticalEnd, "Image"),
        luban_domain::ContextTokenKind::Text => (filename, IconName::BookOpen, "Text"),
        luban_domain::ContextTokenKind::File => (filename, IconName::File, "File"),
    }
}

pub(super) fn chat_composer_attachments_row(
    workspace_id: WorkspaceId,
    thread_id: WorkspaceThreadId,
    attachments: &[luban_domain::DraftAttachment],
    view_handle: &gpui::WeakEntity<LubanRootView>,
    theme: &gpui_component::Theme,
) -> Option<AnyElement> {
    if attachments.is_empty() {
        return None;
    }

    let ordered = ordered_draft_attachments_for_display(attachments);
    let items = ordered
        .iter()
        .map(|attachment| {
            let id = attachment.id;
            let debug_id = format!("chat-composer-attachment-{id}");
            let view_handle = view_handle.clone();

            let remove = move || {
                let view_handle = view_handle.clone();

                div()
                    .id(format!("chat-composer-attachment-remove-{id}"))
                    .w(px(18.0))
                    .h(px(18.0))
                    .rounded_full()
                    .bg(gpui::rgba(0x0000_00dc))
                    .text_color(gpui::rgb(0x00ff_ffff))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(Icon::new(IconName::Close).with_size(Size::XSmall))
                    .on_mouse_down(MouseButton::Left, move |_, _, app| {
                        let _ = view_handle.update(app, |view, cx| {
                            view.dispatch(
                                Action::ChatDraftAttachmentRemoved {
                                    workspace_id,
                                    thread_id,
                                    id,
                                },
                                cx,
                            );
                        });
                    })
            };

            let body = if attachment.failed {
                match attachment.kind {
                    luban_domain::ContextTokenKind::Image => div()
                        .relative()
                        .w(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE))
                        .h(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE))
                        .rounded_xl()
                        .border_1()
                        .border_color(theme.danger_hover)
                        .bg(theme.danger)
                        .text_color(theme.danger_foreground)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(div().text_sm().child("Failed"))
                        .child(div().absolute().top(px(6.0)).right(px(6.0)).child(remove()))
                        .into_any_element(),
                    luban_domain::ContextTokenKind::Text | luban_domain::ContextTokenKind::File => {
                        div()
                            .relative()
                            .h(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE))
                            .w(px(CHAT_ATTACHMENT_FILE_WIDTH))
                            .pl_3()
                            .pr(px(42.0))
                            .rounded_xl()
                            .border_1()
                            .border_color(theme.danger_hover)
                            .bg(theme.danger)
                            .text_color(theme.danger_foreground)
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().text_sm().child("Failed"))
                            .child(div().absolute().top(px(6.0)).right(px(6.0)).child(remove()))
                            .into_any_element()
                    }
                }
            } else if attachment.attachment.is_none() {
                match attachment.kind {
                    luban_domain::ContextTokenKind::Image => div()
                        .relative()
                        .w(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE))
                        .h(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE))
                        .rounded_xl()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.muted)
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Spinner::new().with_size(Size::Small))
                        .child(div().absolute().top(px(6.0)).right(px(6.0)).child(remove()))
                        .into_any_element(),
                    luban_domain::ContextTokenKind::Text | luban_domain::ContextTokenKind::File => {
                        let (icon_name, subtitle) = match attachment.kind {
                            luban_domain::ContextTokenKind::Text => (IconName::BookOpen, "Text"),
                            luban_domain::ContextTokenKind::File => (IconName::File, "File"),
                            luban_domain::ContextTokenKind::Image => unreachable!(),
                        };
                        div()
                            .relative()
                            .h(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE))
                            .w(px(CHAT_ATTACHMENT_FILE_WIDTH))
                            .pl_3()
                            .pr(px(42.0))
                            .rounded_xl()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.muted)
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .rounded_md()
                                    .bg(theme.background)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme.muted_foreground)
                                    .child(Icon::new(icon_name).with_size(Size::Small)),
                            )
                            .child(min_width_zero(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_0()
                                    .text_color(theme.foreground)
                                    .child(div().text_sm().truncate().child("Importing…"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(subtitle),
                                    ),
                            ))
                            .child(div().absolute().top(px(6.0)).right(px(6.0)).child(remove()))
                            .into_any_element()
                    }
                }
            } else {
                let stored = attachment
                    .attachment
                    .as_ref()
                    .expect("attachment should be resolved");
                match attachment.kind {
                    luban_domain::ContextTokenKind::Image => div()
                        .relative()
                        .w(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE))
                        .h(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE))
                        .rounded_xl()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.muted)
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(theme.muted_foreground)
                        .child(Icon::new(IconName::GalleryVerticalEnd).with_size(Size::Small))
                        .child(div().absolute().top(px(6.0)).right(px(6.0)).child(remove()))
                        .into_any_element(),
                    luban_domain::ContextTokenKind::Text | luban_domain::ContextTokenKind::File => {
                        let (filename, icon_name, subtitle) =
                            attachment_title_and_icon(attachment.kind, Some(stored));
                        div()
                            .relative()
                            .h(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE))
                            .w(px(CHAT_ATTACHMENT_FILE_WIDTH))
                            .pl_3()
                            .pr(px(42.0))
                            .rounded_xl()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.muted)
                            .text_color(theme.muted_foreground)
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .rounded_md()
                                    .bg(theme.background)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme.muted_foreground)
                                    .child(Icon::new(icon_name).with_size(Size::Small)),
                            )
                            .child(min_width_zero(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_0()
                                    .text_color(theme.foreground)
                                    .child(div().text_sm().truncate().child(filename))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(subtitle),
                                    ),
                            ))
                            .child(div().absolute().top(px(6.0)).right(px(6.0)).child(remove()))
                            .into_any_element()
                    }
                }
            };

            div()
                .debug_selector(move || debug_id.clone())
                .flex_shrink_0()
                .child(body)
                .into_any_element()
        })
        .collect::<Vec<_>>();

    Some(
        div()
            .debug_selector(|| "chat-composer-attachments-row".to_owned())
            .w_full()
            .h(px(CHAT_ATTACHMENT_THUMBNAIL_SIZE + 20.0))
            .px_4()
            .pt_2()
            .pb_1()
            .overflow_x_scrollbar()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .children(items),
            )
            .into_any_element(),
    )
}
