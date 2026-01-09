PRAGMA foreign_keys = ON;

CREATE TABLE projects (
  id         INTEGER PRIMARY KEY,
  slug       TEXT NOT NULL UNIQUE,
  name       TEXT NOT NULL,
  path       TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE workspaces (
  id               INTEGER PRIMARY KEY,
  project_id       INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  workspace_name   TEXT NOT NULL,
  branch_name      TEXT NOT NULL,
  worktree_path    TEXT NOT NULL,
  status           INTEGER NOT NULL,
  last_activity_at INTEGER,
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL,
  UNIQUE(project_id, workspace_name)
);

CREATE TABLE conversations (
  workspace_id INTEGER PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
  thread_id    TEXT,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);

CREATE TABLE conversation_entries (
  id           INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  seq          INTEGER NOT NULL,
  kind         TEXT NOT NULL,
  codex_item_id TEXT,
  payload_json TEXT NOT NULL,
  created_at   INTEGER NOT NULL,
  UNIQUE(workspace_id, seq),
  UNIQUE(workspace_id, codex_item_id)
);

CREATE INDEX conversation_entries_workspace_seq
  ON conversation_entries(workspace_id, seq);

