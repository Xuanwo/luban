use crate::persistence;
use crate::state::{
    apply_draft_text_diff, codex_item_id, entries_contain_codex_item, entries_is_prefix,
    entries_is_suffix, flush_in_progress_items,
};
use crate::{
    Action, AgentRunConfig, AppState, CodexThreadEvent, ConversationEntry, DraftAttachment, Effect,
    MainPane, OperationStatus, PersistedAppState, Project, ProjectId, QueuedPrompt, RightPane,
    ThinkingEffort, Workspace, WorkspaceConversation, WorkspaceId, WorkspaceStatus, WorkspaceTabs,
    WorkspaceThreadId, default_agent_model_id, default_thinking_effort, normalize_thinking_effort,
    thinking_effort_supported,
};
use std::collections::VecDeque;
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

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
            agent_default_model_id: default_agent_model_id().to_owned(),
            agent_default_thinking_effort: default_thinking_effort(),
            conversations: HashMap::new(),
            workspace_tabs: HashMap::new(),
            dashboard_preview_workspace_id: None,
            last_open_workspace_id: None,
            last_error: None,
            workspace_chat_scroll_y10: HashMap::new(),
        }
    }

    pub fn demo() -> Self {
        let mut this = Self::new();
        let p1 = this.add_project(PathBuf::from("/Users/example/luban"));
        let p2 = this.add_project(PathBuf::from("/Users/example/scratch"));

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

        this
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
                    let thread_id = self.ensure_workspace_tabs_mut(workspace_id).active_tab;
                    effects.push(Effect::LoadWorkspaceThreads { workspace_id });
                    effects.push(Effect::LoadConversation {
                        workspace_id,
                        thread_id,
                    });
                }
                effects
            }
            Action::DashboardPreviewOpened { workspace_id } => {
                if self.workspace(workspace_id).is_none() {
                    return Vec::new();
                }
                self.dashboard_preview_workspace_id = Some(workspace_id);
                let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                vec![
                    Effect::LoadWorkspaceThreads { workspace_id },
                    Effect::LoadConversation {
                        workspace_id,
                        thread_id: tabs.active_tab,
                    },
                ]
            }
            Action::DashboardPreviewClosed => {
                self.dashboard_preview_workspace_id = None;
                Vec::new()
            }

            Action::AddProject { path } => {
                let project_id = self.add_project(path);
                self.insert_main_workspace(project_id);
                vec![Effect::SaveAppState]
            }
            Action::ToggleProjectExpanded { project_id } => {
                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    project.expanded = !project.expanded;
                }
                vec![Effect::SaveAppState]
            }
            Action::OpenProjectSettings { project_id } => {
                self.main_pane = MainPane::ProjectSettings(project_id);
                self.right_pane = RightPane::None;
                self.dashboard_preview_workspace_id = None;
                Vec::new()
            }

            Action::CreateWorkspace { project_id } => {
                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    if project.create_workspace_status == OperationStatus::Running {
                        return Vec::new();
                    }
                    project.create_workspace_status = OperationStatus::Running;
                }
                vec![Effect::CreateWorkspace { project_id }]
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
                let initial_thread_id = WorkspaceThreadId(1);
                self.workspace_tabs.insert(
                    workspace_id,
                    WorkspaceTabs::new_with_initial(initial_thread_id),
                );
                self.conversations.insert(
                    (workspace_id, initial_thread_id),
                    self.default_conversation(initial_thread_id),
                );
                vec![
                    Effect::SaveAppState,
                    Effect::EnsureConversation {
                        workspace_id,
                        thread_id: initial_thread_id,
                    },
                ]
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
                let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                let thread_id = tabs.active_tab;
                vec![
                    Effect::SaveAppState,
                    Effect::LoadWorkspaceThreads { workspace_id },
                    Effect::LoadConversation {
                        workspace_id,
                        thread_id,
                    },
                ]
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
            Action::ArchiveWorkspace { workspace_id } => {
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

                    let project = &mut self.projects[project_idx];
                    let workspace = &mut project.workspaces[workspace_idx];

                    if workspace.archive_status == OperationStatus::Running {
                        return Vec::new();
                    }
                    workspace.archive_status = OperationStatus::Running;
                    project.expanded = true;
                }
                vec![Effect::ArchiveWorkspace { workspace_id }]
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

            Action::ConversationLoaded {
                workspace_id,
                thread_id,
                snapshot,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);

                if conversation.thread_id.is_none() {
                    conversation.thread_id = snapshot.thread_id;
                }

                if conversation.entries.is_empty() {
                    conversation.entries = snapshot.entries;
                    return Vec::new();
                }

                let snapshot_is_newer = entries_is_prefix(&conversation.entries, &snapshot.entries)
                    || entries_is_suffix(&conversation.entries, &snapshot.entries);
                let conversation_is_newer =
                    entries_is_prefix(&snapshot.entries, &conversation.entries)
                        || entries_is_suffix(&snapshot.entries, &conversation.entries);

                if snapshot_is_newer && !conversation_is_newer {
                    conversation.entries = snapshot.entries;
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
            } => {
                let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                tabs.activate(thread_id);

                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                conversation.draft.clear();
                conversation.draft_attachments.clear();

                if conversation.entries.is_empty() && conversation.title.starts_with("Thread ") {
                    let title = derive_thread_title(&text);
                    if !title.is_empty() {
                        conversation.title = title;
                    }
                }

                let run_config = AgentRunConfig {
                    model_id: conversation.agent_model_id.clone(),
                    thinking_effort: conversation.thinking_effort,
                };

                if conversation.run_status == OperationStatus::Running {
                    conversation
                        .pending_prompts
                        .push_back(QueuedPrompt { text, run_config });
                    return Vec::new();
                }

                if conversation.queue_paused && !conversation.pending_prompts.is_empty() {
                    conversation
                        .entries
                        .push(ConversationEntry::UserMessage { text: text.clone() });
                    conversation.run_status = OperationStatus::Running;
                    conversation.current_run_config = Some(run_config.clone());
                    conversation.in_progress_items.clear();
                    conversation.in_progress_order.clear();
                    return vec![Effect::RunAgentTurn {
                        workspace_id,
                        thread_id,
                        text,
                        run_config,
                    }];
                }

                if conversation.pending_prompts.is_empty() {
                    conversation.queue_paused = false;
                    conversation
                        .entries
                        .push(ConversationEntry::UserMessage { text: text.clone() });
                    conversation.run_status = OperationStatus::Running;
                    conversation.current_run_config = Some(run_config.clone());
                    conversation.in_progress_items.clear();
                    conversation.in_progress_order.clear();
                    return vec![Effect::RunAgentTurn {
                        workspace_id,
                        thread_id,
                        text,
                        run_config,
                    }];
                }

                conversation
                    .pending_prompts
                    .push_back(QueuedPrompt { text, run_config });
                start_next_queued_prompt(conversation, workspace_id, thread_id)
                    .into_iter()
                    .collect()
            }
            Action::ChatModelChanged {
                workspace_id,
                thread_id,
                model_id,
            } => {
                let thinking_effort = {
                    let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                    conversation.agent_model_id = model_id.clone();
                    conversation.thinking_effort =
                        normalize_thinking_effort(&model_id, conversation.thinking_effort);
                    conversation.thinking_effort
                };
                self.agent_default_model_id = model_id;
                self.agent_default_thinking_effort = thinking_effort;
                vec![Effect::SaveAppState]
            }
            Action::ThinkingEffortChanged {
                workspace_id,
                thread_id,
                thinking_effort,
            } => {
                {
                    let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                    if !thinking_effort_supported(&conversation.agent_model_id, thinking_effort) {
                        return Vec::new();
                    }
                    conversation.thinking_effort = thinking_effort;
                }
                self.agent_default_thinking_effort = thinking_effort;
                vec![Effect::SaveAppState]
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
                    path: None,
                    failed: false,
                });
                Vec::new()
            }
            Action::ChatDraftAttachmentResolved {
                workspace_id,
                thread_id,
                id,
                path,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                if let Some(attachment) = conversation
                    .draft_attachments
                    .iter_mut()
                    .find(|a| a.id == id)
                {
                    attachment.path = Some(path);
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
                index,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);
                let _ = conversation.pending_prompts.remove(index);
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
            Action::AgentEventReceived {
                workspace_id,
                thread_id,
                event,
            } => {
                let conversation = self.ensure_conversation_mut(workspace_id, thread_id);

                match event {
                    CodexThreadEvent::ThreadStarted { thread_id } => {
                        conversation.thread_id = Some(thread_id);
                        Vec::new()
                    }
                    CodexThreadEvent::TurnStarted => Vec::new(),
                    CodexThreadEvent::TurnCompleted { usage } => {
                        let _ = usage;
                        conversation.run_status = OperationStatus::Idle;
                        conversation.current_run_config = None;
                        flush_in_progress_items(conversation);
                        conversation.in_progress_items.clear();
                        conversation.in_progress_order.clear();
                        start_next_queued_prompt(conversation, workspace_id, thread_id)
                            .into_iter()
                            .collect()
                    }
                    CodexThreadEvent::TurnDuration { duration_ms } => {
                        conversation
                            .entries
                            .push(ConversationEntry::TurnDuration { duration_ms });
                        Vec::new()
                    }
                    CodexThreadEvent::TurnFailed { error } => {
                        flush_in_progress_items(conversation);
                        conversation.entries.push(ConversationEntry::TurnError {
                            message: error.message.clone(),
                        });
                        conversation.run_status = OperationStatus::Idle;
                        conversation.current_run_config = None;
                        conversation.in_progress_items.clear();
                        conversation.in_progress_order.clear();
                        conversation.queue_paused = true;
                        self.last_error = Some(error.message);
                        Vec::new()
                    }
                    CodexThreadEvent::ItemStarted { item }
                    | CodexThreadEvent::ItemUpdated { item } => {
                        let id = codex_item_id(&item).to_owned();
                        conversation.in_progress_items.insert(id.clone(), item);
                        if !conversation.in_progress_order.iter().any(|v| v == &id) {
                            conversation.in_progress_order.push_back(id);
                        }
                        Vec::new()
                    }
                    CodexThreadEvent::ItemCompleted { item } => {
                        let id = codex_item_id(&item);
                        conversation.in_progress_items.remove(id);
                        if let Some(pos) =
                            conversation.in_progress_order.iter().position(|v| v == id)
                        {
                            conversation.in_progress_order.remove(pos);
                        }
                        let is_duplicate = entries_contain_codex_item(&conversation.entries, &item);
                        if !is_duplicate {
                            conversation.entries.push(ConversationEntry::CodexItem {
                                item: Box::new(item),
                            });
                        }
                        Vec::new()
                    }
                    CodexThreadEvent::Error { message } => {
                        flush_in_progress_items(conversation);
                        conversation.entries.push(ConversationEntry::TurnError {
                            message: message.clone(),
                        });
                        conversation.run_status = OperationStatus::Idle;
                        conversation.current_run_config = None;
                        conversation.in_progress_items.clear();
                        conversation.in_progress_order.clear();
                        conversation.queue_paused = true;
                        self.last_error = Some(message);
                        Vec::new()
                    }
                }
            }
            Action::AgentTurnFinished {
                workspace_id,
                thread_id,
            } => {
                if let Some(conversation) = self.conversations.get_mut(&(workspace_id, thread_id))
                    && conversation.run_status == OperationStatus::Running
                {
                    conversation.run_status = OperationStatus::Idle;
                    conversation.current_run_config = None;
                    flush_in_progress_items(conversation);
                    conversation.in_progress_items.clear();
                    conversation.in_progress_order.clear();
                }
                Vec::new()
            }
            Action::CancelAgentTurn {
                workspace_id,
                thread_id,
            } => {
                let Some(conversation) = self.conversations.get_mut(&(workspace_id, thread_id))
                else {
                    return Vec::new();
                };
                if conversation.run_status != OperationStatus::Running {
                    return Vec::new();
                }
                conversation.run_status = OperationStatus::Idle;
                conversation.current_run_config = None;
                flush_in_progress_items(conversation);
                conversation.in_progress_items.clear();
                conversation.in_progress_order.clear();
                conversation.queue_paused = true;
                conversation.entries.push(ConversationEntry::TurnCanceled);
                vec![Effect::CancelAgentTurn {
                    workspace_id,
                    thread_id,
                }]
            }
            Action::CreateWorkspaceThread { workspace_id } => {
                let thread_id = {
                    let tabs = self.ensure_workspace_tabs_mut(workspace_id);
                    tabs.allocate_thread_id()
                };
                self.conversations.insert(
                    (workspace_id, thread_id),
                    self.default_conversation(thread_id),
                );
                self.ensure_workspace_tabs_mut(workspace_id)
                    .activate(thread_id);
                vec![
                    Effect::SaveAppState,
                    Effect::EnsureConversation {
                        workspace_id,
                        thread_id,
                    },
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
                let mut effects = vec![Effect::SaveAppState];
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
                tabs.reorder_tab(thread_id, to_index);
                Vec::new()
            }
            Action::WorkspaceThreadsLoaded {
                workspace_id,
                threads,
            } => {
                self.ensure_workspace_tabs_mut(workspace_id);
                let default_model_id = self.agent_default_model_id.clone();
                let default_thinking_effort = self.agent_default_thinking_effort;
                let mut max_thread_id = 0u64;
                for meta in threads {
                    max_thread_id = max_thread_id.max(meta.thread_id.0);
                    let conversation = self
                        .conversations
                        .entry((workspace_id, meta.thread_id))
                        .or_insert_with(|| {
                            Self::default_conversation_with_defaults(
                                meta.thread_id,
                                default_model_id.clone(),
                                default_thinking_effort,
                            )
                        });
                    conversation.title = meta.title;
                    conversation.thread_id = meta.remote_thread_id;
                }
                if let Some(tabs) = self.workspace_tabs.get_mut(&workspace_id) {
                    tabs.next_thread_id = tabs.next_thread_id.max(max_thread_id + 1);
                }
                Vec::new()
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
            Action::SidebarWidthChanged { width } => {
                self.sidebar_width = Some(width);
                vec![Effect::SaveAppState]
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
            Action::SaveAppState => vec![Effect::SaveAppState],

            Action::AppStateLoaded { persisted } => {
                persistence::apply_persisted_app_state(self, persisted)
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
        let thread_id = self
            .workspace_tabs
            .get(&workspace_id)
            .map(|tabs| tabs.active_tab)?;
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
        self.workspace_tabs.get(&workspace_id).map(|t| t.active_tab)
    }

    fn ensure_workspace_tabs_mut(&mut self, workspace_id: WorkspaceId) -> &mut WorkspaceTabs {
        use std::collections::hash_map::Entry;

        let default_model_id = self.agent_default_model_id.clone();
        let default_thinking_effort = self.agent_default_thinking_effort;
        match self.workspace_tabs.entry(workspace_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let initial = WorkspaceThreadId(1);
                let conversation = Self::default_conversation_with_defaults(
                    initial,
                    default_model_id,
                    default_thinking_effort,
                );
                self.conversations
                    .insert((workspace_id, initial), conversation);
                entry.insert(WorkspaceTabs::new_with_initial(initial))
            }
        }
    }

    fn ensure_conversation_mut(
        &mut self,
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
    ) -> &mut WorkspaceConversation {
        self.ensure_workspace_tabs_mut(workspace_id);
        let default_model_id = self.agent_default_model_id.clone();
        let default_thinking_effort = self.agent_default_thinking_effort;
        self.conversations
            .entry((workspace_id, thread_id))
            .or_insert_with(|| {
                Self::default_conversation_with_defaults(
                    thread_id,
                    default_model_id.clone(),
                    default_thinking_effort,
                )
            })
    }

    fn default_conversation_with_defaults(
        thread_id: WorkspaceThreadId,
        model_id: String,
        thinking_effort: ThinkingEffort,
    ) -> WorkspaceConversation {
        WorkspaceConversation {
            local_thread_id: thread_id,
            title: format!("Thread {}", thread_id.0),
            thread_id: None,
            draft: String::new(),
            draft_attachments: Vec::new(),
            agent_model_id: model_id,
            thinking_effort,
            entries: Vec::new(),
            run_status: OperationStatus::Idle,
            current_run_config: None,
            in_progress_items: BTreeMap::new(),
            in_progress_order: VecDeque::new(),
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
        )
    }

    fn add_project(&mut self, path: PathBuf) -> ProjectId {
        let id = ProjectId(self.next_project_id);
        self.next_project_id += 1;

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("project")
            .to_owned();

        let slug = self.unique_project_slug(sanitize_slug(&name));

        self.projects.push(Project {
            id,
            name,
            path,
            slug,
            expanded: false,
            create_workspace_status: OperationStatus::Idle,
            workspaces: Vec::new(),
        });

        id
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
        });

        workspace_id
    }

    pub(crate) fn ensure_main_workspaces(&mut self) -> bool {
        let mut upgraded = false;

        let project_ids: Vec<ProjectId> = self.projects.iter().map(|p| p.id).collect();
        for project_id in project_ids {
            let has_main = self
                .projects
                .iter()
                .find(|p| p.id == project_id)
                .map(|project| {
                    project.workspaces.iter().any(|w| {
                        w.status == WorkspaceStatus::Active && w.worktree_path == project.path
                    })
                })
                .unwrap_or(false);
            if has_main {
                continue;
            }

            self.insert_main_workspace(project_id);
            upgraded = true;
        }

        upgraded
    }

    fn workspace_is_main(project: &Project, workspace: &Workspace) -> bool {
        workspace.worktree_path == project.path
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

fn sanitize_slug(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = false;

    for ch in input.chars() {
        let mapped = match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            'A'..='Z' => Some(ch.to_ascii_lowercase()),
            _ => None,
        };

        match mapped {
            Some(ch) => {
                out.push(ch);
                prev_dash = false;
            }
            None => {
                if !prev_dash && !out.is_empty() {
                    out.push('-');
                    prev_dash = true;
                }
            }
        }
    }

    while out.ends_with('-') {
        out.pop();
    }

    if out.is_empty() {
        "project".to_owned()
    } else {
        out
    }
}

pub fn derive_thread_title(text: &str) -> String {
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for ch in first_line.chars().take(48) {
        out.push(ch);
    }
    out
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

    conversation.entries.push(ConversationEntry::UserMessage {
        text: queued.text.clone(),
    });
    conversation.run_status = OperationStatus::Running;
    conversation.current_run_config = Some(queued.run_config.clone());
    conversation.in_progress_items.clear();
    conversation.in_progress_order.clear();
    Some(Effect::RunAgentTurn {
        workspace_id,
        thread_id,
        text: queued.text,
        run_config: queued.run_config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CodexCommandExecutionStatus, CodexThreadError, CodexThreadItem, CodexUsage,
        ContextTokenKind, ConversationSnapshot,
    };

    fn default_thread_id() -> WorkspaceThreadId {
        WorkspaceThreadId(1)
    }

    fn main_workspace_id(state: &AppState) -> WorkspaceId {
        let project = &state.projects[0];
        project
            .workspaces
            .iter()
            .find(|w| w.status == WorkspaceStatus::Active && w.worktree_path == project.path)
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
            .find(|w| w.status == WorkspaceStatus::Active && w.worktree_path != project.path)
            .expect("missing non-main workspace")
            .id
    }

    #[test]
    fn new_threads_use_last_selected_agent_settings() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
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

        let effects = state.apply(Action::ChatModelChanged {
            workspace_id,
            thread_id,
            model_id: "gpt-5.2-codex".to_owned(),
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

        let effects = state.apply(Action::ThinkingEffortChanged {
            workspace_id,
            thread_id,
            thinking_effort: ThinkingEffort::High,
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));

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
    fn running_turn_keeps_its_run_config_when_user_changes_defaults() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
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
        });
        assert_eq!(effects.len(), 1);
        let (sent_model_id, sent_effort) = match &effects[0] {
            Effect::RunAgentTurn { run_config, .. } => {
                (run_config.model_id.as_str(), run_config.thinking_effort)
            }
            other => panic!("unexpected effect: {other:?}"),
        };
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
    fn queued_turn_updates_current_run_config_when_started() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
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
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert_eq!(conversation.run_status, OperationStatus::Running);
        assert_eq!(conversation.pending_prompts.len(), 1);

        let effects = state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
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
    fn open_dashboard_loads_conversations_for_non_main_workspaces() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: "w1".to_owned(),
            branch_name: "repo/w1".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        });

        let main_id = main_workspace_id(&state);
        let w1 = workspace_id_by_name(&state, "w1");

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
            persisted: PersistedAppState {
                projects: Vec::new(),
                sidebar_width: None,
                terminal_pane_width: Some(480),
                agent_default_model_id: None,
                agent_default_thinking_effort: None,
                last_open_workspace_id: None,
                workspace_active_thread_id: HashMap::new(),
                workspace_open_tabs: HashMap::new(),
                workspace_archived_tabs: HashMap::new(),
                workspace_next_thread_id: HashMap::new(),
                workspace_chat_scroll_y10: HashMap::new(),
            },
        });
        assert_eq!(state.terminal_pane_width, Some(480));
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
            persisted: PersistedAppState {
                projects: Vec::new(),
                sidebar_width: Some(360),
                terminal_pane_width: None,
                agent_default_model_id: None,
                agent_default_thinking_effort: None,
                last_open_workspace_id: None,
                workspace_active_thread_id: HashMap::new(),
                workspace_open_tabs: HashMap::new(),
                workspace_archived_tabs: HashMap::new(),
                workspace_next_thread_id: HashMap::new(),
                workspace_chat_scroll_y10: HashMap::new(),
            },
        });
        assert_eq!(state.sidebar_width, Some(360));
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
        loaded.apply(Action::AppStateLoaded { persisted });
        assert_eq!(
            loaded
                .workspace_chat_scroll_y10
                .get(&(workspace_id, thread_id))
                .copied(),
            Some(-1234)
        );
    }

    #[test]
    fn workspace_thread_tabs_preserve_order_and_allow_reorder() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
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

        state.apply(Action::ReorderWorkspaceThreadTab {
            workspace_id,
            thread_id: thread_ids[3],
            to_index: 1,
        });
        let tabs = state.workspace_tabs(workspace_id).unwrap();
        assert_eq!(
            tabs.open_tabs,
            vec![thread_ids[0], thread_ids[3], thread_ids[1], thread_ids[2]]
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
        loaded.apply(Action::AppStateLoaded { persisted });
        assert!(loaded.projects[0].expanded);
    }

    #[test]
    fn in_progress_order_tracks_started_items_and_removes_on_complete() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
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
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
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
        assert_eq!(
            conversation
                .in_progress_order
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["r-1".to_owned(), "c-1".to_owned()]
        );

        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            event: CodexThreadEvent::ItemCompleted {
                item: CodexThreadItem::Reasoning {
                    id: "r-1".to_owned(),
                    text: "done".to_owned(),
                },
            },
        });

        let conversation = state
            .workspace_thread_conversation(workspace_id, thread_id)
            .expect("missing conversation");
        assert_eq!(
            conversation
                .in_progress_order
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["c-1".to_owned()]
        );
        assert!(!conversation.in_progress_items.contains_key("r-1"));
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
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SaveAppState));
    }

    #[test]
    fn main_workspace_cannot_be_archived() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
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
    fn demo_state_is_consistent() {
        let state = AppState::demo();

        assert!(!state.projects.is_empty());
    }

    #[test]
    fn project_slug_is_sanitized_and_unique() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/My Project"),
        });
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/My Project"),
        });

        assert_eq!(state.projects.len(), 2);
        assert_eq!(state.projects[0].slug, "my-project");
        assert_eq!(state.projects[1].slug, "my-project-2");
    }

    #[test]
    fn create_workspace_sets_busy_and_emits_effect() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
        });
        let project_id = state.projects[0].id;

        let effects = state.apply(Action::CreateWorkspace { project_id });
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
        let effects = loaded.apply(Action::AppStateLoaded { persisted });

        assert!(
            matches!(loaded.main_pane, MainPane::Workspace(id) if id == workspace_id),
            "expected main pane to restore workspace"
        );
        assert_eq!(loaded.right_pane, RightPane::Terminal);
        assert_eq!(effects.len(), 2);
        assert!(matches!(effects[0], Effect::LoadWorkspaceThreads { .. }));
        assert!(matches!(
            effects[1],
            Effect::LoadConversation { workspace_id: id, .. } if id == workspace_id
        ));
    }

    #[test]
    fn chat_drafts_are_isolated_and_preserved_on_reload() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
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
                thread_id: None,
                entries: Vec::new(),
            },
        });
        assert_eq!(state.workspace_conversation(w1).unwrap().draft, "draft-1");
    }

    #[test]
    fn chat_draft_edits_update_attachment_anchors_without_removing() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
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
            path: PathBuf::from("/tmp/a.png"),
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
            path: PathBuf::from("/tmp/b.txt"),
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
        });

        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            event: CodexThreadEvent::ItemStarted { item },
        });

        assert_eq!(
            state
                .workspace_conversation(workspace_id)
                .unwrap()
                .run_status,
            OperationStatus::Running
        );
        assert_eq!(
            state
                .workspace_conversation(workspace_id)
                .unwrap()
                .in_progress_items
                .len(),
            1
        );

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                thread_id: Some("thread_0".to_owned()),
                entries: Vec::new(),
            },
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Running);
        assert_eq!(conversation.in_progress_items.len(), 1);
        assert_eq!(conversation.entries.len(), 1);
        assert!(matches!(
            &conversation.entries[0],
            ConversationEntry::UserMessage { text } if text == "Hello"
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
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            event: CodexThreadEvent::TurnDuration { duration_ms: 1234 },
        });

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                thread_id: None,
                entries: vec![ConversationEntry::UserMessage {
                    text: "Hello".to_owned(),
                }],
            },
        });

        let after = &state.workspace_conversation(workspace_id).unwrap().entries;
        assert_eq!(after.len(), 2);
        assert!(matches!(
            &after[0],
            ConversationEntry::UserMessage { text } if text == "Hello"
        ));
        assert!(matches!(
            &after[1],
            ConversationEntry::TurnDuration { duration_ms: 1234 }
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
                thread_id: None,
                entries: vec![ConversationEntry::UserMessage {
                    text: "Hello".to_owned(),
                }],
            },
        });

        state.apply(Action::ConversationLoaded {
            workspace_id,
            thread_id,
            snapshot: ConversationSnapshot {
                thread_id: None,
                entries: vec![
                    ConversationEntry::UserMessage {
                        text: "Hello".to_owned(),
                    },
                    ConversationEntry::TurnDuration { duration_ms: 1234 },
                ],
            },
        });

        let after = &state.workspace_conversation(workspace_id).unwrap().entries;
        assert!(matches!(
            &after[..],
            [
                ConversationEntry::UserMessage { .. },
                ConversationEntry::TurnDuration { duration_ms: 1234 }
            ]
        ));
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
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            &effects[0],
            Effect::RunAgentTurn {
                workspace_id: wid,
                thread_id: tid,
                text,
                run_config
            } if *wid == workspace_id
                && *tid == thread_id
                && text == "Hello"
                && run_config.model_id == default_agent_model_id()
                && run_config.thinking_effort == default_thinking_effort()
        ));

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Running);
        assert_eq!(conversation.entries.len(), 1);
        assert!(matches!(
            &conversation.entries[0],
            ConversationEntry::UserMessage { text } if text == "Hello"
        ));
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
        });

        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };

        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            event: CodexThreadEvent::ItemCompleted { item: item.clone() },
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            event: CodexThreadEvent::ItemCompleted { item },
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        let completed_items = conversation
            .entries
            .iter()
            .filter(|e| matches!(e, ConversationEntry::CodexItem { .. }))
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
        });

        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };

        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            event: CodexThreadEvent::ItemCompleted { item: item.clone() },
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            event: CodexThreadEvent::TurnDuration { duration_ms: 1000 },
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            event: CodexThreadEvent::ItemCompleted { item },
        });

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        let completed_items = conversation
            .entries
            .iter()
            .filter(|e| matches!(e, ConversationEntry::CodexItem { .. }))
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
        });

        let effects = state.apply(Action::CancelAgentTurn {
            workspace_id,
            thread_id,
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::CancelAgentTurn { .. }));

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Idle);
        assert!(conversation.in_progress_items.is_empty());
        assert!(matches!(
            conversation.entries.last(),
            Some(ConversationEntry::TurnCanceled)
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
        });
        let effects = state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Second".to_owned(),
        });
        assert!(effects.is_empty());

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.entries.len(), 1);
        assert_eq!(conversation.pending_prompts.len(), 1);
        assert_eq!(conversation.pending_prompts[0].text, "Second");
        assert_eq!(conversation.run_status, OperationStatus::Running);
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
        });
        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Second".to_owned(),
        });

        let effects = state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
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
                run_config
            } if *wid == workspace_id
                && *tid == thread_id
                && text == "Second"
                && run_config.model_id == default_agent_model_id()
                && run_config.thinking_effort == default_thinking_effort()
        ));

        let conversation = state.workspace_conversation(workspace_id).unwrap();
        assert_eq!(conversation.run_status, OperationStatus::Running);
        assert!(conversation.pending_prompts.is_empty());
        assert!(matches!(
            &conversation.entries[0],
            ConversationEntry::UserMessage { text } if text == "First"
        ));
        assert!(matches!(
            &conversation.entries[1],
            ConversationEntry::UserMessage { text } if text == "Second"
        ));
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
        });
        state.apply(Action::SendAgentMessage {
            workspace_id,
            thread_id,
            text: "Second".to_owned(),
        });

        let effects = state.apply(Action::AgentEventReceived {
            workspace_id,
            thread_id,
            event: CodexThreadEvent::TurnFailed {
                error: CodexThreadError {
                    message: "boom".to_owned(),
                },
            },
        });
        assert!(effects.is_empty());

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
                run_config
            } if *wid == workspace_id
                && *tid == thread_id
                && text == "Second"
                && run_config.model_id == default_agent_model_id()
                && run_config.thinking_effort == default_thinking_effort()
        ));
    }

    #[test]
    fn open_workspace_in_ide_emits_effect_for_existing_workspace() {
        let mut state = AppState::new();
        state.apply(Action::AddProject {
            path: PathBuf::from("/tmp/repo"),
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
}
