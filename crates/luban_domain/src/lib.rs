#[derive(Clone, Debug)]
pub struct Project {
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineStatus {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug)]
pub struct TimelineItem {
    pub title: String,
    pub status: TimelineStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RightPaneTab {
    Diff,
    Terminal,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub projects: Vec<Project>,
    pub selected_project: usize,
    pub timeline: Vec<TimelineItem>,
    pub selected_timeline_item: usize,
    pub right_pane_tab: RightPaneTab,
}

impl AppState {
    pub fn demo() -> Self {
        Self {
            projects: vec![
                Project {
                    name: "luban".to_owned(),
                },
                Project {
                    name: "scratch".to_owned(),
                },
            ],
            selected_project: 0,
            timeline: vec![
                TimelineItem {
                    title: "Scaffold project".to_owned(),
                    status: TimelineStatus::Done,
                },
                TimelineItem {
                    title: "Index workspace".to_owned(),
                    status: TimelineStatus::Running,
                },
                TimelineItem {
                    title: "Propose patch".to_owned(),
                    status: TimelineStatus::Pending,
                },
            ],
            selected_timeline_item: 1,
            right_pane_tab: RightPaneTab::Diff,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_state_is_consistent() {
        let state = AppState::demo();

        assert!(!state.projects.is_empty());
        assert!(state.selected_project < state.projects.len());

        assert!(!state.timeline.is_empty());
        assert!(state.selected_timeline_item < state.timeline.len());
    }
}
