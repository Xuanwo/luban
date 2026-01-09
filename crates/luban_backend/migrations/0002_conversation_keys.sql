PRAGMA foreign_keys = OFF;

CREATE TABLE conversations_v2 (
  project_slug   TEXT NOT NULL,
  workspace_name TEXT NOT NULL,
  thread_id      TEXT,
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  PRIMARY KEY (project_slug, workspace_name)
);

CREATE TABLE conversation_entries_v2 (
  id            INTEGER PRIMARY KEY,
  project_slug  TEXT NOT NULL,
  workspace_name TEXT NOT NULL,
  seq           INTEGER NOT NULL,
  kind          TEXT NOT NULL,
  codex_item_id TEXT,
  payload_json  TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  UNIQUE(project_slug, workspace_name, seq),
  UNIQUE(project_slug, workspace_name, codex_item_id)
);

INSERT INTO conversations_v2 (project_slug, workspace_name, thread_id, created_at, updated_at)
SELECT p.slug, w.workspace_name, c.thread_id, c.created_at, c.updated_at
FROM conversations c
JOIN workspaces w ON w.id = c.workspace_id
JOIN projects p ON p.id = w.project_id;

INSERT INTO conversation_entries_v2 (project_slug, workspace_name, seq, kind, codex_item_id, payload_json, created_at)
SELECT p.slug, w.workspace_name, e.seq, e.kind, e.codex_item_id, e.payload_json, e.created_at
FROM conversation_entries e
JOIN workspaces w ON w.id = e.workspace_id
JOIN projects p ON p.id = w.project_id;

DROP TABLE conversation_entries;
DROP TABLE conversations;

ALTER TABLE conversations_v2 RENAME TO conversations;
ALTER TABLE conversation_entries_v2 RENAME TO conversation_entries;

CREATE INDEX conversation_entries_workspace_seq
  ON conversation_entries(project_slug, workspace_name, seq);

PRAGMA foreign_keys = ON;

