use super::GitWorkspaceService;
use anyhow::{Context as _, anyhow};
use std::{ffi::OsStr, path::Path, process::Command};

impl GitWorkspaceService {
    pub(super) fn run_git<I, S>(&self, repo_path: &Path, args: I) -> anyhow::Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .context("failed to spawn git")?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "git failed ({}):\nstdout:\n{}\nstderr:\n{}",
                output.status,
                stdout.trim(),
                stderr.trim()
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    pub(super) fn select_remote(&self, repo_path: &Path) -> anyhow::Result<String> {
        let out = self.run_git(repo_path, ["remote"])?;
        let remotes = out
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        if remotes.contains(&"origin") {
            return Ok("origin".to_owned());
        }

        if remotes.len() == 1 {
            return Ok(remotes[0].to_owned());
        }

        Err(anyhow!("cannot select remote: found {:?}", remotes))
    }

    pub(super) fn resolve_default_upstream_ref(
        &self,
        repo_path: &Path,
        remote: &str,
    ) -> anyhow::Result<String> {
        let head_ref = self
            .run_git(
                repo_path,
                [
                    "symbolic-ref",
                    "--quiet",
                    &format!("refs/remotes/{remote}/HEAD"),
                ],
            )
            .context("failed to resolve remote HEAD ref (missing refs/remotes/<remote>/HEAD?)")?;

        let prefix = format!("refs/remotes/{remote}/");
        let Some(branch) = head_ref.strip_prefix(&prefix) else {
            return Err(anyhow!("unexpected remote HEAD ref: {head_ref}"));
        };

        let verify_ref = format!("refs/remotes/{remote}/{branch}");
        self.run_git(repo_path, ["show-ref", "--verify", "--quiet", &verify_ref])
            .with_context(|| format!("remote default branch ref not found: {verify_ref}"))?;

        Ok(format!("{remote}/{branch}"))
    }
}
