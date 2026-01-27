# Contract Progress Tracker

This document tracks all current web/server interaction surfaces and their contract status.

## Update rules

- If you change `crates/luban_server/src/server.rs` routes, update this file in the same change.
- If you add/change `ClientAction` / `ServerEvent` behavior, update `docs/contracts/features/c-ws-events.md`.
- If you add/change `AgentRunnerKind` variants, update `docs/contracts/features/c-ws-events.md`.
- If you add/change `SystemTaskKind` variants, update:
  - `docs/contracts/features/c-http-app.md`
  - `docs/contracts/features/c-ws-events.md`

## Local checks

- `just test` (includes a contract coverage test for server routes)

Legend:

- Status: `Draft` (in flux) / `Stable` (expected to be backward-compatible)
- Verification:
  - Mock: implemented in web mock mode
  - Provider: implemented in the Rust server
  - CI: enforced by provider contract tests

## HTTP endpoints

| Contract | Surface | Server handler | Web entrypoint | Status | Mock | Provider | CI |
| --- | --- | --- | --- | --- | --- | --- | --- |
| C-HTTP-HEALTH | `GET /api/health` | `crates/luban_server/src/server.rs:health` | n/a | Draft | n/a | ✅ | ⬜ |
| C-HTTP-APP | `GET /api/app` | `crates/luban_server/src/server.rs:get_app` | `web/lib/luban-http.ts:fetchApp` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-CODEX-PROMPTS | `GET /api/codex/prompts` | `crates/luban_server/src/server.rs:get_codex_prompts` | `web/lib/luban-http.ts:fetchCodexCustomPrompts` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-THREADS | `GET /api/workspaces/{workspace_id}/threads` | `crates/luban_server/src/server.rs:get_threads` | `web/lib/luban-http.ts:fetchThreads` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-CONVERSATION | `GET /api/workspaces/{workspace_id}/conversations/{thread_id}` | `crates/luban_server/src/server.rs:get_conversation` | `web/lib/luban-http.ts:fetchConversation` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-CHANGES | `GET /api/workspaces/{workspace_id}/changes` | `crates/luban_server/src/server.rs:get_changes` | `web/lib/luban-http.ts:fetchWorkspaceChanges` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-DIFF | `GET /api/workspaces/{workspace_id}/diff` | `crates/luban_server/src/server.rs:get_diff` | `web/lib/luban-http.ts:fetchWorkspaceDiff` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-CONTEXT | `GET /api/workspaces/{workspace_id}/context` | `crates/luban_server/src/server.rs:get_context` | `web/lib/luban-http.ts:fetchContext` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-CONTEXT-DELETE | `DELETE /api/workspaces/{workspace_id}/context/{context_id}` | `crates/luban_server/src/server.rs:delete_context_item` | `web/lib/luban-http.ts:deleteContextItem` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-MENTIONS | `GET /api/workspaces/{workspace_id}/mentions` | `crates/luban_server/src/server.rs:get_workspace_mentions` | `web/lib/luban-http.ts:fetchMentionItems` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-ATTACHMENTS-UPLOAD | `POST /api/workspaces/{workspace_id}/attachments` | `crates/luban_server/src/server.rs:upload_attachment` | `web/lib/luban-http.ts:uploadAttachment` | Draft | ✅ | ✅ | ⬜ |
| C-HTTP-ATTACHMENTS-DOWNLOAD | `GET /api/workspaces/{workspace_id}/attachments/{attachment_id}` | `crates/luban_server/src/server.rs:download_attachment` | `web/components/*` (direct link usage) | Draft | ✅ | ✅ | ⬜ |

## WebSocket endpoints

| Contract | Surface | Server handler | Web entrypoint | Status | Mock | Provider | CI |
| --- | --- | --- | --- | --- | --- | --- | --- |
| C-WS-EVENTS | `WS /api/events` | `crates/luban_server/src/server.rs:ws_events` | `web/lib/luban-transport.ts:useLubanTransport` | Draft | ✅ | ✅ | ⬜ |
| C-WS-PTY | `WS /api/pty/{workspace_id}/{thread_id}` | `crates/luban_server/src/server.rs:ws_pty` | `web/components/pty-terminal.tsx` | Draft | ✅ | ✅ | ✅ |

## Feature contracts

- `docs/contracts/features/c-auth-single-user.md`
- `docs/contracts/features/c-http-health.md`
- `docs/contracts/features/c-http-app.md`
- `docs/contracts/features/c-http-codex-prompts.md`
- `docs/contracts/features/c-http-threads.md`
- `docs/contracts/features/c-http-conversation.md`
- `docs/contracts/features/c-http-changes.md`
- `docs/contracts/features/c-http-diff.md`
- `docs/contracts/features/c-http-context.md`
- `docs/contracts/features/c-http-context-delete.md`
- `docs/contracts/features/c-http-mentions.md`
- `docs/contracts/features/c-http-attachments-upload.md`
- `docs/contracts/features/c-http-attachments-download.md`
- `docs/contracts/features/c-ws-events.md`
- `docs/contracts/features/c-ws-pty.md`
