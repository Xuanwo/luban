#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppearanceTheme {
    Light,
    Dark,
    #[default]
    System,
}

impl AppearanceTheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppearanceFonts {
    pub ui_font: String,
    pub chat_font: String,
    pub code_font: String,
    pub terminal_font: String,
}

impl Default for AppearanceFonts {
    fn default() -> Self {
        Self {
            ui_font: "Inter".to_owned(),
            chat_font: "Inter".to_owned(),
            code_font: "Geist Mono".to_owned(),
            terminal_font: "Geist Mono".to_owned(),
        }
    }
}
