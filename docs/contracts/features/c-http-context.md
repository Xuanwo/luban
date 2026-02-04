# C-HTTP-CONTEXT

Status: Draft
Verification: Mock=no, Provider=yes, CI=yes

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/context`

## Purpose

Return the current workspace context attachments.

## Response

- `200 OK`
- JSON body: `ContextSnapshot`

## Web usage

- n/a (web context UI removed)
