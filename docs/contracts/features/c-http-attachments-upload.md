# C-HTTP-ATTACHMENTS-UPLOAD

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `POST`
- Path: `/api/workdirs/{workdir_id}/attachments`

## Purpose

Upload a single attachment to be referenced by chat messages.

## Request

Multipart form-data:

- `kind`: `AttachmentKind` (`image` | `text` | `file`)
- `file`: binary payload

Optional headers:

- `Idempotency-Key`: an opaque client-generated key used to deduplicate retries. If the same
  request is retried with the same key, the provider may return the original `AttachmentRef`
  instead of creating a new attachment.

## Response

- `200 OK`
- JSON body: `AttachmentRef`

## Web usage

- `web/lib/luban-http.ts` `uploadAttachment({ workspaceId: workdirId, file, kind })`
