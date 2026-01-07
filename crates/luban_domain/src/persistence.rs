use crate::agent_settings::parse_thinking_effort;
use crate::{
    AppState, Effect, MainPane, OperationStatus, PersistedAppState, PersistedProject,
    PersistedWorkspace, Project, ProjectId, RightPane, Workspace, WorkspaceId, WorkspaceStatus,
    WorkspaceTabs, WorkspaceThreadId,
};
use crate::{default_agent_model_id, default_thinking_effort, normalize_thinking_effort};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

pub(crate) fn apply_persisted_app_state(
    state: &mut AppState,
    persisted: PersistedAppState,
) -> Vec<Effect> {
    if !state.projects.is_empty() {
        return Vec::new();
    }

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

    state.agent_default_model_id = agent_default_model_id;
    state.agent_default_thinking_effort = agent_default_thinking_effort;

    let (projects, projects_upgraded) = load_projects(persisted.projects);
    state.projects = projects;
    state.sidebar_width = persisted.sidebar_width;
    state.terminal_pane_width = persisted.terminal_pane_width;
    state.last_open_workspace_id = persisted.last_open_workspace_id.map(WorkspaceId);
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

    let upgraded = projects_upgraded || state.ensure_main_workspaces();
    let mut effects = if upgraded {
        vec![Effect::SaveAppState]
    } else {
        Vec::new()
    };

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
        agent_default_model_id: Some(state.agent_default_model_id.clone()),
        agent_default_thinking_effort: Some(
            state.agent_default_thinking_effort.as_str().to_owned(),
        ),
        last_open_workspace_id: state.last_open_workspace_id.map(|id| id.0),
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
        workspace_unread_completions: state
            .workspace_unread_completions
            .iter()
            .map(|workspace_id| (workspace_id.0, true))
            .collect(),
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

        if dedupe_workspace_names(&mut canonical.workspaces) {
            upgraded = true;
        }

        merged.push(canonical);
    }

    merged.sort_by_key(|p| p.id.0);
    (merged, upgraded)
}

fn dedupe_workspace_names(workspaces: &mut [Workspace]) -> bool {
    use std::collections::HashSet;

    let mut upgraded = false;
    let mut used: HashSet<String> = HashSet::new();

    for workspace in workspaces.iter_mut() {
        let base = workspace.workspace_name.clone();
        if used.insert(base.clone()) {
            continue;
        }

        for i in 2.. {
            let candidate = format!("{base}-{i}");
            if used.insert(candidate.clone()) {
                workspace.workspace_name = candidate;
                upgraded = true;
                break;
            }
        }
    }

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
