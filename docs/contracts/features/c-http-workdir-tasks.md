# C-HTTP-WORKDIR-TASKS

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/tasks`

## Purpose

List task metadata and tab state for a workdir.

Task titles are user-facing and should be short:

- Default title is `"Thread {task_id}"`.
- After the first user message, the provider may update the title based on:
  - A deterministic first-line derivation (immediate).
  - An asynchronous AI-generated title (may arrive later).
- Titles should be limited to `40` Unicode scalar values.

## Parameters

- `workdir_id`: integer path parameter

## Response

- `200 OK`
- JSON body: `ThreadsSnapshot`

## Invariants

- The response must be deserializable into `ThreadsSnapshot`.
- Task ordering must match the UI expectations documented in `docs/workspace-thread-tabs.md`.
- `ThreadsSnapshot.tasks[]` items are `ThreadMeta`.
- A workdir may have zero tasks (e.g. a newly created workdir). Providers must not implicitly create
  placeholder tasks on read-only endpoints.
- If legacy conversation storage exists for a workdir (e.g. pre-SQLite `conversation.json` /
  `events.jsonl`), the provider may perform a one-time migration when serving this endpoint, so
  tasks derived from legacy data become visible to the UI.
- `ThreadMeta.created_at_unix_seconds` is the stable task creation timestamp.
- `ThreadMeta.task_status` is the explicit lifecycle stage (`TaskStatus`).
- `ThreadMeta.turn_status` and `ThreadMeta.last_turn_result` are derived turn-level status (see `docs/task-and-turn-status.md`).
- `TaskStatus` values: `backlog` / `todo` / `iterating` / `validating` / `done` / `canceled` (legacy aliases: `in_progress` -> `iterating`, `in_review` -> `validating`).

## Web usage

- `web/lib/luban-http.ts` `fetchThreads(workdirId)`
