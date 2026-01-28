use super::strings::normalize_string;

pub(super) fn normalize_font(raw: Option<&str>, fallback: &str) -> String {
    normalize_string(raw, fallback, 128)
}
