# AGENTS.md

This document constrains and guides how AI agents (and human contributors) should work in this repository. The goals are: predictable behavior, regressibility, reviewability, and maintainability. If this document conflicts with existing repository conventions, the repository conventions take precedence, and this document should be updated in the PR/change accordingly.

## 0. Quick start (read this first)
- Default branch: `main`.
- Read `postmortem/README.md` before implementing new features or large refactors.
- Use `pnpm` for all web-related commands; do not use `npm`.
- Prefer `just` recipes over ad-hoc `cargo` commands:
  - `just -l` (discover workflows)
  - `just fmt && just lint && just test` (full local verification)
  - `just test-fast` (domain-only tests)
  - `just run` / `just run release`
  - `just build` / `just build release`
- Keep changes small and reviewable. For functional changes, add or update tests.
- After finishing a task, run the relevant checks, then commit and push.

## 0.0 Mock-first UI workflow + contract-driven integration (required)

The UI source of truth is `web/` in mock mode, and the web/server boundary is governed by explicit
consumer-driven contracts (CDC) under `docs/contracts/`.

See:

- `docs/migrations/2026-01-22-web-mock-mode-contracts.md`
- `docs/contracts/README.md`
- `docs/contracts/progress.md`

For UI/UX work, the workflow is:

- **Mock first**: iterate interaction and UI behavior in `web/` mock mode without requiring the Rust server.
- **Contract first**: when UI changes require new backend capabilities, update contracts before (or together with) server changes.
- **Provider verified**: the Rust server must be validated against contracts via automated checks (contract tests in CI).

## 0.1 Contract alignment rules (hard requirement)

Any change that affects the web/server interaction surface must be aligned with contracts:

- HTTP: `/api/*`
- WebSocket: `/api/events`, `/api/pty/*`
- Message schemas: `WsClientMessage`, `WsServerMessage`, `ClientAction`, `ServerEvent`

Alignment means, at minimum:

1. Update the relevant contract documents under `docs/contracts/features/`.
2. Update `docs/contracts/progress.md` to reflect the new/changed surface and verification status.

For functional changes (non-refactor) that touch these surfaces, also:

3. Add or update provider-side contract tests so the Rust server is enforced against the contract in CI.

## 0.2 Postmortems (required)

This repository treats postmortems as a first-class engineering artifact.

- Before implementing a new feature, skim the relevant postmortems in `postmortem/`.
  - Prefer to reuse proven patterns and avoid repeating known failure modes.
- When fixing a **Sev-1** or **Sev-2** issue (or any bug that required multiple iterations):
  1. Create a new postmortem in `postmortem/` using the template in `postmortem/README.md`.
  2. Update `postmortem/README.md` to include the new incident in the index.
  3. Ensure the postmortem includes:
     - severity, impact, and detection
     - root cause analysis
     - "introduced by" commit(s) and "fixed by" commit(s)
     - reproduction and verification steps
     - prevention/action items

## 0.3 User-Facing Prompt Templates (required)

Luban ships prompt templates as a user-facing feature for tasks across *any* repository.

- Prompts must be **repository-agnostic**:
  - Do not hardcode Luban-specific commands, tools, or workflows.
  - Always instruct the code agent to discover and follow the target repository's own practices
    (README/CONTRIBUTING/CI).
- For code agents (Claude Code/Codex), prefer actionable guidance:
  - Encourage direct investigation and implementation in the worktree.
  - Avoid requiring "patch/diff" artifacts as the primary output, unless the user explicitly asks.

## 1. Project context and goals (first-screen context for agents)
- Tech stack: Rust server + web frontend (optionally wrapped by Tauri)
- All engineering commands are managed via `justfile` (prefer `just` over invoking `cargo` directly)
- Primary goals:
  1) Keep the server responsive; avoid blocking the async runtime or request handlers
  2) Make business logic unit-testable, property-testable, and regression-testable
  3) Make changes reviewable: small steps, and each change has corresponding verification/tests

## 2. Agent workflow (must follow)
### 2.1 After receiving a task
1) Clarify "user-observable behavior": prerequisites → operation → expected result (UI / state / files / logs)
2) Locate relevant code in the repo (prefer grep/ripgrep and module-based reading); do not invent structure
3) Provide a minimal change plan (by file/layer) and describe the test strategy

### 2.2 Before coding (required)
- Run `just -l` to see available commands; prefer existing recipes
- Find and follow this repository's:
  - Rust edition, clippy rules, fmt configuration
  - error handling style (anyhow/thiserror, etc.)
  - logging framework (tracing/log)

### 2.3 After completing changes (required)
- Run formatting and static checks (via `just`)
- Run tests (via `just`)
- Change description must include:
  - what changed (in terms of behavior/acceptance criteria)
  - scope of impact (which modules, and whether compatibility is affected)
  - how to verify (commands + manual steps)
  - which tests were added/updated

## 2.4 Repository map (where things live)
- `crates/luban_domain/`: pure state + reducers (`AppState`, `Action`, `Effect`), deterministic logic, most regressions should be captured here.
- `crates/luban_server/`: local server, WebSocket event stream, PTY endpoint, static file serving for `web/`.
- `crates/luban_tauri/`: desktop wrapper for the web UI.
- `web/`: browser UI (and agent-browser UI smoke tests under `web/tests/ui`).
- `tools/`: helper tooling.
- `docs/`: design notes and decisions. Add/update docs for non-trivial UX or architecture changes.
- `.context/` (gitignored): local scratchpad for collaboration between agents.

## 3. Architecture constraints (strong constraints)
Goal: UI is replaceable, logic is testable, and IO is isolated.

### 3.1 Layering principles
- `ui/`: rendering and event forwarding only; no direct IO; no complex business rules
- `domain/`: pure logic layer (state machines / use-cases / rules); must be unit-testable; prefer pure functions
- `adapters/`: boundaries to external systems (filesystem / network / database / system APIs); provide replaceable interfaces
- `app/` (or near `main.rs`): wire dependencies, bootstrap, routing, global resource management

### 3.2 Server constraints (general and practical)
- Do not block in request handlers (filesystem, network, long computations).
- Prefer async IO and bounded work; offload expensive tasks to background workers.
- Avoid unbounded memory growth in websocket/PTY streams (use backpressure or bounded buffers).

## 4. State management and actions (recommended pattern, unless the repo already defines one)
Prefer writing interactions as an "action-driven" state machine:
- `State`: minimal state needed by the UI (serializable is better)
- `Action`: user intent/events (click, input, open file, refresh)
- `Reducer/Update`: `(state, action) -> (state, effects)`
- `Effect`: an async/IO work unit (executed by a runner, then dispatches `Action::EffectCompleted(...)`)

Requirements:
- Reducers must be unit-testable (clear inputs/outputs)
- Effects must be mockable (adapter interfaces are replaceable)
- Anything that looks like "business rules" should live in `domain` first

## 5. Testing strategy (must be wired into the justfile)
### 5.1 Test pyramid
- Unit tests (required): cover domain rules, parsing, state transitions, sorting/filtering, and error branches
- Property tests (recommended): use proptest/quickcheck for key invariants
- Integration tests (as needed): validate adapters with real dependencies (controlled by features/env vars)
- UI smoke (required for user-visible changes): cover key user paths with agent-browser (`just test-ui`)

### 5.2 Hard requirements for agents
- Every functional change must add or update at least one test (unless it is pure refactoring; refactors still must not reduce coverage)
- For regression bugs: add a failing test case first, then fix (tests-first for bugfix)
- Any new or changed user-visible feature must add or update at least one UI test under `web/tests/ui` and keep `just test-ui` green

### 5.3 Suggested just recipes (if the repo does not provide them, you may add them)
When writing or using commands, choose by intent (names may differ; follow the repo):
- `just fmt`: formatting (rustfmt)
- `just lint`: clippy / deny / extra static checks
- `just test`: run tests
- `just test-fast`: run unit tests only (optional)
- `just run`: run locally
- `just ci`: simulate the full CI flow (optional)

> If a `justfile` already provides similar commands with different names, do not force new naming. Align this document to the existing naming instead.

## 5.4 agent-browser tips (for stable tests)
- Prefer stable `data-testid` selectors and avoid text-based selectors for dynamic content.
- Keep tests focused on smoke coverage for core user flows to reduce flakiness.

## 6. Error handling and logging (debuggability requirements)
- User-visible errors:
  - must be understandable and copyable (error codes/key fields are preferred)
  - must distinguish "retryable" vs "non-retryable"
- Internal errors:
  - use a unified error type and context (prefer thiserror + anyhow context, or an equivalent approach)
- Logging:
  - critical paths must include structured log fields (e.g., operation name, resource id, latency, result)
  - do not spam logs on high-frequency rendering paths
- Performance:
  - add spans/timing instrumentation for expensive UI-related operations (e.g., >50ms), consistent with the repo's logging framework

## 7. Concurrency and thread safety (must follow)
- Ownership and mutation paths for UI state must be single and auditable:
  - prefer that only the reducer/dispatcher mutates state
- Background tasks must not hold direct mutable references into UI internals
- Avoid long-held locks; do not wait for background task results on the UI thread (no synchronous wait/join)

## 8. Code style and maintainability
- Small PRs / small commits: one change focuses on one behavior or one bug
- Clear naming: prefer intent-based names like `Action::OpenFile`, `Action::SearchChanged`, `Effect::LoadDataset`
- Do not introduce a "second abstraction": do not add a parallel state management system just because it is convenient
- Public API changes must include doc comments and examples (for public crates or modules)

## 9. Change acceptance checklist (must be included in agent outputs)
Every delivery must include at least:
1) A list of behavior changes (as user-observable results)
2) Run and test commands (via `just`, e.g., `just fmt && just lint && just test`)
3) Manual verification steps (3–7 steps)
4) Risk points and rollback approach (if applicable)

For any changes that affect the web/server integration surface, also include:
5) The contract(s) updated (`docs/contracts/features/*` and `docs/contracts/progress.md`)
6) The contract verification status (what is mocked, what the provider implements, and what CI enforces)

## 10. Prohibited items (hard prohibitions)
- Do not bypass the `just` workflow by writing a pile of cargo commands (unless the justfile does not cover it, and you explain why)
- Do not introduce blocking IO into rendering/event handling hot paths
- Do not mix large refactors and features in one commit
- Do not modify lockfiles / toolchains / editions and other infrastructure without explaining the reason and impact

## 11. What to do when information is insufficient
- First discover via code and `just -l`:
  - the existing directory structure and patterns
  - existing test commands and CI entrypoints
- If still uncertain:
  - write down the uncertainties as assumptions
  - provide two implementation paths and explain the selection criteria and the impact on repository consistency
