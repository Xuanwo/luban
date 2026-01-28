fn normalize_string(raw: Option<&str>, fallback: &str, max_len: usize) -> String {
    raw.map(str::trim)
        .filter(|v| !v.is_empty())
        .filter(|v| v.len() <= max_len)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| fallback.to_owned())
}

pub(super) fn normalize_font(raw: Option<&str>, fallback: &str) -> String {
    normalize_string(raw, fallback, 128)
}
