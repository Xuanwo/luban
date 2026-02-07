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
| C-HTTP-HEALTH | `GET /api/health` | `crates/luban_server/src/server.rs:health` | n/a | Draft | n/a | ✅ | ✅ |
| C-HTTP-APP | `GET /api/app` | `crates/luban_server/src/server.rs:get_app` | `web/lib/luban-http.ts:fetchApp` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-PROJECTS-AVATAR | `GET /api/projects/avatar` | `crates/luban_server/src/server.rs:get_project_avatar` | `web/components/luban-sidebar.tsx` | Draft | n/a | ✅ | ✅ |
| C-HTTP-CODEX-PROMPTS | `GET /api/codex/prompts` | `crates/luban_server/src/server.rs:get_codex_prompts` | `web/lib/luban-http.ts:fetchCodexCustomPrompts` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-WORKDIR-TASKS | `GET /api/workdirs/{workdir_id}/tasks` | `crates/luban_server/src/server.rs:get_threads` | `web/lib/luban-http.ts:fetchThreads` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-TASKS | `GET /api/tasks` | `crates/luban_server/src/server.rs:get_tasks` | `web/lib/luban-http.ts:fetchTasks` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-NEW-TASK-DRAFTS | `GET /api/new_task/drafts` | `crates/luban_server/src/server.rs:list_new_task_drafts` | `web/lib/luban-http.ts:fetchNewTaskDrafts` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-NEW-TASK-DRAFT | `DELETE /api/new_task/drafts/{draft_id}` | `crates/luban_server/src/server.rs:delete_new_task_draft` | `web/lib/luban-http.ts:deleteNewTaskDraft` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-NEW-TASK-STASH | `GET /api/new_task/stash` | `crates/luban_server/src/server.rs:get_new_task_stash` | `web/lib/luban-http.ts:fetchNewTaskStash` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-CONVERSATION | `GET /api/workdirs/{workdir_id}/conversations/{task_id}` | `crates/luban_server/src/server.rs:get_conversation` | `web/lib/luban-http.ts:fetchConversation` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-CHANGES | `GET /api/workdirs/{workdir_id}/changes` | `crates/luban_server/src/server.rs:get_changes` | n/a (right sidebar removed) | Draft | ✅ | ✅ | ✅ |
| C-HTTP-DIFF | `GET /api/workdirs/{workdir_id}/diff` | `crates/luban_server/src/server.rs:get_diff` | `web/lib/luban-http.ts:fetchWorkspaceDiff` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-CONTEXT | `GET /api/workdirs/{workdir_id}/context` | `crates/luban_server/src/server.rs:get_context` | n/a (web context UI removed) | Draft | n/a | ✅ | ✅ |
| C-HTTP-CONTEXT-DELETE | `DELETE /api/workdirs/{workdir_id}/context/{context_id}` | `crates/luban_server/src/server.rs:delete_context_item` | n/a (web context UI removed) | Draft | n/a | ✅ | ✅ |
| C-HTTP-MENTIONS | `GET /api/workdirs/{workdir_id}/mentions` | `crates/luban_server/src/server.rs:get_workspace_mentions` | `web/lib/luban-http.ts:fetchMentionItems` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-ATTACHMENTS-UPLOAD | `POST /api/workdirs/{workdir_id}/attachments` | `crates/luban_server/src/server.rs:upload_attachment` | `web/lib/luban-http.ts:uploadAttachment` | Draft | ✅ | ✅ | ✅ |
| C-HTTP-ATTACHMENTS-DOWNLOAD | `GET /api/workdirs/{workdir_id}/attachments/{attachment_id}` | `crates/luban_server/src/server.rs:download_attachment` | `web/components/*` (direct link usage) | Draft | ✅ | ✅ | ✅ |

## WebSocket endpoints

| Contract | Surface | Server handler | Web entrypoint | Status | Mock | Provider | CI |
| --- | --- | --- | --- | --- | --- | --- | --- |
| C-WS-EVENTS | `WS /api/events` | `crates/luban_server/src/server.rs:ws_events` | `web/lib/luban-transport.ts:useLubanTransport` | Draft | ✅ | ✅ | ✅ |
| C-WS-PTY | `WS /api/pty/{workdir_id}/{task_id}` | `crates/luban_server/src/server.rs:ws_pty` | `web/components/pty-terminal.tsx` | Draft | ✅ | ✅ | ✅ |

## Notes

- `C-WS-EVENTS`: `ClientAction::ChatRunnerChanged` and `ClientAction::ChatAmpModeChanged` are implemented in mock + provider and enforced in CI via `crates/luban_server/src/engine.rs` unit tests (see `agent_turn_uses_pinned_chat_runner_and_amp_mode`).
- `C-WS-EVENTS`: `ClientAction::TaskExecute` accepts optional `attachments` and forwards them to the initial `SendAgentMessage` when `mode=start` (enforced via `task_execute_start_passes_attachments_to_agent_turn`).
- `C-WS-EVENTS`: `ClientAction::SidebarWorktreeOrderChanged` was removed (task-first UI no longer persists worktree ordering).
- `C-WS-EVENTS`: `ClientAction::TaskPreview` and `ServerEvent::TaskPreviewReady` were removed; task execution is prompt-based.
- `C-WS-EVENTS`: `ClientAction::TaskStarSet` is implemented in mock + provider and verified in CI via `crates/luban_server/tests/contracts_http.rs` (roundtrip: WS toggle then `GET /api/tasks`).
- `C-WS-EVENTS`: `ClientAction::TaskStatusSet` updates per-task `TaskStatus` and is implemented in mock + provider.
- `C-WS-EVENTS`: Telegram integration actions (`TelegramBotTokenSet` / `TelegramBotTokenClear` / `TelegramPairStart` / `TelegramUnpair`) are implemented in mock + provider and verified in CI via `crates/luban_server/tests/contracts_ws_events_telegram.rs`.
- `C-WS-EVENTS`: Telegram progress relay reuses a single per-task progress message via `editMessageText` and treats `message is not modified` as idempotent success (see `docs/contracts/features/c-ws-events.md`, "Telegram progress relay behavior").
- `C-WS-EVENTS`: Telegram passive conversation forwarding also keeps a single per-task relay message (after first send) and updates it via `editMessageText` on subsequent new updates.
- `C-WS-EVENTS`: `ServerEvent::TaskSummariesChanged` pushes per-workdir `TaskSummarySnapshot[]` updates for task-first UI surfaces (inbox, global task lists).
- `C-HTTP-CONVERSATION`: `ConversationSnapshot` includes per-thread run config (`agent_runner` / `agent_model_id` / `thinking_effort` / `amp_mode`).
- `C-HTTP-CONVERSATION`: `ConversationSnapshot.task_status` exposes the per-task lifecycle stage.
- `C-HTTP-CONVERSATION`: `ConversationSnapshot.entries` is a timeline of `ConversationEntry` values tagged by `type` (`system_event` / `user_event` / `agent_event`). Each entry includes a stable `entry_id` and `created_at_unix_ms`, and streaming/tool updates are appended as additional `agent_event` entries (clients may fold by `AgentEvent.id` if desired).
- `C-HTTP-CONVERSATION`: `ConversationEntry.type=system_event` may include `event_type=task_status_suggestion` to recommend a status change; clients apply via `ClientAction::TaskStatusSet`.
- `C-HTTP-CONVERSATION`: `ConversationEntry.type=system_event` may include `event_type=task_archived` after provider cleanup for a closed task.
- `C-HTTP-CONVERSATION`: `ConversationEntry.type=user_event` supports `event.type=message`, `terminal_command_started`, and `terminal_command_finished`.
- `C-HTTP-CONVERSATION`: `ConversationSnapshot.title` matches `ThreadMeta.title` and may be updated after the first user message.
- `C-HTTP-TASKS`: `TaskSummarySnapshot` includes `is_starred` for rendering Favorites and in-view star toggles.
- `C-HTTP-TASKS`: `GET /api/tasks` supports `workdir_status=active|archived|all` to control whether archived workdirs are included.
- `C-HTTP-TASKS`: `GET /api/tasks` supports `task_status=...` (comma-separated) to filter by `TaskStatus` (use `all` or omit to disable filtering).
- `C-HTTP-TASKS` / `C-HTTP-WORKDIR-TASKS`: thread metadata includes `created_at_unix_seconds` for stable creation-time sorting.
- `C-HTTP-TASKS` / `C-HTTP-WORKDIR-TASKS`: thread metadata includes `task_status`, `turn_status`, and `last_turn_result` (see `docs/task-and-turn-status.md`).
- `C-HTTP-WORKDIR-TASKS`: a workdir may start with zero tasks, and providers must not seed placeholder tasks on read; when legacy conversation storage exists, providers may perform a one-time migration to make tasks visible.
- `TaskStatus` uses `iterating` / `validating` (legacy aliases: `in_progress` / `in_review`).

## Feature contracts

- `docs/contracts/features/c-auth-single-user.md`
- `docs/contracts/features/c-http-health.md`
- `docs/contracts/features/c-http-app.md`
- `docs/contracts/features/c-http-projects-avatar.md`
- `docs/contracts/features/c-http-codex-prompts.md`
- `docs/contracts/features/c-http-workdir-tasks.md`
- `docs/contracts/features/c-http-tasks.md`
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
