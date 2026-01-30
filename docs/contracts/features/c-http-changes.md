# C-HTTP-CHANGES

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/changes`

## Purpose

Return workspace VCS summary (status/changed files) for UI panels.

## Response

- `200 OK`
- JSON body: `WorkspaceChangesSnapshot`

## Web usage

- `web/lib/luban-http.ts` `fetchWorkspaceChanges(workdirId)`
