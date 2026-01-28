use anyhow::{Context as _, anyhow};
use std::{path::Path, process::Command};

pub fn check_cli_version(binary: &Path, tool_name: &'static str) -> anyhow::Result<()> {
    let output = Command::new(binary)
        .args(["--version"])
        .output()
        .with_context(|| format!("failed to spawn {}", binary.display()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !stderr.is_empty() {
        return Err(anyhow!("{stderr}"));
    }
    if !stdout.is_empty() {
        return Err(anyhow!("{stdout}"));
    }

    Err(anyhow!("{tool_name} exited with status {}", output.status))
}
