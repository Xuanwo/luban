# Agent Runner Integration Playbook

This document captures the practical lessons from integrating `amp` as a first-class agent runner in Luban.
Use it as a checklist and a set of guardrails when adding the next runner.

## Goals

- Support multiple agent runners behind one UI and one persistence model.
- Keep the domain layer deterministic and testable.
- Normalize runner-specific streaming into a stable, runner-agnostic event model (`AgentThreadEvent`).
- Share one attachment model across runners, including images.
- Keep UI behavior consistent via `web/` mock mode and explicit contracts.

Non-goals:

- Designing the upstream runner protocol.
- Adding runner-specific UX everywhere (prefer one picker pattern).

## Architectural Principles

### 1) Runner-agnostic domain state

The domain should not care about "how the CLI streams" or "how the runner formats events".
It should only care about:

- the selected runner and per-turn run config
- the normalized event stream (`AgentThreadEvent`)
- durable defaults (runner defaults, model defaults, etc.)

### 2) Normalize at the boundary

Parsing and normalization belong at the runner boundary (services/adapters), not in reducers.
Reducers should treat incoming events as already normalized.

### 3) Attachments are always structured

- User message text stays plain text.
- Attachments are passed as structured fields (not embedded as inline tokens).
- All runners share the same attachment kinds and storage semantics.

### 4) Mock-first + contract-first workflow for UI

- Iterate interaction and UI behavior in `web/` mock mode first.
- Update consumer-driven contracts under `docs/contracts/` together with the UI behavior change.
- Keep the picker interaction model stable across runners.

## Integration Checklist (End-to-End)

### A) Add the runner kind

- Add/extend `AgentRunnerKind` (domain + API + web types).
- Ensure persistence includes:
  - default runner
  - runner-specific durable defaults (if any)

### B) Implement runner config management

For runner-specific config files (like Amp):

- Expose a config tree/list/read/write API.
- Keep tree listing shallow; use list-dir pagination or explicit directory listing for deep traversal.
- Do not mix runner config controls into unrelated settings sections.

### C) Define and implement `AgentThreadEvent` normalization

Create a single internal event model to represent:

- thread start/end
- turn start/end
- stdout/stderr/log lines
- tool use + tool result (with stable correlation IDs)
- file changes and diffs (if available)
- final assistant message
- errors/cancellation

Implementation guidelines:

- Prefer lossless normalization (keep original raw payload when useful for debugging).
- Scope IDs per turn if the upstream does not guarantee global uniqueness.
- Make normalization tolerant of unknown/extra events.
- Ensure reconnection and resync behavior does not duplicate side effects.

Add tests for:

- parsing representative CLI outputs
- unknown events being ignored safely
- correlation invariants between tool use and tool result

### D) Implement a shared attachment pipeline

When the user uploads attachments:

- Always copy attachments into the workspace context store.
- Prefer content-addressed storage for deduplication and stable references.
- Preserve a user-facing display name alongside the content address.

Images:

- Store original bytes and a thumbnail (for UI rendering and bandwidth).
- Ensure the prompt builder for each runner receives whatever it needs:
  - some runners need image file paths
  - some runners may accept image references differently

Add tests for:

- "context files are content addressed and preserve display name"
- "context images store thumbnail alongside original"
- prompt builder behavior for each runner

### E) Per-turn overrides vs durable defaults

Define precedence rules explicitly and make them testable.

A typical precedence chain:

1. explicit environment overrides (if supported)
2. per-turn override from UI
3. durable defaults stored in app state / config files

Implementation guidelines:

- Per-turn run config should be attached to the queued/running turn and remain stable while the turn runs.
- Changing durable defaults should not retroactively mutate an already-running turn.

### F) UI: one agent picker, dynamic per runner

Keep one picker interaction model:

- Horizontal, multi-column layout.
- First column selects the runner (agent).
- Subsequent columns are conditional per runner:
  - Codex: `Model` + `Reasoning`
  - Amp: `Mode` (`Smart` / `Rush`)

Default behavior:

- Do not render a separate "Default Agent" option.
- When opening the picker, preselect the default runner.
- Use a hover overlay for the default row (and a settings shortcut), but do not block clicking the row.

Runner-specific defaults in the picker:

- Do not add an explicit "Default <value>" row.
- Instead, mark the default option with a hover `default` overlay.
- Selecting the default option clears the per-turn override.

### G) Manual smoke + automated verification

Automated:

- `just fmt && just lint && just test`
- `pnpm -C web exec tsc --noEmit`
- `pnpm -C web lint`

Manual (minimal):

1. Start the server (`just run-server`).
2. Open the agent picker and confirm:
   - default runner is preselected
   - default overlays do not intercept clicks
3. Select each runner and verify:
   - correct columns appear
   - per-turn overrides are applied to the next send/queue action
4. Upload:
   - one text file
   - one image
   Verify both appear as attachments in the composer and the run succeeds.

## Amp-Specific Lessons

- Amp mode is primarily configured via Amp config files (Amp config panel).
- The agent picker only provides a per-turn override for mode (`smart` / `rush`).
- The picker should derive the default mode from the durable/default state and:
  - highlight it via a hover `default` overlay
  - clear override when the user selects that default

See `docs/amp-support.md` for current implementation details.
