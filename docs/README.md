# Documentation Index

This directory documents the current, implemented architecture of Luban.

## Start here

- `docs/cs-web-quickstart.md`: how to run the localhost server + web UI.
- `docs/cs-web-architecture.md`: system architecture and API contract.
- `docs/contracts/README.md`: consumer-driven contracts and progress tracking.

## In progress

- `docs/migrations/2026-01-22-web-mock-mode-contracts.md`: migration to `web` mock mode as the only
  high-fidelity UI plus contract-driven integration.

## Deep dives

- `docs/decisions.md`: current technical consensus (implemented decisions + explicit non-goals).
- `docs/persistence.md`: what is durable (SQLite + workspace context) vs UI-only (`localStorage`).
- `docs/terminal.md`: web terminal model (`ghostty-web` + PTY over WebSocket).
- `docs/workspace-thread-tabs.md`: conversation threads and tab strip behavior.
- `docs/codex-cli.md`: how Codex CLI streaming maps to conversation entries.
- `docs/claude-code.md`: how Claude Code streaming maps to conversation entries.
- `docs/agent-runner-integration.md`: playbook for adding new agent runners (Amp learnings).
- `docs/ui-testing.md`: UI regression testing guidance (agent-browser).
