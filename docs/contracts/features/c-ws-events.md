# C-WS-EVENTS

Status: Draft
Verification: Mock=yes, Provider=yes, CI=yes

## Surface

- WebSocket path: `/api/events`

## Purpose

Primary action/event protocol used by the UI:

- Client sends `WsClientMessage::Hello` then `WsClientMessage::Action`.
- Server sends `WsServerMessage::Hello`, `WsServerMessage::Ack`, and `WsServerMessage::Event`.

This surface is designed to be resilient to transient network failures:

- The client may reconnect and resend `Hello` with a `last_seen_rev` cursor.
- The server may proactively send a full `AppChanged` snapshot to resynchronize state.

## Message types

See `crates/luban_api`:

- `WsClientMessage`
- `WsServerMessage`
- `ClientAction`
- `ServerEvent`

## Invariants

- Provider wire protocol invariants:
  - JSON must be tagged with `type` and must deserialize into the enums above.
  - `protocol_version` must match `PROTOCOL_VERSION`.
  - Each `Action` must eventually be followed by either:
    - `Ack` (plus any number of `Event`)
    - `Error` with the matching `request_id`

- Resync invariants:
  - The client should send `WsClientMessage::Hello { last_seen_rev: <cursor> }` on every connection.
  - If `last_seen_rev` does not match the provider's current revision, the provider may send an
    `AppChanged` snapshot to allow the client to resynchronize.
  - If the provider detects that a subscriber has lagged (dropped broadcast messages), it may send
    an `AppChanged` snapshot and continue streaming.

- Mock-mode invariant:
  - The web UI must be able to run without a real WebSocket by directly executing `ClientAction`
    against an in-process mock runtime and emitting `ServerEvent` snapshots.

## Enumerations (tracked)

These enums are part of the wire surface. Adding/removing variants must update this contract.

- `AgentRunnerKind`:
  - `codex`
  - `amp`
  - `claude`
- `SystemTaskKind`:
  - `infer-type`
  - `rename-branch`
  - `auto-title-thread`

## Web usage

- `web/lib/luban-transport.ts` `useLubanTransport()`

## Config filesystem semantics

The following actions expose a filesystem-backed config tree to the UI:

- `CodexConfigTree`, `CodexConfigListDir`, `CodexConfigReadFile`, `CodexConfigWriteFile`
- `AmpConfigTree`, `AmpConfigListDir`, `AmpConfigReadFile`, `AmpConfigWriteFile`
- `ClaudeConfigTree`, `ClaudeConfigListDir`, `ClaudeConfigReadFile`, `ClaudeConfigWriteFile`

Rules:

- Entries may be `file` or `folder` (see `CodexConfigEntryKind` / `AmpConfigEntryKind`).
- Symbolic links are included in listings. The entry kind is derived by following the link target:
  - Symlink to a directory is reported as `folder`.
  - Symlink to a regular file is reported as `file`.
  - If the link target cannot be stat'ed, the entry is treated as `file` (and reads may fail).

## Action inventory (tracked)

All `ClientAction` variants are part of this contract surface. For reviewability, new actions must
update this section.

- `PickProjectPath`
- `AddProject`
- `AddProjectAndOpen`
- `TaskPreview`
- `TaskExecute`
- `FeedbackSubmit`
- `DeleteProject`
- `ToggleProjectExpanded`
- `CreateWorkspace`
- `OpenWorkspace`
- `OpenWorkspaceInIde`
- `OpenWorkspaceWith`
- `OpenWorkspacePullRequest`
- `OpenWorkspacePullRequestFailedAction`
- `ArchiveWorkspace`
- `EnsureMainWorkspace`
- `ChatModelChanged`
- `ThinkingEffortChanged`
- `SendAgentMessage`
- `CancelAndSendAgentMessage`
- `QueueAgentMessage`
- `RemoveQueuedPrompt`
- `ReorderQueuedPrompt`
- `UpdateQueuedPrompt`
- `WorkspaceRenameBranch`
- `WorkspaceAiRenameBranch`
- `CancelAgentTurn`
- `CreateWorkspaceThread`
- `ActivateWorkspaceThread`
- `CloseWorkspaceThreadTab`
- `RestoreWorkspaceThreadTab`
- `ReorderWorkspaceThreadTab`
- `OpenButtonSelectionChanged`
- `SidebarProjectOrderChanged`
- `SidebarWorktreeOrderChanged`
- `AppearanceThemeChanged`
- `AppearanceFontsChanged`
- `AppearanceGlobalZoomChanged`
- `CodexEnabledChanged`
- `AgentRunnerChanged`
- `AgentAmpModeChanged`
- `TaskPromptTemplateChanged`
- `SystemPromptTemplateChanged`
- `CodexCheck`
- `CodexConfigTree`
- `CodexConfigListDir`
- `CodexConfigReadFile`
- `CodexConfigWriteFile`
- `AmpCheck`
- `AmpConfigTree`
- `AmpConfigListDir`
- `AmpConfigReadFile`
- `AmpConfigWriteFile`
- `ClaudeCheck`
- `ClaudeConfigTree`
- `ClaudeConfigListDir`
- `ClaudeConfigReadFile`
- `ClaudeConfigWriteFile`

## Event inventory (tracked)

All `ServerEvent` variants are part of this contract surface:

- `AppChanged`
- `WorkspaceThreadsChanged`
- `ConversationChanged`
- `Toast`
- `ProjectPathPicked`
- `AddProjectAndOpenReady`
- `TaskPreviewReady`
- `TaskExecuted`
- `FeedbackSubmitted`
- `CodexCheckReady`
- `CodexConfigTreeReady`
- `CodexConfigListDirReady`
- `CodexConfigFileReady`
- `CodexConfigFileSaved`
- `AmpCheckReady`
- `AmpConfigTreeReady`
- `AmpConfigListDirReady`
- `AmpConfigFileReady`
- `AmpConfigFileSaved`
- `ClaudeCheckReady`
- `ClaudeConfigTreeReady`
- `ClaudeConfigListDirReady`
- `ClaudeConfigFileReady`
- `ClaudeConfigFileSaved`

## Request/response style events

The web UI treats some `ServerEvent` variants as request/response completions keyed by
`request_id` (see `web/lib/luban-transport.ts`):

- `ProjectPathPicked`
- `AddProjectAndOpenReady`
- `TaskPreviewReady`
- `TaskExecuted`
- `FeedbackSubmitted`
- `CodexCheckReady`
- `CodexConfigTreeReady`
- `CodexConfigListDirReady`
- `CodexConfigFileReady`
- `CodexConfigFileSaved`
- `AmpCheckReady`
- `AmpConfigTreeReady`
- `AmpConfigListDirReady`
- `AmpConfigFileReady`
- `AmpConfigFileSaved`
- `ClaudeCheckReady`
- `ClaudeConfigTreeReady`
- `ClaudeConfigListDirReady`
- `ClaudeConfigFileReady`
- `ClaudeConfigFileSaved`
