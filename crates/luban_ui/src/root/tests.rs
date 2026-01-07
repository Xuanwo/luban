use super::*;
use gpui::{
    Modifiers, MouseButton, MouseDownEvent, ScrollDelta, ScrollWheelEvent, point, px, size,
};
use luban_domain::{
    ConversationEntry, ConversationSnapshot, CreatedWorkspace, PersistedAppState, PullRequestState,
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};

fn main_workspace_id(state: &AppState) -> WorkspaceId {
    let project = &state.projects[0];
    project
        .workspaces
        .iter()
        .find(|w| w.status == WorkspaceStatus::Active && w.worktree_path == project.path)
        .expect("missing main workspace")
        .id
}

fn workspace_id_by_name(state: &AppState, name: &str) -> WorkspaceId {
    state.projects[0]
        .workspaces
        .iter()
        .find(|w| w.status == WorkspaceStatus::Active && w.workspace_name == name)
        .unwrap_or_else(|| panic!("missing workspace {name}"))
        .id
}

fn default_thread_id() -> WorkspaceThreadId {
    WorkspaceThreadId::from_u64(1)
}

fn thread_key(workspace_id: WorkspaceId) -> (WorkspaceId, WorkspaceThreadId) {
    (workspace_id, default_thread_id())
}

#[test]
fn agent_turn_summary_uses_thinking_label_and_omits_messages() {
    let summary = format_agent_turn_summary(TurnSummaryCounts {
        tool_calls: 2,
        reasonings: 3,
    });
    assert_eq!(summary, "2 tool calls, 3 thinking");
    assert!(!summary.contains("message"));
    assert!(!summary.contains("reasoning"));
}

#[test]
fn debug_layout_env_parsing() {
    assert!(!debug_layout::parse_enabled(None));
    assert!(!debug_layout::parse_enabled(Some("")));
    assert!(!debug_layout::parse_enabled(Some("0")));
    assert!(!debug_layout::parse_enabled(Some("false")));
    assert!(!debug_layout::parse_enabled(Some("off")));
    assert!(!debug_layout::parse_enabled(Some("no")));

    assert!(debug_layout::parse_enabled(Some("1")));
    assert!(debug_layout::parse_enabled(Some("true")));
    assert!(debug_layout::parse_enabled(Some("yes")));
    assert!(debug_layout::parse_enabled(Some("on")));

    assert!(debug_layout::parse_enabled(Some(" TRUE ")));
    assert!(debug_layout::parse_enabled(Some("Yes")));
    assert!(debug_layout::parse_enabled(Some("ON")));
}

#[gpui::test]
async fn scroll_wheel_events_are_delivered(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let counter = Arc::new(AtomicUsize::new(0));

    struct TestView {
        counter: Arc<AtomicUsize>,
    }

    impl gpui::Render for TestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let counter = self.counter.clone();
            div()
                .flex_1()
                .w_full()
                .h_full()
                .on_scroll_wheel(move |_, _, _app| {
                    counter.fetch_add(1, Ordering::SeqCst);
                })
                .child("scroll-target")
        }
    }

    let (_view, window_cx) = cx.add_window_view(|_, _cx| TestView {
        counter: counter.clone(),
    });
    window_cx.simulate_resize(size(px(200.0), px(200.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let position = point(px(100.0), px(100.0));
    window_cx.simulate_mouse_move(position, None, Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    window_cx.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Pixels(point(px(0.0), px(30.0))),
        ..Default::default()
    });
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        counter.load(Ordering::SeqCst) > 0,
        "expected scroll wheel listener to fire at least once"
    );
}

#[derive(Default)]
struct FakeService;

impl ProjectWorkspaceService for FakeService {
    fn load_app_state(&self) -> Result<PersistedAppState, String> {
        Ok(PersistedAppState {
            projects: Vec::new(),
            sidebar_width: None,
            terminal_pane_width: None,
            agent_default_model_id: None,
            agent_default_thinking_effort: None,
            last_open_workspace_id: None,
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
        })
    }

    fn save_app_state(&self, _snapshot: PersistedAppState) -> Result<(), String> {
        Ok(())
    }

    fn create_workspace(
        &self,
        _project_path: PathBuf,
        _project_slug: String,
    ) -> Result<CreatedWorkspace, String> {
        Ok(CreatedWorkspace {
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        })
    }

    fn open_workspace_in_ide(&self, _worktree_path: PathBuf) -> Result<(), String> {
        Ok(())
    }

    fn archive_workspace(
        &self,
        _project_path: PathBuf,
        _worktree_path: PathBuf,
    ) -> Result<(), String> {
        Ok(())
    }

    fn ensure_conversation(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
    ) -> Result<(), String> {
        Ok(())
    }

    fn list_conversation_threads(
        &self,
        _project_slug: String,
        _workspace_name: String,
    ) -> Result<Vec<luban_domain::ConversationThreadMeta>, String> {
        Ok(Vec::new())
    }

    fn load_conversation(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
    ) -> Result<ConversationSnapshot, String> {
        Ok(ConversationSnapshot {
            thread_id: None,
            entries: Vec::new(),
        })
    }

    fn store_context_image(
        &self,
        _project_slug: String,
        _workspace_name: String,
        image: luban_domain::ContextImage,
    ) -> Result<PathBuf, String> {
        Ok(PathBuf::from(format!(
            "/tmp/luban/context/{}.{}",
            image.bytes.len(),
            image.extension
        )))
    }

    fn store_context_text(
        &self,
        _project_slug: String,
        _workspace_name: String,
        text: String,
        extension: String,
    ) -> Result<PathBuf, String> {
        Ok(PathBuf::from(format!(
            "/tmp/luban/context/{}.{}",
            text.len(),
            extension
        )))
    }

    fn store_context_file(
        &self,
        _project_slug: String,
        _workspace_name: String,
        source_path: PathBuf,
    ) -> Result<PathBuf, String> {
        Ok(source_path)
    }

    fn run_agent_turn_streamed(
        &self,
        request: RunAgentTurnRequest,
        _cancel: Arc<AtomicBool>,
        on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync>,
    ) -> Result<(), String> {
        let thread_id = request.thread_id.unwrap_or_else(|| "thread-1".to_owned());
        on_event(CodexThreadEvent::ThreadStarted {
            thread_id: thread_id.clone(),
        });
        on_event(CodexThreadEvent::ItemStarted {
            item: CodexThreadItem::CommandExecution {
                id: "cmd-1".to_owned(),
                command: "echo hello".to_owned(),
                aggregated_output: "".to_owned(),
                exit_code: None,
                status: luban_domain::CodexCommandExecutionStatus::InProgress,
            },
        });
        on_event(CodexThreadEvent::ItemCompleted {
            item: CodexThreadItem::AgentMessage {
                id: "item-1".to_owned(),
                text: format!("Echo: {}", request.prompt),
            },
        });
        on_event(CodexThreadEvent::TurnCompleted {
            usage: luban_domain::CodexUsage {
                input_tokens: 1,
                cached_input_tokens: 0,
                output_tokens: 1,
            },
        });
        Ok(())
    }

    fn gh_is_authorized(&self) -> Result<bool, String> {
        Ok(false)
    }

    fn gh_pull_request_info(
        &self,
        _worktree_path: PathBuf,
    ) -> Result<Option<PullRequestInfo>, String> {
        Ok(None)
    }

    fn gh_open_pull_request(&self, _worktree_path: PathBuf) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn compact_item_summary_is_stable() {
    let item = CodexThreadItem::CommandExecution {
        id: "cmd-1".to_owned(),
        command: "echo hello\necho world".to_owned(),
        aggregated_output: String::new(),
        exit_code: None,
        status: luban_domain::CodexCommandExecutionStatus::InProgress,
    };
    assert_eq!(
        codex_item_compact_summary(&item),
        ("Bash", "echo hello".to_owned())
    );

    let item = CodexThreadItem::Reasoning {
        id: "r-1".to_owned(),
        text: "\n".to_owned(),
    };
    assert_eq!(
        codex_item_compact_summary(&item),
        ("Thinking", "…".to_owned())
    );
}

#[test]
fn codex_item_icon_paths_are_stable() {
    let item = CodexThreadItem::Reasoning {
        id: "r-1".to_owned(),
        text: "x".to_owned(),
    };
    assert_eq!(codex_item_icon_path(&item).as_ref(), "icons/brain.svg");
}

#[test]
fn duration_format_is_compact() {
    assert_eq!(
        format_duration_compact(Duration::from_millis(1234)),
        "1.2s".to_owned()
    );
    assert_eq!(
        format_duration_compact(Duration::from_secs(62)),
        "1m02s".to_owned()
    );
}

#[test]
fn main_pane_title_tracks_selected_context() {
    let mut state = AppState::new();
    assert_eq!(main_pane_title(&state, MainPane::None), String::new());

    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    let project_name = state.projects[0].name.clone();

    assert_eq!(
        main_pane_title(&state, MainPane::ProjectSettings(project_id)),
        project_name
    );

    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");

    assert_eq!(
        main_pane_title(&state, MainPane::Workspace(workspace_id)),
        "abandon-about".to_owned()
    );
}

#[test]
fn sidebar_workspace_title_uses_branch_name() {
    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });

    let workspace = state
        .projects
        .iter()
        .flat_map(|p| &p.workspaces)
        .find(|w| w.status == WorkspaceStatus::Active && w.workspace_name == "w1")
        .expect("missing workspace w1");

    assert_eq!(
        sidebar::sidebar_workspace_title(workspace),
        "repo/w1".to_owned()
    );
}

#[test]
fn sidebar_workspace_metadata_uses_workspace_name() {
    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project = &state.projects[0];
    let workspace = project
        .workspaces
        .iter()
        .find(|w| w.status == WorkspaceStatus::Active && w.worktree_path == project.path)
        .expect("missing main workspace");

    assert_eq!(
        sidebar::sidebar_workspace_metadata(workspace),
        "main".to_owned()
    );
}

#[test]
fn titlebar_context_tracks_selected_workspace() {
    let mut state = AppState::new();
    assert_eq!(
        titlebar::titlebar_context(&state),
        titlebar::TitlebarContext {
            branch_label: String::new(),
            ide_workspace_id: None,
        }
    );

    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;

    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");

    state.apply(Action::OpenWorkspace { workspace_id });

    let context = titlebar::titlebar_context(&state);
    assert_eq!(context.branch_label, "luban/abandon-about".to_owned());
    assert_eq!(context.ide_workspace_id, Some(workspace_id));
}

#[gpui::test]
async fn titlebar_buttons_keep_terminal_toggle_on_far_right(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "b".repeat(256),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.apply(Action::OpenWorkspace { workspace_id });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    let window_size = size(px(900.0), px(240.0));
    window_cx.simulate_resize(window_size);
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let toggle_bounds = window_cx
        .debug_bounds("titlebar-toggle-terminal")
        .expect("missing titlebar terminal toggle button");

    assert!(
        toggle_bounds.right() >= window_size.width - px(24.0),
        "toggle={:?} window={:?}",
        toggle_bounds,
        window_size
    );
}

#[gpui::test]
async fn terminal_title_is_rendered_in_titlebar_when_visible(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "repo/branch".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.right_pane = RightPane::Terminal;

    let (view, window_cx) = cx.add_window_view(|_window, cx| {
        let mut view = LubanRootView::with_state(services, state, cx);
        view.terminal_enabled = true;
        view.workspace_terminal_errors
            .insert(thread_key(workspace_id), "stub terminal".to_owned());
        view
    });
    window_cx.simulate_resize(size(px(900.0), px(240.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        window_cx.debug_bounds("titlebar-terminal").is_some(),
        "expected terminal header to be rendered in titlebar"
    );
    assert!(
        window_cx
            .debug_bounds("titlebar-terminal-divider")
            .is_some(),
        "expected divider to be rendered when terminal is visible"
    );
    let divider = window_cx
        .debug_bounds("titlebar-terminal-divider")
        .expect("missing divider bounds");
    assert!(
        divider.size.width >= px(0.9),
        "expected divider to be visible: {divider:?}"
    );

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(Action::ToggleTerminalPane, cx);
        });
    });
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }
    let right_pane = view.read_with(window_cx, |v, _| v.debug_state().right_pane);
    assert_eq!(right_pane, RightPane::None);
    let divider = window_cx
        .debug_bounds("titlebar-terminal-divider")
        .expect("missing divider bounds after collapse");
    assert!(
        divider.size.width <= px(0.1),
        "expected divider to be hidden when terminal is collapsed: {divider:?}"
    );
}

#[gpui::test]
async fn titlebar_workspace_title_does_not_render_prefix_icon(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "repo/branch".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.apply(Action::OpenWorkspace { workspace_id });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(240.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        window_cx.debug_bounds("titlebar-branch-symbol").is_none(),
        "expected titlebar to avoid rendering a prefix icon"
    );
    assert!(
        window_cx
            .debug_bounds("titlebar-branch-indicator")
            .is_some(),
        "expected titlebar workspace title to remain rendered"
    );
}

#[gpui::test]
async fn titlebar_double_click_triggers_window_action(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);
    let state = AppState::new();

    let (_view, window_cx) =
        cx.add_window_view(|_window, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(240.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let zoom_bounds = window_cx
        .debug_bounds("titlebar-zoom-area")
        .expect("missing titlebar zoom area");

    assert!(
        !window_cx.update(|window, _| window.is_fullscreen()),
        "expected test window to start not fullscreen"
    );

    window_cx.simulate_event(MouseDownEvent {
        position: zoom_bounds.center(),
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    assert!(
        !window_cx.update(|window, _| window.is_fullscreen()),
        "single click should not trigger titlebar double-click action"
    );

    window_cx.simulate_event(MouseDownEvent {
        position: zoom_bounds.center(),
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    assert!(
        window_cx.update(|window, _| window.is_fullscreen()),
        "double click should trigger titlebar double-click action"
    );
}

#[gpui::test]
async fn open_button_is_in_titlebar_for_workspace(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.apply(Action::OpenWorkspace { workspace_id });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(240.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        window_cx.debug_bounds("workspace-pane-header").is_none(),
        "workspace header should not be rendered when controls are in titlebar"
    );

    let titlebar_bounds = window_cx
        .debug_bounds("titlebar-main")
        .expect("missing titlebar main segment");
    let title_bounds = window_cx
        .debug_bounds("titlebar-branch-indicator")
        .expect("missing titlebar branch indicator");
    let open_bounds = window_cx
        .debug_bounds("titlebar-open-in-zed")
        .expect("missing titlebar open button");

    let gap = open_bounds.left() - title_bounds.right();
    assert!(
        gap >= px(-1.0) && gap <= px(32.0),
        "open should be placed next to the workspace title: gap={gap:?} title={title_bounds:?} open={open_bounds:?}",
    );
    assert!(
        open_bounds.right() <= titlebar_bounds.right() + px(2.0),
        "open={:?} titlebar={:?}",
        open_bounds,
        titlebar_bounds
    );
}

#[gpui::test]
async fn workspace_thread_tabs_render_all_open_tabs_and_menu(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.apply(Action::OpenWorkspace { workspace_id });
    for _ in 0..3 {
        state.apply(Action::CreateWorkspaceThread { workspace_id });
    }

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(320.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        window_cx.debug_bounds("workspace-thread-tabs").is_some(),
        "missing workspace thread tab strip"
    );
    assert!(window_cx.debug_bounds("workspace-thread-tab-0").is_some());
    assert!(window_cx.debug_bounds("workspace-thread-tab-1").is_some());
    assert!(window_cx.debug_bounds("workspace-thread-tab-2").is_some());
    assert!(window_cx.debug_bounds("workspace-thread-tab-3").is_some());
    assert!(
        window_cx
            .debug_bounds("workspace-thread-tabs-menu-trigger")
            .is_some(),
        "missing thread menu trigger"
    );
    assert!(window_cx.debug_bounds("workspace-thread-tab-new").is_some());
}

#[gpui::test]
async fn workspace_thread_tabs_menu_separates_active_and_archived(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.apply(Action::OpenWorkspace { workspace_id });

    let mut thread_ids = vec![state.active_thread_id(workspace_id).unwrap()];
    for _ in 0..3 {
        state.apply(Action::CreateWorkspaceThread { workspace_id });
        thread_ids.push(state.active_thread_id(workspace_id).unwrap());
    }

    let archived_thread = thread_ids[1];
    state.apply(Action::CloseWorkspaceThreadTab {
        workspace_id,
        thread_id: archived_thread,
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(320.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let trigger = window_cx
        .debug_bounds("workspace-thread-tabs-menu-trigger")
        .expect("missing thread menu trigger");
    window_cx.simulate_click(trigger.center(), Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        window_cx
            .debug_bounds("workspace-thread-tabs-menu-active-section")
            .is_some(),
        "expected an active section header in the thread menu"
    );
    assert!(
        window_cx
            .debug_bounds("workspace-thread-tabs-menu-archived-section")
            .is_some(),
        "expected an archived section header in the thread menu"
    );
    assert!(
        window_cx
            .debug_bounds("workspace-thread-tabs-menu-archived-0")
            .is_some(),
        "expected archived entries to be listed in the thread menu"
    );

    let archived_row = window_cx
        .debug_bounds("workspace-thread-tabs-menu-archived-0")
        .expect("missing archived row");
    window_cx.simulate_click(archived_row.center(), Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let tabs = view.read_with(window_cx, |v, _| {
        v.debug_state()
            .workspace_tabs(workspace_id)
            .expect("missing tabs")
            .clone()
    });
    assert!(
        tabs.open_tabs.contains(&archived_thread),
        "expected archived tab to be restored into open tabs"
    );
    assert!(
        !tabs.archived_tabs.contains(&archived_thread),
        "expected restored tab to be removed from archived tabs"
    );
}

#[gpui::test]
async fn workspace_thread_tabs_menu_hides_archived_section_when_empty(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.apply(Action::OpenWorkspace { workspace_id });
    for _ in 0..2 {
        state.apply(Action::CreateWorkspaceThread { workspace_id });
    }

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(320.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let trigger = window_cx
        .debug_bounds("workspace-thread-tabs-menu-trigger")
        .expect("missing thread menu trigger");
    window_cx.simulate_click(trigger.center(), Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        window_cx
            .debug_bounds("workspace-thread-tabs-menu-active-section")
            .is_some(),
        "expected active section header in the thread menu"
    );
    assert!(
        window_cx
            .debug_bounds("workspace-thread-tabs-menu-archived-section")
            .is_none(),
        "archived section should be hidden when there are no archived tabs"
    );
    assert!(
        window_cx
            .debug_bounds("workspace-thread-tabs-menu-archived-0")
            .is_none(),
        "archived rows should be hidden when there are no archived tabs"
    );
}

#[gpui::test]
async fn workspace_thread_tabs_do_not_expand_to_fill_strip(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.apply(Action::OpenWorkspace { workspace_id });
    state.apply(Action::CreateWorkspaceThread { workspace_id });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(320.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let tab = window_cx
        .debug_bounds("workspace-thread-tab-0")
        .expect("missing first thread tab");
    assert!(
        tab.size.width <= px(242.0),
        "expected tab width to be capped instead of filling the strip: {tab:?}"
    );
}

#[gpui::test]
async fn clicking_project_header_toggles_expanded(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });

    let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    cx.refresh().unwrap();

    let bounds = cx
        .debug_bounds("project-header-0")
        .expect("missing debug bounds for project-header-0");
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.refresh().unwrap();

    let expanded = view.read_with(cx, |v, _| v.debug_state().projects[0].expanded);
    assert!(!expanded);
}

#[gpui::test]
async fn main_workspace_row_is_rendered_and_is_not_archivable(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(240.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let main_bounds = window_cx
        .debug_bounds("workspace-main-row-0")
        .expect("missing main workspace row");
    assert!(
        window_cx.debug_bounds("workspace-main-badge-0").is_none(),
        "main workspace should not render a separate badge label"
    );
    assert!(
        window_cx.debug_bounds("workspace-main-icon-0").is_some(),
        "main workspace should render a leading icon"
    );
    assert!(
        window_cx.debug_bounds("workspace-row-0-0").is_none(),
        "main workspace should not be rendered as a normal workspace row"
    );
    assert!(
        window_cx.debug_bounds("workspace-archive-0-0").is_none(),
        "main workspace should not be archivable"
    );

    let main_workspace_id = view.read_with(window_cx, |v, _| main_workspace_id(v.debug_state()));

    window_cx.simulate_click(main_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    let selected = view.read_with(window_cx, |v, _| v.debug_state().main_pane);
    assert!(
        matches!(selected, MainPane::Workspace(id) if id == main_workspace_id),
        "expected main workspace to be selected after click"
    );
}

#[gpui::test]
async fn workspace_row_shows_running_spinner_and_unread_completion_badge(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    state.apply(Action::OpenProjectSettings { project_id });

    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    let thread_id = default_thread_id();
    state.apply(Action::SendAgentMessage {
        workspace_id,
        thread_id,
        text: "run".to_owned(),
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    assert!(
        window_cx
            .debug_bounds("workspace-status-running-0-0")
            .is_some(),
        "expected running spinner indicator to be visible"
    );

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(
                Action::AgentTurnFinished {
                    workspace_id,
                    thread_id,
                },
                cx,
            );
        });
    });
    window_cx.refresh().unwrap();
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let running = view.read_with(window_cx, |v, _| {
        v.debug_state().workspace_has_running_turn(workspace_id)
    });
    assert!(
        !running,
        "expected state to clear running status after turn finishes"
    );
    let unread = view.read_with(window_cx, |v, _| {
        v.debug_state()
            .workspace_has_unread_completion(workspace_id)
    });
    assert!(
        unread,
        "expected state to mark workspace as having unread completion after turn finishes"
    );
    assert!(
        window_cx
            .debug_bounds("workspace-status-unread-0-0")
            .is_some(),
        "expected unread completion badge to be rendered in the sidebar"
    );

    let row_bounds = window_cx
        .debug_bounds("workspace-row-0-0")
        .expect("missing debug bounds for workspace-row-0-0");
    window_cx.simulate_click(row_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let (main_pane, unread) = view.read_with(window_cx, |v, _| {
        (
            v.debug_state().main_pane,
            v.debug_state()
                .workspace_has_unread_completion(workspace_id),
        )
    });
    assert!(
        matches!(main_pane, MainPane::Workspace(id) if id == workspace_id),
        "expected workspace to become selected after click"
    );
    assert!(
        !unread,
        "expected unread completion badge to clear when the workspace is opened"
    );
}

#[gpui::test]
async fn archiving_workspace_shows_prompt_and_updates_state(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });

    let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    cx.refresh().unwrap();

    let row_bounds = cx
        .debug_bounds("workspace-row-0-0")
        .expect("missing debug bounds for workspace-row-0-0");
    cx.simulate_mouse_move(row_bounds.center(), None, Modifiers::none());
    cx.refresh().unwrap();

    let bounds = cx
        .debug_bounds("workspace-archive-0-0")
        .expect("missing debug bounds for workspace-archive-0-0");
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    cx.refresh().unwrap();

    let status = view.read_with(cx, |v, _| {
        v.debug_state().projects[0]
            .workspaces
            .iter()
            .find(|w| w.workspace_name == "abandon-about")
            .expect("missing abandon-about workspace")
            .status
    });
    assert_eq!(status, WorkspaceStatus::Active);

    let row_bounds = cx
        .debug_bounds("workspace-row-0-0")
        .expect("missing debug bounds for workspace-row-0-0");
    cx.simulate_mouse_move(row_bounds.center(), None, Modifiers::none());
    cx.refresh().unwrap();

    let bounds = cx
        .debug_bounds("workspace-archive-0-0")
        .expect("missing debug bounds for workspace-archive-0-0");
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Archive");
    cx.run_until_parked();
    cx.refresh().unwrap();

    let status = view.read_with(cx, |v, _| {
        v.debug_state().projects[0]
            .workspaces
            .iter()
            .find(|w| w.workspace_name == "abandon-about")
            .expect("missing abandon-about workspace")
            .status
    });
    assert_eq!(status, WorkspaceStatus::Archived);
}

#[gpui::test]
async fn markdown_messages_render_in_workspace(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "Hello **world**\n\n- a\n- b\n\n`inline`".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "item-1".to_owned(),
                        text: "Reply:\n\n- one\n- two\n\n[gpui](https://example.com)".to_owned(),
                    }),
                },
            ],
        },
    });

    let (_, window_cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    let bounds = window_cx
        .debug_bounds("conversation-agent-message-agent-turn-0-item-1")
        .expect("missing debug bounds for conversation-agent-message-agent-turn-0-item-1");
    assert!(bounds.size.height > px(0.0));
}

#[gpui::test]
async fn duplicate_agent_message_ids_render_independently(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "First".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "item-1".to_owned(),
                        text: "First reply".to_owned(),
                    }),
                },
                ConversationEntry::UserMessage {
                    text: "Second".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "item-1".to_owned(),
                        text: "Second reply".to_owned(),
                    }),
                },
            ],
        },
    });

    let (_, window_cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    let first = window_cx
        .debug_bounds("conversation-agent-message-agent-turn-0-item-1")
        .expect("missing debug bounds for conversation-agent-message-agent-turn-0-item-1");
    let second = window_cx
        .debug_bounds("conversation-agent-message-agent-turn-1-item-1")
        .expect("missing debug bounds for conversation-agent-message-agent-turn-1-item-1");

    assert!(first.size.height > px(0.0));
    assert!(second.size.height > px(0.0));
    assert!(second.top() > first.top());
}

#[gpui::test]
async fn clicking_turn_summary_row_toggles_expanded(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "Test".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::CommandExecution {
                        id: "item-1".to_owned(),
                        command: "echo hello".to_owned(),
                        aggregated_output: "hello".to_owned(),
                        exit_code: Some(0),
                        status: luban_domain::CodexCommandExecutionStatus::Completed,
                    }),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "item-2".to_owned(),
                        text: "Reply".to_owned(),
                    }),
                },
            ],
        },
    });

    let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    cx.refresh().unwrap();

    let expanded = view.read_with(cx, |v, _| v.expanded_agent_turns.contains("agent-turn-0"));
    assert!(!expanded);

    let row_bounds = cx
        .debug_bounds("agent-turn-summary-agent-turn-0")
        .expect("missing debug bounds for agent-turn-summary-agent-turn-0");
    cx.simulate_click(row_bounds.center(), Modifiers::none());
    cx.refresh().unwrap();

    let expanded = view.read_with(cx, |v, _| v.expanded_agent_turns.contains("agent-turn-0"));
    assert!(expanded);
}

#[gpui::test]
async fn clicking_turn_item_summary_row_toggles_expanded(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "Test".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::Reasoning {
                        id: "item-1".to_owned(),
                        text: "Reasoning details".to_owned(),
                    }),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "item-2".to_owned(),
                        text: "Reply".to_owned(),
                    }),
                },
            ],
        },
    });

    let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    cx.refresh().unwrap();

    let row_bounds = cx
        .debug_bounds("agent-turn-summary-agent-turn-0")
        .expect("missing debug bounds for agent-turn-summary-agent-turn-0");
    cx.simulate_click(row_bounds.center(), Modifiers::none());
    cx.refresh().unwrap();

    let expanded = view.read_with(cx, |v, _| v.expanded_agent_turns.contains("agent-turn-0"));
    assert!(expanded);

    let item_bounds = cx
        .debug_bounds("agent-turn-item-summary-agent-turn-0-item-1")
        .expect("missing debug bounds for agent-turn-item-summary-agent-turn-0-item-1");
    cx.simulate_click(item_bounds.center(), Modifiers::none());
    cx.refresh().unwrap();

    let expanded = view.read_with(cx, |v, _| {
        v.expanded_agent_items.contains("agent-turn-0::item-1")
    });
    assert!(expanded);
}

#[gpui::test]
async fn running_turn_summary_is_expanded_and_not_toggleable(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    state.apply(Action::SendAgentMessage {
        workspace_id,
        thread_id: default_thread_id(),
        text: "Test".to_owned(),
    });
    state.apply(Action::AgentEventReceived {
        workspace_id,
        thread_id: default_thread_id(),
        event: CodexThreadEvent::ItemStarted {
            item: CodexThreadItem::Reasoning {
                id: "item-1".to_owned(),
                text: "x".to_owned(),
            },
        },
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    window_cx
        .debug_bounds("agent-turn-summary-agent-turn-0")
        .expect("missing running turn summary row");
    window_cx
        .debug_bounds("agent-turn-item-summary-agent-turn-0-item-1")
        .expect("missing running summary item row");

    let expanded = view.read_with(window_cx, |v, _| {
        v.expanded_agent_turns.contains("agent-turn-0")
    });
    assert!(!expanded);

    let header_bounds = window_cx
        .debug_bounds("agent-turn-summary-agent-turn-0")
        .expect("missing running turn summary row");
    window_cx.simulate_click(header_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    let expanded = view.read_with(window_cx, |v, _| {
        v.expanded_agent_turns.contains("agent-turn-0")
    });
    assert!(!expanded);

    window_cx
        .debug_bounds("agent-turn-item-summary-agent-turn-0-item-1")
        .expect("missing running summary item row after click");
}

#[gpui::test]
async fn running_turn_summary_keeps_completed_items_visible_while_running(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    state.apply(Action::SendAgentMessage {
        workspace_id,
        thread_id: default_thread_id(),
        text: "Test".to_owned(),
    });
    state.apply(Action::AgentEventReceived {
        workspace_id,
        thread_id: default_thread_id(),
        event: CodexThreadEvent::ItemStarted {
            item: CodexThreadItem::Reasoning {
                id: "item-1".to_owned(),
                text: "x".to_owned(),
            },
        },
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    window_cx
        .debug_bounds("agent-turn-item-summary-agent-turn-0-item-1")
        .expect("missing running summary item row");

    view.update(window_cx, |view, cx| {
        view.dispatch(
            Action::AgentEventReceived {
                workspace_id,
                thread_id: default_thread_id(),
                event: CodexThreadEvent::ItemCompleted {
                    item: CodexThreadItem::Reasoning {
                        id: "item-1".to_owned(),
                        text: "final".to_owned(),
                    },
                },
            },
            cx,
        );
    });
    window_cx.refresh().unwrap();

    window_cx
        .debug_bounds("agent-turn-item-summary-agent-turn-0-item-1")
        .expect("missing completed summary item row while running");
}

#[gpui::test]
async fn running_turn_summary_auto_collapses_on_turn_end(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    state.apply(Action::SendAgentMessage {
        workspace_id,
        thread_id: default_thread_id(),
        text: "Test".to_owned(),
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    view.update(window_cx, |view, cx| {
        view.expanded_agent_turns.insert("agent-turn-0".to_owned());
        view.expanded_agent_items
            .insert("agent-turn-0::item-1".to_owned());
        view.dispatch(
            Action::AgentEventReceived {
                workspace_id,
                thread_id: default_thread_id(),
                event: CodexThreadEvent::TurnCompleted {
                    usage: luban_domain::CodexUsage {
                        input_tokens: 0,
                        cached_input_tokens: 0,
                        output_tokens: 0,
                    },
                },
            },
            cx,
        );
    });
    window_cx.refresh().unwrap();

    let expanded = view.read_with(window_cx, |v, _| {
        v.expanded_agent_turns.contains("agent-turn-0")
    });
    assert!(!expanded);

    let item_expanded = view.read_with(window_cx, |v, _| {
        v.expanded_agent_items.contains("agent-turn-0::item-1")
    });
    assert!(!item_expanded);

    assert!(
        window_cx
            .debug_bounds("agent-turn-item-summary-agent-turn-0-item-1")
            .is_none()
    );
}

#[gpui::test]
async fn turn_summary_includes_error_items(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "Test".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::Error {
                        id: "err-1".to_owned(),
                        message: "reconnecting ...1/5".to_owned(),
                    }),
                },
            ],
        },
    });

    let (view, cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    cx.refresh().unwrap();

    let row_bounds = cx
        .debug_bounds("agent-turn-summary-agent-turn-0")
        .expect("missing debug bounds for agent-turn-summary-agent-turn-0");
    cx.simulate_click(row_bounds.center(), Modifiers::none());
    cx.refresh().unwrap();

    let expanded = view.read_with(cx, |v, _| v.expanded_agent_turns.contains("agent-turn-0"));
    assert!(expanded);

    let _ = cx
        .debug_bounds("agent-turn-item-summary-agent-turn-0-err-1")
        .expect("missing debug bounds for agent-turn-item-summary-agent-turn-0-err-1");
}

#[gpui::test]
async fn user_message_context_attachments_render_in_order(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let message = concat!(
        "hello\n",
        "<<context:text:/tmp/a.txt>>>\n",
        "world\n",
        "<<context:text:/tmp/b.txt>>>\n"
    );

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: None,
            entries: vec![ConversationEntry::UserMessage {
                text: message.to_owned(),
            }],
        },
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    let first = window_cx
        .debug_bounds("conversation-user-attachment-0-0")
        .expect("missing first attachment");
    let second = window_cx
        .debug_bounds("conversation-user-attachment-0-1")
        .expect("missing second attachment");

    assert!(
        first.origin.y < second.origin.y,
        "expected first attachment to be above second"
    );
}

#[gpui::test]
async fn user_message_reflows_on_window_resize(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let long_text = std::iter::repeat_n("word", 200)
        .collect::<Vec<_>>()
        .join(" ");
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage { text: long_text }],
        },
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));

    window_cx.simulate_resize(size(px(1200.0), px(800.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    let wide_column = window_cx
        .debug_bounds("workspace-chat-column")
        .expect("missing debug bounds for workspace-chat-column");
    let wide_bubble = window_cx
        .debug_bounds("conversation-user-bubble-0")
        .expect("missing debug bounds for conversation-user-bubble-0");

    window_cx.simulate_resize(size(px(520.0), px(800.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    let narrow_column = window_cx
        .debug_bounds("workspace-chat-column")
        .expect("missing debug bounds for workspace-chat-column");
    let narrow_bubble = window_cx
        .debug_bounds("conversation-user-bubble-0")
        .expect("missing debug bounds for conversation-user-bubble-0");

    assert!(narrow_column.size.width < wide_column.size.width);
    assert!(
        narrow_bubble.size.height > wide_bubble.size.height,
        "wide={:?} narrow={:?}",
        wide_bubble.size,
        narrow_bubble.size
    );
    assert!(narrow_bubble.right() <= narrow_column.right() + px(2.0));
    assert!(narrow_bubble.right() >= narrow_column.right() - px(8.0));
}

#[gpui::test]
async fn chat_surface_does_not_shift_horizontally_on_wide_window_resize(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Test".to_owned(),
            }],
        },
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));

    window_cx.simulate_resize(size(px(1200.0), px(800.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    let wide_column = window_cx
        .debug_bounds("workspace-chat-column")
        .expect("missing debug bounds for workspace-chat-column");
    let wide_surface = window_cx
        .debug_bounds("chat-composer-surface")
        .expect("missing debug bounds for chat-composer-surface");

    window_cx.simulate_resize(size(px(1800.0), px(800.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    let wider_column = window_cx
        .debug_bounds("workspace-chat-column")
        .expect("missing debug bounds for workspace-chat-column");
    let wider_surface = window_cx
        .debug_bounds("chat-composer-surface")
        .expect("missing debug bounds for chat-composer-surface");

    let column_shift = (wide_column.origin.x - wider_column.origin.x).abs();
    let surface_shift = (wide_surface.origin.x - wider_surface.origin.x).abs();
    assert!(
        column_shift <= px(0.5),
        "expected chat column to remain left-anchored on wide resize: wide={wide_column:?} wider={wider_column:?}"
    );
    assert!(
        surface_shift <= px(0.5),
        "expected chat composer surface to remain left-anchored on wide resize: wide={wide_surface:?} wider={wider_surface:?}"
    );
}

#[gpui::test]
async fn long_user_message_bubble_keeps_right_gutter(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let long_text = std::iter::repeat_n("word", 800)
        .collect::<Vec<_>>()
        .join(" ");
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage { text: long_text }],
        },
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));

    window_cx.simulate_resize(size(px(760.0), px(800.0)));
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let column = window_cx
        .debug_bounds("workspace-chat-column")
        .expect("missing debug bounds for workspace-chat-column");
    let bubble = window_cx
        .debug_bounds("conversation-user-bubble-0")
        .expect("missing debug bounds for conversation-user-bubble-0");

    let gutter = bubble.left() - column.left();
    assert!(
        gutter >= px(24.0),
        "expected long user bubble to leave a left gutter (avoid full-width drift): gutter={gutter:?} column={column:?} bubble={bubble:?}"
    );
    assert!(bubble.right() <= column.right() + px(2.0));
    assert!(bubble.right() >= column.right() - px(8.0));
}

#[gpui::test]
async fn user_message_text_can_be_selected_and_copied(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let message = "select me".to_owned();
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: message.clone(),
            }],
        },
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(720.0), px(400.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let text_bounds = window_cx
        .debug_bounds("user-message-0-plain-text")
        .expect("missing debug bounds for user-message-0-plain-text");
    let y = text_bounds.center().y;
    let start = point(text_bounds.left() + px(1.0), y);
    let end = point(text_bounds.right() + px(200.0), y);

    window_cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
    window_cx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), Modifiers::none());
    window_cx.simulate_mouse_up(end, gpui::MouseButton::Left, Modifiers::none());
    window_cx.refresh().unwrap();

    window_cx.simulate_keystrokes("cmd-c");
    window_cx.run_until_parked();

    let copied = window_cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied, Some(message));
}

#[gpui::test]
async fn chat_copy_buttons_copy_user_and_agent_messages(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let user_message = "User message".to_owned();
    let agent_message = "Agent message".to_owned();
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: user_message.clone(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "turn-a/item_0".to_owned(),
                        text: agent_message.clone(),
                    }),
                },
                ConversationEntry::TurnDuration { duration_ms: 1000 },
            ],
        },
    });

    let (_view, window_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
        gpui_component::Root::new(view, window, cx)
    });
    window_cx.simulate_resize(size(px(720.0), px(400.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let user_copy = window_cx
        .debug_bounds("conversation-user-copy-button-0")
        .expect("missing user copy button");
    assert!(
        user_copy.size.width > px(0.0) && user_copy.size.height > px(0.0),
        "expected user copy button to have non-zero bounds: {user_copy:?}"
    );
    let agent_copy = window_cx
        .debug_bounds("conversation-agent-copy-button-agent-turn-0-turn-a/item_0")
        .expect("missing agent copy button");
    assert!(
        agent_copy.size.width > px(0.0) && agent_copy.size.height > px(0.0),
        "expected agent copy button to have non-zero bounds: {agent_copy:?}"
    );
    assert_ne!(
        (
            user_copy.left(),
            user_copy.top(),
            user_copy.size.width,
            user_copy.size.height
        ),
        (
            agent_copy.left(),
            agent_copy.top(),
            agent_copy.size.width,
            agent_copy.size.height
        ),
        "expected user and agent copy buttons to have distinct bounds"
    );
    let agent_click = point(agent_copy.left() + px(1.0), agent_copy.top() + px(1.0));
    window_cx.simulate_mouse_down(agent_click, MouseButton::Left, Modifiers::none());
    window_cx.simulate_mouse_up(agent_click, MouseButton::Left, Modifiers::none());
    window_cx.run_until_parked();
    assert_eq!(
        window_cx.read_from_clipboard().and_then(|item| item.text()),
        Some(agent_message)
    );

    let user_click = point(user_copy.left() + px(1.0), user_copy.top() + px(1.0));
    window_cx.simulate_mouse_down(user_click, MouseButton::Left, Modifiers::none());
    window_cx.simulate_mouse_up(user_click, MouseButton::Left, Modifiers::none());
    window_cx.run_until_parked();
    assert_eq!(
        window_cx.read_from_clipboard().and_then(|item| item.text()),
        Some(user_message)
    );
}

#[gpui::test]
async fn chat_composer_is_visible_in_workspace(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Test".to_owned(),
            }],
        },
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(720.0), px(400.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    window_cx
        .debug_bounds("chat-send-message")
        .expect("missing chat composer send button");
}

#[gpui::test]
async fn chat_composer_renders_model_and_effort_selectors(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Test".to_owned(),
            }],
        },
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(720.0), px(400.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    window_cx
        .debug_bounds("chat-model-selector")
        .expect("missing chat model selector");
    window_cx
        .debug_bounds("chat-thinking-effort-selector")
        .expect("missing chat thinking effort selector");

    let surface_bounds = window_cx
        .debug_bounds("chat-composer-surface")
        .expect("missing chat composer surface");
    let model_bounds = window_cx
        .debug_bounds("chat-model-selector")
        .expect("missing chat model selector");
    let send_bounds = window_cx
        .debug_bounds("chat-send-message")
        .expect("missing chat composer send button");

    let left_inset = model_bounds.left() - surface_bounds.left();
    let right_inset = surface_bounds.right() - send_bounds.right();
    assert!(
        left_inset >= px(18.0) && left_inset <= px(28.0),
        "unexpected chat composer inset: left={left_inset:?} surface={surface_bounds:?} model={model_bounds:?}",
    );
    let inset_diff = if left_inset > right_inset {
        left_inset - right_inset
    } else {
        right_inset - left_inset
    };
    assert!(
        inset_diff <= px(4.0),
        "expected symmetric insets: left={left_inset:?} right={right_inset:?} diff={inset_diff:?}",
    );
}

#[gpui::test]
async fn chat_composer_renders_attachments_inside_surface_in_order(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Test".to_owned(),
            }],
        },
    });

    state.apply(Action::ChatDraftAttachmentAdded {
        workspace_id,
        thread_id: default_thread_id(),
        id: 1,
        kind: luban_domain::ContextTokenKind::Image,
        anchor: 8,
    });
    state.apply(Action::ChatDraftAttachmentResolved {
        workspace_id,
        thread_id: default_thread_id(),
        id: 1,
        path: PathBuf::from("/tmp/missing.png"),
    });
    state.apply(Action::ChatDraftAttachmentAdded {
        workspace_id,
        thread_id: default_thread_id(),
        id: 2,
        kind: luban_domain::ContextTokenKind::Text,
        anchor: 0,
    });
    state.apply(Action::ChatDraftAttachmentResolved {
        workspace_id,
        thread_id: default_thread_id(),
        id: 2,
        path: PathBuf::from("/tmp/missing.txt"),
    });

    state.apply(Action::ChatDraftChanged {
        workspace_id,
        thread_id: default_thread_id(),
        text: "Hello world".to_owned(),
    });

    let (_view, window_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
        gpui_component::Root::new(view, window, cx)
    });
    window_cx.simulate_resize(size(px(720.0), px(420.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let surface = window_cx
        .debug_bounds("chat-composer-surface")
        .expect("missing chat composer surface");
    let attachments_row = window_cx
        .debug_bounds("chat-composer-attachments-row")
        .expect("missing chat composer attachments row");
    let input = window_cx
        .debug_bounds("chat-composer-input")
        .expect("missing chat composer input");
    let first = window_cx
        .debug_bounds("chat-composer-attachment-2")
        .expect("missing first attachment item");
    let second = window_cx
        .debug_bounds("chat-composer-attachment-1")
        .expect("missing second attachment item");

    assert!(
        first.top() >= surface.top() && first.bottom() <= surface.bottom(),
        "expected attachment to render inside composer surface: surface={surface:?} first={first:?}",
    );
    assert!(
        first.left() < second.left(),
        "expected attachments to render in anchor order: first={first:?} second={second:?}"
    );
    assert!(
        first.size.width >= px(CHAT_ATTACHMENT_FILE_WIDTH - 4.0)
            && first.size.width <= px(CHAT_ATTACHMENT_FILE_WIDTH + 4.0),
        "expected file attachment card to have fixed width: first={first:?}",
    );
    assert!(
        second.size.width >= px(CHAT_ATTACHMENT_THUMBNAIL_SIZE - 4.0)
            && second.size.width <= px(CHAT_ATTACHMENT_THUMBNAIL_SIZE + 4.0),
        "expected image attachment thumbnail to have fixed width: second={second:?}",
    );

    let top_inset = first.top() - surface.top();
    assert!(
        top_inset >= px(8.0),
        "expected attachment thumbnails to be inset from surface top: inset={top_inset:?} surface={surface:?} first={first:?}",
    );

    let gap_to_input = input.top() - first.bottom();
    assert!(
        gap_to_input >= px(10.0),
        "expected visible spacing between attachment thumbnails and input: gap={gap_to_input:?} first={first:?} input={input:?}",
    );

    let row_to_input_gap = input.top() - attachments_row.bottom();
    assert!(
        row_to_input_gap >= px(6.0),
        "expected attachment row to be separated from input: gap={row_to_input_gap:?} row={attachments_row:?} input={input:?}",
    );
}

#[gpui::test]
async fn chat_composer_can_send_attachments_without_text(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: Vec::new(),
        },
    });
    state.apply(Action::ChatDraftAttachmentAdded {
        workspace_id,
        thread_id: default_thread_id(),
        id: 1,
        kind: luban_domain::ContextTokenKind::Image,
        anchor: 0,
    });
    state.apply(Action::ChatDraftAttachmentResolved {
        workspace_id,
        thread_id: default_thread_id(),
        id: 1,
        path: PathBuf::from("/tmp/a.png"),
    });
    state.apply(Action::ChatDraftAttachmentAdded {
        workspace_id,
        thread_id: default_thread_id(),
        id: 2,
        kind: luban_domain::ContextTokenKind::Text,
        anchor: 0,
    });
    state.apply(Action::ChatDraftAttachmentResolved {
        workspace_id,
        thread_id: default_thread_id(),
        id: 2,
        path: PathBuf::from("/tmp/b.txt"),
    });
    state.apply(Action::ChatDraftChanged {
        workspace_id,
        thread_id: default_thread_id(),
        text: String::new(),
    });

    let view_slot: Arc<std::sync::Mutex<Option<gpui::Entity<LubanRootView>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let view_slot_for_window = view_slot.clone();

    let (_, window_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
        *view_slot_for_window.lock().expect("poisoned mutex") = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_slot
        .lock()
        .expect("poisoned mutex")
        .clone()
        .expect("missing view handle");

    window_cx.simulate_resize(size(px(720.0), px(420.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let send_bounds = window_cx
        .debug_bounds("chat-send-message")
        .expect("missing chat send button");
    window_cx.simulate_click(send_bounds.center(), Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let attachment_count = view.read_with(window_cx, |view, _| {
        view.debug_state()
            .workspace_conversation(workspace_id)
            .map(|c| c.draft_attachments.len())
            .unwrap_or(0)
    });
    assert_eq!(
        attachment_count, 0,
        "expected attachments to be cleared in state"
    );

    // `debug_bounds` can retain the last known bounds for removed elements. Prefer asserting on
    // state-level invariants for this behavior.

    let (entries, entries_len) = view.read_with(window_cx, |view, _| {
        let Some(conversation) = view.debug_state().workspace_conversation(workspace_id) else {
            return (Vec::new(), 0);
        };
        (conversation.entries.clone(), conversation.entries.len())
    });
    let last_user = entries.iter().rev().find_map(|entry| {
        let luban_domain::ConversationEntry::UserMessage { text } = entry else {
            return None;
        };
        Some(text.clone())
    });
    let expected = format!(
        "{}\n{}",
        context_token("image", "/tmp/a.png"),
        context_token("text", "/tmp/b.txt")
    );
    assert_eq!(
        last_user.as_deref(),
        Some(expected.as_str()),
        "expected latest user message to match (entries_len={entries_len})"
    );
}

#[gpui::test]
async fn chat_composer_inserts_attachments_at_anchor_positions(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: Vec::new(),
        },
    });
    state.apply(Action::ChatDraftChanged {
        workspace_id,
        thread_id: default_thread_id(),
        text: "HelloWorld".to_owned(),
    });
    state.apply(Action::ChatDraftAttachmentAdded {
        workspace_id,
        thread_id: default_thread_id(),
        id: 1,
        kind: luban_domain::ContextTokenKind::Image,
        anchor: 5,
    });
    state.apply(Action::ChatDraftAttachmentResolved {
        workspace_id,
        thread_id: default_thread_id(),
        id: 1,
        path: PathBuf::from("/tmp/a.png"),
    });

    let view_slot: Arc<std::sync::Mutex<Option<gpui::Entity<LubanRootView>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let view_slot_for_window = view_slot.clone();

    let (_, window_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
        *view_slot_for_window.lock().expect("poisoned mutex") = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_slot
        .lock()
        .expect("poisoned mutex")
        .clone()
        .expect("missing view handle");

    window_cx.simulate_resize(size(px(720.0), px(420.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let send_bounds = window_cx
        .debug_bounds("chat-send-message")
        .expect("missing chat send button");
    window_cx.simulate_click(send_bounds.center(), Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let last_user = view.read_with(window_cx, |view, _| {
        let conversation = view
            .debug_state()
            .workspace_conversation(workspace_id)
            .expect("missing conversation");
        conversation.entries.iter().rev().find_map(|entry| {
            let luban_domain::ConversationEntry::UserMessage { text } = entry else {
                return None;
            };
            Some(text.clone())
        })
    });

    let expected = format!("Hello\n{}\nWorld", context_token("image", "/tmp/a.png"));
    assert_eq!(last_user.as_deref(), Some(expected.as_str()));
}

#[gpui::test]
async fn user_messages_render_context_tokens_in_order(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: format!(
                    "before\n{}\nafter",
                    context_token("image", "/tmp/missing.png")
                ),
            }],
        },
    });

    let (_view, window_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
        gpui_component::Root::new(view, window, cx)
    });
    window_cx.simulate_resize(size(px(720.0), px(480.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let before = window_cx
        .debug_bounds("user-message-0-seg-0-plain-text")
        .expect("missing message segment before token");
    let attachment = window_cx
        .debug_bounds("conversation-user-attachment-0-0")
        .expect("missing attachment element");
    let after = window_cx
        .debug_bounds("user-message-0-tail-plain-text")
        .expect("missing message segment after token");

    assert!(
        before.top() < attachment.top() && attachment.top() < after.top(),
        "expected segments and token to render in order: before={before:?} attachment={attachment:?} after={after:?}",
    );
}

#[gpui::test]
async fn chat_composer_remains_visible_with_long_history(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let long_markdownish = std::iter::repeat_n(
            "- A very long bullet line that should wrap and force the history to overflow and scroll.\n",
            40,
        )
        .collect::<String>();

    let mut entries = Vec::new();
    for i in 0..12 {
        entries.push(ConversationEntry::UserMessage {
            text: format!("message {i} {long_markdownish}"),
        });
        entries.push(ConversationEntry::TurnDuration { duration_ms: 1000 });
    }

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries,
        },
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let send_bounds = window_cx
        .debug_bounds("chat-send-message")
        .expect("missing chat composer send button");
    let main_bounds = window_cx
        .debug_bounds("main-pane")
        .expect("missing debug bounds for main-pane");
    assert!(send_bounds.size.height > px(0.0));
    assert!(
        main_bounds.bottom() <= px(720.0) + px(1.0),
        "main={:?}",
        main_bounds
    );
    assert!(send_bounds.bottom() <= px(720.0) + px(1.0));
}

#[gpui::test]
async fn chat_scroll_position_is_saved_and_restored(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let long_markdownish = std::iter::repeat_n(
            "- A very long bullet line that should wrap and force the history to overflow and scroll.\n",
            40,
        )
        .collect::<String>();

    let mut entries = Vec::new();
    for i in 0..24 {
        entries.push(ConversationEntry::UserMessage {
            text: format!("message {i} {long_markdownish}"),
        });
        entries.push(ConversationEntry::TurnDuration { duration_ms: 1000 });
    }

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries,
        },
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(320.0)));
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let max_y10 = view.read_with(window_cx, |v, _| v.debug_chat_scroll_max_offset_y10());
    assert!(max_y10 > 0, "expected a scrollable chat history");

    let desired_offset_y10 = -((max_y10 / 2).max(10));
    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.chat_scroll_handle
                .set_offset(point(px(0.0), px(desired_offset_y10 as f32 / 10.0)));
            cx.notify();
        });
    });
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let current_offset_y10 = view.read_with(window_cx, |v, _| v.debug_chat_scroll_offset_y10());
    assert!(current_offset_y10 < 0, "expected a non-zero scroll offset");

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(Action::OpenDashboard, cx);
        });
    });
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let saved_offset_y10 = view.read_with(window_cx, |v, _| {
        v.debug_state()
            .workspace_chat_scroll_y10
            .get(&thread_key(workspace_id))
            .copied()
    });
    assert_eq!(saved_offset_y10, Some(current_offset_y10));

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.chat_scroll_handle.set_offset(point(px(0.0), px(0.0)));
            view.dispatch(Action::OpenWorkspace { workspace_id }, cx);
        });
    });
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let restored_offset_y10 = view.read_with(window_cx, |v, _| v.debug_chat_scroll_offset_y10());
    assert_eq!(restored_offset_y10, current_offset_y10);
}

#[gpui::test]
async fn chat_auto_scroll_follows_tail_on_new_entries(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let long_text = std::iter::repeat_n(
        "This is a long message that should wrap and increase history height.\n",
        80,
    )
    .collect::<String>();

    let mut entries = Vec::new();
    for i in 0..18 {
        entries.push(ConversationEntry::UserMessage {
            text: format!("message {i}\n{long_text}"),
        });
        entries.push(ConversationEntry::TurnDuration { duration_ms: 1000 });
    }

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: entries.clone(),
        },
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(320.0)));
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let max_y10_before = view.read_with(window_cx, |v, _| v.debug_chat_scroll_max_offset_y10());
    let offset_y10_before = view.read_with(window_cx, |v, _| v.debug_chat_scroll_offset_y10());
    assert!(max_y10_before > 0, "expected a scrollable chat history");
    assert!(
        (offset_y10_before + max_y10_before).abs() <= 250,
        "expected the chat to start near the bottom (offset_y10={offset_y10_before}, max_y10={max_y10_before})"
    );

    entries.push(ConversationEntry::UserMessage {
        text: "new message".to_owned(),
    });
    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(
                Action::ConversationLoaded {
                    workspace_id,
                    thread_id: default_thread_id(),
                    snapshot: ConversationSnapshot {
                        thread_id: Some("thread-1".to_owned()),
                        entries: entries.clone(),
                    },
                },
                cx,
            );
        });
    });

    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let max_y10_after = view.read_with(window_cx, |v, _| v.debug_chat_scroll_max_offset_y10());
    let offset_y10_after = view.read_with(window_cx, |v, _| v.debug_chat_scroll_offset_y10());
    assert!(max_y10_after >= max_y10_before, "expected content to grow");
    assert!(
        (offset_y10_after + max_y10_after).abs() <= 250,
        "expected the chat to stay near the bottom as new entries arrive (offset_y10={offset_y10_after}, max_y10={max_y10_after})"
    );
}

#[gpui::test]
async fn chat_auto_scroll_stops_when_user_scrolled_up(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let long_text = std::iter::repeat_n(
        "This is a long message that should wrap and increase history height.\n",
        80,
    )
    .collect::<String>();

    let mut entries = Vec::new();
    for i in 0..18 {
        entries.push(ConversationEntry::UserMessage {
            text: format!("message {i}\n{long_text}"),
        });
        entries.push(ConversationEntry::TurnDuration { duration_ms: 1000 });
    }

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: entries.clone(),
        },
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(320.0)));
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let max_y10 = view.read_with(window_cx, |v, _| v.debug_chat_scroll_max_offset_y10());
    assert!(max_y10 > 0, "expected a scrollable chat history");

    let user_offset_y10 = -((max_y10 / 2).max(10));
    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.chat_scroll_handle
                .set_offset(point(px(0.0), px(user_offset_y10 as f32 / 10.0)));
            cx.notify();
        });
    });
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let offset_y10_before = view.read_with(window_cx, |v, _| v.debug_chat_scroll_offset_y10());
    assert_eq!(offset_y10_before, user_offset_y10);

    entries.push(ConversationEntry::UserMessage {
        text: "new message".to_owned(),
    });
    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(
                Action::ConversationLoaded {
                    workspace_id,
                    thread_id: default_thread_id(),
                    snapshot: ConversationSnapshot {
                        thread_id: Some("thread-1".to_owned()),
                        entries: entries.clone(),
                    },
                },
                cx,
            );
        });
    });
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let offset_y10_after = view.read_with(window_cx, |v, _| v.debug_chat_scroll_offset_y10());
    assert_eq!(
        offset_y10_after, user_offset_y10,
        "expected new entries to not force-scroll when the user scrolled up"
    );
}

#[gpui::test]
async fn chat_auto_scroll_follows_tail_after_returning_to_chat(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let long_text = std::iter::repeat_n(
        "This is a long message that should wrap and increase history height.\n",
        80,
    )
    .collect::<String>();

    let mut entries = Vec::new();
    for i in 0..18 {
        entries.push(ConversationEntry::UserMessage {
            text: format!("message {i}\n{long_text}"),
        });
        entries.push(ConversationEntry::TurnDuration { duration_ms: 1000 });
    }

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: entries.clone(),
        },
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(320.0)));
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(Action::OpenDashboard, cx);
        });
    });
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    entries.push(ConversationEntry::UserMessage {
        text: "new message while away".to_owned(),
    });
    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(
                Action::ConversationLoaded {
                    workspace_id,
                    thread_id: default_thread_id(),
                    snapshot: ConversationSnapshot {
                        thread_id: Some("thread-1".to_owned()),
                        entries: entries.clone(),
                    },
                },
                cx,
            );
        });
    });

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(Action::OpenWorkspace { workspace_id }, cx);
        });
    });
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let max_y10_after = view.read_with(window_cx, |v, _| v.debug_chat_scroll_max_offset_y10());
    let offset_y10_after = view.read_with(window_cx, |v, _| v.debug_chat_scroll_offset_y10());
    assert!(
        (offset_y10_after + max_y10_after).abs() <= 250,
        "expected returning to chat to follow the newest entries (offset_y10={offset_y10_after}, max_y10={max_y10_after})"
    );
}

#[gpui::test]
async fn chat_composer_stays_in_viewport_with_terminal_and_long_history(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.right_pane = RightPane::Terminal;

    let long_text = std::iter::repeat_n(
        "This is a long message that should wrap and increase history height.\n",
        120,
    )
    .collect::<String>();

    let mut entries = Vec::new();
    for i in 0..18 {
        entries.push(ConversationEntry::UserMessage {
            text: format!("message {i}\n{long_text}"),
        });
        entries.push(ConversationEntry::TurnDuration { duration_ms: 1000 });
    }

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries,
        },
    });

    let (_view, window_cx) = cx.add_window_view(|_window, cx| {
        let mut view = LubanRootView::with_state(services, state, cx);
        view.terminal_enabled = true;
        view.workspace_terminal_errors
            .insert(thread_key(workspace_id), "stub terminal".to_owned());
        view
    });

    let window_size = size(px(1200.0), px(801.0));
    window_cx.simulate_resize(window_size);

    for _ in 0..4 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let main_bounds = window_cx
        .debug_bounds("main-pane")
        .expect("missing debug bounds for main-pane");
    assert!(
        main_bounds.bottom() <= window_size.height + px(1.0),
        "main={:?}",
        main_bounds
    );

    let send_bounds = window_cx
        .debug_bounds("chat-send-message")
        .expect("missing chat composer send button");
    assert!(send_bounds.size.height > px(0.0));
    assert!(
        send_bounds.bottom() <= window_size.height + px(1.0),
        "send={:?} main={:?}",
        send_bounds,
        main_bounds
    );
}

#[gpui::test]
async fn sidebar_buttons_remain_visible_with_long_titles(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.projects[0].name = "a".repeat(256);
    state.apply(Action::ToggleProjectExpanded { project_id });

    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w".repeat(128),
        branch_name: "repo/branch".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(320.0), px(480.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let header_bounds = window_cx
        .debug_bounds("project-header-0")
        .expect("missing debug bounds for project-header-0");
    let title_bounds = window_cx
        .debug_bounds("project-title-0")
        .expect("missing debug bounds for project-title-0");

    window_cx.simulate_mouse_move(header_bounds.center(), None, Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let toggle_bounds = window_cx
        .debug_bounds("project-toggle-0")
        .expect("missing debug bounds for project-toggle-0");
    let create_bounds = window_cx
        .debug_bounds("project-create-workspace-0")
        .expect("missing debug bounds for project-create-workspace-0");
    let settings_bounds = window_cx
        .debug_bounds("project-settings-0")
        .expect("missing debug bounds for project-settings-0");

    assert!(settings_bounds.right() <= header_bounds.right() + px(2.0));
    assert!(create_bounds.right() <= settings_bounds.left() + px(4.0));
    assert!(toggle_bounds.left() <= title_bounds.right() + px(8.0));

    let row_bounds = window_cx
        .debug_bounds("workspace-row-0-0")
        .expect("missing debug bounds for workspace-row-0-0");
    let archive_bounds = window_cx
        .debug_bounds("workspace-archive-0-0")
        .expect("missing debug bounds for workspace-archive-0-0");
    assert!(archive_bounds.right() <= row_bounds.right() + px(2.0));
}

#[gpui::test]
async fn workspace_icons_are_vertically_centered_in_rows(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(420.0), px(360.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let main_row = window_cx
        .debug_bounds("workspace-main-row-0")
        .expect("missing main workspace row");
    let main_icon = window_cx
        .debug_bounds("workspace-main-icon-0")
        .expect("missing main workspace icon");
    let main_dy = (main_icon.center().y - main_row.center().y).abs();
    assert!(
        main_dy <= px(2.0),
        "main icon should be vertically centered: icon={:?} row={:?}",
        main_icon,
        main_row
    );

    let row = window_cx
        .debug_bounds("workspace-row-0-0")
        .expect("missing workspace row");
    let icon = window_cx
        .debug_bounds("workspace-git-icon-branch-0-0")
        .expect("missing workspace icon");
    let dy = (icon.center().y - row.center().y).abs();
    assert!(
        dy <= px(2.0),
        "workspace icon should be vertically centered: icon={:?} row={:?}",
        icon,
        row
    );
}

#[gpui::test]
async fn dashboard_uses_full_window_and_renders_horizontal_columns(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w2".to_owned(),
        branch_name: "repo/w2".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w2"),
    });
    state.apply(Action::OpenDashboard);

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        window_cx.debug_bounds("sidebar").is_none(),
        "dashboard should not render the sidebar"
    );
    assert!(
        window_cx.debug_bounds("sidebar-resizer").is_none(),
        "dashboard should not render the sidebar resizer"
    );

    let titlebar = window_cx
        .debug_bounds("titlebar-sidebar")
        .expect("missing sidebar titlebar");
    let title = window_cx
        .debug_bounds("titlebar-dashboard-title")
        .expect("missing dashboard toggle");
    let center_dx = (title.center().x - titlebar.center().x).abs();
    assert!(
        center_dx <= px(2.0),
        "dashboard toggle should be centered in sidebar titlebar: title={:?} titlebar={:?}",
        title,
        titlebar
    );

    assert!(
        window_cx.debug_bounds("add-project-button").is_none(),
        "dashboard should not render add project button"
    );
    assert!(
        window_cx.debug_bounds("titlebar-dashboard-label").is_some(),
        "dashboard toggle label should switch to dashboard"
    );

    let start = window_cx
        .debug_bounds("dashboard-column-start")
        .expect("missing start column");
    let running = window_cx
        .debug_bounds("dashboard-column-running")
        .expect("missing running column");
    assert!(
        start.center().x < running.center().x,
        "dashboard columns should lay out horizontally: start={:?} running={:?}",
        start,
        running
    );

    let w1 = window_cx
        .debug_bounds("dashboard-card-0-w1")
        .expect("missing w1 card");
    let w2 = window_cx
        .debug_bounds("dashboard-card-0-w2")
        .expect("missing w2 card");
    let dx = (w1.center().x - w2.center().x).abs();
    let dy = (w1.center().y - w2.center().y).abs();
    assert!(
        dx <= px(2.0) && dy > px(8.0),
        "dashboard cards should stack vertically within a column: w1={:?} w2={:?}",
        w1,
        w2
    );
}

#[gpui::test]
async fn dashboard_toggle_returns_to_workspace_without_moving(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    let w1 = workspace_id_by_name(&state, "w1");
    state.apply(Action::OpenWorkspace { workspace_id: w1 });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(980.0), px(640.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let last_workspace = view.read_with(window_cx, |v, _| v.last_chat_workspace_id);
    assert_eq!(
        last_workspace,
        Some(thread_key(w1)),
        "expected last chat workspace to be set after rendering a workspace"
    );

    let initial_toggle = window_cx
        .debug_bounds("titlebar-dashboard-title")
        .expect("missing dashboard toggle");
    let initial_center_x = initial_toggle.center().x;

    window_cx.simulate_click(initial_toggle.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    let selected = view.read_with(window_cx, |v, _| v.debug_state().main_pane);
    assert!(
        matches!(selected, MainPane::Dashboard),
        "expected dashboard to open after clicking the toggle"
    );

    let last_workspace = view.read_with(window_cx, |v, _| v.last_workspace_before_dashboard);
    assert_eq!(
        last_workspace,
        Some(w1),
        "expected the dashboard toggle to remember the previous workspace"
    );

    let dashboard_toggle = window_cx
        .debug_bounds("titlebar-dashboard-title")
        .expect("missing dashboard toggle on dashboard");
    let dashboard_center_x = dashboard_toggle.center().x;
    assert!(
        (dashboard_center_x - initial_center_x).abs() <= px(2.0),
        "toggle should not move when entering dashboard: before={:?} after={:?}",
        initial_toggle,
        dashboard_toggle
    );
    assert!(
        window_cx.debug_bounds("titlebar-dashboard-label").is_some(),
        "expected toggle label to switch to dashboard on dashboard"
    );

    window_cx.simulate_click(dashboard_toggle.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    let selected = view.read_with(window_cx, |v, _| v.debug_state().main_pane);
    assert!(
        matches!(selected, MainPane::Workspace(id) if id == w1),
        "expected workspace view to return after clicking the toggle on dashboard"
    );
}

#[gpui::test]
async fn dashboard_renders_kanban_cards_and_preview(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    let w1 = workspace_id_by_name(&state, "w1");

    state.apply(Action::ConversationLoaded {
        workspace_id: w1,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "Hello".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "item-1".to_owned(),
                        text: "Hi from agent".to_owned(),
                    }),
                },
            ],
        },
    });

    state.apply(Action::OpenDashboard);

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        window_cx.debug_bounds("dashboard-column-start").is_some(),
        "expected start column to be rendered"
    );
    assert!(
        window_cx.debug_bounds("dashboard-card-0-main").is_none(),
        "main workspace should not be rendered in the dashboard"
    );
    let card_bounds = window_cx
        .debug_bounds("dashboard-card-0-w1")
        .expect("missing dashboard card for w1");

    window_cx.simulate_click(card_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    assert!(
        window_cx.debug_bounds("dashboard-preview").is_some(),
        "expected preview to be rendered after clicking a card"
    );
    assert!(
        window_cx.debug_bounds("workspace-chat-composer").is_some(),
        "expected the preview panel to render an editable chat composer"
    );

    let surface_bounds = window_cx
        .debug_bounds("chat-composer-surface")
        .expect("missing preview chat composer surface");
    let model_bounds = window_cx
        .debug_bounds("chat-model-selector")
        .expect("missing preview chat model selector");
    let send_bounds = window_cx
        .debug_bounds("chat-send-message")
        .expect("missing preview chat composer send button");

    let left_inset = model_bounds.left() - surface_bounds.left();
    let right_inset = surface_bounds.right() - send_bounds.right();
    let inset_diff = if left_inset > right_inset {
        left_inset - right_inset
    } else {
        right_inset - left_inset
    };
    assert!(
        left_inset >= px(14.0) && left_inset <= px(22.0),
        "unexpected preview chat composer inset: left={left_inset:?} surface={surface_bounds:?} model={model_bounds:?}",
    );
    assert!(
        inset_diff <= px(4.0),
        "expected symmetric preview insets: left={left_inset:?} right={right_inset:?} diff={inset_diff:?}",
    );

    let open_bounds = window_cx
        .debug_bounds("dashboard-preview-open-task")
        .expect("missing preview open button");
    window_cx.simulate_click(open_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    let selected = view.read_with(window_cx, |v, _| v.debug_state().main_pane);
    assert!(
        matches!(selected, MainPane::Workspace(id) if id == w1),
        "expected task view to open when clicking preview open button"
    );
}

#[gpui::test]
async fn dashboard_preview_panel_can_be_resized(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    let w1 = workspace_id_by_name(&state, "w1");
    state.apply(Action::ConversationLoaded {
        workspace_id: w1,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Hello".to_owned(),
            }],
        },
    });
    state.apply(Action::OpenDashboard);

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let card_bounds = window_cx
        .debug_bounds("dashboard-card-0-w1")
        .expect("missing dashboard card for w1");
    window_cx.simulate_click(card_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    let panel_before = view
        .read_with(window_cx, |v, _| {
            v.debug_inspector_bounds("dashboard-preview-panel")
        })
        .expect("missing preview panel");
    let resizer_bounds = view
        .read_with(window_cx, |v, _| {
            v.debug_inspector_bounds("dashboard-preview-resizer")
        })
        .expect("missing preview resizer");

    let start = resizer_bounds.center();
    let mid = point(start.x - px(24.0), start.y);
    let end = point(start.x - px(120.0), start.y);

    window_cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    window_cx.simulate_mouse_move(mid, Some(MouseButton::Left), Modifiers::none());
    window_cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
    window_cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let panel_after = view
        .read_with(window_cx, |v, _| {
            v.debug_inspector_bounds("dashboard-preview-panel")
        })
        .expect("missing preview panel after resize");
    assert!(
        panel_after.size.width > panel_before.size.width + px(40.0),
        "expected preview panel width to increase after dragging resizer: before={:?} after={:?}",
        panel_before.size,
        panel_after.size
    );
}

#[gpui::test]
async fn dashboard_preview_updates_chat_draft(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    let w1 = workspace_id_by_name(&state, "w1");
    state.apply(Action::ConversationLoaded {
        workspace_id: w1,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Hello".to_owned(),
            }],
        },
    });
    state.apply(Action::OpenDashboard);

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let card_bounds = window_cx
        .debug_bounds("dashboard-card-0-w1")
        .expect("missing dashboard card for w1");
    window_cx.simulate_click(card_bounds.center(), Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let draft_text = "Draft from dashboard preview".to_owned();
    window_cx.update(|window, app| {
        view.update(app, |view, cx| {
            let input = view.ensure_chat_input(window, cx);
            input.update(cx, |state, cx| {
                state.set_value(&draft_text, window, cx);
            });
        });
    });
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let saved = view.read_with(window_cx, |view, _| {
        view.debug_state()
            .workspace_conversation(w1)
            .map(|c| c.draft.clone())
    });
    assert_eq!(saved.as_deref(), Some(draft_text.as_str()));
}

#[gpui::test]
async fn dashboard_columns_show_inset_and_card_spacing(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });
    for name in ["w1", "w2", "w3"] {
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: name.to_owned(),
            branch_name: format!("repo/{name}"),
            worktree_path: PathBuf::from(format!("/tmp/luban/worktrees/repo/{name}")),
        });
    }
    state.apply(Action::OpenDashboard);

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let column_bounds = window_cx
        .debug_bounds("dashboard-column-start")
        .expect("missing dashboard start column");

    let mut card_bounds = [
        window_cx
            .debug_bounds("dashboard-card-0-w1")
            .expect("missing dashboard card for w1"),
        window_cx
            .debug_bounds("dashboard-card-0-w2")
            .expect("missing dashboard card for w2"),
        window_cx
            .debug_bounds("dashboard-card-0-w3")
            .expect("missing dashboard card for w3"),
    ];
    card_bounds.sort_by(|a, b| a.top().partial_cmp(&b.top()).unwrap());

    let first = card_bounds[0];
    assert!(
        first.left() >= column_bounds.left() + px(6.0),
        "expected card inset within column: card={first:?} column={column_bounds:?}",
    );
    assert!(
        first.right() <= column_bounds.right() - px(6.0),
        "expected card inset within column: card={first:?} column={column_bounds:?}",
    );

    for window in card_bounds.windows(2) {
        let prev = window[0];
        let next = window[1];
        let gap = next.top() - prev.bottom();
        assert!(
            gap >= px(6.0),
            "expected visible gap between dashboard cards: prev={prev:?} next={next:?} gap={gap:?}",
        );
    }
}

#[gpui::test]
async fn dashboard_preview_closes_when_clicking_outside(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    let w1 = workspace_id_by_name(&state, "w1");
    state.apply(Action::ConversationLoaded {
        workspace_id: w1,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Hello".to_owned(),
            }],
        },
    });
    state.apply(Action::OpenDashboard);

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let card_bounds = window_cx
        .debug_bounds("dashboard-card-0-w1")
        .expect("missing dashboard card for w1");
    window_cx.simulate_click(card_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    assert!(
        window_cx.debug_bounds("dashboard-preview").is_some(),
        "expected preview to be rendered after clicking a card"
    );
    assert!(
        view.read_with(window_cx, |v, _| v
            .debug_state()
            .dashboard_preview_workspace_id)
            .is_some(),
        "expected selected preview workspace to be set"
    );

    let backdrop_bounds = window_cx
        .debug_bounds("dashboard-preview-backdrop")
        .expect("missing preview backdrop");
    window_cx.simulate_event(MouseDownEvent {
        position: backdrop_bounds.center(),
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let preview_state_after_backdrop = view.read_with(window_cx, |v, _| {
        v.debug_state().dashboard_preview_workspace_id
    });
    assert!(
        preview_state_after_backdrop.is_none(),
        "expected preview workspace to clear after closing: backdrop={backdrop_bounds:?} state={preview_state_after_backdrop:?}"
    );

    window_cx.simulate_click(card_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    assert!(
        view.read_with(window_cx, |v, _| v
            .debug_state()
            .dashboard_preview_workspace_id)
            .is_some(),
        "expected preview to be re-openable after closing"
    );

    let titlebar_bounds = window_cx
        .debug_bounds("titlebar-main")
        .expect("missing titlebar main segment");
    window_cx.simulate_click(titlebar_bounds.center(), Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        view.read_with(window_cx, |v, _| v
            .debug_state()
            .dashboard_preview_workspace_id)
            .is_none(),
        "expected preview to close after clicking titlebar"
    );
}

#[gpui::test]
async fn dashboard_preview_blocks_kanban_scroll(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });
    for i in 0..24 {
        state.apply(Action::WorkspaceCreated {
            project_id,
            workspace_name: format!("w{i}"),
            branch_name: format!("repo/w{i}"),
            worktree_path: PathBuf::from(format!("/tmp/luban/worktrees/repo/w{i}")),
        });
    }
    state.apply(Action::OpenDashboard);

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let root_bounds = window_cx
        .debug_bounds("dashboard-root")
        .expect("missing dashboard root");
    window_cx.simulate_mouse_move(root_bounds.center(), None, Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    let count_before = view.read_with(window_cx, |v, _| v.debug_dashboard_scroll_wheel_events());
    window_cx.simulate_event(ScrollWheelEvent {
        position: root_bounds.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(200.0))),
        ..Default::default()
    });
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let card_after_scroll =
        view.read_with(window_cx, |v, _| v.debug_dashboard_scroll_wheel_events());
    assert!(
        card_after_scroll > count_before,
        "expected scroll wheel event to reach kanban: before={count_before} after={card_after_scroll}",
    );

    view.update(window_cx, |view, cx| {
        let workspace_id = workspace_id_by_name(&view.state, "w0");
        view.dispatch(Action::DashboardPreviewOpened { workspace_id }, cx);
    });
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let count_before_overlay_scroll =
        view.read_with(window_cx, |v, _| v.debug_dashboard_scroll_wheel_events());
    let backdrop_bounds = window_cx
        .debug_bounds("dashboard-preview-backdrop")
        .expect("missing preview backdrop");

    window_cx.simulate_mouse_move(backdrop_bounds.center(), None, Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();
    window_cx.simulate_event(ScrollWheelEvent {
        position: backdrop_bounds.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(200.0))),
        ..Default::default()
    });
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let card_after_overlay_scroll =
        view.read_with(window_cx, |v, _| v.debug_dashboard_scroll_wheel_events());
    assert!(
        card_after_overlay_scroll == count_before_overlay_scroll,
        "expected kanban scroll to be disabled while preview is open: before={count_before_overlay_scroll} after={card_after_overlay_scroll}",
    );
}

#[gpui::test]
async fn chat_column_remains_primary_when_terminal_is_visible(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.right_pane = RightPane::Terminal;

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Test".to_owned(),
            }],
        },
    });

    let (_view, window_cx) = cx.add_window_view(|_window, cx| {
        let mut view = LubanRootView::with_state(services, state, cx);
        view.terminal_enabled = true;
        view.workspace_terminal_errors
            .insert(thread_key(workspace_id), "stub terminal".to_owned());
        view
    });

    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let chat_bounds = window_cx
        .debug_bounds("workspace-chat-column")
        .expect("missing debug bounds for workspace-chat-column");
    let main_bounds = window_cx
        .debug_bounds("main-pane")
        .expect("missing debug bounds for main-pane");
    let right_pane_bounds = window_cx
        .debug_bounds("workspace-right-pane")
        .expect("missing debug bounds for workspace-right-pane");

    assert!(
        main_bounds.size.width >= px(640.0),
        "main={:?} right_pane={:?}",
        main_bounds.size,
        right_pane_bounds.size
    );
    assert!(
        chat_bounds.size.width >= px(600.0),
        "chat={:?} right_pane={:?}",
        chat_bounds.size,
        right_pane_bounds.size
    );
    assert!(
        chat_bounds.size.width >= right_pane_bounds.size.width + px(120.0),
        "chat={:?} right_pane={:?}",
        chat_bounds.size,
        right_pane_bounds.size
    );
}

#[gpui::test]
async fn terminal_pane_has_reasonable_default_width(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "repo/branch".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.right_pane = RightPane::Terminal;

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Test".to_owned(),
            }],
        },
    });

    let (_view, window_cx) = cx.add_window_view(|_window, cx| {
        let mut view = LubanRootView::with_state(services, state, cx);
        view.terminal_enabled = true;
        view.workspace_terminal_errors
            .insert(thread_key(workspace_id), "stub terminal".to_owned());
        view
    });

    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let right_pane_bounds = window_cx
        .debug_bounds("workspace-right-pane")
        .expect("missing debug bounds for workspace-right-pane");
    assert!(
        right_pane_bounds.size.width >= px(240.0),
        "right_pane={:?}",
        right_pane_bounds.size
    );
}

#[gpui::test]
async fn terminal_pane_can_be_resized_by_dragging_divider(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "repo/branch".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.right_pane = RightPane::Terminal;

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Test".to_owned(),
            }],
        },
    });

    let (view, window_cx) = cx.add_window_view(|_window, cx| {
        let mut view = LubanRootView::with_state(services, state, cx);
        view.terminal_enabled = true;
        view.workspace_terminal_errors
            .insert(thread_key(workspace_id), "stub terminal".to_owned());
        view
    });

    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let initial_right_pane = window_cx
        .debug_bounds("workspace-right-pane")
        .expect("missing debug bounds for workspace-right-pane");
    let initial_grid = view
        .read_with(window_cx, |v, _| v.debug_last_terminal_grid_size())
        .expect("missing initial terminal grid size");
    let resizer = window_cx
        .debug_bounds("terminal-pane-resizer")
        .expect("missing terminal pane resizer");

    let start = resizer.center();
    let mid = point(start.x - px(24.0), start.y);
    let end = point(start.x - px(200.0), start.y);

    window_cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
    window_cx.simulate_mouse_move(mid, Some(gpui::MouseButton::Left), Modifiers::none());
    window_cx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), Modifiers::none());
    window_cx.simulate_mouse_up(end, gpui::MouseButton::Left, Modifiers::none());

    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let resized_right_pane = window_cx
        .debug_bounds("workspace-right-pane")
        .expect("missing debug bounds for workspace-right-pane");
    assert!(
        resized_right_pane.size.width >= initial_right_pane.size.width + px(120.0),
        "initial={:?} resized={:?}",
        initial_right_pane.size,
        resized_right_pane.size
    );

    let resized_grid = view
        .read_with(window_cx, |v, _| v.debug_last_terminal_grid_size())
        .expect("missing resized terminal grid size");
    assert!(
        resized_grid.0 > initial_grid.0,
        "initial={initial_grid:?} resized={resized_grid:?}"
    );
}

#[gpui::test]
async fn terminal_is_reinitialized_after_session_exits(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    if std::path::Path::new("/bin/bash").exists() {
        unsafe {
            std::env::set_var("SHELL", "/bin/bash");
        }
    }

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "repo/branch".to_owned(),
        worktree_path: PathBuf::from("/tmp"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.right_pane = RightPane::Terminal;

    let (view, window_cx) = cx.add_window_view(|_window, cx| {
        let mut view = LubanRootView::with_state(services, state, cx);
        view.terminal_enabled = true;
        view
    });

    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    for _ in 0..12 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let initial_closed = view.read_with(window_cx, |view, _| {
        let key = thread_key(workspace_id);
        view.workspace_terminals.get(&key).map(|t| t.is_closed())
    });
    assert_eq!(
        initial_closed,
        Some(false),
        "expected a running terminal session for the workspace"
    );

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            let key = thread_key(workspace_id);
            if let Some(terminal) = view.workspace_terminals.get_mut(&key) {
                terminal.kill();
            }
            cx.notify();
        });
    });

    for _ in 0..24 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let after_closed = view.read_with(window_cx, |view, _| {
        let key = thread_key(workspace_id);
        view.workspace_terminals.get(&key).map(|t| t.is_closed())
    });
    assert_eq!(
        after_closed,
        Some(false),
        "expected terminal to be reinitialized after session exit"
    );
    assert!(
        view.read_with(window_cx, |view, _| {
            !view
                .workspace_terminal_errors
                .contains_key(&thread_key(workspace_id))
        }),
        "expected terminal to reinitialize without errors"
    );
}

#[gpui::test]
async fn project_header_has_extra_spacing_before_main_workspace(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::ToggleProjectExpanded { project_id });

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(240.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let header = window_cx
        .debug_bounds("project-header-0")
        .expect("missing project header");
    let main_row = window_cx
        .debug_bounds("workspace-main-row-0")
        .expect("missing main workspace row");
    let gap = main_row.top() - header.bottom();
    assert!(
        gap >= px(4.0),
        "expected extra spacing between project header and main row: gap={gap:?} header={header:?} main={main_row:?}"
    );
}

#[gpui::test]
async fn sidebar_can_be_resized_by_dragging_divider(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "repo/branch".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let (view, window_cx) = cx.add_window_view(|_window, cx| {
        let mut view = LubanRootView::with_state(services, state, cx);
        view.terminal_enabled = true;
        view
    });

    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let initial_sidebar = window_cx
        .debug_bounds("sidebar")
        .expect("missing debug bounds for sidebar");
    let resizer = window_cx
        .debug_bounds("sidebar-resizer")
        .expect("missing debug bounds for sidebar-resizer");

    let start = resizer.center();
    let mid = point(start.x + px(24.0), start.y);
    let end = point(start.x + px(200.0), start.y);

    window_cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
    window_cx.simulate_mouse_move(mid, Some(gpui::MouseButton::Left), Modifiers::none());
    window_cx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), Modifiers::none());
    window_cx.simulate_mouse_up(end, gpui::MouseButton::Left, Modifiers::none());

    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let resized_sidebar = window_cx
        .debug_bounds("sidebar")
        .expect("missing debug bounds for sidebar");
    assert!(
        resized_sidebar.size.width >= initial_sidebar.size.width + px(120.0),
        "initial={:?} resized={:?}",
        initial_sidebar.size,
        resized_sidebar.size
    );

    let saved_width = view.read_with(window_cx, |v, _| v.debug_state().sidebar_width);
    assert!(saved_width.is_some());
}

#[gpui::test]
async fn sidebar_projects_list_renders_scrollbar_when_overflowing(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    for i in 0..48 {
        state.apply(Action::AddProject {
            path: PathBuf::from(format!("/tmp/repo-{i}")),
        });
    }

    let (_view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(240.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    assert!(
        window_cx.debug_bounds("projects-scrollbar").is_some(),
        "expected a scrollbar for the projects list"
    );
}

#[gpui::test]
async fn sidebar_projects_list_can_scroll_with_wheel(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    for i in 0..64 {
        state.apply(Action::AddProject {
            path: PathBuf::from(format!("/tmp/repo-{i}")),
        });
    }

    let (view, window_cx) =
        cx.add_window_view(|_window, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(240.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let _ = view.read_with(window_cx, |v, _| v.debug_projects_scroll_offset_y10());

    let scroll_bounds = window_cx
        .debug_bounds("projects-scroll")
        .expect("missing debug bounds for projects scroll");
    let position = point(
        scroll_bounds.origin.x + px(10.0),
        scroll_bounds.origin.y + px(10.0),
    );

    let header_before = window_cx
        .debug_bounds("project-header-0")
        .expect("missing debug bounds for first project header");

    window_cx.simulate_mouse_move(position, None, Modifiers::none());
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    window_cx.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-160.0))),
        ..Default::default()
    });
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let header_after = window_cx.debug_bounds("project-header-0");
    assert!(
        header_after.is_none()
            || header_after.expect("checked is_some").origin.y != header_before.origin.y,
        "expected projects list to scroll"
    );
}

#[gpui::test]
async fn terminal_grid_updates_after_sidebar_resize(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "repo/branch".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.right_pane = RightPane::Terminal;

    let (view, window_cx) = cx.add_window_view(|_window, cx| {
        let mut view = LubanRootView::with_state(services, state, cx);
        view.terminal_enabled = true;
        view.workspace_terminal_errors
            .insert(thread_key(workspace_id), "stub terminal".to_owned());
        view
    });

    window_cx.simulate_resize(size(px(1200.0), px(720.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let initial_grid = view
        .read_with(window_cx, |v, _| v.debug_last_terminal_grid_size())
        .expect("missing initial terminal grid size");
    let resizer = window_cx
        .debug_bounds("sidebar-resizer")
        .expect("missing debug bounds for sidebar-resizer");

    let start = resizer.center();
    let mid = point(start.x + px(24.0), start.y);
    let end = point(start.x + px(160.0), start.y);
    window_cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
    window_cx.simulate_mouse_move(mid, Some(gpui::MouseButton::Left), Modifiers::none());
    window_cx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), Modifiers::none());
    window_cx.simulate_mouse_up(end, gpui::MouseButton::Left, Modifiers::none());

    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let resized_grid = view
        .read_with(window_cx, |v, _| v.debug_last_terminal_grid_size())
        .expect("missing resized terminal grid size");
    assert_ne!(resized_grid, initial_grid);
}

#[gpui::test]
async fn short_user_message_does_not_fill_max_width(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Test".to_owned(),
            }],
        },
    });

    let (_, window_cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(1200.0), px(800.0)));
    window_cx.run_until_parked();
    window_cx.refresh().unwrap();

    let bubble = window_cx
        .debug_bounds("conversation-user-bubble-0")
        .expect("missing debug bounds for conversation-user-bubble-0");
    assert!(bubble.size.width < px(300.0), "bubble={:?}", bubble.size);
}

#[gpui::test]
async fn long_in_progress_reasoning_does_not_expand_chat_column(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    state.apply(Action::SendAgentMessage {
        workspace_id,
        thread_id: default_thread_id(),
        text: "Test".to_owned(),
    });
    state.apply(Action::AgentEventReceived {
        workspace_id,
        thread_id: default_thread_id(),
        event: CodexThreadEvent::ItemStarted {
            item: CodexThreadItem::Reasoning {
                id: "item-1".to_owned(),
                text: "a".repeat(16_384),
            },
        },
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(720.0), px(800.0)));
    window_cx.refresh().unwrap();

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.expanded_agent_items.insert("item-1".to_owned());
            cx.notify();
        });
    });
    window_cx.refresh().unwrap();

    let column = window_cx
        .debug_bounds("workspace-chat-column")
        .expect("missing debug bounds for workspace-chat-column");
    assert!(column.size.width <= px(720.0));

    let bubble = window_cx
        .debug_bounds("conversation-user-bubble-0")
        .expect("missing debug bounds for conversation-user-bubble-0");
    assert!(bubble.right() <= column.right() + px(2.0));
    assert!(bubble.right() >= column.right() - px(8.0));
}

#[gpui::test]
async fn long_command_execution_does_not_expand_chat_column(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);

    let long_command = format!(
        "bash -lc 'echo {} && echo \"{}\" && printf \"%s\" {}'",
        "a".repeat(4096),
        "b".repeat(4096),
        "c".repeat(4096)
    );
    let long_output = format!("{}\n{}", "x".repeat(4096), "y".repeat(4096));

    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "Test".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::CommandExecution {
                        id: "item-1".to_owned(),
                        command: long_command,
                        aggregated_output: long_output,
                        exit_code: Some(0),
                        status: luban_domain::CodexCommandExecutionStatus::Completed,
                    }),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "item-2".to_owned(),
                        text: "Reply".to_owned(),
                    }),
                },
            ],
        },
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(720.0), px(800.0)));
    window_cx.refresh().unwrap();

    let turn_bounds = window_cx
        .debug_bounds("agent-turn-summary-agent-turn-0")
        .expect("missing debug bounds for agent-turn-summary-agent-turn-0");
    window_cx.simulate_click(turn_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    let row_bounds = window_cx
        .debug_bounds("agent-turn-item-summary-agent-turn-0-item-1")
        .expect("missing debug bounds for agent-turn-item-summary-agent-turn-0-item-1");
    window_cx.simulate_click(row_bounds.center(), Modifiers::none());
    window_cx.refresh().unwrap();

    let expanded = view.read_with(window_cx, |v, _| {
        v.expanded_agent_items.contains("agent-turn-0::item-1")
    });
    assert!(expanded);

    let column = window_cx
        .debug_bounds("workspace-chat-column")
        .expect("missing debug bounds for workspace-chat-column");
    assert!(column.size.width <= px(720.0));

    let bubble = window_cx
        .debug_bounds("conversation-user-bubble-0")
        .expect("missing debug bounds for conversation-user-bubble-0");
    assert!(bubble.right() <= column.right() + px(2.0));
    assert!(bubble.right() >= column.right() - px(8.0));
}

#[gpui::test]
async fn turn_duration_renders_below_messages(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "Test".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "item-1".to_owned(),
                        text: "Reply".to_owned(),
                    }),
                },
                ConversationEntry::TurnDuration { duration_ms: 6300 },
            ],
        },
    });

    let (_, window_cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    let bounds = window_cx
        .debug_bounds("turn-duration-2")
        .expect("missing debug bounds for turn-duration-2");
    assert!(bounds.size.width > px(0.0));
}

#[gpui::test]
async fn tail_turn_duration_renders_from_view_pending_state(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![ConversationEntry::UserMessage {
                text: "Test".to_owned(),
            }],
        },
    });

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));

    view.update(window_cx, |v, cx| {
        v.pending_turn_durations
            .insert(thread_key(workspace_id), Duration::from_millis(1234));
        cx.notify();
    });
    window_cx.refresh().unwrap();

    let bounds = window_cx
        .debug_bounds("chat-tail-turn-duration")
        .expect("missing debug bounds for chat-tail-turn-duration");
    assert!(bounds.size.width > px(0.0));
}

#[gpui::test]
async fn agent_messages_with_scoped_ids_render_in_multiple_turns(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "First".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "turn-a/item_0".to_owned(),
                        text: "A".to_owned(),
                    }),
                },
                ConversationEntry::TurnDuration { duration_ms: 1000 },
                ConversationEntry::UserMessage {
                    text: "Second".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "turn-b/item_0".to_owned(),
                        text: "B".to_owned(),
                    }),
                },
                ConversationEntry::TurnDuration { duration_ms: 2000 },
            ],
        },
    });

    let (_, window_cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    let a = window_cx
        .debug_bounds("conversation-agent-message-agent-turn-0-turn-a/item_0")
        .expect("missing agent message A");
    let b = window_cx
        .debug_bounds("conversation-agent-message-agent-turn-1-turn-b/item_0")
        .expect("missing agent message B");
    assert!(a.size.width > px(0.0));
    assert!(b.size.width > px(0.0));
}

#[gpui::test]
async fn chat_new_items_badge_is_not_rendered(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "abandon-about".to_owned(),
        branch_name: "luban/abandon-about".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
    });
    let workspace_id = workspace_id_by_name(&state, "abandon-about");
    state.main_pane = MainPane::Workspace(workspace_id);
    state.apply(Action::ConversationLoaded {
        workspace_id,
        thread_id: default_thread_id(),
        snapshot: ConversationSnapshot {
            thread_id: Some("thread-1".to_owned()),
            entries: vec![
                ConversationEntry::UserMessage {
                    text: "First".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "turn-a/item_0".to_owned(),
                        text: "A".to_owned(),
                    }),
                },
                ConversationEntry::TurnDuration { duration_ms: 1000 },
                ConversationEntry::UserMessage {
                    text: "Second".to_owned(),
                },
                ConversationEntry::CodexItem {
                    item: Box::new(CodexThreadItem::AgentMessage {
                        id: "turn-b/item_0".to_owned(),
                        text: "B".to_owned(),
                    }),
                },
            ],
        },
    });

    let (_, window_cx) = cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.refresh().unwrap();

    assert!(window_cx.debug_bounds("chat-new-items").is_none());
    assert!(window_cx.debug_bounds("chat-new-items-button").is_none());
}

#[gpui::test]
async fn chat_input_draft_is_isolated_per_workspace(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w2".to_owned(),
        branch_name: "repo/w2".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w2"),
    });
    let w1 = workspace_id_by_name(&state, "w1");
    let w2 = workspace_id_by_name(&state, "w2");

    state.apply(Action::ChatDraftChanged {
        workspace_id: w1,
        thread_id: default_thread_id(),
        text: "draft-1".to_owned(),
    });
    state.apply(Action::ChatDraftChanged {
        workspace_id: w2,
        thread_id: default_thread_id(),
        text: "draft-2".to_owned(),
    });
    state.main_pane = MainPane::Workspace(w1);

    let view_slot: Arc<std::sync::Mutex<Option<gpui::Entity<LubanRootView>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let view_slot_for_window = view_slot.clone();

    let (_, window_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
        *view_slot_for_window.lock().expect("poisoned mutex") = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_slot
        .lock()
        .expect("poisoned mutex")
        .clone()
        .expect("missing view handle");
    window_cx.refresh().unwrap();

    let value = view.read_with(window_cx, |v, cx| {
        v.chat_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
    });
    assert_eq!(value, Some("draft-1".to_owned()));

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(Action::OpenWorkspace { workspace_id: w2 }, cx);
        });
    });
    window_cx.refresh().unwrap();

    let value = view.read_with(window_cx, |v, cx| {
        v.chat_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
    });
    assert_eq!(value, Some("draft-2".to_owned()));

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(Action::OpenWorkspace { workspace_id: w1 }, cx);
        });
    });
    window_cx.refresh().unwrap();

    let value = view.read_with(window_cx, |v, cx| {
        v.chat_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
    });
    assert_eq!(value, Some("draft-1".to_owned()));
}

#[gpui::test]
async fn chat_input_cursor_moves_to_end_on_workspace_switch(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w2".to_owned(),
        branch_name: "repo/w2".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w2"),
    });
    let w1 = workspace_id_by_name(&state, "w1");
    let w2 = workspace_id_by_name(&state, "w2");

    state.apply(Action::ChatDraftChanged {
        workspace_id: w1,
        thread_id: default_thread_id(),
        text: "draft-1".to_owned(),
    });
    state.apply(Action::ChatDraftChanged {
        workspace_id: w2,
        thread_id: default_thread_id(),
        text: "draft-2".to_owned(),
    });
    state.main_pane = MainPane::Workspace(w1);

    let view_slot: Arc<std::sync::Mutex<Option<gpui::Entity<LubanRootView>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let view_slot_for_window = view_slot.clone();

    let (_, window_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
        *view_slot_for_window.lock().expect("poisoned mutex") = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_slot
        .lock()
        .expect("poisoned mutex")
        .clone()
        .expect("missing view handle");
    window_cx.refresh().unwrap();

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(Action::OpenWorkspace { workspace_id: w2 }, cx);
        });
    });
    window_cx.refresh().unwrap();

    let (value, cursor_at_end) = view.read_with(window_cx, |v, cx| {
        let Some(input) = v.chat_input.as_ref() else {
            return (None, false);
        };
        let state = input.read(cx);
        let value = state.value().to_string();
        let end = state.text().offset_to_position(state.text().len());
        (Some(value), state.cursor_position() == end)
    });
    assert_eq!(value, Some("draft-2".to_owned()));
    assert!(cursor_at_end);
}

struct FakeGhService {
    pr_numbers: HashMap<PathBuf, Option<PullRequestInfo>>,
    open_calls: Arc<AtomicUsize>,
}

impl ProjectWorkspaceService for FakeGhService {
    fn load_app_state(&self) -> Result<PersistedAppState, String> {
        Ok(PersistedAppState {
            projects: Vec::new(),
            sidebar_width: None,
            terminal_pane_width: None,
            agent_default_model_id: None,
            agent_default_thinking_effort: None,
            last_open_workspace_id: None,
            workspace_active_thread_id: HashMap::new(),
            workspace_open_tabs: HashMap::new(),
            workspace_archived_tabs: HashMap::new(),
            workspace_next_thread_id: HashMap::new(),
            workspace_chat_scroll_y10: HashMap::new(),
            workspace_unread_completions: HashMap::new(),
        })
    }

    fn save_app_state(&self, _snapshot: PersistedAppState) -> Result<(), String> {
        Ok(())
    }

    fn create_workspace(
        &self,
        _project_path: PathBuf,
        _project_slug: String,
    ) -> Result<CreatedWorkspace, String> {
        Ok(CreatedWorkspace {
            workspace_name: "abandon-about".to_owned(),
            branch_name: "luban/abandon-about".to_owned(),
            worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/abandon-about"),
        })
    }

    fn open_workspace_in_ide(&self, _worktree_path: PathBuf) -> Result<(), String> {
        Ok(())
    }

    fn archive_workspace(
        &self,
        _project_path: PathBuf,
        _worktree_path: PathBuf,
    ) -> Result<(), String> {
        Ok(())
    }

    fn ensure_conversation(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
    ) -> Result<(), String> {
        Ok(())
    }

    fn list_conversation_threads(
        &self,
        _project_slug: String,
        _workspace_name: String,
    ) -> Result<Vec<luban_domain::ConversationThreadMeta>, String> {
        Ok(Vec::new())
    }

    fn load_conversation(
        &self,
        _project_slug: String,
        _workspace_name: String,
        _thread_id: u64,
    ) -> Result<ConversationSnapshot, String> {
        Ok(ConversationSnapshot {
            thread_id: None,
            entries: Vec::new(),
        })
    }

    fn store_context_image(
        &self,
        project_slug: String,
        workspace_name: String,
        image: luban_domain::ContextImage,
    ) -> Result<PathBuf, String> {
        FakeService.store_context_image(project_slug, workspace_name, image)
    }

    fn store_context_text(
        &self,
        project_slug: String,
        workspace_name: String,
        text: String,
        extension: String,
    ) -> Result<PathBuf, String> {
        FakeService.store_context_text(project_slug, workspace_name, text, extension)
    }

    fn store_context_file(
        &self,
        project_slug: String,
        workspace_name: String,
        source_path: PathBuf,
    ) -> Result<PathBuf, String> {
        FakeService.store_context_file(project_slug, workspace_name, source_path)
    }

    fn run_agent_turn_streamed(
        &self,
        request: RunAgentTurnRequest,
        cancel: Arc<AtomicBool>,
        on_event: Arc<dyn Fn(CodexThreadEvent) + Send + Sync>,
    ) -> Result<(), String> {
        FakeService.run_agent_turn_streamed(request, cancel, on_event)
    }

    fn gh_is_authorized(&self) -> Result<bool, String> {
        Ok(true)
    }

    fn gh_pull_request_info(
        &self,
        worktree_path: PathBuf,
    ) -> Result<Option<PullRequestInfo>, String> {
        Ok(self.pr_numbers.get(&worktree_path).copied().flatten())
    }

    fn gh_open_pull_request(&self, _worktree_path: PathBuf) -> Result<(), String> {
        self.open_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[gpui::test]
async fn workspace_row_shows_pull_request_number_when_available(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w2".to_owned(),
        branch_name: "repo/w2".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w2"),
    });
    state.projects[0].expanded = true;

    let mut pr_numbers = HashMap::new();
    pr_numbers.insert(
        PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        Some(PullRequestInfo {
            number: 123,
            is_draft: false,
            state: PullRequestState::Open,
        }),
    );
    pr_numbers.insert(PathBuf::from("/tmp/luban/worktrees/repo/w2"), None);
    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeGhService {
        pr_numbers,
        open_calls: Arc::new(AtomicUsize::new(0)),
    });

    let view_slot: Arc<std::sync::Mutex<Option<gpui::Entity<LubanRootView>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let view_slot_for_window = view_slot.clone();

    let (_, window_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
        *view_slot_for_window.lock().expect("poisoned mutex") = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_slot
        .lock()
        .expect("poisoned mutex")
        .clone()
        .expect("missing view handle");

    window_cx.refresh().unwrap();
    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(Action::ClearError, cx);
        });
    });

    for _ in 0..8 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    assert!(
        window_cx.debug_bounds("workspace-pr-0-0").is_some(),
        "expected PR label to be rendered for workspace with PR number"
    );
    assert!(
        window_cx.debug_bounds("workspace-pr-0-1").is_none(),
        "expected PR label to be hidden for workspace without PR number"
    );
    assert!(
        window_cx
            .debug_bounds("workspace-git-icon-pr-0-0")
            .is_some(),
        "expected PR icon for workspace with PR number"
    );
    assert!(
        window_cx
            .debug_bounds("workspace-git-icon-branch-0-1")
            .is_some(),
        "expected branch icon for workspace without PR number"
    );
}

#[gpui::test]
async fn workspace_pr_label_opens_pull_request(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let mut state = AppState::new();
    state.apply(Action::AddProject {
        path: PathBuf::from("/tmp/repo"),
    });
    let project_id = state.projects[0].id;
    state.apply(Action::WorkspaceCreated {
        project_id,
        workspace_name: "w1".to_owned(),
        branch_name: "repo/w1".to_owned(),
        worktree_path: PathBuf::from("/tmp/luban/worktrees/repo/w1"),
    });
    state.projects[0].expanded = true;

    let mut pr_numbers = HashMap::new();
    pr_numbers.insert(
        PathBuf::from("/tmp/luban/worktrees/repo/w1"),
        Some(PullRequestInfo {
            number: 123,
            is_draft: false,
            state: PullRequestState::Open,
        }),
    );

    let open_calls = Arc::new(AtomicUsize::new(0));
    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeGhService {
        pr_numbers,
        open_calls: open_calls.clone(),
    });

    let view_slot: Arc<std::sync::Mutex<Option<gpui::Entity<LubanRootView>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let view_slot_for_window = view_slot.clone();

    let (_, window_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| LubanRootView::with_state(services, state, cx));
        *view_slot_for_window.lock().expect("poisoned mutex") = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_slot
        .lock()
        .expect("poisoned mutex")
        .clone()
        .expect("missing view handle");

    window_cx.refresh().unwrap();
    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(Action::ClearError, cx);
        });
    });

    for _ in 0..8 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let bounds = window_cx
        .debug_bounds("workspace-pr-0-0")
        .expect("missing PR label bounds");
    let click = bounds.center();

    window_cx.simulate_mouse_move(click, None, Modifiers::none());
    window_cx.simulate_mouse_down(click, MouseButton::Left, Modifiers::none());
    window_cx.simulate_mouse_up(click, MouseButton::Left, Modifiers::none());

    for _ in 0..4 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    assert_eq!(open_calls.load(Ordering::SeqCst), 1);
}

#[gpui::test]
async fn success_toast_is_shown_and_auto_dismissed(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);

    let services: Arc<dyn ProjectWorkspaceService> = Arc::new(FakeService);
    let state = AppState::new();

    let (view, window_cx) =
        cx.add_window_view(|_, cx| LubanRootView::with_state(services, state, cx));
    window_cx.simulate_resize(size(px(900.0), px(320.0)));

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dispatch(
                Action::AddProject {
                    path: PathBuf::from("/tmp/repo"),
                },
                cx,
            );
        });
    });

    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let toast_bounds = window_cx
        .debug_bounds("success-toast")
        .expect("missing success toast bounds");
    assert!(
        toast_bounds.size.height > px(0.0),
        "expected success toast to be visible after AddProject: {toast_bounds:?}"
    );

    let generation = view.read_with(window_cx, |view, _| view.success_toast_generation);
    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.dismiss_success_toast(generation, cx);
        });
    });
    for _ in 0..3 {
        window_cx.run_until_parked();
        window_cx.refresh().unwrap();
    }

    let toast_cleared = view.read_with(window_cx, |view, _| view.success_toast_message.is_none());
    assert!(
        toast_cleared,
        "expected success toast message to be cleared"
    );

    let toast_bounds_after = window_cx
        .debug_bounds("success-toast")
        .expect("missing success toast bounds after dismiss");
    assert!(
        toast_bounds_after.size.height <= px(0.0) + px(0.5),
        "expected success toast to be dismissed: {toast_bounds_after:?}"
    );
}
