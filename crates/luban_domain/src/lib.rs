use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ProjectId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WorkspaceId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainPane {
    None,
    ProjectSettings(ProjectId),
    Workspace(WorkspaceId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceStatus {
    Active,
    Archived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationStatus {
    Idle,
    Running,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexCommandExecutionStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexPatchChangeKind {
    Add,
    Delete,
    Update,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexFileUpdateChange {
    pub path: String,
    pub kind: CodexPatchChangeKind,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexPatchApplyStatus {
    Completed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexMcpToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexErrorMessage {
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexTodoItem {
    pub text: String,
    pub completed: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum CodexThreadItem {
    #[serde(rename = "agent_message")]
    AgentMessage { id: String, text: String },
    #[serde(rename = "reasoning")]
    Reasoning { id: String, text: String },
    #[serde(rename = "command_execution")]
    CommandExecution {
        id: String,
        command: String,
        aggregated_output: String,
        exit_code: Option<i32>,
        status: CodexCommandExecutionStatus,
    },
    #[serde(rename = "file_change")]
    FileChange {
        id: String,
        changes: Vec<CodexFileUpdateChange>,
        status: CodexPatchApplyStatus,
    },
    #[serde(rename = "mcp_tool_call")]
    McpToolCall {
        id: String,
        server: String,
        tool: String,
        arguments: serde_json::Value,
        result: Option<serde_json::Value>,
        error: Option<CodexErrorMessage>,
        status: CodexMcpToolCallStatus,
    },
    #[serde(rename = "web_search")]
    WebSearch { id: String, query: String },
    #[serde(rename = "todo_list")]
    TodoList {
        id: String,
        items: Vec<CodexTodoItem>,
    },
    #[serde(rename = "error")]
    Error { id: String, message: String },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexThreadError {
    pub message: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum CodexThreadEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted { thread_id: String },
    #[serde(rename = "turn.started")]
    TurnStarted,
    #[serde(rename = "turn.completed")]
    TurnCompleted { usage: CodexUsage },
    #[serde(rename = "turn.duration")]
    TurnDuration { duration_ms: u64 },
    #[serde(rename = "turn.failed")]
    TurnFailed { error: CodexThreadError },

    #[serde(rename = "item.started")]
    ItemStarted { item: CodexThreadItem },
    #[serde(rename = "item.updated")]
    ItemUpdated { item: CodexThreadItem },
    #[serde(rename = "item.completed")]
    ItemCompleted { item: CodexThreadItem },

    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationEntry {
    UserMessage { text: String },
    CodexItem { item: Box<CodexThreadItem> },
    TurnUsage { usage: Option<CodexUsage> },
    TurnDuration { duration_ms: u64 },
    TurnCanceled,
    TurnError { message: String },
}

fn entry_is_same_codex_item(entry: &ConversationEntry, item: &CodexThreadItem) -> bool {
    match entry {
        ConversationEntry::CodexItem { item: existing } => {
            codex_item_id(existing) == codex_item_id(item)
        }
        _ => false,
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConversationSnapshot {
    pub thread_id: Option<String>,
    pub entries: Vec<ConversationEntry>,
}

#[derive(Clone, Debug)]
pub struct WorkspaceConversation {
    pub thread_id: Option<String>,
    pub entries: Vec<ConversationEntry>,
    pub run_status: OperationStatus,
    pub in_progress_items: BTreeMap<String, CodexThreadItem>,
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub workspace_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub status: WorkspaceStatus,
    pub last_activity_at: Option<std::time::SystemTime>,
    pub archive_status: OperationStatus,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    pub slug: String,
    pub expanded: bool,
    pub create_workspace_status: OperationStatus,
    pub workspaces: Vec<Workspace>,
}

#[derive(Clone, Debug)]
pub struct AppState {
    next_project_id: u64,
    next_workspace_id: u64,

    pub projects: Vec<Project>,
    pub main_pane: MainPane,
    pub conversations: HashMap<WorkspaceId, WorkspaceConversation>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Action {
    AddProject {
        path: PathBuf,
    },
    ToggleProjectExpanded {
        project_id: ProjectId,
    },
    OpenProjectSettings {
        project_id: ProjectId,
    },

    CreateWorkspace {
        project_id: ProjectId,
    },
    WorkspaceCreated {
        project_id: ProjectId,
        workspace_name: String,
        branch_name: String,
        worktree_path: PathBuf,
    },
    WorkspaceCreateFailed {
        project_id: ProjectId,
        message: String,
    },

    OpenWorkspace {
        workspace_id: WorkspaceId,
    },
    ArchiveWorkspace {
        workspace_id: WorkspaceId,
    },
    WorkspaceArchived {
        workspace_id: WorkspaceId,
    },
    WorkspaceArchiveFailed {
        workspace_id: WorkspaceId,
        message: String,
    },

    ConversationLoaded {
        workspace_id: WorkspaceId,
        snapshot: ConversationSnapshot,
    },
    ConversationLoadFailed {
        workspace_id: WorkspaceId,
        message: String,
    },
    SendAgentMessage {
        workspace_id: WorkspaceId,
        text: String,
    },
    AgentEventReceived {
        workspace_id: WorkspaceId,
        event: CodexThreadEvent,
    },
    AgentTurnFinished {
        workspace_id: WorkspaceId,
    },
    CancelAgentTurn {
        workspace_id: WorkspaceId,
    },

    ClearError,
}

#[derive(Clone, Debug)]
pub enum Effect {
    CreateWorkspace {
        project_id: ProjectId,
    },
    ArchiveWorkspace {
        workspace_id: WorkspaceId,
    },
    EnsureConversation {
        workspace_id: WorkspaceId,
    },
    LoadConversation {
        workspace_id: WorkspaceId,
    },
    RunAgentTurn {
        workspace_id: WorkspaceId,
        text: String,
    },
    CancelAgentTurn {
        workspace_id: WorkspaceId,
    },
}

impl AppState {
    pub fn new() -> Self {
        Self {
            next_project_id: 1,
            next_workspace_id: 1,
            projects: Vec::new(),
            main_pane: MainPane::None,
            conversations: HashMap::new(),
            last_error: None,
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
            Action::AddProject { path } => {
                self.add_project(path);
                Vec::new()
            }
            Action::ToggleProjectExpanded { project_id } => {
                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    project.expanded = !project.expanded;
                }
                Vec::new()
            }
            Action::OpenProjectSettings { project_id } => {
                self.main_pane = MainPane::ProjectSettings(project_id);
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
                self.conversations.insert(
                    workspace_id,
                    WorkspaceConversation {
                        thread_id: None,
                        entries: Vec::new(),
                        run_status: OperationStatus::Idle,
                        in_progress_items: BTreeMap::new(),
                    },
                );
                vec![Effect::EnsureConversation { workspace_id }]
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
                vec![Effect::LoadConversation { workspace_id }]
            }
            Action::ArchiveWorkspace { workspace_id } => {
                if let Some((project_idx, workspace_idx)) =
                    self.find_workspace_indices(workspace_id)
                {
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
                if matches!(self.main_pane, MainPane::Workspace(id) if id == workspace_id) {
                    self.main_pane = MainPane::None;
                }
                Vec::new()
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
                snapshot,
            } => {
                self.conversations.insert(
                    workspace_id,
                    WorkspaceConversation {
                        thread_id: snapshot.thread_id,
                        entries: snapshot.entries,
                        run_status: OperationStatus::Idle,
                        in_progress_items: BTreeMap::new(),
                    },
                );
                Vec::new()
            }
            Action::ConversationLoadFailed {
                workspace_id: _,
                message,
            } => {
                self.last_error = Some(message);
                Vec::new()
            }
            Action::SendAgentMessage { workspace_id, text } => {
                let conversation = self.conversations.entry(workspace_id).or_insert_with(|| {
                    WorkspaceConversation {
                        thread_id: None,
                        entries: Vec::new(),
                        run_status: OperationStatus::Idle,
                        in_progress_items: BTreeMap::new(),
                    }
                });

                if conversation.run_status == OperationStatus::Running {
                    return Vec::new();
                }

                conversation
                    .entries
                    .push(ConversationEntry::UserMessage { text: text.clone() });
                conversation.run_status = OperationStatus::Running;
                conversation.in_progress_items.clear();
                vec![Effect::RunAgentTurn { workspace_id, text }]
            }
            Action::AgentEventReceived {
                workspace_id,
                event,
            } => {
                let conversation = self.conversations.entry(workspace_id).or_insert_with(|| {
                    WorkspaceConversation {
                        thread_id: None,
                        entries: Vec::new(),
                        run_status: OperationStatus::Idle,
                        in_progress_items: BTreeMap::new(),
                    }
                });

                match event {
                    CodexThreadEvent::ThreadStarted { thread_id } => {
                        conversation.thread_id = Some(thread_id);
                    }
                    CodexThreadEvent::TurnStarted => {}
                    CodexThreadEvent::TurnCompleted { usage } => {
                        let _ = usage;
                        conversation.run_status = OperationStatus::Idle;
                        conversation.in_progress_items.clear();
                    }
                    CodexThreadEvent::TurnDuration { duration_ms } => {
                        conversation
                            .entries
                            .push(ConversationEntry::TurnDuration { duration_ms });
                    }
                    CodexThreadEvent::TurnFailed { error } => {
                        conversation.entries.push(ConversationEntry::TurnError {
                            message: error.message.clone(),
                        });
                        conversation.run_status = OperationStatus::Idle;
                        conversation.in_progress_items.clear();
                        self.last_error = Some(error.message);
                    }
                    CodexThreadEvent::ItemStarted { item }
                    | CodexThreadEvent::ItemUpdated { item } => {
                        conversation
                            .in_progress_items
                            .insert(codex_item_id(&item).to_owned(), item);
                    }
                    CodexThreadEvent::ItemCompleted { item } => {
                        conversation.in_progress_items.remove(codex_item_id(&item));
                        let is_duplicate = conversation
                            .entries
                            .last()
                            .is_some_and(|e| entry_is_same_codex_item(e, &item));
                        if !is_duplicate {
                            conversation.entries.push(ConversationEntry::CodexItem {
                                item: Box::new(item),
                            });
                        }
                    }
                    CodexThreadEvent::Error { message } => {
                        conversation.entries.push(ConversationEntry::TurnError {
                            message: message.clone(),
                        });
                        conversation.run_status = OperationStatus::Idle;
                        conversation.in_progress_items.clear();
                        self.last_error = Some(message);
                    }
                }

                Vec::new()
            }
            Action::AgentTurnFinished { workspace_id } => {
                if let Some(conversation) = self.conversations.get_mut(&workspace_id)
                    && conversation.run_status == OperationStatus::Running
                {
                    conversation.run_status = OperationStatus::Idle;
                    conversation.in_progress_items.clear();
                }
                Vec::new()
            }
            Action::CancelAgentTurn { workspace_id } => {
                let Some(conversation) = self.conversations.get_mut(&workspace_id) else {
                    return Vec::new();
                };
                if conversation.run_status != OperationStatus::Running {
                    return Vec::new();
                }
                conversation.run_status = OperationStatus::Idle;
                conversation.in_progress_items.clear();
                conversation.entries.push(ConversationEntry::TurnCanceled);
                vec![Effect::CancelAgentTurn { workspace_id }]
            }

            Action::ClearError => {
                self.last_error = None;
                Vec::new()
            }
        }
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
        self.conversations.get(&workspace_id)
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let workspace_id = state.projects[0].workspaces[0].id;

        let effects = state.apply(Action::OpenWorkspace { workspace_id });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::LoadConversation { .. }));
    }

    #[test]
    fn send_agent_message_sets_running_and_emits_effect() {
        let mut state = AppState::demo();
        let workspace_id = state.projects[0].workspaces[0].id;

        let effects = state.apply(Action::SendAgentMessage {
            workspace_id,
            text: "Hello".to_owned(),
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::RunAgentTurn { .. }));

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
        let workspace_id = state.projects[0].workspaces[0].id;

        state.apply(Action::SendAgentMessage {
            workspace_id,
            text: "Hello".to_owned(),
        });

        let item = CodexThreadItem::AgentMessage {
            id: "item_0".to_owned(),
            text: "Hi".to_owned(),
        };

        state.apply(Action::AgentEventReceived {
            workspace_id,
            event: CodexThreadEvent::ItemCompleted { item: item.clone() },
        });
        state.apply(Action::AgentEventReceived {
            workspace_id,
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
        let workspace_id = state.projects[0].workspaces[0].id;

        state.apply(Action::SendAgentMessage {
            workspace_id,
            text: "Hello".to_owned(),
        });

        let effects = state.apply(Action::CancelAgentTurn { workspace_id });
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
}
