use gpui::point;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, CursorStyle, ElementId, IntoElement, MouseButton, Pixels, PromptButton,
    PromptLevel, SharedString, Window, div, px, rems,
};
use gpui_component::input::RopeExt as _;
use gpui_component::{
    ActiveTheme as _, Disableable as _, ElementExt as _, Icon, IconName, IconNamed as _,
    Sizable as _, Size, StyledExt as _,
    button::*,
    collapsible::Collapsible,
    input::{Input, InputEvent, InputState},
    popover::Popover,
    scroll::{ScrollableElement as _, Scrollbar, ScrollbarShow},
    spinner::Spinner,
    text::{TextView, TextViewStyle},
};
use luban_domain::{
    Action, AgentRunConfig, AppState, CodexThreadEvent, CodexThreadItem, DashboardCardModel,
    DashboardPreviewModel, DashboardStage, Effect, MainPane, OperationStatus, ProjectId,
    ProjectWorkspaceService, PullRequestInfo, RightPane, RunAgentTurnRequest, ThinkingEffort,
    WorkspaceId, WorkspaceStatus, WorkspaceThreadId, agent_model_label, agent_models,
    compose_user_message_text, dashboard_cards, dashboard_preview, default_agent_model_id,
    default_thinking_effort, draft_text_and_attachments_from_message_text,
    ordered_draft_attachments_for_display, thinking_effort_supported,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime},
};

use crate::selectable_text::SelectablePlainText;
use crate::terminal_panel::{WorkspaceTerminal, spawn_workspace_terminal, terminal_cell_metrics};

mod attachments;
mod chat;
mod dashboard;
mod gh;
mod layout;
mod right_pane;
mod sidebar;
mod thread_tabs;
mod titlebar;
use layout::{
    DASHBOARD_PREVIEW_RESIZER_WIDTH, DashboardPreviewResizeDrag, DashboardPreviewResizeGhost,
    DashboardPreviewResizeState, RIGHT_PANE_CONTENT_PADDING, SIDEBAR_RESIZER_WIDTH,
    SidebarResizeDrag, SidebarResizeGhost, SidebarResizeState, TERMINAL_PANE_RESIZER_WIDTH,
    TITLEBAR_HEIGHT, TerminalPaneResizeDrag, TerminalPaneResizeGhost, TerminalPaneResizeState,
};
use sidebar::render_sidebar;
use titlebar::render_titlebar;
const CHAT_ATTACHMENT_THUMBNAIL_SIZE: f32 = 72.0;
const CHAT_ATTACHMENT_FILE_WIDTH: f32 = CHAT_ATTACHMENT_THUMBNAIL_SIZE * 2.0;
const CHAT_SCROLL_BOTTOM_TOLERANCE_Y10: i32 = 200;
const CHAT_SCROLL_USER_SCROLL_UP_THRESHOLD_Y10: i32 = 10;

use chat::composer::ContextImportSpec;

type WorkspaceThreadKey = (WorkspaceId, WorkspaceThreadId);

static PENDING_CONTEXT_TOKEN_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

#[cfg(not(test))]
const SUCCESS_TOAST_DURATION: Duration = Duration::from_secs(2);
#[cfg(test)]
const SUCCESS_TOAST_DURATION: Duration = Duration::from_millis(30);

fn next_pending_context_id() -> u64 {
    PENDING_CONTEXT_TOKEN_ID.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
fn context_token(kind: &str, value: &str) -> String {
    format!("<<context:{kind}:{value}>>>")
}

pub struct LubanRootView {
    state: AppState,
    services: Arc<dyn ProjectWorkspaceService>,
    terminal_enabled: bool,
    terminal_resize_hooked: bool,
    debug_layout_enabled: bool,
    debug_scrollbar_enabled: bool,
    sidebar_width_preview: Option<Pixels>,
    last_sidebar_width: Pixels,
    sidebar_resize: Option<SidebarResizeState>,
    terminal_pane_width_preview: Option<Pixels>,
    terminal_pane_resize: Option<TerminalPaneResizeState>,
    dashboard_preview_width_preview: Option<Pixels>,
    dashboard_preview_resize: Option<DashboardPreviewResizeState>,
    last_dashboard_preview_width: Pixels,
    #[cfg(test)]
    last_terminal_grid_size: Option<(u16, u16)>,
    workspace_terminals: HashMap<WorkspaceThreadKey, WorkspaceTerminal>,
    workspace_terminal_errors: HashMap<WorkspaceThreadKey, String>,
    gh_authorized: Option<bool>,
    gh_auth_check_inflight: bool,
    gh_last_auth_check_at: Option<Instant>,
    workspace_pull_request_numbers: HashMap<WorkspaceId, Option<PullRequestInfo>>,
    workspace_pull_request_inflight: HashSet<WorkspaceId>,
    chat_input: Option<gpui::Entity<InputState>>,
    expanded_agent_items: HashSet<String>,
    expanded_agent_turns: HashSet<String>,
    chat_column_width: Option<Pixels>,
    running_turn_started_at: HashMap<WorkspaceThreadKey, Instant>,
    running_turn_tickers: HashSet<WorkspaceThreadKey>,
    pending_turn_durations: HashMap<WorkspaceThreadKey, Duration>,
    running_turn_user_message_count: HashMap<WorkspaceThreadKey, usize>,
    running_turn_summary_order: HashMap<WorkspaceThreadKey, Vec<String>>,
    turn_generation: HashMap<WorkspaceThreadKey, u64>,
    turn_cancel_flags: HashMap<WorkspaceThreadKey, Arc<AtomicBool>>,
    chat_scroll_handle: gpui::ScrollHandle,
    chat_follow_tail: HashMap<WorkspaceThreadKey, bool>,
    chat_last_observed_scroll_offset_y10: HashMap<WorkspaceThreadKey, i32>,
    projects_scroll_handle: gpui::ScrollHandle,
    last_chat_workspace_id: Option<WorkspaceThreadKey>,
    last_chat_item_count: usize,
    last_workspace_before_dashboard: Option<WorkspaceId>,
    workspace_thread_tab_bounds: HashMap<WorkspaceThreadKey, gpui::Bounds<Pixels>>,
    workspace_thread_tab_reorder: Option<thread_tabs::WorkspaceThreadTabReorderState>,
    success_toast_message: Option<String>,
    success_toast_generation: u64,
    pending_context_imports: HashMap<WorkspaceThreadKey, usize>,
    #[cfg(test)]
    inspector_bounds: HashMap<&'static str, gpui::Bounds<Pixels>>,
    #[cfg(test)]
    dashboard_scroll_wheel_events: usize,
    _subscriptions: Vec<gpui::Subscription>,
}

impl LubanRootView {
    pub fn new(services: Arc<dyn ProjectWorkspaceService>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            state: AppState::new(),
            services,
            terminal_enabled: true,
            terminal_resize_hooked: false,
            debug_layout_enabled: debug_layout::enabled_from_env(),
            debug_scrollbar_enabled: debug_scrollbar::enabled_from_env(),
            sidebar_width_preview: None,
            last_sidebar_width: px(300.0),
            sidebar_resize: None,
            terminal_pane_width_preview: None,
            terminal_pane_resize: None,
            dashboard_preview_width_preview: None,
            dashboard_preview_resize: None,
            last_dashboard_preview_width: px(420.0),
            #[cfg(test)]
            last_terminal_grid_size: None,
            workspace_terminals: HashMap::new(),
            workspace_terminal_errors: HashMap::new(),
            gh_authorized: None,
            gh_auth_check_inflight: false,
            gh_last_auth_check_at: None,
            workspace_pull_request_numbers: HashMap::new(),
            workspace_pull_request_inflight: HashSet::new(),
            chat_input: None,
            expanded_agent_items: HashSet::new(),
            expanded_agent_turns: HashSet::new(),
            chat_column_width: None,
            running_turn_started_at: HashMap::new(),
            running_turn_tickers: HashSet::new(),
            pending_turn_durations: HashMap::new(),
            running_turn_user_message_count: HashMap::new(),
            running_turn_summary_order: HashMap::new(),
            turn_generation: HashMap::new(),
            turn_cancel_flags: HashMap::new(),
            chat_scroll_handle: gpui::ScrollHandle::new(),
            chat_follow_tail: HashMap::new(),
            chat_last_observed_scroll_offset_y10: HashMap::new(),
            projects_scroll_handle: gpui::ScrollHandle::new(),
            last_chat_workspace_id: None,
            last_chat_item_count: 0,
            last_workspace_before_dashboard: None,
            workspace_thread_tab_bounds: HashMap::new(),
            workspace_thread_tab_reorder: None,
            success_toast_message: None,
            success_toast_generation: 0,
            pending_context_imports: HashMap::new(),
            #[cfg(test)]
            inspector_bounds: HashMap::new(),
            #[cfg(test)]
            dashboard_scroll_wheel_events: 0,
            _subscriptions: Vec::new(),
        };

        this.dispatch(Action::AppStarted, cx);
        this
    }

    #[cfg(test)]
    pub fn with_state(
        services: Arc<dyn ProjectWorkspaceService>,
        state: AppState,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            last_sidebar_width: state
                .sidebar_width
                .map(|v| px(v as f32))
                .unwrap_or(px(300.0)),
            state,
            services,
            terminal_enabled: false,
            terminal_resize_hooked: false,
            debug_layout_enabled: false,
            debug_scrollbar_enabled: false,
            sidebar_width_preview: None,
            sidebar_resize: None,
            terminal_pane_width_preview: None,
            terminal_pane_resize: None,
            dashboard_preview_width_preview: None,
            dashboard_preview_resize: None,
            last_dashboard_preview_width: px(420.0),
            #[cfg(test)]
            last_terminal_grid_size: None,
            workspace_terminals: HashMap::new(),
            workspace_terminal_errors: HashMap::new(),
            gh_authorized: None,
            gh_auth_check_inflight: false,
            gh_last_auth_check_at: None,
            workspace_pull_request_numbers: HashMap::new(),
            workspace_pull_request_inflight: HashSet::new(),
            chat_input: None,
            expanded_agent_items: HashSet::new(),
            expanded_agent_turns: HashSet::new(),
            chat_column_width: None,
            running_turn_started_at: HashMap::new(),
            running_turn_tickers: HashSet::new(),
            pending_turn_durations: HashMap::new(),
            running_turn_user_message_count: HashMap::new(),
            running_turn_summary_order: HashMap::new(),
            turn_generation: HashMap::new(),
            turn_cancel_flags: HashMap::new(),
            chat_scroll_handle: gpui::ScrollHandle::new(),
            chat_follow_tail: HashMap::new(),
            chat_last_observed_scroll_offset_y10: HashMap::new(),
            projects_scroll_handle: gpui::ScrollHandle::new(),
            last_chat_workspace_id: None,
            last_chat_item_count: 0,
            last_workspace_before_dashboard: None,
            workspace_thread_tab_bounds: HashMap::new(),
            workspace_thread_tab_reorder: None,
            success_toast_message: None,
            success_toast_generation: 0,
            pending_context_imports: HashMap::new(),
            #[cfg(test)]
            inspector_bounds: HashMap::new(),
            #[cfg(test)]
            dashboard_scroll_wheel_events: 0,
            _subscriptions: Vec::new(),
        }
    }

    fn default_thread_id() -> WorkspaceThreadId {
        WorkspaceThreadId::from_u64(1)
    }

    fn active_thread_id_for_workspace(&self, workspace_id: WorkspaceId) -> WorkspaceThreadId {
        self.state
            .active_thread_id(workspace_id)
            .unwrap_or_else(Self::default_thread_id)
    }

    fn active_thread_key_for_workspace(&self, workspace_id: WorkspaceId) -> WorkspaceThreadKey {
        (
            workspace_id,
            self.active_thread_id_for_workspace(workspace_id),
        )
    }

    fn current_chat_workspace_id(&self) -> Option<WorkspaceId> {
        match self.state.main_pane {
            MainPane::Workspace(workspace_id) => Some(workspace_id),
            MainPane::Dashboard => self.state.dashboard_preview_workspace_id,
            _ => None,
        }
    }

    fn current_chat_key(&self) -> Option<WorkspaceThreadKey> {
        let workspace_id = self.current_chat_workspace_id()?;
        Some(self.active_thread_key_for_workspace(workspace_id))
    }

    #[cfg(test)]
    pub fn debug_state(&self) -> &AppState {
        &self.state
    }

    #[cfg(test)]
    pub fn debug_inspector_bounds(&self, key: &'static str) -> Option<gpui::Bounds<Pixels>> {
        self.inspector_bounds.get(key).copied()
    }

    #[cfg(test)]
    pub fn debug_dashboard_scroll_wheel_events(&self) -> usize {
        self.dashboard_scroll_wheel_events
    }

    #[cfg(test)]
    fn record_inspector_bounds(&mut self, key: &'static str, bounds: gpui::Bounds<Pixels>) {
        self.inspector_bounds.insert(key, bounds);
    }

    #[cfg(test)]
    pub fn debug_last_terminal_grid_size(&self) -> Option<(u16, u16)> {
        self.last_terminal_grid_size
    }

    #[cfg(test)]
    pub fn debug_chat_scroll_offset_y10(&self) -> i32 {
        quantize_pixels_y10(self.chat_scroll_handle.offset().y)
    }

    #[cfg(test)]
    pub fn debug_chat_scroll_max_offset_y10(&self) -> i32 {
        quantize_pixels_y10(self.chat_scroll_handle.max_offset().height)
    }

    #[cfg(test)]
    pub fn debug_projects_scroll_offset_y10(&self) -> i32 {
        quantize_pixels_y10(self.projects_scroll_handle.offset().y)
    }

    #[cfg(test)]
    pub fn debug_projects_scroll_max_offset_y10(&self) -> i32 {
        quantize_pixels_y10(self.projects_scroll_handle.max_offset().height)
    }

    fn should_chat_follow_tail(&self, chat_key: WorkspaceThreadKey) -> bool {
        self.chat_follow_tail
            .get(&chat_key)
            .copied()
            .unwrap_or(true)
    }

    fn is_chat_near_bottom(offset_y10: i32, max_y10: i32) -> bool {
        if max_y10 <= 0 {
            return true;
        }
        let bottom_y10 = -max_y10;
        (offset_y10 - bottom_y10).abs() <= CHAT_SCROLL_BOTTOM_TOLERANCE_Y10
    }

    fn dispatch(&mut self, action: Action, cx: &mut Context<Self>) {
        let previous_chat_key_for_scroll = match self.state.main_pane {
            MainPane::Workspace(workspace_id) => {
                Some(self.active_thread_key_for_workspace(workspace_id))
            }
            _ => None,
        };

        let success_toast = match &action {
            Action::AddProject { .. } => Some("Project added".to_owned()),
            Action::WorkspaceCreated { workspace_name, .. } => {
                Some(format!("Workspace \"{workspace_name}\" created"))
            }
            Action::WorkspaceArchived { workspace_id } => self
                .state
                .workspace(*workspace_id)
                .map(|workspace| format!("Workspace \"{}\" archived", workspace.workspace_name))
                .or_else(|| Some("Workspace archived".to_owned())),
            _ => None,
        };

        match &action {
            Action::OpenDashboard => {
                if let MainPane::Workspace(workspace_id) = self.state.main_pane {
                    self.last_workspace_before_dashboard = Some(workspace_id);
                } else if let Some((workspace_id, _)) = self.last_chat_workspace_id {
                    self.last_workspace_before_dashboard = Some(workspace_id);
                }
            }
            Action::OpenWorkspace { workspace_id } => {
                self.last_workspace_before_dashboard = Some(*workspace_id);
            }
            _ => {}
        }

        if let Action::WorkspaceArchived { workspace_id } = &action {
            self.workspace_terminal_errors
                .retain(|(wid, _), _| *wid != *workspace_id);
            let keys = self
                .workspace_terminals
                .keys()
                .copied()
                .filter(|(wid, _)| *wid == *workspace_id)
                .collect::<Vec<_>>();
            for key in keys {
                if let Some(mut terminal) = self.workspace_terminals.remove(&key) {
                    terminal.kill();
                }
            }
            self.workspace_pull_request_numbers.remove(workspace_id);
            self.workspace_pull_request_inflight.remove(workspace_id);
            if self.last_workspace_before_dashboard == Some(*workspace_id) {
                self.last_workspace_before_dashboard = None;
            }
        }

        let start_timer_key = match &action {
            Action::SendAgentMessage {
                workspace_id,
                thread_id,
                ..
            } => Some((*workspace_id, *thread_id)),
            _ => None,
        };
        let stop_timer_key = match &action {
            Action::AgentEventReceived {
                workspace_id,
                thread_id,
                event:
                    CodexThreadEvent::TurnCompleted { .. }
                    | CodexThreadEvent::TurnFailed { .. }
                    | CodexThreadEvent::Error { .. },
            }
            | Action::AgentTurnFinished {
                workspace_id,
                thread_id,
            } => Some((*workspace_id, *thread_id)),
            Action::CancelAgentTurn {
                workspace_id,
                thread_id,
            } => Some((*workspace_id, *thread_id)),
            _ => None,
        };
        let clear_pending_duration_key = match &action {
            Action::AgentEventReceived {
                workspace_id,
                thread_id,
                event: CodexThreadEvent::TurnDuration { .. },
            } => Some((*workspace_id, *thread_id)),
            _ => None,
        };

        let stop_timer_turn_id = stop_timer_key.and_then(|(workspace_id, thread_id)| {
            self.state
                .workspace_thread_conversation(workspace_id, thread_id)
                .and_then(|c| latest_agent_turn_id(&c.entries))
        });

        let mut effects = self.state.apply(action);
        let next_chat_key_for_scroll = match self.state.main_pane {
            MainPane::Workspace(workspace_id) => {
                Some(self.active_thread_key_for_workspace(workspace_id))
            }
            _ => None,
        };
        if previous_chat_key_for_scroll != next_chat_key_for_scroll
            && let Some((workspace_id, thread_id)) = previous_chat_key_for_scroll
        {
            let offset_y10 = quantize_pixels_y10(self.chat_scroll_handle.offset().y);
            effects.extend(self.state.apply(Action::WorkspaceChatScrollSaved {
                workspace_id,
                thread_id,
                offset_y10,
            }));
        }
        cx.notify();
        if let Some(message) = success_toast {
            self.show_success_toast(message, cx);
        }

        if let Some(key) = start_timer_key {
            self.pending_turn_durations.remove(&key);
            let is_running = self
                .state
                .workspace_thread_conversation(key.0, key.1)
                .map(|c| c.run_status == OperationStatus::Running)
                .unwrap_or(false);
            if is_running {
                self.ensure_running_turn_timer(key, cx);
            }
        }

        if let Some(key) = stop_timer_key {
            if let Some(started_at) = self.running_turn_started_at.get(&key) {
                self.pending_turn_durations
                    .insert(key, started_at.elapsed());
            }
            self.running_turn_started_at.remove(&key);
            self.running_turn_tickers.remove(&key);
            self.running_turn_user_message_count.remove(&key);
            self.running_turn_summary_order.remove(&key);
            if let Some(turn_id) = stop_timer_turn_id {
                self.collapse_agent_turn_summary(&turn_id);
            }
            cx.notify();
        }

        if let Some(key) = clear_pending_duration_key {
            self.pending_turn_durations.remove(&key);
            cx.notify();
        }

        for effect in effects {
            self.run_effect(effect, cx);
        }

        self.ensure_workspace_pull_request_numbers(cx);
    }

    fn bump_turn_generation(&mut self, key: WorkspaceThreadKey) -> u64 {
        let entry = self.turn_generation.entry(key).or_insert(0);
        *entry += 1;
        *entry
    }

    fn collapse_agent_turn_summary(&mut self, id: &str) {
        self.expanded_agent_turns.remove(id);
        let prefix = format!("{id}::");
        self.expanded_agent_items
            .retain(|item_id| !item_id.starts_with(&prefix));
    }

    fn toggle_agent_turn_expanded(&mut self, id: &str) {
        if self.expanded_agent_turns.contains(id) {
            self.expanded_agent_turns.remove(id);
        } else {
            self.expanded_agent_turns.insert(id.to_owned());
            let prefix = format!("{id}::");
            self.expanded_agent_items
                .retain(|item_id| !item_id.starts_with(&prefix));
        }
    }

    fn ensure_running_turn_timer(&mut self, key: WorkspaceThreadKey, cx: &mut Context<Self>) {
        self.running_turn_started_at
            .entry(key)
            .or_insert_with(Instant::now);
        if !self.running_turn_tickers.insert(key) {
            return;
        }

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    loop {
                        gpui::Timer::after(Duration::from_secs(1)).await;

                        let still_running = this
                            .update(&mut async_cx, |view: &mut LubanRootView, view_cx| {
                                let running = view
                                    .state
                                    .workspace_thread_conversation(key.0, key.1)
                                    .map(|c| c.run_status == OperationStatus::Running)
                                    .unwrap_or(false);
                                if running {
                                    view_cx.notify();
                                } else {
                                    view.running_turn_started_at.remove(&key);
                                    view.running_turn_tickers.remove(&key);
                                }
                                running
                            })
                            .unwrap_or(false);

                        if !still_running {
                            break;
                        }
                    }
                }
            },
        )
        .detach();
    }

    fn run_effect(&mut self, effect: Effect, cx: &mut Context<Self>) {
        match effect {
            Effect::LoadAppState => self.run_load_app_state(cx),
            Effect::SaveAppState => self.run_save_app_state(cx),
            Effect::CreateWorkspace { project_id } => self.run_create_workspace(project_id, cx),
            Effect::OpenWorkspaceInIde { workspace_id } => {
                self.run_open_workspace_in_ide(workspace_id, cx)
            }
            Effect::OpenWorkspacePullRequest { workspace_id } => {
                self.run_open_workspace_pull_request(workspace_id, cx)
            }
            Effect::ArchiveWorkspace { workspace_id } => {
                self.run_archive_workspace(workspace_id, cx)
            }
            Effect::EnsureConversation {
                workspace_id,
                thread_id,
            } => self.run_ensure_conversation(workspace_id, thread_id, cx),
            Effect::LoadWorkspaceThreads { workspace_id } => {
                self.run_load_workspace_threads(workspace_id, cx)
            }
            Effect::LoadConversation {
                workspace_id,
                thread_id,
            } => self.run_load_conversation(workspace_id, thread_id, cx),
            Effect::RunAgentTurn {
                workspace_id,
                thread_id,
                text,
                run_config,
            } => self.run_agent_turn(workspace_id, thread_id, text, run_config, cx),
            Effect::CancelAgentTurn {
                workspace_id,
                thread_id,
            } => {
                let key = (workspace_id, thread_id);
                self.bump_turn_generation(key);
                if let Some(flag) = self.turn_cancel_flags.get(&key) {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        }
    }

    fn run_load_app_state(&mut self, cx: &mut Context<Self>) {
        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move { services.load_app_state() })
                        .await;

                    let action = match result {
                        Ok(persisted) => Action::AppStateLoaded { persisted },
                        Err(message) => Action::AppStateLoadFailed { message },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_save_app_state(&mut self, cx: &mut Context<Self>) {
        let services = self.services.clone();
        let snapshot = self.state.to_persisted();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move { services.save_app_state(snapshot) })
                        .await;

                    let action = match result {
                        Ok(()) => Action::AppStateSaved,
                        Err(message) => Action::AppStateSaveFailed { message },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_open_workspace_in_ide(&mut self, workspace_id: WorkspaceId, cx: &mut Context<Self>) {
        let Some(workspace) = self.state.workspace(workspace_id) else {
            self.dispatch(
                Action::OpenWorkspaceInIdeFailed {
                    message: "Workspace not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let services = self.services.clone();
        let worktree_path = workspace.worktree_path.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(
                            async move { services.open_workspace_in_ide(worktree_path) },
                        )
                        .await;

                    let Err(message) = result else {
                        return;
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(Action::OpenWorkspaceInIdeFailed { message }, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_open_workspace_pull_request(
        &mut self,
        workspace_id: WorkspaceId,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.state.workspace(workspace_id) else {
            self.dispatch(
                Action::OpenWorkspacePullRequestFailed {
                    message: "Workspace not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let services = self.services.clone();
        let worktree_path = workspace.worktree_path.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(
                            async move { services.gh_open_pull_request(worktree_path) },
                        )
                        .await;

                    let Err(message) = result else {
                        return;
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(
                                Action::OpenWorkspacePullRequestFailed { message },
                                view_cx,
                            )
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn enqueue_context_import(
        &mut self,
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        id: u64,
        _kind: luban_domain::ContextTokenKind,
        spec: ContextImportSpec,
        cx: &mut Context<Self>,
    ) {
        let Some(agent_context) = workspace_agent_context(&self.state, workspace_id) else {
            return;
        };

        let key = (workspace_id, thread_id);
        *self.pending_context_imports.entry(key).or_insert(0) += 1;
        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let project_slug = agent_context.project_slug;
                    let workspace_name = agent_context.workspace_name;
                    let attachment_id = id;

                    let result = async_cx
                        .background_spawn(async move {
                            match spec {
                                ContextImportSpec::Image { extension, bytes } => services
                                    .store_context_image(
                                        project_slug,
                                        workspace_name,
                                        luban_domain::ContextImage { extension, bytes },
                                    ),
                                ContextImportSpec::Text { extension, text } => services
                                    .store_context_text(
                                        project_slug,
                                        workspace_name,
                                        text,
                                        extension,
                                    ),
                                ContextImportSpec::File { source_path } => services
                                    .store_context_file(project_slug, workspace_name, source_path),
                            }
                        })
                        .await;

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            if let Ok(path) = result {
                                view.dispatch(
                                    Action::ChatDraftAttachmentResolved {
                                        workspace_id,
                                        thread_id,
                                        id: attachment_id,
                                        path,
                                    },
                                    view_cx,
                                );
                            } else {
                                view.dispatch(
                                    Action::ChatDraftAttachmentFailed {
                                        workspace_id,
                                        thread_id,
                                        id: attachment_id,
                                    },
                                    view_cx,
                                );
                            }

                            if let Some(count) = view.pending_context_imports.get_mut(&key) {
                                *count = count.saturating_sub(1);
                                if *count == 0 {
                                    view.pending_context_imports.remove(&key);
                                }
                            }

                            view_cx.notify();
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn toggle_agent_item_expanded(&mut self, id: &str) {
        if !self.expanded_agent_items.insert(id.to_owned()) {
            self.expanded_agent_items.remove(id);
        }
    }

    fn run_create_workspace(&mut self, project_id: ProjectId, cx: &mut Context<Self>) {
        let Some(project) = self.state.project(project_id) else {
            self.dispatch(
                Action::WorkspaceCreateFailed {
                    project_id,
                    message: "Project not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let project_path = project.path.clone();
        let project_slug = project.slug.clone();
        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move {
                            services.create_workspace(project_path, project_slug)
                        })
                        .await;

                    let action = match result {
                        Ok(created) => Action::WorkspaceCreated {
                            project_id,
                            workspace_name: created.workspace_name,
                            branch_name: created.branch_name,
                            worktree_path: created.worktree_path,
                        },
                        Err(message) => Action::WorkspaceCreateFailed {
                            project_id,
                            message,
                        },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_archive_workspace(&mut self, workspace_id: WorkspaceId, cx: &mut Context<Self>) {
        let Some((project_path, worktree_path)) = workspace_context(&self.state, workspace_id)
        else {
            self.dispatch(
                Action::WorkspaceArchiveFailed {
                    workspace_id,
                    message: "Workspace not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move {
                            services.archive_workspace(project_path, worktree_path)
                        })
                        .await;

                    let action = match result {
                        Ok(()) => Action::WorkspaceArchived { workspace_id },
                        Err(message) => Action::WorkspaceArchiveFailed {
                            workspace_id,
                            message,
                        },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_ensure_conversation(
        &mut self,
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        cx: &mut Context<Self>,
    ) {
        let Some(agent_context) = workspace_agent_context(&self.state, workspace_id) else {
            self.dispatch(
                Action::ConversationLoadFailed {
                    workspace_id,
                    thread_id,
                    message: "Workspace not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let services = self.services.clone();
        let local_thread_id = thread_id.as_u64();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move {
                            services.ensure_conversation(
                                agent_context.project_slug,
                                agent_context.workspace_name,
                                local_thread_id,
                            )
                        })
                        .await;

                    if let Err(message) = result {
                        let _ = this.update(
                            &mut async_cx,
                            |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                                view.dispatch(
                                    Action::ConversationLoadFailed {
                                        workspace_id,
                                        thread_id,
                                        message,
                                    },
                                    view_cx,
                                )
                            },
                        );
                    }
                }
            },
        )
        .detach();
    }

    fn run_load_conversation(
        &mut self,
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        cx: &mut Context<Self>,
    ) {
        let Some(agent_context) = workspace_agent_context(&self.state, workspace_id) else {
            self.dispatch(
                Action::ConversationLoadFailed {
                    workspace_id,
                    thread_id,
                    message: "Workspace not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let services = self.services.clone();
        let local_thread_id = thread_id.as_u64();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move {
                            services.load_conversation(
                                agent_context.project_slug,
                                agent_context.workspace_name,
                                local_thread_id,
                            )
                        })
                        .await;

                    let action = match result {
                        Ok(snapshot) => Action::ConversationLoaded {
                            workspace_id,
                            thread_id,
                            snapshot,
                        },
                        Err(message) => Action::ConversationLoadFailed {
                            workspace_id,
                            thread_id,
                            message,
                        },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_load_workspace_threads(&mut self, workspace_id: WorkspaceId, cx: &mut Context<Self>) {
        let Some(agent_context) = workspace_agent_context(&self.state, workspace_id) else {
            self.dispatch(
                Action::WorkspaceThreadsLoadFailed {
                    workspace_id,
                    message: "Workspace not found".to_owned(),
                },
                cx,
            );
            return;
        };

        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move {
                            services.list_conversation_threads(
                                agent_context.project_slug,
                                agent_context.workspace_name,
                            )
                        })
                        .await;

                    let action = match result {
                        Ok(threads) => Action::WorkspaceThreadsLoaded {
                            workspace_id,
                            threads,
                        },
                        Err(message) => Action::WorkspaceThreadsLoadFailed {
                            workspace_id,
                            message,
                        },
                    };

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.dispatch(action, view_cx)
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn run_agent_turn(
        &mut self,
        workspace_id: WorkspaceId,
        thread_id: WorkspaceThreadId,
        text: String,
        run_config: AgentRunConfig,
        cx: &mut Context<Self>,
    ) {
        let Some(agent_context) = workspace_agent_context(&self.state, workspace_id) else {
            self.dispatch(
                Action::AgentTurnFinished {
                    workspace_id,
                    thread_id,
                },
                cx,
            );
            return;
        };

        let key = (workspace_id, thread_id);
        let generation = self.bump_turn_generation(key);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.turn_cancel_flags.insert(key, cancel_flag.clone());

        let remote_thread_id = self
            .state
            .workspace_thread_conversation(workspace_id, thread_id)
            .and_then(|c| c.thread_id.clone());
        let request = RunAgentTurnRequest {
            project_slug: agent_context.project_slug,
            workspace_name: agent_context.workspace_name,
            worktree_path: agent_context.worktree_path,
            thread_local_id: thread_id.as_u64(),
            thread_id: remote_thread_id,
            prompt: text,
            model: Some(run_config.model_id),
            model_reasoning_effort: Some(run_config.thinking_effort.as_str().to_owned()),
        };
        let services = self.services.clone();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let (tx, rx) = async_channel::unbounded::<CodexThreadEvent>();

                    let tx_for_events = tx.clone();
                    let tx_for_error = tx.clone();
                    let on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync> =
                        Arc::new(move |e| {
                            let _ = tx_for_events.send_blocking(e);
                        });

                    std::thread::spawn(move || {
                        let result =
                            services.run_agent_turn_streamed(request, cancel_flag, on_event);

                        if let Err(message) = result {
                            let _ = tx_for_error.send_blocking(CodexThreadEvent::Error { message });
                        }
                    });

                    drop(tx);

                    while let Ok(event) = rx.recv().await {
                        let _ = this.update(
                            &mut async_cx,
                            |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                                let current_generation =
                                    view.turn_generation.get(&key).copied().unwrap_or(0);
                                if current_generation != generation {
                                    return;
                                }

                                view.dispatch(
                                    Action::AgentEventReceived {
                                        workspace_id,
                                        thread_id: key.1,
                                        event,
                                    },
                                    view_cx,
                                );
                            },
                        );
                    }

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            let current_generation =
                                view.turn_generation.get(&key).copied().unwrap_or(0);
                            if current_generation != generation {
                                return;
                            }
                            view.dispatch(
                                Action::AgentTurnFinished {
                                    workspace_id,
                                    thread_id: key.1,
                                },
                                view_cx,
                            );
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn show_success_toast(&mut self, message: String, cx: &mut Context<Self>) {
        self.success_toast_generation = self.success_toast_generation.wrapping_add(1);
        let generation = self.success_toast_generation;
        self.success_toast_message = Some(message);
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    gpui::Timer::after(SUCCESS_TOAST_DURATION).await;
                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, cx: &mut Context<LubanRootView>| {
                            view.dismiss_success_toast(generation, cx);
                        },
                    );
                }
            },
        )
        .detach();
    }

    fn dismiss_success_toast(&mut self, generation: u64, cx: &mut Context<Self>) {
        if self.success_toast_generation != generation {
            return;
        }
        if self.success_toast_message.is_none() {
            return;
        }
        self.success_toast_message = None;
        cx.notify();
    }

    fn render_success_toast(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let Some(message) = self.success_toast_message.as_deref() else {
            return div()
                .absolute()
                .top(px(TITLEBAR_HEIGHT))
                .left_0()
                .w(px(0.0))
                .h(px(0.0))
                .debug_selector(|| "success-toast".to_owned())
                .into_any_element();
        };

        div()
            .absolute()
            .top(px(TITLEBAR_HEIGHT))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .pt_2()
            .debug_selector(|| "success-toast".to_owned())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.success_hover)
                    .bg(theme.success)
                    .text_color(theme.success_foreground)
                    .child(Icon::new(IconName::Check))
                    .child(div().text_sm().child(message.to_owned())),
            )
            .into_any_element()
    }
}

impl gpui::Render for LubanRootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_terminal_resize_observer(window, cx);

        let theme = cx.theme();
        let background = theme.background;
        let foreground = theme.foreground;
        let transparent = theme.transparent;
        let muted = theme.muted;
        let is_dashboard = self.state.main_pane == MainPane::Dashboard;
        let viewport_width = window.viewport_size().width;
        let sidebar_width = self.sidebar_width(window);
        if !is_dashboard {
            self.last_sidebar_width = sidebar_width;
        }
        let sidebar_width_for_titlebar = if is_dashboard {
            let absolute_max = viewport_width - px(SIDEBAR_RESIZER_WIDTH);
            self.last_sidebar_width.min(absolute_max).max(px(0.0))
        } else {
            sidebar_width
        };
        let sidebar_width_for_layout = if is_dashboard { px(0.0) } else { sidebar_width };
        let right_pane_width = if is_dashboard {
            px(0.0)
        } else {
            self.right_pane_width(window, sidebar_width_for_layout)
        };
        let should_render_right_pane = self.terminal_enabled
            && !is_dashboard
            && self.state.right_pane == RightPane::Terminal
            && right_pane_width > px(0.0);
        let view_handle = cx.entity().downgrade();

        let content = if is_dashboard {
            min_height_zero(div().flex_1().flex().child(self.render_main(window, cx)))
        } else {
            min_height_zero(
                div()
                    .flex_1()
                    .flex()
                    .child(render_sidebar(
                        cx,
                        &self.state,
                        sidebar_width_for_layout,
                        &self.workspace_pull_request_numbers,
                        &self.projects_scroll_handle,
                        self.debug_scrollbar_enabled,
                    ))
                    .child(
                        div()
                            .w(px(SIDEBAR_RESIZER_WIDTH))
                            .h_full()
                            .flex_shrink_0()
                            .cursor(CursorStyle::ResizeLeftRight)
                            .id("sidebar-resizer")
                            .debug_selector(|| "sidebar-resizer".to_owned())
                            .bg(transparent)
                            .hover(move |s| s.bg(muted))
                            .on_drag(SidebarResizeDrag, {
                                let view_handle = view_handle.clone();
                                move |_, _offset, window, app| {
                                    let start_mouse_x = window.mouse_position().x;
                                    let start_width = sidebar_width_for_layout;
                                    let _ = view_handle.update(app, |view, cx| {
                                        view.sidebar_resize = Some(SidebarResizeState {
                                            start_mouse_x,
                                            start_width,
                                        });
                                        view.sidebar_width_preview = Some(start_width);
                                        cx.notify();
                                    });
                                    app.new(|_| SidebarResizeGhost)
                                }
                            })
                            .on_drag_move::<SidebarResizeDrag>({
                                let view_handle = view_handle.clone();
                                move |event, window, app| {
                                    let mouse_x = event.event.position.x;
                                    let viewport_width = window.viewport_size().width;
                                    let _ = view_handle.update(app, |view, cx| {
                                        let Some(state) = view.sidebar_resize else {
                                            return;
                                        };
                                        let desired =
                                            state.start_width + (mouse_x - state.start_mouse_x);
                                        let clamped =
                                            view.clamp_sidebar_width(desired, viewport_width);
                                        view.sidebar_width_preview = Some(clamped);
                                        cx.notify();
                                    });
                                }
                            })
                            .on_mouse_up(MouseButton::Left, {
                                let view_handle = view_handle.clone();
                                move |_, window, app| {
                                    let viewport_width = window.viewport_size().width;
                                    let _ = view_handle.update(app, |view, cx| {
                                        view.finish_sidebar_resize(viewport_width, cx);
                                        view.resize_workspace_terminals(window, cx);
                                    });
                                }
                            })
                            .on_mouse_up_out(MouseButton::Left, {
                                let view_handle = view_handle.clone();
                                move |_, window, app| {
                                    let viewport_width = window.viewport_size().width;
                                    let _ = view_handle.update(app, |view, cx| {
                                        view.finish_sidebar_resize(viewport_width, cx);
                                        view.resize_workspace_terminals(window, cx);
                                    });
                                }
                            }),
                    )
                    .child(self.render_main(window, cx))
                    .when(should_render_right_pane, |s| {
                        let resizer = div()
                            .w(px(TERMINAL_PANE_RESIZER_WIDTH))
                            .h_full()
                            .flex_shrink_0()
                            .cursor(CursorStyle::ResizeLeftRight)
                            .id("terminal-pane-resizer")
                            .debug_selector(|| "terminal-pane-resizer".to_owned())
                            .bg(transparent)
                            .hover(move |s| s.bg(muted))
                            .on_drag(TerminalPaneResizeDrag, {
                                let view_handle = view_handle.clone();
                                move |_, _offset, window, app| {
                                    let start_mouse_x = window.mouse_position().x;
                                    let start_width = right_pane_width;
                                    let _ = view_handle.update(app, |view, cx| {
                                        view.terminal_pane_resize = Some(TerminalPaneResizeState {
                                            start_mouse_x,
                                            start_width,
                                        });
                                        view.terminal_pane_width_preview = Some(start_width);
                                        cx.notify();
                                    });
                                    app.new(|_| TerminalPaneResizeGhost)
                                }
                            })
                            .on_drag_move::<TerminalPaneResizeDrag>({
                                let view_handle = view_handle.clone();
                                move |event, window, app| {
                                    let mouse_x = event.event.position.x;
                                    let viewport_width = window.viewport_size().width;
                                    let _ = view_handle.update(app, |view, cx| {
                                        let Some(state) = view.terminal_pane_resize else {
                                            return;
                                        };
                                        let desired =
                                            state.start_width - (mouse_x - state.start_mouse_x);
                                        let clamped = view.clamp_terminal_pane_width(
                                            desired,
                                            viewport_width,
                                            sidebar_width,
                                        );
                                        view.terminal_pane_width_preview = Some(clamped);
                                        cx.notify();
                                    });
                                }
                            })
                            .on_mouse_up(MouseButton::Left, {
                                let view_handle = view_handle.clone();
                                move |_, window, app| {
                                    let viewport_width = window.viewport_size().width;
                                    let _ = view_handle.update(app, |view, cx| {
                                        view.finish_terminal_pane_resize(
                                            viewport_width,
                                            sidebar_width,
                                            cx,
                                        );
                                        view.resize_workspace_terminals(window, cx);
                                    });
                                }
                            })
                            .on_mouse_up_out(MouseButton::Left, {
                                let view_handle = view_handle.clone();
                                move |_, window, app| {
                                    let viewport_width = window.viewport_size().width;
                                    let _ = view_handle.update(app, |view, cx| {
                                        view.finish_terminal_pane_resize(
                                            viewport_width,
                                            sidebar_width,
                                            cx,
                                        );
                                        view.resize_workspace_terminals(window, cx);
                                    });
                                }
                            });

                        s.child(resizer)
                            .child(self.render_right_pane(right_pane_width, window, cx))
                    }),
            )
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(background)
            .text_color(foreground)
            .child(render_titlebar(
                cx,
                &self.state,
                sidebar_width_for_titlebar,
                right_pane_width,
                self.terminal_enabled,
            ))
            .child(content)
            .child(self.render_success_toast(cx))
    }
}

impl LubanRootView {
    fn render_main(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let view_handle = cx.entity().downgrade();

        let content = match self.state.main_pane {
            MainPane::None => {
                self.last_chat_workspace_id = None;
                self.last_chat_item_count = 0;

                div().flex_1().into_any_element()
            }
            MainPane::Dashboard => {
                if self.state.dashboard_preview_workspace_id.is_none() {
                    self.last_chat_workspace_id = None;
                    self.last_chat_item_count = 0;
                }

                self.render_dashboard(view_handle, window, cx)
            }
            MainPane::ProjectSettings(project_id) => {
                self.last_chat_workspace_id = None;
                self.last_chat_item_count = 0;

                let title = self
                    .state
                    .project(project_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "Project".to_owned());

                div()
                    .flex_1()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_w(px(900.0))
                    .mx_auto()
                    .child(div().text_lg().child(title))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("No settings yet."),
                    )
                    .into_any_element()
            }
            MainPane::Workspace(workspace_id) => {
                if self.state.workspace(workspace_id).is_none() {
                    return div()
                        .flex_1()
                        .p_3()
                        .child(
                            div()
                                .text_color(cx.theme().danger_foreground)
                                .child("Workspace not found"),
                        )
                        .into_any_element();
                }

                let input_state = self.ensure_chat_input(window, cx);

                let conversation = self.state.workspace_conversation(workspace_id);
                let thread_id = self.active_thread_id_for_workspace(workspace_id);
                let chat_key = (workspace_id, thread_id);
                let entries: &[luban_domain::ConversationEntry] =
                    conversation.map(|c| c.entries.as_slice()).unwrap_or(&[]);
                let entries_len = conversation.map(|c| c.entries.len()).unwrap_or(0);
                let ordered_in_progress_items: Vec<&CodexThreadItem> = conversation
                    .map(|c| {
                        c.in_progress_order
                            .iter()
                            .filter_map(|id| c.in_progress_items.get(id))
                            .collect()
                    })
                    .unwrap_or_default();
                let run_status = conversation
                    .map(|c| c.run_status)
                    .unwrap_or(OperationStatus::Idle);
                let queued_prompts: Vec<luban_domain::QueuedPrompt> = conversation
                    .map(|c| c.pending_prompts.iter().cloned().collect())
                    .unwrap_or_default();
                let queue_paused = conversation.map(|c| c.queue_paused).unwrap_or(false);
                let _thread_id = conversation.and_then(|c| c.thread_id.as_deref());
                let (model_id, thinking_effort) = match (conversation, run_status) {
                    (Some(conversation), OperationStatus::Running) => conversation
                        .current_run_config
                        .as_ref()
                        .map(|cfg| (cfg.model_id.clone(), cfg.thinking_effort))
                        .unwrap_or_else(|| {
                            (
                                conversation.agent_model_id.clone(),
                                conversation.thinking_effort,
                            )
                        }),
                    (Some(conversation), _) => (
                        conversation.agent_model_id.clone(),
                        conversation.thinking_effort,
                    ),
                    (None, _) => (
                        default_agent_model_id().to_owned(),
                        default_thinking_effort(),
                    ),
                };

                let is_running = run_status == OperationStatus::Running;
                let chat_target_changed = self.last_chat_workspace_id != Some(chat_key);
                if chat_target_changed {
                    let offset_y10 = self
                        .state
                        .workspace_chat_scroll_y10
                        .get(&(workspace_id, thread_id))
                        .copied()
                        .unwrap_or(0);
                    self.chat_scroll_handle
                        .set_offset(point(px(0.0), px(offset_y10 as f32 / 10.0)));

                    let saved_draft = conversation.map(|c| c.draft.clone()).unwrap_or_default();
                    let current_value = input_state.read(cx).value().to_owned();
                    let should_move_cursor = !saved_draft.is_empty();
                    if current_value != saved_draft.as_str() || should_move_cursor {
                        input_state.update(cx, |state, cx| {
                            if current_value != saved_draft.as_str() {
                                state.set_value(&saved_draft, window, cx);
                            }

                            if should_move_cursor {
                                let end = state.text().offset_to_position(state.text().len());
                                if state.cursor_position() != end {
                                    state.set_cursor_position(end, window, cx);
                                }
                            }
                        });
                    }
                }

                if chat_target_changed {
                    let offset_y10 = quantize_pixels_y10(self.chat_scroll_handle.offset().y);
                    self.chat_last_observed_scroll_offset_y10
                        .insert(chat_key, offset_y10);
                }
                update_chat_follow_state(
                    chat_key,
                    &self.chat_scroll_handle,
                    &mut self.chat_follow_tail,
                    &mut self.chat_last_observed_scroll_offset_y10,
                );

                let theme = cx.theme();

                let draft_text = conversation.map(|c| c.draft.clone()).unwrap_or_default();
                let draft_attachments: Vec<luban_domain::DraftAttachment> = conversation
                    .map(|c| c.draft_attachments.clone())
                    .unwrap_or_default();
                let composed = compose_user_message_text(&draft_text, &draft_attachments);
                let pending_context_imports = self
                    .pending_context_imports
                    .get(&chat_key)
                    .copied()
                    .unwrap_or(0);
                let send_disabled = pending_context_imports > 0 || composed.trim().is_empty();
                let running_elapsed = if is_running {
                    self.running_turn_started_at
                        .get(&chat_key)
                        .map(|t| t.elapsed())
                } else {
                    None
                };
                let tail_duration = running_elapsed.map(|elapsed| (elapsed, true)).or_else(|| {
                    self.pending_turn_durations
                        .get(&chat_key)
                        .copied()
                        .map(|elapsed| (elapsed, false))
                });

                let expanded = self.expanded_agent_items.clone();
                let expanded_turns = self.expanded_agent_turns.clone();
                let has_in_progress_items = !ordered_in_progress_items.is_empty();
                let force_expand_current_turn = is_running || has_in_progress_items;

                let running_turn_summary_items: Vec<&CodexThreadItem> = if force_expand_current_turn
                {
                    let turn_count = agent_turn_count(entries);
                    if self.running_turn_user_message_count.get(&chat_key).copied()
                        != Some(turn_count)
                    {
                        self.running_turn_user_message_count
                            .insert(chat_key, turn_count);
                        self.running_turn_summary_order.insert(chat_key, Vec::new());
                    }

                    let order = self.running_turn_summary_order.entry(chat_key).or_default();

                    if let Some(conversation) = conversation {
                        for id in conversation.in_progress_order.iter() {
                            let Some(item) = conversation.in_progress_items.get(id) else {
                                continue;
                            };
                            if !codex_item_is_summary_item(item) {
                                continue;
                            }
                            if order.iter().any(|v| v == id) {
                                continue;
                            }
                            order.push(id.clone());
                        }
                    }

                    if let Some(last_user_message_index) = entries.iter().rposition(|e| {
                        matches!(e, luban_domain::ConversationEntry::UserMessage { .. })
                    }) {
                        for entry in &entries[(last_user_message_index + 1)..] {
                            let luban_domain::ConversationEntry::CodexItem { item } = entry else {
                                continue;
                            };
                            let item = item.as_ref();
                            if !codex_item_is_summary_item(item) {
                                continue;
                            }
                            let id = codex_item_id(item);
                            if order.iter().any(|v| v == id) {
                                continue;
                            }
                            order.push(id.to_owned());
                        }
                    }

                    let order_snapshot = order.clone();
                    let mut items = Vec::new();
                    if let Some(conversation) = conversation {
                        for id in &order_snapshot {
                            if let Some(item) = conversation.in_progress_items.get(id) {
                                if codex_item_is_summary_item(item) {
                                    items.push(item);
                                }
                                continue;
                            }

                            if let Some(item) = find_summary_item_in_current_turn(entries, id) {
                                items.push(item);
                            }
                        }
                    }
                    items
                } else {
                    self.running_turn_user_message_count.remove(&chat_key);
                    self.running_turn_summary_order.remove(&chat_key);
                    Vec::new()
                };

                let history_children = build_workspace_history_children(
                    entries,
                    theme,
                    &expanded,
                    &expanded_turns,
                    self.chat_column_width,
                    &view_handle,
                    &running_turn_summary_items,
                    force_expand_current_turn,
                );

                if self.should_chat_follow_tail(chat_key) {
                    self.chat_scroll_handle.scroll_to_bottom();
                }
                self.last_chat_workspace_id = Some(chat_key);
                self.last_chat_item_count = entries_len;

                let debug_layout_enabled = self.debug_layout_enabled;
                let history = min_height_zero(
                    div()
                        .flex_1()
                        .id("workspace-chat-scroll")
                        .overflow_scroll()
                        .track_scroll(&self.chat_scroll_handle)
                        .overflow_x_hidden()
                        .when(debug_layout_enabled, |s| {
                            s.on_prepaint(move |bounds, window, _app| {
                                debug_layout::record(
                                    "workspace-chat-scroll",
                                    window.viewport_size(),
                                    bounds,
                                );
                            })
                        })
                        .w_full()
                        .px_4()
                        .py_3()
                        .child(min_width_zero(
                            div()
                                .debug_selector(|| "workspace-chat-column".to_owned())
                                .on_prepaint({
                                    let view_handle = view_handle.clone();
                                    move |bounds, _window, app| {
                                        let width = bounds.size.width;
                                        let _ = view_handle.update(app, |view, cx| {
                                            let should_update = match view.chat_column_width {
                                                Some(prev) => (prev - width).abs() > px(0.5),
                                                None => true,
                                            };
                                            if should_update {
                                                view.chat_column_width = Some(width);
                                                cx.notify();
                                            }
                                        });
                                    }
                                })
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .whitespace_normal()
                                .pb_2()
                                .children(history_children)
                                .when_some(tail_duration, |s, (elapsed, running)| {
                                    s.child(
                                        div()
                                            .debug_selector(|| "chat-tail-turn-duration".to_owned())
                                            .child(render_turn_duration_row(
                                                theme, elapsed, running,
                                            )),
                                    )
                                }),
                        )),
                );

                let queue_panel = if !queued_prompts.is_empty() {
                    let theme = cx.theme();
                    let view_handle = view_handle.clone();
                    let input_state = input_state.clone();

                    let toolbar = div()
                        .h(px(24.0))
                        .w_full()
                        .px_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(div().text_xs().text_color(theme.muted_foreground).child(
                            if queue_paused {
                                "Queued • Paused"
                            } else {
                                "Queued"
                            },
                        ))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .when(queue_paused && !is_running, |s| {
                                    let view_handle = view_handle.clone();
                                    s.child(
                                        Button::new("queued-resume")
                                            .primary()
                                            .compact()
                                            .icon(IconName::Redo2)
                                            .tooltip("Resume queued messages")
                                            .on_click(move |_, _, app| {
                                                let _ = view_handle.update(app, |view, cx| {
                                                    view.dispatch(
                                                        Action::ResumeQueuedPrompts {
                                                            workspace_id,
                                                            thread_id,
                                                        },
                                                        cx,
                                                    );
                                                });
                                            }),
                                    )
                                })
                                .child(
                                    Button::new("queued-clear-all")
                                        .ghost()
                                        .compact()
                                        .icon(IconName::Delete)
                                        .tooltip("Clear queued messages")
                                        .on_click({
                                            let view_handle = view_handle.clone();
                                            move |_, window, app| {
                                                let receiver = window.prompt(
                                                    PromptLevel::Warning,
                                                    "Clear queued messages?",
                                                    Some("This will remove all queued messages."),
                                                    &[
                                                        PromptButton::ok("Clear"),
                                                        PromptButton::cancel("Cancel"),
                                                    ],
                                                    app,
                                                );

                                                let view_handle = view_handle.clone();
                                                app.spawn(move |cx: &mut gpui::AsyncApp| {
                                                    let mut async_cx = cx.clone();
                                                    async move {
                                                        let Ok(choice) = receiver.await else {
                                                            return;
                                                        };
                                                        if choice != 0 {
                                                            return;
                                                        }
                                                        let _ = view_handle.update(
                                                            &mut async_cx,
                                                            |view: &mut LubanRootView, view_cx| {
                                                                view.dispatch(
                                                                    Action::ClearQueuedPrompts {
                                                                        workspace_id,
                                                                        thread_id,
                                                                    },
                                                                    view_cx,
                                                                );
                                                            },
                                                        );
                                                    }
                                                })
                                                .detach();
                                            }
                                        }),
                                ),
                        );

                    let content = div().pt_2().px_2().flex().flex_col().gap_1().children(
                        queued_prompts.iter().enumerate().map(|(idx, queued)| {
                            let view_handle_for_edit = view_handle.clone();
                            let view_handle_for_remove = view_handle.clone();
                            let input_state = input_state.clone();
                            let text = queued.text.clone();
                            div()
                                .h(px(28.0))
                                .w_full()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .truncate()
                                        .text_color(theme.muted_foreground)
                                        .child(text.clone()),
                                )
                                .child(
                                    Button::new(format!("queued-edit-{idx}"))
                                        .ghost()
                                        .compact()
                                        .icon(IconName::Replace)
                                        .tooltip("Move to input and remove from queue")
                                        .on_click(move |_, window, app| {
                                            let (draft_text, attachments) =
                                                draft_text_and_attachments_from_message_text(&text);
                                            input_state.update(app, |state, cx| {
                                                state.set_value(&draft_text, window, cx);
                                                let end = state
                                                    .text()
                                                    .offset_to_position(state.text().len());
                                                state.set_cursor_position(end, window, cx);
                                            });
                                            let _ = view_handle_for_edit.update(app, |view, cx| {
                                                let existing_ids = view
                                                    .state
                                                    .workspace_conversation(workspace_id)
                                                    .map(|c| {
                                                        c.draft_attachments
                                                            .iter()
                                                            .map(|a| a.id)
                                                            .collect::<Vec<_>>()
                                                    })
                                                    .unwrap_or_default();
                                                for id in existing_ids {
                                                    view.dispatch(
                                                        Action::ChatDraftAttachmentRemoved {
                                                            workspace_id,
                                                            thread_id,
                                                            id,
                                                        },
                                                        cx,
                                                    );
                                                }
                                                view.dispatch(
                                                    Action::ChatDraftChanged {
                                                        workspace_id,
                                                        thread_id,
                                                        text: draft_text.clone(),
                                                    },
                                                    cx,
                                                );
                                                for (kind, anchor, path) in attachments {
                                                    let id = next_pending_context_id();
                                                    view.dispatch(
                                                        Action::ChatDraftAttachmentAdded {
                                                            workspace_id,
                                                            thread_id,
                                                            id,
                                                            kind,
                                                            anchor,
                                                        },
                                                        cx,
                                                    );
                                                    view.dispatch(
                                                        Action::ChatDraftAttachmentResolved {
                                                            workspace_id,
                                                            thread_id,
                                                            id,
                                                            path,
                                                        },
                                                        cx,
                                                    );
                                                }
                                                view.dispatch(
                                                    Action::RemoveQueuedPrompt {
                                                        workspace_id,
                                                        thread_id,
                                                        index: idx,
                                                    },
                                                    cx,
                                                );
                                            });
                                        }),
                                )
                                .child({
                                    Button::new(format!("queued-remove-{idx}"))
                                        .ghost()
                                        .compact()
                                        .icon(IconName::Close)
                                        .tooltip("Remove from queue")
                                        .on_click(move |_, _, app| {
                                            let _ =
                                                view_handle_for_remove.update(app, |view, cx| {
                                                    view.dispatch(
                                                        Action::RemoveQueuedPrompt {
                                                            workspace_id,
                                                            thread_id,
                                                            index: idx,
                                                        },
                                                        cx,
                                                    );
                                                });
                                        })
                                })
                                .into_any_element()
                        }),
                    );

                    div()
                        .w_full()
                        .child(toolbar)
                        .child(content)
                        .into_any_element()
                } else {
                    div().hidden().into_any_element()
                };

                let debug_layout_enabled = self.debug_layout_enabled;
                let composer = chat::composer_view::render_chat_composer(
                    chat::composer_view::ChatComposerViewProps {
                        debug_layout_enabled,
                        workspace_id,
                        thread_id,
                        input_state: input_state.clone(),
                        draft_attachments: &draft_attachments,
                        model_id: &model_id,
                        thinking_effort,
                        composed: &composed,
                        send_disabled,
                        is_running,
                        queue_panel,
                        view_handle: &view_handle,
                        theme,
                    },
                );

                min_height_zero(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .child(self.render_workspace_thread_tabs(
                            workspace_id,
                            thread_id,
                            &view_handle,
                            cx,
                        ))
                        .child(history)
                        .child(composer),
                )
                .into_any_element()
            }
        };

        let theme = cx.theme();
        let debug_layout_enabled = self.debug_layout_enabled;

        min_width_zero(min_height_zero(
            div()
                .debug_selector(|| "main-pane".to_owned())
                .when(debug_layout_enabled, |s| {
                    s.on_prepaint(move |bounds, window, _app| {
                        debug_layout::record("main-pane", window.viewport_size(), bounds);
                    })
                })
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .bg(theme.background)
                .when_some(self.state.last_error.clone(), |s, message| {
                    let theme = cx.theme();
                    let view_handle = cx.entity().downgrade();
                    s.child(
                        div()
                            .mx_4()
                            .mt_3()
                            .p_3()
                            .rounded_md()
                            .bg(theme.danger)
                            .border_1()
                            .border_color(theme.danger_hover)
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(theme.danger_foreground)
                            .child(div().child(message))
                            .child(
                                div().debug_selector(|| "error-dismiss".to_owned()).child(
                                    Button::new("error-dismiss")
                                        .ghost()
                                        .compact()
                                        .label("Dismiss")
                                        .on_click(move |_, _, app| {
                                            let _ = view_handle.update(app, |view, cx| {
                                                view.dispatch(Action::ClearError, cx);
                                            });
                                        }),
                                ),
                            ),
                    )
                })
                .child(content),
        ))
        .into_any_element()
    }
}

fn main_pane_title(state: &AppState, pane: MainPane) -> String {
    match pane {
        MainPane::None => String::new(),
        MainPane::Dashboard => "Dashboard".to_owned(),
        MainPane::ProjectSettings(project_id) => state
            .project(project_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Project Settings".to_owned()),
        MainPane::Workspace(workspace_id) => state
            .workspace(workspace_id)
            .map(|w| w.workspace_name.clone())
            .unwrap_or_else(|| "Workspace".to_owned()),
    }
}

fn render_conversation_entry(
    entry_index: usize,
    entry: &luban_domain::ConversationEntry,
    theme: &gpui_component::Theme,
    expanded_items: &HashSet<String>,
    chat_column_width: Option<Pixels>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    match entry {
        luban_domain::ConversationEntry::UserMessage { text } => {
            let is_short_single_line = text.lines().nth(1).is_none() && text.chars().count() <= 80;
            let bubble_max_w = chat_column_width
                .map(|w| (w - px(48.0)).min(px(680.0)).max(px(0.0)))
                .unwrap_or(px(680.0));
            let wrap_width = if is_short_single_line {
                None
            } else {
                Some((bubble_max_w - px(32.0)).max(px(0.0)))
            };
            let message = user_message_view_with_context_tokens(
                entry_index,
                text,
                wrap_width,
                theme.foreground,
                theme.border,
            );
            let copy_text = text.to_owned();
            let copy_button = copy_to_clipboard_button(
                format!("conversation-user-copy-button-{entry_index}"),
                copy_text,
                theme,
            );
            let bubble = min_width_zero(
                div()
                    .max_w(bubble_max_w)
                    .overflow_x_hidden()
                    .p_2()
                    .rounded_md()
                    .bg(theme.accent)
                    .border_1()
                    .border_color(theme.border)
                    .child(min_width_zero(
                        div().w_full().whitespace_normal().child(message),
                    )),
            );

            div()
                .debug_selector(move || format!("conversation-user-row-{entry_index}"))
                .id(format!("conversation-user-{entry_index}"))
                .w_full()
                .overflow_x_hidden()
                .flex()
                .flex_row()
                .justify_end()
                .child(
                    div()
                        .max_w(bubble_max_w)
                        .flex()
                        .flex_col()
                        .items_end()
                        .child(bubble.debug_selector(move || {
                            format!("conversation-user-bubble-{entry_index}")
                        }))
                        .child(
                            div()
                                .pt_1()
                                .flex()
                                .items_center()
                                .justify_end()
                                .child(copy_button),
                        ),
                )
                .into_any_element()
        }
        luban_domain::ConversationEntry::CodexItem { item } => div()
            .id(format!(
                "conversation-codex-{}-{entry_index}",
                codex_item_id(item)
            ))
            .w_full()
            .child(render_codex_item(
                &format!("entry-{entry_index}-{}", codex_item_id(item.as_ref())),
                item.as_ref(),
                theme,
                false,
                expanded_items,
                chat_column_width,
                view_handle,
            ))
            .into_any_element(),
        luban_domain::ConversationEntry::TurnUsage { usage: _ } => div()
            .id(format!("conversation-usage-{entry_index}"))
            .hidden()
            .into_any_element(),
        luban_domain::ConversationEntry::TurnDuration { duration_ms } => div()
            .debug_selector(move || format!("turn-duration-{entry_index}"))
            .id(format!("conversation-duration-{entry_index}"))
            .child(render_turn_duration_row(
                theme,
                Duration::from_millis(*duration_ms),
                false,
            ))
            .into_any_element(),
        luban_domain::ConversationEntry::TurnCanceled => div()
            .id(format!("conversation-canceled-{entry_index}"))
            .p_2()
            .rounded_md()
            .bg(theme.muted)
            .border_1()
            .border_color(theme.border)
            .text_color(theme.muted_foreground)
            .child(div().child("Canceled"))
            .into_any_element(),
        luban_domain::ConversationEntry::TurnError { message } => div()
            .id(format!("conversation-error-{entry_index}"))
            .p_2()
            .rounded_md()
            .bg(theme.danger)
            .border_1()
            .border_color(theme.danger_hover)
            .text_color(theme.danger_foreground)
            .child(div().child(message.clone()))
            .into_any_element(),
    }
}

fn copy_to_clipboard_button(
    debug_id: String,
    copy_text: String,
    theme: &gpui_component::Theme,
) -> AnyElement {
    let icon = Icon::new(IconName::Copy)
        .with_size(Size::Small)
        .text_color(theme.muted_foreground);

    let debug_id_for_render = debug_id.clone();
    div()
        .debug_selector(move || debug_id_for_render.clone())
        .w(px(24.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_, _, app| {
            app.write_to_clipboard(gpui::ClipboardItem::new_string(copy_text.clone()));
        })
        .child(icon)
        .into_any_element()
}

fn user_message_view_with_context_tokens(
    entry_index: usize,
    text: &str,
    wrap_width: Option<Pixels>,
    text_color: gpui::Hsla,
    border_color: gpui::Hsla,
) -> AnyElement {
    let tokens = luban_domain::find_context_tokens(text);
    if tokens.is_empty() {
        return chat_message_view(
            &format!("user-message-{entry_index}"),
            text,
            wrap_width,
            text_color,
        );
    }

    let mut children: Vec<AnyElement> = Vec::new();
    let mut cursor = 0usize;

    for (attachment_index, token) in tokens.into_iter().enumerate() {
        if token.range.start > cursor {
            let segment = &text[cursor..token.range.start];
            if !segment.trim().is_empty() {
                children.push(chat_message_view(
                    &format!("user-message-{entry_index}-seg-{attachment_index}"),
                    segment,
                    wrap_width,
                    text_color,
                ));
            }
        }

        let debug_id = format!("conversation-user-attachment-{entry_index}-{attachment_index}");
        let attachment = match token.kind {
            luban_domain::ContextTokenKind::Image => {
                let path = token.path;
                gpui::img(path)
                    .w_full()
                    .h(px(220.0))
                    .rounded_md()
                    .border_1()
                    .border_color(border_color)
                    .object_fit(gpui::ObjectFit::Contain)
                    .with_loading(|| Spinner::new().with_size(Size::Small).into_any_element())
                    .with_fallback(|| {
                        div()
                            .px_2()
                            .py_2()
                            .rounded_md()
                            .border_1()
                            .child("Missing image")
                            .into_any_element()
                    })
                    .into_any_element()
            }
            luban_domain::ContextTokenKind::Text | luban_domain::ContextTokenKind::File => {
                let filename = token
                    .path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "attachment".to_owned());
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(border_color)
                    .child(format!("Attachment: {filename}"))
                    .into_any_element()
            }
        };

        children.push(
            div()
                .debug_selector(move || debug_id.clone())
                .child(attachment)
                .into_any_element(),
        );

        cursor = token.range.end;
    }

    if cursor < text.len() {
        let tail = &text[cursor..];
        if !tail.trim().is_empty() {
            children.push(chat_message_view(
                &format!("user-message-{entry_index}-tail"),
                tail,
                wrap_width,
                text_color,
            ));
        }
    }

    div()
        .id(format!("user-message-{entry_index}-with-context"))
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .children(children)
        .into_any_element()
}

fn min_width_zero<E: gpui::Styled>(mut element: E) -> E {
    element.style().min_size.width = Some(px(0.0).into());
    element
}

fn min_height_zero<E: gpui::Styled>(mut element: E) -> E {
    element.style().min_size.height = Some(px(0.0).into());
    element
}

fn quantize_pixels_y10(pixels: Pixels) -> i32 {
    (f32::from(pixels) * 10.0).round() as i32
}

fn update_chat_follow_state(
    chat_key: WorkspaceThreadKey,
    scroll_handle: &gpui::ScrollHandle,
    chat_follow_tail: &mut HashMap<WorkspaceThreadKey, bool>,
    chat_last_observed_scroll_offset_y10: &mut HashMap<WorkspaceThreadKey, i32>,
) {
    let offset_y10 = quantize_pixels_y10(scroll_handle.offset().y);
    let max_y10 = quantize_pixels_y10(scroll_handle.max_offset().height);
    let near_bottom = LubanRootView::is_chat_near_bottom(offset_y10, max_y10);

    let previous_offset_y10 = chat_last_observed_scroll_offset_y10.get(&chat_key).copied();
    let follow = chat_follow_tail.get(&chat_key).copied().unwrap_or(true);

    if follow {
        let user_scrolled_up = previous_offset_y10
            .map(|prev| offset_y10 > prev + CHAT_SCROLL_USER_SCROLL_UP_THRESHOLD_Y10)
            .unwrap_or(false);
        if user_scrolled_up && !near_bottom {
            chat_follow_tail.insert(chat_key, false);
        } else {
            chat_follow_tail.insert(chat_key, true);
        }
    } else if near_bottom {
        chat_follow_tail.insert(chat_key, true);
    }

    chat_last_observed_scroll_offset_y10.insert(chat_key, offset_y10);
}

mod debug_layout {
    use gpui::{Bounds, Pixels, Size};
    use std::{
        collections::HashMap,
        sync::{Mutex, OnceLock},
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct LayoutSample {
        viewport_w10: i32,
        viewport_h10: i32,
        x10: i32,
        y10: i32,
        w10: i32,
        h10: i32,
    }

    impl LayoutSample {
        fn new(viewport: Size<Pixels>, bounds: Bounds<Pixels>) -> Self {
            Self {
                viewport_w10: quantize(viewport.width),
                viewport_h10: quantize(viewport.height),
                x10: quantize(bounds.origin.x),
                y10: quantize(bounds.origin.y),
                w10: quantize(bounds.size.width),
                h10: quantize(bounds.size.height),
            }
        }

        fn to_f32(v10: i32) -> f32 {
            v10 as f32 / 10.0
        }
    }

    fn quantize(pixels: Pixels) -> i32 {
        (f32::from(pixels) * 10.0).round() as i32
    }

    fn store() -> &'static Mutex<HashMap<&'static str, LayoutSample>> {
        static STORE: OnceLock<Mutex<HashMap<&'static str, LayoutSample>>> = OnceLock::new();
        STORE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub(super) fn enabled_from_env() -> bool {
        parse_enabled(std::env::var("LUBAN_DEBUG_LAYOUT").ok().as_deref())
    }

    pub(super) fn parse_enabled(value: Option<&str>) -> bool {
        let Some(raw) = value else {
            return false;
        };

        let normalized = raw.trim().to_ascii_lowercase();
        matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
    }

    pub(super) fn record(label: &'static str, viewport: Size<Pixels>, bounds: Bounds<Pixels>) {
        let sample = LayoutSample::new(viewport, bounds);
        let mut map = store().lock().unwrap();

        if map.get(label).copied() == Some(sample) {
            return;
        }
        map.insert(label, sample);

        eprintln!(
            "layout {label} viewport={:.1}x{:.1} bounds=({:.1},{:.1}) {:.1}x{:.1}",
            LayoutSample::to_f32(sample.viewport_w10),
            LayoutSample::to_f32(sample.viewport_h10),
            LayoutSample::to_f32(sample.x10),
            LayoutSample::to_f32(sample.y10),
            LayoutSample::to_f32(sample.w10),
            LayoutSample::to_f32(sample.h10),
        );
    }
}

mod debug_scrollbar {
    use gpui::{Bounds, Pixels, ScrollHandle, Size};
    use std::{
        collections::HashMap,
        sync::{Mutex, OnceLock},
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct ScrollSample {
        viewport_w10: i32,
        viewport_h10: i32,
        x10: i32,
        y10: i32,
        w10: i32,
        h10: i32,
        offset_x10: i32,
        offset_y10: i32,
        max_x10: i32,
        max_y10: i32,
    }

    impl ScrollSample {
        fn new(viewport: Size<Pixels>, bounds: Bounds<Pixels>, handle: &ScrollHandle) -> Self {
            let offset = handle.offset();
            let max = handle.max_offset();
            Self {
                viewport_w10: quantize(viewport.width),
                viewport_h10: quantize(viewport.height),
                x10: quantize(bounds.origin.x),
                y10: quantize(bounds.origin.y),
                w10: quantize(bounds.size.width),
                h10: quantize(bounds.size.height),
                offset_x10: quantize(offset.x),
                offset_y10: quantize(offset.y),
                max_x10: quantize(max.width),
                max_y10: quantize(max.height),
            }
        }

        fn to_f32(v10: i32) -> f32 {
            v10 as f32 / 10.0
        }
    }

    fn quantize(pixels: Pixels) -> i32 {
        (f32::from(pixels) * 10.0).round() as i32
    }

    fn store() -> &'static Mutex<HashMap<&'static str, ScrollSample>> {
        static STORE: OnceLock<Mutex<HashMap<&'static str, ScrollSample>>> = OnceLock::new();
        STORE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub(super) fn enabled_from_env() -> bool {
        parse_enabled(std::env::var("LUBAN_DEBUG_SCROLLBAR").ok().as_deref())
    }

    fn parse_enabled(value: Option<&str>) -> bool {
        let Some(raw) = value else {
            return false;
        };
        let normalized = raw.trim().to_ascii_lowercase();
        matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
    }

    pub(super) fn record(
        label: &'static str,
        viewport: Size<Pixels>,
        bounds: Bounds<Pixels>,
        handle: &ScrollHandle,
    ) {
        let sample = ScrollSample::new(viewport, bounds, handle);
        let mut map = store().lock().unwrap();
        if map.get(label).copied() == Some(sample) {
            return;
        }
        map.insert(label, sample);

        let content_w10 = sample.w10 + sample.max_x10;
        let content_h10 = sample.h10 + sample.max_y10;

        eprintln!(
            "scroll {label} viewport={:.1}x{:.1} bounds=({:.1},{:.1}) {:.1}x{:.1} offset=({:.1},{:.1}) max=({:.1},{:.1}) content={:.1}x{:.1}",
            ScrollSample::to_f32(sample.viewport_w10),
            ScrollSample::to_f32(sample.viewport_h10),
            ScrollSample::to_f32(sample.x10),
            ScrollSample::to_f32(sample.y10),
            ScrollSample::to_f32(sample.w10),
            ScrollSample::to_f32(sample.h10),
            ScrollSample::to_f32(sample.offset_x10),
            ScrollSample::to_f32(sample.offset_y10),
            ScrollSample::to_f32(sample.max_x10),
            ScrollSample::to_f32(sample.max_y10),
            ScrollSample::to_f32(content_w10),
            ScrollSample::to_f32(content_h10),
        );
    }
}

#[derive(Clone, Copy)]
struct TurnSummaryCounts {
    tool_calls: usize,
    reasonings: usize,
}

fn format_agent_turn_summary(counts: TurnSummaryCounts) -> String {
    format!(
        "{} tool calls, {} thinking",
        counts.tool_calls, counts.reasonings
    )
}

fn format_agent_turn_summary_header(counts: TurnSummaryCounts, in_progress: bool) -> String {
    let _ = in_progress;
    format_agent_turn_summary(counts)
}

fn agent_turn_count(entries: &[luban_domain::ConversationEntry]) -> usize {
    entries
        .iter()
        .filter(|e| matches!(e, luban_domain::ConversationEntry::UserMessage { .. }))
        .count()
}

fn latest_agent_turn_id(entries: &[luban_domain::ConversationEntry]) -> Option<String> {
    let turn_count = agent_turn_count(entries);
    if turn_count == 0 {
        return None;
    }
    Some(format!("agent-turn-{}", turn_count - 1))
}

fn codex_item_is_summary_item(item: &CodexThreadItem) -> bool {
    matches!(
        item,
        CodexThreadItem::Reasoning { .. }
            | CodexThreadItem::TodoList { .. }
            | CodexThreadItem::Error { .. }
    ) || codex_item_is_tool_call(item)
}

fn find_summary_item_in_current_turn<'a>(
    entries: &'a [luban_domain::ConversationEntry],
    item_id: &str,
) -> Option<&'a CodexThreadItem> {
    for entry in entries.iter().rev() {
        match entry {
            luban_domain::ConversationEntry::UserMessage { .. } => break,
            luban_domain::ConversationEntry::CodexItem { item } => {
                let item = item.as_ref();
                if codex_item_id(item) == item_id && codex_item_is_summary_item(item) {
                    return Some(item);
                }
            }
            _ => {}
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn build_workspace_history_children(
    entries: &[luban_domain::ConversationEntry],
    theme: &gpui_component::Theme,
    expanded_items: &HashSet<String>,
    expanded_turns: &HashSet<String>,
    chat_column_width: Option<Pixels>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
    running_turn_summary_items: &[&CodexThreadItem],
    force_expand_current_turn: bool,
) -> Vec<AnyElement> {
    struct TurnAccumulator<'a> {
        id: String,
        tool_calls: usize,
        reasonings: usize,
        summary_items: Vec<&'a CodexThreadItem>,
        agent_messages: Vec<&'a CodexThreadItem>,
    }

    let mut children = Vec::new();
    let mut turn_index = 0usize;
    let mut current_turn: Option<TurnAccumulator<'_>> = None;
    let mut pending_turn_agent_copy: Option<(String, String)> = None;

    let flush_turn =
        |turn: TurnAccumulator<'_>, children: &mut Vec<AnyElement>, in_progress: bool| {
            if !in_progress && turn.summary_items.is_empty() && turn.agent_messages.is_empty() {
                return None;
            }

            let turn_id = turn.id.clone();
            if in_progress || !turn.summary_items.is_empty() {
                let turn_container_id = turn.id.clone();
                let allow_toggle = !in_progress;
                let expanded = in_progress || expanded_turns.contains(&turn.id);
                let header = render_agent_turn_summary_row(
                    &turn.id,
                    TurnSummaryCounts {
                        tool_calls: turn.tool_calls,
                        reasonings: turn.reasonings,
                    },
                    true,
                    expanded,
                    in_progress,
                    allow_toggle,
                    theme,
                    view_handle,
                );
                let mut summary_children = Vec::with_capacity(turn.summary_items.len());
                for item in turn.summary_items {
                    summary_children.push(render_tool_summary_item(
                        &turn_id,
                        item,
                        theme,
                        expanded_items,
                        chat_column_width,
                        view_handle,
                    ));
                }
                let content = div()
                    .pl_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(summary_children);

                children.push(
                    div()
                        .id(format!("conversation-turn-{turn_container_id}"))
                        .w_full()
                        .child(
                            Collapsible::new()
                                .open(expanded)
                                .w_full()
                                .child(header)
                                .content(content),
                        )
                        .into_any_element(),
                );
            }

            let pending_agent_copy = (!in_progress)
                .then(|| {
                    let last = turn.agent_messages.last()?;
                    let CodexThreadItem::AgentMessage { text, .. } = last else {
                        return None;
                    };
                    let render_id = format!("{}-{}", turn_id, codex_item_id(last));
                    Some((
                        format!("conversation-agent-copy-button-{render_id}"),
                        text.to_owned(),
                    ))
                })
                .flatten();

            for item in turn.agent_messages {
                children.push(render_codex_item(
                    &format!("{}-{}", turn_id, codex_item_id(item)),
                    item,
                    theme,
                    false,
                    expanded_items,
                    chat_column_width,
                    view_handle,
                ));
            }

            pending_agent_copy
        };

    for (entry_index, entry) in entries.iter().enumerate() {
        match entry {
            luban_domain::ConversationEntry::UserMessage { text: _ } => {
                if let Some(turn) = current_turn.take() {
                    let _ = flush_turn(turn, &mut children, false);
                }
                pending_turn_agent_copy = None;

                children.push(render_conversation_entry(
                    entry_index,
                    entry,
                    theme,
                    expanded_items,
                    chat_column_width,
                    view_handle,
                ));
                current_turn = Some(TurnAccumulator {
                    id: format!("agent-turn-{turn_index}"),
                    tool_calls: 0,
                    reasonings: 0,
                    summary_items: Vec::new(),
                    agent_messages: Vec::new(),
                });
                turn_index += 1;
            }
            luban_domain::ConversationEntry::CodexItem { item } => {
                let item = item.as_ref();
                if let Some(turn) = &mut current_turn {
                    if matches!(item, CodexThreadItem::AgentMessage { .. }) {
                        turn.agent_messages.push(item);
                        continue;
                    }

                    if matches!(item, CodexThreadItem::Reasoning { .. }) {
                        turn.reasonings += 1;
                        turn.summary_items.push(item);
                        continue;
                    }

                    if matches!(item, CodexThreadItem::TodoList { .. }) {
                        turn.summary_items.push(item);
                        continue;
                    }

                    if matches!(item, CodexThreadItem::Error { .. }) {
                        turn.summary_items.push(item);
                        continue;
                    }

                    if codex_item_is_tool_call(item) {
                        turn.tool_calls += 1;
                        turn.summary_items.push(item);
                    }
                    continue;
                }

                children.push(render_codex_item(
                    &format!("entry-{entry_index}-{}", codex_item_id(item)),
                    item,
                    theme,
                    false,
                    expanded_items,
                    chat_column_width,
                    view_handle,
                ));
            }
            luban_domain::ConversationEntry::TurnUsage { .. } => {
                if let Some(turn) = current_turn.take() {
                    let _ = flush_turn(turn, &mut children, false);
                }
                pending_turn_agent_copy = None;
            }
            luban_domain::ConversationEntry::TurnDuration { .. }
            | luban_domain::ConversationEntry::TurnCanceled
            | luban_domain::ConversationEntry::TurnError { .. } => {
                if let Some(turn) = current_turn.take() {
                    pending_turn_agent_copy = flush_turn(turn, &mut children, false);
                }
                if let luban_domain::ConversationEntry::TurnDuration { duration_ms } = entry {
                    let elapsed = Duration::from_millis(*duration_ms);
                    let duration_row = render_turn_duration_row_for_agent_turn(
                        theme,
                        elapsed,
                        false,
                        pending_turn_agent_copy.take(),
                    );
                    children.push(
                        div()
                            .debug_selector(move || format!("turn-duration-{entry_index}"))
                            .id(format!("conversation-duration-{entry_index}"))
                            .child(duration_row)
                            .into_any_element(),
                    );
                } else {
                    pending_turn_agent_copy = None;
                    children.push(render_conversation_entry(
                        entry_index,
                        entry,
                        theme,
                        expanded_items,
                        chat_column_width,
                        view_handle,
                    ));
                }
            }
        }
    }

    if let Some(mut turn) = current_turn.take() {
        if force_expand_current_turn {
            turn.tool_calls = 0;
            turn.reasonings = 0;
            turn.summary_items.clear();

            for item in running_turn_summary_items {
                if !codex_item_is_summary_item(item) {
                    continue;
                }
                if matches!(item, CodexThreadItem::Reasoning { .. }) {
                    turn.reasonings += 1;
                }
                if codex_item_is_tool_call(item) {
                    turn.tool_calls += 1;
                }
                turn.summary_items.push(item);
            }
        }

        let _ = flush_turn(turn, &mut children, force_expand_current_turn);
    }

    children
}

fn render_turn_duration_row_for_agent_turn(
    theme: &gpui_component::Theme,
    elapsed: Duration,
    in_progress: bool,
    agent_copy: Option<(String, String)>,
) -> AnyElement {
    let icon = if in_progress {
        Spinner::new()
            .with_size(Size::Small)
            .color(theme.muted_foreground)
            .into_any_element()
    } else {
        Icon::empty()
            .path("icons/timer.svg")
            .with_size(Size::Small)
            .text_color(theme.muted_foreground)
            .into_any_element()
    };

    let mut row = div()
        .h(px(24.0))
        .w_full()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .text_color(theme.muted_foreground)
        .child(icon)
        .child(div().truncate().child(format_duration_compact(elapsed)));

    if let Some((debug_id, copy_text)) = agent_copy {
        let dot = Icon::empty()
            .path("icons/circle-dot.svg")
            .with_size(Size::XSmall)
            .text_color(theme.muted_foreground);
        row = row
            .child(dot)
            .child(copy_to_clipboard_button(debug_id, copy_text, theme));
    }

    row.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_agent_turn_summary_row(
    id: &str,
    counts: TurnSummaryCounts,
    has_ops: bool,
    expanded: bool,
    in_progress: bool,
    allow_toggle: bool,
    theme: &gpui_component::Theme,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    let debug_id = format!("agent-turn-summary-{id}");
    let view_handle_for_click = view_handle.clone();
    let id_for_click = id.to_owned();

    let row = div()
        .debug_selector(move || debug_id.clone())
        .h(px(28.0))
        .w_full()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .group("")
        .when(has_ops && allow_toggle, move |s| {
            let view_handle = view_handle_for_click.clone();
            let id = id_for_click.clone();
            s.cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, _, app| {
                    let _ = view_handle.update(app, |view, cx| {
                        view.toggle_agent_turn_expanded(&id);
                        cx.notify();
                    });
                })
        });

    let disclosure_icon = if expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    row.child(
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(min_width_zero(
                div()
                    .flex_1()
                    .truncate()
                    .text_left()
                    .text_color(theme.muted_foreground)
                    .child(format_agent_turn_summary_header(counts, in_progress)),
            ))
            .child(div().w(px(16.0)).when(has_ops && allow_toggle, |s| {
                let debug_id = format!("agent-turn-toggle-{id}");
                s.debug_selector(move || debug_id.clone())
                    .invisible()
                    .when(expanded, |s| s.visible())
                    .group_hover("", |s| s.visible())
                    .child(
                        Icon::new(disclosure_icon)
                            .with_size(Size::Small)
                            .text_color(theme.muted_foreground),
                    )
            })),
    )
    .into_any_element()
}

fn render_tool_summary_item(
    turn_id: &str,
    item: &CodexThreadItem,
    theme: &gpui_component::Theme,
    expanded_items: &HashSet<String>,
    chat_column_width: Option<Pixels>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    let item_id = codex_item_id(item);
    let item_key = format!("{turn_id}::{item_id}");
    let expanded = expanded_items.contains(&item_key);
    let element_id = format!("conversation-turn-item-{}", item_key.replace("::", "-"));
    let debug_id = format!("agent-turn-item-summary-{turn_id}-{item_id}");

    let (title, summary) = codex_item_summary(item, false);
    let icon = Icon::empty()
        .path(codex_item_icon_path(item))
        .with_size(Size::Small)
        .text_color(theme.muted_foreground);

    let disclosure_icon = if expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    let view_handle_for_click = view_handle.clone();
    let item_key_for_click = item_key.clone();
    let header = div()
        .debug_selector(move || debug_id.clone())
        .h(px(28.0))
        .w_full()
        .px_2()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .group("")
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_, _, app| {
            let _ = view_handle_for_click.update(app, |view, cx| {
                view.toggle_agent_item_expanded(&item_key_for_click);
                cx.notify();
            });
        })
        .child(icon)
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(format!("{title}:")),
        )
        .child(min_width_zero(
            div()
                .flex_1()
                .truncate()
                .text_color(theme.muted_foreground)
                .child(summary),
        ))
        .child(
            div()
                .w(px(16.0))
                .invisible()
                .when(expanded, |s| s.visible())
                .group_hover("", |s| s.visible())
                .child(
                    Icon::new(disclosure_icon)
                        .with_size(Size::Small)
                        .text_color(theme.muted_foreground),
                ),
        );

    let details = div()
        .w_full()
        .overflow_x_hidden()
        .whitespace_normal()
        .pl_6()
        .child(render_codex_item_details(
            &element_id,
            item,
            theme,
            chat_column_width,
            view_handle,
        ));

    div()
        .id(element_id)
        .w_full()
        .child(
            Collapsible::new()
                .open(expanded)
                .w_full()
                .child(header)
                .content(details),
        )
        .into_any_element()
}

fn render_codex_item(
    render_id: &str,
    item: &CodexThreadItem,
    theme: &gpui_component::Theme,
    in_progress: bool,
    expanded_items: &HashSet<String>,
    chat_column_width: Option<Pixels>,
    view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    let item_id = codex_item_id(item);
    if !in_progress && let CodexThreadItem::AgentMessage { id: _, text } = item {
        let wrap_width = chat_column_width.map(|w| (w - px(32.0)).max(px(0.0)));
        let message = chat_message_view(
            &format!("agent-message-{render_id}"),
            text,
            wrap_width,
            theme.foreground,
        );
        let debug_id = format!("conversation-agent-message-{render_id}");
        return div()
            .debug_selector(move || debug_id.clone())
            .id(format!("codex-agent-message-{render_id}"))
            .w_full()
            .overflow_x_hidden()
            .px_2()
            .py_1()
            .flex()
            .flex_col()
            .child(min_width_zero(
                div().w_full().whitespace_normal().child(message),
            ))
            .into_any_element();
    }

    let always_expanded = matches!(item, CodexThreadItem::AgentMessage { .. });
    let expanded = always_expanded || expanded_items.contains(item_id);

    let (title, summary) = codex_item_summary(item, in_progress);

    let toggle_button = if always_expanded {
        None
    } else {
        let view_handle = view_handle.clone();
        let id = item_id.to_owned();
        let icon = if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        let tooltip = if expanded { "Hide" } else { "Show" };
        Some(
            Button::new(format!("agent-item-toggle-{render_id}"))
                .ghost()
                .compact()
                .icon(icon)
                .tooltip(tooltip)
                .on_click(move |_, _, app| {
                    let _ = view_handle.update(app, |view, cx| {
                        view.toggle_agent_item_expanded(&id);
                        cx.notify();
                    });
                }),
        )
    };

    if in_progress && !expanded && !always_expanded {
        let (label, text) = codex_item_compact_summary(item);
        let item_icon = if matches!(item, CodexThreadItem::Reasoning { .. }) {
            Spinner::new()
                .with_size(Size::Small)
                .color(theme.muted_foreground)
                .into_any_element()
        } else {
            Icon::empty()
                .path(codex_item_icon_path(item))
                .with_size(Size::Small)
                .text_color(theme.muted_foreground)
                .into_any_element()
        };
        return div()
            .id(format!("codex-compact-{render_id}"))
            .h(px(28.0))
            .w_full()
            .px_1()
            .flex()
            .items_center()
            .gap_2()
            .child(item_icon)
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child(format!("{label}:")),
            )
            .child(min_width_zero(
                div()
                    .flex_1()
                    .truncate()
                    .text_color(theme.muted_foreground)
                    .child(text),
            ))
            .when_some(toggle_button, |s, b| s.child(b))
            .into_any_element();
    }

    let header = div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .child(min_width_zero(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_color(theme.muted_foreground).child(title))
                .child(min_width_zero(div().truncate().child(summary))),
        ))
        .when_some(toggle_button, |s, b| s.child(b));

    div()
        .id(format!("codex-item-{render_id}"))
        .w_full()
        .child(
            Collapsible::new()
                .open(expanded)
                .w_full()
                .p_2()
                .rounded_md()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .child(header)
                .content(render_codex_item_details(
                    render_id,
                    item,
                    theme,
                    chat_column_width,
                    view_handle,
                )),
        )
        .into_any_element()
}

fn render_codex_item_details(
    render_id: &str,
    item: &CodexThreadItem,
    theme: &gpui_component::Theme,
    chat_column_width: Option<Pixels>,
    _view_handle: &gpui::WeakEntity<LubanRootView>,
) -> AnyElement {
    match item {
        CodexThreadItem::AgentMessage { id: _, text } => {
            let wrap_width = chat_column_width.map(|w| (w - px(80.0)).max(px(0.0)));
            let message = chat_message_view(
                &format!("agent-message-{render_id}-details"),
                text,
                wrap_width,
                theme.foreground,
            );
            div()
                .mt_2()
                .w_full()
                .overflow_x_hidden()
                .child(min_width_zero(
                    div().w_full().whitespace_normal().child(message),
                ))
                .into_any_element()
        }
        CodexThreadItem::Reasoning { id: _, text } => {
            let wrap_width = chat_column_width.map(|w| (w - px(80.0)).max(px(0.0)));
            let message = chat_message_view(
                &format!("reasoning-{render_id}-details"),
                text,
                wrap_width,
                theme.muted_foreground,
            );
            div()
                .mt_2()
                .w_full()
                .overflow_x_hidden()
                .child(min_width_zero(
                    div().w_full().whitespace_normal().child(message),
                ))
                .into_any_element()
        }
        CodexThreadItem::CommandExecution {
            id: _,
            command,
            aggregated_output,
            exit_code,
            ..
        } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .flex()
            .flex_col()
            .gap_2()
            .child(min_width_zero(
                div()
                    .w_full()
                    .overflow_x_hidden()
                    .whitespace_normal()
                    .child(
                        chat_markdown_view(
                            &format!("command-{render_id}-details"),
                            &fenced_code_block("sh", command),
                            chat_column_width.map(|w| (w - px(80.0)).max(px(0.0))),
                        )
                        .text_color(theme.foreground),
                    ),
            ))
            .when(!aggregated_output.trim().is_empty(), |s| {
                s.child(min_width_zero(
                    div()
                        .w_full()
                        .overflow_x_hidden()
                        .whitespace_normal()
                        .child(
                            chat_markdown_view(
                                &format!("command-{render_id}-output"),
                                &fenced_code_block("", aggregated_output),
                                chat_column_width.map(|w| (w - px(80.0)).max(px(0.0))),
                            )
                            .text_color(theme.muted_foreground),
                        ),
                ))
            })
            .when_some(*exit_code, |s, code| {
                s.child(
                    div()
                        .whitespace_normal()
                        .text_color(theme.muted_foreground)
                        .child(format!("Exit: {code}")),
                )
            })
            .into_any_element(),
        CodexThreadItem::FileChange { changes, .. } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .flex()
            .flex_col()
            .gap_1()
            .children(changes.iter().map(|c| {
                div()
                    .w_full()
                    .overflow_x_hidden()
                    .whitespace_normal()
                    .text_color(theme.muted_foreground)
                    .child(format!("{:?}: {}", c.kind, c.path))
            }))
            .into_any_element(),
        CodexThreadItem::TodoList { items, .. } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .flex()
            .flex_col()
            .gap_1()
            .children(items.iter().map(|i| {
                let prefix = if i.completed { "[x]" } else { "[ ]" };
                div()
                    .w_full()
                    .overflow_x_hidden()
                    .whitespace_normal()
                    .text_color(theme.muted_foreground)
                    .child(format!("{prefix} {}", i.text))
            }))
            .into_any_element(),
        CodexThreadItem::WebSearch { query, .. } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .child(div().whitespace_normal().child(query.clone()))
            .into_any_element(),
        CodexThreadItem::McpToolCall {
            server,
            tool,
            status,
            ..
        } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().whitespace_normal().child(format!("{server}::{tool}")))
            .child(
                div()
                    .whitespace_normal()
                    .text_color(theme.muted_foreground)
                    .child(format!("{status:?}")),
            )
            .into_any_element(),
        CodexThreadItem::Error { message, .. } => div()
            .mt_2()
            .w_full()
            .overflow_x_hidden()
            .whitespace_normal()
            .text_color(theme.danger_foreground)
            .child(message.clone())
            .into_any_element(),
    }
}

fn chat_markdown_view(id: &str, source: &str, wrap_width: Option<Pixels>) -> TextView {
    let mut code_block_style = gpui::StyleRefinement::default();
    code_block_style.size.width = Some(gpui::relative(1.).into());
    code_block_style.max_size.width = Some(gpui::relative(1.).into());
    code_block_style.min_size.width = Some(px(0.0).into());

    let mut view = TextView::markdown(
        ElementId::Name(SharedString::from(format!("{id}-markdown"))),
        source.to_owned(),
    )
    .style(
        TextViewStyle::default()
            .paragraph_gap(rems(0.5))
            .code_block(code_block_style),
    )
    .selectable(true)
    .text_size(px(16.0))
    .whitespace_normal()
    .flex()
    .flex_col();

    gpui::Styled::style(&mut view).align_items = Some(gpui::AlignItems::Stretch);

    if let Some(wrap_width) = wrap_width {
        view.w(wrap_width)
    } else {
        view
    }
}

fn fenced_code_block(lang: &str, code: &str) -> String {
    let mut max_ticks = 0usize;
    let mut current = 0usize;
    for ch in code.chars() {
        if ch == '`' {
            current += 1;
            max_ticks = max_ticks.max(current);
        } else {
            current = 0;
        }
    }

    let fence_len = (max_ticks + 1).max(3);
    let fence = "`".repeat(fence_len);

    if lang.is_empty() {
        format!("{fence}\n{code}\n{fence}")
    } else {
        format!("{fence}{lang}\n{code}\n{fence}")
    }
}

fn chat_message_view(
    id: &str,
    source: &str,
    wrap_width: Option<Pixels>,
    text_color: gpui::Hsla,
) -> AnyElement {
    let markdown_like = source.contains("```")
        || source.contains("**")
        || source.contains('`')
        || source.contains("](")
        || source
            .lines()
            .any(|line| line.starts_with("# ") || line.starts_with("- ") || line.starts_with("* "));

    if markdown_like {
        return chat_markdown_view(id, source, wrap_width)
            .text_color(text_color)
            .into_any_element();
    }

    let plain_debug_selector = format!("{id}-plain-text");
    let mut container = div()
        .debug_selector(move || plain_debug_selector.clone())
        .id(ElementId::Name(SharedString::from(format!("{id}-text"))))
        .text_size(px(16.0))
        .whitespace_normal()
        .text_color(text_color)
        .child(SelectablePlainText::new(
            SharedString::from(format!("{id}-plain")),
            source.to_owned(),
        ));

    if let Some(wrap_width) = wrap_width {
        container = container.w(wrap_width);
    }

    container.into_any_element()
}

fn codex_item_id(item: &CodexThreadItem) -> &str {
    match item {
        CodexThreadItem::AgentMessage { id, .. } => id,
        CodexThreadItem::Reasoning { id, .. } => id,
        CodexThreadItem::CommandExecution { id, .. } => id,
        CodexThreadItem::FileChange { id, .. } => id,
        CodexThreadItem::McpToolCall { id, .. } => id,
        CodexThreadItem::WebSearch { id, .. } => id,
        CodexThreadItem::TodoList { id, .. } => id,
        CodexThreadItem::Error { id, .. } => id,
    }
}

fn codex_item_summary(item: &CodexThreadItem, in_progress: bool) -> (&'static str, String) {
    let progress_suffix = if in_progress { " (in progress)" } else { "" };
    match item {
        CodexThreadItem::AgentMessage { text, .. } => {
            ("Agent", text.lines().next().unwrap_or("").to_owned())
        }
        CodexThreadItem::Reasoning { text, .. } => (
            if in_progress {
                "Reasoning (in progress)"
            } else {
                "Reasoning"
            },
            if text.trim().is_empty() {
                "…".to_owned()
            } else {
                collapse_inline_markdown_for_summary(text.lines().next().unwrap_or(""))
            },
        ),
        CodexThreadItem::CommandExecution {
            command, status, ..
        } => (
            "Command",
            format!(
                "{status:?}{progress_suffix}: {}",
                command.lines().next().unwrap_or("")
            ),
        ),
        CodexThreadItem::FileChange {
            changes, status, ..
        } => (
            "File change",
            format!("{status:?}{progress_suffix}: {} file(s)", changes.len()),
        ),
        CodexThreadItem::McpToolCall {
            server,
            tool,
            status,
            ..
        } => (
            "MCP tool call",
            format!("{status:?}{progress_suffix}: {server}::{tool}"),
        ),
        CodexThreadItem::WebSearch { query, .. } => (
            "Web search",
            format!(
                "{}{}",
                progress_suffix,
                if query.is_empty() { "" } else { ": " }
            ) + query,
        ),
        CodexThreadItem::TodoList { items, .. } => (
            "Todo list",
            format!("{progress_suffix}: {} item(s)", items.len()),
        ),
        CodexThreadItem::Error { message, .. } => ("Error", message.clone()),
    }
}

fn collapse_inline_markdown_for_summary(text: &str) -> String {
    text.replace("**", "")
        .replace("__", "")
        .replace("`", "")
        .replace('*', "")
        .trim()
        .to_owned()
}

fn codex_item_compact_summary(item: &CodexThreadItem) -> (&'static str, String) {
    match item {
        CodexThreadItem::AgentMessage { text, .. } => {
            ("Agent", text.lines().next().unwrap_or("").to_owned())
        }
        CodexThreadItem::Reasoning { text, .. } => {
            let summary = if text.trim().is_empty() {
                "…".to_owned()
            } else {
                collapse_inline_markdown_for_summary(text.lines().next().unwrap_or(""))
            };
            ("Thinking", summary)
        }
        CodexThreadItem::CommandExecution { command, .. } => {
            ("Bash", command.lines().next().unwrap_or("").to_owned())
        }
        CodexThreadItem::FileChange { changes, .. } => {
            ("Patch", format!("{} file(s)", changes.len()))
        }
        CodexThreadItem::McpToolCall { server, tool, .. } => ("MCP", format!("{server}::{tool}")),
        CodexThreadItem::WebSearch { query, .. } => ("Search", query.clone()),
        CodexThreadItem::TodoList { items, .. } => ("Todo", format!("{} item(s)", items.len())),
        CodexThreadItem::Error { message, .. } => ("Error", message.clone()),
    }
}

fn codex_item_icon_path(item: &CodexThreadItem) -> SharedString {
    match item {
        CodexThreadItem::AgentMessage { .. } => IconName::Bot.path(),
        CodexThreadItem::Reasoning { .. } => "icons/brain.svg".into(),
        CodexThreadItem::CommandExecution { .. } => IconName::SquareTerminal.path(),
        CodexThreadItem::FileChange { .. } => IconName::File.path(),
        CodexThreadItem::McpToolCall { .. } => IconName::Settings2.path(),
        CodexThreadItem::WebSearch { .. } => IconName::Globe.path(),
        CodexThreadItem::TodoList { .. } => IconName::Check.path(),
        CodexThreadItem::Error { .. } => IconName::TriangleAlert.path(),
    }
}

fn codex_item_is_tool_call(item: &CodexThreadItem) -> bool {
    matches!(
        item,
        CodexThreadItem::CommandExecution { .. }
            | CodexThreadItem::FileChange { .. }
            | CodexThreadItem::McpToolCall { .. }
            | CodexThreadItem::WebSearch { .. }
    )
}

fn render_turn_duration_row(
    theme: &gpui_component::Theme,
    elapsed: Duration,
    in_progress: bool,
) -> AnyElement {
    let icon = if in_progress {
        Spinner::new()
            .with_size(Size::Small)
            .color(theme.muted_foreground)
            .into_any_element()
    } else {
        Icon::empty()
            .path("icons/timer.svg")
            .with_size(Size::Small)
            .text_color(theme.muted_foreground)
            .into_any_element()
    };
    div()
        .h(px(24.0))
        .w_full()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .text_color(theme.muted_foreground)
        .child(icon)
        .child(
            div()
                .flex_1()
                .truncate()
                .child(format_duration_compact(elapsed)),
        )
        .into_any_element()
}

fn format_duration_compact(duration: Duration) -> String {
    let ms = duration.as_millis() as u64;
    let secs = ms / 1000;

    if secs < 60 {
        let tenths = (ms % 1000) / 100;
        if secs == 0 && tenths == 0 {
            return "0.0s".to_owned();
        }
        return format!("{secs}.{tenths}s");
    }

    let mins = secs / 60;
    let rem_secs = secs % 60;
    if mins < 60 {
        return format!("{mins}m{rem_secs:02}s");
    }

    let hours = mins / 60;
    let rem_mins = mins % 60;
    format!("{hours}h{rem_mins:02}m")
}

fn workspace_context(state: &AppState, workspace_id: WorkspaceId) -> Option<(PathBuf, PathBuf)> {
    for project in &state.projects {
        for workspace in &project.workspaces {
            if workspace.id == workspace_id && workspace.status == WorkspaceStatus::Active {
                return Some((project.path.clone(), workspace.worktree_path.clone()));
            }
        }
    }
    None
}

struct WorkspaceAgentContext {
    project_slug: String,
    workspace_name: String,
    worktree_path: PathBuf,
}

fn workspace_agent_context(
    state: &AppState,
    workspace_id: WorkspaceId,
) -> Option<WorkspaceAgentContext> {
    for project in &state.projects {
        for workspace in &project.workspaces {
            if workspace.id == workspace_id && workspace.status == WorkspaceStatus::Active {
                return Some(WorkspaceAgentContext {
                    project_slug: project.slug.clone(),
                    workspace_name: workspace.workspace_name.clone(),
                    worktree_path: workspace.worktree_path.clone(),
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
