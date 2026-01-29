# Technical Consensus (Implemented)

This document records the major constraints and decisions that are already implemented in the
codebase. If any other document conflicts with this one, treat this document as the source of
truth and update the conflicting document.

## Localhost-only client/server model

- The system is split into:
  - a local Rust server (`crates/luban_server`)
  - a browser UI served from the same origin (`web/`, served by the server)
- The server binds to loopback only by default (`127.0.0.1:8421`).
- The server is authoritative for domain state and side effects.

See `docs/cs-web-architecture.md`.

## Action-driven protocol

- The UI sends user intent as actions over a single WebSocket (`/api/events`).
- The server applies actions via `luban_domain` reducers and emits rev-stamped incremental events.
- Query APIs exist for hydration and resync (`/api/app`, `/api/workspaces/...`).

See `docs/cs-web-architecture.md`.

## UI workflow (mock-first + contracts)

- `web/` is the UI source of truth.
- UI iteration should happen in `web/` mock mode (`NEXT_PUBLIC_LUBAN_MODE=mock`).
- Web/server integration is governed by consumer-driven contracts under `docs/contracts/`.

## UI state: durable vs device-local

Luban splits "UI state" into two categories:

- **Durable preferences** stored in SQLite via `Effect::SaveAppState` (shared across instances using
  the same DB):
  - appearance (theme + fonts)
  - agent defaults
  - workspace UX state (tabs, scroll anchors, unread markers)
- **Device-local preferences** stored in browser `localStorage` (not shared across machines):
  - draft text
  - follow-tail preference
  - tab strip ordering and other presentation-only toggles
  - pane widths

See `docs/persistence.md`.

## Structured attachments (no inline tokens)

- User message text is plain text.
- Attachments are structured fields on user messages (`attachments: Vec<AttachmentRef>`).
- The browser UI does not embed filesystem paths into message text.

See `docs/cs-web-architecture.md` and `docs/persistence.md`.

## Terminal: `ghostty-web` + PTY over WebSocket

- The terminal UI is rendered with `ghostty-web`.
- The server owns the PTY and exposes a dedicated WS endpoint (`/api/pty/:workspace_id/:pty_id`).
- New PTY connections receive a bounded replay of recent output for a stable refresh/reconnect UX.

See `docs/terminal.md`.

## Codex integration (CLI-first, no sidecar)

- Luban runs the Codex CLI directly rather than using a Node.js sidecar process.
- The CLI binary is discovered via `PATH` or `LUBAN_CODEX_BIN`.

See `docs/codex-cli.md`.

## Explicit non-goals

- Remote access / multi-machine use.
- Authentication/authorization and security hardening (localhost-only use).
- Multi-user support or concurrent writers.
- Maintaining a legacy native UI implementation.
