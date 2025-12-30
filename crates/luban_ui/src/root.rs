use gpui::{AnyElement, Context, ElementId, IntoElement, Render, SharedString, div, prelude::*, px, rgb};
use luban_domain::{AppState, RightPaneTab, TimelineStatus};

pub struct LubanRootView {
    state: AppState,
    title: SharedString,
}

impl LubanRootView {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            state: AppState::demo(),
            title: "Luban".into(),
        }
    }

    fn select_project(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.selected_project = index;
        cx.notify();
    }

    fn select_timeline_item(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.selected_timeline_item = index;
        cx.notify();
    }

    fn set_right_tab(&mut self, tab: RightPaneTab, cx: &mut Context<Self>) {
        self.state.right_pane_tab = tab;
        cx.notify();
    }
}

impl Render for LubanRootView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_width = px(260.0);
        let right_width = px(420.0);
        let right_content = match self.state.right_pane_tab {
            RightPaneTab::Diff => right_diff_view(&self.state),
            RightPaneTab::Terminal => right_terminal_view(),
        };

        div()
            .size_full()
            .flex()
            .bg(rgb(0x0f111a))
            .text_color(rgb(0xd7dae0))
            .text_sm()
            .child(
                div()
                    .w(sidebar_width)
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x11131d))
                    .border_r_1()
                    .border_color(rgb(0x23263a))
                    .child(
                        div()
                            .h(px(44.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0x23263a))
                            .text_color(rgb(0xffffff))
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .id("sidebar-scroll")
                            .overflow_scroll()
                            .py_2()
                            .children(self.state.projects.iter().enumerate().map(|(i, p)| {
                                let is_selected = i == self.state.selected_project;
                                div()
                                    .px_3()
                                    .py_2()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .cursor_pointer()
                                    .bg(if is_selected {
                                        rgb(0x1a1d2c)
                                    } else {
                                        rgb(0x11131d)
                                    })
                                    .hover(|s| s.bg(rgb(0x1a1d2c)))
                                    .id(ElementId::named_usize("sidebar-project", i))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_project(i, cx);
                                    }))
                                    .child(p.name.clone())
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x0f111a))
                    .border_r_1()
                    .border_color(rgb(0x23263a))
                    .child(
                        div()
                            .h(px(44.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0x23263a))
                            .text_color(rgb(0xbac3d4))
                            .child("Timeline"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .id("timeline-scroll")
                            .overflow_scroll()
                            .py_2()
                            .children(
                                self.state
                                    .timeline
                                    .iter()
                                    .enumerate()
                                    .map(|(i, item)| {
                                        let is_selected = i == self.state.selected_timeline_item;
                                        let status_color = match item.status {
                                            TimelineStatus::Pending => rgb(0x6c7485),
                                            TimelineStatus::Running => rgb(0x3ea6ff),
                                            TimelineStatus::Done => rgb(0x3ddb77),
                                            TimelineStatus::Failed => rgb(0xff5c5c),
                                        };

                                        div()
                                            .px_3()
                                            .py_2()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .cursor_pointer()
                                            .bg(if is_selected {
                                                rgb(0x1a1d2c)
                                            } else {
                                                rgb(0x0f111a)
                                            })
                                            .hover(|s| s.bg(rgb(0x1a1d2c)))
                                            .id(ElementId::named_usize("timeline-item", i))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_timeline_item(i, cx);
                                            }))
                                            .child(
                                                div()
                                                    .w(px(8.0))
                                                    .h(px(8.0))
                                                    .rounded_full()
                                                    .bg(status_color),
                                            )
                                            .child(item.title.clone())
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .w(right_width)
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x11131d))
                    .child(
                        div()
                            .h(px(44.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_b_1()
                            .border_color(rgb(0x23263a))
                            .child(right_tab_button(cx, self.state.right_pane_tab, RightPaneTab::Diff))
                            .child(
                                right_tab_button(cx, self.state.right_pane_tab, RightPaneTab::Terminal),
                            ),
                    )
                    .child(right_content),
            )
    }
}

fn right_tab_button(
    cx: &mut Context<LubanRootView>,
    active: RightPaneTab,
    tab: RightPaneTab,
) -> impl IntoElement {
    let is_active = tab == active;
    let label = match tab {
        RightPaneTab::Diff => "Diff",
        RightPaneTab::Terminal => "Terminal",
    };
    let id = match tab {
        RightPaneTab::Diff => ElementId::named_usize("right-pane-tab", 0),
        RightPaneTab::Terminal => ElementId::named_usize("right-pane-tab", 1),
    };

    div()
        .px_3()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .bg(if is_active {
            rgb(0x1a1d2c)
        } else {
            rgb(0x11131d)
        })
        .hover(|s| s.bg(rgb(0x1a1d2c)))
        .id(id)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_right_tab(tab, cx);
        }))
        .child(label)
}

fn right_diff_view(state: &AppState) -> AnyElement {
    let title: SharedString = state
        .timeline
        .get(state.selected_timeline_item)
        .map(|i| i.title.clone().into())
        .unwrap_or_else(|| "No selection".into());

    div()
        .flex_1()
        .id("right-diff-scroll")
        .overflow_scroll()
        .p_3()
        .gap_2()
        .flex()
        .flex_col()
        .child(div().text_color(rgb(0xffffff)).child("Diff"))
        .child(div().text_color(rgb(0xbac3d4)).child(title))
        .child(
            div()
                .mt_2()
                .p_2()
                .rounded_md()
                .bg(rgb(0x0f111a))
                .border_1()
                .border_color(rgb(0x23263a))
                .font_family("SF Mono")
                .text_color(rgb(0xd7dae0))
                .child("+ placeholder diff output"),
        )
        .into_any_element()
}

fn right_terminal_view() -> AnyElement {
    div()
        .flex_1()
        .id("right-terminal-scroll")
        .overflow_scroll()
        .p_3()
        .gap_2()
        .flex()
        .flex_col()
        .child(div().text_color(rgb(0xffffff)).child("Terminal"))
        .child(
            div()
                .mt_2()
                .p_2()
                .rounded_md()
                .bg(rgb(0x0f111a))
                .border_1()
                .border_color(rgb(0x23263a))
                .font_family("SF Mono")
                .text_color(rgb(0xd7dae0))
                .children(["$ luban --help", "placeholder: integrated terminal"]),
        )
        .into_any_element()
}
