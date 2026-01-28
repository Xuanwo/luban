pub fn derive_thread_title(text: &str) -> String {
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return String::new();
    }

    first_line
        .chars()
        .take(crate::THREAD_TITLE_MAX_CHARS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::derive_thread_title;

    #[test]
    fn derive_thread_title_returns_empty_for_whitespace() {
        assert_eq!(derive_thread_title(""), "");
        assert_eq!(derive_thread_title("   \n  "), "");
    }

    #[test]
    fn derive_thread_title_uses_first_line_only() {
        assert_eq!(derive_thread_title("hello\nworld"), "hello");
    }

    #[test]
    fn derive_thread_title_truncates_to_limit() {
        let input = "a".repeat(crate::THREAD_TITLE_MAX_CHARS + 10);
        let out = derive_thread_title(&input);
        assert_eq!(out.len(), crate::THREAD_TITLE_MAX_CHARS);
    }
}
