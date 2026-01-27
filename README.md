# Luban

Luban is a localhost-only AI code editor built as a browser UI served by a local Rust server, with
an optional Tauri desktop shell.

## Quickstart

### Run (recommended, desktop app)

```bash
just app run
```

The Tauri app starts the local server in-process and loads the UI in a WebView.

### Run (browser, zero-config launcher)

```bash
cargo run -p luban_cli -- ui
```

This starts the local server on a random available port, prints a local URL, and opens it in your
browser.

### Run (browser, same-origin UI + APIs)

```bash
just web run
# or: just run
```

Open `http://127.0.0.1:8421/`.

### Run (mock mode, UI-first preview)

Mock mode runs the UI without starting the Rust server and is the preferred workflow for fast
interaction iteration.

```bash
just web dev-mock
```

Open `http://localhost:3000/`.

Mock mode is controlled by `NEXT_PUBLIC_LUBAN_MODE=mock`. Web/server integration is governed by
consumer-driven contracts under `docs/contracts/`.

## Downloads

### macOS prebuilt

Prebuilt macOS artifacts are published under `https://releases.luban.dev/`.

- Installer (recommended for first install): `https://releases.luban.dev/Luban_latest_darwin-universal.dmg`
  - Also available as per-platform aliases: `https://releases.luban.dev/Luban_latest_<platform>.dmg`
- Latest manifest (recommended): `https://releases.luban.dev/latest.json`
  - It contains the exact per-platform archive URL.
- Archive naming convention:
  - `https://releases.luban.dev/<version>/Luban_<version>_<platform>.app.tar.gz`
  - `<platform>` is one of: `darwin-aarch64`, `darwin-x86_64`, `darwin-universal`

For manual install, extract the archive and move `Luban.app` into `/Applications`.

## Development

This project uses `just` to manage common dev commands:

```bash
just -l
```

### Workflows

- Desktop app (Tauri): `just app run`
- Build desktop app (Tauri): `just build` (or `just app build`)
- Web + server (single process, same-origin `/api/*`): `just web run`
- Server only: `just run-server`
- Build server (no web assets): `just build-server`
- Web dev server (hot reload): `just web dev`
  - Note: the Next.js dev server does not proxy `/api/*` by default; for end-to-end testing against
    real APIs, prefer `just web run`.
- Build web assets: `just web build`

### Checks

```bash
just fmt && just lint && just test
```

Full CI-equivalent flow:

```bash
just ci
```

UI E2E (Playwright):

```bash
just test-ui
# or: just test-ui-headed
```

## Prerequisites

- Rust toolchain (edition 2024) + `cargo`
- `just`
- Node.js + `pnpm` (required for `web/`)
- (Optional) Playwright browsers: `cd web && pnpm exec playwright install`
- (Optional) Tauri prerequisites (platform-specific)

## Architecture (high level)

- Local Rust server: `crates/luban_server` (binds to loopback by default)
- Browser UI: `web/` (served from the same origin)
- Primary interaction channel: WebSocket `/api/events` (action-driven protocol)
- Terminal: PTY over WebSocket `/api/pty/*`

See `docs/decisions.md` and `docs/cs-web-architecture.md`.

## Contracts (web <-> server)

When you change any `/api/*` HTTP route, WebSocket path, or message schema, update:

- `docs/contracts/features/*`
- `docs/contracts/progress.md`

See `docs/contracts/README.md`.

## Repository layout

- `crates/luban_cli/`: `luban` CLI entrypoint (`luban ui`)
- `crates/luban_domain/`: pure state + reducers (most regressions should be captured here)
- `crates/luban_server/`: HTTP + WebSocket server, serves `web/`
- `crates/luban_tauri/`: Tauri desktop wrapper
- `web/`: browser UI (Next.js) + Playwright tests
- `docs/`: architecture and workflow documentation
- `postmortem/`: incident writeups and action items
- `dev/`: packaging and release tooling

## Configuration

Common environment variables:

- `LUBAN_SERVER_ADDR`: override bind addr/port (default: `127.0.0.1:8421`)
- `LUBAN_CODEX_BIN`: absolute path to the `codex` CLI binary
- `LUBAN_CLAUDE_BIN`: absolute path to the `claude` (Claude Code) CLI binary
- `LUBAN_CLAUDE_ROOT`: override Claude config root (default: `$HOME/.claude`)
- `LUBAN_AGENT_RUNNER`: agent runner override (`codex` / `amp` / `claude`)

## Troubleshooting

- `pnpm not found`: install `pnpm` and rerun `just web ...`
- Port already in use: set `LUBAN_SERVER_ADDR=127.0.0.1:8422` (or any free port)
- `codex` not found: install Codex CLI or set `LUBAN_CODEX_BIN`
- `claude` not found: install Claude Code or set `LUBAN_CLAUDE_BIN`

## More docs

Start at `docs/README.md`.

### Codex CLI

The agent chat panel streams events from the Codex CLI. The executable is discovered via `PATH` by
default; set `LUBAN_CODEX_BIN` to override the absolute path.
