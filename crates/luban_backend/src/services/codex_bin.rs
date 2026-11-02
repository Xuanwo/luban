use luban_domain::paths;
use std::path::{Path, PathBuf};

#[cfg(not(test))]
use std::sync::OnceLock;

pub(super) fn codex_executable() -> PathBuf {
    if let Some(explicit) = std::env::var_os(paths::LUBAN_CODEX_BIN_ENV).map(PathBuf::from) {
        return explicit;
    }

    for candidate in default_codex_candidates() {
        if let Some(found) = canonicalize_executable(&candidate) {
            return found;
        }
    }

    if let Some(found) = shell_discovered_codex_candidate() {
        return found;
    }

    PathBuf::from("codex")
}

#[cfg(not(test))]
fn shell_discovered_codex_candidate() -> Option<PathBuf> {
    static SHELL_DISCOVERED: OnceLock<Option<PathBuf>> = OnceLock::new();
    SHELL_DISCOVERED
        .get_or_init(|| {
            let shell_path = default_shell_path()?;
            discover_codex_via_shell(&shell_path)
        })
        .clone()
}

#[cfg(test)]
fn shell_discovered_codex_candidate() -> Option<PathBuf> {
    None
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

#[cfg_attr(test, allow(dead_code))]
fn default_shell_path() -> Option<PathBuf> {
    if let Some(shell) = std::env::var_os("SHELL") {
        let trimmed = shell.to_string_lossy();
        if !trimmed.trim().is_empty() {
            let path = PathBuf::from(shell);
            if path.exists() {
                return Some(path);
            }
        }
    }

    #[cfg(windows)]
    {
        if let Some(comspec) = std::env::var_os("COMSPEC") {
            let trimmed = comspec.to_string_lossy();
            if !trimmed.trim().is_empty() {
                return Some(PathBuf::from(comspec));
            }
        }
        return Some(PathBuf::from("C:\\Windows\\System32\\cmd.exe"));
    }

    #[cfg(not(windows))]
    {
        for candidate in ["/bin/zsh", "/bin/bash", "/bin/sh"] {
            let path = PathBuf::from(candidate);
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

fn discover_codex_via_shell(shell_path: &Path) -> Option<PathBuf> {
    let lookup_command = if cfg!(windows) {
        "where codex"
    } else {
        "command -v codex 2>/dev/null || true"
    };

    let output = std::process::Command::new(shell_path)
        .args(shell_lookup_args(shell_path, lookup_command))
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for candidate in parse_absolute_path_tokens(&stdout) {
        if !looks_like_codex_executable_name(&candidate) {
            continue;
        }
        if let Some(found) = canonicalize_executable(&candidate) {
            return Some(found);
        }
    }

    None
}

fn shell_lookup_args(shell_path: &Path, command: &str) -> Vec<String> {
    let name = shell_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if name.contains("zsh") || name.contains("bash") {
        return vec![
            "-l".to_owned(),
            "-i".to_owned(),
            "-c".to_owned(),
            command.to_owned(),
        ];
    }

    if name.contains("powershell") || name.contains("pwsh") {
        return vec!["-Command".to_owned(), command.to_owned()];
    }

    if name == "cmd" || name == "cmd.exe" {
        return vec!["/C".to_owned(), command.to_owned()];
    }

    vec!["-c".to_owned(), command.to_owned()]
}

fn parse_absolute_path_tokens(raw: &str) -> Vec<PathBuf> {
    raw.split_whitespace()
        .filter_map(|token| {
            let cleaned = token.trim_matches(|c: char| {
                c == '\'' || c == '"' || c == ',' || c == ';' || c == ':' || c == '`'
            });
            let candidate = PathBuf::from(cleaned);
            if candidate.is_absolute() {
                Some(candidate)
            } else {
                None
            }
        })
        .collect()
}

fn looks_like_codex_executable_name(path: &Path) -> bool {
    let Some(file_name) = path.file_name() else {
        return false;
    };
    let lowered = file_name.to_string_lossy().to_ascii_lowercase();
    lowered == "codex" || lowered == "codex.exe"
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

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

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

    #[test]
    fn parse_absolute_path_tokens_ignores_non_paths() {
        let parsed =
            parse_absolute_path_tokens("codex is /opt/homebrew/bin/codex and alias codex='x'");
        assert_eq!(parsed, vec![PathBuf::from("/opt/homebrew/bin/codex")]);
    }

    #[test]
    fn looks_like_codex_executable_name_only_accepts_codex_binary_names() {
        assert!(looks_like_codex_executable_name(Path::new(
            "/opt/homebrew/bin/codex"
        )));
        assert!(looks_like_codex_executable_name(Path::new(
            "C:/tools/codex.exe"
        )));
        assert!(!looks_like_codex_executable_name(Path::new(
            "/usr/bin/python3"
        )));
        assert!(!looks_like_codex_executable_name(Path::new(
            "/opt/homebrew/bin/codex-beta"
        )));
    }

    #[cfg(unix)]
    #[test]
    fn discover_codex_via_shell_reads_path_output() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "luban-codex-bin-shell-discovery-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp root should be created");

        let codex = root.join("codex");
        std::fs::write(&codex, "#!/bin/sh\nexit 0\n").expect("fake codex should be written");
        let mut codex_perms = std::fs::metadata(&codex)
            .expect("fake codex should exist")
            .permissions();
        codex_perms.set_mode(0o755);
        std::fs::set_permissions(&codex, codex_perms).expect("fake codex should be executable");

        let shell = root.join("fake-shell");
        let shell_script = format!("#!/bin/sh\necho '{}'\n", codex.display());
        std::fs::write(&shell, shell_script).expect("fake shell should be written");
        let mut shell_perms = std::fs::metadata(&shell)
            .expect("fake shell should exist")
            .permissions();
        shell_perms.set_mode(0o755);
        std::fs::set_permissions(&shell, shell_perms).expect("fake shell should be executable");

        let found = discover_codex_via_shell(&shell);
        let expected = std::fs::canonicalize(&codex).expect("fake codex should canonicalize");
        assert_eq!(found, Some(expected));

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn discover_codex_via_shell_ignores_non_codex_executables_in_noise() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "luban-codex-bin-shell-noise-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).expect("temp root should be created");

        let other = root.join("python3");
        std::fs::write(&other, "#!/bin/sh\nexit 0\n").expect("fake python should be written");
        let mut other_perms = std::fs::metadata(&other)
            .expect("fake python should exist")
            .permissions();
        other_perms.set_mode(0o755);
        std::fs::set_permissions(&other, other_perms).expect("fake python should be executable");

        let codex = root.join("codex");
        std::fs::write(&codex, "#!/bin/sh\nexit 0\n").expect("fake codex should be written");
        let mut codex_perms = std::fs::metadata(&codex)
            .expect("fake codex should exist")
            .permissions();
        codex_perms.set_mode(0o755);
        std::fs::set_permissions(&codex, codex_perms).expect("fake codex should be executable");

        let shell = root.join("fake-shell");
        let shell_script = format!(
            "#!/bin/sh\necho '{}'\necho '{}'\n",
            other.display(),
            codex.display()
        );
        std::fs::write(&shell, shell_script).expect("fake shell should be written");
        let mut shell_perms = std::fs::metadata(&shell)
            .expect("fake shell should exist")
            .permissions();
        shell_perms.set_mode(0o755);
        std::fs::set_permissions(&shell, shell_perms).expect("fake shell should be executable");

        let found = discover_codex_via_shell(&shell);
        let expected = std::fs::canonicalize(&codex).expect("fake codex should canonicalize");
        assert_eq!(found, Some(expected));

        let _ = std::fs::remove_dir_all(root);
    }
}
