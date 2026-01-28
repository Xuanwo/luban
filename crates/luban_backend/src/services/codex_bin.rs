use luban_domain::paths;
use std::path::{Path, PathBuf};

pub(super) fn codex_executable() -> PathBuf {
    if let Some(explicit) = std::env::var_os(paths::LUBAN_CODEX_BIN_ENV).map(PathBuf::from) {
        return explicit;
    }

    for candidate in default_codex_candidates() {
        if let Some(found) = canonicalize_executable(&candidate) {
            return found;
        }
    }

    PathBuf::from("codex")
}

fn default_codex_candidates() -> DefaultCodexCandidates {
    // Avoid depending on developer machine installation paths during unit tests.
    if cfg!(test) {
        return DefaultCodexCandidates::test_only();
    }
    DefaultCodexCandidates::new()
}

struct DefaultCodexCandidates {
    idx: u8,
    cargo_home: Option<PathBuf>,
}

impl DefaultCodexCandidates {
    fn new() -> Self {
        Self {
            idx: 0,
            cargo_home: std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".cargo/bin/codex")),
        }
    }

    fn test_only() -> Self {
        Self {
            idx: 3,
            cargo_home: None,
        }
    }
}

impl Iterator for DefaultCodexCandidates {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let idx = self.idx;
            self.idx = self.idx.saturating_add(1);

            match idx {
                // Homebrew (Apple Silicon / Intel)
                0 => return Some(PathBuf::from("/opt/homebrew/bin/codex")),
                1 => return Some(PathBuf::from("/usr/local/bin/codex")),
                // Rust/Cargo installs (less common, but cheap to check)
                2 => {
                    if let Some(p) = self.cargo_home.take() {
                        return Some(p);
                    }
                    continue;
                }
                // Last resort: rely on PATH (useful for terminal-launched dev)
                3 => return Some(PathBuf::from("codex")),
                _ => return None,
            }
        }
    }
}

fn canonicalize_executable(path: &Path) -> Option<PathBuf> {
    let resolved = std::fs::canonicalize(path)
        .ok()
        .unwrap_or_else(|| path.to_path_buf());
    if !resolved.is_file() {
        return None;
    }
    if !is_executable_file(&resolved) {
        return None;
    }
    Some(resolved)
}

fn is_executable_file(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        let mode = meta.permissions().mode();
        (mode & 0o111) != 0
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_candidates_are_test_only_in_unit_tests() {
        let candidates: Vec<_> = default_codex_candidates().collect();
        assert_eq!(candidates, vec![PathBuf::from("codex")]);
    }

    #[test]
    fn codex_executable_prefers_env_override() {
        let _guard = crate::test_support::EnvVarGuard::set(paths::LUBAN_CODEX_BIN_ENV, "my-codex");
        assert_eq!(codex_executable(), PathBuf::from("my-codex"));
    }
}
