# C-HTTP-CONTEXT

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/context`

## Purpose

Return the current workspace context attachments used by the chat composer.

## Response

- `200 OK`
- JSON body: `ContextSnapshot`

## Web usage

- `web/lib/luban-http.ts` `fetchContext(workdirId)`
