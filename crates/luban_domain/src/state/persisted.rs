use super::{ChatScrollAnchor, WorkspaceStatus};
use std::{collections::HashMap, path::PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedAppState {
    pub projects: Vec<PersistedProject>,
    pub sidebar_width: Option<u16>,
    pub terminal_pane_width: Option<u16>,
    pub global_zoom_percent: Option<u16>,
    pub appearance_theme: Option<String>,
    pub appearance_ui_font: Option<String>,
    pub appearance_chat_font: Option<String>,
    pub appearance_code_font: Option<String>,
    pub appearance_terminal_font: Option<String>,
    pub agent_default_model_id: Option<String>,
    pub agent_default_thinking_effort: Option<String>,
    pub agent_default_runner: Option<String>,
    pub agent_amp_mode: Option<String>,
    pub agent_codex_enabled: Option<bool>,
    pub agent_amp_enabled: Option<bool>,
    pub last_open_workspace_id: Option<u64>,
    pub open_button_selection: Option<String>,
    pub sidebar_project_order: Vec<String>,
    pub sidebar_worktree_order: HashMap<String, Vec<u64>>,
    pub workspace_active_thread_id: HashMap<u64, u64>,
    pub workspace_open_tabs: HashMap<u64, Vec<u64>>,
    pub workspace_archived_tabs: HashMap<u64, Vec<u64>>,
    pub workspace_next_thread_id: HashMap<u64, u64>,
    pub workspace_chat_scroll_y10: HashMap<(u64, u64), i32>,
    pub workspace_chat_scroll_anchor: HashMap<(u64, u64), ChatScrollAnchor>,
    pub workspace_unread_completions: HashMap<u64, bool>,
    pub task_prompt_templates: HashMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedProject {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    pub slug: String,
    pub is_git: bool,
    pub expanded: bool,
    pub workspaces: Vec<PersistedWorkspace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedWorkspace {
    pub id: u64,
    pub workspace_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub status: WorkspaceStatus,
    pub last_activity_at_unix_seconds: Option<u64>,
}
