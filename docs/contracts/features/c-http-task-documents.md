# C-HTTP-TASK-DOCUMENTS

Status: Draft
Verification: Mock=yes, Provider=yes, CI=yes

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/tasks/{task_id}/documents`
- Method: `PUT`
- Path: `/api/workdirs/{workdir_id}/tasks/{task_id}/documents/{kind}`

## Purpose

Expose task-scoped document files for review and editing:

- `TASK.md`: current task/runtime status
- `PLAN.md`: execution plan
- `MEMORY.md`: durable task memory

## Storage model

- Primary source of truth: filesystem under the active worktree.
- Base path: `.luban/tasks/{task_id}/`.
- File names: `TASK.md`, `PLAN.md`, `MEMORY.md`.
- Provider keeps an in-memory cache for change detection only.
- Runtime change detection is driven by filesystem notifications on `.luban/tasks/**`.

## GET behavior

- Ensures the directory exists.
- Returns exactly three documents in semantic order: `task`, `plan`, `memory`.
- If a file is missing, provider initializes it with default template content and persists it to FS.
- Response body: `TaskDocumentsSnapshot`.

## PUT behavior

- `kind` path parameter must be one of: `task`, `plan`, `memory`.
- Request body: `{ "content": string }`.
- Writes content to the matching file atomically.
- Response body: `TaskDocumentSnapshot`.

## Invariants

- `TaskDocumentSnapshot.rel_path` must be a `.luban/tasks/{task_id}/...` relative path.
- `content_hash` must change when content changes.
- `byte_len` must match returned content length in bytes.
- Task document filesystem updates are independent from code diff/change surfaces (`C-HTTP-CHANGES`, `C-HTTP-DIFF`).

## Web usage

- `web/lib/luban-http.ts`
  - `fetchTaskDocuments(workdirId, taskId)`
  - `updateTaskDocument({ workspaceId, threadId, kind, content })`
