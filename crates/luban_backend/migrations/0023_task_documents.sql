CREATE TABLE IF NOT EXISTS task_documents (
  project_slug TEXT NOT NULL,
  workspace_name TEXT NOT NULL,
  thread_local_id INTEGER NOT NULL,
  kind TEXT NOT NULL,
  rel_path TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  byte_len INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (project_slug, workspace_name, thread_local_id, kind)
);

CREATE INDEX IF NOT EXISTS idx_task_documents_scope
  ON task_documents(project_slug, workspace_name, thread_local_id);

CREATE TABLE IF NOT EXISTS task_document_events (
  event_id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_slug TEXT NOT NULL,
  workspace_name TEXT NOT NULL,
  thread_local_id INTEGER NOT NULL,
  kind TEXT NOT NULL,
  event_type TEXT NOT NULL,
  rel_path TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  byte_len INTEGER NOT NULL,
  created_at_unix_ms INTEGER NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_task_document_events_scope
  ON task_document_events(project_slug, workspace_name, thread_local_id, event_id);
