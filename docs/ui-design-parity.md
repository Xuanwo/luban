# UI Design Parity (Source of Truth)

The UI source of truth is maintained in `Xuanwo/luban-design`.

This repository must keep the browser UI **visually and structurally consistent** with that design project.
When there is a mismatch, the design project wins and this repository must be updated to match.

## Requirements

- The frontend framework must match `luban-design`:
  - Next.js (App Router)
  - React (major version must match)
  - Tailwind CSS v4 (CSS-first config via `app/globals.css`)
- The frontend must remain compatible with static export served by `luban_server` (Next `output: "export"`).
- UI implementation should reuse the same design tokens, layout primitives, and component patterns.
- Do not introduce a second UI design system in this repository.
- UI-only state that does not affect domain correctness (draft/scroll/layout) must remain in browser `localStorage`.

## Workflow

1. Make UI/UX changes in `luban-design` first (high-fidelity iteration).
2. Port the changes into this repository under `web/` while keeping:
   - file structure compatible with Next.js export
   - API integration (`/api/*`, `/api/events`, `/api/pty/*`)
3. Keep dependency versions aligned:
   - `web/package.json` should be updated by diffing against `luban-design/package.json`

## Submodule linkage (in this repository)

This repository includes `Xuanwo/luban-design` as a git submodule at `design/`.

The submodule is **read-only** from the perspective of this repository:

- Do not make changes inside `design/`.
- Update the submodule pointer when you want to sync to a newer design revision.

Suggested workflow:

- Update: `git submodule update --remote design`
- Sync `web/` by applying a focused diff against `design/` (keep integration diffs small and localized).

## Non-goals

- Building a standalone web backend in `luban-design`.
- Allowing the UI to drift from `luban-design` "because it is easier" for the backend.
- Implementing authentication/authorization for localhost-only use.
