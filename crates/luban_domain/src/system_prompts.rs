use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SystemTaskKind {
    InferType,
    RenameBranch,
    AutoTitleThread,
    AutoUpdateTaskStatus,
}

impl SystemTaskKind {
    pub const ALL: [SystemTaskKind; 4] = [
        SystemTaskKind::InferType,
        SystemTaskKind::RenameBranch,
        SystemTaskKind::AutoTitleThread,
        SystemTaskKind::AutoUpdateTaskStatus,
    ];

    pub fn as_key(self) -> &'static str {
        match self {
            SystemTaskKind::InferType => "infer-type",
            SystemTaskKind::RenameBranch => "rename-branch",
            SystemTaskKind::AutoTitleThread => "auto-title-thread",
            SystemTaskKind::AutoUpdateTaskStatus => "auto-update-task-status",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SystemTaskKind::InferType => "Infer Type",
            SystemTaskKind::RenameBranch => "Rename Branch",
            SystemTaskKind::AutoTitleThread => "Auto Title Thread",
            SystemTaskKind::AutoUpdateTaskStatus => "Auto Update Task Status",
        }
    }
}

pub fn default_system_prompt_templates() -> HashMap<SystemTaskKind, String> {
    let mut out = HashMap::new();
    for kind in SystemTaskKind::ALL {
        out.insert(kind, default_system_prompt_template(kind));
    }
    out
}

pub fn default_system_prompt_template(kind: SystemTaskKind) -> String {
    match kind {
        SystemTaskKind::InferType => r#"You are classifying a user's input into a task intent kind.

Rules:
- Do NOT run commands.
- Do NOT modify files.
- Output ONLY a single JSON object, no markdown, no extra text.
- Always output a result, even if the input is vague: choose intent_kind="other".
- Do NOT include rationale, notes, or any extra fields in the output.
- Do NOT ask the user for more info in the output.

Allowed intent_kind values:
- fix
- implement
- review
- discuss
- other

Input:
{{task_input}}

Retrieved context (JSON):
{{context_json}}

Output JSON schema:
{
  "intent_kind": "<one of the allowed values>"
}
"#
        .to_owned(),
        SystemTaskKind::RenameBranch => r#"You are generating a git branch name.

Rules:
- Do NOT run commands.
- Do NOT modify files.
- Output ONLY a single line, no markdown, no extra text.
- Always output a result, even if the input is vague: output "luban/misc".
- Do NOT include rationale, notes, quotes, or code fences.
- Do NOT ask the user for more info.

Branch name requirements:
- Must start with "luban/".
- Must use lowercase letters, numbers, and hyphens only after the prefix.
- Keep it short (prefer <= 24 chars after "luban/").
- Use hyphens to connect words.
- Avoid trailing hyphens.
- Include identifiers (like issue/pr number) only when helpful.

Input:
{{task_input}}

Retrieved context:
{{context_json}}
"#
        .to_owned(),
        SystemTaskKind::AutoTitleThread => {
            r#"You are generating a short, user-friendly conversation thread title.

Rules:
- Do NOT run commands.
- Do NOT modify files.
- Output ONLY a single line, no markdown, no extra text.
- Do NOT include quotes, code fences, or surrounding punctuation.
- Do NOT include any personally identifying information.
- Do NOT invent facts beyond the input.
- Keep the title short and readable. Enforce the max length from context_json.max_chars.
- Always output something. If the input is too vague, output "Thread".

Input:
{{task_input}}

Context (JSON):
{{context_json}}
"#
            .to_owned()
        }
        SystemTaskKind::AutoUpdateTaskStatus => {
            r#"You are updating a task status for a conversation thread based on the latest progress.

Rules:
- Do NOT run commands.
- Do NOT modify files.
- Output ONLY a single JSON object, no markdown, no extra text.
- Do NOT include rationale, notes, or any extra fields in the output.
- Always output a result, even if the input is vague: keep the current status.
- Prefer conservative updates: do not mark as "done" unless the task is clearly finished.
- If the conversation indicates the work has been submitted as a pull request, you should generally use task_status="validating".
- When you output task_status="validating", you must try to extract the pull request number (and URL if present) from the conversation context, so the system can auto-complete the task when that PR is merged later.

Allowed task_status values:
- backlog
- todo
- iterating
- validating
- done
- canceled

Input:
{{task_input}}

Context (JSON):
{{context_json}}

Output JSON schema:
{
  "task_status": "<one of the allowed values>",
  "validation_pr_number": "<integer or null>",
  "validation_pr_url": "<string; empty if validation_pr_number is null>"
}

Validation PR rules:
- Always include validation_pr_number and validation_pr_url.
- Only set validation_pr_number if task_status="validating" and the conversation clearly references the PR number.
- If you cannot identify a PR number, set validation_pr_number to null and validation_pr_url to an empty string.
- If you set validation_pr_number, set validation_pr_url if and only if a URL is present in the conversation.
"#
            .to_owned()
        }
    }
}
