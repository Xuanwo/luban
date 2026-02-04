use crate::persistence;
use crate::state::{apply_draft_text_diff, entries_is_prefix, entries_is_suffix};
use crate::{
    Action, AgentRunConfig, AppState, AttachmentRef, CodexThreadEvent, ConversationEntry,
    DraftAttachment, Effect, MainPane, OperationStatus, PersistedAppState, Project, ProjectId,
    QueuedPrompt, RightPane, ThinkingEffort, Workspace, WorkspaceConversation, WorkspaceId,
    WorkspaceStatus, WorkspaceTabs, WorkspaceThreadId, default_agent_model_id,
    default_system_prompt_template, default_system_prompt_templates, default_task_prompt_template,
    default_task_prompt_templates, default_thinking_effort, normalize_thinking_effort,
    thinking_effort_supported,
};
use std::collections::VecDeque;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

mod slug;
mod title;

use slug::sanitize_slug;
pub use title::derive_thread_title;

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

fn cancel_running_turn(conversation: &mut WorkspaceConversation) -> Option<u64> {
    if conversation.run_status != OperationStatus::Running {
        return None;
    }
    let run_id = conversation.active_run_id?;
    conversation.run_status = OperationStatus::Idle;
    conversation.current_run_config = None;
    conversation.active_run_id = None;
    conversation.queue_paused = true;
    conversation.push_entry(ConversationEntry::AgentEvent {
        entry_id: String::new(),
        event: crate::AgentEvent::TurnCanceled,
    });
    Some(run_id)
}

fn runner_is_enabled(state: &AppState, runner: crate::AgentRunnerKind) -> bool {
    match runner {
        crate::AgentRunnerKind::Codex => state.agent_codex_enabled,
        crate::AgentRunnerKind::Amp => state.agent_amp_enabled,
        crate::AgentRunnerKind::Claude => state.agent_claude_enabled,
    }
}

fn truncate_for_system_task(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut out = String::new();
    for ch in input.chars().take(max_chars) {
        out.push(ch);
    }
    out
}

fn task_status_auto_update_input(
    conversation: &WorkspaceConversation,
    turn_outcome: &str,
) -> String {
    const MAX_TEXT_CHARS: usize = 1800;

    let mut last_user_message: Option<String> = None;
    let mut last_agent_message: Option<String> = None;
    let mut last_turn_error: Option<String> = None;

    for entry in conversation.entries.iter().rev() {
        match entry {
            ConversationEntry::UserEvent {
                event: crate::UserEvent::Message { text, .. },
                ..
            } => {
                if last_user_message.is_none() {
                    last_user_message = Some(text.trim().to_owned());
                }
            }
            ConversationEntry::AgentEvent { event, .. } => match event {
                crate::AgentEvent::Message { text, .. } => {
                    if last_agent_message.is_none() {
                        last_agent_message = Some(text.trim().to_owned());
                    }
                }
                crate::AgentEvent::TurnError { message } => {
                    if last_turn_error.is_none() {
                        last_turn_error = Some(message.trim().to_owned());
                    }
                }
                _ => {}
            },
            _ => {}
        }

        if last_user_message.is_some() && last_agent_message.is_some() && last_turn_error.is_some()
        {
            break;
        }
    }

    let mut out = String::new();
    out.push_str("Current task_status: ");
    out.push_str(conversation.task_status.as_str());
    out.push('\n');
    out.push_str("Turn outcome: ");
    out.push_str(turn_outcome);
    out.push('\n');

    if let Some(message) = last_user_message {
        let trimmed = message.trim();
        if !trimmed.is_empty() {
            out.push_str("\nLast user message:\n");
            out.push_str(&truncate_for_system_task(trimmed, MAX_TEXT_CHARS));
            out.push('\n');
        }
    }

    if let Some(message) = last_agent_message {
        let trimmed = message.trim();
        if !trimmed.is_empty() {
            out.push_str("\nLast agent message:\n");
            out.push_str(&truncate_for_system_task(trimmed, MAX_TEXT_CHARS));
            out.push('\n');
        }
    }

    if let Some(message) = last_turn_error {
        let trimmed = message.trim();
        if !trimmed.is_empty() {
            out.push_str("\nLast turn error:\n");
            out.push_str(&truncate_for_system_task(trimmed, MAX_TEXT_CHARS));
            out.push('\n');
        }
    }

    out
}

impl AppState {
    const MAIN_WORKSPACE_NAME: &'static str = "main";
    const MAIN_WORKSPACE_BRANCH: &'static str = "main";

    pub fn new() -> Self {
        Self {
            next_project_id: 1,
            next_workspace_id: 1,
            projects: Vec::new(),
            main_pane: MainPane::None,
            right_pane: RightPane::None,
            sidebar_width: None,
            terminal_pane_width: None,
            global_zoom_percent: 100,
            appearance_theme: crate::AppearanceTheme::default(),
            appearance_fonts: crate::AppearanceFonts::default(),
            agent_default_model_id: default_agent_model_id().to_owned(),
            agent_default_thinking_effort: default_thinking_effort(),
            agent_default_runner: crate::default_agent_runner_kind(),
            agent_amp_mode: crate::default_amp_mode().to_owned(),
            agent_codex_enabled: true,
            agent_amp_enabled: true,
            agent_claude_enabled: true,
            conversations: HashMap::new(),
            workspace_tabs: HashMap::new(),
            dashboard_preview_workspace_id: None,
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            last_error: None,
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashSet::new(),
            starred_tasks: HashSet::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            task_prompt_templates: default_task_prompt_templates(),
            system_prompt_templates: default_system_prompt_templates(),
            telegram_enabled: false,
            telegram_bot_token: None,
            telegram_bot_username: None,
            telegram_paired_chat_id: None,
            telegram_config_rev: 0,
            telegram_last_error: None,
            telegram_topic_bindings: HashMap::new(),
        }
    }

    pub fn demo() -> Self {
        let mut this = Self::new();
        let p1 = this.add_project(PathBuf::from("/Users/example/luban"), true);
        let p2 = this.add_project(PathBuf::from("/Users/example/scratch"), true);

        this.projects
            .iter_mut()
            .find(|p| p.id == p1)
            .unwrap()
            .expanded = true;
        this.projects
            .iter_mut()
            .find(|p| p.id == p2)
            .unwrap()
            .expanded = true;

        this.insert_workspace(
            p1,
            "abandon-about",
            "luban/abandon-about",
            PathBuf::from("/Users/example/luban/worktrees/luban/abandon-about"),
        );
        let workspace_id = this
            .projects
            .iter()
            .flat_map(|p| &p.workspaces)
            .find(|w| w.workspace_name == "abandon-about")
            .map(|w| w.id)
            .expect("missing demo workspace");
        this.apply(Action::CreateWorkspaceThread { workspace_id });

        this
    }

    pub fn workspace_has_unread_completion(&self, workspace_id: WorkspaceId) -> bool {
        self.workspace_unread_completions.contains(&workspace_id)
    }

    pub fn workspace_has_running_turn(&self, workspace_id: WorkspaceId) -> bool {
        self.conversations.iter().any(|((id, _), conversation)| {
            *id == workspace_id && conversation.run_status == OperationStatus::Running
        })
    }

    pub fn apply(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::AppStarted => vec![Effect::LoadAppState],

            Action::OpenDashboard => {
                self.main_pane = MainPane::Dashboard;
                self.right_pane = RightPane::None;
                self.dashboard_preview_workspace_id = None;

                let workspace_ids = self
                    .projects
                    .iter()
                    .flat_map(|project| {
                        project.workspaces.iter().filter_map(move |workspace| {
                            if workspace.status != WorkspaceStatus::Active {
                                return None;
                            }
                            if Self::workspace_is_main(project, workspace) {
                                return None;
                            }
                            Some(workspace.id)
                        })
                    })
                    .collect::<Vec<_>>();

                let mut effects = Vec::new();
                for workspace_id in workspace_ids {
                    self.ensure_workspace_tabs_mut(workspace_id);
                    effects.push(Effect::LoadWorkspaceThreads { workspace_id });
                    if let Some(thread_id) = self.active_thread_id(workspace_id) {
                        effects.push(Effect::LoadConversation {
                            workspace_id,
                            thread_id,
                        });
                    }
                }
                effects
            }
            Action::DashboardPreviewOpened { workspace_id } => {
                if self.workspace(workspace_id).is_none() {
                    return Vec::new();
                }
                self.dashboard_preview_workspace_id = Some(workspace_id);
                let cleared = self.workspace_unread_completions.remove(&workspace_id);
                self.ensure_workspace_tabs_mut(workspace_id);
                let mut effects = vec![Effect::LoadWorkspaceThreads { workspace_id }];
                if let Some(thread_id) = self.active_thread_id(workspace_id) {
                    effects.push(Effect::LoadConversation {
                        workspace_id,
                        thread_id,
                    });
                }
                if cleared {
                    effects.insert(0, Effect::SaveAppState);
                }
                effects
            }
            Action::DashboardPreviewClosed => {
                self.dashboard_preview_workspace_id = None;
                Vec::new()
            }

            Action::AddProject { path, is_git } => {
                self.upsert_project(path, is_git);
                vec![Effect::SaveAppState]
            }
            Action::ToggleProjectExpanded { project_id } => {
                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    project.expanded = !project.expanded;
                }
                vec![Effect::SaveAppState]
            }
            Action::DeleteProject { project_id } => self.delete_project(project_id),
            Action::OpenProjectSettings { project_id } => {
                self.main_pane = MainPane::ProjectSettings(project_id);
                self.right_pane = RightPane::None;
                self.dashboard_preview_workspace_id = None;
                Vec::new()
            }

            Action::CreateWorkspace {
                project_id,
                branch_name_hint,
            } => {
                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    if !project.is_git {
                        self.last_error =
                            Some("Cannot create worktrees for a non-git project".to_owned());
                        return Vec::new();
                    }
                    if project.create_workspace_status == OperationStatus::Running {
                        return Vec::new();
                    }
                    project.create_workspace_status = OperationStatus::Running;
                    if project.workspaces.is_empty() {
                        self.insert_main_workspace(project_id);
                    }
                }
                vec![Effect::CreateWorkspace {
                    project_id,
                    branch_name_hint,
                }]
            }
            Action::EnsureMainWorkspace { project_id } => {
                let Some(project) = self.projects.iter().find(|p| p.id == project_id) else {
                    return Vec::new();
                };

                let has_main = project
                    .workspaces
                    .iter()
                    .any(|w| Self::workspace_is_main(project, w));
                if has_main {
                    return Vec::new();
                }

                self.insert_main_workspace(project_id);
                vec![Effect::SaveAppState]
            }
            Action::WorkspaceCreated {
                project_id,
                workspace_name,
                branch_name,
                worktree_path,
            } => {
                let workspace_id =
                    self.insert_workspace(project_id, &workspace_name, &branch_name, worktree_path);
                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    project.create_workspace_status = OperationStatus::Idle;
                }
                self.workspace_tabs.remove(&workspace_id);
                vec![Effect::SaveAppState]
            }
            Action::WorkspaceCreateFailed {
                project_id,
                message,
            } => {
                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    project.create_workspace_status = OperationStatus::Idle;
                }
                self.last_error = Some(message);
                Vec::new()
            }

            Action::OpenWorkspace { workspace_id } => {
                self.main_pane = MainPane::Workspace(workspace_id);
                self.right_pane = RightPane::Terminal;
                self.dashboard_preview_workspace_id = None;
                self.last_open_workspace_id = Some(workspace_id);
                self.workspace_unread_completions.remove(&workspace_id);
                self.ensure_workspace_tabs_mut(workspace_id);
                let mut effects = vec![
                    Effect::SaveAppState,
                    Effect::LoadWorkspaceThreads { workspace_id },
                ];
                if let Some(thread_id) = self.active_thread_id(workspace_id) {
                    effects.push(Effect::LoadConversation {
                        workspace_id,
                        thread_id,
                    });
                }
                effects
            }
            Action::OpenWorkspaceInIde { workspace_id } => {
                if self.workspace(workspace_id).is_none() {
                    self.last_error = Some("Workspace not found".to_owned());
                    return Vec::new();
                }
                vec![Effect::OpenWorkspaceInIde { workspace_id }]
            }
            Action::OpenWorkspaceInIdeFailed { message } => {
                self.last_error = Some(message);
                Vec::new()
            }
            Action::OpenWorkspaceWith {
                workspace_id,
                target,
            } => {
                if self.workspace(workspace_id).is_none() {
                    self.last_error = Some("Workspace not found".to_owned());
                    return Vec::new();
                }
                vec![Effect::OpenWorkspaceWith {
                    workspace_id,
                    target,
                }]
            }
            Action::OpenWorkspaceWithFailed { message } => {
                self.last_error = Some(message);
                Vec::new()
            }
            Action::OpenWorkspacePullRequest { workspace_id } => {
                if self.workspace(workspace_id).is_none() {
                    self.last_error = Some("Workspace not found".to_owned());
                    return Vec::new();
                }
                vec![Effect::OpenWorkspacePullRequest { workspace_id }]
            }
            Action::OpenWorkspacePullRequestFailed { message } => {
                self.last_error = Some(message);
                Vec::new()
            }
            Action::OpenWorkspacePullRequestFailedAction { workspace_id } => {
                if self.workspace(workspace_id).is_none() {
                    self.last_error = Some("Workspace not found".to_owned());
                    return Vec::new();
                }
                vec![Effect::OpenWorkspacePullRequestFailedAction { workspace_id }]
            }
            Action::OpenWorkspacePullRequestFailedActionFailed { message } => {
                self.last_error = Some(message);
                Vec::new()
            }
            Action::ArchiveWorkspace { workspace_id } => {
                let mut cancel_effects = Vec::new();

                if let Some((project_idx, workspace_idx)) =
                    self.find_workspace_indices(workspace_id)
                {
                    let is_main = {
                        let project = &self.projects[project_idx];
                        let workspace = &project.workspaces[workspace_idx];
                        Self::workspace_is_main(project, workspace)
                    };
                    if is_main {
                        return Vec::new();
                    }

                    {
                        let project = &mut self.projects[project_idx];
                        let workspace = &mut project.workspaces[workspace_idx];

                        if workspace.archive_status == OperationStatus::Running {
                            return Vec::new();
                        }
                        workspace.archive_status = OperationStatus::Running;
                        project.expanded = true;
                    }

                    for ((wid, thread_id), conversation) in self.conversations.iter_mut() {
                        if *wid != workspace_id {
                            continue;
                        }
                        if let Some(run_id) = cancel_running_turn(conversation) {
                            cancel_effects.push(Effect::CancelAgentTurn {
                                workspace_id,
                                thread_id: *thread_id,
                                run_id,
                            });
                        }
                    }
                }
                cancel_effects
                    .into_iter()
                    .chain(std::iter::once(Effect::ArchiveWorkspace { workspace_id }))
                    .collect()
            }
            Action::WorkspaceArchived { workspace_id } => {
                if let Some((project_idx, workspace_idx)) =
                    self.find_workspace_indices(workspace_id)
                {
                    let workspace = &mut self.projects[project_idx].workspaces[workspace_idx];
                    workspace.archive_status = OperationStatus::Idle;
                    workspace.status = WorkspaceStatus::Archived;
                }
                if self.last_open_workspace_id == Some(workspace_id) {
                    self.last_open_workspace_id = None;
                }
                if matches!(self.main_pane, MainPane::Workspace(id) if id == workspace_id) {
                    self.main_pane = MainPane::None;
                    self.right_pane = RightPane::None;
                }
                if self.dashboard_preview_workspace_id == Some(workspace_id) {
                    self.dashboard_preview_workspace_id = None;
                }
                vec![Effect::SaveAppState]
            }
            Action::WorkspaceArchiveFailed {
                workspace_id,
                message,
            } => {
                if let Some((project_idx, workspace_idx)) =
                    self.find_workspace_indices(workspace_id)
                {
                    let workspace = &mut self.projects[project_idx].workspaces[workspace_idx];
                    workspace.archive_status = OperationStatus::Idle;
                }
                self.last_error = Some(message);
                Vec::new()
            }

            Action::WorkspaceBranchRenameRequested {
                workspace_id,
                requested_branch_name,
            } => {
                let Some((project_idx, workspace_idx)) = self.find_workspace_indices(workspace_id)
                else {
                    return Vec::new();
                };

                let project = &self.projects[project_idx];
                let workspace = &project.workspaces[workspace_idx];
                if !project.is_git {
                    return Vec::new();
                }
                if Self::workspace_is_main(project, workspace) {
                    return Vec::new();
                }

                let workspace = &mut self.projects[project_idx].workspaces[workspace_idx];
                if workspace.branch_rename_status == OperationStatus::Running {
                    return Vec::new();
                }
                workspace.branch_rename_status = OperationStatus::Running;

                vec![Effect::RenameWorkspaceBranch {
                    workspace_id,
                    requested_branch_name,
                }]
            }
            Action::WorkspaceBranchAiRenameRequested {
                workspace_id,
                thread_id,
            } => {
                let default_amp_mode = self.agent_amp_mode.clone();
                let Some((project_idx, workspace_idx)) = self.find_workspace_indices(workspace_id)
                else {
                    return Vec::new();
                };

                let project = &self.projects[project_idx];
                let workspace = &project.workspaces[workspace_idx];
                if !project.is_git {
                    return Vec::new();
                }
                if Self::workspace_is_main(project, workspace) {
                    return Vec::new();
                }

                let current_branch_name = self.projects[project_idx].workspaces[workspace_idx]
                    .branch_name
                    .clone();
                if self.projects[project_idx].workspaces[workspace_idx].branch_rename_status
                    == OperationStatus::Running
                {
                    return Vec::new();
                }

                let (runner, model_id, thinking_effort, amp_mode, input) = self
                    .conversations
                    .get(&(workspace_id, thread_id))
                    .map(|conversation| {
                        const MAX_USER_MESSAGES: usize = 6;
                        let input = conversation
                            .entries
                            .iter()
                            .filter_map(|entry| match entry {
                                ConversationEntry::UserEvent { event, .. } => match event {
                                    crate::UserEvent::Message { text, .. } => {
                                        Some(text.trim().to_owned())
                                    }
                                },
                                _ => None,
                            })
                            .filter(|text| !text.is_empty())
                            .take(MAX_USER_MESSAGES)
                            .collect::<Vec<_>>()
                            .join("\n\n");

                        let runner = conversation.agent_runner;
                        let model_id = conversation.agent_model_id.clone();
                        let thinking_effort = conversation.thinking_effort;
                        let amp_mode = if runner == crate::AgentRunnerKind::Amp {
                            conversation
                                .amp_mode
                                .clone()
                                .or(Some(default_amp_mode.clone()))
                        } else {
                            None
                        };

                        (runner, model_id, thinking_effort, amp_mode, input)
                    })
                    .unwrap_or_else(|| {
                        let runner = self.agent_default_runner;
                        let model_id = self.agent_default_model_id.clone();
                        let thinking_effort = self.agent_default_thinking_effort;
                        let amp_mode = if runner == crate::AgentRunnerKind::Amp {
                            Some(default_amp_mode.clone())
                        } else {
                            None
                        };
                        (runner, model_id, thinking_effort, amp_mode, String::new())
                    });

                if !runner_is_enabled(self, runner) {
                    return Vec::new();
                }

                let workspace = &mut self.projects[project_idx].workspaces[workspace_idx];
                workspace.branch_rename_status = OperationStatus::Running;

                let input = if input.trim().is_empty() {
                    current_branch_name
                } else {
                    input
                };

                vec![Effect::AiRenameWorkspaceBranch {
                    workspace_id,
                    input,
                    runner,
                    model_id,
                    thinking_effort,
                    amp_mode,
                }]
            }
            Action::WorkspaceBranchRenamed {
                workspace_id,
                branch_name,
            } => {
                if let Some((project_idx, workspace_idx)) =
                    self.find_workspace_indices(workspace_id)
                {
                    let workspace = &mut self.projects[project_idx].workspaces[workspace_idx];
                    workspace.branch_name = branch_name;
                    workspace.branch_rename_status = OperationStatus::Idle;
                }
                Vec::new()
            }
            Action::WorkspaceBranchSynced {
                workspace_id,
                branch_name,
            } => {
                let Some((project_idx, workspace_idx)) = self.find_workspace_indices(workspace_id)
                else {
                    return Vec::new();
                };

                let project = &self.projects[project_idx];
                if !project.is_git {
                    return Vec::new();
                }

                let workspace = &mut self.projects[project_idx].workspaces[workspace_idx];
                if workspace.branch_name == branch_name {
                    return Vec::new();
                }
                workspace.branch_name = branch_name;
                vec![Effect::SaveAppState]
            }
            Action::WorkspaceBranchRenameFailed {
                workspace_id,
                message,
            } => {
                if let Some((project_idx, workspace_idx)) =
                    self.find_workspace_indices(workspace_id)
                {
                    let workspace = &mut self.projects[project_idx].workspaces[workspace_idx];
                    workspace.branch_rename_status = OperationStatus::Idle;
                }
                self.last_error = Some(message);
                Vec::new()
            }

            Action::ConversationLoaded {
                workspace_id,
                thread_id,
                snapshot,
            } => {
                let default_amp_mode = self.agent_amp_mode.clone();
                let mut snapshot = snapshot;
                snapshot.ensure_entry_ids();
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                let local_has_non_system_entries = conversation
                    .entries
                    .iter()
                    .any(|e| !matches!(e, ConversationEntry::SystemEvent { .. }));
                if let Some(title) = snapshot
                    .title
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    conversation.title = title.to_owned();
                }
                let snapshot_model_id = snapshot
                    .agent_model_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToOwned::to_owned);
                let snapshot_thinking_effort = snapshot.thinking_effort;
                let snapshot_runner = snapshot.runner;
                let snapshot_amp_mode = snapshot.amp_mode.clone();

                if conversation.thread_id.is_none() {
                    conversation.thread_id = snapshot.thread_id.clone();
                }

                let should_apply_snapshot_run_config = !conversation.run_config_overridden_by_user
                    || conversation.agent_model_id.trim().is_empty();
                if should_apply_snapshot_run_config {
                    if let Some(runner) = snapshot_runner {
                        conversation.agent_runner = runner;
                    }
                    if let Some(mode) = snapshot_amp_mode
                        .as_deref()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                    {
                        conversation.amp_mode = Some(mode.to_owned());
                    }
                    if conversation.agent_runner == crate::AgentRunnerKind::Amp
                        && conversation.amp_mode.is_none()
                    {
                        conversation.amp_mode = Some(default_amp_mode);
                    }

                    if let Some(model_id) = snapshot_model_id {
                        let effort =
                            snapshot_thinking_effort.unwrap_or(conversation.thinking_effort);
                        let normalized = normalize_thinking_effort(&model_id, effort);
                        conversation.agent_model_id = model_id;
                        conversation.thinking_effort = normalized;
                    } else if let Some(effort) = snapshot_thinking_effort {
                        conversation.thinking_effort =
                            normalize_thinking_effort(&conversation.agent_model_id, effort);
                    }
                }

                let should_apply_queue_snapshot = !local_has_non_system_entries
                    && conversation.pending_prompts.is_empty()
                    && conversation.run_status == OperationStatus::Idle;
                if should_apply_queue_snapshot {
                    conversation.pending_prompts = VecDeque::from(snapshot.pending_prompts.clone());
                    conversation.queue_paused = snapshot.queue_paused;
                    conversation.run_started_at_unix_ms = snapshot.run_started_at_unix_ms;
                    conversation.run_finished_at_unix_ms = snapshot.run_finished_at_unix_ms;
                    conversation.next_queued_prompt_id = conversation
                        .pending_prompts
                        .iter()
                        .map(|prompt| prompt.id)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1);
                }

                if !local_has_non_system_entries {
                    conversation.reset_entries_from_snapshot(snapshot);
                    return Vec::new();
                }

                let snapshot_is_newer = entries_is_prefix(&conversation.entries, &snapshot.entries)
                    || entries_is_suffix(&conversation.entries, &snapshot.entries);
                let conversation_is_newer =
                    entries_is_prefix(&snapshot.entries, &conversation.entries)
                        || entries_is_suffix(&snapshot.entries, &conversation.entries);

                if snapshot_is_newer && !conversation_is_newer {
                    conversation.reset_entries_from_snapshot(snapshot);
                }

                Vec::new()
            }
            Action::ConversationLoadFailed {
                workspace_id: _,
                thread_id: _,
                message,
            } => {
                self.last_error = Some(message);
                Vec::new()
            }
            Action::SendAgentMessage {
                workspace_id,
                thread_id,
                text,
                attachments,
                runner,
                amp_mode,
            } => {
                let default_amp_mode = self.agent_amp_mode.clone();
                let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                tabs.activate(thread_id);

                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                conversation.draft.clear();
                conversation.draft_attachments.clear();

                let mut task_status_effects = Vec::new();
                if matches!(
                    conversation.task_status,
                    crate::TaskStatus::Backlog | crate::TaskStatus::Todo
                ) {
                    conversation.task_status = crate::TaskStatus::Iterating;
                    task_status_effects.push(Effect::StoreConversationTaskStatus {
                        workspace_id,
                        thread_id,
                        task_status: conversation.task_status,
                    });
                    task_status_effects.push(Effect::LoadWorkspaceThreads { workspace_id });
                }

                let runner = runner.unwrap_or(conversation.agent_runner);
                let amp_mode = if runner == crate::AgentRunnerKind::Amp {
                    amp_mode
                        .or(conversation.amp_mode.clone())
                        .or(Some(default_amp_mode))
                } else {
                    None
                };

                let has_non_system_entries = conversation.entries.iter().any(|entry| {
                    matches!(
                        entry,
                        ConversationEntry::UserEvent { .. } | ConversationEntry::AgentEvent { .. }
                    )
                });
                let should_auto_title =
                    !has_non_system_entries && conversation.title.starts_with("Thread ");
                let input_for_auto_title = if should_auto_title {
                    text.clone()
                } else {
                    String::new()
                };
                let mut expected_current_title = conversation.title.clone();
                if should_auto_title {
                    let title = derive_thread_title(&text);
                    if !title.is_empty() {
                        conversation.title = title.clone();
                        expected_current_title = title;
                    }
                }

                let run_config = AgentRunConfig {
                    runner,
                    model_id: conversation.agent_model_id.clone(),
                    thinking_effort: conversation.thinking_effort,
                    amp_mode,
                };

                if conversation.run_status == OperationStatus::Running {
                    let id = conversation.next_queued_prompt_id;
                    conversation.next_queued_prompt_id =
                        conversation.next_queued_prompt_id.saturating_add(1);
                    conversation.pending_prompts.push_back(QueuedPrompt {
                        id,
                        text,
                        attachments,
                        run_config,
                    });
                    return task_status_effects;
                }

                if conversation.queue_paused && !conversation.pending_prompts.is_empty() {
                    let mut effects = task_status_effects;
                    effects.push(start_agent_run(
                        conversation,
                        workspace_id,
                        thread_id,
                        text,
                        attachments,
                        run_config,
                    ));
                    return effects;
                }

                if conversation.pending_prompts.is_empty() {
                    conversation.queue_paused = false;
                    let mut effects = task_status_effects;
                    let run_config_for_system_tasks = run_config.clone();
                    effects.push(start_agent_run(
                        conversation,
                        workspace_id,
                        thread_id,
                        text,
                        attachments,
                        run_config,
                    ));
                    if should_auto_title {
                        effects.push(Effect::LoadWorkspaceThreads { workspace_id });
                        if runner_is_enabled(self, run_config_for_system_tasks.runner) {
                            effects.push(Effect::AiAutoTitleThread {
                                workspace_id,
                                thread_id,
                                input: input_for_auto_title,
                                expected_current_title,
                                runner: run_config_for_system_tasks.runner,
                                model_id: run_config_for_system_tasks.model_id.clone(),
                                thinking_effort: run_config_for_system_tasks.thinking_effort,
                                amp_mode: run_config_for_system_tasks.amp_mode.clone(),
                            });
                        }
                    }
                    return effects;
                }

                let id = conversation.next_queued_prompt_id;
                conversation.next_queued_prompt_id =
                    conversation.next_queued_prompt_id.saturating_add(1);
                conversation.pending_prompts.push_back(QueuedPrompt {
                    id,
                    text,
                    attachments,
                    run_config,
                });
                let mut effects = task_status_effects;
                effects.extend(start_next_queued_prompt(
                    conversation,
                    workspace_id,
                    thread_id,
                ));
                effects
            }
            Action::QueueAgentMessage {
                workspace_id,
                thread_id,
                text,
                attachments,
                runner,
                amp_mode,
            } => {
                let default_amp_mode = self.agent_amp_mode.clone();
                let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                tabs.activate(thread_id);

                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                conversation.draft.clear();
                conversation.draft_attachments.clear();

                let runner = runner.unwrap_or(conversation.agent_runner);
                let amp_mode = if runner == crate::AgentRunnerKind::Amp {
                    amp_mode
                        .or(conversation.amp_mode.clone())
                        .or(Some(default_amp_mode))
                } else {
                    None
                };

                let has_non_system_entries = conversation.entries.iter().any(|entry| {
                    matches!(
                        entry,
                        ConversationEntry::UserEvent { .. } | ConversationEntry::AgentEvent { .. }
                    )
                });
                if !has_non_system_entries && conversation.title.starts_with("Thread ") {
                    let title = derive_thread_title(&text);
                    if !title.is_empty() {
                        conversation.title = title;
                    }
                }

                let run_config = AgentRunConfig {
                    runner,
                    model_id: conversation.agent_model_id.clone(),
                    thinking_effort: conversation.thinking_effort,
                    amp_mode,
                };

                let id = conversation.next_queued_prompt_id;
                conversation.next_queued_prompt_id =
                    conversation.next_queued_prompt_id.saturating_add(1);
                conversation.pending_prompts.push_back(QueuedPrompt {
                    id,
                    text,
                    attachments,
                    run_config,
                });
                Vec::new()
            }
            Action::ChatModelChanged {
                workspace_id,
                thread_id,
                model_id,
            } => {
                let default_amp_mode = self.agent_amp_mode.clone();
                let (thinking_effort, runner, amp_mode) = {
                    let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                    let normalized =
                        normalize_thinking_effort(&model_id, conversation.thinking_effort);
                    conversation.run_config_overridden_by_user = true;
                    conversation.agent_model_id = model_id.clone();
                    conversation.thinking_effort = normalized;
                    let runner = conversation.agent_runner;
                    let amp_mode = if runner == crate::AgentRunnerKind::Amp {
                        conversation.amp_mode.clone().or(Some(default_amp_mode))
                    } else {
                        None
                    };
                    (normalized, runner, amp_mode)
                };
                self.workspace_thread_run_config_overrides.insert(
                    (workspace_id, thread_id),
                    crate::PersistedWorkspaceThreadRunConfigOverride {
                        runner: Some(runner.as_str().to_owned()),
                        amp_mode: amp_mode.clone(),
                        model_id: model_id.clone(),
                        thinking_effort: thinking_effort.as_str().to_owned(),
                    },
                );
                vec![
                    Effect::StoreConversationRunConfig {
                        workspace_id,
                        thread_id,
                        runner,
                        model_id,
                        thinking_effort,
                        amp_mode,
                    },
                    Effect::SaveAppState,
                ]
            }
            Action::ChatRunnerChanged {
                workspace_id,
                thread_id,
                runner,
            } => {
                let default_amp_mode = self.agent_amp_mode.clone();
                let (model_id, thinking_effort, amp_mode) = {
                    let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                    conversation.run_config_overridden_by_user = true;
                    conversation.agent_runner = runner;
                    if runner == crate::AgentRunnerKind::Amp && conversation.amp_mode.is_none() {
                        conversation.amp_mode = Some(default_amp_mode);
                    }
                    let model_id = conversation.agent_model_id.clone();
                    let thinking_effort = conversation.thinking_effort;
                    let amp_mode = if runner == crate::AgentRunnerKind::Amp {
                        conversation.amp_mode.clone()
                    } else {
                        None
                    };
                    (model_id, thinking_effort, amp_mode)
                };
                self.workspace_thread_run_config_overrides.insert(
                    (workspace_id, thread_id),
                    crate::PersistedWorkspaceThreadRunConfigOverride {
                        runner: Some(runner.as_str().to_owned()),
                        amp_mode: amp_mode.clone(),
                        model_id: model_id.clone(),
                        thinking_effort: thinking_effort.as_str().to_owned(),
                    },
                );
                vec![
                    Effect::StoreConversationRunConfig {
                        workspace_id,
                        thread_id,
                        runner,
                        model_id,
                        thinking_effort,
                        amp_mode,
                    },
                    Effect::SaveAppState,
                ]
            }
            Action::ChatAmpModeChanged {
                workspace_id,
                thread_id,
                amp_mode,
            } => {
                let trimmed = amp_mode.trim();
                if trimmed.is_empty() {
                    return Vec::new();
                }

                let (runner, model_id, thinking_effort, amp_mode) = {
                    let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                    conversation.run_config_overridden_by_user = true;
                    conversation.amp_mode = Some(trimmed.to_owned());
                    let runner = conversation.agent_runner;
                    let model_id = conversation.agent_model_id.clone();
                    let thinking_effort = conversation.thinking_effort;
                    let amp_mode = if runner == crate::AgentRunnerKind::Amp {
                        conversation.amp_mode.clone()
                    } else {
                        None
                    };
                    (runner, model_id, thinking_effort, amp_mode)
                };
                self.workspace_thread_run_config_overrides.insert(
                    (workspace_id, thread_id),
                    crate::PersistedWorkspaceThreadRunConfigOverride {
                        runner: Some(runner.as_str().to_owned()),
                        amp_mode: amp_mode.clone(),
                        model_id: model_id.clone(),
                        thinking_effort: thinking_effort.as_str().to_owned(),
                    },
                );
                vec![
                    Effect::StoreConversationRunConfig {
                        workspace_id,
                        thread_id,
                        runner,
                        model_id,
                        thinking_effort,
                        amp_mode,
                    },
                    Effect::SaveAppState,
                ]
            }
            Action::ThinkingEffortChanged {
                workspace_id,
                thread_id,
                thinking_effort,
            } => {
                let default_amp_mode = self.agent_amp_mode.clone();
                let (runner, model_id, thinking_effort, amp_mode) = {
                    let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                    if !thinking_effort_supported(&conversation.agent_model_id, thinking_effort) {
                        return Vec::new();
                    }
                    conversation.run_config_overridden_by_user = true;
                    conversation.thinking_effort = thinking_effort;
                    let runner = conversation.agent_runner;
                    let model_id = conversation.agent_model_id.clone();
                    let amp_mode = if runner == crate::AgentRunnerKind::Amp {
                        conversation.amp_mode.clone().or(Some(default_amp_mode))
                    } else {
                        None
                    };
                    (runner, model_id, thinking_effort, amp_mode)
                };
                self.workspace_thread_run_config_overrides.insert(
                    (workspace_id, thread_id),
                    crate::PersistedWorkspaceThreadRunConfigOverride {
                        runner: Some(runner.as_str().to_owned()),
                        amp_mode: amp_mode.clone(),
                        model_id: model_id.clone(),
                        thinking_effort: thinking_effort.as_str().to_owned(),
                    },
                );
                vec![
                    Effect::StoreConversationRunConfig {
                        workspace_id,
                        thread_id,
                        runner,
                        model_id,
                        thinking_effort,
                        amp_mode,
                    },
                    Effect::SaveAppState,
                ]
            }
            Action::ChatDraftChanged {
                workspace_id,
                thread_id,
                text,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                apply_draft_text_diff(conversation, &text);
                Vec::new()
            }
            Action::ChatDraftAttachmentAdded {
                workspace_id,
                thread_id,
                id,
                kind,
                anchor,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                conversation.draft_attachments.push(DraftAttachment {
                    id,
                    kind,
                    anchor,
                    attachment: None,
                    failed: false,
                });
                Vec::new()
            }
            Action::ChatDraftAttachmentResolved {
                workspace_id,
                thread_id,
                id,
                attachment: resolved,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                if let Some(attachment) = conversation
                    .draft_attachments
                    .iter_mut()
                    .find(|a| a.id == id)
                {
                    attachment.attachment = Some(resolved);
                    attachment.failed = false;
                }
                Vec::new()
            }
            Action::ChatDraftAttachmentFailed {
                workspace_id,
                thread_id,
                id,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                if let Some(attachment) = conversation
                    .draft_attachments
                    .iter_mut()
                    .find(|a| a.id == id)
                {
                    attachment.failed = true;
                }
                Vec::new()
            }
            Action::ChatDraftAttachmentRemoved {
                workspace_id,
                thread_id,
                id,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                conversation.draft_attachments.retain(|a| a.id != id);
                Vec::new()
            }
            Action::RemoveQueuedPrompt {
                workspace_id,
                thread_id,
                prompt_id,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                if let Some(pos) = conversation
                    .pending_prompts
                    .iter()
                    .position(|p| p.id == prompt_id)
                {
                    let _ = conversation.pending_prompts.remove(pos);
                }
                Vec::new()
            }
            Action::ReorderQueuedPrompt {
                workspace_id,
                thread_id,
                active_id,
                over_id,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                if active_id == over_id {
                    return Vec::new();
                }

                let from = conversation
                    .pending_prompts
                    .iter()
                    .position(|p| p.id == active_id);
                let to = conversation
                    .pending_prompts
                    .iter()
                    .position(|p| p.id == over_id);
                let (Some(from), Some(to)) = (from, to) else {
                    return Vec::new();
                };
                if from == to {
                    return Vec::new();
                }

                let mut items = conversation
                    .pending_prompts
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let item = items.remove(from);
                items.insert(to, item);
                conversation.pending_prompts = VecDeque::from(items);
                Vec::new()
            }
            Action::UpdateQueuedPrompt {
                workspace_id,
                thread_id,
                prompt_id,
                text,
                attachments,
                model_id,
                thinking_effort,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                let Some(pos) = conversation
                    .pending_prompts
                    .iter()
                    .position(|p| p.id == prompt_id)
                else {
                    return Vec::new();
                };

                let trimmed = text.trim().to_owned();
                if trimmed.is_empty() && attachments.is_empty() {
                    let _ = conversation.pending_prompts.remove(pos);
                    return Vec::new();
                }

                let normalized_effort = normalize_thinking_effort(&model_id, thinking_effort);
                let entry = conversation.pending_prompts.get_mut(pos).unwrap();
                entry.text = trimmed;
                entry.attachments = attachments;
                let runner = entry.run_config.runner;
                let amp_mode = entry.run_config.amp_mode.clone();
                entry.run_config = AgentRunConfig {
                    runner,
                    model_id,
                    thinking_effort: normalized_effort,
                    amp_mode,
                };
                Vec::new()
            }
            Action::ClearQueuedPrompts {
                workspace_id,
                thread_id,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                conversation.pending_prompts.clear();
                Vec::new()
            }
            Action::ResumeQueuedPrompts {
                workspace_id,
                thread_id,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                conversation.queue_paused = false;
                start_next_queued_prompt(conversation, workspace_id, thread_id)
                    .into_iter()
                    .collect()
            }
            Action::AgentRunStartedAt {
                workspace_id,
                thread_id,
                run_id,
                started_at_unix_ms,
            } => {
                let Some(conversation) = self.conversations.get_mut(&(workspace_id, thread_id))
                else {
                    return Vec::new();
                };
                if conversation.active_run_id != Some(run_id) {
                    return Vec::new();
                }
                conversation.run_started_at_unix_ms = Some(started_at_unix_ms);
                conversation.run_finished_at_unix_ms = None;
                Vec::new()
            }
            Action::AgentRunFinishedAt {
                workspace_id,
                thread_id,
                run_id,
                finished_at_unix_ms,
            } => {
                let Some(conversation) = self.conversations.get_mut(&(workspace_id, thread_id))
                else {
                    return Vec::new();
                };
                if conversation.active_run_id != Some(run_id) {
                    return Vec::new();
                }
                conversation.run_finished_at_unix_ms = Some(finished_at_unix_ms);
                Vec::new()
            }
            Action::AgentEventReceived {
                workspace_id,
                thread_id,
                run_id,
                event,
            } => {
                let agent_codex_enabled = self.agent_codex_enabled;
                let agent_amp_enabled = self.agent_amp_enabled;
                let agent_claude_enabled = self.agent_claude_enabled;
                let mut last_error_message: Option<String> = None;
                let effects = {
                    let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                    match event {
                        CodexThreadEvent::ThreadStarted { thread_id } => {
                            if conversation.thread_id.is_none() {
                                conversation.thread_id = Some(thread_id);
                            }
                            Vec::new()
                        }
                        CodexThreadEvent::TurnStarted => Vec::new(),
                        CodexThreadEvent::TurnCompleted { usage } => {
                            if conversation.active_run_id != Some(run_id) {
                                return Vec::new();
                            }
                            let _ = usage;
                            let finished_run_config = conversation
                                .current_run_config
                                .clone()
                                .unwrap_or(AgentRunConfig {
                                    runner: conversation.agent_runner,
                                    model_id: conversation.agent_model_id.clone(),
                                    thinking_effort: conversation.thinking_effort,
                                    amp_mode: conversation.amp_mode.clone(),
                                });
                            conversation.run_status = OperationStatus::Idle;
                            conversation.current_run_config = None;
                            let next =
                                start_next_queued_prompt(conversation, workspace_id, thread_id);
                            if let Some(effect) = next {
                                return vec![effect];
                            }

                            if !matches!(
                                conversation.task_status,
                                crate::TaskStatus::Iterating | crate::TaskStatus::Validating
                            ) {
                                return Vec::new();
                            }
                            let runner_enabled = match finished_run_config.runner {
                                crate::AgentRunnerKind::Codex => agent_codex_enabled,
                                crate::AgentRunnerKind::Amp => agent_amp_enabled,
                                crate::AgentRunnerKind::Claude => agent_claude_enabled,
                            };
                            if !runner_enabled {
                                return Vec::new();
                            }

                            vec![Effect::AiAutoUpdateTaskStatus {
                                workspace_id,
                                thread_id,
                                input: task_status_auto_update_input(conversation, "completed"),
                                expected_current_task_status: conversation.task_status,
                                runner: finished_run_config.runner,
                                model_id: finished_run_config.model_id.clone(),
                                thinking_effort: finished_run_config.thinking_effort,
                                amp_mode: finished_run_config.amp_mode.clone(),
                            }]
                        }
                        CodexThreadEvent::TurnDuration { duration_ms } => {
                            if conversation.active_run_id != Some(run_id) {
                                return Vec::new();
                            }
                            conversation.push_entry(ConversationEntry::AgentEvent {
                                entry_id: String::new(),
                                event: crate::AgentEvent::TurnDuration { duration_ms },
                            });
                            Vec::new()
                        }
                        CodexThreadEvent::TurnFailed { error } => {
                            if conversation.active_run_id != Some(run_id) {
                                return Vec::new();
                            }
                            let finished_run_config = conversation
                                .current_run_config
                                .clone()
                                .unwrap_or(AgentRunConfig {
                                    runner: conversation.agent_runner,
                                    model_id: conversation.agent_model_id.clone(),
                                    thinking_effort: conversation.thinking_effort,
                                    amp_mode: conversation.amp_mode.clone(),
                                });
                            let error_message = error.message.clone();
                            conversation.push_entry(ConversationEntry::AgentEvent {
                                entry_id: String::new(),
                                event: crate::AgentEvent::TurnError {
                                    message: error_message.clone(),
                                },
                            });
                            conversation.run_status = OperationStatus::Idle;
                            conversation.current_run_config = None;
                            conversation.queue_paused = true;
                            last_error_message = Some(error_message);

                            let should_auto_update = matches!(
                                conversation.task_status,
                                crate::TaskStatus::Iterating | crate::TaskStatus::Validating
                            );
                            let runner_enabled = match finished_run_config.runner {
                                crate::AgentRunnerKind::Codex => agent_codex_enabled,
                                crate::AgentRunnerKind::Amp => agent_amp_enabled,
                                crate::AgentRunnerKind::Claude => agent_claude_enabled,
                            };

                            if should_auto_update && runner_enabled {
                                vec![Effect::AiAutoUpdateTaskStatus {
                                    workspace_id,
                                    thread_id,
                                    input: task_status_auto_update_input(conversation, "failed"),
                                    expected_current_task_status: conversation.task_status,
                                    runner: finished_run_config.runner,
                                    model_id: finished_run_config.model_id.clone(),
                                    thinking_effort: finished_run_config.thinking_effort,
                                    amp_mode: finished_run_config.amp_mode.clone(),
                                }]
                            } else {
                                Vec::new()
                            }
                        }
                        CodexThreadEvent::ItemStarted { item }
                        | CodexThreadEvent::ItemUpdated { item } => {
                            if conversation.active_run_id != Some(run_id) {
                                return Vec::new();
                            }
                            conversation.push_codex_item(item);
                            Vec::new()
                        }
                        CodexThreadEvent::ItemCompleted { item } => {
                            if conversation.active_run_id != Some(run_id) {
                                return Vec::new();
                            }
                            conversation.push_codex_item(item);
                            Vec::new()
                        }
                        CodexThreadEvent::Error { message } => {
                            if conversation.active_run_id != Some(run_id) {
                                return Vec::new();
                            }
                            conversation.push_entry(ConversationEntry::AgentEvent {
                                entry_id: String::new(),
                                event: crate::AgentEvent::TurnError {
                                    message: message.clone(),
                                },
                            });
                            conversation.run_status = OperationStatus::Idle;
                            conversation.current_run_config = None;
                            conversation.queue_paused = true;
                            last_error_message = Some(message);
                            Vec::new()
                        }
                    }
                };
                if let Some(message) = last_error_message {
                    self.last_error = Some(message);
                }

                effects
            }
            Action::AgentTurnFinished {
                workspace_id,
                thread_id,
                run_id,
            } => {
                let is_visible = matches!(self.main_pane, MainPane::Workspace(id) if id == workspace_id)
                    || self.dashboard_preview_workspace_id == Some(workspace_id);
                let mut effects = Vec::new();

                if let Some(conversation) = self.conversations.get_mut(&(workspace_id, thread_id)) {
                    if conversation.active_run_id != Some(run_id) {
                        return Vec::new();
                    }
                    conversation.active_run_id = None;
                    if conversation.run_status == OperationStatus::Running {
                        conversation.run_status = OperationStatus::Idle;
                        conversation.current_run_config = None;
                    }
                }

                if !is_visible && self.workspace(workspace_id).is_some() {
                    let inserted = self.workspace_unread_completions.insert(workspace_id);
                    if inserted {
                        effects.push(Effect::SaveAppState);
                    }
                }

                effects
            }
            Action::CancelAgentTurn {
                workspace_id,
                thread_id,
            } => {
                let Some(conversation) = self.conversations.get_mut(&(workspace_id, thread_id))
                else {
                    return Vec::new();
                };
                let Some(run_id) = cancel_running_turn(conversation) else {
                    return Vec::new();
                };
                vec![Effect::CancelAgentTurn {
                    workspace_id,
                    thread_id,
                    run_id,
                }]
            }
            Action::CreateWorkspaceThread { workspace_id } => {
                let thread_id = {
                    let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                    tabs.allocate_thread_id()
                };
                let mut conversation = self.default_conversation(thread_id);
                conversation.task_status = crate::TaskStatus::Backlog;
                conversation.push_entry(ConversationEntry::SystemEvent {
                    entry_id: format!("sys_{}", conversation.entries_total.saturating_add(1)),
                    created_at_unix_ms: now_unix_ms(),
                    event: crate::ConversationSystemEvent::TaskCreated,
                });
                self.conversations
                    .insert((workspace_id, thread_id), conversation);
                self.ensure_workspace_tabs_mut(workspace_id)
                    .activate(thread_id);
                vec![
                    Effect::SaveAppState,
                    Effect::EnsureConversation {
                        workspace_id,
                        thread_id,
                    },
                    Effect::LoadWorkspaceThreads { workspace_id },
                ]
            }
            Action::ActivateWorkspaceThread {
                workspace_id,
                thread_id,
            } => {
                let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                tabs.activate(thread_id);
                self.ensure_conversation_mut(workspace_id, thread_id);
                vec![
                    Effect::SaveAppState,
                    Effect::LoadConversation {
                        workspace_id,
                        thread_id,
                    },
                ]
            }
            Action::CloseWorkspaceThreadTab {
                workspace_id,
                thread_id,
            } => {
                let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                if tabs.open_tabs.len() <= 1 {
                    return Vec::new();
                }
                let previous_active = tabs.active_tab;
                tabs.archive_tab(thread_id);
                let mut effects = vec![
                    Effect::SaveAppState,
                    // Clean up any persistent Claude process associated with this thread
                    Effect::CleanupClaudeProcess {
                        workspace_id,
                        thread_id,
                    },
                ];
                if tabs.active_tab != previous_active {
                    effects.push(Effect::LoadConversation {
                        workspace_id,
                        thread_id: tabs.active_tab,
                    });
                }
                effects
            }
            Action::RestoreWorkspaceThreadTab {
                workspace_id,
                thread_id,
            } => {
                let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                let previous_active = tabs.active_tab;
                tabs.restore_tab(thread_id, true);
                let mut effects = vec![Effect::SaveAppState];
                if tabs.active_tab != previous_active {
                    effects.push(Effect::LoadConversation {
                        workspace_id,
                        thread_id,
                    });
                }
                effects
            }
            Action::ReorderWorkspaceThreadTab {
                workspace_id,
                thread_id,
                to_index,
            } => {
                let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                if tabs.reorder_tab(thread_id, to_index) {
                    vec![Effect::SaveAppState]
                } else {
                    Vec::new()
                }
            }
            Action::WorkspaceThreadsLoaded {
                workspace_id,
                threads,
            } => {
                self.ensure_workspace_tabs_mut(workspace_id);
                let default_model_id = self.agent_default_model_id.clone();
                let default_thinking_effort = self.agent_default_thinking_effort;
                let mut max_thread_id = 0u64;
                let mut loaded_thread_ids = Vec::new();
                for meta in threads {
                    max_thread_id = max_thread_id.max(meta.thread_id.0);
                    loaded_thread_ids.push(meta.thread_id);
                    let run_config_override = self
                        .workspace_thread_run_config_overrides
                        .get(&(workspace_id, meta.thread_id))
                        .cloned();
                    let conversation = self
                        .conversations
                        .entry((workspace_id, meta.thread_id))
                        .or_insert_with(|| {
                            let mut conversation = Self::default_conversation_with_defaults(
                                meta.thread_id,
                                default_model_id.clone(),
                                default_thinking_effort,
                                self.agent_default_runner,
                            );
                            if let Some(run_config) = run_config_override.clone() {
                                let mut overridden = false;
                                if let Some(runner) = run_config
                                    .runner
                                    .as_deref()
                                    .and_then(crate::agent_settings::parse_agent_runner_kind)
                                {
                                    conversation.run_config_overridden_by_user = true;
                                    conversation.agent_runner = runner;
                                    overridden = true;
                                }
                                if let Some(mode) = run_config
                                    .amp_mode
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty())
                                {
                                    conversation.run_config_overridden_by_user = true;
                                    conversation.amp_mode = Some(mode.to_owned());
                                    overridden = true;
                                }
                                if let Some(parsed_effort) =
                                    crate::agent_settings::parse_thinking_effort(
                                        &run_config.thinking_effort,
                                    )
                                {
                                    let normalized = normalize_thinking_effort(
                                        &run_config.model_id,
                                        parsed_effort,
                                    );
                                    conversation.run_config_overridden_by_user = true;
                                    conversation.agent_model_id = run_config.model_id;
                                    conversation.thinking_effort = normalized;
                                    overridden = true;
                                }
                                if overridden
                                    && conversation.agent_runner == crate::AgentRunnerKind::Amp
                                    && conversation.amp_mode.is_none()
                                {
                                    conversation.amp_mode = Some(self.agent_amp_mode.clone());
                                }
                            }
                            conversation
                        });
                    conversation.title = meta.title;
                    conversation.thread_id = meta.remote_thread_id;
                    conversation.task_status = meta.task_status;
                }
                let mut did_update_tabs = false;
                if let Some(tabs) = self.workspace_tabs.get_mut(&workspace_id) {
                    let previous_next_thread_id = tabs.next_thread_id;
                    tabs.next_thread_id = tabs.next_thread_id.max(max_thread_id + 1);
                    if tabs.next_thread_id != previous_next_thread_id {
                        did_update_tabs = true;
                    }

                    let mut known_tabs = tabs.open_tabs.iter().copied().collect::<HashSet<_>>();
                    known_tabs.extend(tabs.archived_tabs.iter().copied());
                    loaded_thread_ids.sort_by_key(|id| id.0);
                    loaded_thread_ids.dedup();
                    let mut recovered_tabs = Vec::new();
                    for thread_id in loaded_thread_ids {
                        if known_tabs.insert(thread_id) {
                            recovered_tabs.push(thread_id);
                            did_update_tabs = true;
                        }
                    }
                    if !recovered_tabs.is_empty() {
                        recovered_tabs.sort_by_key(|id| id.0);
                        tabs.archived_tabs.splice(0..0, recovered_tabs);
                    }
                }

                if did_update_tabs {
                    vec![Effect::SaveAppState]
                } else {
                    Vec::new()
                }
            }
            Action::WorkspaceThreadsLoadFailed {
                workspace_id: _,
                message,
            } => {
                self.last_error = Some(message);
                Vec::new()
            }
            Action::ToggleTerminalPane => {
                let can_show_terminal = match self.main_pane {
                    MainPane::Workspace(workspace_id) => self.workspace(workspace_id).is_some(),
                    _ => false,
                };

                if can_show_terminal {
                    self.right_pane = match self.right_pane {
                        RightPane::Terminal => RightPane::None,
                        RightPane::None => RightPane::Terminal,
                    };
                } else {
                    self.right_pane = RightPane::None;
                }

                Vec::new()
            }
            Action::TerminalPaneWidthChanged { width } => {
                self.terminal_pane_width = Some(width);
                vec![Effect::SaveAppState]
            }
            Action::AppearanceGlobalZoomChanged { zoom } => {
                let clamped = zoom.clamp(0.7, 1.6);
                let percent = (clamped * 100.0).round() as u16;
                if self.global_zoom_percent == percent {
                    return Vec::new();
                }
                self.global_zoom_percent = percent;
                vec![Effect::SaveAppState]
            }
            Action::SidebarWidthChanged { width } => {
                self.sidebar_width = Some(width);
                vec![Effect::SaveAppState]
            }
            Action::AppearanceThemeChanged { theme } => {
                if self.appearance_theme == theme {
                    return Vec::new();
                }
                self.appearance_theme = theme;
                vec![Effect::SaveAppState]
            }
            Action::AppearanceFontsChanged {
                ui_font,
                chat_font,
                code_font,
                terminal_font,
            } => {
                let normalize = |raw: String| {
                    let trimmed = raw.trim();
                    if trimmed.is_empty() || trimmed.len() > 128 {
                        None
                    } else {
                        Some(trimmed.to_owned())
                    }
                };

                let Some(ui_font) = normalize(ui_font) else {
                    return Vec::new();
                };
                let Some(chat_font) = normalize(chat_font) else {
                    return Vec::new();
                };
                let Some(code_font) = normalize(code_font) else {
                    return Vec::new();
                };
                let Some(terminal_font) = normalize(terminal_font) else {
                    return Vec::new();
                };

                let next = crate::AppearanceFonts {
                    ui_font,
                    chat_font,
                    code_font,
                    terminal_font,
                };
                if self.appearance_fonts == next {
                    return Vec::new();
                }
                self.appearance_fonts = next;
                vec![Effect::SaveAppState]
            }
            Action::AgentCodexEnabledChanged { enabled } => {
                if self.agent_codex_enabled == enabled {
                    return Vec::new();
                }
                self.agent_codex_enabled = enabled;
                vec![Effect::SaveAppState]
            }
            Action::AgentAmpEnabledChanged { enabled } => {
                if self.agent_amp_enabled == enabled {
                    return Vec::new();
                }
                self.agent_amp_enabled = enabled;
                vec![Effect::SaveAppState]
            }
            Action::AgentClaudeEnabledChanged { enabled } => {
                if self.agent_claude_enabled == enabled {
                    return Vec::new();
                }
                self.agent_claude_enabled = enabled;
                vec![Effect::SaveAppState]
            }
            Action::AgentRunnerChanged { runner } => {
                if self.agent_default_runner == runner {
                    return Vec::new();
                }
                self.agent_default_runner = runner;
                vec![Effect::SaveAppState]
            }
            Action::AgentAmpModeChanged { mode } => {
                let next = mode.trim();
                let next = if next.is_empty() {
                    crate::default_amp_mode().to_owned()
                } else if next.len() <= 32 {
                    next.to_owned()
                } else {
                    return Vec::new();
                };

                if self.agent_amp_mode == next {
                    return Vec::new();
                }
                self.agent_amp_mode = next;
                vec![Effect::SaveAppState]
            }
            Action::TelegramBotTokenSet { token } => {
                let trimmed = token.trim();
                if trimmed.is_empty() || trimmed.len() > 256 {
                    return Vec::new();
                }
                if trimmed.chars().any(|c| c.is_control()) {
                    return Vec::new();
                }

                let next = Some(trimmed.to_owned());
                if self.telegram_bot_token == next && self.telegram_enabled {
                    return Vec::new();
                }
                self.telegram_bot_token = next;
                self.telegram_enabled = true;
                self.telegram_last_error = None;
                self.telegram_config_rev = self.telegram_config_rev.saturating_add(1);
                vec![Effect::SaveAppState]
            }
            Action::TelegramBotTokenCleared => {
                if self.telegram_bot_token.is_none()
                    && !self.telegram_enabled
                    && self.telegram_bot_username.is_none()
                    && self.telegram_paired_chat_id.is_none()
                    && self.telegram_topic_bindings.is_empty()
                    && self.telegram_last_error.is_none()
                {
                    return Vec::new();
                }
                self.telegram_enabled = false;
                self.telegram_bot_token = None;
                self.telegram_bot_username = None;
                self.telegram_paired_chat_id = None;
                self.telegram_last_error = None;
                self.telegram_topic_bindings.clear();
                self.telegram_config_rev = self.telegram_config_rev.saturating_add(1);
                vec![Effect::SaveAppState]
            }
            Action::TelegramBotUsernameSet { username } => {
                let next = username
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .filter(|v| v.len() <= 64)
                    .map(ToOwned::to_owned);
                if self.telegram_bot_username == next {
                    return Vec::new();
                }
                self.telegram_bot_username = next;
                vec![Effect::SaveAppState]
            }
            Action::TelegramChatPaired { chat_id } => {
                if self.telegram_paired_chat_id == Some(chat_id)
                    && self.telegram_topic_bindings.is_empty()
                {
                    return Vec::new();
                }
                self.telegram_paired_chat_id = Some(chat_id);
                self.telegram_last_error = None;
                self.telegram_topic_bindings.clear();
                self.telegram_config_rev = self.telegram_config_rev.saturating_add(1);
                vec![Effect::SaveAppState]
            }
            Action::TelegramUnpaired => {
                if self.telegram_paired_chat_id.is_none() && self.telegram_topic_bindings.is_empty()
                {
                    return Vec::new();
                }
                self.telegram_paired_chat_id = None;
                self.telegram_topic_bindings.clear();
                self.telegram_config_rev = self.telegram_config_rev.saturating_add(1);
                vec![Effect::SaveAppState]
            }
            Action::TelegramLastErrorSet { message } => {
                let next = message
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .filter(|v| v.len() <= 1024)
                    .map(ToOwned::to_owned);
                if self.telegram_last_error == next {
                    return Vec::new();
                }
                self.telegram_last_error = next;
                Vec::new()
            }
            Action::TelegramTopicBound {
                message_thread_id,
                workspace_id,
                thread_id,
                replayed_up_to,
            } => {
                let next = crate::TelegramTopicBinding {
                    message_thread_id,
                    workspace_id,
                    thread_id,
                    replayed_up_to,
                };
                if let Some(existing) = self.telegram_topic_bindings.get(&message_thread_id)
                    && *existing == next
                {
                    return Vec::new();
                }
                self.telegram_topic_bindings.insert(message_thread_id, next);
                vec![Effect::SaveAppState]
            }
            Action::TelegramTopicUnbound { message_thread_id } => {
                if self
                    .telegram_topic_bindings
                    .remove(&message_thread_id)
                    .is_none()
                {
                    return Vec::new();
                }
                vec![Effect::SaveAppState]
            }
            Action::TelegramTopicBindingsCleared => {
                if self.telegram_topic_bindings.is_empty() {
                    return Vec::new();
                }
                self.telegram_topic_bindings.clear();
                vec![Effect::SaveAppState]
            }
            Action::CodexDefaultsLoaded {
                model_id,
                thinking_effort,
            } => {
                if let Some(model_id) = model_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToOwned::to_owned)
                {
                    self.agent_default_model_id = model_id;
                }

                if let Some(next_effort) = thinking_effort {
                    let normalized =
                        normalize_thinking_effort(&self.agent_default_model_id, next_effort);
                    self.agent_default_thinking_effort = normalized;
                } else {
                    let normalized = normalize_thinking_effort(
                        &self.agent_default_model_id,
                        self.agent_default_thinking_effort,
                    );
                    self.agent_default_thinking_effort = normalized;
                }

                Vec::new()
            }
            Action::TaskPromptTemplateChanged {
                intent_kind,
                template,
            } => {
                let trimmed = template.trim();
                if trimmed.is_empty() {
                    return Vec::new();
                }
                let existing = self
                    .task_prompt_templates
                    .get(&intent_kind)
                    .map(|t| t.as_str());
                if existing == Some(trimmed) {
                    return Vec::new();
                }
                self.task_prompt_templates
                    .insert(intent_kind, trimmed.to_owned());
                let default = default_task_prompt_template(intent_kind);
                if trimmed == default.trim() {
                    vec![Effect::DeleteTaskPromptTemplate { intent_kind }]
                } else {
                    vec![Effect::StoreTaskPromptTemplate {
                        intent_kind,
                        template: trimmed.to_owned(),
                    }]
                }
            }
            Action::TaskPromptTemplatesLoaded { templates } => {
                let mut next = default_task_prompt_templates();
                for (kind, template) in templates {
                    let trimmed = template.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    next.insert(kind, trimmed.to_owned());
                }
                self.task_prompt_templates = next;
                Vec::new()
            }
            Action::SystemPromptTemplateChanged { kind, template } => {
                let trimmed = template.trim();
                if trimmed.is_empty() {
                    return Vec::new();
                }
                let existing = self.system_prompt_templates.get(&kind).map(|t| t.as_str());
                if existing == Some(trimmed) {
                    return Vec::new();
                }
                self.system_prompt_templates
                    .insert(kind, trimmed.to_owned());
                let default = default_system_prompt_template(kind);
                if trimmed == default.trim() {
                    vec![Effect::DeleteSystemPromptTemplate { kind }]
                } else {
                    vec![Effect::StoreSystemPromptTemplate {
                        kind,
                        template: trimmed.to_owned(),
                    }]
                }
            }
            Action::SystemPromptTemplatesLoaded { templates } => {
                let mut next = default_system_prompt_templates();
                for (kind, template) in templates {
                    let trimmed = template.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    next.insert(kind, trimmed.to_owned());
                }
                self.system_prompt_templates = next;
                Vec::new()
            }
            Action::WorkspaceChatScrollSaved {
                workspace_id,
                thread_id,
                offset_y10,
            } => {
                let key = (workspace_id, thread_id);
                if self.workspace_chat_scroll_y10.get(&key).copied() == Some(offset_y10) {
                    return Vec::new();
                }
                self.workspace_chat_scroll_y10.insert(key, offset_y10);
                vec![Effect::SaveAppState]
            }
            Action::WorkspaceChatScrollAnchorSaved {
                workspace_id,
                thread_id,
                anchor,
            } => {
                let key = (workspace_id, thread_id);
                if self
                    .workspace_chat_scroll_anchor
                    .get(&key)
                    .is_some_and(|existing| existing == &anchor)
                {
                    return Vec::new();
                }
                self.workspace_chat_scroll_anchor.insert(key, anchor);
                vec![Effect::SaveAppState]
            }
            Action::TaskStarSet {
                workspace_id,
                thread_id,
                starred,
            } => {
                let key = (workspace_id, thread_id);
                if starred {
                    if self.starred_tasks.insert(key) {
                        vec![Effect::SaveAppState]
                    } else {
                        Vec::new()
                    }
                } else if self.starred_tasks.remove(&key) {
                    vec![Effect::SaveAppState]
                } else {
                    Vec::new()
                }
            }
            Action::TaskStatusSet {
                workspace_id,
                thread_id,
                task_status,
            } => {
                let Some(conversation) = self.conversations.get_mut(&(workspace_id, thread_id))
                else {
                    return Vec::new();
                };
                if conversation.task_status == task_status {
                    return Vec::new();
                }
                let from_status = conversation.task_status;
                conversation.task_status = task_status;
                conversation.push_entry(ConversationEntry::SystemEvent {
                    entry_id: format!("sys_{}", conversation.entries_total.saturating_add(1)),
                    created_at_unix_ms: now_unix_ms(),
                    event: crate::ConversationSystemEvent::TaskStatusChanged {
                        from: from_status,
                        to: task_status,
                    },
                });
                vec![
                    Effect::StoreConversationTaskStatus {
                        workspace_id,
                        thread_id,
                        task_status,
                    },
                    Effect::LoadWorkspaceThreads { workspace_id },
                ]
            }
            Action::TaskStatusAutoUpdateSuggested {
                workspace_id,
                thread_id,
                expected_current_task_status,
                suggested_task_status,
            } => {
                let Some(conversation) = self.conversations.get_mut(&(workspace_id, thread_id))
                else {
                    return Vec::new();
                };
                if conversation.task_status != expected_current_task_status {
                    return Vec::new();
                }
                if suggested_task_status == expected_current_task_status {
                    return Vec::new();
                }
                if matches!(
                    expected_current_task_status,
                    crate::TaskStatus::Done | crate::TaskStatus::Canceled
                ) {
                    return Vec::new();
                }

                let from_status = conversation.task_status;
                conversation.task_status = suggested_task_status;
                conversation.push_entry(ConversationEntry::SystemEvent {
                    entry_id: format!("sys_{}", conversation.entries_total.saturating_add(1)),
                    created_at_unix_ms: now_unix_ms(),
                    event: crate::ConversationSystemEvent::TaskStatusChanged {
                        from: from_status,
                        to: suggested_task_status,
                    },
                });
                vec![
                    Effect::StoreConversationTaskStatus {
                        workspace_id,
                        thread_id,
                        task_status: suggested_task_status,
                    },
                    Effect::LoadWorkspaceThreads { workspace_id },
                ]
            }
            Action::SidebarProjectOrderChanged { project_ids } => {
                let mut seen = HashSet::<String>::new();
                let valid: HashSet<String> = self
                    .projects
                    .iter()
                    .map(|p| p.path.to_string_lossy().to_string())
                    .collect();

                let mut next = Vec::with_capacity(project_ids.len().min(1024));
                for raw in project_ids {
                    if next.len() >= 1024 {
                        break;
                    }
                    let trimmed = raw.trim();
                    if trimmed.is_empty() || trimmed.len() > 4096 {
                        continue;
                    }
                    if !valid.contains(trimmed) {
                        continue;
                    }
                    let key = trimmed.to_owned();
                    if !seen.insert(key.clone()) {
                        continue;
                    }
                    next.push(key);
                }

                if self.sidebar_project_order == next {
                    return Vec::new();
                }
                self.sidebar_project_order = next;
                vec![Effect::SaveAppState]
            }
            Action::OpenButtonSelectionChanged { selection } => {
                let trimmed = selection.trim();
                if trimmed.len() > 1024 {
                    return Vec::new();
                }
                let next = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                };
                if self.open_button_selection == next {
                    return Vec::new();
                }
                self.open_button_selection = next;
                vec![Effect::SaveAppState]
            }
            Action::SaveAppState => vec![Effect::SaveAppState],

            Action::AppStateLoaded { persisted } => {
                persistence::apply_persisted_app_state(self, *persisted)
            }
            Action::AppStateLoadFailed { message } => {
                self.last_error = Some(message);
                Vec::new()
            }
            Action::AppStateSaved => Vec::new(),
            Action::AppStateSaveFailed { message } => {
                self.last_error = Some(message);
                Vec::new()
            }

            Action::ClearError => {
                self.last_error = None;
                Vec::new()
            }
        }
    }

    pub fn to_persisted(&self) -> PersistedAppState {
        persistence::to_persisted_app_state(self)
    }

    pub fn project(&self, project_id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == project_id)
    }

    pub fn workspace(&self, workspace_id: WorkspaceId) -> Option<&Workspace> {
        self.projects
            .iter()
            .flat_map(|p| &p.workspaces)
            .find(|w| w.id == workspace_id)
    }

    pub fn workspace_conversation(
        &self,
        workspace_id: WorkspaceId,
    ) -> Option<&WorkspaceConversation> {
        let thread_id = self.active_thread_id(workspace_id)?;
        self.conversations.get(&(workspace_id, thread_id))
    }

    pub fn workspace_thread_conversation(
        &self,
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    ) -> Option<&WorkspaceConversation> {
        self.conversations.get(&(workspace_id, thread_id))
    }

    pub fn workspace_tabs(&self, workspace_id: WorkspaceId) -> Option<&WorkspaceTabs> {
        self.workspace_tabs.get(&workspace_id)
    }

    pub fn active_thread_id(&self, workspace_id: WorkspaceId) -> Option<WorkspaceThreadId> {
        let tabs = self.workspace_tabs.get(&workspace_id)?;
        if tabs.open_tabs.is_empty() {
            return None;
        }
        if tabs.open_tabs.contains(&tabs.active_tab) {
            Some(tabs.active_tab)
        } else {
            tabs.open_tabs.first().copied()
        }
    }

    fn ensure_workspace_tabs_mut(&mut self, workspace_id: WorkspaceId) -> &mut WorkspaceTabs {
        use std::collections::hash_map::Entry;

        match self.workspace_tabs.entry(workspace_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(WorkspaceTabs::new_empty()),
        }
    }

    fn ensure_conversation_mut(
        &mut self,
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    ) -> &mut WorkspaceConversation {
        use std::collections::hash_map::Entry;

        self.ensure_workspace_tabs_mut(workspace_id);
        let default_model_id = self.agent_default_model_id.clone();
        let default_thinking_effort = self.agent_default_thinking_effort;
        let run_config_override = self
            .workspace_thread_run_config_overrides
            .get(&(workspace_id, thread_id))
            .cloned();
        match self.conversations.entry((workspace_id, thread_id)) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let mut conversation = Self::default_conversation_with_defaults(
                    thread_id,
                    default_model_id.clone(),
                    default_thinking_effort,
                    self.agent_default_runner,
                );
                if let Some(run_config) = run_config_override {
                    let mut overridden = false;
                    if let Some(runner) = run_config
                        .runner
                        .as_deref()
                        .and_then(crate::agent_settings::parse_agent_runner_kind)
                    {
                        conversation.run_config_overridden_by_user = true;
                        conversation.agent_runner = runner;
                        overridden = true;
                    }
                    if let Some(mode) = run_config
                        .amp_mode
                        .as_deref()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                    {
                        conversation.run_config_overridden_by_user = true;
                        conversation.amp_mode = Some(mode.to_owned());
                        overridden = true;
                    }
                    if let Some(parsed_effort) =
                        crate::agent_settings::parse_thinking_effort(&run_config.thinking_effort)
                    {
                        let normalized =
                            normalize_thinking_effort(&run_config.model_id, parsed_effort);
                        conversation.run_config_overridden_by_user = true;
                        conversation.agent_model_id = run_config.model_id;
                        conversation.thinking_effort = normalized;
                        overridden = true;
                    }
                    if overridden
                        && conversation.agent_runner == crate::AgentRunnerKind::Amp
                        && conversation.amp_mode.is_none()
                    {
                        conversation.amp_mode = Some(self.agent_amp_mode.clone());
                    }
                }
                entry.insert(conversation)
            }
        }
    }

    fn default_conversation_with_defaults(
        thread_id: WorkspaceThreadId,
        model_id: String,
        thinking_effort: ThinkingEffort,
        agent_runner: crate::AgentRunnerKind,
    ) -> WorkspaceConversation {
        WorkspaceConversation {
            local_thread_id: thread_id,
            title: format!("Thread {}", thread_id.0),
            thread_id: None,
            task_status: crate::TaskStatus::Todo,
            draft: String::new(),
            draft_attachments: Vec::new(),
            run_config_overridden_by_user: false,
            agent_runner,
            agent_model_id: model_id,
            thinking_effort,
            amp_mode: None,
            entries: Vec::new(),
            entries_total: 0,
            entries_start: 0,
            active_run_id: None,
            next_run_id: 1,
            run_status: OperationStatus::Idle,
            run_started_at_unix_ms: None,
            run_finished_at_unix_ms: None,
            current_run_config: None,
            next_queued_prompt_id: 1,
            pending_prompts: VecDeque::new(),
            queue_paused: false,
        }
    }

    pub(crate) fn default_conversation(
        &self,
        thread_id: WorkspaceThreadId,
    ) -> WorkspaceConversation {
        Self::default_conversation_with_defaults(
            thread_id,
            self.agent_default_model_id.clone(),
            self.agent_default_thinking_effort,
            self.agent_default_runner,
        )
    }

    fn add_project(&mut self, path: PathBuf, is_git: bool) -> ProjectId {
        let normalized_path = crate::paths::normalize_project_path(&path);

        if let Some(project) = self
            .projects
            .iter_mut()
            .find(|p| crate::paths::normalize_project_path(&p.path) == normalized_path)
        {
            project.is_git = is_git;
            return project.id;
        }

        let id = ProjectId(self.next_project_id);
        self.next_project_id += 1;

        let name = normalized_path
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("project")
            .to_owned();

        let slug = self.unique_project_slug(sanitize_slug(&name));

        self.projects.push(Project {
            id,
            name,
            path: normalized_path,
            slug,
            is_git,
            expanded: false,
            create_workspace_status: OperationStatus::Idle,
            workspaces: Vec::new(),
        });

        id
    }

    fn upsert_project(&mut self, path: PathBuf, is_git: bool) -> (ProjectId, bool) {
        let before = self.projects.len();
        let id = self.add_project(path, is_git);
        (id, self.projects.len() != before)
    }

    fn delete_project(&mut self, project_id: ProjectId) -> Vec<Effect> {
        let Some(project_idx) = self.projects.iter().position(|p| p.id == project_id) else {
            return Vec::new();
        };

        let workspace_ids: Vec<WorkspaceId> = self.projects[project_idx]
            .workspaces
            .iter()
            .map(|w| w.id)
            .collect();

        self.projects.remove(project_idx);

        for workspace_id in &workspace_ids {
            self.workspace_tabs.remove(workspace_id);
            self.workspace_unread_completions.remove(workspace_id);
            self.workspace_chat_scroll_y10
                .retain(|(wid, _), _| wid != workspace_id);
            self.workspace_chat_scroll_anchor
                .retain(|(wid, _), _| wid != workspace_id);
            self.workspace_thread_run_config_overrides
                .retain(|(wid, _), _| wid != workspace_id);
            self.conversations.retain(|(wid, _), _| wid != workspace_id);
        }

        if let Some(workspace_id) = self.last_open_workspace_id
            && workspace_ids.contains(&workspace_id)
        {
            self.last_open_workspace_id = None;
        }

        if let MainPane::Workspace(workspace_id) = self.main_pane
            && workspace_ids.contains(&workspace_id)
        {
            self.main_pane = MainPane::Dashboard;
            self.right_pane = RightPane::None;
        }

        if let Some(workspace_id) = self.dashboard_preview_workspace_id
            && workspace_ids.contains(&workspace_id)
        {
            self.dashboard_preview_workspace_id = None;
        }

        if matches!(self.main_pane, MainPane::ProjectSettings(id) if id == project_id) {
            self.main_pane = MainPane::Dashboard;
            self.right_pane = RightPane::None;
        }

        vec![Effect::SaveAppState]
    }

    fn insert_main_workspace(&mut self, project_id: ProjectId) -> WorkspaceId {
        let workspace_id = WorkspaceId(self.next_workspace_id);
        self.next_workspace_id += 1;

        let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) else {
            return workspace_id;
        };

        project.workspaces.push(Workspace {
            id: workspace_id,
            workspace_name: Self::MAIN_WORKSPACE_NAME.to_owned(),
            branch_name: Self::MAIN_WORKSPACE_BRANCH.to_owned(),
            worktree_path: project.path.clone(),
            status: WorkspaceStatus::Active,
            last_activity_at: None,
            archive_status: OperationStatus::Idle,
            branch_rename_status: OperationStatus::Idle,
        });

        workspace_id
    }

    fn workspace_is_main(project: &Project, workspace: &Workspace) -> bool {
        workspace.workspace_name == Self::MAIN_WORKSPACE_NAME
            && workspace.worktree_path == project.path
    }

    fn insert_workspace(
        &mut self,
        project_id: ProjectId,
        workspace_name: &str,
        branch_name: &str,
        worktree_path: PathBuf,
    ) -> WorkspaceId {
        let workspace_id = WorkspaceId(self.next_workspace_id);
        self.next_workspace_id += 1;

        if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
            project.workspaces.push(Workspace {
                id: workspace_id,
                workspace_name: workspace_name.to_owned(),
                branch_name: branch_name.to_owned(),
                worktree_path,
                status: WorkspaceStatus::Active,
                last_activity_at: None,
                archive_status: OperationStatus::Idle,
                branch_rename_status: OperationStatus::Idle,
            });
            project.expanded = true;
            self.main_pane = MainPane::Workspace(workspace_id);
        }

        workspace_id
    }

    fn find_workspace_indices(&self, workspace_id: WorkspaceId) -> Option<(usize, usize)> {
        for (project_idx, project) in self.projects.iter().enumerate() {
            if let Some(workspace_idx) = project
                .workspaces
                .iter()
                .position(|w| w.id == workspace_id && w.status == WorkspaceStatus::Active)
            {
                return Some((project_idx, workspace_idx));
            }
        }
        None
    }

    fn unique_project_slug(&self, base: String) -> String {
        if !self.projects.iter().any(|p| p.slug == base) {
            return base;
        }

        for i in 2.. {
            let candidate = format!("{base}-{i}");
            if !self.projects.iter().any(|p| p.slug == candidate) {
                return candidate;
            }
        }

        unreachable!("infinite iterator");
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

fn start_next_queued_prompt(
    conversation: &mut WorkspaceConversation,
    workspace_id: WorkspaceId,
    thread_id: WorkspaceThreadId,
) -> Option<Effect> {
    if conversation.queue_paused || conversation.run_status != OperationStatus::Idle {
        return None;
    }

    let queued = conversation.pending_prompts.pop_front()?;
    Some(start_agent_run(
        conversation,
        workspace_id,
        thread_id,
        queued.text,
        queued.attachments,
        queued.run_config,
    ))
}

fn start_agent_run(
    conversation: &mut WorkspaceConversation,
    workspace_id: WorkspaceId,
    thread_id: WorkspaceThreadId,
    text: String,
    attachments: Vec<AttachmentRef>,
    run_config: AgentRunConfig,
) -> Effect {
    let run_id = conversation.next_run_id;
    conversation.next_run_id = conversation.next_run_id.saturating_add(1);
    conversation.active_run_id = Some(run_id);

    conversation.push_entry(ConversationEntry::UserEvent {
        entry_id: String::new(),
        event: crate::UserEvent::Message {
            text: text.clone(),
            attachments: attachments.clone(),
        },
    });
    conversation.run_status = OperationStatus::Running;
    conversation.run_started_at_unix_ms = None;
    conversation.run_finished_at_unix_ms = None;
    conversation.current_run_config = Some(run_config.clone());

    Effect::RunAgentTurn {
        workspace_id,
        thread_id,
        run_id,
        text,
        attachments,
        run_config,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ChatScrollAnchor, CodexCommandExecutionStatus, CodexThreadError, CodexThreadItem,
        CodexUsage, ContextTokenKind, ConversationSnapshot, ConversationThreadMeta,
    };

    fn codex_item_id(item: &CodexThreadItem) -> &str {
        match item {
            CodexThreadItem::AgentMessage { id, .. } => id,
            CodexThreadItem::Reasoning { id, .. } => id,
            CodexThreadItem::CommandExecution { id, .. } => id,
            CodexThreadItem::FileChange { id, .. } => id,
            CodexThreadItem::McpToolCall { id, .. } => id,
            CodexThreadItem::WebSearch { id, .. } => id,
            CodexThreadItem::TodoList { id, .. } => id,
            CodexThreadItem::Error { id, .. } => id,
        }
    }

    fn default_thread_id() -> WorkspaceThreadId {
        WorkspaceThreadId(1)
    }

    fn main_workspace_id(state: &AppState) -> WorkspaceId {
        let project = &state.projects[0];
        project
            .workspaces
            .iter()
            .find(|w| {
                w.status == WorkspaceStatus::Active
                    && w.workspace_name == "main"
                    && w.worktree_path == project.path
            })
            .expect("missing main workspace")
            .id
    }

    fn workspace_id_by_name(state: &AppState, name: &str) -> WorkspaceId {
        state.projects[0]
            .workspaces
            .iter()
            .find(|w| w.status == WorkspaceStatus::Active && w.workspace_name == name)
            .unwrap_or_else(|| panic!("missing workspace {name}"))
            .id
    }

    fn first_non_main_workspace_id(state: &AppState) -> WorkspaceId {
        let project = &state.projects[0];
        project
            .workspaces
            .iter()
            .find(|w| {
                w.status == WorkspaceStatus::Active
                    && !(w.workspace_name == "main" && w.worktree_path == project.path)
            })
            .expect("missing non-main workspace")
            .id
    }

    #[test]
    fn new_threads_use_codex_defaults() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });

        let workspace_id = workspace_id_by_name(&state, "w1");
        state.apply(Action::CreateWorkspaceThread { workspace_id });
        let thread_id = WorkspaceThreadId(1);

        state.apply(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: "gpt-5.2".to_owned(),
        });
        state.apply(Action::ThinkingEffortChanged {
            workspace_id,
            thread_id,
            thinking_effort: ThinkingEffort::Low,
        });

        state.apply(Action::CodexDefaultsLoaded {
            model_id: Some("gpt-5.2-codex".to_owned()),
            thinking_effort: Some(ThinkingEffort::High),
        });

        state.apply(Action::CreateWorkspaceThread { workspace_id });
        let created_thread_id = state
            .workspace_tabs(workspace_id)
            .expect("missing workspace tabs")
            .active_tab;
        assert_eq!(created_thread_id, WorkspaceThreadId(2));

        let conversation = state
            .workspace_thread_conversation(workspace_id, created_thread_id)
            .expect("missing conversation");
        assert_eq!(conversation.agent_model_id, "gpt-5.2-codex");
        assert_eq!(conversation.thinking_effort, ThinkingEffort::High);
    }

    #[test]
    fn workspace_threads_loaded_restores_missing_tabs() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });

        let workspace_id = workspace_id_by_name(&state, "w1");
        let effects = state.apply(Action::WorkspaceThreadsLoaded {
            workspace_id,
            threads: vec![
                ConversationThreadMeta {
                    thread_id: WorkspaceThreadId(3),
                    remote_thread_id: Some("remote-3".to_owned()),
                    title: "Thread 3".to_owned(),
                    created_at_unix_seconds: 300,
                    updated_at_unix_seconds: 300,
                    task_status: crate::TaskStatus::Todo,
                    last_message_seq: 0,
                    task_status_last_analyzed_message_seq: 0,
                    turn_status: crate::TurnStatus::Idle,
                    last_turn_result: None,
                },
                ConversationThreadMeta {
                    thread_id: WorkspaceThreadId(2),
                    remote_thread_id: Some("remote-2".to_owned()),
                    title: "Thread 2".to_owned(),
                    created_at_unix_seconds: 200,
                    updated_at_unix_seconds: 200,
                    task_status: crate::TaskStatus::Todo,
                    last_message_seq: 0,
                    task_status_last_analyzed_message_seq: 0,
                    turn_status: crate::TurnStatus::Idle,
                    last_turn_result: None,
                },
                ConversationThreadMeta {
                    thread_id: WorkspaceThreadId(1),
                    remote_thread_id: Some("remote-1".to_owned()),
                    title: "Thread 1".to_owned(),
                    created_at_unix_seconds: 100,
                    updated_at_unix_seconds: 100,
                    task_status: crate::TaskStatus::Todo,
                    last_message_seq: 0,
                    task_status_last_analyzed_message_seq: 0,
                    turn_status: crate::TurnStatus::Idle,
                    last_turn_result: None,
                },
            ],
        });

        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::SaveAppState))
        );

        let tabs = state
            .workspace_tabs(workspace_id)
            .expect("missing workspace tabs");
        assert_eq!(tabs.open_tabs, Vec::<WorkspaceThreadId>::new());
        assert_eq!(
            tabs.archived_tabs,
            vec![
                WorkspaceThreadId(1),
                WorkspaceThreadId(2),
                WorkspaceThreadId(3),
            ]
        );
        assert_eq!(tabs.next_thread_id, 4);

        for thread_id in [
            WorkspaceThreadId(1),
            WorkspaceThreadId(2),
            WorkspaceThreadId(3),
        ] {
            let conversation = state
                .workspace_thread_conversation(workspace_id, thread_id)
                .expect("missing conversation");
            assert_eq!(
                conversation.thread_id,
                Some(format!("remote-{}", thread_id.0))
            );
        }
    }

    #[test]
    fn running_turn_keeps_its_run_config_when_user_changes_defaults() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        let workspace_id = workspace_id_by_name(&state, "w1");
        state.apply(Action::CreateWorkspaceThread { workspace_id });
        let thread_id = WorkspaceThreadId(1);

        state.apply(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: "gpt-5.2-codex".to_owned(),
        });
        state.apply(Action::ThinkingEffortChanged {
            workspace_id,
            thread_id,
            thinking_effort: ThinkingEffort::High,
        });

        let effects = state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "hi".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let (sent_model_id, sent_effort) = effects
            .iter()
            .find_map(|effect| match effect {
                Effect::RunAgentTurn { run_config, .. } => {
                    Some((run_config.model_id.as_str(), run_config.thinking_effort))
                }
                _ => None,
            })
            .expect("missing RunAgentTurn effect");
        assert_eq!(sent_model_id, "gpt-5.2-codex");
        assert_eq!(sent_effort, ThinkingEffort::High);

        state.apply(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: "gpt-5.2".to_owned(),
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert_eq!(conversation.agent_model_id, "gpt-5.2");
        let running = conversation
            .current_run_config
            .as_ref()
            .expect("missing current run config");
        assert_eq!(running.model_id, "gpt-5.2-codex");
        assert_eq!(running.thinking_effort, ThinkingEffort::High);
    }

    #[test]
    fn auto_title_thread_ignores_system_events_on_first_user_message() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });

        let workspace_id = workspace_id_by_name(&state, "w1");
        let thread_id = WorkspaceThreadId(1);

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                title: Some("Thread 1".to_owned()),
                thread_id: None,
                task_status: crate::TaskStatus::Todo,
                runner: None,
                agent_model_id: None,
                thinking_effort: None,
                amp_mode: None,
                entries: vec![ConversationEntry::SystemEvent {
                    entry_id: "sys_1".to_owned(),
                    created_at_unix_ms: 1,
                    event: crate::ConversationSystemEvent::TaskCreated,
                }],
                entries_total: 1,
                entries_start: 0,
                pending_prompts: Vec::new(),
                queue_paused: false,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
            },
        });

        let text = "Fix title auto summary".to_owned();
        let expected_title = derive_thread_title(&text);
        assert!(
            !expected_title.is_empty(),
            "expected derive_thread_title to produce a non-empty title"
        );

        let effects = state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: text.clone(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert_eq!(conversation.title, expected_title);

        let (input, expected_current_title) = effects
            .iter()
            .find_map(|effect| match effect {
                Effect::AiAutoTitleThread {
                    input,
                    expected_current_title,
                    ..
                } => Some((input.as_str(), expected_current_title.as_str())),
                _ => None,
            })
            .expect("missing AiAutoTitleThread effect");
        assert_eq!(input, text.as_str());
        assert_eq!(expected_current_title, expected_title.as_str());
    }

    #[test]
    fn queue_agent_message_derives_title_with_only_system_events() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });

        let workspace_id = workspace_id_by_name(&state, "w1");
        let thread_id = WorkspaceThreadId(1);

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                title: Some("Thread 1".to_owned()),
                thread_id: None,
                task_status: crate::TaskStatus::Todo,
                runner: None,
                agent_model_id: None,
                thinking_effort: None,
                amp_mode: None,
                entries: vec![ConversationEntry::SystemEvent {
                    entry_id: "sys_1".to_owned(),
                    created_at_unix_ms: 1,
                    event: crate::ConversationSystemEvent::TaskCreated,
                }],
                entries_total: 1,
                entries_start: 0,
                pending_prompts: Vec::new(),
                queue_paused: false,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
            },
        });

        let text = "Fix title auto summary".to_owned();
        let expected_title = derive_thread_title(&text);
        assert!(
            !expected_title.is_empty(),
            "expected derive_thread_title to produce a non-empty title"
        );

        state.apply(Action::QueueAgentMessage {
            workspace_id,
            thread_id,
            text: text.clone(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert_eq!(conversation.title, expected_title);
    }

    #[test]
    fn conversation_loaded_does_not_override_user_run_config() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        state.apply(Action::CodexDefaultsLoaded {
            model_id: Some("gpt-5.2-codex".to_owned()),
            thinking_effort: Some(ThinkingEffort::High),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        let workspace_id = workspace_id_by_name(&state, "w1");
        state.apply(Action::CreateWorkspaceThread { workspace_id });
        let thread_id = default_thread_id();

        state.apply(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: "gpt-5".to_owned(),
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert!(conversation.run_config_overridden_by_user);
        assert_eq!(conversation.agent_model_id, "gpt-5");

        let snapshot = ConversationSnapshot {
            title: None,
            thread_id: None,
            task_status: crate::TaskStatus::Todo,
            runner: None,
            agent_model_id: Some("gpt-5.2-codex".to_owned()),
            thinking_effort: Some(ThinkingEffort::High),
            amp_mode: None,
            entries: Vec::new(),
            entries_total: 0,
            entries_start: 0,
            pending_prompts: Vec::new(),
            queue_paused: false,
            run_started_at_unix_ms: None,
            run_finished_at_unix_ms: None,
        };

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot,
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert_eq!(conversation.agent_model_id, "gpt-5");
    }

    #[test]
    fn thread_run_config_override_is_persisted_in_app_state() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        state.apply(Action::CodexDefaultsLoaded {
            model_id: Some("gpt-5.2-codex".to_owned()),
            thinking_effort: Some(ThinkingEffort::High),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        let workspace_id = workspace_id_by_name(&state, "w1");
        state.apply(Action::CreateWorkspaceThread { workspace_id });
        let thread_id = default_thread_id();

        state.apply(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: "gpt-5".to_owned(),
        });

        let persisted = state.to_persisted();
        let saved = persisted
            .workspace_thread_run_config_overrides
            .get(&(workspace_id.as_u64(), thread_id.as_u64()))
            .expect("missing persisted run config override");
        assert_eq!(saved.model_id, "gpt-5");

        let mut restored = AppState::new();
        restored.apply(Action::AppStateLoaded {
            persisted: Box::new(persisted),
        });

        let conversation = restored
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert!(conversation.run_config_overridden_by_user);
        assert_eq!(conversation.agent_model_id, "gpt-5");
    }

    #[test]
    fn queued_turn_updates_current_run_config_when_started() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        let workspace_id = workspace_id_by_name(&state, "w1");
        state.apply(Action::CreateWorkspaceThread { workspace_id });
        let thread_id = WorkspaceThreadId(1);

        state.apply(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: "gpt-5.2-codex".to_owned(),
        });
        state.apply(Action::ThinkingEffortChanged {
            workspace_id,
            thread_id,
            thinking_effort: ThinkingEffort::High,
        });
        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "first".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        state.apply(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: "gpt-5.2".to_owned(),
        });
        state.apply(Action::ThinkingEffortChanged {
            workspace_id,
            thread_id,
            thinking_effort: ThinkingEffort::Low,
        });
        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "second".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert_eq!(conversation.run_status, OperationStatus::Running);
        assert_eq!(conversation.pending_prompts.len(), 1);
        let run_id = conversation.active_run_id.expect("missing active run id");

        let effects = state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::TurnCompleted {
                usage: CodexUsage {
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                },
            },
        });
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::RunAgentTurn { run_config, .. } => {
                assert_eq!(run_config.model_id, "gpt-5.2");
                assert_eq!(run_config.thinking_effort, ThinkingEffort::Low);
            }
            other => panic!("unexpected effect: {other:?}"),
        }

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        let running = conversation
            .current_run_config
            .as_ref()
            .expect("missing current run config");
        assert_eq!(running.model_id, "gpt-5.2");
        assert_eq!(running.thinking_effort, ThinkingEffort::Low);
    }

    #[test]
    fn first_message_does_not_trigger_ai_branch_rename() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "luban/random-name".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });

        let workspace_id = workspace_id_by_name(&state, "w1");
        let thread_id = default_thread_id();
        let effects = state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Implement feature X".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        assert!(
            effects.iter().any(|e| matches!(e, Effect::RunAgentTurn { workspace_id: wid, .. } if *wid == workspace_id)),
            "missing RunAgentTurn effect"
        );
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::AiRenameWorkspaceBranch { .. })),
            "unexpected AiRenameWorkspaceBranch effect"
        );
    }

    #[test]
    fn turn_completed_triggers_ai_task_status_auto_update() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "luban/random-name".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });

        let workspace_id = workspace_id_by_name(&state, "w1");
        let thread_id = default_thread_id();
        let effects = state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Implement feature X".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        let (run_id, run_config) = effects
            .iter()
            .find_map(|effect| match effect {
                Effect::RunAgentTurn {
                    run_id, run_config, ..
                } => Some((*run_id, run_config.clone())),
                _ => None,
            })
            .expect("missing RunAgentTurn effect");

        let completed_effects = state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::TurnCompleted {
                usage: CodexUsage {
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                },
            },
        });

        let auto_update = completed_effects
            .iter()
            .find_map(|effect| match effect {
                Effect::AiAutoUpdateTaskStatus {
                    input,
                    expected_current_task_status,
                    runner,
                    model_id,
                    thinking_effort,
                    amp_mode,
                    ..
                } => Some((
                    input.as_str(),
                    *expected_current_task_status,
                    *runner,
                    model_id.as_str(),
                    *thinking_effort,
                    amp_mode.as_deref(),
                )),
                _ => None,
            })
            .expect("missing AiAutoUpdateTaskStatus effect");

        assert_eq!(auto_update.1, crate::TaskStatus::Iterating);
        assert_eq!(auto_update.2, run_config.runner);
        assert_eq!(auto_update.3, run_config.model_id.as_str());
        assert_eq!(auto_update.4, run_config.thinking_effort);
        assert_eq!(auto_update.5, run_config.amp_mode.as_deref());
        assert!(
            auto_update.0.contains("Current task_status: iterating"),
            "{}",
            auto_update.0
        );
    }

    #[test]
    fn manual_ai_branch_rename_uses_first_user_messages_as_input() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "luban/feature-x".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        let workspace_id = workspace_id_by_name(&state, "w1");
        let thread_id = default_thread_id();

        let snapshot = ConversationSnapshot {
            title: None,
            thread_id: None,
            task_status: crate::TaskStatus::Todo,
            runner: None,
            agent_model_id: None,
            thinking_effort: None,
            amp_mode: None,
            entries: (1..=8)
                .map(|idx| ConversationEntry::UserEvent {
                    entry_id: String::new(),
                    event: crate::UserEvent::Message {
                        text: format!("Message {idx}"),
                        attachments: Vec::new(),
                    },
                })
                .collect(),
            entries_total: 0,
            entries_start: 0,
            pending_prompts: Vec::new(),
            queue_paused: false,
            run_started_at_unix_ms: None,
            run_finished_at_unix_ms: None,
        };
        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot,
        });

        let effects = state.apply(Action::WorkspaceBranchAiRenameRequested {
            workspace_id,
            thread_id,
        });

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::AiRenameWorkspaceBranch { input, .. } => {
                assert!(input.contains("Message 1"), "{input}");
                assert!(input.contains("Message 6"), "{input}");
                assert!(!input.contains("Message 7"), "{input}");
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn workspace_branch_synced_updates_and_persists_when_changed() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });

        let workspace_id = workspace_id_by_name(&state, "w1");

        let effects = state.apply(Action::WorkspaceBranchSynced {
            workspace_id,
            branch_name: "repo/renamed".to_owned(),
        });
        assert_eq!(
            state.workspace(workspace_id).unwrap().branch_name,
            "repo/renamed"
        );
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        let effects = state.apply(Action::WorkspaceBranchSynced {
            workspace_id,
            branch_name: "repo/renamed".to_owned(),
        });
        assert!(effects.is_empty());
    }

    #[test]
    fn open_dashboard_loads_conversations_for_non_main_workspaces() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });

        let main_id = main_workspace_id(&state);
        let w1 = workspace_id_by_name(&state, "w1");
        state.apply(Action::CreateWorkspaceThread { workspace_id: w1 });

        let effects = state.apply(Action::OpenDashboard);
        assert_eq!(state.main_pane, MainPane::Dashboard);
        assert_eq!(state.right_pane, RightPane::None);
        assert_eq!(state.dashboard_preview_workspace_id, None);

        assert!(
            effects.iter().any(
                |e| matches!(e, Effect::LoadConversation { workspace_id, .. } if *workspace_id == w1)
            ),
            "expected dashboard to load non-main workspace conversation"
        );
        assert!(
            !effects.iter()
                .any(|e| matches!(e, Effect::LoadConversation { workspace_id, .. } if *workspace_id == main_id)),
            "dashboard should not load main workspace conversation"
        );
    }

    #[test]
    fn right_pane_tracks_selected_main_pane() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        let workspace_id = workspace_id_by_name(&state, "w1");

        state.apply(Action::OpenWorkspace { workspace_id });
        assert_eq!(state.right_pane, RightPane::Terminal);

        state.apply(Action::OpenProjectSettings { project_id });
        assert_eq!(state.right_pane, RightPane::None);
    }

    #[test]
    fn toggle_terminal_pane_hides_and_shows_when_workspace_open() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        let workspace_id = workspace_id_by_name(&state, "w1");
        state.apply(Action::OpenWorkspace { workspace_id });

        assert_eq!(state.right_pane, RightPane::Terminal);

        state.apply(Action::ToggleTerminalPane);
        assert_eq!(state.right_pane, RightPane::None);

        state.apply(Action::ToggleTerminalPane);
        assert_eq!(state.right_pane, RightPane::Terminal);
    }

    #[test]
    fn toggle_terminal_pane_is_disabled_outside_workspace() {
        let mut state = AppState::new();
        state.apply(Action::ToggleTerminalPane);
        assert_eq!(state.right_pane, RightPane::None);

        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::OpenProjectSettings { project_id });
        state.apply(Action::ToggleTerminalPane);
        assert_eq!(state.right_pane, RightPane::None);
    }

    #[test]
    fn terminal_pane_width_is_persisted() {
        let mut state = AppState::new();
        let effects = state.apply(Action::TerminalPaneWidthChanged { width: 360 });
        assert_eq!(state.terminal_pane_width, Some(360));
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        let persisted = state.to_persisted();
        assert_eq!(persisted.sidebar_width, None);
        assert_eq!(persisted.terminal_pane_width, Some(360));

        let mut state = AppState::new();
        state.apply(Action::AppStateLoaded {
            persisted: Box::new(PersistedAppState {
                projects: Vec::new(),
                sidebar_width: None,
                terminal_pane_width: Some(480),
                global_zoom_percent: None,
                appearance_theme: None,
                appearance_ui_font: None,
                appearance_chat_font: None,
                appearance_code_font: None,
                appearance_terminal_font: None,
                agent_default_model_id: None,
                agent_default_thinking_effort: None,
                agent_default_runner: None,
                agent_amp_mode: None,
                agent_codex_enabled: Some(true),
                agent_amp_enabled: Some(true),
                agent_claude_enabled: Some(true),
                last_open_workspace_id: None,
                open_button_selection: None,
                sidebar_project_order: Vec::new(),
                workspace_active_thread_id: HashMap::new(),
                workspace_open_tabs: HashMap::new(),
                workspace_archived_tabs: HashMap::new(),
                workspace_next_thread_id: HashMap::new(),
                workspace_chat_scroll_y10: HashMap::new(),
                workspace_chat_scroll_anchor: HashMap::new(),
                workspace_unread_completions: HashMap::new(),
                workspace_thread_run_config_overrides: HashMap::new(),
                starred_tasks: HashMap::new(),
                task_prompt_templates: HashMap::new(),
                telegram_enabled: None,
                telegram_bot_token: None,
                telegram_bot_username: None,
                telegram_paired_chat_id: None,
                telegram_topic_bindings: None,
            }),
        });
        assert_eq!(state.terminal_pane_width, Some(480));
    }

    #[test]
    fn global_zoom_is_persisted() {
        let mut state = AppState::new();
        let effects = state.apply(Action::AppearanceGlobalZoomChanged { zoom: 1.2 });
        assert_eq!(state.global_zoom_percent, 120);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        let persisted = state.to_persisted();
        assert_eq!(persisted.global_zoom_percent, Some(120));

        let mut restored = AppState::new();
        restored.apply(Action::AppStateLoaded {
            persisted: Box::new(PersistedAppState {
                projects: Vec::new(),
                sidebar_width: None,
                terminal_pane_width: None,
                global_zoom_percent: Some(135),
                appearance_theme: None,
                appearance_ui_font: None,
                appearance_chat_font: None,
                appearance_code_font: None,
                appearance_terminal_font: None,
                agent_default_model_id: None,
                agent_default_thinking_effort: None,
                agent_default_runner: None,
                agent_amp_mode: None,
                agent_codex_enabled: Some(true),
                agent_amp_enabled: Some(true),
                agent_claude_enabled: Some(true),
                last_open_workspace_id: None,
                open_button_selection: None,
                sidebar_project_order: Vec::new(),
                workspace_active_thread_id: HashMap::new(),
                workspace_open_tabs: HashMap::new(),
                workspace_archived_tabs: HashMap::new(),
                workspace_next_thread_id: HashMap::new(),
                workspace_chat_scroll_y10: HashMap::new(),
                workspace_chat_scroll_anchor: HashMap::new(),
                workspace_unread_completions: HashMap::new(),
                workspace_thread_run_config_overrides: HashMap::new(),
                starred_tasks: HashMap::new(),
                task_prompt_templates: HashMap::new(),
                telegram_enabled: None,
                telegram_bot_token: None,
                telegram_bot_username: None,
                telegram_paired_chat_id: None,
                telegram_topic_bindings: None,
            }),
        });
        assert_eq!(restored.global_zoom_percent, 135);
    }

    #[test]
    fn sidebar_width_is_persisted() {
        let mut state = AppState::new();
        let effects = state.apply(Action::SidebarWidthChanged { width: 280 });
        assert_eq!(state.sidebar_width, Some(280));
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        let persisted = state.to_persisted();
        assert_eq!(persisted.sidebar_width, Some(280));

        let mut state = AppState::new();
        state.apply(Action::AppStateLoaded {
            persisted: Box::new(PersistedAppState {
                projects: Vec::new(),
                sidebar_width: Some(360),
                terminal_pane_width: None,
                global_zoom_percent: None,
                appearance_theme: None,
                appearance_ui_font: None,
                appearance_chat_font: None,
                appearance_code_font: None,
                appearance_terminal_font: None,
                agent_default_model_id: None,
                agent_default_thinking_effort: None,
                agent_default_runner: None,
                agent_amp_mode: None,
                agent_codex_enabled: Some(true),
                agent_amp_enabled: Some(true),
                agent_claude_enabled: Some(true),
                last_open_workspace_id: None,
                open_button_selection: None,
                sidebar_project_order: Vec::new(),
                workspace_active_thread_id: HashMap::new(),
                workspace_open_tabs: HashMap::new(),
                workspace_archived_tabs: HashMap::new(),
                workspace_next_thread_id: HashMap::new(),
                workspace_chat_scroll_y10: HashMap::new(),
                workspace_chat_scroll_anchor: HashMap::new(),
                workspace_unread_completions: HashMap::new(),
                workspace_thread_run_config_overrides: HashMap::new(),
                starred_tasks: HashMap::new(),
                task_prompt_templates: HashMap::new(),
                telegram_enabled: None,
                telegram_bot_token: None,
                telegram_bot_username: None,
                telegram_paired_chat_id: None,
                telegram_topic_bindings: None,
            }),
        });
        assert_eq!(state.sidebar_width, Some(360));
    }

    #[test]
    fn sidebar_project_order_is_persisted() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/sidebar-order-a"),
            is_git: true,
        });
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/sidebar-order-b"),
            is_git: true,
        });

        let project_a = state.projects[0].path.to_string_lossy().to_string();
        let project_b = state.projects[1].path.to_string_lossy().to_string();

        let effects = state.apply(Action::SidebarProjectOrderChanged {
            project_ids: vec![project_b.clone(), project_a.clone()],
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));
        assert_eq!(
            state.sidebar_project_order,
            vec![project_b.clone(), project_a.clone()]
        );

        let persisted = state.to_persisted();
        assert_eq!(
            persisted.sidebar_project_order,
            vec![project_b.clone(), project_a.clone()]
        );

        let mut restored = AppState::new();
        restored.apply(Action::AppStateLoaded {
            persisted: Box::new(persisted),
        });
        assert_eq!(
            restored.sidebar_project_order,
            vec![project_b.clone(), project_a.clone()]
        );
    }

    #[test]
    fn appearance_theme_is_persisted() {
        let mut state = AppState::new();
        let effects = state.apply(Action::AppearanceThemeChanged {
            theme: crate::AppearanceTheme::Dark,
        });
        assert_eq!(state.appearance_theme, crate::AppearanceTheme::Dark);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        let persisted = state.to_persisted();
        assert_eq!(persisted.appearance_theme.as_deref(), Some("dark"));

        let mut restored = AppState::new();
        restored.apply(Action::AppStateLoaded {
            persisted: Box::new(PersistedAppState {
                projects: Vec::new(),
                sidebar_width: None,
                terminal_pane_width: None,
                global_zoom_percent: None,
                appearance_theme: Some("light".to_owned()),
                appearance_ui_font: None,
                appearance_chat_font: None,
                appearance_code_font: None,
                appearance_terminal_font: None,
                agent_default_model_id: None,
                agent_default_thinking_effort: None,
                agent_default_runner: None,
                agent_amp_mode: None,
                agent_codex_enabled: Some(true),
                agent_amp_enabled: Some(true),
                agent_claude_enabled: Some(true),
                last_open_workspace_id: None,
                open_button_selection: None,
                sidebar_project_order: Vec::new(),
                workspace_active_thread_id: HashMap::new(),
                workspace_open_tabs: HashMap::new(),
                workspace_archived_tabs: HashMap::new(),
                workspace_next_thread_id: HashMap::new(),
                workspace_chat_scroll_y10: HashMap::new(),
                workspace_chat_scroll_anchor: HashMap::new(),
                workspace_unread_completions: HashMap::new(),
                workspace_thread_run_config_overrides: HashMap::new(),
                starred_tasks: HashMap::new(),
                task_prompt_templates: HashMap::new(),
                telegram_enabled: None,
                telegram_bot_token: None,
                telegram_bot_username: None,
                telegram_paired_chat_id: None,
                telegram_topic_bindings: None,
            }),
        });
        assert_eq!(restored.appearance_theme, crate::AppearanceTheme::Light);
    }

    #[test]
    fn appearance_fonts_are_persisted() {
        let mut state = AppState::new();
        let effects = state.apply(Action::AppearanceFontsChanged {
            ui_font: "Inter".to_owned(),
            chat_font: "Roboto".to_owned(),
            code_font: "Geist Mono".to_owned(),
            terminal_font: "JetBrains Mono".to_owned(),
        });
        assert_eq!(
            state.appearance_fonts,
            crate::AppearanceFonts {
                ui_font: "Inter".to_owned(),
                chat_font: "Roboto".to_owned(),
                code_font: "Geist Mono".to_owned(),
                terminal_font: "JetBrains Mono".to_owned(),
            }
        );
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        let persisted = state.to_persisted();
        assert_eq!(persisted.appearance_ui_font.as_deref(), Some("Inter"));
        assert_eq!(persisted.appearance_chat_font.as_deref(), Some("Roboto"));
        assert_eq!(
            persisted.appearance_code_font.as_deref(),
            Some("Geist Mono")
        );
        assert_eq!(
            persisted.appearance_terminal_font.as_deref(),
            Some("JetBrains Mono")
        );
    }

    #[test]
    fn workspace_chat_scroll_is_persisted() {
        let mut state = AppState::new();
        let workspace_id = WorkspaceId(42);
        let thread_id = default_thread_id();

        let effects = state.apply(Action::WorkspaceChatScrollSaved {
            workspace_id,
            thread_id,
            offset_y10: -1234,
        });
        assert_eq!(
            state
                .workspace_chat_scroll_y10
                .get(&(workspace_id, thread_id))
                .copied(),
            Some(-1234)
        );
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        let persisted = state.to_persisted();
        assert_eq!(
            persisted.workspace_chat_scroll_y10.get(&(42, 1)).copied(),
            Some(-1234)
        );

        let mut loaded = AppState::new();
        loaded.apply(Action::AppStateLoaded {
            persisted: Box::new(persisted),
        });
        assert_eq!(
            loaded
                .workspace_chat_scroll_y10
                .get(&(workspace_id, thread_id))
                .copied(),
            Some(-1234)
        );
    }

    #[test]
    fn workspace_chat_scroll_anchor_is_persisted() {
        let mut state = AppState::new();
        let workspace_id = WorkspaceId(42);
        let thread_id = default_thread_id();

        let anchor = ChatScrollAnchor::Block {
            block_id: "history-block-agent-turn-3".to_owned(),
            block_index: 3,
            offset_in_block_y10: 420,
        };

        let effects = state.apply(Action::WorkspaceChatScrollAnchorSaved {
            workspace_id,
            thread_id,
            anchor: anchor.clone(),
        });
        assert_eq!(
            state
                .workspace_chat_scroll_anchor
                .get(&(workspace_id, thread_id))
                .cloned(),
            Some(anchor.clone())
        );
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        let persisted = state.to_persisted();
        assert_eq!(
            persisted
                .workspace_chat_scroll_anchor
                .get(&(42, 1))
                .cloned(),
            Some(anchor.clone())
        );

        let mut loaded = AppState::new();
        loaded.apply(Action::AppStateLoaded {
            persisted: Box::new(persisted),
        });
        assert_eq!(
            loaded
                .workspace_chat_scroll_anchor
                .get(&(workspace_id, thread_id))
                .cloned(),
            Some(anchor)
        );
    }

    #[test]
    fn workspace_thread_tabs_preserve_order_and_allow_reorder() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });
        let workspace_id = workspace_id_by_name(&state, "w1");
        state.apply(Action::OpenWorkspace { workspace_id });

        state.apply(Action::CreateWorkspaceThread { workspace_id });
        let mut thread_ids = vec![state.active_thread_id(workspace_id).unwrap()];
        for _ in 0..3 {
            state.apply(Action::CreateWorkspaceThread { workspace_id });
            thread_ids.push(state.active_thread_id(workspace_id).unwrap());
        }

        let tabs = state.workspace_tabs(workspace_id).unwrap();
        assert_eq!(tabs.open_tabs, thread_ids);

        state.apply(Action::ActivateWorkspaceThread {
            workspace_id,
            thread_id: thread_ids[1],
        });
        let tabs = state.workspace_tabs(workspace_id).unwrap();
        assert_eq!(tabs.active_tab, thread_ids[1]);
        assert_eq!(
            tabs.open_tabs, thread_ids,
            "activating a thread should not reorder tabs"
        );

        let effects = state.apply(Action::ReorderWorkspaceThreadTab {
            workspace_id,
            thread_id: thread_ids[3],
            to_index: 1,
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));
        let tabs = state.workspace_tabs(workspace_id).unwrap();
        assert_eq!(
            tabs.open_tabs,
            vec![thread_ids[0], thread_ids[3], thread_ids[1], thread_ids[2]]
        );

        let persisted = state.to_persisted();
        assert_eq!(
            persisted.workspace_open_tabs.get(&workspace_id.0).cloned(),
            Some(vec![
                thread_ids[0].0,
                thread_ids[3].0,
                thread_ids[1].0,
                thread_ids[2].0
            ])
        );

        let closed_thread = thread_ids[3];
        state.apply(Action::CloseWorkspaceThreadTab {
            workspace_id,
            thread_id: closed_thread,
        });
        let tabs = state.workspace_tabs(workspace_id).unwrap();
        assert!(
            !tabs.open_tabs.contains(&closed_thread),
            "closing a tab should archive it"
        );
        assert!(
            tabs.archived_tabs.contains(&closed_thread),
            "archived tabs should retain the closed thread id"
        );
        assert!(
            state
                .workspace_thread_conversation(workspace_id, closed_thread)
                .is_some(),
            "archiving a tab should not delete the conversation"
        );

        state.apply(Action::RestoreWorkspaceThreadTab {
            workspace_id,
            thread_id: closed_thread,
        });
        let tabs = state.workspace_tabs(workspace_id).unwrap();
        assert!(
            tabs.open_tabs.contains(&closed_thread),
            "restoring a tab should re-open it"
        );
        assert!(
            !tabs.archived_tabs.contains(&closed_thread),
            "restoring a tab should remove it from the archive list"
        );
    }

    #[test]
    fn project_expanded_is_persisted() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;

        let effects = state.apply(Action::ToggleProjectExpanded { project_id });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));
        assert!(state.projects[0].expanded);

        let persisted = state.to_persisted();
        assert_eq!(persisted.projects.len(), 1);
        assert!(persisted.projects[0].expanded);

        let mut loaded = AppState::new();
        loaded.apply(Action::AppStateLoaded {
            persisted: Box::new(persisted),
        });
        assert!(loaded.projects[0].expanded);
    }

    #[test]
    fn agent_item_updates_are_appended_as_entries() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = workspace_id_by_name(&state, "abandon-about");
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Test".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let run_id = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation")
            .active_run_id
            .expect("missing active run id");

        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::ItemStarted {
                item: CodexThreadItem::Reasoning {
                    id: "r-1".to_owned(),
                    text: "x".to_owned(),
                },
            },
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::ItemStarted {
                item: CodexThreadItem::CommandExecution {
                    id: "c-1".to_owned(),
                    command: "echo hello".to_owned(),
                    aggregated_output: String::new(),
                    exit_code: None,
                    status: CodexCommandExecutionStatus::InProgress,
                },
            },
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");

        let agent_item_entries: Vec<(&str, &str)> = conversation
            .entries
            .iter()
            .filter_map(|entry| match entry {
                ConversationEntry::AgentEvent {
                    entry_id,
                    event: crate::AgentEvent::Item { item },
                } => Some((entry_id.as_str(), codex_item_id(item.as_ref()))),
                _ => None,
            })
            .collect();
        assert_eq!(agent_item_entries.len(), 2);
        assert_eq!(agent_item_entries[0].1, "r-1");
        assert_eq!(agent_item_entries[1].1, "c-1");
        assert_ne!(agent_item_entries[0].0, agent_item_entries[1].0);
    }

    #[test]
    fn app_started_emits_load_app_state_effect() {
        let mut state = AppState::new();
        let effects = state.apply(Action::AppStarted);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::LoadAppState));
    }

    #[test]
    fn add_project_emits_save_app_state_effect() {
        let mut state = AppState::new();
        let effects = state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));
    }

    #[test]
    fn main_workspace_cannot_be_archived() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });

        let workspace_id = main_workspace_id(&state);
        let effects = state.apply(Action::ArchiveWorkspace { workspace_id });
        assert!(effects.is_empty());

        let project = &state.projects[0];
        let workspace = project
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .expect("missing main workspace after archive attempt");
        assert_eq!(workspace.archive_status, OperationStatus::Idle);
        assert_eq!(workspace.status, WorkspaceStatus::Active);
        assert_eq!(workspace.worktree_path, project.path);
    }

    #[test]
    fn archiving_a_running_workspace_cancels_agent_turns_first() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;

        let worktree_path = PathBuf::from("/tmp/repo/worktrees/wt");
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "wt".to_owned(),
            branch_name: "feature".to_owned(),
            worktree_path: worktree_path.clone(),
        });

        let workspace_id = state.projects[0]
            .workspaces
            .iter()
            .find(|w| w.worktree_path == worktree_path)
            .expect("missing workspace")
            .id;
        state.apply(Action::CreateWorkspaceThread { workspace_id });
        let thread_id = state.active_thread_id(workspace_id).unwrap();

        {
            let conversation = state
                .conversations
                .get_mut(&(workspace_id, thread_id))
                .expect("missing conversation");
            conversation.run_status = OperationStatus::Running;
            conversation.active_run_id = Some(99);
        }

        let effects = state.apply(Action::ArchiveWorkspace { workspace_id });
        assert_eq!(effects.len(), 2);

        match &effects[0] {
            Effect::CancelAgentTurn {
                workspace_id: wid,
                thread_id: tid,
                run_id,
            } => {
                assert_eq!(*wid, workspace_id);
                assert_eq!(*tid, thread_id);
                assert_eq!(*run_id, 99);
            }
            other => panic!("expected CancelAgentTurn, got {other:?}"),
        }
        assert!(matches!(
            &effects[1],
            Effect::ArchiveWorkspace { workspace_id: wid } if *wid == workspace_id
        ));

        let conversation = state
            .conversations
            .get(&(workspace_id, thread_id))
            .expect("missing conversation");
        assert_eq!(conversation.run_status, OperationStatus::Idle);
        assert_eq!(conversation.active_run_id, None);
        assert!(conversation.queue_paused);
        assert!(matches!(
            conversation.entries.last(),
            Some(ConversationEntry::AgentEvent {
                event: crate::AgentEvent::TurnCanceled,
                ..
            })
        ));

        let workspace = state
            .workspace(workspace_id)
            .expect("missing workspace after archive request");
        assert_eq!(workspace.archive_status, OperationStatus::Running);
    }

    #[test]
    fn demo_state_is_consistent() {
        let state = AppState::demo();

        assert!(!state.projects.is_empty());
    }

    #[test]
    fn project_slug_is_sanitized_and_unique() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/My Project"),
            is_git: true,
        });
        state.apply(Action::AddProject {
            path: PathBuf::from("/home/My Project"),
            is_git: true,
        });

        assert_eq!(state.projects.len(), 2);
        assert_eq!(state.projects[0].slug, "my-project");
        assert_eq!(state.projects[1].slug, "my-project-2");
    }

    #[test]
    fn projects_are_deduped_by_normalized_path() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo/"),
            is_git: true,
        });

        assert_eq!(state.projects.len(), 1);
        assert_eq!(state.projects[0].path, PathBuf::from("/tmp/repo"));
    }

    #[test]
    fn delete_project_removes_state_and_emits_save_effect() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "main".to_owned(),
            branch_name: "main".to_owned(),
            worktree_path: PathBuf::from("/tmp/repo"),
        });
        let main_id = workspace_id_by_name(&state, "main");

        state.apply(Action::OpenWorkspace {
            workspace_id: main_id,
        });
        assert!(matches!(state.main_pane, MainPane::Workspace(_)));

        let effects = state.apply(Action::DeleteProject { project_id });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        assert!(state.projects.is_empty());
        assert!(!state.workspace_tabs.contains_key(&main_id));
        assert!(state.conversations.keys().all(|(wid, _)| *wid != main_id));
        assert!(state.last_open_workspace_id.is_none());
        assert_eq!(state.main_pane, MainPane::Dashboard);
        assert_eq!(state.right_pane, RightPane::None);
    }

    #[test]
    fn create_workspace_sets_busy_and_emits_effect() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;

        let effects = state.apply(Action::CreateWorkspace {
            project_id,
            branch_name_hint: None,
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::CreateWorkspace { .. }));

        let project = state.project(project_id).unwrap();
        assert_eq!(project.create_workspace_status, OperationStatus::Running);
    }

    #[test]
    fn open_workspace_emits_conversation_load_effect() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);

        let effects = state.apply(Action::OpenWorkspace { workspace_id });
        assert_eq!(effects.len(), 3);
        assert!(matches!(effects[0], Effect::SaveAppState));
        assert!(matches!(effects[1], Effect::LoadWorkspaceThreads { .. }));
        assert!(matches!(effects[2], Effect::LoadConversation { .. }));
    }

    #[test]
    fn app_state_restores_last_open_workspace() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = workspace_id_by_name(&state, "abandon-about");
        state.apply(Action::OpenWorkspace { workspace_id });

        let persisted = state.to_persisted();
        assert_eq!(persisted.last_open_workspace_id, Some(workspace_id.0));

        let mut loaded = AppState::new();
        let effects = loaded.apply(Action::AppStateLoaded {
            persisted: Box::new(persisted),
        });

        assert!(
            matches!(loaded.main_pane, MainPane::Workspace(id) if id == workspace_id),
            "expected main pane to restore workspace"
        );
        assert_eq!(loaded.right_pane, RightPane::Terminal);
        assert_eq!(effects.len(), 5);
        assert!(matches!(effects[0], Effect::LoadCodexDefaults));
        assert!(matches!(effects[1], Effect::LoadTaskPromptTemplates));
        assert!(matches!(effects[2], Effect::LoadSystemPromptTemplates));
        assert!(matches!(effects[3], Effect::LoadWorkspaceThreads { .. }));
        assert!(matches!(
            effects[4],
            Effect::LoadConversation { workspace_id: id, .. } if id == workspace_id
        ));
    }

    #[test]
    fn chat_drafts_are_isolated_and_preserved_on_reload() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;

        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/repo/worktrees/w1"),
        });
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w2".to_owned(),
            branch_name: "repo/w2".to_owned(),
            worktree_path: PathBuf::from("/tmp/repo/worktrees/w2"),
        });

        let w1 = workspace_id_by_name(&state, "w1");
        let w2 = workspace_id_by_name(&state, "w2");
        let thread_id = default_thread_id();
        state.apply(Action::CreateWorkspaceThread { workspace_id: w1 });
        state.apply(Action::CreateWorkspaceThread { workspace_id: w2 });

        state.apply(Action::ChatDraftChanged {
            workspace_id: w1,
            thread_id,
            text: "draft-1".to_owned(),
        });
        state.apply(Action::ChatDraftChanged {
            workspace_id: w2,
            thread_id,
            text: "draft-2".to_owned(),
        });

        assert_eq!(state.workspace_conversation(w1).unwrap().draft, "draft-1");
        assert_eq!(state.workspace_conversation(w2).unwrap().draft, "draft-2");

        state.apply(Action::ConversationLoaded {
            workspace_id: w1,
            thread_id,
            snapshot: ConversationSnapshot {
                title: None,
                thread_id: None,
                task_status: crate::TaskStatus::Todo,
                runner: None,
                agent_model_id: None,
                thinking_effort: None,
                amp_mode: None,
                entries: Vec::new(),
                entries_total: 0,
                entries_start: 0,
                pending_prompts: Vec::new(),
                queue_paused: false,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
            },
        });
        assert_eq!(state.workspace_conversation(w1).unwrap().draft, "draft-1");
    }

    #[test]
    fn chat_draft_edits_update_attachment_anchors_without_removing() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;

        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/repo/worktrees/w1"),
        });
        let w1 = workspace_id_by_name(&state, "w1");
        let thread_id = default_thread_id();
        state.apply(Action::CreateWorkspaceThread { workspace_id: w1 });

        state.apply(Action::ChatDraftChanged {
            workspace_id: w1,
            thread_id,
            text: "0123456789".to_owned(),
        });
        state.apply(Action::ChatDraftAttachmentAdded {
            workspace_id: w1,
            thread_id,
            id: 1,
            kind: ContextTokenKind::Image,
            anchor: 8,
        });
        state.apply(Action::ChatDraftAttachmentResolved {
            workspace_id: w1,
            thread_id,
            id: 1,
            attachment: crate::AttachmentRef {
                id: "blob-a".to_owned(),
                kind: crate::AttachmentKind::Image,
                name: "a.png".to_owned(),
                extension: "png".to_owned(),
                mime: None,
                byte_len: 1,
            },
        });
        state.apply(Action::ChatDraftAttachmentAdded {
            workspace_id: w1,
            thread_id,
            id: 2,
            kind: ContextTokenKind::Text,
            anchor: 5,
        });
        state.apply(Action::ChatDraftAttachmentResolved {
            workspace_id: w1,
            thread_id,
            id: 2,
            attachment: crate::AttachmentRef {
                id: "blob-b".to_owned(),
                kind: crate::AttachmentKind::Text,
                name: "b.txt".to_owned(),
                extension: "txt".to_owned(),
                mime: None,
                byte_len: 1,
            },
        });

        // Delete bytes [3,7): "3456" -> "012789".
        state.apply(Action::ChatDraftChanged {
            workspace_id: w1,
            thread_id,
            text: "012789".to_owned(),
        });

        let conversation = state
            .workspace_conversation(w1)
            .expect("missing conversation");
        assert_eq!(conversation.draft, "012789");
        assert_eq!(conversation.draft_attachments.len(), 2);

        let a = conversation
            .draft_attachments
            .iter()
            .find(|a| a.id == 1)
            .expect("missing attachment 1");
        let b = conversation
            .draft_attachments
            .iter()
            .find(|a| a.id == 2)
            .expect("missing attachment 2");

        // Anchor 8 shifts by -4 -> 4.
        assert_eq!(a.anchor, 4);
        // Anchor 5 is inside the deleted range -> snaps to start (3).
        assert_eq!(b.anchor, 3);
    }

    #[test]
    fn conversation_loaded_does_not_reset_running_turn_state() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Hello".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let run_id = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation")
            .active_run_id
            .expect("missing active run id");

        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::ItemStarted { item },
        });

        assert_eq!(
            state
                .workspace_conversation(workspace_id)
                .unwrap()
                .run_status,
            OperationStatus::Running
        );
        let before_entries = state
            .workspace_conversation(workspace_id)
            .unwrap()
            .entries
            .iter()
            .filter(|e| !matches!(e, ConversationEntry::SystemEvent { .. }))
            .collect::<Vec<_>>();
        assert_eq!(before_entries.len(), 2);
        assert!(matches!(
            &before_entries[1],
            ConversationEntry::AgentEvent {
                event: crate::AgentEvent::Message { id, .. },
                ..
            } if id == "item_0"
        ));

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                title: None,
                thread_id: Some("thread_0".to_owned()),
                task_status: crate::TaskStatus::Todo,
                runner: None,
                agent_model_id: None,
                thinking_effort: None,
                amp_mode: None,
                entries: Vec::new(),
                entries_total: 0,
                entries_start: 0,
                pending_prompts: Vec::new(),
                queue_paused: false,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
            },
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Running);
        let entries = conversation
            .entries
            .iter()
            .filter(|e| !matches!(e, ConversationEntry::SystemEvent { .. }))
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 2);
        assert!(matches!(
            &entries[0],
            ConversationEntry::UserEvent {
                event: crate::UserEvent::Message { text, .. },
                ..
            } if text == "Hello"
        ));
        assert_eq!(conversation.thread_id.as_deref(), Some("thread_0"));
    }

    #[test]
    fn conversation_loaded_does_not_overwrite_newer_local_entries() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Hello".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id: state
                .workspace_thread_conversation(workspace_id, thread_id)
                .expect("missing conversation")
                .active_run_id
                .expect("missing active run id"),
            event: CodexThreadEvent::TurnDuration { duration_ms: 1234 },
        });

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                title: None,
                thread_id: None,
                task_status: crate::TaskStatus::Todo,
                runner: None,
                agent_model_id: None,
                thinking_effort: None,
                amp_mode: None,
                entries: vec![ConversationEntry::UserEvent {
                    entry_id: String::new(),
                    event: crate::UserEvent::Message {
                        text: "Hello".to_owned(),
                        attachments: Vec::new(),
                    },
                }],
                entries_total: 0,
                entries_start: 0,
                pending_prompts: Vec::new(),
                queue_paused: false,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
            },
        });

        let after = state
            .workspace_conversation(workspace_id)
            .unwrap()
            .entries
            .iter()
            .filter(|e| !matches!(e, ConversationEntry::SystemEvent { .. }))
            .collect::<Vec<_>>();
        assert_eq!(after.len(), 2);
        assert!(matches!(
            &after[0],
            ConversationEntry::UserEvent {
                event: crate::UserEvent::Message { text, .. },
                ..
            } if text == "Hello"
        ));
        assert!(matches!(
            &after[1],
            ConversationEntry::AgentEvent {
                event: crate::AgentEvent::TurnDuration { duration_ms: 1234 },
                ..
            }
        ));
    }

    #[test]
    fn conversation_loaded_replaces_entries_when_snapshot_is_newer() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                title: None,
                thread_id: None,
                task_status: crate::TaskStatus::Todo,
                runner: None,
                agent_model_id: None,
                thinking_effort: None,
                amp_mode: None,
                entries: vec![ConversationEntry::UserEvent {
                    entry_id: String::new(),
                    event: crate::UserEvent::Message {
                        text: "Hello".to_owned(),
                        attachments: Vec::new(),
                    },
                }],
                entries_total: 0,
                entries_start: 0,
                pending_prompts: Vec::new(),
                queue_paused: false,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
            },
        });

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                title: None,
                thread_id: None,
                task_status: crate::TaskStatus::Todo,
                runner: None,
                agent_model_id: None,
                thinking_effort: None,
                amp_mode: None,
                entries: vec![
                    ConversationEntry::UserEvent {
                        entry_id: String::new(),
                        event: crate::UserEvent::Message {
                            text: "Hello".to_owned(),
                            attachments: Vec::new(),
                        },
                    },
                    ConversationEntry::AgentEvent {
                        entry_id: String::new(),
                        event: crate::AgentEvent::TurnDuration { duration_ms: 1234 },
                    },
                ],
                entries_total: 0,
                entries_start: 0,
                pending_prompts: Vec::new(),
                queue_paused: false,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
            },
        });

        let after = &state.workspace_conversation(workspace_id).unwrap().entries;
        assert!(matches!(
            &after[..],
            [
                ConversationEntry::UserEvent {
                    event: crate::UserEvent::Message { .. },
                    ..
                },
                ConversationEntry::AgentEvent {
                    event: crate::AgentEvent::TurnDuration { duration_ms: 1234 },
                    ..
                }
            ]
        ));
    }

    #[test]
    fn conversation_loaded_restores_queued_prompts_when_local_is_empty() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                title: None,
                thread_id: None,
                task_status: crate::TaskStatus::Todo,
                runner: None,
                agent_model_id: None,
                thinking_effort: None,
                amp_mode: None,
                entries: Vec::new(),
                entries_total: 0,
                entries_start: 0,
                pending_prompts: vec![QueuedPrompt {
                    id: 3,
                    text: "Queued".to_owned(),
                    attachments: Vec::new(),
                    run_config: AgentRunConfig {
                        runner: crate::AgentRunnerKind::Codex,
                        model_id: "gpt-5.1-codex-mini".to_owned(),
                        thinking_effort: ThinkingEffort::Low,
                        amp_mode: None,
                    },
                }],
                queue_paused: true,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
            },
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert!(conversation.queue_paused);
        assert_eq!(conversation.pending_prompts.len(), 1);
        assert_eq!(conversation.pending_prompts[0].id, 3);
        assert_eq!(conversation.next_queued_prompt_id, 4);
    }

    #[test]
    fn conversation_loaded_applies_persisted_run_config() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                title: None,
                thread_id: None,
                task_status: crate::TaskStatus::Todo,
                runner: None,
                agent_model_id: Some("gpt-5.2-codex".to_owned()),
                thinking_effort: Some(ThinkingEffort::High),
                amp_mode: None,
                entries: Vec::new(),
                entries_total: 0,
                entries_start: 0,
                pending_prompts: Vec::new(),
                queue_paused: false,
                run_started_at_unix_ms: None,
                run_finished_at_unix_ms: None,
            },
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert_eq!(conversation.agent_model_id, "gpt-5.2-codex");
        assert_eq!(conversation.thinking_effort, ThinkingEffort::High);
    }

    #[test]
    fn conversation_entries_are_bounded_in_memory() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Hello".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let run_id = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation")
            .active_run_id
            .expect("missing active run id");

        let total = crate::state::MAX_CONVERSATION_ENTRIES_IN_MEMORY + 100;
        for idx in 0..total {
            state.apply(Action::AgentEventReceived {
                workspace_id,
                thread_id,
                run_id,
                event: CodexThreadEvent::TurnDuration {
                    duration_ms: idx as u64,
                },
            });
        }

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(
            conversation.entries.len(),
            crate::state::MAX_CONVERSATION_ENTRIES_IN_MEMORY
        );
        assert_eq!(conversation.entries_start, 102);
        assert_eq!(conversation.entries_total, (total + 2) as u64);
    }

    #[test]
    fn send_agent_message_sets_running_and_emits_effect() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        let effects = state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Hello".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let run_effect = effects
            .iter()
            .find(|e| matches!(e, Effect::RunAgentTurn { .. }))
            .expect("missing RunAgentTurn effect");
        assert!(matches!(
            run_effect,
            Effect::RunAgentTurn {
                workspace_id: wid,
                thread_id: tid,
                text,
                run_config,
                ..
            } if *wid == workspace_id
                && *tid == thread_id
                && text == "Hello"
                && run_config.model_id == default_agent_model_id()
                && run_config.thinking_effort == default_thinking_effort()
        ));

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Running);
        let user_messages = conversation
            .entries
            .iter()
            .filter_map(|e| match e {
                ConversationEntry::UserEvent { event, .. } => match event {
                    crate::UserEvent::Message { text, .. } => Some(text.as_str()),
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(user_messages, vec!["Hello"]);
    }

    #[test]
    fn agent_item_completed_is_idempotent() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Hello".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let run_id = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation")
            .active_run_id
            .expect("missing active run id");

        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };

        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::ItemCompleted { item: item.clone() },
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::ItemCompleted { item },
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        let completed_items = conversation
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    ConversationEntry::AgentEvent {
                        event: crate::AgentEvent::Message { id, .. },
                        ..
                    } if id == "item_0"
                )
            })
            .count();
        assert_eq!(completed_items, 1);
    }

    #[test]
    fn agent_item_completed_is_idempotent_even_if_not_last_entry() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Hello".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let run_id = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation")
            .active_run_id
            .expect("missing active run id");

        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };

        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::ItemCompleted { item: item.clone() },
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::TurnDuration { duration_ms: 1000 },
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::ItemCompleted { item },
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        let completed_items = conversation
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    ConversationEntry::AgentEvent {
                        event: crate::AgentEvent::Message { id, .. },
                        ..
                    } if id == "item_0"
                )
            })
            .count();
        assert_eq!(completed_items, 1);
    }

    #[test]
    fn cancel_agent_turn_sets_idle_and_emits_effect() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Hello".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        let effects = state.apply(Action::CancelAgentTurn {
            workspace_id,
            thread_id,
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::CancelAgentTurn { .. }));

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Idle);
        assert!(matches!(
            conversation.entries.last(),
            Some(ConversationEntry::AgentEvent {
                event: crate::AgentEvent::TurnCanceled,
                ..
            })
        ));
    }

    #[test]
    fn send_agent_message_while_running_is_queued() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "First".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let effects = state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Second".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        assert!(effects.is_empty());

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(
            conversation
                .entries
                .iter()
                .filter(|e| matches!(e, ConversationEntry::UserEvent { .. }))
                .count(),
            1
        );
        assert_eq!(conversation.pending_prompts.len(), 1);
        assert_eq!(conversation.pending_prompts[0].text, "Second");
        assert_eq!(conversation.pending_prompts[0].id, 1);
        assert_eq!(conversation.run_status, OperationStatus::Running);
    }

    #[test]
    fn queued_prompts_can_be_reordered_and_edited() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "First".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Second".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Third".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.pending_prompts.len(), 2);
        assert_eq!(conversation.pending_prompts[0].id, 1);
        assert_eq!(conversation.pending_prompts[1].id, 2);

        state.apply(Action::ReorderQueuedPrompt {
            workspace_id,
            thread_id,
            active_id: 2,
            over_id: 1,
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.pending_prompts[0].text, "Third");
        assert_eq!(conversation.pending_prompts[1].text, "Second");

        state.apply(Action::UpdateQueuedPrompt {
            workspace_id,
            thread_id,
            prompt_id: 1,
            text: "Second updated".to_owned(),
            attachments: Vec::new(),
            model_id: default_agent_model_id().to_owned(),
            thinking_effort: default_thinking_effort(),
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.pending_prompts[1].text, "Second updated");

        state.apply(Action::RemoveQueuedPrompt {
            workspace_id,
            thread_id,
            prompt_id: 2,
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.pending_prompts.len(), 1);
        assert_eq!(conversation.pending_prompts[0].id, 1);
    }

    #[test]
    fn completed_turn_auto_sends_next_queued_prompt() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "First".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Second".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        let run_id = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation")
            .active_run_id
            .expect("missing active run id");
        let effects = state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::TurnCompleted {
                usage: CodexUsage {
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                },
            },
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            &effects[0],
            Effect::RunAgentTurn {
                workspace_id: wid,
                thread_id: tid,
                text,
                run_config,
                ..
            } if *wid == workspace_id
                && *tid == thread_id
                && text == "Second"
                && run_config.model_id == default_agent_model_id()
                && run_config.thinking_effort == default_thinking_effort()
        ));

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Running);
        assert!(conversation.pending_prompts.is_empty());
        let user_messages = conversation
            .entries
            .iter()
            .filter_map(|e| match e {
                ConversationEntry::UserEvent { event, .. } => match event {
                    crate::UserEvent::Message { text, .. } => Some(text.as_str()),
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(user_messages, vec!["First", "Second"]);
    }

    #[test]
    fn failed_turn_pauses_queue_until_resumed() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "First".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Second".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });

        let run_id = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation")
            .active_run_id
            .expect("missing active run id");
        let effects = state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id,
            event: CodexThreadEvent::TurnFailed {
                error: CodexThreadError {
                    message: "boom".to_owned(),
                },
            },
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::AiAutoUpdateTaskStatus { .. }));

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Idle);
        assert_eq!(conversation.pending_prompts.len(), 1);
        assert!(conversation.queue_paused);

        let effects = state.apply(Action::ResumeQueuedPrompts {
            workspace_id,
            thread_id,
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            &effects[0],
            Effect::RunAgentTurn {
                workspace_id: wid,
                thread_id: tid,
                text,
                run_config,
                ..
            } if *wid == workspace_id
                && *tid == thread_id
                && text == "Second"
                && run_config.model_id == default_agent_model_id()
                && run_config.thinking_effort == default_thinking_effort()
        ));
    }

    #[test]
    fn stale_agent_events_are_ignored_after_new_run_starts() {
        let mut state = AppState::demo();
        let workspace_id = first_non_main_workspace_id(&state);
        let thread_id = default_thread_id();

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "First".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let run_id_a = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation")
            .active_run_id
            .expect("missing active run id");

        state.apply(Action::CancelAgentTurn {
            workspace_id,
            thread_id,
        });

        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Second".to_owned(),
            attachments: Vec::new(),
            runner: None,
            amp_mode: None,
        });
        let run_id_b = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation")
            .active_run_id
            .expect("missing active run id");
        assert_ne!(run_id_a, run_id_b);

        let effects = state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            run_id: run_id_a,
            event: CodexThreadEvent::TurnCompleted {
                usage: CodexUsage {
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                },
            },
        });
        assert!(effects.is_empty());

        state.apply(Action::AgentRunFinishedAt {
            workspace_id,
            thread_id,
            run_id: run_id_a,
            finished_at_unix_ms: 123,
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Running);
        assert_eq!(conversation.active_run_id, Some(run_id_b));
        assert_eq!(conversation.run_finished_at_unix_ms, None);
        assert!(conversation.entries.iter().all(|entry| {
            !matches!(
                entry,
                ConversationEntry::AgentEvent {
                    event: crate::AgentEvent::TurnError { .. },
                    ..
                }
            )
        }));
    }

    #[test]
    fn open_workspace_in_ide_emits_effect_for_existing_workspace() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = workspace_id_by_name(&state, "abandon-about");

        let effects = state.apply(Action::OpenWorkspaceInIde { workspace_id });
        assert!(
            matches!(
                effects.as_slice(),
                [Effect::OpenWorkspaceInIde {
                    workspace_id: effect_workspace_id
                }] if *effect_workspace_id == workspace_id
            ),
            "unexpected effects: {effects:?}"
        );
    }

    #[test]
    fn open_workspace_in_ide_sets_error_when_workspace_missing() {
        let mut state = AppState::new();
        let effects = state.apply(Action::OpenWorkspaceInIde {
            workspace_id: WorkspaceId(1),
        });
        assert!(effects.is_empty());
        assert_eq!(state.last_error.as_deref(), Some("Workspace not found"));
    }

    #[test]
    fn open_workspace_pull_request_emits_effect_for_existing_workspace() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = workspace_id_by_name(&state, "abandon-about");

        let effects = state.apply(Action::OpenWorkspacePullRequest { workspace_id });
        assert!(
            matches!(
                effects.as_slice(),
                [Effect::OpenWorkspacePullRequest {
                    workspace_id: effect_workspace_id
                }] if *effect_workspace_id == workspace_id
            ),
            "unexpected effects: {effects:?}"
        );
    }

    #[test]
    fn open_workspace_pull_request_sets_error_when_workspace_missing() {
        let mut state = AppState::new();
        let effects = state.apply(Action::OpenWorkspacePullRequest {
            workspace_id: WorkspaceId(1),
        });
        assert!(effects.is_empty());
        assert_eq!(state.last_error.as_deref(), Some("Workspace not found"));
    }

    #[test]
    fn open_workspace_pull_request_failed_action_emits_effect_for_existing_workspace() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
            is_git: true,
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        });
        let workspace_id = workspace_id_by_name(&state, "abandon-about");

        let effects = state.apply(Action::OpenWorkspacePullRequestFailedAction { workspace_id });
        assert!(
            matches!(
                effects.as_slice(),
                [Effect::OpenWorkspacePullRequestFailedAction {
                    workspace_id: effect_workspace_id
                }] if *effect_workspace_id == workspace_id
            ),
            "unexpected effects: {effects:?}"
        );
    }

    #[test]
    fn open_workspace_pull_request_failed_action_sets_error_when_workspace_missing() {
        let mut state = AppState::new();
        let effects = state.apply(Action::OpenWorkspacePullRequestFailedAction {
            workspace_id: WorkspaceId(1),
        });
        assert!(effects.is_empty());
        assert_eq!(state.last_error.as_deref(), Some("Workspace not found"));
    }
}
