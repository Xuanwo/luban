# Task and Turn Status

This document defines how Luban models task lifecycle state (`TaskStatus`) and agent execution state (`TurnStatus`).

## Core model

- A **Task** is a conversation thread (`workdir_id` + `task_id`).
- A **Turn** is a single agent execution triggered by a user message within a task.
- A task can have many turns.
- A turn always belongs to exactly one task.

## Task status

`TaskStatus` is an explicit, user-facing lifecycle stage. It must not encode runtime execution details.

Enum values:

- `backlog`: captured but not ready to start
- `todo`: ready to start
- `in_progress`: actively being worked on
- `in_review`: awaiting verification/acceptance
- `done`: accepted and closed
- `canceled`: stopped and closed

Notes:

- Archive is intentionally not modeled as a task status.
- Task status changes are triggered by explicit user actions (including sending a message, which is treated as an explicit start-work action).

## Turn status and last turn result

`TurnStatus` is an execution-time state derived from the conversation runtime/queue state.

Enum values:

- `idle`: no active turn and no queued work
- `running`: an active turn is executing
- `awaiting`: queued prompts exist and the queue is not paused
- `paused`: queued prompts exist and the queue is paused

`TurnResult` is the terminal outcome of the most recent finished turn:

- `completed`
- `failed`

### Derivation (scheme A)

We do not persist a separate "turn table". Instead, we derive:

- `turn_status` from conversation run timing and queue state (`run_started_at_unix_ms`, `run_finished_at_unix_ms`, `queue_paused`, queued prompt count).
- `last_turn_result` from the most recent terminal turn marker in `conversation_entries`:
  - `turn_duration` => `completed`
  - `turn_error` or `turn_canceled` => `failed`

## Storage

- `TaskStatus` is persisted in the `conversations.task_status` column.
- `TurnStatus` and `TurnResult` are derived for thread list responses.

## API surface

The following snapshots expose task/turn state:

- `ConversationSnapshot.task_status`
- `ThreadMeta.task_status`, `ThreadMeta.turn_status`, `ThreadMeta.last_turn_result`
- `TaskSummarySnapshot.task_status`, `TaskSummarySnapshot.turn_status`, `TaskSummarySnapshot.last_turn_result`

