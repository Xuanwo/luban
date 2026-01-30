# UI Testing (agent-browser)

The web UI is the primary frontend. UI regression testing is done with `agent-browser` and should
prioritize stability over pixel-perfect diffs.

## Principles

- Prefer semantic assertions (element exists, state changes, scroll behavior) over screenshot-only
  comparisons.
- When a screenshot is useful, keep it scoped (component-level) and treat it as supporting evidence,
  not the only assertion.

## Selectors

- Prefer stable `data-testid` attributes, then `title=` attributes and structural selectors aligned
  with the UI contracts and documented interaction expectations.
- Avoid selectors derived from transient values (timestamps, random ids, etc.).

## Recommended checks

- Tab behavior:
  - new tab always appends to the end
  - restore appends to the end
- Scroll behavior:
  - follow-tail when at bottom
  - show "Scroll to bottom" only when user scrolls away
  - no page-level scroll (only content panes scroll)
- Terminal:
  - reconnect/refresh preserves output (bounded replay)
  - resize sends rows/cols correctly
  - theme matches CSS variables

## Running locally

Run:

- `just test-ui`
- `just test-ui-headed`

Prerequisites:

- `pnpm` must be installed for starting the web dev server.
- A Chromium-compatible browser must be available for Playwright to launch (see `AGENT_BROWSER_EXECUTABLE_PATH` in
  the upstream agent-browser docs if you need to point to a custom executable).

## Isolation and safety (mock mode)

UI tests run in `web/` mock mode by default:

- Tests start `next dev` with `NEXT_PUBLIC_LUBAN_MODE=mock`, so no Rust server is required.
- Tests do not touch your real `LUBAN_ROOT` state.

## Session / profile isolation

The smoke test uses a random `luban-<hex>` session id and an isolated persistent profile directory by default.

Useful overrides:

- `LUBAN_AGENT_BROWSER_SESSION`: set the session id
- `LUBAN_AGENT_BROWSER_PROFILE_DIR`: set the persistent profile directory
- `LUBAN_AGENT_BROWSER_BASE_URL`: reuse an existing dev server instead of starting one
