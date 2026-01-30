# C-HTTP-CONTEXT-DELETE

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `DELETE`
- Path: `/api/workdirs/{workdir_id}/context/{context_id}`

## Purpose

Remove a context item from a workspace.

## Response

- `204 No Content` on success

## Web usage

- `web/lib/luban-http.ts` `deleteContextItem(workdirId, contextId)`
