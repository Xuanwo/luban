use super::GitWorkspaceService;
use anyhow::{Context as _, anyhow};
use std::{ffi::OsStr, path::Path, path::PathBuf, process::Command};

fn push_ascii_lowercase(dst: &mut String, s: &str) {
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            dst.push(ch.to_ascii_lowercase());
        } else {
            dst.push(ch);
        }
    }
}

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

    pub(super) fn repo_root(&self, repo_path: &Path) -> anyhow::Result<PathBuf> {
        let root = self
            .run_git(repo_path, ["rev-parse", "--show-toplevel"])
            .context("failed to resolve git repository root")?;
        if root.is_empty() {
            return Err(anyhow!("git rev-parse returned empty root"));
        }
        Ok(PathBuf::from(root))
    }

    pub(super) fn select_remote_best_effort(
        &self,
        repo_path: &Path,
    ) -> anyhow::Result<Option<String>> {
        let out = self.run_git(repo_path, ["remote"])?;
        let mut first_remote: Option<&str> = None;
        for line in out.lines() {
            let remote = line.trim();
            if remote.is_empty() {
                continue;
            }
            if remote == "origin" {
                return Ok(Some("origin".to_owned()));
            }
            if first_remote.is_none() {
                first_remote = Some(remote);
            }
        }
        Ok(first_remote.map(ToOwned::to_owned))
    }

    pub(super) fn github_repo_id_from_remote_url(url: &str) -> Option<String> {
        let trimmed = url.trim().trim_end_matches('/');
        if trimmed.is_empty() {
            return None;
        }

        let (host, path) = if let Some(rest) = trimmed.strip_prefix("git@") {
            // git@github.com:owner/repo(.git)
            rest.split_once(':')?
        } else if let Some(rest) = trimmed.strip_prefix("ssh://") {
            // ssh://git@github.com/owner/repo(.git)
            let rest = rest.strip_prefix("git@").unwrap_or(rest);
            rest.split_once('/')?
        } else if let Some(rest) = trimmed.strip_prefix("https://") {
            let mut parts = rest.splitn(2, '/');
            let host = parts.next()?;
            let path = parts.next().unwrap_or_default();
            (host, path)
        } else if let Some(rest) = trimmed.strip_prefix("http://") {
            let mut parts = rest.splitn(2, '/');
            let host = parts.next()?;
            let path = parts.next().unwrap_or_default();
            (host, path)
        } else {
            return None;
        };

        if !host.eq_ignore_ascii_case("github.com") {
            return None;
        }

        let path = path.trim_start_matches('/').trim_end_matches(".git");
        let mut iter = path.split('/').filter(|s| !s.is_empty());
        let owner = iter.next()?;
        let repo = iter.next()?;

        let mut out = String::with_capacity("github.com/".len() + owner.len() + 1 + repo.len());
        out.push_str("github.com/");
        push_ascii_lowercase(&mut out, owner);
        out.push('/');
        push_ascii_lowercase(&mut out, repo);
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::GitWorkspaceService;

    #[test]
    fn github_repo_id_from_remote_url_parses_https() {
        assert_eq!(
            GitWorkspaceService::github_repo_id_from_remote_url(
                "https://github.com/apache/opendal"
            ),
            Some("github.com/apache/opendal".to_owned())
        );
        assert_eq!(
            GitWorkspaceService::github_repo_id_from_remote_url(
                "https://github.com/apache/opendal.git"
            ),
            Some("github.com/apache/opendal".to_owned())
        );
    }

    #[test]
    fn github_repo_id_from_remote_url_parses_ssh() {
        assert_eq!(
            GitWorkspaceService::github_repo_id_from_remote_url(
                "git@github.com:apache/opendal.git"
            ),
            Some("github.com/apache/opendal".to_owned())
        );
        assert_eq!(
            GitWorkspaceService::github_repo_id_from_remote_url(
                "ssh://git@github.com/apache/opendal.git"
            ),
            Some("github.com/apache/opendal".to_owned())
        );
    }

    #[test]
    fn github_repo_id_from_remote_url_ignores_non_github() {
        assert_eq!(
            GitWorkspaceService::github_repo_id_from_remote_url(
                "https://gitlab.com/apache/opendal"
            ),
            None
        );
    }

    #[test]
    fn github_repo_id_from_remote_url_lowercases_owner_and_repo() {
        assert_eq!(
            GitWorkspaceService::github_repo_id_from_remote_url(
                "https://github.com/ApAcHe/OpEnDaL/"
            ),
            Some("github.com/apache/opendal".to_owned())
        );
        assert_eq!(
            GitWorkspaceService::github_repo_id_from_remote_url(
                "git@github.com:ApAcHe/OpEnDaL.git"
            ),
            Some("github.com/apache/opendal".to_owned())
        );
    }
}
