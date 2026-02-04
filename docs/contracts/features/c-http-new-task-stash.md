# C-HTTP-NEW-TASK-STASH

Status: Draft
Verification: Mock=yes, Provider=yes, CI=yes

## Surface

- Methods: `GET`, `PUT`, `DELETE`
- Path: `/api/new_task/stash`

## Purpose

Persist the implicit New Task composer stash used for focus-loss/Esc dismissal recovery.

## Request

### `PUT`

- JSON body:
  - `text: string` (required, non-empty)
  - `project_id: string | null`
  - `workdir_id: number | null`
  - `editing_draft_id: string | null`

## Response

### `GET`

- `200 OK`
- JSON body: `NewTaskStashResponse` (nullable `stash`)

### `PUT`

- `204 No Content`

### `DELETE`

- `204 No Content`

## Web usage

- `web/lib/luban-http.ts`: `fetchNewTaskStash`, `saveNewTaskStash`, `clearNewTaskStash`

