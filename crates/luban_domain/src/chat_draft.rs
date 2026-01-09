use crate::{AttachmentRef, DraftAttachment};

pub fn ordered_draft_attachments_for_display(
    attachments: &[DraftAttachment],
) -> Vec<DraftAttachment> {
    let mut out = attachments.to_vec();
    out.sort_by(|a, b| (a.anchor, a.id).cmp(&(b.anchor, b.id)));
    out
}

pub fn compose_user_message_text(draft_text: &str, attachments: &[DraftAttachment]) -> String {
    let _ = attachments;
    draft_text.trim().to_owned()
}

pub fn ordered_draft_attachments_for_send(attachments: &[DraftAttachment]) -> Vec<AttachmentRef> {
    let mut ready = attachments
        .iter()
        .filter(|a| a.attachment.is_some() && !a.failed)
        .collect::<Vec<_>>();
    if ready.is_empty() {
        return Vec::new();
    }

    ready.sort_by(|a, b| (a.anchor, a.id).cmp(&(b.anchor, b.id)));
    ready
        .into_iter()
        .filter_map(|a| a.attachment.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_returns_plain_text() {
        let composed = compose_user_message_text(" Hello ", &[]);
        assert_eq!(composed, "Hello");
    }
}
