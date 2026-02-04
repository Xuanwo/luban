use crate::time::unix_seconds;
use crate::{
    AppState, PersistedAppState, PersistedProject, PersistedWorkspace,
    PersistedWorkspaceThreadRunConfigOverride,
};
use std::collections::HashMap;

pub(crate) fn to_persisted_app_state(state: &AppState) -> PersistedAppState {
    let mut workspace_active_thread_id = HashMap::new();
    let mut workspace_open_tabs = HashMap::new();
    let mut workspace_archived_tabs = HashMap::new();
    let mut workspace_next_thread_id = HashMap::new();

    for (workspace_id, tabs) in &state.workspace_tabs {
        if tabs.open_tabs.is_empty() && tabs.archived_tabs.is_empty() {
            continue;
        }
        workspace_active_thread_id.insert(workspace_id.0, tabs.active_tab.0);
        workspace_open_tabs.insert(
            workspace_id.0,
            tabs.open_tabs.iter().map(|id| id.0).collect(),
        );
        workspace_archived_tabs.insert(
            workspace_id.0,
            tabs.archived_tabs.iter().map(|id| id.0).collect(),
        );
        workspace_next_thread_id.insert(workspace_id.0, tabs.next_thread_id);
    }

    PersistedAppState {
        projects: state
            .projects
            .iter()
            .map(|p| PersistedProject {
                id: p.id.0,
                name: p.name.clone(),
                path: p.path.clone(),
                slug: p.slug.clone(),
                is_git: p.is_git,
                expanded: p.expanded,
                workspaces: p
                    .workspaces
                    .iter()
                    .map(|w| PersistedWorkspace {
                        id: w.id.0,
                        workspace_name: w.workspace_name.clone(),
                        branch_name: w.branch_name.clone(),
                        worktree_path: w.worktree_path.clone(),
                        status: w.status,
                        last_activity_at_unix_seconds: w.last_activity_at.and_then(unix_seconds),
                    })
                    .collect(),
            })
            .collect(),
        sidebar_width: state.sidebar_width,
        terminal_pane_width: state.terminal_pane_width,
        global_zoom_percent: Some(state.global_zoom_percent),
        appearance_theme: Some(state.appearance_theme.as_str().to_owned()),
        appearance_ui_font: Some(state.appearance_fonts.ui_font.clone()),
        appearance_chat_font: Some(state.appearance_fonts.chat_font.clone()),
        appearance_code_font: Some(state.appearance_fonts.code_font.clone()),
        appearance_terminal_font: Some(state.appearance_fonts.terminal_font.clone()),
        agent_default_model_id: Some(state.agent_default_model_id.clone()),
        agent_default_thinking_effort: Some(
            state.agent_default_thinking_effort.as_str().to_owned(),
        ),
        agent_default_runner: Some(state.agent_default_runner.as_str().to_owned()),
        agent_amp_mode: Some(state.agent_amp_mode.clone()),
        agent_codex_enabled: Some(state.agent_codex_enabled),
        agent_amp_enabled: Some(state.agent_amp_enabled),
        agent_claude_enabled: Some(state.agent_claude_enabled),
        last_open_workspace_id: state.last_open_workspace_id.map(|id| id.0),
        open_button_selection: state.open_button_selection.clone(),
        sidebar_project_order: state.sidebar_project_order.clone(),
        workspace_active_thread_id,
        workspace_open_tabs,
        workspace_archived_tabs,
        workspace_next_thread_id,
        workspace_chat_scroll_y10: state
            .workspace_chat_scroll_y10
            .iter()
            .map(|((workspace_id, thread_id), offset_y10)| {
                ((workspace_id.0, thread_id.0), *offset_y10)
            })
            .collect(),
        workspace_chat_scroll_anchor: state
            .workspace_chat_scroll_anchor
            .iter()
            .map(|((workspace_id, thread_id), anchor)| {
                ((workspace_id.0, thread_id.0), anchor.clone())
            })
            .collect(),
        workspace_unread_completions: state
            .workspace_unread_completions
            .iter()
            .map(|workspace_id| (workspace_id.0, true))
            .collect::<HashMap<_, _>>(),
        workspace_thread_run_config_overrides: state
            .workspace_thread_run_config_overrides
            .iter()
            .map(|((workspace_id, thread_id), run_config)| {
                (
                    (workspace_id.0, thread_id.0),
                    PersistedWorkspaceThreadRunConfigOverride {
                        runner: run_config.runner.clone(),
                        amp_mode: run_config.amp_mode.clone(),
                        model_id: run_config.model_id.clone(),
                        thinking_effort: run_config.thinking_effort.clone(),
                    },
                )
            })
            .collect(),
        starred_tasks: state
            .starred_tasks
            .iter()
            .map(|(workspace_id, thread_id)| ((workspace_id.0, thread_id.0), true))
            .collect(),
        task_prompt_templates: HashMap::new(),
        telegram_enabled: Some(state.telegram_enabled),
        telegram_bot_token: state.telegram_bot_token.clone(),
        telegram_bot_username: state.telegram_bot_username.clone(),
        telegram_paired_chat_id: state.telegram_paired_chat_id,
        telegram_topic_bindings: serialize_telegram_topic_bindings(&state.telegram_topic_bindings),
    }
}

fn serialize_telegram_topic_bindings(
    bindings: &HashMap<i64, crate::TelegramTopicBinding>,
) -> Option<String> {
    if bindings.is_empty() {
        return None;
    }

    let mut list = bindings.values().cloned().collect::<Vec<_>>();
    list.sort_by_key(|b| b.message_thread_id);
    serde_json::to_string(&list).ok()
}
