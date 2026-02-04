# C-HTTP-NEW-TASK-DRAFTS

Status: Draft
Verification: Mock=yes, Provider=yes, CI=yes

## Surface

- Methods: `GET`, `POST`
- Path: `/api/new_task/drafts`

## Purpose

List and create persisted drafts for the New Task composer.

## Request

### `POST`

- JSON body:
  - `text: string` (required, non-empty)
  - `project_id: string | null`
  - `workdir_id: number | null`

## Response

### `GET`

- `200 OK`
- JSON body: `NewTaskDraftsSnapshot`

### `POST`

- `200 OK`
- JSON body: `NewTaskDraftSnapshot`

## Web usage

- `web/lib/luban-http.ts`: `fetchNewTaskDrafts`, `createNewTaskDraft`

