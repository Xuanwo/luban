use luban_domain::paths;
use std::path::PathBuf;

use crate::env::{home_dir, optional_trimmed_path_from_env};
use crate::time::unix_epoch_nanos_now;

pub(super) fn resolve_luban_root() -> anyhow::Result<PathBuf> {
    if let Some(root) = optional_trimmed_path_from_env(paths::LUBAN_ROOT_ENV)? {
        return Ok(root);
    }

    if cfg!(test) {
        let nanos = unix_epoch_nanos_now();
        let pid = std::process::id();
        return Ok(std::env::temp_dir().join(format!("luban-test-{pid}-{nanos}")));
    }

    Ok(home_dir()?.join("luban"))
}

pub(super) fn resolve_codex_root() -> anyhow::Result<PathBuf> {
    if let Some(root) = optional_trimmed_path_from_env(paths::LUBAN_CODEX_ROOT_ENV)? {
        return Ok(root);
    }

    if cfg!(test) {
        return Ok(PathBuf::from(".codex"));
    }

    Ok(home_dir()?.join(".codex"))
}

pub(super) fn resolve_amp_root() -> anyhow::Result<PathBuf> {
    if let Some(root) = optional_trimmed_path_from_env(paths::LUBAN_AMP_ROOT_ENV)? {
        return Ok(root);
    }

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
}

pub(super) fn resolve_claude_root() -> anyhow::Result<PathBuf> {
    if let Some(root) = optional_trimmed_path_from_env(paths::LUBAN_CLAUDE_ROOT_ENV)? {
        return Ok(root);
    }

    if cfg!(test) {
        return Ok(PathBuf::from(".claude"));
    }

    Ok(home_dir()?.join(".claude"))
}
