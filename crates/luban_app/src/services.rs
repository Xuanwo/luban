use anyhow::{Context as _, anyhow};
use bip39::Language;
use rand::{Rng as _, rngs::OsRng};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use luban_ui::{CreatedWorkspace, ProjectWorkspaceService};

#[derive(Clone)]
pub struct GitWorkspaceService {
    worktrees_root: PathBuf,
}

impl GitWorkspaceService {
    pub fn new() -> anyhow::Result<Arc<Self>> {
        let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
        let mut worktrees_root = PathBuf::from(home);
        worktrees_root.push("luban");
        worktrees_root.push("worktrees");

        Ok(Arc::new(Self { worktrees_root }))
    }

    fn run_git<I, S>(&self, repo_path: &Path, args: I) -> anyhow::Result<String>
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

    fn select_remote(&self, repo_path: &Path) -> anyhow::Result<String> {
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

    fn resolve_default_upstream_ref(
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

    fn generate_workspace_name(&self) -> anyhow::Result<String> {
        let words = Language::English.word_list();
        let mut rng = OsRng;
        let w1 = words[rng.gen_range(0..words.len())];
        let w2 = words[rng.gen_range(0..words.len())];
        Ok(format!("{w1}-{w2}"))
    }

    fn worktree_path(&self, project_slug: &str, workspace_name: &str) -> PathBuf {
        let mut path = self.worktrees_root.clone();
        path.push(project_slug);
        path.push(workspace_name);
        path
    }
}

impl ProjectWorkspaceService for GitWorkspaceService {
    fn create_workspace(
        &self,
        project_path: PathBuf,
        project_slug: String,
    ) -> Result<CreatedWorkspace, String> {
        let result: anyhow::Result<CreatedWorkspace> = (|| {
            let remote = self.select_remote(&project_path)?;

            self.run_git(&project_path, ["fetch", "--prune", &remote])
                .with_context(|| format!("failed to fetch remote '{remote}'"))?;

            let upstream_ref = self.resolve_default_upstream_ref(&project_path, &remote)?;

            std::fs::create_dir_all(self.worktrees_root.join(&project_slug))
                .context("failed to create worktrees root")?;

            for _ in 0..64 {
                let workspace_name = self.generate_workspace_name()?;
                let branch_name = format!("luban/{workspace_name}");
                let worktree_path = self.worktree_path(&project_slug, &workspace_name);

                if worktree_path.exists() {
                    continue;
                }

                let branch_ref = format!("refs/heads/{branch_name}");
                let branch_exists = Command::new("git")
                    .args(["show-ref", "--verify", "--quiet", &branch_ref])
                    .current_dir(&project_path)
                    .status()
                    .ok()
                    .map(|s| s.success())
                    .unwrap_or(false);

                if branch_exists {
                    continue;
                }

                self.run_git(
                    &project_path,
                    ["branch", "--track", &branch_name, &upstream_ref],
                )
                .with_context(|| format!("failed to create branch '{branch_name}'"))?;

                self.run_git(
                    &project_path,
                    [
                        "worktree",
                        "add",
                        worktree_path
                            .to_str()
                            .ok_or_else(|| anyhow!("invalid worktree path"))?,
                        &branch_name,
                    ],
                )
                .with_context(|| {
                    format!("failed to create worktree at {}", worktree_path.display())
                })?;

                return Ok(CreatedWorkspace {
                    workspace_name,
                    branch_name,
                    worktree_path,
                });
            }

            Err(anyhow!(
                "failed to generate a unique workspace name after retries"
            ))
        })();

        result.map_err(|e| format!("{e:#}"))
    }

    fn archive_workspace(
        &self,
        project_path: PathBuf,
        worktree_path: PathBuf,
    ) -> Result<(), String> {
        let result: anyhow::Result<()> = (|| {
            let path_str = worktree_path
                .to_str()
                .ok_or_else(|| anyhow!("invalid worktree path"))?;
            self.run_git(&project_path, ["worktree", "remove", path_str])
                .with_context(|| {
                    format!("failed to remove worktree at {}", worktree_path.display())
                })?;
            Ok(())
        })();
        result.map_err(|e| format!("{e:#}"))
    }
}
