use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};
use luban_ui::LubanRootView;

mod services;
use services::GitWorkspaceService;

fn main() {
    Application::new().run(|cx: &mut App| {
        let services = GitWorkspaceService::new().expect("failed to init services");
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |_, cx| cx.new(|cx| LubanRootView::new(services.clone(), cx)),
        )
        .unwrap();

        cx.activate(true);
    });
}
