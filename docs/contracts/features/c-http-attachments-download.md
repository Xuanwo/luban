# C-HTTP-ATTACHMENTS-DOWNLOAD

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/workdirs/{workdir_id}/attachments/{attachment_id}`

## Purpose

Download a previously uploaded attachment.

## Query (optional)

## Query

- `ext`: string (required; file extension used by the UI and provider to resolve and type the blob)

## Response

- `200 OK`
- Body: attachment bytes

## Notes

- The UI may use direct links or `fetch` depending on the rendering path.
- In mock mode, attachments are resolved to `data:` / `blob:` URLs and do not hit this HTTP endpoint.
