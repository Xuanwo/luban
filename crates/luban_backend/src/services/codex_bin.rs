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

fn default_codex_candidates() -> Vec<PathBuf> {
    if cfg!(test) {
        // Avoid depending on developer machine installation paths during unit tests.
        return Vec::new();
    }

    let mut out = Vec::new();

    // Homebrew (Apple Silicon / Intel)
    out.push(PathBuf::from("/opt/homebrew/bin/codex"));
    out.push(PathBuf::from("/usr/local/bin/codex"));

    // Rust/Cargo installs (less common, but cheap to check).
    if let Some(home) = std::env::var_os("HOME") {
        out.push(PathBuf::from(home).join(".cargo/bin/codex"));
    }

    // Last resort: rely on PATH (useful for terminal-launched dev).
    out.push(PathBuf::from("codex"));

    out
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
