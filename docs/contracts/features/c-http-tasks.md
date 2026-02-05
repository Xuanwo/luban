# C-HTTP-TASKS

Status: Draft
Verification: Mock=yes, Provider=yes, CI=yes

## Surface

- Method: `GET`
- Path: `/api/tasks`

## Purpose

Return an aggregated task list across workdirs (optionally scoped to a single project).

This endpoint exists to support task-first UI surfaces (inbox, global lists) without requiring the
client to iterate all workdirs and fan out requests.

## Query (optional)

- `project_id`: `ProjectId` (string). When provided, only tasks for that project are returned.
- `workdir_status`: `active` (default) / `archived` / `all`. Controls which workdir statuses are included in the response.
- `task_status`: Comma-separated `TaskStatus` values. When provided, only tasks whose `task_status` is in the set are returned. Use `all` or omit to disable filtering. Legacy aliases are accepted (`in_progress` -> `iterating`, `in_review` -> `validating`).

## Response

- `200 OK`
- JSON body: `TasksSnapshot`

## Schema notes

- `TasksSnapshot.tasks[]` items are `TaskSummarySnapshot`.
- `TaskSummarySnapshot.created_at_unix_seconds` is the stable task creation timestamp.
- `TaskSummarySnapshot.updated_at_unix_seconds` is updated when the task timeline changes (for example user/agent messages, status changes).
- `TaskSummarySnapshot.is_starred` indicates whether the user has starred the task.
- `TaskSummarySnapshot.task_status` is an explicit lifecycle stage (`TaskStatus`).
- `TaskSummarySnapshot.turn_status` and `TaskSummarySnapshot.last_turn_result` provide derived turn-level status (see `docs/task-and-turn-status.md`).
- `TaskStatus` values: `backlog` / `todo` / `iterating` / `validating` / `done` / `canceled` (legacy aliases: `in_progress` -> `iterating`, `in_review` -> `validating`).

## Invariants

- The response must be deserializable into `TasksSnapshot`.
- Task ordering should be stable for a given snapshot revision.

## Web usage

- `web/lib/luban-http.ts` `fetchTasks({ projectId?, workdirStatus? })`
