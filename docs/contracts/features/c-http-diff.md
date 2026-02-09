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

## Invariants

- Runtime-internal task document files under `.luban/` are excluded from this surface.
- This endpoint represents source diff for code/worktree files, not task document edits.

## Web usage

- `web/lib/luban-http.ts` `fetchWorkspaceDiff(workdirId)`
