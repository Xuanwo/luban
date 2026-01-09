# Localhost Web UI Quickstart

This repo contains a local Rust server (`luban_server`) and a browser UI (`web/`).

## Run (recommended)

1. Run:
   - `just web run`
2. Open:
   - `http://127.0.0.1:8421/`

`just web run` builds `web/` (prefers `pnpm`, falls back to `npm`) and starts `luban_server`, which serves
the built assets from `/`.

## Run (Tauri shell)

1. Run:
   - `just app run`

The Tauri app starts the same local `luban_server` in-process and loads it in a WebView.

## Run (development, hot reload)

1. Start the Rust server:
   - `just run-server`
2. Start the web dev server:
   - `cd web && pnpm install && pnpm dev`
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

## Notes

- The server binds to loopback only (localhost).
- Override bind addr/port via `LUBAN_SERVER_ADDR` (e.g. `127.0.0.1:8422`).
- UI-only state (draft/scroll/layout) lives in browser `localStorage`.
