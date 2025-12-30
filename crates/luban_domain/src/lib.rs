use std::path::PathBuf;

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

    ClearError,
}

#[derive(Clone, Debug)]
pub enum Effect {
    CreateWorkspace { project_id: ProjectId },
    ArchiveWorkspace { workspace_id: WorkspaceId },
}

impl AppState {
    pub fn new() -> Self {
        Self {
            next_project_id: 1,
            next_workspace_id: 1,
            projects: Vec::new(),
            main_pane: MainPane::None,
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
                self.insert_workspace(project_id, &workspace_name, &branch_name, worktree_path);
                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    project.create_workspace_status = OperationStatus::Idle;
                }
                Vec::new()
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
                Vec::new()
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
}
