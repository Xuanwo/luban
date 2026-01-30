use super::{
    AppearanceFonts, AppearanceTheme, ChatScrollAnchor, MainPane, OperationStatus,
    PersistedWorkspaceThreadRunConfigOverride, ProjectId, RightPane, WorkspaceConversation,
    WorkspaceId, WorkspaceStatus, WorkspaceTabs, WorkspaceThreadId,
};
use crate::{SystemTaskKind, TaskIntentKind};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Clone, Debug)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub workspace_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub status: WorkspaceStatus,
    pub last_activity_at: Option<std::time::SystemTime>,
    pub archive_status: OperationStatus,
    pub branch_rename_status: OperationStatus,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    pub slug: String,
    pub is_git: bool,
    pub expanded: bool,
    pub create_workspace_status: OperationStatus,
    pub workspaces: Vec<Workspace>,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub(crate) next_project_id: u64,
    pub(crate) next_workspace_id: u64,

    pub projects: Vec<Project>,
    pub main_pane: MainPane,
    pub right_pane: RightPane,
    pub sidebar_width: Option<u16>,
    pub terminal_pane_width: Option<u16>,
    pub global_zoom_percent: u16,
    pub appearance_theme: AppearanceTheme,
    pub appearance_fonts: AppearanceFonts,
    pub(crate) agent_default_model_id: String,
    pub(crate) agent_default_thinking_effort: crate::ThinkingEffort,
    pub(crate) agent_default_runner: crate::AgentRunnerKind,
    pub(crate) agent_amp_mode: String,
    pub(crate) agent_codex_enabled: bool,
    pub(crate) agent_amp_enabled: bool,
    pub(crate) agent_claude_enabled: bool,
    pub conversations: HashMap<(WorkspaceId, WorkspaceThreadId), WorkspaceConversation>,
    pub workspace_tabs: HashMap<WorkspaceId, WorkspaceTabs>,
    pub dashboard_preview_workspace_id: Option<WorkspaceId>,
    pub last_open_workspace_id: Option<WorkspaceId>,
    pub open_button_selection: Option<String>,
    pub sidebar_project_order: Vec<String>,
    pub last_error: Option<String>,
    pub workspace_chat_scroll_y10: HashMap<(WorkspaceId, WorkspaceThreadId), i32>,
    pub workspace_chat_scroll_anchor: HashMap<(WorkspaceId, WorkspaceThreadId), ChatScrollAnchor>,
    pub workspace_unread_completions: HashSet<WorkspaceId>,
    pub workspace_thread_run_config_overrides:
        HashMap<(WorkspaceId, WorkspaceThreadId), PersistedWorkspaceThreadRunConfigOverride>,
    pub task_prompt_templates: HashMap<TaskIntentKind, String>,
    pub system_prompt_templates: HashMap<SystemTaskKind, String>,
}

impl AppState {
    pub fn agent_codex_enabled(&self) -> bool {
        self.agent_codex_enabled
    }

    pub fn agent_amp_enabled(&self) -> bool {
        self.agent_amp_enabled
    }

    pub fn agent_claude_enabled(&self) -> bool {
        self.agent_claude_enabled
    }

    pub fn agent_default_model_id(&self) -> &str {
        &self.agent_default_model_id
    }

    pub fn agent_default_thinking_effort(&self) -> crate::ThinkingEffort {
        self.agent_default_thinking_effort
    }

    pub fn agent_default_runner(&self) -> crate::AgentRunnerKind {
        self.agent_default_runner
    }

    pub fn agent_amp_mode(&self) -> &str {
        &self.agent_amp_mode
    }
}
