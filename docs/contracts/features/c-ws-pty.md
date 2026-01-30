# C-WS-PTY

Status: Draft
Verification: Mock=yes, Provider=yes, CI=yes

## Surface

- WebSocket path: `/api/pty/{workdir_id}/{task_id}`
- Query parameters:
  - `reconnect` (string, optional): client-generated token that keys the PTY session.

## Purpose

PTY streaming for the terminal UI.

## Invariants

- Provider invariants:
  - The server must provide a bounded replay on connect for stable refresh/reconnect UX.
  - The `reconnect` token must map to a stable PTY session within a workspace.
  - The provider must apply backpressure (no output byte dropping). If a client cannot keep up, the
    server may close the connection to force a resync.
  - The protocol must be robust to reconnects and network loss.
  - If a client lags behind and drops output (e.g. due to a slow network), the server may replay the
    bounded output history again to help the client recover.

- Mock-mode invariant:
  - The UI must render an interactive terminal without requiring the server. This is implemented as a
    local-only shell simulation and is not expected to match provider behavior byte-for-byte.

## Web usage

- `web/components/pty-terminal.tsx` (terminal transport)
