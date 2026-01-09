PRAGMA foreign_keys = OFF;

CREATE TABLE conversations_v3 (
  project_slug     TEXT NOT NULL,
  workspace_name   TEXT NOT NULL,
  thread_local_id  INTEGER NOT NULL,
  thread_id        TEXT,
  title            TEXT,
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL,
  PRIMARY KEY (project_slug, workspace_name, thread_local_id)
);

CREATE TABLE conversation_entries_v3 (
  id              INTEGER PRIMARY KEY,
  project_slug    TEXT NOT NULL,
  workspace_name  TEXT NOT NULL,
  thread_local_id INTEGER NOT NULL,
  seq             INTEGER NOT NULL,
  kind            TEXT NOT NULL,
  codex_item_id   TEXT,
  payload_json    TEXT NOT NULL,
  created_at      INTEGER NOT NULL,
  UNIQUE(project_slug, workspace_name, thread_local_id, seq),
  UNIQUE(project_slug, workspace_name, thread_local_id, codex_item_id)
);

INSERT INTO conversations_v3 (project_slug, workspace_name, thread_local_id, thread_id, title, created_at, updated_at)
SELECT project_slug, workspace_name, 1, thread_id, NULL, created_at, updated_at
FROM conversations;

INSERT INTO conversation_entries_v3 (project_slug, workspace_name, thread_local_id, seq, kind, codex_item_id, payload_json, created_at)
SELECT project_slug, workspace_name, 1, seq, kind, codex_item_id, payload_json, created_at
FROM conversation_entries;

DROP TABLE conversation_entries;
DROP TABLE conversations;

ALTER TABLE conversations_v3 RENAME TO conversations;
ALTER TABLE conversation_entries_v3 RENAME TO conversation_entries;

CREATE INDEX conversation_entries_workspace_seq
  ON conversation_entries(project_slug, workspace_name, thread_local_id, seq);

PRAGMA foreign_keys = ON;

