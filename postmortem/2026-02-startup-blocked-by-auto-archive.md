# Startup blocked by auto-archive cleanup

## Summary

On `v0.2.14+20260205`, Luban could appear to "start" but render an empty UI (for example the sidebar Projects list is empty) because the server engine never became responsive. The engine startup path awaited auto-archive maintenance that could take seconds (or effectively hang on slow filesystem/git operations), preventing HTTP/WebSocket handlers from serving snapshots and events.

## Severity

Sev-2 (High)

## Impact

- On launch, the web UI shows no projects/tasks and does not recover.
- HTTP endpoints that depend on the engine (for example snapshot endpoints) can stall until maintenance finishes.
- Users may misinterpret this as data loss.

## Detection

User report: "after upgrading from `v0.2.13+20260205` to `v0.2.14+20260205`, the UI is completely empty".

## Root cause

- `Engine::start` executed `engine.bootstrap().await` before entering the command loop that serves `EngineCommand`s.
- `bootstrap()` performed synchronous maintenance:
  - scanning workspaces and calling `list_conversation_threads` for each candidate
  - triggering `ArchiveWorkspace` effects
- `Effect::ArchiveWorkspace` awaited blocking cleanup work (git/worktree removal) via `spawn_blocking(...).await`.

Because the command loop did not start until `bootstrap()` completed, any slow maintenance made the entire server appear unresponsive to the UI.

## Triggering commits (introduced by)

- `a66f848` ("Treat done/canceled tasks as archived") introduced auto-archive cleanup behavior that could run during startup.

## Fix commits (resolved/mitigated by)

- Unreleased patch: make startup maintenance non-blocking and make archive cleanup asynchronous.

## Reproduction steps

1. Have at least one git project with multiple workspaces.
2. Ensure some workspaces are eligible for auto-archive (all tasks `done`/`canceled` and turns idle).
3. Make `list_conversation_threads` and/or the workspace cleanup slow (for example slow disk or expensive git operations).
4. Start Luban.
5. Observe the UI loads but shows an empty Projects list and does not populate.

## Resolution

- Keep `bootstrap()` lightweight and schedule maintenance in background tasks.
- Make `ArchiveWorkspace` effect fire-and-forget and report completion back into the engine loop via `EngineCommand::DispatchAction`.
- Add regression tests to ensure:
  - auto-archive scan cannot block `GetAppSnapshot`
  - archive cleanup cannot block the engine from serving snapshots

## Lessons learned

- Startup should never await best-effort maintenance.
- Long-running cleanup must not run on the engine command loop.

## Prevention / action items

- Keep a dedicated test for engine responsiveness during startup maintenance.
- Treat archival/cleanup as bounded background work with explicit completion signals.
