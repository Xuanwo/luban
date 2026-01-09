use gpui::App;
use gpui_component::{Theme, ThemeConfig, ThemeConfigColors, ThemeMode};
use std::rc::Rc;

pub fn apply_linear_theme(cx: &mut App) {
    let light = Rc::new(linear_light_theme());
    Theme::global_mut(cx).apply_config(&light);
}

fn linear_light_theme() -> ThemeConfig {
    let mut colors = ThemeConfigColors::default();
    // Aligned with the high-fidelity web prototype design tokens (localhost:3000).
    colors.background = Some("#ffffff".into());
    colors.foreground = Some("#333333".into());
    colors.muted = Some("#f9fafb".into());
    colors.muted_foreground = Some("#6b7280".into());
    colors.border = Some("#e5e7eb".into());
    colors.input = Some("#e5e7eb".into());
    colors.secondary = Some("#f3f4f6".into());
    colors.secondary_foreground = Some("#4b5563".into());
    colors.primary = Some("#3b82f6".into());
    colors.primary_foreground = Some("#ffffff".into());
    colors.primary_hover = Some("#2563eb".into());
    colors.primary_active = Some("#1d4ed8".into());
    colors.accent = Some("#e0f2fe".into());
    colors.accent_foreground = Some("#1e3a8a".into());
    colors.ring = Some("#3b82f6".into());
    colors.scrollbar_thumb = Some("#cbd5e1".into());
    colors.scrollbar_thumb_hover = Some("#94a3b8".into());
    colors.sidebar = Some("#f9fafb".into());
    colors.sidebar_foreground = Some("#333333".into());
    colors.sidebar_border = Some("#e5e7eb".into());
    colors.sidebar_accent = Some("#e0f2fe".into());
    colors.sidebar_accent_foreground = Some("#1e3a8a".into());
    colors.sidebar_primary = Some("#3b82f6".into());
    colors.sidebar_primary_foreground = Some("#ffffff".into());
    colors.title_bar = Some("#ffffff".into());
    colors.title_bar_border = Some("#e5e7eb".into());
    colors.danger = Some("#fee2e2".into());
    colors.danger_hover = Some("#fecaca".into());
    colors.danger_active = Some("#fecaca".into());
    // Prototype uses Tailwind's red-500 for error emphasis.
    colors.danger_foreground = Some("#ef4444".into());
    colors.success = Some("#dcfce7".into());
    colors.success_hover = Some("#bbf7d0".into());
    colors.success_active = Some("#bbf7d0".into());
    // Prototype uses Tailwind's green-500 for success emphasis.
    colors.success_foreground = Some("#22c55e".into());
    colors.warning = Some("#fef9c3".into());
    colors.warning_hover = Some("#fef08a".into());
    colors.warning_active = Some("#fde047".into());
    // Prototype uses Tailwind's amber-500 for warning emphasis.
    colors.warning_foreground = Some("#f59e0b".into());
    colors.info = Some("#dbeafe".into());
    colors.info_hover = Some("#bfdbfe".into());
    colors.info_active = Some("#bfdbfe".into());
    colors.info_foreground = Some("#1e40af".into());
    // Sidebar PR links in the prototype use Tailwind's blue-400/300.
    colors.link = Some("#60a5fa".into());
    colors.link_hover = Some("#93c5fd".into());
    colors.link_active = Some("#3b82f6".into());
    colors.list = Some("#ffffff".into());
    colors.list_hover = Some("#f3f4f6".into());
    colors.list_active = Some("#e0f2fe".into());
    colors.list_active_border = Some("#3b82f6".into());
    colors.table = Some("#ffffff".into());
    colors.table_head = Some("#f9fafb".into());
    colors.table_head_foreground = Some("#6b7280".into());
    colors.table_hover = Some("#f3f4f6".into());
    colors.table_active = Some("#e0f2fe".into());
    colors.table_active_border = Some("#3b82f6".into());
    colors.table_row_border = Some("#e5e7eb".into());
    colors.popover = Some("#ffffff".into());
    colors.popover_foreground = Some("#333333".into());

    ThemeConfig {
        is_default: true,
        name: "Luban Prototype Light".into(),
        mode: ThemeMode::Light,
        font_size: Some(16.0),
        font_family: Some("Inter".into()),
        mono_font_family: Some("Geist Mono".into()),
        mono_font_size: Some(12.0),
        radius: Some(6),
        radius_lg: Some(8),
        shadow: Some(false),
        colors,
        highlight: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_matches_prototype_tokens() {
        let theme = linear_light_theme();
        assert_eq!(theme.name, "Luban Prototype Light");
        assert_eq!(theme.mode, ThemeMode::Light);
        assert_eq!(theme.font_size, Some(16.0));
        assert_eq!(
            theme.font_family.as_ref().map(|f| f.as_ref()),
            Some("Inter")
        );
        assert_eq!(
            theme.mono_font_family.as_ref().map(|f| f.as_ref()),
            Some("Geist Mono")
        );
        assert_eq!(theme.mono_font_size, Some(12.0));
        assert_eq!(theme.radius, Some(6));
        assert_eq!(theme.radius_lg, Some(8));

        let colors = theme.colors;
        assert_eq!(
            colors.background.as_ref().map(|c| c.as_ref()),
            Some("#ffffff")
        );
        assert_eq!(
            colors.foreground.as_ref().map(|c| c.as_ref()),
            Some("#333333")
        );
        assert_eq!(colors.border.as_ref().map(|c| c.as_ref()), Some("#e5e7eb"));
        assert_eq!(colors.primary.as_ref().map(|c| c.as_ref()), Some("#3b82f6"));
        assert_eq!(colors.sidebar.as_ref().map(|c| c.as_ref()), Some("#f9fafb"));
        assert_eq!(
            colors.sidebar_accent.as_ref().map(|c| c.as_ref()),
            Some("#e0f2fe")
        );
        assert_eq!(
            colors
                .sidebar_accent_foreground
                .as_ref()
                .map(|c| c.as_ref()),
            Some("#1e3a8a")
        );
    }
}
