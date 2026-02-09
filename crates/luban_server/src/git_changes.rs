use anyhow::{Context as _, anyhow};
use luban_api::{
    ChangedFileSnapshot, DiffFileContents, FileChangeGroup, FileChangeStatus,
    WorkspaceDiffFileSnapshot,
};
use std::{ffi::OsStr, path::Path, process::Command};

fn run_git_bytes<I, S>(repo_path: &Path, args: I) -> anyhow::Result<Vec<u8>>
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

    Ok(output.stdout)
}

fn run_git_text<I, S>(repo_path: &Path, args: I) -> anyhow::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = run_git_bytes(repo_path, args)?;
    Ok(String::from_utf8_lossy(&out).trim().to_owned())
}

fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_owned()
}

fn status_from_code(code: char) -> FileChangeStatus {
    match code {
        'A' => FileChangeStatus::Added,
        'D' => FileChangeStatus::Deleted,
        'R' | 'C' => FileChangeStatus::Renamed,
        _ => FileChangeStatus::Modified,
    }
}

fn is_runtime_internal_path(path: &str) -> bool {
    let normalized = path.trim_start_matches("./");
    normalized == ".luban" || normalized.starts_with(".luban/")
}

fn upstream_ref(repo_path: &Path) -> Option<String> {
    run_git_text(
        repo_path,
        ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )
    .ok()
}

fn file_id(group: FileChangeGroup, path: &str) -> String {
    let prefix = match group {
        FileChangeGroup::Committed => "committed",
        FileChangeGroup::Staged => "staged",
        FileChangeGroup::Unstaged => "unstaged",
    };
    format!("{prefix}:{path}")
}

fn parse_name_status_line(
    group: FileChangeGroup,
    line: &str,
    upstream: Option<&str>,
) -> Option<ChangedFileSnapshot> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts: Vec<&str> = trimmed.split('\t').collect();
    let first = parts.first()?.trim();
    let code = first.chars().next()?;
    let status = status_from_code(code);

    let (old_path, path) = match status {
        FileChangeStatus::Renamed => {
            if parts.len() < 3 {
                return None;
            }
            (Some(parts[1].to_owned()), parts[2].to_owned())
        }
        _ => {
            if parts.len() < 2 {
                return None;
            }
            (None, parts[1].to_owned())
        }
    };

    if is_runtime_internal_path(&path)
        || old_path
            .as_deref()
            .map(is_runtime_internal_path)
            .unwrap_or(false)
    {
        return None;
    }

    if group == FileChangeGroup::Committed && upstream.is_none() {
        return None;
    }

    Some(ChangedFileSnapshot {
        id: file_id(group, &path),
        name: basename(&path),
        path,
        status,
        group,
        additions: None,
        deletions: None,
        old_path,
    })
}

fn parse_status_porcelain_v2(repo_path: &Path) -> anyhow::Result<Vec<ChangedFileSnapshot>> {
    let out = run_git_bytes(repo_path, ["status", "--porcelain=v2", "-z"])?;
    let text = String::from_utf8_lossy(&out);
    let mut files: Vec<ChangedFileSnapshot> = Vec::new();

    for record in text.split('\0') {
        let record = record.trim_end_matches('\n');
        if record.trim().is_empty() {
            continue;
        }
        if record.starts_with('#') {
            continue;
        }

        if let Some(rest) = record.strip_prefix("? ") {
            let path = rest.to_owned();
            if is_runtime_internal_path(&path) {
                continue;
            }
            files.push(ChangedFileSnapshot {
                id: file_id(FileChangeGroup::Unstaged, &path),
                name: basename(&path),
                path,
                status: FileChangeStatus::Added,
                group: FileChangeGroup::Unstaged,
                additions: None,
                deletions: None,
                old_path: None,
            });
            continue;
        }

        if let Some(rest) = record.strip_prefix("1 ") {
            let mut parts = rest.splitn(9, ' ');
            let xy = parts.next().unwrap_or("");
            let _sub = parts.next().unwrap_or("");
            let _m1 = parts.next().unwrap_or("");
            let _m2 = parts.next().unwrap_or("");
            let _m3 = parts.next().unwrap_or("");
            let _mh = parts.next().unwrap_or("");
            let _mi = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("").to_owned();

            let mut xy_chars = xy.chars();
            let x = xy_chars.next().unwrap_or('.');
            let y = xy_chars.next().unwrap_or('.');
            let (group, code) = if y != '.' {
                (FileChangeGroup::Unstaged, y)
            } else if x != '.' {
                (FileChangeGroup::Staged, x)
            } else {
                continue;
            };
            if is_runtime_internal_path(&path) {
                continue;
            }

            files.push(ChangedFileSnapshot {
                id: file_id(group, &path),
                name: basename(&path),
                path,
                status: status_from_code(code),
                group,
                additions: None,
                deletions: None,
                old_path: None,
            });
            continue;
        }

        if let Some(rest) = record.strip_prefix("2 ") {
            let mut parts = rest.splitn(10, ' ');
            let xy = parts.next().unwrap_or("");
            let _sub = parts.next().unwrap_or("");
            let _m1 = parts.next().unwrap_or("");
            let _m2 = parts.next().unwrap_or("");
            let _m3 = parts.next().unwrap_or("");
            let _mh = parts.next().unwrap_or("");
            let _mi = parts.next().unwrap_or("");
            let _score = parts.next().unwrap_or("");
            let paths = parts.next().unwrap_or("");

            let mut xy_chars = xy.chars();
            let x = xy_chars.next().unwrap_or('.');
            let y = xy_chars.next().unwrap_or('.');
            let group = if y != '.' {
                FileChangeGroup::Unstaged
            } else if x != '.' {
                FileChangeGroup::Staged
            } else {
                continue;
            };

            let (old_path, path) = paths
                .split_once('\t')
                .map(|(a, b)| (a.to_owned(), b.to_owned()))
                .unwrap_or_else(|| (paths.to_owned(), paths.to_owned()));
            if is_runtime_internal_path(&path) || is_runtime_internal_path(&old_path) {
                continue;
            }

            files.push(ChangedFileSnapshot {
                id: file_id(group, &path),
                name: basename(&path),
                path,
                status: FileChangeStatus::Renamed,
                group,
                additions: None,
                deletions: None,
                old_path: Some(old_path),
            });
            continue;
        }
    }

    Ok(files)
}

fn compute_numstat(
    repo_path: &Path,
    file: &ChangedFileSnapshot,
    upstream: Option<&str>,
) -> (Option<u64>, Option<u64>) {
    let mut args: Vec<String> = Vec::new();
    args.push("diff".to_owned());
    match file.group {
        FileChangeGroup::Committed => {
            let Some(upstream) = upstream else {
                return (None, None);
            };
            args.push(format!("{upstream}..HEAD"));
        }
        FileChangeGroup::Staged => {
            args.push("--cached".to_owned());
        }
        FileChangeGroup::Unstaged => {}
    }
    args.push("--numstat".to_owned());
    args.push("--".to_owned());
    args.push(file.path.clone());

    let out = run_git_text(repo_path, args.iter().map(|s| s.as_str())).unwrap_or_default();

    if out.trim().is_empty() {
        if file.group == FileChangeGroup::Unstaged && file.status == FileChangeStatus::Added {
            let path = repo_path.join(&file.path);
            if let Ok(bytes) = std::fs::read(&path) {
                let text = String::from_utf8_lossy(&bytes);
                let lines = if text.is_empty() {
                    0
                } else {
                    text.lines().count() as u64
                };
                return (Some(lines), Some(0));
            }
        }
        return (None, None);
    }

    let first = out.lines().next().unwrap_or("");
    let parts: Vec<&str> = first.split('\t').collect();
    if parts.len() < 2 {
        return (None, None);
    }
    let add = parts[0].parse::<u64>().ok();
    let del = parts[1].parse::<u64>().ok();
    (add, del)
}

fn git_show_utf8(repo_path: &Path, spec: &str) -> String {
    let out = Command::new("git")
        .args(["show", spec])
        .current_dir(repo_path)
        .output();
    match out {
        Ok(out) if out.status.success() => {
            String::from_utf8(out.stdout).unwrap_or_else(|_| "<binary file>".to_owned())
        }
        _ => String::new(),
    }
}

fn git_show_index_utf8(repo_path: &Path, path: &str) -> String {
    git_show_utf8(repo_path, &format!(":{path}"))
}

fn git_show_head_utf8(repo_path: &Path, path: &str) -> String {
    git_show_utf8(repo_path, &format!("HEAD:{path}"))
}

fn git_show_commit_utf8(repo_path: &Path, commit: &str, path: &str) -> String {
    git_show_utf8(repo_path, &format!("{commit}:{path}"))
}

fn read_worktree_utf8(repo_path: &Path, path: &str) -> String {
    let full = repo_path.join(path);
    match std::fs::read(&full) {
        Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| "<binary file>".to_owned()),
        Err(_) => String::new(),
    }
}

fn diff_contents_for_file(
    repo_path: &Path,
    file: &ChangedFileSnapshot,
    upstream: Option<&str>,
) -> (String, String) {
    let path = file.path.as_str();
    let old_path = file.old_path.as_deref().unwrap_or(path);

    match file.group {
        FileChangeGroup::Committed => {
            let Some(upstream) = upstream else {
                return (String::new(), String::new());
            };
            let old = match file.status {
                FileChangeStatus::Added => String::new(),
                _ => git_show_commit_utf8(repo_path, upstream, old_path),
            };
            let new = match file.status {
                FileChangeStatus::Deleted => String::new(),
                _ => git_show_head_utf8(repo_path, path),
            };
            (old, new)
        }
        FileChangeGroup::Staged => {
            let old = match file.status {
                FileChangeStatus::Added => String::new(),
                _ => git_show_head_utf8(repo_path, old_path),
            };
            let new = match file.status {
                FileChangeStatus::Deleted => String::new(),
                _ => git_show_index_utf8(repo_path, path),
            };
            (old, new)
        }
        FileChangeGroup::Unstaged => {
            let old = match file.status {
                FileChangeStatus::Added => String::new(),
                _ => {
                    let index = git_show_index_utf8(repo_path, old_path);
                    if index.is_empty() {
                        git_show_head_utf8(repo_path, old_path)
                    } else {
                        index
                    }
                }
            };
            let new = match file.status {
                FileChangeStatus::Deleted => String::new(),
                _ => read_worktree_utf8(repo_path, path),
            };
            (old, new)
        }
    }
}

pub fn collect_changes(repo_path: &Path) -> anyhow::Result<Vec<ChangedFileSnapshot>> {
    let upstream = upstream_ref(repo_path);
    let mut staged_unstaged = parse_status_porcelain_v2(repo_path)?;

    let mut present_paths = std::collections::HashSet::new();
    for f in &staged_unstaged {
        present_paths.insert(f.path.clone());
    }

    if let Some(upstream) = upstream.as_deref() {
        let out = run_git_text(
            repo_path,
            [
                "diff",
                "--name-status",
                "--find-renames",
                &format!("{upstream}..HEAD"),
            ],
        )
        .unwrap_or_default();
        let mut committed: Vec<ChangedFileSnapshot> = Vec::new();
        for line in out.lines() {
            if let Some(file) =
                parse_name_status_line(FileChangeGroup::Committed, line, Some(upstream))
            {
                if present_paths.contains(&file.path) {
                    continue;
                }
                committed.push(file);
            }
        }
        staged_unstaged.splice(0..0, committed);
    }

    let upstream = upstream.as_deref();
    for file in &mut staged_unstaged {
        let (add, del) = compute_numstat(repo_path, file, upstream);
        file.additions = add;
        file.deletions = del;
    }

    Ok(staged_unstaged)
}

pub fn collect_diff(repo_path: &Path) -> anyhow::Result<Vec<WorkspaceDiffFileSnapshot>> {
    let upstream = upstream_ref(repo_path);
    let mut files = collect_changes(repo_path)?;

    // Ensure deterministic ordering: group then path.
    files.sort_by(|a, b| {
        fn rank(group: FileChangeGroup) -> u8 {
            match group {
                FileChangeGroup::Committed => 0,
                FileChangeGroup::Staged => 1,
                FileChangeGroup::Unstaged => 2,
            }
        }

        rank(a.group)
            .cmp(&rank(b.group))
            .then_with(|| a.path.cmp(&b.path))
    });

    let mut out = Vec::with_capacity(files.len());
    for file in files {
        let (old_contents, new_contents) =
            diff_contents_for_file(repo_path, &file, upstream.as_deref());
        out.push(WorkspaceDiffFileSnapshot {
            old_file: DiffFileContents {
                name: file.name.clone(),
                contents: old_contents,
            },
            new_file: DiffFileContents {
                name: file.name.clone(),
                contents: new_contents,
            },
            file,
        });
    }
    Ok(out)
}
