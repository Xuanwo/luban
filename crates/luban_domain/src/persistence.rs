use crate::agent_settings::{parse_agent_runner_kind, parse_thinking_effort};
use crate::{
    AppState, AppearanceFonts, AppearanceTheme, Effect, MainPane, OperationStatus,
    PersistedAppState, PersistedProject, PersistedWorkspace, Project, ProjectId, RightPane,
    Workspace, WorkspaceId, WorkspaceStatus, WorkspaceTabs, WorkspaceThreadId,
};
use crate::{
    TaskIntentKind, default_agent_model_id, default_agent_runner_kind, default_amp_mode,
    default_system_prompt_templates, default_task_prompt_templates, default_thinking_effort,
    normalize_thinking_effort,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

fn normalize_font(raw: Option<&str>, fallback: &str) -> String {
    raw.map(str::trim)
        .filter(|v| !v.is_empty())
        .filter(|v| v.len() <= 128)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| fallback.to_owned())
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

    let agent_amp_mode = persisted
        .agent_amp_mode
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .filter(|v| v.len() <= 32)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| default_amp_mode().to_owned());

    state.agent_default_model_id = agent_default_model_id;
    state.agent_default_thinking_effort = agent_default_thinking_effort;
    state.agent_default_runner = agent_default_runner;
    state.agent_amp_mode = agent_amp_mode;
    state.agent_codex_enabled = persisted.agent_codex_enabled.unwrap_or(true);
    state.agent_amp_enabled = persisted.agent_amp_enabled.unwrap_or(true);

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
    state.workspace_tabs = HashMap::new();
    state.conversations = HashMap::new();
    state.workspace_unread_completions = persisted
        .workspace_unread_completions
        .into_iter()
        .filter_map(|(workspace_id, unread)| unread.then_some(WorkspaceId(workspace_id)))
        .collect::<HashSet<_>>();

    for workspace in state.projects.iter().flat_map(|p| &p.workspaces) {
        let workspace_id = workspace.id;
        let active = persisted
            .workspace_active_thread_id
            .get(&workspace_id.0)
            .copied()
            .map(WorkspaceThreadId)
            .unwrap_or(WorkspaceThreadId(1));

        let mut open_tabs = persisted
            .workspace_open_tabs
            .get(&workspace_id.0)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(WorkspaceThreadId)
            .collect::<Vec<_>>();
        let mut archived_tabs = persisted
            .workspace_archived_tabs
            .get(&workspace_id.0)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(WorkspaceThreadId)
            .collect::<Vec<_>>();
        archived_tabs.retain(|id| !open_tabs.contains(id));
        if open_tabs.is_empty() {
            open_tabs.push(active);
        }
        if !open_tabs.contains(&active) {
            open_tabs.push(active);
        }
        archived_tabs.retain(|id| *id != active);

        let next_thread_id = persisted
            .workspace_next_thread_id
            .get(&workspace_id.0)
            .copied()
            .unwrap_or(active.0 + 1);

        state.workspace_tabs.insert(
            workspace_id,
            WorkspaceTabs {
                open_tabs: open_tabs.clone(),
                archived_tabs,
                active_tab: active,
                next_thread_id,
            },
        );

        for thread_id in open_tabs {
            state.conversations.insert(
                (workspace_id, thread_id),
                state.default_conversation(thread_id),
            );
        }
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

pub(crate) fn to_persisted_app_state(state: &AppState) -> PersistedAppState {
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
                        last_activity_at_unix_seconds: w
                            .last_activity_at
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())),
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
        last_open_workspace_id: state.last_open_workspace_id.map(|id| id.0),
        open_button_selection: state.open_button_selection.clone(),
        workspace_active_thread_id: state
            .workspace_tabs
            .iter()
            .map(|(workspace_id, tabs)| (workspace_id.0, tabs.active_tab.0))
            .collect(),
        workspace_open_tabs: state
            .workspace_tabs
            .iter()
            .map(|(workspace_id, tabs)| {
                (
                    workspace_id.0,
                    tabs.open_tabs.iter().map(|id| id.0).collect(),
                )
            })
            .collect(),
        workspace_archived_tabs: state
            .workspace_tabs
            .iter()
            .map(|(workspace_id, tabs)| {
                (
                    workspace_id.0,
                    tabs.archived_tabs.iter().map(|id| id.0).collect(),
                )
            })
            .collect(),
        workspace_next_thread_id: state
            .workspace_tabs
            .iter()
            .map(|(workspace_id, tabs)| (workspace_id.0, tabs.next_thread_id))
            .collect(),
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
            .collect(),
        task_prompt_templates: HashMap::new(),
    }
}

fn load_projects(projects: Vec<PersistedProject>) -> (Vec<Project>, bool) {
    use std::collections::hash_map::Entry;

    let mut upgraded = false;
    let mut grouped: HashMap<PathBuf, Vec<Project>> = HashMap::new();

    for persisted in projects {
        let normalized_path = normalize_project_path(&persisted.path);
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
                        .map(|secs| UNIX_EPOCH + Duration::from_secs(secs)),
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
    use std::collections::{HashMap, HashSet};

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
    use std::collections::HashMap;

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

fn normalize_project_path(path: &std::path::Path) -> PathBuf {
    use std::path::Component;

    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let popped = out.pop();
                if !popped {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PersistedProject, PersistedWorkspace};

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
}
