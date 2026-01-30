# Workspace Thread Tabs

This document defines how Luban supports multiple conversation threads within the same workspace.
The UI model is browser-like tabs. A tab is an opened view of a thread.

## Goals

- Allow multiple independent conversation threads within one workspace.
- Provide browser-like tab navigation with minimal chrome.
- Preserve draft/scroll/model settings per thread (UI-only where applicable).
- Allow concurrent agent runs per thread.

## Non-goals (initial)

- PDF/DOCX attachment previews (handled elsewhere).
- Cross-workspace tab grouping.
- Global search across threads.

## Concepts

### Thread

A thread is the durable conversation unit:

- Messages (user/assistant/agent events)
- Draft text
- Follow-tail / scroll preference (UI-only)
- Per-thread run configuration (model + thinking effort)
- Turn state (running, pending, etc.)

Thread identity is local-first:

- `WorkspaceThreadId` is a locally-generated `u64` used for UI, persistence, and stable references.
- `remote_thread_id: Option<String>` is the Codex thread id. It is `None` until the first request
  starts a remote thread and we receive `thread.started`.

### Tab

A tab is an opened view of a thread. Tabs are not durable objects; they are derived from thread ids.

Workspace state stores:

- `open_tabs: Vec<WorkspaceThreadId>`: the visible tab strip order (user-defined).
- `archived_tabs: Vec<WorkspaceThreadId>`: closed tabs that can be restored.
- `active_tab: WorkspaceThreadId`: the currently selected thread.

Closing a tab archives it:

- Remove from `open_tabs`.
- Append to `archived_tabs` (no duplicates).
- Select a neighbor tab as the next active tab when possible.

Archiving does not delete the thread. Messages and drafts remain durable.

## UI

### Placement

The tab strip is rendered inside the chat pane:

- Workspace page: above message history, below the workspace pane title bar.
- Dashboard preview panel: in the preview header (same style), above the preview message list.

### Tab strip rules

- Tabs are shown in a stable, user-defined order.
- `+` creates a new thread, appends a tab at the end, and activates it.
- A `v` dropdown button opens the Threads menu (all threads in this workspace).
- Activating a thread does not reorder the tab strip.
- The tab strip does not horizontally scroll. Tabs compress to fit (min/max width per tab). Use the
  dropdown menu when tabs become too narrow.
- Tabs show:
  - Title (derived from thread title)
  - Dirty indicator when the thread has a non-empty draft or draft attachments
  - Running indicator when the thread has an active turn
- `x` close button on hover archives the tab (no confirmation).

### Threads menu (overflow)

The menu renders two sections:

- **Active**: threads currently in `open_tabs`
- **Archived**: threads in `archived_tabs` (this section is hidden when empty)

Rules:

- The menu supports vertical scrolling with a scrollbar when needed.
- Clicking an item activates the thread and ensures it is in `open_tabs`.
- Active rows can be archived from the menu (disabled when it would leave zero open tabs).
- Archived rows can be restored from the menu.

Future (not implemented in the initial delivery):

- Rename thread.
- Delete thread (confirmation; deletes local thread state only).

## Terminal behavior

The web terminal is scoped per workspace (shared across threads):

- The current web UI uses one reserved PTY session per workspace.
- Switching tabs does not switch PTY state.

See `docs/terminal.md`.

## Agent execution model

Turns are tracked per thread:

- Running/pending state is keyed by `(workspace_id, thread_id)`.
- Cancels are per thread.
- Turn event routing uses a `(workspace_id, thread_id)` key to append entries to the correct thread.

If a thread has `remote_thread_id = None`, the first turn starts a new remote thread; the returned
`thread.started` event binds the remote id to the local thread id.

## Persistence

Persisted app state includes:

- Server (SQLite): projects/workspaces, threads, conversation entries, agent state, and tab strip state
  (`open_tabs`, `archived_tabs`, `active_tab`).
- Browser (`localStorage`): drafts and follow-tail preference (device-local).

## Test strategy

- Domain tests:
  - Closing tabs archives them and preserves thread state (draft/messages preserved).
  - Restoring an archived tab returns it to `open_tabs`.
  - Concurrent turns are isolated per thread.
- UI tests (agent-browser):
  - Smoke coverage is maintained in `web/tests/agent-browser/`.
