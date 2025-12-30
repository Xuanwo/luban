use gpui::App;
use gpui_component::{Theme, ThemeConfig, ThemeConfigColors, ThemeMode};
use std::rc::Rc;

pub fn apply_linear_theme(cx: &mut App) {
    let light = Rc::new(linear_light_theme());
    Theme::global_mut(cx).apply_config(&light);
}

fn linear_light_theme() -> ThemeConfig {
    let mut colors = ThemeConfigColors::default();
    colors.background = Some("#fbfbfc".into());
    colors.foreground = Some("#111827".into());
    colors.muted = Some("#f3f4f6".into());
    colors.muted_foreground = Some("#6b7280".into());
    colors.border = Some("#e5e7eb".into());
    colors.input = Some("#d1d5db".into());
    colors.secondary = Some("#f9fafb".into());
    colors.secondary_foreground = Some("#111827".into());
    colors.primary = Some("#5e6ad2".into());
    colors.primary_foreground = Some("#ffffff".into());
    colors.primary_hover = Some("#4f5bd5".into());
    colors.primary_active = Some("#4451d0".into());
    colors.accent = Some("#eef2ff".into());
    colors.accent_foreground = Some("#3730a3".into());
    colors.ring = Some("#5e6ad2".into());
    colors.sidebar = Some("#f7f7f8".into());
    colors.sidebar_foreground = Some("#111827".into());
    colors.sidebar_border = Some("#e5e7eb".into());
    colors.sidebar_accent = Some("#eef0f2".into());
    colors.sidebar_accent_foreground = Some("#111827".into());
    colors.sidebar_primary = Some("#5e6ad2".into());
    colors.sidebar_primary_foreground = Some("#ffffff".into());
    colors.title_bar = Some("#fbfbfc".into());
    colors.title_bar_border = Some("#e5e7eb".into());
    colors.danger = Some("#fee2e2".into());
    colors.danger_hover = Some("#fecaca".into());
    colors.danger_active = Some("#fecaca".into());
    colors.danger_foreground = Some("#991b1b".into());
    colors.success = Some("#dcfce7".into());
    colors.success_hover = Some("#bbf7d0".into());
    colors.success_active = Some("#bbf7d0".into());
    colors.success_foreground = Some("#166534".into());
    colors.warning = Some("#fef9c3".into());
    colors.warning_hover = Some("#fef08a".into());
    colors.warning_active = Some("#fde047".into());
    colors.warning_foreground = Some("#854d0e".into());
    colors.info = Some("#dbeafe".into());
    colors.info_hover = Some("#bfdbfe".into());
    colors.info_active = Some("#bfdbfe".into());
    colors.info_foreground = Some("#1e40af".into());
    colors.link = Some("#5e6ad2".into());
    colors.link_hover = Some("#4f5bd5".into());
    colors.link_active = Some("#4451d0".into());
    colors.list = Some("#fbfbfc".into());
    colors.list_hover = Some("#f3f4f6".into());
    colors.list_active = Some("#eef2ff".into());
    colors.list_active_border = Some("#5e6ad2".into());
    colors.table = Some("#fbfbfc".into());
    colors.table_head = Some("#f9fafb".into());
    colors.table_head_foreground = Some("#6b7280".into());
    colors.table_hover = Some("#f3f4f6".into());
    colors.table_active = Some("#eef2ff".into());
    colors.table_active_border = Some("#5e6ad2".into());
    colors.table_row_border = Some("#e5e7eb".into());

    ThemeConfig {
        is_default: true,
        name: "Luban Linear Light".into(),
        mode: ThemeMode::Light,
        font_size: Some(14.0),
        font_family: None,
        mono_font_family: None,
        mono_font_size: Some(12.0),
        radius: Some(8),
        radius_lg: Some(10),
        shadow: Some(false),
        colors,
        highlight: None,
    }
}
