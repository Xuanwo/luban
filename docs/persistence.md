## SQLite persistence and migrations design

### Goals

- Persist user-added `Project`s and `Workspace`s so a restart restores the sidebar list.
- Preserve existing per-workspace conversation history across restarts.
- Support automated, testable schema evolution (migrations) without blocking the UI thread.
- Keep the `domain` crate pure (no IO) and keep all IO in an adapter layer.

### Non-goals

- Cross-device sync.
- Multi-user / concurrent writers.
- Full-text search, indexing, or analytics over conversations (can be added later).
- Migrating existing on-disk worktrees/conversations into a new storage engine in the first iteration.

---

## Current state (observed)

- Conversations are persisted under `~/luban/conversations/<project_slug>/<workspace_name>/`:
  - `conversation.json` (meta; includes `version` and `thread_id`)
  - `events.jsonl` (append-only JSON Lines of `ConversationEntry`)
- Projects/workspaces exist only in memory (`luban_domain::AppState`) and are lost on restart.
- IO is already correctly executed off the UI thread via `background_spawn` in `luban_ui`.

---

## Recommended v2: SQLite as the source of truth

The database becomes the durable store for:

- projects/workspaces (the sidebar model)
- per-workspace conversations (thread id + event stream)

The UI and `domain` continue to use an in-memory `AppState`, but it is loaded from SQLite on startup and persisted back via explicit effects.

### Storage layout

- Root directory: `~/luban/`
- Database file: `~/luban/luban.db`
- Existing `~/luban/conversations/...` can be kept as legacy (optional, see “Migration from legacy files”).

### SQLite configuration (startup)

Open the database in background work and apply the following per-connection settings:

- `PRAGMA foreign_keys = ON;`
- `PRAGMA journal_mode = WAL;`
- `PRAGMA synchronous = NORMAL;`
- `PRAGMA busy_timeout = 5000;`

Notes:

- WAL keeps the UI responsive under frequent small writes (conversation streaming) and provides crash safety.
- `busy_timeout` reduces “database is locked” errors under contention (still prefer a single-writer design).

### Schema overview

Core entities:

- `projects`
- `workspaces`
- `conversations` (1:1 with workspace; stores `thread_id`)
- `conversation_entries` (append-only event stream per workspace)

Durable keys:

- Use stable integer IDs for projects/workspaces.
- Preserve `slug` and `workspace_name` as stable routing keys (also helpful for imports and debugging).

### DDL (initial migration example)

Prefer embedding migrations as SQL files (monotonic numbering). Example `0001_init.sql`:

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE projects (
  id              INTEGER PRIMARY KEY,
  slug            TEXT NOT NULL UNIQUE,
  name            TEXT NOT NULL,
  path            TEXT NOT NULL,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL
);

CREATE TABLE workspaces (
  id              INTEGER PRIMARY KEY,
  project_id      INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  workspace_name  TEXT NOT NULL,
  branch_name     TEXT NOT NULL,
  worktree_path   TEXT NOT NULL,
  status          INTEGER NOT NULL, -- 0 = active, 1 = archived
  last_activity_at INTEGER,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  UNIQUE(project_id, workspace_name)
);

CREATE TABLE conversations (
  project_slug    TEXT NOT NULL,
  workspace_name  TEXT NOT NULL,
  thread_id       TEXT,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  PRIMARY KEY (project_slug, workspace_name)
);

CREATE TABLE conversation_entries (
  id              INTEGER PRIMARY KEY,
  project_slug    TEXT NOT NULL,
  workspace_name  TEXT NOT NULL,
  seq             INTEGER NOT NULL,
  kind            TEXT NOT NULL, -- "user_message" | "codex_item" | "turn_duration" | ...
  codex_item_id   TEXT,          -- populated only when kind = "codex_item"
  payload_json    TEXT NOT NULL, -- serde_json of ConversationEntry
  created_at      INTEGER NOT NULL,
  UNIQUE(project_slug, workspace_name, seq),
  UNIQUE(project_slug, workspace_name, codex_item_id)
);

CREATE INDEX conversation_entries_workspace_seq
  ON conversation_entries(project_slug, workspace_name, seq);
```

### Event encoding (`conversation_entries.payload_json`)

Store `luban_domain::ConversationEntry` as JSON, but also denormalize:

- `kind` for cheap filtering/diagnostics
- `codex_item_id` for idempotent insertion of Codex items
- `seq` for stable ordering

This pattern minimizes schema churn when the conversation model evolves.

### Migration strategy

Migrations must be:

- automatic (applied on startup)
- transactional (either fully applied or not applied)
- monotonic and testable

Recommended mechanism:

- Track the current schema version using `PRAGMA user_version`.
- Keep migrations as an ordered list of SQL scripts.
- Apply `BEGIN IMMEDIATE;` → run scripts → set `PRAGMA user_version = <n>;` → `COMMIT;`
- On failure: rollback and surface a user-visible error (do not proceed with partial schema).

This avoids adding a “schema_migrations” table while still being robust for an internal desktop app.

### Concurrency model

To keep the UI responsive and eliminate lock contention:

- Treat SQLite as a single-writer resource.
- All DB access runs in background tasks.
- Prefer one of two implementations:
  1. **Dedicated DB worker thread (recommended):** a single `rusqlite::Connection` lives on that thread; all operations are executed by sending commands over a channel.
  2. **Per-operation connection (simpler):** open a `rusqlite::Connection` inside each background task, set PRAGMAs, execute, close.

Option (1) is more stable under frequent streaming writes.

### Domain integration (actions/effects)

Keep `luban_domain` pure by modeling persistence as effects and by avoiding DB-driven IDs in the reducer.

- New `Effect` variants (examples):
  - `LoadAppState`
  - `SaveAppState`
- New `Action` variants (examples):
  - `AppStateLoaded { snapshot: PersistedSnapshot }`
  - `AppStateLoadFailed { message: String }`
  - `AppStateSaved`
  - `AppStateSaveFailed { message: String }`

Reducer behavior:

- On startup, issue `Effect::LoadAppState`.
- On state mutations that change projects/workspaces, issue `Effect::SaveAppState`.
- Optional: debounce saves (e.g. 250ms) to reduce write churn.

Adapter behavior:

- Perform load/save on background threads.
  - For v2, load from SQLite (projects/workspaces tables).
  - Restore `next_project_id`/`next_workspace_id` from `MAX(id) + 1`.

### Conversation compatibility and evolution

Persist conversations in SQLite as an append-only stream.

Rules to maintain forward/backward compatibility:

- Do not remove/rename existing `ConversationEntry` variants.
- When adding fields, use `#[serde(default)]` for new fields.
- If future-proofing is needed, introduce an `Unknown` entry variant and preserve raw JSON to avoid hard failures on unknown tags.

---

## Migration from legacy files (optional, recommended)

If `~/luban/conversations/...` exists, we can import it once:

- On first startup with SQLite enabled:
  - If `projects` is empty, scan the legacy directory tree and import known conversation logs.
  - If `projects` is non-empty, skip import.

Import strategy:

- Import only conversations for existing projects/workspaces (match by `slug` and `workspace_name`).
- Use `INSERT OR IGNORE` for `conversation_entries` to make imports idempotent.
- Record a marker in `PRAGMA user_version` or a small `meta` table once completed.

If we want to keep legacy files for debugging, add an export command later; it should not be part of the critical path.

---

## Verification plan (for implementation)

### Unit tests

- Migration application:
  - start from an empty DB and reach `LATEST`
  - start from an older `user_version` and upgrade
- Persistence roundtrip:
  - create projects/workspaces, persist, reload, compare the durable fields
- Conversation append/load:
  - append a sequence of `ConversationEntry`s, reload, verify ordering and deduplication behavior for Codex items
- Failure handling:
  - invalid DB path, locked DB, corrupt DB file (surface error, keep UI responsive)

### Manual steps

1. Launch the app and ensure it loads without UI stalls.
2. Add a project and create a workspace.
3. Send a message and wait for a few streamed entries.
4. Restart the app.
5. Verify the project/workspace list is restored.
6. Open the workspace and verify conversation history loads from SQLite.
7. Archive a workspace, restart, verify it remains archived.
