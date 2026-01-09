# Persistence

This document describes what is durable (server-side) and what is UI-only (browser-side).

## Durable (server-side)

### SQLite

- SQLite is the durable store for projects/workspaces and conversation history.
- Migrations are applied via `PRAGMA user_version` in `crates/luban_backend/src/sqlite_store.rs`.
- Domain <-> persisted mapping is centralized in `crates/luban_domain/src/persistence.rs`.

### Workspace-scoped context storage

Attachments are stored on disk under a workspace-scoped directory:

```
~/luban/conversations/<project_slug>/<workspace_name>/context/
  blobs/
    <blake3>.<ext>
    <blake3>.thumb.<ext>
```

- Blobs are content-addressed (BLAKE3).
- Attachments are referenced from messages as structured `AttachmentRef`s; the web UI does not embed
  filesystem paths in message text.

## UI-only (browser-side)

The web UI is a single-page app. State that does not affect correctness lives in browser
`localStorage` (and is not required to be durable across machines).

Current keys (non-exhaustive):

- `luban:ui:right_sidebar_open`
- `luban:ui:view_mode`
- `luban:ui:sidebar_width_px`
- `luban:ui:right_sidebar_width_px`
- `luban:draft:<workspace_id>:<thread_id>`
- `luban:follow_tail:<workspace_id>:<thread_id>`
- `luban:ui:thread_order:<workspace_id>`

## Non-goals

- Cross-device sync.
- Multi-user / concurrent writers.


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
