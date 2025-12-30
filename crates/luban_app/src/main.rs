use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};
use luban_ui::LubanRootView;

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| LubanRootView::new(cx)),
        )
        .unwrap();

        cx.activate(true);
    });
}
