use super::fonts::normalize_font;
use super::strings::normalize_optional_string;
use crate::agent_settings::{parse_agent_runner_kind, parse_thinking_effort};
use crate::time::system_time_from_unix_seconds;
use crate::{
    AppState, AppearanceFonts, AppearanceTheme, Effect, MainPane, OperationStatus,
    PersistedAppState, PersistedProject, Project, ProjectId, RightPane, TaskIntentKind, Workspace,
    WorkspaceId, WorkspaceStatus, WorkspaceTabs, WorkspaceThreadId, default_agent_model_id,
    default_agent_runner_kind, default_amp_mode, default_system_prompt_templates,
    default_task_prompt_templates, default_thinking_effort, normalize_thinking_effort,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn normalize_thread_tabs(
    active: WorkspaceThreadId,
    mut open_tabs: Vec<WorkspaceThreadId>,
    mut archived_tabs: Vec<WorkspaceThreadId>,
) -> (Vec<WorkspaceThreadId>, Vec<WorkspaceThreadId>) {
    let mut open_set: HashSet<WorkspaceThreadId> = open_tabs.iter().copied().collect();
    archived_tabs.retain(|id| !open_set.contains(id));
    if open_tabs.is_empty() {
        open_tabs.push(active);
        open_set.insert(active);
    }
    if !open_set.contains(&active) {
        open_tabs.push(active);
        open_set.insert(active);
    }
    archived_tabs.retain(|id| *id != active);
    (open_tabs, archived_tabs)
}

pub(crate) fn apply_persisted_app_state(
    state: &mut AppState,
    persisted: PersistedAppState,
) -> Vec<Effect> {
    if !state.projects.is_empty() {
        return Vec::new();
    }

    let legacy_templates: HashMap<TaskIntentKind, String> = persisted
        .task_prompt_templates
        .iter()
        .filter_map(|(key, template)| {
            let kind = TaskIntentKind::parse_key(key);
            let trimmed = template.trim();
            if trimmed.is_empty() {
                return None;
            }
            Some((kind, trimmed.to_owned()))
        })
        .collect();
    let clear_legacy_templates = !persisted.task_prompt_templates.is_empty();

    let agent_default_model_id = persisted
        .agent_default_model_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| default_agent_model_id().to_owned());
    let agent_default_thinking_effort = persisted
        .agent_default_thinking_effort
        .as_deref()
        .and_then(parse_thinking_effort)
        .unwrap_or_else(default_thinking_effort);
    let agent_default_thinking_effort =
        normalize_thinking_effort(&agent_default_model_id, agent_default_thinking_effort);

    let agent_default_runner = persisted
        .agent_default_runner
        .as_deref()
        .and_then(parse_agent_runner_kind)
        .unwrap_or_else(default_agent_runner_kind);

    let agent_amp_mode = normalize_optional_string(persisted.agent_amp_mode.as_deref(), 32)
        .unwrap_or_else(|| default_amp_mode().to_owned());

    state.agent_default_model_id = agent_default_model_id;
    state.agent_default_thinking_effort = agent_default_thinking_effort;
    state.agent_default_runner = agent_default_runner;
    state.agent_amp_mode = agent_amp_mode;
    state.agent_codex_enabled = persisted.agent_codex_enabled.unwrap_or(true);
    state.agent_amp_enabled = persisted.agent_amp_enabled.unwrap_or(true);
    state.agent_claude_enabled = persisted.agent_claude_enabled.unwrap_or(true);

    state.task_prompt_templates = default_task_prompt_templates();
    state.system_prompt_templates = default_system_prompt_templates();

    let (projects, projects_upgraded) = load_projects(persisted.projects);
    state.projects = projects;
    state.sidebar_width = persisted.sidebar_width;
    state.terminal_pane_width = persisted.terminal_pane_width;
    state.global_zoom_percent = persisted.global_zoom_percent.unwrap_or(100);
    state.appearance_theme = persisted
        .appearance_theme
        .as_deref()
        .and_then(AppearanceTheme::parse)
        .unwrap_or_default();
    let defaults = AppearanceFonts::default();
    state.appearance_fonts = AppearanceFonts {
        ui_font: normalize_font(persisted.appearance_ui_font.as_deref(), &defaults.ui_font),
        chat_font: normalize_font(
            persisted.appearance_chat_font.as_deref(),
            &defaults.chat_font,
        ),
        code_font: normalize_font(
            persisted.appearance_code_font.as_deref(),
            &defaults.code_font,
        ),
        terminal_font: normalize_font(
            persisted.appearance_terminal_font.as_deref(),
            &defaults.terminal_font,
        ),
    };
    state.last_open_workspace_id = persisted.last_open_workspace_id.map(WorkspaceId);
    state.open_button_selection = persisted
        .open_button_selection
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| s.len() <= 1024)
        .map(ToOwned::to_owned);
    let valid_project_ids: HashSet<String> = state
        .projects
        .iter()
        .map(|p| p.path.to_string_lossy().to_string())
        .collect();
    state.sidebar_project_order = {
        let mut out = Vec::with_capacity(persisted.sidebar_project_order.len().min(1024));
        let mut seen = HashSet::<String>::new();
        for raw in persisted.sidebar_project_order {
            if out.len() >= 1024 {
                break;
            }
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.len() > 4096 {
                continue;
            }
            if !valid_project_ids.contains(trimmed) {
                continue;
            }
            let key = trimmed.to_owned();
            if !seen.insert(key.clone()) {
                continue;
            }
            out.push(key);
        }
        out
    };
    state.sidebar_worktree_order = {
        let mut out = HashMap::new();
        for (project_id, workspace_ids) in persisted.sidebar_worktree_order {
            if out.len() >= 1024 {
                break;
            }
            let project_id = project_id.trim();
            if project_id.is_empty() || project_id.len() > 4096 {
                continue;
            }
            if !valid_project_ids.contains(project_id) {
                continue;
            }
            let Some(project) = state
                .projects
                .iter()
                .find(|p| p.path.to_string_lossy().as_ref() == project_id)
            else {
                continue;
            };
            let valid: HashSet<u64> = project.workspaces.iter().map(|w| w.id.0).collect();
            let mut seen = HashSet::<u64>::new();
            let mut ids = Vec::with_capacity(workspace_ids.len().min(4096));
            for raw in workspace_ids {
                if ids.len() >= 4096 {
                    break;
                }
                if !valid.contains(&raw) {
                    continue;
                }
                if !seen.insert(raw) {
                    continue;
                }
                ids.push(WorkspaceId(raw));
            }
            if ids.is_empty() {
                continue;
            }
            out.insert(project_id.to_owned(), ids);
        }
        out
    };
    state.workspace_tabs = HashMap::new();
    state.conversations = HashMap::new();
    state.workspace_unread_completions = persisted
        .workspace_unread_completions
        .into_iter()
        .filter_map(|(workspace_id, unread)| unread.then_some(WorkspaceId(workspace_id)))
        .collect::<HashSet<_>>();
    let valid_workspace_ids: HashSet<WorkspaceId> = state
        .projects
        .iter()
        .flat_map(|p| &p.workspaces)
        .map(|w| w.id)
        .collect();
    state.workspace_thread_run_config_overrides = persisted
        .workspace_thread_run_config_overrides
        .into_iter()
        .filter_map(|((workspace_id, thread_id), run_config)| {
            if !valid_workspace_ids.contains(&WorkspaceId(workspace_id)) {
                return None;
            }
            let model_id = run_config.model_id.trim();
            if model_id.is_empty() || model_id.len() > 256 {
                return None;
            }
            let runner = run_config
                .runner
                .as_deref()
                .and_then(crate::agent_settings::parse_agent_runner_kind)
                .map(|r| r.as_str().to_owned());
            let amp_mode = run_config
                .amp_mode
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .filter(|v| v.len() <= 128)
                .map(ToOwned::to_owned);
            let parsed_effort = parse_thinking_effort(&run_config.thinking_effort)?;
            let normalized = normalize_thinking_effort(model_id, parsed_effort);
            Some((
                (WorkspaceId(workspace_id), WorkspaceThreadId(thread_id)),
                crate::PersistedWorkspaceThreadRunConfigOverride {
                    runner,
                    amp_mode,
                    model_id: model_id.to_owned(),
                    thinking_effort: normalized.as_str().to_owned(),
                },
            ))
        })
        .collect();

    for workspace in state.projects.iter().flat_map(|p| &p.workspaces) {
        let workspace_id = workspace.id;
        let active = persisted
            .workspace_active_thread_id
            .get(&workspace_id.0)
            .copied()
            .map(WorkspaceThreadId)
            .unwrap_or(WorkspaceThreadId(1));

        let open_tabs = persisted
            .workspace_open_tabs
            .get(&workspace_id.0)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(WorkspaceThreadId)
            .collect::<Vec<_>>();
        let archived_tabs = persisted
            .workspace_archived_tabs
            .get(&workspace_id.0)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(WorkspaceThreadId)
            .collect::<Vec<_>>();
        let (open_tabs, archived_tabs) = normalize_thread_tabs(active, open_tabs, archived_tabs);

        let next_thread_id = persisted
            .workspace_next_thread_id
            .get(&workspace_id.0)
            .copied()
            .unwrap_or(active.0 + 1);

        for thread_id in open_tabs.iter().copied() {
            let mut conversation = state.default_conversation(thread_id);
            if let Some(run_config) = state
                .workspace_thread_run_config_overrides
                .get(&(workspace_id, thread_id))
            {
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
                if let Some(parsed_effort) = parse_thinking_effort(&run_config.thinking_effort) {
                    let normalized = normalize_thinking_effort(&run_config.model_id, parsed_effort);
                    conversation.run_config_overridden_by_user = true;
                    conversation.agent_model_id = run_config.model_id.clone();
                    conversation.thinking_effort = normalized;
                    overridden = true;
                }
                if overridden
                    && conversation.agent_runner == crate::AgentRunnerKind::Amp
                    && conversation.amp_mode.is_none()
                {
                    conversation.amp_mode = Some(state.agent_amp_mode.clone());
                }
            }
            state
                .conversations
                .insert((workspace_id, thread_id), conversation);
        }

        state.workspace_tabs.insert(
            workspace_id,
            WorkspaceTabs {
                open_tabs,
                archived_tabs,
                active_tab: active,
                next_thread_id,
            },
        );
    }

    state.workspace_chat_scroll_y10 = persisted
        .workspace_chat_scroll_y10
        .into_iter()
        .map(|((workspace_id, thread_id), offset)| {
            (
                (WorkspaceId(workspace_id), WorkspaceThreadId(thread_id)),
                offset,
            )
        })
        .collect();

    state.workspace_chat_scroll_anchor = persisted
        .workspace_chat_scroll_anchor
        .into_iter()
        .map(|((workspace_id, thread_id), anchor)| {
            (
                (WorkspaceId(workspace_id), WorkspaceThreadId(thread_id)),
                anchor,
            )
        })
        .collect();

    let max_project_id = state.projects.iter().map(|p| p.id.0).max().unwrap_or(0);
    let max_workspace_id = state
        .projects
        .iter()
        .flat_map(|p| &p.workspaces)
        .map(|w| w.id.0)
        .max()
        .unwrap_or(0);

    state.next_project_id = max_project_id + 1;
    state.next_workspace_id = max_workspace_id + 1;
    state.main_pane = MainPane::None;
    state.right_pane = RightPane::None;
    state.dashboard_preview_workspace_id = None;

    let mut effects = Vec::new();
    if !legacy_templates.is_empty() {
        effects.push(Effect::MigrateLegacyTaskPromptTemplates {
            templates: legacy_templates,
        });
    }
    effects.push(Effect::LoadCodexDefaults);
    effects.push(Effect::LoadTaskPromptTemplates);
    effects.push(Effect::LoadSystemPromptTemplates);
    if projects_upgraded || clear_legacy_templates {
        effects.push(Effect::SaveAppState);
    }

    let restored_workspace_id = state.last_open_workspace_id.and_then(|workspace_id| {
        state
            .workspace(workspace_id)
            .filter(|w| w.status == WorkspaceStatus::Active)
            .map(|_| workspace_id)
    });

    if let Some(workspace_id) = restored_workspace_id {
        state.main_pane = MainPane::Workspace(workspace_id);
        state.right_pane = RightPane::Terminal;
        let thread_id = state
            .workspace_tabs
            .get(&workspace_id)
            .map(|tabs| tabs.active_tab)
            .unwrap_or(WorkspaceThreadId(1));
        effects.push(Effect::LoadWorkspaceThreads { workspace_id });
        effects.push(Effect::LoadConversation {
            workspace_id,
            thread_id,
        });
    }

    effects
}

fn load_projects(projects: Vec<PersistedProject>) -> (Vec<Project>, bool) {
    use std::collections::hash_map::Entry;

    let mut upgraded = false;
    let mut grouped: HashMap<PathBuf, Vec<Project>> = HashMap::new();

    for persisted in projects {
        let normalized_path = crate::paths::normalize_project_path(&persisted.path);
        if normalized_path != persisted.path {
            upgraded = true;
        }

        let project = Project {
            id: ProjectId(persisted.id),
            name: persisted.name,
            path: normalized_path.clone(),
            slug: persisted.slug,
            is_git: persisted.is_git,
            expanded: persisted.expanded,
            create_workspace_status: OperationStatus::Idle,
            workspaces: persisted
                .workspaces
                .into_iter()
                .map(|w| Workspace {
                    id: WorkspaceId(w.id),
                    workspace_name: w.workspace_name,
                    branch_name: w.branch_name,
                    worktree_path: w.worktree_path,
                    status: w.status,
                    last_activity_at: w
                        .last_activity_at_unix_seconds
                        .map(system_time_from_unix_seconds),
                    archive_status: OperationStatus::Idle,
                    branch_rename_status: OperationStatus::Idle,
                })
                .collect(),
        };

        match grouped.entry(normalized_path) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().push(project);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![project]);
            }
        }
    }

    let mut merged = Vec::new();
    for (_path, mut group) in grouped {
        group.sort_by_key(|p| p.id.0);
        if group.len() > 1 {
            upgraded = true;
        }

        let mut canonical = group.remove(0);

        for other in group {
            canonical.expanded |= other.expanded;
            canonical.workspaces.extend(other.workspaces);
        }

        if dedupe_worktree_paths(&mut canonical.workspaces) {
            upgraded = true;
        }

        if dedupe_workspace_names(&mut canonical.workspaces) {
            upgraded = true;
        }

        merged.push(canonical);
    }

    merged.sort_by_key(|p| p.id.0);
    (merged, upgraded)
}

fn dedupe_workspace_names(workspaces: &mut [Workspace]) -> bool {
    let mut upgraded = false;
    let mut used: HashSet<String> = HashSet::new();
    let mut next_suffix: HashMap<String, u64> = HashMap::new();

    for workspace in workspaces.iter_mut() {
        if used.insert(workspace.workspace_name.clone()) {
            continue;
        }

        let base = workspace.workspace_name.clone();
        let counter = next_suffix.entry(base.clone()).or_insert(2);
        for i in *counter.. {
            let candidate = format!("{base}-{i}");
            *counter = i + 1;
            if used.insert(candidate.clone()) {
                workspace.workspace_name = candidate;
                upgraded = true;
                break;
            }
        }
    }

    upgraded
}

fn dedupe_worktree_paths(workspaces: &mut Vec<Workspace>) -> bool {
    if workspaces.len() <= 1 {
        return false;
    }

    let mut upgraded = false;
    let mut grouped: HashMap<PathBuf, Vec<Workspace>> = HashMap::new();

    for workspace in workspaces.drain(..) {
        grouped
            .entry(workspace.worktree_path.clone())
            .or_default()
            .push(workspace);
    }

    let mut merged = Vec::new();
    for (_path, mut group) in grouped {
        group.sort_by_key(|w| {
            let is_main = w.workspace_name == "main";
            let is_active = w.status == WorkspaceStatus::Active;
            (
                std::cmp::Reverse(is_main),
                std::cmp::Reverse(is_active),
                w.id.0,
            )
        });

        let mut canonical = group.remove(0);
        for other in group {
            upgraded = true;
            canonical.last_activity_at = match (canonical.last_activity_at, other.last_activity_at)
            {
                (Some(a), Some(b)) => Some(std::cmp::max(a, b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
        }
        merged.push(canonical);
    }

    merged.sort_by_key(|w| w.id.0);
    *workspaces = merged;
    upgraded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PersistedWorkspace, WorkspaceStatus};

    #[test]
    fn load_projects_dedupes_duplicate_worktree_paths() {
        let path = PathBuf::from("/tmp/repo");
        let projects = vec![
            PersistedProject {
                id: 1,
                name: "Repo".to_owned(),
                path: path.clone(),
                slug: "repo-1".to_owned(),
                is_git: true,
                expanded: false,
                workspaces: vec![PersistedWorkspace {
                    id: 10,
                    workspace_name: "main".to_owned(),
                    branch_name: "main".to_owned(),
                    worktree_path: path.clone(),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                }],
            },
            PersistedProject {
                id: 2,
                name: "Repo".to_owned(),
                path: path.clone(),
                slug: "repo-2".to_owned(),
                is_git: true,
                expanded: true,
                workspaces: vec![PersistedWorkspace {
                    id: 11,
                    workspace_name: "main".to_owned(),
                    branch_name: "main".to_owned(),
                    worktree_path: path.clone(),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                }],
            },
        ];

        let (loaded, upgraded) = load_projects(projects);
        assert!(upgraded, "expected a persistence upgrade");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].workspaces.len(), 1);
        assert_eq!(loaded[0].workspaces[0].workspace_name, "main");
        assert_eq!(loaded[0].workspaces[0].worktree_path, path);
    }

    #[test]
    fn load_projects_prefers_main_name_for_duplicate_worktree_paths() {
        let path = PathBuf::from("/tmp/repo");
        let projects = vec![PersistedProject {
            id: 1,
            name: "Repo".to_owned(),
            path: path.clone(),
            slug: "repo".to_owned(),
            is_git: true,
            expanded: false,
            workspaces: vec![
                PersistedWorkspace {
                    id: 10,
                    workspace_name: "main-2".to_owned(),
                    branch_name: "main".to_owned(),
                    worktree_path: path.clone(),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                },
                PersistedWorkspace {
                    id: 11,
                    workspace_name: "main".to_owned(),
                    branch_name: "main".to_owned(),
                    worktree_path: path.clone(),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                },
            ],
        }];

        let (loaded, upgraded) = load_projects(projects);
        assert!(upgraded, "expected a persistence upgrade");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].workspaces.len(), 1);
        assert_eq!(loaded[0].workspaces[0].workspace_name, "main");
    }

    #[test]
    fn dedupe_workspace_names_preserves_ordered_suffix_selection() {
        let mut workspaces = vec![
            Workspace {
                id: WorkspaceId(1),
                workspace_name: "dev".to_owned(),
                branch_name: "dev".to_owned(),
                worktree_path: PathBuf::from("/tmp/repo/dev"),
                status: WorkspaceStatus::Active,
                last_activity_at: None,
                archive_status: OperationStatus::Idle,
                branch_rename_status: OperationStatus::Idle,
            },
            Workspace {
                id: WorkspaceId(2),
                workspace_name: "dev".to_owned(),
                branch_name: "dev".to_owned(),
                worktree_path: PathBuf::from("/tmp/repo/dev-2"),
                status: WorkspaceStatus::Active,
                last_activity_at: None,
                archive_status: OperationStatus::Idle,
                branch_rename_status: OperationStatus::Idle,
            },
            Workspace {
                id: WorkspaceId(3),
                workspace_name: "dev-2".to_owned(),
                branch_name: "dev".to_owned(),
                worktree_path: PathBuf::from("/tmp/repo/dev-3"),
                status: WorkspaceStatus::Active,
                last_activity_at: None,
                archive_status: OperationStatus::Idle,
                branch_rename_status: OperationStatus::Idle,
            },
            Workspace {
                id: WorkspaceId(4),
                workspace_name: "dev".to_owned(),
                branch_name: "dev".to_owned(),
                worktree_path: PathBuf::from("/tmp/repo/dev-4"),
                status: WorkspaceStatus::Active,
                last_activity_at: None,
                archive_status: OperationStatus::Idle,
                branch_rename_status: OperationStatus::Idle,
            },
        ];

        assert!(dedupe_workspace_names(&mut workspaces));
        let names: Vec<String> = workspaces.into_iter().map(|w| w.workspace_name).collect();
        assert_eq!(names, vec!["dev", "dev-2", "dev-2-2", "dev-3"]);
    }

    #[test]
    fn apply_persisted_app_state_creates_conversations_for_open_tabs() {
        let path = PathBuf::from("/tmp/repo");
        let workspace_id = 10_u64;

        let persisted = PersistedAppState {
            projects: vec![PersistedProject {
                id: 1,
                name: "Repo".to_owned(),
                path: path.clone(),
                slug: "repo".to_owned(),
                is_git: true,
                expanded: true,
                workspaces: vec![PersistedWorkspace {
                    id: workspace_id,
                    workspace_name: "main".to_owned(),
                    branch_name: "main".to_owned(),
                    worktree_path: path.clone(),
                    status: WorkspaceStatus::Active,
                    last_activity_at_unix_seconds: None,
                }],
            }],
            sidebar_width: None,
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
            agent_codex_enabled: None,
            agent_amp_enabled: None,
            agent_claude_enabled: None,
            last_open_workspace_id: None,
            open_button_selection: None,
            sidebar_project_order: Vec::new(),
            sidebar_worktree_order: HashMap::new(),
            workspace_active_thread_id: HashMap::from([(workspace_id, 2)]),
            workspace_open_tabs: HashMap::from([(workspace_id, vec![1, 2])]),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_chat_scroll_anchor: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
            workspace_thread_run_config_overrides: HashMap::new(),
            task_prompt_templates: HashMap::new(),
        };

        let mut state = AppState::new();
        let _effects = apply_persisted_app_state(&mut state, persisted);

        let tabs = state
            .workspace_tabs
            .get(&WorkspaceId(workspace_id))
            .unwrap();
        assert!(tabs.open_tabs.contains(&WorkspaceThreadId(1)));
        assert!(tabs.open_tabs.contains(&WorkspaceThreadId(2)));
        assert_eq!(tabs.active_tab, WorkspaceThreadId(2));

        assert!(
            state
                .conversations
                .contains_key(&(WorkspaceId(workspace_id), WorkspaceThreadId(1)))
        );
        assert!(
            state
                .conversations
                .contains_key(&(WorkspaceId(workspace_id), WorkspaceThreadId(2)))
        );
    }

    #[test]
    fn normalize_thread_tabs_keeps_active_open_and_removes_from_archived() {
        let active = WorkspaceThreadId(2);
        let (open, archived) =
            normalize_thread_tabs(active, vec![WorkspaceThreadId(1)], vec![active]);
        assert!(open.contains(&active));
        assert!(!archived.contains(&active));
    }

    #[test]
    fn normalize_thread_tabs_removes_archived_tabs_that_are_open() {
        let active = WorkspaceThreadId(1);
        let (open, archived) = normalize_thread_tabs(
            active,
            vec![WorkspaceThreadId(2)],
            vec![WorkspaceThreadId(2)],
        );
        assert_eq!(open, vec![WorkspaceThreadId(2), active]);
        assert!(archived.is_empty());
    }

    #[test]
    fn normalize_thread_tabs_keeps_open_tab_duplicates() {
        let active = WorkspaceThreadId(1);
        let (open, archived) =
            normalize_thread_tabs(active, vec![active, active], vec![WorkspaceThreadId(2)]);
        assert_eq!(open, vec![active, active]);
        assert_eq!(archived, vec![WorkspaceThreadId(2)]);
    }
}
