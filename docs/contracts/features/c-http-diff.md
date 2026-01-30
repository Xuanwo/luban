# C-HTTP-DIFF

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/diff`

## Purpose

Return a structured diff for the workspace.

## Response

- `200 OK`
- JSON body: `WorkspaceDiffSnapshot`

## Web usage

- `web/lib/luban-http.ts` `fetchWorkspaceDiff(workdirId)`
