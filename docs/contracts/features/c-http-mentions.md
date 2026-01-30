# C-HTTP-MENTIONS

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/mentions`

## Purpose

Search mention candidates (files/folders) for the chat composer.

## Query

- `q`: string (required, non-empty after trimming)

## Response

- `200 OK`
- JSON body: `MentionItemSnapshot[]`

## Web usage

- `web/lib/luban-http.ts` `fetchMentionItems({ workspaceId: workdirId, query })`
