# UI Testing (Playwright-first)

The web UI is the primary frontend. UI regression testing should be done with Playwright and should
prioritize stability over pixel-perfect diffs.

## Principles

- Prefer semantic assertions (element exists, state changes, scroll behavior) over screenshot-only
  comparisons.
- When a screenshot is useful, keep it scoped (component-level) and treat it as supporting evidence,
  not the only assertion.

## Selectors

- Prefer stable `title=` attributes and structural selectors aligned with `design/`.
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

Install browsers (once):

`cd web && pnpm exec playwright install`

Run:

- `just test-ui`
- `just test-ui-headed`
