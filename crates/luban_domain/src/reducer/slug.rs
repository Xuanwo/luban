pub(super) fn sanitize_slug(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = false;

    for ch in input.chars() {
        let mapped = match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            'A'..='Z' => Some(ch.to_ascii_lowercase()),
            _ => None,
        };

        match mapped {
            Some(ch) => {
                out.push(ch);
                prev_dash = false;
            }
            None => {
                if !prev_dash && !out.is_empty() {
                    out.push('-');
                    prev_dash = true;
                }
            }
        }
    }

    while out.ends_with('-') {
        out.pop();
    }

    if out.is_empty() {
        "project".to_owned()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_slug;

    #[test]
    fn sanitize_slug_lowercases_and_collapses_separators() {
        assert_eq!(sanitize_slug("Hello, World!"), "hello-world");
        assert_eq!(sanitize_slug("Hello---World"), "hello-world");
        assert_eq!(sanitize_slug("Hello   World"), "hello-world");
    }

    #[test]
    fn sanitize_slug_keeps_ascii_digits() {
        assert_eq!(sanitize_slug("Repo 123"), "repo-123");
    }

    #[test]
    fn sanitize_slug_returns_fallback_when_empty() {
        assert_eq!(sanitize_slug(""), "project");
        assert_eq!(sanitize_slug("!!!"), "project");
        assert_eq!(sanitize_slug("   "), "project");
    }
}
