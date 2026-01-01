use gpui::{
    App, Application, Bounds, KeyBinding, WindowBounds, WindowOptions, prelude::*, px, size,
};
use gpui_component::{Root, Theme, ThemeMode};
use gpui_ghostty_terminal::view::{Copy, Paste, SelectAll};
use luban_ui::{LubanRootView, apply_linear_theme};

mod app_assets;
mod services;
mod sqlite_store;
use services::GitWorkspaceService;

fn init_components(cx: &mut App) {
    gpui_component::init(cx);
    apply_linear_theme(cx);
    Theme::change(ThemeMode::Light, None, cx);
}

fn main() {
    Application::new()
        .with_assets(app_assets::AppAssets::default())
        .run(|cx: &mut App| {
            init_components(cx);
            cx.bind_keys([
                KeyBinding::new("cmd-a", SelectAll, None),
                KeyBinding::new("cmd-c", Copy, None),
                KeyBinding::new("cmd-v", Paste, None),
            ]);

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
            assert_eq!(Theme::global(app).font_size, px(14.0));
            assert_eq!(Theme::global(app).radius, px(8.0));
            assert_eq!(
                Theme::global(app).theme_name().as_ref(),
                "Luban Linear Light"
            );
        });
    }
}
