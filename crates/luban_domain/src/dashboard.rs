use crate::time::unix_seconds_or_zero;
use crate::{
    AppState, CodexThreadItem, ConversationEntry, OperationStatus, Project, PullRequestInfo,
    Workspace, WorkspaceId, WorkspaceStatus,
};
use std::collections::HashMap;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DashboardStage {
    Start,
    Running,
    Pending,
    Reviewing,
    Finished,
}

impl DashboardStage {
    pub const ALL: [DashboardStage; 5] = [
        DashboardStage::Start,
        DashboardStage::Running,
        DashboardStage::Pending,
        DashboardStage::Reviewing,
        DashboardStage::Finished,
    ];

    pub fn title(self) -> &'static str {
        match self {
            DashboardStage::Start => "Start",
            DashboardStage::Running => "Running",
            DashboardStage::Pending => "Pending",
            DashboardStage::Reviewing => "Reviewing",
            DashboardStage::Finished => "Finished",
        }
    }

    pub fn debug_id(self) -> &'static str {
        match self {
            DashboardStage::Start => "start",
            DashboardStage::Running => "running",
            DashboardStage::Pending => "pending",
            DashboardStage::Reviewing => "reviewing",
            DashboardStage::Finished => "finished",
        }
    }
}

#[derive(Clone, Debug)]
pub struct DashboardCardModel {
    pub project_index: usize,
    pub project_name: String,
    pub workspace_name: String,
    pub branch_name: String,
    pub workspace_id: WorkspaceId,
    pub stage: DashboardStage,
    pub pr_info: Option<PullRequestInfo>,
    pub sort_key: u64,
    pub snippet: Option<String>,
}

#[derive(Clone, Debug)]
pub enum DashboardPreviewMessage {
    User(String),
    Agent(String),
}

#[derive(Clone, Debug)]
pub struct DashboardPreviewModel {
    pub workspace_id: WorkspaceId,
    pub workspace_name: String,
    pub project_name: String,
    pub stage: DashboardStage,
    pub pr_info: Option<PullRequestInfo>,
    pub messages: Vec<DashboardPreviewMessage>,
}

pub fn dashboard_cards(
    state: &AppState,
    pull_requests: &HashMap<WorkspaceId, Option<PullRequestInfo>>,
) -> Vec<DashboardCardModel> {
    let estimated_workspaces: usize = state.projects.iter().map(|p| p.workspaces.len()).sum();
    let mut cards = Vec::with_capacity(estimated_workspaces);

    for (project_index, project) in state.projects.iter().enumerate() {
        for workspace in &project.workspaces {
            if workspace.status != WorkspaceStatus::Active {
                continue;
            }
            if workspace.workspace_name == "main" && workspace.worktree_path == project.path {
                continue;
            }

            let pr_info = pull_requests.get(&workspace.id).copied().flatten();
            let stage = stage_for_workspace(state, project, workspace, pr_info);
            let sort_key = sort_key(workspace.last_activity_at);
            let snippet = state
                .workspace_conversation(workspace.id)
                .and_then(|c| latest_message_snippet(&c.entries));

            cards.push(DashboardCardModel {
                project_index,
                project_name: project.name.clone(),
                workspace_name: workspace.workspace_name.clone(),
                branch_name: workspace.branch_name.clone(),
                workspace_id: workspace.id,
                stage,
                pr_info,
                sort_key,
                snippet,
            });
        }
    }

    cards
}

pub fn dashboard_preview(
    state: &AppState,
    workspace_id: WorkspaceId,
    pr_info: Option<PullRequestInfo>,
) -> Option<DashboardPreviewModel> {
    let mut project_name = None;
    let mut workspace_name = None;
    let mut stage = None;

    for project in &state.projects {
        if let Some(found) = project
            .workspaces
            .iter()
            .find(|w| w.status == WorkspaceStatus::Active && w.id == workspace_id)
        {
            if found.workspace_name == "main" && found.worktree_path == project.path {
                return None;
            }

            project_name = Some(project.name.clone());
            workspace_name = Some(found.workspace_name.clone());
            stage = Some(stage_for_workspace(state, project, found, pr_info));
            break;
        }
    }

    let project_name = project_name?;
    let workspace_name = workspace_name?;
    let stage = stage?;
    let conversation = state.workspace_conversation(workspace_id);

    let mut messages: Vec<DashboardPreviewMessage> = Vec::new();
    if let Some(conversation) = conversation {
        for entry in conversation.entries.iter().rev() {
            match entry {
                ConversationEntry::UserMessage { text, .. } => {
                    messages.push(DashboardPreviewMessage::User(text.clone()));
                }
                ConversationEntry::CodexItem { item } => {
                    if let CodexThreadItem::AgentMessage { text, .. } = item.as_ref() {
                        messages.push(DashboardPreviewMessage::Agent(text.clone()));
                    }
                }
                _ => {}
            }

            if messages.len() >= 10 {
                break;
            }
        }
        messages.reverse();
    }

    Some(DashboardPreviewModel {
        workspace_id,
        workspace_name,
        project_name,
        stage,
        pr_info,
        messages,
    })
}

fn sort_key(when: Option<SystemTime>) -> u64 {
    unix_seconds_or_zero(when)
}

fn stage_for_workspace(
    state: &AppState,
    project: &Project,
    workspace: &Workspace,
    pr_info: Option<PullRequestInfo>,
) -> DashboardStage {
    if workspace.workspace_name == "main" && workspace.worktree_path == project.path {
        return DashboardStage::Start;
    }

    let conversation = state.workspace_conversation(workspace.id);
    let run_status = conversation
        .map(|c| c.run_status)
        .unwrap_or(OperationStatus::Idle);
    if run_status == OperationStatus::Running {
        return DashboardStage::Running;
    }

    let pending = conversation
        .map(|c| !c.pending_prompts.is_empty() || c.queue_paused)
        .unwrap_or(false);
    if pending {
        return DashboardStage::Pending;
    }

    if let Some(pr_info) = pr_info {
        if pr_info.state.is_finished() {
            return DashboardStage::Finished;
        }
        return DashboardStage::Reviewing;
    }

    DashboardStage::Start
}

fn latest_message_snippet(entries: &[ConversationEntry]) -> Option<String> {
    for entry in entries.iter().rev() {
        match entry {
            ConversationEntry::UserMessage { text, .. } => {
                return normalize_snippet(text);
            }
            ConversationEntry::CodexItem { item } => match item.as_ref() {
                CodexThreadItem::AgentMessage { text, .. } => {
                    return normalize_snippet(text);
                }
                _ => continue,
            },
            _ => continue,
        }
    }
    None
}

fn normalize_snippet(input: &str) -> Option<String> {
    const LIMIT: usize = 120;

    let mut out = String::new();
    let mut is_first_word = true;
    let mut truncated = false;
    let mut written_chars = 0usize;

    for word in input.split_whitespace() {
        if !is_first_word {
            if written_chars < LIMIT {
                out.push(' ');
            } else {
                truncated = true;
                break;
            }
            written_chars += 1;
        }
        is_first_word = false;

        for ch in word.chars() {
            if written_chars < LIMIT {
                out.push(ch);
                written_chars += 1;
            } else {
                truncated = true;
                break;
            }
        }

        if truncated {
            break;
        }
    }

    if out.is_empty() {
        return None;
    }

    if truncated {
        out.push('…');
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_id_by_name(state: &AppState, name: &str) -> WorkspaceId {
        for project in &state.projects {
            for workspace in &project.workspaces {
                if workspace.status == WorkspaceStatus::Active && workspace.workspace_name == name {
                    return workspace.id;
                }
            }
        }
        panic!("missing workspace {name}");
    }

    #[test]
    fn dashboard_cards_exclude_main_workspaces() {
        let state = AppState::demo();
        let pull_requests: HashMap<WorkspaceId, Option<PullRequestInfo>> = HashMap::new();
        let cards = dashboard_cards(&state, &pull_requests);

        assert!(
            cards.iter().all(|card| card.workspace_name != "main"),
            "expected dashboard cards to exclude main workspaces"
        );
        assert!(
            cards
                .iter()
                .any(|card| card.workspace_name == "abandon-about"),
            "expected demo workspace to be visible on dashboard"
        );
    }

    #[test]
    fn dashboard_stage_tracks_pull_request_completion() {
        let state = AppState::demo();
        let workspace_id = workspace_id_by_name(&state, "abandon-about");
        let mut pull_requests: HashMap<WorkspaceId, Option<PullRequestInfo>> = HashMap::new();
        pull_requests.insert(
            workspace_id,
            Some(PullRequestInfo {
                number: 42,
                is_draft: false,
                state: crate::PullRequestState::Merged,
                ci_state: None,
                merge_ready: false,
            }),
        );

        let cards = dashboard_cards(&state, &pull_requests);
        let card = cards
            .iter()
            .find(|c| c.workspace_id == workspace_id)
            .expect("missing card");
        assert_eq!(card.stage, DashboardStage::Finished);
    }

    #[test]
    fn normalize_snippet_returns_none_for_whitespace() {
        assert_eq!(normalize_snippet("   \n\t  "), None);
    }

    #[test]
    fn normalize_snippet_collapses_whitespace() {
        assert_eq!(normalize_snippet("  a  b\tc\n"), Some("a b c".to_owned()));
    }

    #[test]
    fn normalize_snippet_truncates_to_limit() {
        let input = "a".repeat(121);
        let expected = format!("{}…", "a".repeat(120));
        assert_eq!(normalize_snippet(&input), Some(expected));
    }
}
