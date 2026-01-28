use anyhow::{Context as _, anyhow};
use serde::Deserialize;
use std::process::Command;

pub(super) fn ensure_gh_cli() -> anyhow::Result<()> {
    let status = Command::new("gh")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                anyhow!("missing gh executable: install GitHub CLI (gh) and ensure it is available on PATH")
            } else {
                anyhow!(err).context("failed to spawn gh")
            }
        })?;
    if !status.success() {
        return Err(anyhow!("gh --version failed with status: {status}"));
    }
    Ok(())
}

pub(super) fn run_gh_json<T: for<'de> Deserialize<'de>>(args: &[&str]) -> anyhow::Result<T> {
    let out = Command::new("gh").args(args).output().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            anyhow!(
                "missing gh executable: install GitHub CLI (gh) and ensure it is available on PATH"
            )
        } else {
            anyhow!(err).context("failed to spawn gh")
        }
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("gh failed ({}): {}", out.status, stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed = serde_json::from_str::<T>(stdout.trim())
        .with_context(|| format!("failed to parse gh json for args: {}", args.join(" ")))?;
    Ok(parsed)
}
