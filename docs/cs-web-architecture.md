# Client/Server Architecture (Localhost Web UI)

This document describes the current, implemented architecture of Luban as a local client/server
system:

- **Server**: a Rust process running on `localhost`, authoritative for domain state and side effects.
- **Client**: a browser-based UI that renders state and sends user intent as actions.

The system preserves the domain-driven design (`Action` + `Effect` + reducer) while using a browser
UI as the primary frontend.

## Goals

- Keep `crates/luban_domain` as the single source of truth for application behavior.
- Move "dispatch/apply/effect runner" to the server, and make the client a thin renderer.
- Support query APIs for initial state hydration.
- Support a single WebSocket (`/api/events`) for:
  - client -> server actions
  - server -> client incremental events (state deltas and streaming updates)
- Provide an interactive terminal pane in the web UI using `ghostty-web`.
- Move draft/scroll/layout persistence from SQLite into browser `localStorage`.
- Make attachments fully structured (no tokens embedded in message text).

## Non-goals

- Remote access / multi-machine use.
- Authentication/authorization and security hardening (server binds to loopback only).
- Multi-user support or concurrent writers (the server is single-writer authoritative).

## Current constraints from the codebase

- The domain layer is already action/effect driven and IO-free.
- Persistence is already SQLite and migrations are already implemented.
- The server PTY model is per `(workspace_id, pty_id)` session isolation.

## Architecture overview

### Layers

- `luban_domain`:
  - Pure state (`AppState`) and reducer (`apply(Action) -> Vec<Effect>`).
  - Domain models for projects/workspaces/conversations.
- `luban_backend` (new):
  - Adapter implementations for IO (git worktrees, sqlite, codex CLI, context blobs).
  - Designed to be usable by both a local server and tests.
- `luban_api` (new):
  - Wire DTOs (serde) for HTTP/WS between the web client and local server.
  - No `PathBuf` in client inputs.
- `luban_server` (new):
  - HTTP + WebSocket API.
  - Authoritative engine that:
    - accepts `ClientAction`
    - applies it to `luban_domain::AppState`
    - runs `Effect` via `luban_backend`
    - publishes `ServerEvent` to subscribers
- `web/` (new):
  - Browser UI (SPA) that:
    - hydrates from query APIs
    - sends actions via WebSocket
    - renders incremental updates
    - manages "UI-only" state via `localStorage`

### Engine model (single-writer)

The server runs an engine loop:

1. Receive `ClientAction` from the WebSocket.
2. Map to `luban_domain::Action` (or reject with an error).
3. Apply action: `effects = app_state.apply(action)`.
4. Emit `ServerEvent`s (rev-stamped) describing the state change.
5. Execute each effect asynchronously using `luban_backend`.
6. When effects complete, the engine dispatches follow-up domain actions (e.g. load failed, event received).

The engine is the only component that mutates `AppState`.

## API surface

### Query API (HTTP)

Minimal initial endpoints:

- `GET /api/app` -> `AppSnapshot`
- `GET /api/workspaces/:workspace_id/threads` -> `ThreadsSnapshot`
- `GET /api/workspaces/:workspace_id/conversations/:thread_id` -> `ConversationSnapshot`
- `POST /api/workspaces/:workspace_id/attachments` -> upload (multipart) -> `AttachmentRef`
- `GET /api/workspaces/:workspace_id/attachments/:attachment_id?ext=...` -> bytes + content-type

### Subscription + actions (WebSocket)

Single WebSocket:

- `GET /api/events` upgrades to WS.
- Client sends `WsClientMessage`.
- Server sends `WsServerMessage`.

The protocol uses a monotonic `rev: u64`:

- Every accepted action increments `rev`.
- Every server event is stamped with the resulting `rev`.
- Client tracks `last_rev`; if it observes a gap, it should re-hydrate via HTTP.

This avoids requiring the client to interpret complex diffs while still supporting incremental UI updates.

## Attachments (structured)

### Motivation

The previous token-in-text approach embedded absolute filesystem paths. This is incompatible with a
browser-based UI and forces message text to contain filesystem details.

### New model

- A user message has:
  - `text: String`
  - `attachments: Vec<AttachmentRef>`
- An attachment is addressed by an opaque `attachment_id` (server-local).
- The client uploads new attachments to the server (HTTP endpoint) and receives `attachment_id`s.
- The server persists attachment metadata and stores blobs in the existing workspace-scoped context
  directory.

The server is responsible for:

- rendering attachments in conversation history
- passing image attachments to the Codex CLI (`--image` paths resolved server-side)

## Terminal (interactive PTY)

The web terminal uses `ghostty-web`.

Server responsibilities:

- own PTY processes per `(workspace_id, pty_id)`
- expose a dedicated WebSocket:
  - `GET /api/pty/:workspace_id/:pty_id`
- support:
  - input (client -> server)
  - output (server -> client)
  - resize events
  - bounded output replay on reconnect (best-effort)

The PTY channel is intentionally separate from `/api/events` to avoid head-of-line blocking when
terminal output is high-volume.

## Browser `localStorage` (UI-only persistence)

The following state moves from domain/SQLite into browser storage:

- draft text and draft attachments (by `attachment_id`)
- scroll position / follow-tail preference
- layout widths (sidebar, terminal pane, dashboard preview)
- active workspace/thread selection
- other purely presentational toggles

The server remains authoritative for:

- projects/workspaces
- conversation entries
- agent runs and streamed events

## Serving the Web UI

The server serves the built web assets so the user can open a browser and visit `http://localhost:<port>/`.

**UI parity requirement:** the web UI must follow the same frontend framework and design system as
`Xuanwo/luban-design`. See `docs/ui-design-parity.md`.

The browser UI is built via Next.js static export (`output: "export"`), and the Rust server serves the output folder.

- Development: run `just run-server` + `cd web && pnpm dev` (hot reload).
  - Note: the dev server does not proxy `/api/*` by default; for end-to-end testing, prefer `just web run` (alias: `just run`).
- Production: build `web/` into `web/out` and serve it from `luban_server` (default).
  - Override the served directory via `LUBAN_WEB_DIST_DIR`.
