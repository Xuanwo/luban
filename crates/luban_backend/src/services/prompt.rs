use luban_domain::{AttachmentKind, AttachmentRef};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(super) struct PromptAttachment {
    pub(super) kind: AttachmentKind,
    pub(super) name: String,
    pub(super) path: PathBuf,
}

pub(super) fn resolve_prompt_attachments(
    blobs_dir: &Path,
    attachments: &[AttachmentRef],
) -> Vec<PromptAttachment> {
    attachments
        .iter()
        .map(|attachment| PromptAttachment {
            kind: attachment.kind,
            name: attachment.name.clone(),
            path: blobs_dir.join(format!("{}.{}", attachment.id, attachment.extension)),
        })
        .collect()
}

fn format_prompt(
    prompt: &str,
    attachments: &[PromptAttachment],
    attachment_path_prefix: &str,
) -> String {
    if attachments.is_empty() {
        return prompt.to_owned();
    }

    let mut out = String::with_capacity(prompt.len() + attachments.len() * 96);
    out.push_str(prompt.trim_end());
    out.push_str("\n\nAttached files:\n");
    for attachment in attachments {
        out.push_str("- ");
        let name = attachment.name.trim();
        if name.is_empty() {
            out.push_str(match attachment.kind {
                AttachmentKind::Image => "image",
                AttachmentKind::Text => "text",
                AttachmentKind::File => "file",
            });
        } else {
            out.push_str(name);
        }
        out.push_str(": ");
        out.push_str(attachment_path_prefix);
        out.push_str(&attachment.path.to_string_lossy());
        out.push('\n');
    }
    out
}

pub(super) fn format_amp_prompt(prompt: &str, attachments: &[PromptAttachment]) -> String {
    format_prompt(prompt, attachments, "@")
}

pub(super) fn format_codex_prompt(prompt: &str, attachments: &[PromptAttachment]) -> String {
    format_prompt(prompt, attachments, "")
}
