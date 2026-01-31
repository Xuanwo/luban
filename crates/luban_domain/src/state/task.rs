#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Backlog,
    Todo,
    InProgress,
    InReview,
    Done,
    Canceled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Backlog => "backlog",
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::InReview => "in_review",
            TaskStatus::Done => "done",
            TaskStatus::Canceled => "canceled",
        }
    }
}

pub fn parse_task_status(value: &str) -> Option<TaskStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "backlog" => Some(TaskStatus::Backlog),
        "todo" => Some(TaskStatus::Todo),
        "in_progress" => Some(TaskStatus::InProgress),
        "in_review" => Some(TaskStatus::InReview),
        "done" => Some(TaskStatus::Done),
        "canceled" => Some(TaskStatus::Canceled),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Idle,
    Running,
    Awaiting,
    Paused,
}

impl TurnStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TurnStatus::Idle => "idle",
            TurnStatus::Running => "running",
            TurnStatus::Awaiting => "awaiting",
            TurnStatus::Paused => "paused",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnResult {
    Completed,
    Failed,
}

impl TurnResult {
    pub fn as_str(self) -> &'static str {
        match self {
            TurnResult::Completed => "completed",
            TurnResult::Failed => "failed",
        }
    }
}
