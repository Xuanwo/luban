# C-HTTP-NEW-TASK-DRAFT

Status: Draft
Verification: Mock=yes, Provider=yes, CI=yes

## Surface

- Methods: `PUT`, `DELETE`
- Path: `/api/new_task/drafts/{draft_id}`

## Purpose

Update or delete an existing New Task draft.

## Request

### `PUT`

- JSON body:
  - `text: string` (required, non-empty)
  - `project_id: string | null`
  - `workdir_id: number | null`

## Response

### `PUT`

- `200 OK`
- JSON body: `NewTaskDraftSnapshot`

### `DELETE`

- `204 No Content`

## Web usage

- `web/lib/luban-http.ts`: `updateNewTaskDraft`, `deleteNewTaskDraft`

