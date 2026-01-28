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
- `ui.sidebar_worktree_order`: stable ordering for worktrees per project.

## Response

- `200 OK`
- JSON body: `AppSnapshot` (see `crates/luban_api::AppSnapshot`)

## Invariants

- The response must be valid JSON and deserializable into `AppSnapshot`.
- `rev` must be monotonically increasing over time (within a single server instance).

## Web usage

- `web/lib/luban-http.ts` `fetchApp()`
- Playwright E2E uses it as a readiness/hydration primitive.
