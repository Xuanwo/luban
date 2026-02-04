# C-HTTP-APP

Status: Draft
Verification: Mock=yes, Provider=yes, CI=no

## Surface

- Method: `GET`
- Path: `/api/app`

## Purpose

Hydrate the UI with the latest `AppSnapshot`.

This includes Task settings:

- `task.system_prompt_templates[]` / `task.default_system_prompt_templates[]`
  - `kind` is a `SystemTaskKind` string, currently:
    - `infer-type`
    - `rename-branch`
    - `auto-title-thread`

This includes persisted UI preferences for the sidebar:

- `ui.sidebar_project_order`: stable ordering for the project list.

This includes current UI selection:

- `ui.active_workdir_id`: the currently selected workdir id (optional).
- `ui.active_task_id`: the currently selected task id within the active workdir (optional).

This includes Agent settings:

- `agent.codex_enabled` / `agent.amp_enabled` / `agent.claude_enabled`
- `agent.default_runner` / `agent.amp_mode`
- `agent.default_model_id` / `agent.default_thinking_effort`

This includes integration status:

- `integrations.telegram.enabled` / `integrations.telegram.has_token`
- `integrations.telegram.bot_username` / `integrations.telegram.paired_chat_id`
- `integrations.telegram.config_rev` / `integrations.telegram.last_error`

## Response

- `200 OK`
- JSON body: `AppSnapshot` (see `crates/luban_api::AppSnapshot`)

## Invariants

- The response must be valid JSON and deserializable into `AppSnapshot`.
- `rev` must be monotonically increasing over time (within a single server instance).
- The provider must not include Telegram bot tokens in `AppSnapshot`.

## Web usage

- `web/lib/luban-http.ts` `fetchApp()`
- UI smoke tests treat it as a hydration primitive (indirectly via the app shell).
