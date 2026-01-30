# C-HTTP-WORKDIR-TASKS

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/tasks`

## Purpose

List task metadata and tab state for a workdir.

Task titles are user-facing and should be short:

- Default title is `"Task {task_id}"`.
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

## Web usage

- `web/lib/luban-http.ts` `fetchThreads(workdirId)`
