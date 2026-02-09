# C-HTTP-CHANGES

Status: Draft
Verification: Mock=yes, Provider=yes, CI=yes

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/changes`

## Purpose

Return workspace VCS summary (status/changed files) for UI panels.

## Response

- `200 OK`
- JSON body: `WorkspaceChangesSnapshot`

## Invariants

- Runtime-internal task document files under `.luban/` are excluded from this surface.
- This endpoint represents code/worktree VCS changes, not task document edits.

## Web usage

- n/a (right sidebar removed)
