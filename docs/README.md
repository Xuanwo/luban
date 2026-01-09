# Documentation Index

This directory documents the current, implemented architecture of Luban.

## Start here

- `docs/cs-web-quickstart.md`: how to run the localhost server + web UI.
- `docs/cs-web-architecture.md`: system architecture and API contract.
- `docs/ui-design-parity.md`: design parity requirements (`design/` submodule is the source of truth).

## Deep dives

- `docs/decisions.md`: current technical consensus (implemented decisions + explicit non-goals).
- `docs/persistence.md`: what is durable (SQLite + workspace context) vs UI-only (`localStorage`).
- `docs/terminal.md`: web terminal model (`ghostty-web` + PTY over WebSocket).
- `docs/workspace-thread-tabs.md`: conversation threads and tab strip behavior.
- `docs/codex-cli.md`: how Codex CLI streaming maps to conversation entries.
- `docs/ui-testing.md`: UI regression testing guidance (Playwright-first).

