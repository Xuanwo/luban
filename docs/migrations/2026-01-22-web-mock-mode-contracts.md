# Web Mock Mode as the Only High-Fidelity UI + Contract-Driven Integration

Date: 2026-01-22
Status: Accepted (UI Migration Completed; Contract Verification In Progress)

## Context

The repository previously maintained:

- `design/`: a high-fidelity UI prototype (frontend-only)
- `web/`: the production browser UI (integrated with the local Rust server)

Keeping `web/` visually and behaviorally aligned with `design/` adds ongoing coordination cost and
slows down interaction iteration.

We want to optimize for:

- fast, backend-independent UI iteration
- predictable, reviewable integration with the Rust server
- fewer "alignment by manual diff" chores

## Decision

1. Remove `design/` and stop treating it as the UI source of truth.
2. Make `web/` in **mock mode** the only high-fidelity UI environment for interaction design.
3. Replace "design diff parity" with **contract-driven integration** (consumer-driven contracts).
   - The web UI (consumer) defines contracts and examples first.
   - The server (provider) must pass contract checks in CI.

Contracts live under `docs/contracts/`.

Current contract coverage and verification progress is tracked in `docs/contracts/progress.md`.

## Goals

- `cd web && pnpm dev` can run a full interaction demo without starting the Rust server.
- The same UI code can switch between:
  - mock adapters (default for UI iteration)
  - real adapters (for integration and end-to-end verification)
- Backend alignment is enforced by automation, not by convention.

## Non-goals

- Replacing the current wire protocol (WebSocket actions/events + HTTP hydration).
- Introducing a remote backend or multi-user model.
- Making mock mode production-grade or security hardened.

## Contract model

We use consumer-driven contracts (CDC):

- A contract is a stable, reviewable artifact that describes:
  - the interaction surface (endpoint / WS messages)
  - the expected behavior and invariants
  - minimal examples (fixtures) and edge cases
- The web UI can evolve contracts quickly (as features are designed).
- The server must implement and satisfy those contracts.

## Migration plan (progress tracked here)

- [x] Create a mock-mode switch for `web/` (env-based) and route IO through adapters.
- [x] Implement mock adapters for HTTP endpoints used by the UI.
- [x] Implement a mock transport for `/api/events` (including request/response patterns used by the UI).
- [x] Implement a mock terminal (local-only) for `/api/pty/*` UX iteration.
- [ ] Add server-side contract tests that validate responses against fixtures and invariants.
- [ ] Add a small set of real-mode E2E tests that run against `just web run`.
- [x] Remove `design/` from the repository and update documentation accordingly.
- [x] Remove "design parity" constraints from `docs/decisions.md` once the migration is complete.

## Risks and mitigations

- Risk: contracts drift into "wishful thinking" and the server lags indefinitely.
  - Mitigation: CI gate for provider contract tests; contracts must be updated in the same change as UI behavior changes.
- Risk: mock mode hides integration issues (timing, error handling, streaming).
  - Mitigation: keep a small real-mode E2E suite; add record/replay fixtures when needed.
- Risk: breaking changes to contracts without a compatibility plan.
  - Mitigation: version contracts explicitly and prefer additive evolution; use capability flags when necessary.
