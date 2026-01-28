use luban_domain::paths;
use std::path::PathBuf;

use crate::env::{home_dir, optional_trimmed_path_from_env};
use crate::time::unix_epoch_nanos_now;

fn resolve_root_from_env_or_default(
    env_name: &str,
    default: impl FnOnce() -> anyhow::Result<PathBuf>,
) -> anyhow::Result<PathBuf> {
    if let Some(root) = optional_trimmed_path_from_env(env_name)? {
        return Ok(root);
    }

    default()
}

pub(super) fn resolve_luban_root() -> anyhow::Result<PathBuf> {
    resolve_root_from_env_or_default(paths::LUBAN_ROOT_ENV, || {
        if cfg!(test) {
            let nanos = unix_epoch_nanos_now();
            let pid = std::process::id();
            return Ok(std::env::temp_dir().join(format!("luban-test-{pid}-{nanos}")));
        }

        Ok(home_dir()?.join("luban"))
    })
}

pub(super) fn resolve_codex_root() -> anyhow::Result<PathBuf> {
    resolve_root_from_env_or_default(paths::LUBAN_CODEX_ROOT_ENV, || {
        if cfg!(test) {
            return Ok(PathBuf::from(".codex"));
        }

        Ok(home_dir()?.join(".codex"))
    })
}

pub(super) fn resolve_amp_root() -> anyhow::Result<PathBuf> {
    resolve_root_from_env_or_default(paths::LUBAN_AMP_ROOT_ENV, || {
        if cfg!(test) {
            return Ok(PathBuf::from(".amp"));
        }

        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let xdg = xdg.to_string_lossy();
            let trimmed = xdg.trim();
            if !trimmed.is_empty() {
                return Ok(PathBuf::from(trimmed).join("amp"));
            }
        }

        Ok(home_dir()?.join(".config").join("amp"))
    })
}

pub(super) fn resolve_claude_root() -> anyhow::Result<PathBuf> {
    resolve_root_from_env_or_default(paths::LUBAN_CLAUDE_ROOT_ENV, || {
        if cfg!(test) {
            return Ok(PathBuf::from(".claude"));
        }

        Ok(home_dir()?.join(".claude"))
    })
}

#[cfg(test)]
mod tests {
    use super::{resolve_amp_root, resolve_claude_root, resolve_codex_root, resolve_luban_root};
    use luban_domain::paths;
    use std::path::PathBuf;

    fn set_env(name: &str, value: &str) -> Option<std::ffi::OsString> {
        let prev = std::env::var_os(name);
        unsafe {
            std::env::set_var(name, value);
        }
        prev
    }

    fn unset_env(name: &str) -> Option<std::ffi::OsString> {
        let prev = std::env::var_os(name);
        unsafe {
            std::env::remove_var(name);
        }
        prev
    }

    fn restore_env(name: &str, prev: Option<std::ffi::OsString>) {
        if let Some(value) = prev {
            unsafe {
                std::env::set_var(name, value);
            }
        } else {
            unsafe {
                std::env::remove_var(name);
            }
        }
    }

    #[test]
    fn resolve_codex_root_uses_env_override() {
        let _guard = crate::env::lock_env_for_tests();

        let prev = set_env(paths::LUBAN_CODEX_ROOT_ENV, " codex ");
        let loaded = resolve_codex_root().expect("codex root should resolve");
        assert_eq!(loaded, PathBuf::from("codex"));
        restore_env(paths::LUBAN_CODEX_ROOT_ENV, prev);
    }

    #[test]
    fn resolve_codex_root_defaults_in_tests() {
        let _guard = crate::env::lock_env_for_tests();

        let prev = unset_env(paths::LUBAN_CODEX_ROOT_ENV);
        let loaded = resolve_codex_root().expect("codex root should resolve");
        assert_eq!(loaded, PathBuf::from(".codex"));
        restore_env(paths::LUBAN_CODEX_ROOT_ENV, prev);
    }

    #[test]
    fn resolve_amp_root_uses_env_override() {
        let _guard = crate::env::lock_env_for_tests();

        let prev = set_env(paths::LUBAN_AMP_ROOT_ENV, " amp-root ");
        let loaded = resolve_amp_root().expect("amp root should resolve");
        assert_eq!(loaded, PathBuf::from("amp-root"));
        restore_env(paths::LUBAN_AMP_ROOT_ENV, prev);
    }

    #[test]
    fn resolve_amp_root_defaults_in_tests() {
        let _guard = crate::env::lock_env_for_tests();

        let prev = unset_env(paths::LUBAN_AMP_ROOT_ENV);
        let loaded = resolve_amp_root().expect("amp root should resolve");
        assert_eq!(loaded, PathBuf::from(".amp"));
        restore_env(paths::LUBAN_AMP_ROOT_ENV, prev);
    }

    #[test]
    fn resolve_claude_root_uses_env_override() {
        let _guard = crate::env::lock_env_for_tests();

        let prev = set_env(paths::LUBAN_CLAUDE_ROOT_ENV, " claude-root ");
        let loaded = resolve_claude_root().expect("claude root should resolve");
        assert_eq!(loaded, PathBuf::from("claude-root"));
        restore_env(paths::LUBAN_CLAUDE_ROOT_ENV, prev);
    }

    #[test]
    fn resolve_claude_root_defaults_in_tests() {
        let _guard = crate::env::lock_env_for_tests();

        let prev = unset_env(paths::LUBAN_CLAUDE_ROOT_ENV);
        let loaded = resolve_claude_root().expect("claude root should resolve");
        assert_eq!(loaded, PathBuf::from(".claude"));
        restore_env(paths::LUBAN_CLAUDE_ROOT_ENV, prev);
    }

    #[test]
    fn resolve_luban_root_uses_env_override() {
        let _guard = crate::env::lock_env_for_tests();

        let prev = set_env(paths::LUBAN_ROOT_ENV, " luban-root ");
        let loaded = resolve_luban_root().expect("luban root should resolve");
        assert_eq!(loaded, PathBuf::from("luban-root"));
        restore_env(paths::LUBAN_ROOT_ENV, prev);
    }

    #[test]
    fn resolve_luban_root_defaults_in_tests() {
        let _guard = crate::env::lock_env_for_tests();

        let prev = unset_env(paths::LUBAN_ROOT_ENV);
        let loaded = resolve_luban_root().expect("luban root should resolve");

        assert!(loaded.starts_with(std::env::temp_dir()));
        let pid = std::process::id();
        let file_name = loaded
            .file_name()
            .expect("temp root should have a file name")
            .to_string_lossy();
        assert!(
            file_name.starts_with(&format!("luban-test-{pid}-")),
            "unexpected file name: {file_name}"
        );

        restore_env(paths::LUBAN_ROOT_ENV, prev);
    }
}
