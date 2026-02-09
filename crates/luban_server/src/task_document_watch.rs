use crate::engine::EngineCommand;
use luban_domain::{TaskDocumentKind, WorkspaceId, WorkspaceThreadId, paths};
use notify::{Event, RecursiveMode, Watcher as _};
use serde::Deserialize;
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
    SyncWorkspaces { workspaces: Vec<WorkspaceId> },
    Shutdown,
}

#[derive(Debug)]
struct WatchedState {
    tasks_root: PathBuf,
    active_workspaces: HashSet<WorkspaceId>,
}

#[derive(Clone, Debug)]
struct TaskIdentity {
    workspace_id: WorkspaceId,
    task_id: WorkspaceThreadId,
}

#[derive(Deserialize)]
struct TaskIdentityRecord {
    task_ulid: String,
    workspace_id: u64,
    task_id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CachedDocument {
    content_hash: String,
    byte_len: u64,
    updated_at_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TaskEventFile {
    Identity,
    Document(TaskDocumentKind),
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

            let mut watched: Option<WatchedState> = None;
            let mut cache = HashMap::<(String, TaskDocumentKind), CachedDocument>::new();
            let mut identities = HashMap::<String, TaskIdentity>::new();

            while let Ok(msg) = rx.recv() {
                match msg {
                    TaskDocumentWatchMessage::Command(cmd) => match cmd {
                        TaskDocumentWatchCommand::SyncWorkspaces { workspaces } => {
                            sync_workspaces(
                                &mut watcher,
                                &mut watched,
                                &mut cache,
                                &mut identities,
                                workspaces,
                            );
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
                        let Some(watched_state) = watched.as_ref() else {
                            continue;
                        };
                        let changed = changed_documents_from_event(
                            watched_state,
                            &mut cache,
                            &mut identities,
                            &event,
                        );
                        for (workspace_id, task_id, kind) in changed {
                            let _ = engine_tx.try_send(EngineCommand::TaskDocumentObserved {
                                workspace_id,
                                thread_id: task_id,
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

    pub(crate) fn sync_workspaces(&self, workspaces: Vec<WorkspaceId>) {
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
    watched: &mut Option<WatchedState>,
    cache: &mut HashMap<(String, TaskDocumentKind), CachedDocument>,
    identities: &mut HashMap<String, TaskIdentity>,
    workspaces: Vec<WorkspaceId>,
) {
    let active_workspaces: HashSet<WorkspaceId> = workspaces.into_iter().collect();
    let Some(tasks_root) = resolve_task_documents_root() else {
        tracing::debug!("task document watcher disabled: failed to resolve luban root");
        return;
    };
    if std::fs::create_dir_all(&tasks_root).is_err() {
        tracing::debug!(path = %tasks_root.display(), "task document watcher failed to create root");
        return;
    }
    let tasks_root = std::fs::canonicalize(&tasks_root).unwrap_or(tasks_root);

    let needs_rewatch = match watched.as_ref() {
        Some(entry) => entry.tasks_root != tasks_root,
        None => true,
    };
    if needs_rewatch {
        if let Some(existing) = watched.take() {
            let _ = watcher.unwatch(&existing.tasks_root);
        }
        cache.clear();
        identities.clear();
        if watcher
            .watch(&tasks_root, RecursiveMode::Recursive)
            .is_err()
        {
            tracing::debug!(path = %tasks_root.display(), "task document watcher failed to watch root");
            return;
        }
        *watched = Some(WatchedState {
            tasks_root: tasks_root.clone(),
            active_workspaces: active_workspaces.clone(),
        });
    }

    if let Some(entry) = watched.as_mut() {
        entry.active_workspaces = active_workspaces;
    }
}

fn changed_documents_from_event(
    watched: &WatchedState,
    cache: &mut HashMap<(String, TaskDocumentKind), CachedDocument>,
    identities: &mut HashMap<String, TaskIdentity>,
    event: &Event,
) -> Vec<(WorkspaceId, WorkspaceThreadId, TaskDocumentKind)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for path in &event.paths {
        let Some((task_ulid, event_file)) = resolve_task_event_path(&watched.tasks_root, path)
        else {
            continue;
        };

        match event_file {
            TaskEventFile::Identity => {
                if let Some(identity) = read_task_identity(&watched.tasks_root, &task_ulid) {
                    identities.insert(task_ulid, identity);
                } else {
                    identities.remove(&task_ulid);
                }
            }
            TaskEventFile::Document(kind) => {
                let key = (task_ulid.clone(), kind);
                let file_path = watched.tasks_root.join(&task_ulid).join(kind.file_name());
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

                if let Some(next) = next {
                    cache.insert(key, next);
                } else {
                    cache.remove(&key);
                }

                let Some(identity) =
                    load_task_identity(&watched.tasks_root, &task_ulid, identities)
                else {
                    continue;
                };
                if !watched.active_workspaces.contains(&identity.workspace_id) {
                    continue;
                }

                if seen.insert((identity.workspace_id, identity.task_id, kind)) {
                    out.push((identity.workspace_id, identity.task_id, kind));
                }
            }
        }
    }
    out
}

fn load_task_identity(
    tasks_root: &Path,
    task_ulid: &str,
    identities: &mut HashMap<String, TaskIdentity>,
) -> Option<TaskIdentity> {
    if let Some(identity) = identities.get(task_ulid) {
        return Some(identity.clone());
    }
    let identity = read_task_identity(tasks_root, task_ulid)?;
    identities.insert(task_ulid.to_owned(), identity.clone());
    Some(identity)
}

fn read_task_identity(tasks_root: &Path, task_ulid: &str) -> Option<TaskIdentity> {
    let path = tasks_root.join(task_ulid).join("task.json");
    if let Ok(content) = std::fs::read_to_string(path) {
        let record: TaskIdentityRecord = serde_json::from_str(&content).ok()?;
        if record.task_ulid != task_ulid {
            return None;
        }
        return Some(TaskIdentity {
            workspace_id: WorkspaceId::from_u64(record.workspace_id),
            task_id: WorkspaceThreadId::from_u64(record.task_id),
        });
    }

    parse_identity_from_fallback_ulid(task_ulid)
}

fn parse_identity_from_fallback_ulid(task_ulid: &str) -> Option<TaskIdentity> {
    let trimmed = task_ulid.trim();
    let rest = trimmed.strip_prefix("task-")?;
    let (workspace_str, task_str) = rest.split_once('-')?;
    if workspace_str.is_empty() || task_str.is_empty() {
        return None;
    }
    if task_str.contains('-') {
        return None;
    }
    let workspace_id = workspace_str.parse::<u64>().ok()?;
    let task_id = task_str.parse::<u64>().ok()?;
    Some(TaskIdentity {
        workspace_id: WorkspaceId::from_u64(workspace_id),
        task_id: WorkspaceThreadId::from_u64(task_id),
    })
}

fn resolve_task_event_path(tasks_root: &Path, raw_path: &Path) -> Option<(String, TaskEventFile)> {
    let canonical = std::fs::canonicalize(raw_path).ok();
    let relative = if let Ok(relative) = raw_path.strip_prefix(tasks_root) {
        relative.to_path_buf()
    } else if let Some(canonical) = &canonical {
        if let Ok(relative) = canonical.strip_prefix(tasks_root) {
            relative.to_path_buf()
        } else {
            return None;
        }
    } else {
        return None;
    };

    let mut components = relative.components();
    let task_component = components.next()?;
    let file_component = components.next()?;
    if components.next().is_some() {
        return None;
    }

    let task_ulid = task_component.as_os_str().to_str()?.trim().to_owned();
    if task_ulid.is_empty() {
        return None;
    }
    let file_name = file_component.as_os_str().to_str()?.trim();
    if file_name.eq_ignore_ascii_case("task.json") {
        return Some((task_ulid, TaskEventFile::Identity));
    }
    if file_name.eq_ignore_ascii_case("TASK.md") {
        return Some((task_ulid, TaskEventFile::Document(TaskDocumentKind::Task)));
    }
    if file_name.eq_ignore_ascii_case("PLAN.md") {
        return Some((task_ulid, TaskEventFile::Document(TaskDocumentKind::Plan)));
    }
    if file_name.eq_ignore_ascii_case("MEMORY.md") {
        return Some((task_ulid, TaskEventFile::Document(TaskDocumentKind::Memory)));
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

fn resolve_task_documents_root() -> Option<PathBuf> {
    let root = if let Some(root) = std::env::var_os(paths::LUBAN_ROOT_ENV) {
        let root = root.to_string_lossy();
        let trimmed = root.trim();
        if trimmed.is_empty() {
            return None;
        }
        PathBuf::from(trimmed)
    } else {
        let home = std::env::var_os("HOME")?;
        PathBuf::from(home).join("luban")
    };
    Some(root.join("tasks").join("v1").join("tasks"))
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
    fn resolve_task_event_path_ignores_unrelated_files() {
        let root = PathBuf::from("/tmp/luban/tasks/v1/tasks");
        assert!(
            resolve_task_event_path(&root, Path::new("/tmp/luban/tasks/v1/tasks/readme.md"))
                .is_none()
        );
        assert!(
            resolve_task_event_path(&root, Path::new("/tmp/luban/tasks/v1/tasks/1/other.md"))
                .is_none()
        );
    }

    #[test]
    fn resolve_task_event_path_parses_document_and_identity() {
        let root = PathBuf::from("/tmp/luban/tasks/v1/tasks");
        let doc = resolve_task_event_path(
            &root,
            Path::new("/tmp/luban/tasks/v1/tasks/01ARZ3NDEKTSV4RRFFQ69G5FAV/TASK.md"),
        );
        assert_eq!(
            doc,
            Some((
                "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
                TaskEventFile::Document(TaskDocumentKind::Task),
            ))
        );

        let meta = resolve_task_event_path(
            &root,
            Path::new("/tmp/luban/tasks/v1/tasks/01ARZ3NDEKTSV4RRFFQ69G5FAV/task.json"),
        );
        assert_eq!(
            meta,
            Some((
                "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
                TaskEventFile::Identity,
            ))
        );
    }

    #[test]
    fn parse_identity_from_fallback_ulid_works() {
        let identity = parse_identity_from_fallback_ulid("task-12-34");
        assert!(identity.is_some());
        let identity = identity.expect("identity should be parsed");
        assert_eq!(identity.workspace_id.as_u64(), 12);
        assert_eq!(identity.task_id.as_u64(), 34);
    }

    #[test]
    fn parse_identity_from_fallback_ulid_rejects_invalid() {
        assert!(parse_identity_from_fallback_ulid("task-12").is_none());
        assert!(parse_identity_from_fallback_ulid("task-12-34-56").is_none());
        assert!(parse_identity_from_fallback_ulid("task-a-34").is_none());
        assert!(parse_identity_from_fallback_ulid("foo-12-34").is_none());
    }
}
