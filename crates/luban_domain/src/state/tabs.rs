use super::WorkspaceThreadId;

#[derive(Clone, Debug)]
pub struct WorkspaceTabs {
    pub open_tabs: Vec<WorkspaceThreadId>,
    pub archived_tabs: Vec<WorkspaceThreadId>,
    pub active_tab: WorkspaceThreadId,
    pub next_thread_id: u64,
}

impl WorkspaceTabs {
    pub fn new_with_initial(thread_id: WorkspaceThreadId) -> Self {
        Self {
            open_tabs: vec![thread_id],
            archived_tabs: Vec::new(),
            active_tab: thread_id,
            next_thread_id: thread_id.0 + 1,
        }
    }

    fn remove_open(&mut self, thread_id: WorkspaceThreadId) {
        self.open_tabs.retain(|id| *id != thread_id);
    }

    fn remove_archived(&mut self, thread_id: WorkspaceThreadId) {
        self.archived_tabs.retain(|id| *id != thread_id);
    }

    fn ensure_open(&mut self, thread_id: WorkspaceThreadId) {
        if !self.open_tabs.contains(&thread_id) {
            self.open_tabs.push(thread_id);
        }
    }

    fn ensure_archived(&mut self, thread_id: WorkspaceThreadId) {
        if !self.archived_tabs.contains(&thread_id) {
            self.archived_tabs.push(thread_id);
        }
    }

    pub fn activate(&mut self, thread_id: WorkspaceThreadId) {
        self.active_tab = thread_id;
        self.remove_archived(thread_id);
        self.ensure_open(thread_id);
    }

    pub fn archive_tab(&mut self, thread_id: WorkspaceThreadId) {
        let mut active_fallback: Option<WorkspaceThreadId> = None;
        if self.active_tab == thread_id
            && let Some(idx) = self.open_tabs.iter().position(|id| *id == thread_id)
        {
            if idx > 0 {
                active_fallback = Some(self.open_tabs[idx - 1]);
            } else if idx + 1 < self.open_tabs.len() {
                active_fallback = Some(self.open_tabs[idx + 1]);
            }
        }
        self.remove_open(thread_id);
        self.ensure_archived(thread_id);
        if let Some(next) = active_fallback.or_else(|| self.open_tabs.first().copied()) {
            self.active_tab = next;
        }
    }

    pub fn restore_tab(&mut self, thread_id: WorkspaceThreadId, activate: bool) {
        self.remove_archived(thread_id);
        self.ensure_open(thread_id);
        if activate {
            self.active_tab = thread_id;
        }
    }

    pub fn allocate_thread_id(&mut self) -> WorkspaceThreadId {
        let id = WorkspaceThreadId(self.next_thread_id);
        self.next_thread_id += 1;
        id
    }

    pub fn reorder_tab(&mut self, thread_id: WorkspaceThreadId, to_index: usize) -> bool {
        let Some(from_index) = self.open_tabs.iter().position(|id| *id == thread_id) else {
            return false;
        };
        if from_index == to_index {
            return false;
        }
        let tab = self.open_tabs.remove(from_index);
        let mut target = to_index.min(self.open_tabs.len());
        if from_index < to_index {
            target = target.saturating_sub(1);
        }
        self.open_tabs.insert(target, tab);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_tab_opens_even_when_not_archived() {
        let mut tabs = WorkspaceTabs::new_with_initial(WorkspaceThreadId(1));
        tabs.restore_tab(WorkspaceThreadId(2), true);
        assert_eq!(tabs.active_tab, WorkspaceThreadId(2));
        assert!(tabs.open_tabs.contains(&WorkspaceThreadId(2)));
        assert!(!tabs.archived_tabs.contains(&WorkspaceThreadId(2)));
    }
}
