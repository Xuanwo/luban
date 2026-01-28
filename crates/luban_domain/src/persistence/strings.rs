pub(super) fn normalize_optional_string(raw: Option<&str>, max_len: usize) -> Option<String> {
    raw.map(str::trim)
        .filter(|v| !v.is_empty())
        .filter(|v| v.len() <= max_len)
        .map(ToOwned::to_owned)
}

pub(super) fn normalize_string(raw: Option<&str>, fallback: &str, max_len: usize) -> String {
    normalize_optional_string(raw, max_len).unwrap_or_else(|| fallback.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_optional_string_trims_and_enforces_max_len() {
        assert_eq!(
            normalize_optional_string(Some("  abc  "), 3),
            Some("abc".to_owned())
        );
        assert_eq!(normalize_optional_string(Some("  abc  "), 2), None);
        assert_eq!(normalize_optional_string(Some("   "), 10), None);
        assert_eq!(normalize_optional_string(None, 10), None);
    }

    #[test]
    fn normalize_string_falls_back_on_invalid_input() {
        assert_eq!(normalize_string(Some("  abc  "), "fallback", 3), "abc");
        assert_eq!(normalize_string(Some("  abc  "), "fallback", 2), "fallback");
        assert_eq!(normalize_string(Some("   "), "fallback", 10), "fallback");
        assert_eq!(normalize_string(None, "fallback", 10), "fallback");
    }
}
