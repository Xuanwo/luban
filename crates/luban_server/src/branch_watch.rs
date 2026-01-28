use crate::engine::EngineCommand;
use luban_domain::WorkspaceId;
use notify::{Event, RecursiveMode, Watcher as _};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

#[derive(Debug)]
pub(crate) struct BranchWatchHandle {
    tx: mpsc::Sender<BranchWatchMessage>,
    join: Option<thread::JoinHandle<()>>,
}

#[derive(Debug)]
enum BranchWatchMessage {
    Command(BranchWatchCommand),
    Event(notify::Result<Event>),
}

#[derive(Debug)]
enum BranchWatchCommand {
    SyncWorkspaces {
        workspaces: Vec<(WorkspaceId, PathBuf)>,
    },
    Shutdown,
}

#[derive(Debug)]
struct WatchedWorkspace {
    head_path: PathBuf,
    last_branch_name: Option<String>,
}

impl BranchWatchHandle {
    pub(crate) fn start(engine_tx: tokio::sync::mpsc::Sender<EngineCommand>) -> Self {
        let (tx, rx) = mpsc::channel::<BranchWatchMessage>();

        let callback_tx = tx.clone();
        let join = thread::spawn(move || {
            let mut watcher = match notify::recommended_watcher(move |res| {
                let _ = callback_tx.send(BranchWatchMessage::Event(res));
            }) {
                Ok(w) => w,
                Err(err) => {
                    tracing::error!(error = %err, "failed to initialize branch watcher");
                    return;
                }
            };

            let mut watched = HashMap::<WorkspaceId, WatchedWorkspace>::new();
            let mut path_to_workspace = HashMap::<PathBuf, WorkspaceId>::new();

            while let Ok(msg) = rx.recv() {
                match msg {
                    BranchWatchMessage::Command(cmd) => match cmd {
                        BranchWatchCommand::SyncWorkspaces { workspaces } => {
                            sync_workspaces(
                                &mut watcher,
                                &mut watched,
                                &mut path_to_workspace,
                                workspaces,
                            );
                        }
                        BranchWatchCommand::Shutdown => break,
                    },
                    BranchWatchMessage::Event(res) => {
                        let event = match res {
                            Ok(event) => event,
                            Err(err) => {
                                tracing::debug!(error = %err, "branch watcher event error");
                                continue;
                            }
                        };

                        for path in &event.paths {
                            let Some(workspace_id) = path_to_workspace.get(path).copied() else {
                                continue;
                            };
                            let Some(entry) = watched.get_mut(&workspace_id) else {
                                continue;
                            };

                            let Some(branch_name) = read_branch_name_from_head(&entry.head_path)
                            else {
                                continue;
                            };
                            if entry.last_branch_name.as_deref() == Some(branch_name.as_str()) {
                                continue;
                            }
                            entry.last_branch_name = Some(branch_name.clone());
                            let _ = engine_tx.try_send(EngineCommand::WorkspaceBranchObserved {
                                workspace_id,
                                branch_name,
                            });
                        }
                    }
                }
            }
        });

        Self {
            tx,
            join: Some(join),
        }
    }

    #[cfg(test)]
    pub(crate) fn disabled() -> Self {
        let (tx, _rx) = mpsc::channel::<BranchWatchMessage>();
        Self { tx, join: None }
    }

    pub(crate) fn sync_workspaces(&self, workspaces: Vec<(WorkspaceId, PathBuf)>) {
        let _ = self.tx.send(BranchWatchMessage::Command(
            BranchWatchCommand::SyncWorkspaces { workspaces },
        ));
    }
}

impl Drop for BranchWatchHandle {
    fn drop(&mut self) {
        let _ = self
            .tx
            .send(BranchWatchMessage::Command(BranchWatchCommand::Shutdown));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn sync_workspaces(
    watcher: &mut notify::RecommendedWatcher,
    watched: &mut HashMap<WorkspaceId, WatchedWorkspace>,
    path_to_workspace: &mut HashMap<PathBuf, WorkspaceId>,
    workspaces: Vec<(WorkspaceId, PathBuf)>,
) {
    let desired_ids = workspaces.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    let desired_set = desired_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();

    let existing_ids = watched.keys().copied().collect::<Vec<_>>();
    for workspace_id in existing_ids {
        if desired_set.contains(&workspace_id) {
            continue;
        }
        let Some(entry) = watched.remove(&workspace_id) else {
            continue;
        };
        let _ = watcher.unwatch(&entry.head_path);
        path_to_workspace.remove(&entry.head_path);
    }

    for (workspace_id, worktree_path) in workspaces {
        let Some(head_path) = resolve_head_path(&worktree_path) else {
            continue;
        };

        if let Some(existing) = watched.get(&workspace_id)
            && existing.head_path == head_path
        {
            continue;
        }

        if let Some(existing) = watched.remove(&workspace_id) {
            let _ = watcher.unwatch(&existing.head_path);
            path_to_workspace.remove(&existing.head_path);
        }

        if watcher
            .watch(&head_path, RecursiveMode::NonRecursive)
            .is_err()
        {
            continue;
        }
        path_to_workspace.insert(head_path.clone(), workspace_id);
        watched.insert(
            workspace_id,
            WatchedWorkspace {
                head_path,
                last_branch_name: None,
            },
        );
    }
}

fn resolve_head_path(worktree_path: &Path) -> Option<PathBuf> {
    let dot_git = worktree_path.join(".git");
    if dot_git.is_dir() {
        let head = dot_git.join("HEAD");
        return Some(std::fs::canonicalize(&head).unwrap_or(head));
    }

    let git_file = std::fs::read_to_string(&dot_git).ok()?;
    let git_file = git_file.trim();
    let rest = git_file.strip_prefix("gitdir:")?.trim();
    if rest.is_empty() {
        return None;
    }

    let gitdir = PathBuf::from(rest);
    let gitdir = if gitdir.is_absolute() {
        gitdir
    } else {
        worktree_path.join(gitdir)
    };
    let head = gitdir.join("HEAD");
    Some(std::fs::canonicalize(&head).unwrap_or(head))
}

fn read_branch_name_from_head(head_path: &Path) -> Option<String> {
    let head = std::fs::read_to_string(head_path).ok()?;
    Some(branch_name_from_head_contents(&head))
}

fn branch_name_from_head_contents(contents: &str) -> String {
    let trimmed = contents.trim();
    let Some(rest) = trimmed.strip_prefix("ref:") else {
        return "HEAD".to_owned();
    };

    let reference = rest.trim();
    if let Some(stripped) = reference.strip_prefix("refs/heads/") {
        return stripped.to_owned();
    }
    if let Some(stripped) = reference.strip_prefix("refs/") {
        return stripped.to_owned();
    }

    reference.to_owned()
}
