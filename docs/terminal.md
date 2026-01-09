# Web Terminal (PTY + `ghostty-web`)

Luban provides an interactive terminal pane in the web UI.

## Components

- Client: `web/components/pty-terminal.tsx` renders the terminal with `ghostty-web`.
- Server: `crates/luban_server/src/pty.rs` owns the PTY process and multiplexes IO over WebSocket.

## Protocol

- Endpoint: `GET /api/pty/:workspace_id/:pty_id`
- Messages:
  - client -> server:
    - `{ "type": "input", "data": "..." }`
    - `{ "type": "resize", "cols": <u16>, "rows": <u16> }`
  - server -> client:
    - binary frames containing PTY output bytes

The PTY channel is separate from `/api/events` to avoid head-of-line blocking.

## Session scoping

- The server keys PTY sessions by `(workspace_id, pty_id)`.
- The current web UI uses a single reserved `pty_id = 1` per workspace, so the terminal is shared
  across conversation threads within the workspace.

If strict per-thread terminal isolation is needed later, the client can use a distinct `pty_id` per
conversation thread and keep the server unchanged.

## Refresh/reconnect behavior

When a browser refreshes or reconnects:

- the server replays a bounded amount of recent output for that PTY session (best-effort)
- then the connection continues streaming new output

This avoids a blank terminal after refresh while keeping memory bounded.

## Theme + layout

- The terminal theme is derived from CSS variables and applied by emitting OSC color sequences on
  initialization.
- Pane width is resizable and stored in browser `localStorage` (UI-only).

