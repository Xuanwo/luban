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
- `iterating`: active developer ↔ agent iteration (implementation and local feedback loops)
- `validating`: external verification (review, CI, QA, or any acceptance gate outside the developer↔agent loop)
- `done`: accepted and closed
- `canceled`: stopped and closed

Notes:

- Archive is intentionally not modeled as a task status.
- Closed tasks (`done` / `canceled`) are treated as archived for UX and cleanup purposes:
  - Clients must treat them as read-only (no new turns/messages).
  - Providers may cleanup the workspace worktree/branch asynchronously and append a `system_event`
    marker (see `ConversationSystemEvent::TaskArchived`).
- Task status changes are triggered by explicit user actions (including sending a message, which is treated as an explicit start-work action).
- Legacy values `in_progress` and `in_review` may exist in persisted data and are treated as aliases for `iterating` and `validating` respectively.

### Stage semantics (what each stage means)

- `backlog`: the task exists, but work is intentionally not started (missing requirements, lower priority, or waiting on prerequisites).
- `todo`: the task is ready to start (clear enough to begin; still no active iteration yet).
- `iterating`: the task is in the active working loop (developer and agent are producing/adjusting changes, running local checks, and refining until it is ready for external validation).
- `validating`: the task is waiting on acceptance signals outside the iteration loop (code review, CI, external testing, stakeholder approval, etc.). Changes may still happen, but the default expectation is “validate what was produced”.
- `done`: the task is accepted and no further work is expected.
- `canceled`: the task is intentionally stopped and no further work is expected.

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
