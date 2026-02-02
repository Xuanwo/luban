PRAGMA foreign_keys = OFF;

CREATE TABLE conversation_entries_v4 (
  id              INTEGER PRIMARY KEY,
  project_slug    TEXT NOT NULL,
  workspace_name  TEXT NOT NULL,
  thread_local_id INTEGER NOT NULL,
  seq             INTEGER NOT NULL,
  entry_id        TEXT NOT NULL,
  kind            TEXT NOT NULL,
  codex_item_id   TEXT,
  payload_json    TEXT NOT NULL,
  created_at      INTEGER NOT NULL,
  UNIQUE(project_slug, workspace_name, thread_local_id, seq),
  UNIQUE(project_slug, workspace_name, thread_local_id, entry_id)
);

INSERT INTO conversation_entries_v4 (id, project_slug, workspace_name, thread_local_id, seq, entry_id, kind, codex_item_id, payload_json, created_at)
SELECT id, project_slug, workspace_name, thread_local_id, seq, printf('e_%d', seq), kind, codex_item_id, payload_json, created_at
FROM conversation_entries;

DROP TABLE conversation_entries;
ALTER TABLE conversation_entries_v4 RENAME TO conversation_entries;

CREATE INDEX conversation_entries_workspace_seq
  ON conversation_entries(project_slug, workspace_name, thread_local_id, seq);

PRAGMA foreign_keys = ON;
