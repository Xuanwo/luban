use crate::engine::EngineCommand;
use luban_domain::{TaskDocumentKind, WorkspaceId, WorkspaceThreadId};
use notify::{Event, RecursiveMode, Watcher as _};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub(crate) struct TaskDocumentWatchHandle {
    tx: mpsc::Sender<TaskDocumentWatchMessage>,
    join: Option<thread::JoinHandle<()>>,
}

#[derive(Debug)]
enum TaskDocumentWatchMessage {
    Command(TaskDocumentWatchCommand),
    Event(notify::Result<Event>),
}

#[derive(Debug)]
enum TaskDocumentWatchCommand {
    SyncWorkspaces {
        workspaces: Vec<(WorkspaceId, PathBuf)>,
    },
    Shutdown,
}

#[derive(Debug)]
struct WatchedWorkspace {
    root_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CachedDocument {
    content_hash: String,
    byte_len: u64,
    updated_at_unix_ms: u64,
}

impl TaskDocumentWatchHandle {
    pub(crate) fn start(engine_tx: tokio::sync::mpsc::Sender<EngineCommand>) -> Self {
        let (tx, rx) = mpsc::channel::<TaskDocumentWatchMessage>();
        let callback_tx = tx.clone();
        let join = thread::spawn(move || {
            let mut watcher = match notify::recommended_watcher(move |res| {
                let _ = callback_tx.send(TaskDocumentWatchMessage::Event(res));
            }) {
                Ok(w) => w,
                Err(err) => {
                    tracing::error!(error = %err, "failed to initialize task document watcher");
                    return;
                }
            };

            let mut watched = HashMap::<WorkspaceId, WatchedWorkspace>::new();
            let mut cache =
                HashMap::<(WorkspaceId, WorkspaceThreadId, TaskDocumentKind), CachedDocument>::new(
                );

            while let Ok(msg) = rx.recv() {
                match msg {
                    TaskDocumentWatchMessage::Command(cmd) => match cmd {
                        TaskDocumentWatchCommand::SyncWorkspaces { workspaces } => {
                            sync_workspaces(&mut watcher, &mut watched, &mut cache, workspaces);
                        }
                        TaskDocumentWatchCommand::Shutdown => break,
                    },
                    TaskDocumentWatchMessage::Event(res) => {
                        let event = match res {
                            Ok(event) => event,
                            Err(err) => {
                                tracing::debug!(error = %err, "task document watcher event error");
                                continue;
                            }
                        };
                        let changed = changed_documents_from_event(&watched, &mut cache, &event);
                        for (workspace_id, thread_id, kind) in changed {
                            let _ = engine_tx.try_send(EngineCommand::TaskDocumentObserved {
                                workspace_id,
                                thread_id,
                                kind,
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
        let (tx, _rx) = mpsc::channel::<TaskDocumentWatchMessage>();
        Self { tx, join: None }
    }

    pub(crate) fn sync_workspaces(&self, workspaces: Vec<(WorkspaceId, PathBuf)>) {
        let _ = self.tx.send(TaskDocumentWatchMessage::Command(
            TaskDocumentWatchCommand::SyncWorkspaces { workspaces },
        ));
    }
}

impl Drop for TaskDocumentWatchHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(TaskDocumentWatchMessage::Command(
            TaskDocumentWatchCommand::Shutdown,
        ));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn sync_workspaces(
    watcher: &mut notify::RecommendedWatcher,
    watched: &mut HashMap<WorkspaceId, WatchedWorkspace>,
    cache: &mut HashMap<(WorkspaceId, WorkspaceThreadId, TaskDocumentKind), CachedDocument>,
    workspaces: Vec<(WorkspaceId, PathBuf)>,
) {
    let desired_set: HashSet<WorkspaceId> = workspaces.iter().map(|(id, _)| *id).collect();

    let existing_ids = watched.keys().copied().collect::<Vec<_>>();
    for workspace_id in existing_ids {
        if desired_set.contains(&workspace_id) {
            continue;
        }
        if let Some(entry) = watched.remove(&workspace_id) {
            let _ = watcher.unwatch(&entry.root_path);
        }
        cache.retain(|(wid, _, _), _| *wid != workspace_id);
    }

    for (workspace_id, worktree_path) in workspaces {
        let root = worktree_path.join(".luban").join("tasks");
        if std::fs::create_dir_all(&root).is_err() {
            continue;
        }
        let root = std::fs::canonicalize(&root).unwrap_or(root);

        if let Some(existing) = watched.get(&workspace_id)
            && existing.root_path == root
        {
            continue;
        }

        if let Some(existing) = watched.remove(&workspace_id) {
            let _ = watcher.unwatch(&existing.root_path);
        }
        cache.retain(|(wid, _, _), _| *wid != workspace_id);

        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            continue;
        }
        watched.insert(
            workspace_id,
            WatchedWorkspace {
                root_path: root.clone(),
            },
        );

        seed_workspace_cache(workspace_id, &root, cache);
    }
}

fn seed_workspace_cache(
    workspace_id: WorkspaceId,
    root: &Path,
    cache: &mut HashMap<(WorkspaceId, WorkspaceThreadId, TaskDocumentKind), CachedDocument>,
) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(thread_name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        let Ok(thread_id_raw) = thread_name.parse::<u64>() else {
            continue;
        };
        let thread_id = WorkspaceThreadId::from_u64(thread_id_raw);
        for kind in [
            TaskDocumentKind::Task,
            TaskDocumentKind::Plan,
            TaskDocumentKind::Memory,
        ] {
            let file_path = path.join(kind.file_name());
            let Some(next) = read_document_state(&file_path) else {
                continue;
            };
            cache.insert((workspace_id, thread_id, kind), next);
        }
    }
}

fn changed_documents_from_event(
    watched: &HashMap<WorkspaceId, WatchedWorkspace>,
    cache: &mut HashMap<(WorkspaceId, WorkspaceThreadId, TaskDocumentKind), CachedDocument>,
    event: &Event,
) -> Vec<(WorkspaceId, WorkspaceThreadId, TaskDocumentKind)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for path in &event.paths {
        let Some((workspace_id, thread_id, kind, file_path)) = resolve_document_path(watched, path)
        else {
            continue;
        };

        let key = (workspace_id, thread_id, kind);
        let prev = cache.get(&key).cloned();
        let next = read_document_state(&file_path);

        let changed = match (&prev, &next) {
            (None, None) => false,
            (Some(_), None) => true,
            (None, Some(_)) => true,
            (Some(prev), Some(next)) => prev != next,
        };
        if !changed {
            continue;
        }

        match next {
            Some(next) => {
                cache.insert(key, next);
            }
            None => {
                cache.remove(&key);
            }
        }

        if seen.insert(key) {
            out.push((workspace_id, thread_id, kind));
        }
    }
    out
}

fn resolve_document_path(
    watched: &HashMap<WorkspaceId, WatchedWorkspace>,
    raw_path: &Path,
) -> Option<(WorkspaceId, WorkspaceThreadId, TaskDocumentKind, PathBuf)> {
    let canonical = std::fs::canonicalize(raw_path).ok();
    for (workspace_id, watched_workspace) in watched {
        let relative = if let Ok(relative) = raw_path.strip_prefix(&watched_workspace.root_path) {
            relative.to_path_buf()
        } else if let Some(canonical) = &canonical {
            if let Ok(relative) = canonical.strip_prefix(&watched_workspace.root_path) {
                relative.to_path_buf()
            } else {
                continue;
            }
        } else {
            continue;
        };
        let mut components = relative.components();
        let Some(thread_component) = components.next() else {
            continue;
        };
        let Some(file_component) = components.next() else {
            continue;
        };
        if components.next().is_some() {
            continue;
        }

        let thread_name = thread_component.as_os_str().to_str()?;
        let thread_id_raw = thread_name.parse::<u64>().ok()?;
        let file_name = file_component.as_os_str().to_str()?;
        let kind = if file_name.eq_ignore_ascii_case("TASK.md") {
            TaskDocumentKind::Task
        } else if file_name.eq_ignore_ascii_case("PLAN.md") {
            TaskDocumentKind::Plan
        } else if file_name.eq_ignore_ascii_case("MEMORY.md") {
            TaskDocumentKind::Memory
        } else {
            continue;
        };
        let thread_id = WorkspaceThreadId::from_u64(thread_id_raw);
        let file_path = watched_workspace
            .root_path
            .join(thread_id_raw.to_string())
            .join(kind.file_name());
        return Some((*workspace_id, thread_id, kind, file_path));
    }
    None
}

fn read_document_state(path: &Path) -> Option<CachedDocument> {
    let bytes = std::fs::read(path).ok()?;
    let byte_len = bytes.len() as u64;
    let content_hash = blake3::hash(&bytes).to_hex().to_string();
    let updated_at_unix_ms = std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_else(now_unix_millis);
    Some(CachedDocument {
        content_hash,
        byte_len,
        updated_at_unix_ms,
    })
}

fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_document_path_ignores_unrelated_files() {
        let mut watched = HashMap::new();
        watched.insert(
            WorkspaceId::from_u64(7),
            WatchedWorkspace {
                root_path: PathBuf::from("/tmp/work/.luban/tasks"),
            },
        );

        assert!(
            resolve_document_path(&watched, Path::new("/tmp/work/.luban/tasks/1/other.md"))
                .is_none()
        );
        assert!(
            resolve_document_path(&watched, Path::new("/tmp/work/.luban/tasks/readme.md"))
                .is_none()
        );
    }
}
