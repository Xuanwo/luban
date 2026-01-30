use anyhow::Context as _;
use luban_api::{MentionItemKind, MentionItemSnapshot};

fn should_skip_dir(name: &str) -> bool {
    matches!(name, ".git" | "target" | "node_modules")
}

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

fn scan_paths_without_rg(
    worktree_path: &std::path::Path,
    needle_lower: &[u8],
    max_files: usize,
) -> anyhow::Result<Vec<(String, String)>> {
    let mut pending = vec![std::path::PathBuf::from("")];
    let mut out: Vec<(String, String)> = Vec::with_capacity(max_files.min(64));

    while let Some(rel_dir) = pending.pop() {
        let abs_dir = worktree_path.join(&rel_dir);
        let entries = std::fs::read_dir(&abs_dir)
            .with_context(|| format!("failed to read directory: {}", abs_dir.to_string_lossy()))?;

        for entry in entries {
            let entry = entry.with_context(|| {
                format!(
                    "failed to read directory entry: {}",
                    abs_dir.to_string_lossy()
                )
            })?;
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to stat: {}", entry.path().to_string_lossy()))?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            if file_type.is_dir() {
                if should_skip_dir(file_name.as_ref()) {
                    continue;
                }
                let next_rel_dir = if rel_dir.as_os_str().is_empty() {
                    std::path::PathBuf::from(file_name.as_ref())
                } else {
                    rel_dir.join(file_name.as_ref())
                };
                pending.push(next_rel_dir);
                continue;
            }

            if !file_type.is_file() {
                continue;
            }

            if !fuzzy_match_ascii(needle_lower, file_name.as_bytes()) {
                continue;
            }

            let rel_path = if rel_dir.as_os_str().is_empty() {
                file_name.to_string()
            } else {
                format!("{}/{}", rel_dir.to_string_lossy(), file_name)
            };
            let rel_path = rel_path.replace('\\', "/");
            let name_lower = file_name.to_ascii_lowercase();
            out.push((rel_path, name_lower));
            if out.len() >= max_files {
                return Ok(out);
            }
        }
    }

    Ok(out)
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
    let max_files = 200usize;

    let needle_lower = trimmed.to_ascii_lowercase();
    let needle_bytes = needle_lower.as_bytes();

    let mut file_paths: Vec<(String, String)> = match std::process::Command::new("rg")
        .args(["--files", "--hidden", "--sort", "path", "--iglob", &glob])
        .current_dir(worktree_path)
        .output()
    {
        Ok(output) => {
            if !output.status.success() && output.status.code() != Some(1) {
                anyhow::bail!(
                    "rg failed (status {}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let max_rg_lines = 2000usize;
            let mut out = Vec::with_capacity(max_files.min(64));
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let file = line.replace('\\', "/");
                let name_lower = {
                    let name = file.rsplit('/').next().unwrap_or(file.as_str());
                    if !fuzzy_match_ascii(needle_bytes, name.as_bytes()) {
                        continue;
                    }
                    name.to_ascii_lowercase()
                };
                out.push((file, name_lower));
                if out.len() >= max_files {
                    break;
                }
                if out.len() >= max_rg_lines {
                    break;
                }
            }
            out
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            scan_paths_without_rg(worktree_path, needle_bytes, max_files)?
        }
        Err(err) => return Err(err).context("failed to execute rg"),
    };

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
    fn scan_paths_without_rg_discovers_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("README.md"), b"hi").expect("write");
        std::fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        std::fs::write(dir.path().join("src").join("main.rs"), b"fn main() {}").expect("write");

        let out = scan_paths_without_rg(dir.path(), b"rdm", 200).expect("scan");
        assert!(out.iter().any(|(path, _)| path == "README.md"));
    }

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
