# AGENTS.md

This document constrains and guides how AI agents (and human contributors) should work in this repository. The goals are: predictable behavior, regressibility, reviewability, and maintainability. If this document conflicts with existing repository conventions, the repository conventions take precedence, and this document should be updated in the PR/change accordingly.

## 1. Project context and goals (first-screen context for agents)
- Tech stack: Rust + gpui (native UI framework)
- All engineering commands are managed via `justfile` (prefer `just` over invoking `cargo` directly)
- Primary goals:
  1) Keep the UI thread responsive; avoid blocking work / heavy recomputation / IO
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

## 3. Architecture constraints (strong constraints)
Goal: UI is replaceable, logic is testable, and IO is isolated.

### 3.1 Layering principles
- `ui/`: rendering and event forwarding only; no direct IO; no complex business rules
- `domain/`: pure logic layer (state machines / use-cases / rules); must be unit-testable; prefer pure functions
- `adapters/`: boundaries to external systems (filesystem / network / database / system APIs); provide replaceable interfaces
- `app/` (or near `main.rs`): wire dependencies, bootstrap, routing, global resource management

### 3.2 gpui constraints (general and practical)
- The UI thread should only do:
  - state reads / lightweight computation
  - view rendering
  - event dispatching (action/command)
- Anything that may block (file IO, network, heavy recomputation, index building, etc.) must run in background tasks.
- Background task results must return to the UI through a unified "message/event/scheduler" entrypoint to update state (avoid mutating shared state directly from many places).
- Allowed side-effect entrypoints:
  - adapter layer
  - a unified "effect runner" defined in the app wiring layer
- Forbidden in the view rendering path:
  - file/network IO
  - large allocations / sorting / traversing full datasets
  - uncontrollable lock contention

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
- UI/E2E (small amount): only cover smoke tests for key user paths; if gpui is hard to automate, at least provide "structure snapshot tests" or "state snapshot tests"

### 5.2 Hard requirements for agents
- Every functional change must add or update at least one test (unless it is pure refactoring; refactors still must not reduce coverage)
- For regression bugs: add a failing test case first, then fix (tests-first for bugfix)

### 5.3 Suggested just recipes (if the repo does not provide them, you may add them)
When writing or using commands, choose by intent (names may differ; follow the repo):
- `just fmt`: formatting (rustfmt)
- `just lint`: clippy / deny / extra static checks
- `just test`: run tests
- `just test-fast`: run unit tests only (optional)
- `just run`: run locally
- `just ci`: simulate the full CI flow (optional)

> If a `justfile` already provides similar commands with different names, do not force new naming. Align this document to the existing naming instead.

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
