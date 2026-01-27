# Localhost Web UI Quickstart

This repo contains a local Rust server (`luban_server`) and a browser UI (`web/`).

## Run (recommended)

1. Run:
   - `just web run`
2. Open:
   - `http://127.0.0.1:8421/`

`just web run` builds `web/` (requires `pnpm`) and starts `luban_server`, which serves
the built assets from `/`.

## Run (browser, zero-config launcher)

1. Run:
   - `cargo run -p luban_cli -- ui`
2. Your default browser should open automatically.

The launcher:

- binds to `127.0.0.1` with a random available port
- bootstraps a session cookie via `GET /auth?token=...`
- then navigates to `/` to load the UI

## Run (mock mode, UI-first preview)

Mock mode runs the browser UI without starting the Rust server. This is the preferred workflow for
fast interaction iteration.

1. Run:
   - `just web dev-mock`
2. Open:
   - `http://localhost:3000/` (Next.js dev server)

Notes:

- Mock mode is controlled by `NEXT_PUBLIC_LUBAN_MODE=mock`.
- Integration with the Rust server is tracked and enforced via contracts under `docs/contracts/`.
- See `docs/migrations/2026-01-22-web-mock-mode-contracts.md` for the migration plan.
- Terminal PTY streaming is mocked locally in mock mode (no server required, limited shell behavior).

## Run (Tauri shell)

1. Run:
   - `just app run`

The Tauri app starts the same local `luban_server` in-process and loads it in a WebView.

## Run (development, hot reload)

1. Start the Rust server:
   - `just run-server`
2. Start the web dev server:
   - `just web dev`
3. Open:
   - `http://localhost:3000/` (Next.js)

Note: `pnpm dev` does not proxy `/api/*` by default. For end-to-end testing against real APIs,
prefer `just web run` (single process, same-origin `/api/*`).

## Run (single process serving web assets)

1. Build the web UI:
   - `cd web && pnpm install && pnpm build`
2. Start the Rust server:
   - `just run-server`
3. Open:
   - `http://127.0.0.1:8421/`

## Verify (local)

- Rust: `just fmt && just lint && just test`
- Web: `cd web && pnpm run typecheck && pnpm run build`
- Contracts: `just test` (enforces basic contract coverage for server routes)

## Notes

- The server binds to loopback only (localhost).
- Override bind addr/port via `LUBAN_SERVER_ADDR` (e.g. `127.0.0.1:8422`).
- Device-local UI state (draft/scroll/layout) lives in browser `localStorage`.
- Durable app settings (appearance, agent defaults) live in SQLite.
