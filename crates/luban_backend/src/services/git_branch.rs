use std::{path::Path, process::Command};

pub(crate) fn normalize_branch_suffix(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut value = trimmed;
    if let Some(stripped) = value.strip_prefix("refs/heads/") {
        value = stripped;
    }
    if let Some(stripped) = value.strip_prefix("luban/") {
        value = stripped;
    }

    let mut out = String::new();
    let mut prev_hyphen = false;
    for ch in value.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };
        if next == '-' {
            if prev_hyphen {
                continue;
            }
            prev_hyphen = true;
            out.push('-');
            continue;
        }
        prev_hyphen = false;
        out.push(next);
    }

    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        return None;
    }

    const MAX_SUFFIX_LEN: usize = 24;
    let limited = trimmed.chars().take(MAX_SUFFIX_LEN).collect::<String>();
    let limited = limited.trim_matches('-').to_owned();
    if limited.is_empty() {
        return None;
    }
    Some(limited)
}

pub(crate) fn branch_exists(repo_path: &Path, branch_name: &str) -> bool {
    let branch_ref = format!("refs/heads/{branch_name}");
    Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &branch_ref])
        .current_dir(repo_path)
        .status()
        .ok()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::normalize_branch_suffix;

    #[test]
    fn normalize_branch_suffix_strips_prefixes_and_sanitizes() {
        assert_eq!(
            normalize_branch_suffix("refs/heads/luban/Foo_Bar").as_deref(),
            Some("foo-bar")
        );
        assert_eq!(
            normalize_branch_suffix("luban/Hello---World").as_deref(),
            Some("hello-world")
        );
        assert_eq!(
            normalize_branch_suffix("  hello world  ").as_deref(),
            Some("hello-world")
        );
        assert_eq!(normalize_branch_suffix("   "), None);
        assert_eq!(normalize_branch_suffix("luban/---"), None);
    }

    #[test]
    fn normalize_branch_suffix_limits_length() {
        let input = "refs/heads/luban/abcdefghijklmnopqrstuvwxyz";
        let suffix = normalize_branch_suffix(input).expect("suffix should be present");
        assert_eq!(suffix.len(), 24);
        assert_eq!(suffix, "abcdefghijklmnopqrstuvwx");
    }
}
