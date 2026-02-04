PRAGMA foreign_keys = ON;

CREATE TABLE new_task_drafts (
  id             TEXT PRIMARY KEY,
  text           TEXT NOT NULL,
  project_id     TEXT,
  workspace_id   INTEGER,
  created_at_ms  INTEGER NOT NULL,
  updated_at_ms  INTEGER NOT NULL
);

CREATE INDEX new_task_drafts_updated_at
  ON new_task_drafts(updated_at_ms DESC, created_at_ms DESC, id DESC);

CREATE TABLE new_task_stash (
  id             INTEGER PRIMARY KEY CHECK (id = 1),
  text           TEXT NOT NULL,
  project_id     TEXT,
  workspace_id   INTEGER,
  editing_draft_id TEXT,
  updated_at_ms  INTEGER NOT NULL
);

