# C-HTTP-TASKS

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/tasks`

## Purpose

Return an aggregated task list across workdirs (optionally scoped to a single project).

This endpoint exists to support task-first UI surfaces (inbox, global lists) without requiring the
client to iterate all workdirs and fan out requests.

## Query (optional)

- `project_id`: `ProjectId` (string). When provided, only tasks for that project are returned.

## Response

- `200 OK`
- JSON body: `TasksSnapshot`

## Invariants

- The response must be deserializable into `TasksSnapshot`.
- Task ordering should be stable for a given snapshot revision.

## Web usage

- `web/lib/luban-http.ts` `fetchTasks({ projectId? })`

