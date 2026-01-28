use crate::{AttachmentRef, DraftAttachment};

fn draft_attachment_sort_key(a: &DraftAttachment) -> (usize, u64) {
    (a.anchor, a.id)
}

pub fn ordered_draft_attachments_for_display(
    attachments: &[DraftAttachment],
) -> Vec<DraftAttachment> {
    let mut out = attachments.to_vec();
    out.sort_by_key(draft_attachment_sort_key);
    out
}

pub fn compose_user_message_text(draft_text: &str, attachments: &[DraftAttachment]) -> String {
    let _ = attachments;
    draft_text.trim().to_owned()
}

pub fn ordered_draft_attachments_for_send(attachments: &[DraftAttachment]) -> Vec<AttachmentRef> {
    let mut out = Vec::new();
    for attachment in attachments {
        let Some(value) = attachment.attachment.as_ref() else {
            continue;
        };
        if attachment.failed {
            continue;
        }
        out.push((draft_attachment_sort_key(attachment), value.clone()));
    }

    out.sort_by_key(|(key, _)| *key);
    out.into_iter().map(|(_, v)| v).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_returns_plain_text() {
        let composed = compose_user_message_text(" Hello ", &[]);
        assert_eq!(composed, "Hello");
    }

    #[test]
    fn attachments_for_display_are_sorted_by_anchor_then_id() {
        let attachments = vec![
            DraftAttachment {
                id: 2,
                kind: crate::ContextTokenKind::File,
                anchor: 10,
                attachment: None,
                failed: false,
            },
            DraftAttachment {
                id: 1,
                kind: crate::ContextTokenKind::File,
                anchor: 10,
                attachment: None,
                failed: false,
            },
            DraftAttachment {
                id: 3,
                kind: crate::ContextTokenKind::File,
                anchor: 5,
                attachment: None,
                failed: false,
            },
        ];

        let ordered = ordered_draft_attachments_for_display(&attachments);
        let ids: Vec<u64> = ordered.into_iter().map(|a| a.id).collect();
        assert_eq!(ids, vec![3, 1, 2]);
    }

    #[test]
    fn attachments_for_send_are_filtered_and_sorted() {
        let attachments = vec![
            DraftAttachment {
                id: 2,
                kind: crate::ContextTokenKind::File,
                anchor: 10,
                attachment: Some(AttachmentRef {
                    id: "b".to_owned(),
                    kind: crate::AttachmentKind::File,
                    name: "b".to_owned(),
                    extension: "txt".to_owned(),
                    mime: None,
                    byte_len: 2,
                }),
                failed: false,
            },
            DraftAttachment {
                id: 1,
                kind: crate::ContextTokenKind::File,
                anchor: 10,
                attachment: Some(AttachmentRef {
                    id: "a".to_owned(),
                    kind: crate::AttachmentKind::File,
                    name: "a".to_owned(),
                    extension: "txt".to_owned(),
                    mime: None,
                    byte_len: 1,
                }),
                failed: false,
            },
            DraftAttachment {
                id: 3,
                kind: crate::ContextTokenKind::File,
                anchor: 5,
                attachment: None,
                failed: false,
            },
            DraftAttachment {
                id: 4,
                kind: crate::ContextTokenKind::File,
                anchor: 0,
                attachment: Some(AttachmentRef {
                    id: "c".to_owned(),
                    kind: crate::AttachmentKind::File,
                    name: "c".to_owned(),
                    extension: "txt".to_owned(),
                    mime: None,
                    byte_len: 3,
                }),
                failed: true,
            },
        ];

        let ordered = ordered_draft_attachments_for_send(&attachments);
        let names: Vec<String> = ordered.into_iter().map(|a| a.name).collect();
        assert_eq!(names, vec!["a", "b"]);
    }
}
