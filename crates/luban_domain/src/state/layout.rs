use super::{ProjectId, WorkspaceId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainPane {
    None,
    Dashboard,
    ProjectSettings(ProjectId),
    Workspace(WorkspaceId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RightPane {
    None,
    Terminal,
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
