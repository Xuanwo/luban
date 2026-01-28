use anyhow::Context as _;
use luban_api::{MentionItemKind, MentionItemSnapshot};

fn append_escaped_glob_char(out: &mut String, ch: char) {
    match ch {
        '*' | '?' | '[' | ']' | '{' | '}' | '!' => {
            out.push('\\');
            out.push(ch);
        }
        other => out.push(other),
    }
}

fn fuzzy_glob_pattern(query: &str) -> String {
    let mut out = String::with_capacity("**/*".len() + query.len() * 2);
    out.push_str("**/*");
    for ch in query.chars() {
        append_escaped_glob_char(&mut out, ch);
        out.push('*');
    }
    out
}

fn fuzzy_match_ascii(needle_lower: &[u8], haystack: &[u8]) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    let mut hi = 0usize;
    for &b in needle_lower {
        while hi < haystack.len() && haystack[hi].to_ascii_lowercase() != b {
            hi += 1;
        }
        if hi == haystack.len() {
            return false;
        }
        hi += 1;
    }
    true
}

pub fn search_workspace_mentions(
    worktree_path: &std::path::Path,
    query: &str,
) -> anyhow::Result<Vec<MentionItemSnapshot>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let glob = fuzzy_glob_pattern(trimmed);
    let output = std::process::Command::new("rg")
        .args(["--files", "--hidden", "--sort", "path", "--iglob", &glob])
        .current_dir(worktree_path)
        .output()
        .context("failed to execute rg")?;

    if !output.status.success() && output.status.code() != Some(1) {
        anyhow::bail!(
            "rg failed (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let max_rg_lines = 2000usize;
    let max_files = 200usize;
    let mut candidate_paths = Vec::with_capacity(max_rg_lines.min(256));
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        candidate_paths.push(line.replace('\\', "/"));
        if candidate_paths.len() >= max_rg_lines {
            break;
        }
    }

    let needle_lower = trimmed.to_ascii_lowercase();
    let needle_bytes = needle_lower.as_bytes();

    let mut file_paths: Vec<(String, String)> = Vec::with_capacity(max_files.min(64));
    for file in candidate_paths.into_iter() {
        let name_lower = {
            let name = file.rsplit('/').next().unwrap_or(file.as_str());
            if !fuzzy_match_ascii(needle_bytes, name.as_bytes()) {
                continue;
            }
            name.to_ascii_lowercase()
        };
        file_paths.push((file, name_lower));
        if file_paths.len() >= max_files {
            break;
        }
    }

    file_paths.sort_by(|(a_path, a_name_lower), (b_path, b_name_lower)| {
        a_name_lower
            .cmp(b_name_lower)
            .then_with(|| a_path.cmp(b_path))
    });

    let mut folder_paths = std::collections::BTreeSet::new();
    for (file, _) in &file_paths {
        let path = std::path::Path::new(file);
        let mut parent = path.parent();
        while let Some(dir) = parent {
            let s = dir.to_string_lossy().replace('\\', "/");
            if s.is_empty() || s == "." {
                break;
            }
            let name = s.rsplit('/').next().unwrap_or(&s);
            if fuzzy_match_ascii(needle_bytes, name.as_bytes()) {
                folder_paths.insert(format!("{}/", s.trim_end_matches('/')));
            }
            parent = dir.parent();
        }
    }

    let mut items = Vec::new();
    for folder in folder_paths.into_iter() {
        let name = folder
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&folder)
            .to_owned();
        items.push(MentionItemSnapshot {
            id: format!("folder:{folder}"),
            name,
            path: folder,
            kind: MentionItemKind::Folder,
        });
        if items.len() >= 20 {
            return Ok(items);
        }
    }

    for (file, _) in file_paths.into_iter() {
        let name = file.rsplit('/').next().unwrap_or(&file).to_owned();
        items.push(MentionItemSnapshot {
            id: format!("file:{file}"),
            name,
            path: file,
            kind: MentionItemKind::File,
        });
        if items.len() >= 20 {
            break;
        }
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_ascii_is_ordered() {
        let needle = "rdm".as_bytes();
        assert!(fuzzy_match_ascii(needle, b"README.md"));
        assert!(fuzzy_match_ascii(needle, b"readme.md"));
        assert!(!fuzzy_match_ascii(needle, b"mdrea"));
    }

    #[test]
    fn fuzzy_glob_pattern_escapes_glob_chars() {
        assert_eq!(fuzzy_glob_pattern("*?[!]"), "**/*\\**\\?*\\[*\\!*\\]*");
    }
}
