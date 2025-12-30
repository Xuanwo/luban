use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_component::{Root, Theme, ThemeMode};
use luban_ui::LubanRootView;

mod services;
use services::GitWorkspaceService;

fn init_components(cx: &mut App) {
    gpui_component::init(cx);
    Theme::change(ThemeMode::Light, None, cx);
}

fn main() {
    Application::new().run(|cx: &mut App| {
        init_components(cx);

        let services = GitWorkspaceService::new().expect("failed to init services");
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| LubanRootView::new(services.clone(), cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();

        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_components_forces_light_theme() {
        let cx = gpui::TestAppContext::single();
        cx.update(init_components);

        cx.read(|app| {
            assert_eq!(Theme::global(app).mode, ThemeMode::Light);
        });
    }
}
