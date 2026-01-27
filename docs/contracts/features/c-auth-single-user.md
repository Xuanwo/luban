# C-AUTH-SINGLE-USER

This contract defines the optional single-user authentication flow used by the `luban ui` launcher.

## Scope

When `AuthMode::SingleUser` is enabled on the Rust server:

- All `/api/*` endpoints are protected **except** `GET /api/health`.
- Both WebSocket endpoints (`/api/events`, `/api/pty/*`) are protected during the handshake.

When auth is disabled, requests behave as documented by the individual endpoint contracts.

## Bootstrap flow

The launcher generates a random token and opens the browser at:

- `GET /auth?token=<bootstrap_token>`

On success, the server:

- Sets a session cookie:
  - `Set-Cookie: luban_session=<bootstrap_token>; Path=/; HttpOnly; SameSite=Lax`
- Returns a small HTML page that replaces the current history entry and navigates to `/`.

The `<bootstrap_token>` is accepted **once** to establish the session, but may be reused to
re-issue the session cookie (idempotent retry) for the lifetime of the server process.

## Unauthorized behavior

For protected surfaces, when no valid session cookie is present:

- HTTP endpoints return `401` with a plain-text body: `unauthorized`.
- WebSocket handshakes return `401`.
