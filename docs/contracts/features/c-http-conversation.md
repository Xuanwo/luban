# C-HTTP-CONVERSATION

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/workspaces/{workspace_id}/conversations/{thread_id}`

## Purpose

Paginated read for a single conversation thread.

## Parameters

- `workspace_id`: integer path parameter
- `thread_id`: integer path parameter

## Query

- `limit`: integer (default is controlled by the web UI)
- `before`: integer (optional pagination cursor)

## Response

- `200 OK`
- JSON body: `ConversationSnapshot`

### Run config fields

The response includes the effective per-thread run configuration used by the next agent turn:

- `snapshot.agent_runner`: `AgentRunnerKind` (`codex` / `amp` / `claude`)
- `snapshot.agent_model_id`: codex model id string (kept per-thread)
- `snapshot.thinking_effort`: codex thinking effort (kept per-thread)
- `snapshot.amp_mode`: optional string (only meaningful when `agent_runner` is `amp`)

## Invariants

- Pagination must be stable (no duplicates across pages for the same cursor).
- Entries must remain in a deterministic order.

## Web usage

- `web/lib/luban-http.ts` `fetchConversation(workspaceId, threadId, { before?, limit? })`
