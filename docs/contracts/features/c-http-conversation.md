# C-HTTP-CONVERSATION

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/conversations/{task_id}`

## Purpose

Paginated read for a single conversation thread.

## Parameters

- `workdir_id`: integer path parameter
- `task_id`: integer path parameter

## Query

- `limit`: integer (default is controlled by the web UI)
- `before`: integer (optional pagination cursor)

## Response

- `200 OK`
- JSON body: `ConversationSnapshot`

### Events

`snapshot.entries` is an ordered timeline. Each element is a `ConversationEntry` tagged by `type` and includes a stable `entry_id` (unique per entry):

- `system_event`: provider-appended lifecycle transitions (task created, status changes, etc.)
- `user_event`: user-originated events (for example: messages, terminal commands)
- `agent_event`: agent-originated events (messages, tool steps, turn lifecycle events)

Streaming/tool updates are represented as additional appended `agent_event` entries; clients may fold by `AgentEvent.id` if desired.

### System events

`snapshot.entries` may include `type=system_event` entries. These are appended by the provider for
user-visible lifecycle transitions (for example: task creation, task status changes).

The event payload is structured:

- `type`: `system_event`
- `entry_id`: stable string identifier (unique within the conversation)
- `created_at_unix_ms`: millisecond timestamp
- `event.event_type`: `task_created` | `task_archived` | `task_status_changed` | `task_status_suggestion`
  - `task_archived` indicates the provider has completed archival cleanup for a closed task (for
    example: removing the worktree and deleting the local `luban/*` branch). Clients should treat
    archived tasks as read-only.

For `event.event_type=task_status_suggestion`:

- `event.from`: `TaskStatus`
- `event.to`: `TaskStatus`
- `event.title`: string (may be empty)
- `event.explanation_markdown`: string (may be empty; markdown is allowed)

Semantics:

- The provider emits this when it has analyzed the conversation progress and recommends updating the explicit `snapshot.task_status`.
- The provider does not apply the change automatically; the client may apply it via `ClientAction::TaskStatusSet`.

### User events

User events are structured:

- `type`: `user_event`
- `entry_id`: stable string identifier (unique within the conversation)
- `created_at_unix_ms`: millisecond timestamp
- `event.type`: `message` | `terminal_command_started` | `terminal_command_finished`

For `event.type=message`:

- `event.text`: string
- `event.attachments`: array of `AttachmentRef`

For `event.type=terminal_command_started`:

- `event.id`: stable identifier for folding start/finish
- `event.command`: string
- `event.reconnect`: string (see `C-WS-PTY`)

For `event.type=terminal_command_finished`:

- `event.id`: stable identifier for folding start/finish
- `event.command`: string
- `event.reconnect`: string
- `event.output_byte_len`: integer
- `event.output_base64`: base64-encoded bytes (may be empty when `output_byte_len=0`)

### Agent events

Agent events are structured:

- `type`: `agent_event`
- `entry_id`: stable string identifier (unique within the conversation)
- `created_at_unix_ms`: millisecond timestamp
- `event.type`: `message` | `item` | `turn_usage` | `turn_duration` | `turn_canceled` | `turn_error`

For `event.type=message`:

- `event.id`: stable string identifier (stable per message; multiple entries may share the same id)
- `event.text`: string

For `event.type=item`:

- `event.id`: stable string identifier (stable per tool item; multiple entries may share the same id)
- `event.kind`: `AgentItemKind`
- `event.payload`: JSON value (implementation-defined)

### Task status

- `snapshot.task_status`: explicit lifecycle stage (`TaskStatus`, see `docs/task-and-turn-status.md`)
- `TaskStatus` values: `backlog` / `todo` / `iterating` / `validating` / `done` / `canceled` (legacy aliases: `in_progress` -> `iterating`, `in_review` -> `validating`).

### Title

The response includes the user-facing thread title:

- `snapshot.title`: same semantics as `ThreadMeta.title` in `C-HTTP-WORKDIR-TASKS`.
  - Default is `"Thread {task_id}"`.
  - After the first user message, the provider may update the title (deterministic first-line derivation and/or an asynchronous AI-generated title).

### Run config fields

The response includes the effective per-thread run configuration used by the next agent turn:

- `snapshot.agent_runner`: `AgentRunnerKind` (`codex` / `amp` / `claude`)
- `snapshot.agent_model_id`: codex model id string (kept per-thread)
- `snapshot.thinking_effort`: codex thinking effort (kept per-thread)
- `snapshot.amp_mode`: optional string (only meaningful when `agent_runner` is `amp`)

## Invariants

- Pagination must be stable (no duplicates across pages for the same cursor).
- Entries must remain in a deterministic order.

## Web usage

- `web/lib/luban-http.ts` `fetchConversation(workdirId, taskId, { before?, limit? })`
