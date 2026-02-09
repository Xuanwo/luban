# C-HTTP-TASK-DOCUMENTS

Status: Draft
Verification: Mock=yes, Provider=yes, CI=yes

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/tasks/{task_id}/documents`
- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/tasks/{task_id}/documents/{kind}`
- Method: `PUT`
- Path: `/api/workdirs/{workdir_id}/tasks/{task_id}/documents/{kind}`

## Purpose

Expose task-scoped document files for review and editing:

- `TASK.md`: current task/runtime status
- `PLAN.md`: execution plan
- `MEMORY.md`: durable task memory

## Storage model

- Primary source of truth: global Luban storage (`$LUBAN_ROOT`), task-native layout.
- Base path: `tasks/v1/tasks/{task_ulid}/`.
- Reverse index path: `tasks/v1/index/workdir_task/{workdir_id}-{task_id}.json`.
- File names: `TASK.md`, `PLAN.md`, `MEMORY.md`.
- Identity metadata: `task.json` in each task directory, linking `{task_ulid}` to `{workdir_id, task_id}`.
- Provider keeps an in-memory cache for change detection only.
- Runtime change detection is driven by filesystem notifications on `tasks/v1/tasks/**`.

## GET behavior

- Read-only path: does not create identity metadata or document files.
- Returns exactly three documents in semantic order: `task`, `plan`, `memory`.
- If identity/files are missing, provider returns empty content and virtual `rel_path`.
- Response body: `TaskDocumentsSnapshot`.

## GET by kind behavior

- Read-only path: does not create identity metadata or document files.
- Returns only the requested kind (`task` / `plan` / `memory`).
- If identity/files are missing, provider returns empty content and virtual `rel_path`.
- Response body: `TaskDocumentSnapshot`.

## PUT behavior

- `kind` path parameter must be one of: `task`, `plan`, `memory`.
- Request body: `{ "content": string }`.
- Lazily creates task identity/directory on first write, then writes content atomically.
- First-write identity uses deterministic fallback task id (`task-{workdir_id}-{task_id}`) to align
  with agent-created files before provider writes.
- Response body: `TaskDocumentSnapshot`.

## Invariants

- `TaskDocumentSnapshot.rel_path` must be a `tasks/v1/tasks/{task_ulid}/...` relative path.
- `content_hash` must change when content changes.
- `byte_len` must match returned content length in bytes.
- Task document filesystem updates are independent from code diff/change surfaces (`C-HTTP-CHANGES`, `C-HTTP-DIFF`).

## Web usage

- `web/lib/luban-http.ts`
  - `fetchTaskDocuments(workdirId, taskId)`
  - `fetchTaskDocument({ workspaceId, taskId, kind })`
  - `updateTaskDocument({ workspaceId, taskId, kind, content })`
