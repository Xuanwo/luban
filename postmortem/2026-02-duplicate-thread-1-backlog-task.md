# Duplicate "Thread 1" Backlog Task After Creating a New Worktree

## 1. Summary

Creating a new git worktree could cause Luban to show two tasks: the intended new task and an extra
placeholder task titled `"Thread 1"` in `backlog`.

The issue was persistent across multiple reports because the root cause spanned both the domain
state initialization and the SQLite provider behavior.

## 2. Severity

Sev-3 (Medium)

## 3. Impact

- Users see an unexpected extra task (`"Thread 1"`) in the task list after creating a new worktree.
- The extra task is confusing and can pollute task history/state.
- Workaround: manually ignore/close the placeholder task (not obvious, not reliable long-term).

## 4. Detection

- Reported by users via repeated feedback after observing duplicate tasks on new worktrees.

## 5. Root cause

Two issues combined into a user-visible duplicate:

1) **Read endpoints had side effects** in the provider.
   - The SQLite layer implicitly created conversation rows (including `task_status='backlog'` and an
     auto-inserted `TaskCreated` system entry) when listing threads or reading a conversation.
   - This violated the expectation that a brand-new workdir may legitimately have **zero tasks**
     until the client explicitly creates one.

2) **Inconsistent “workspace has a default thread” assumptions** across layers.
   - The web client already supports empty task lists and will call `create_task` when none exist.
   - The domain/persistence code historically injected a default thread/tab state, which interacted
     poorly with the provider’s “ensure” behavior and resulted in an extra visible `"Thread 1"`.

## 6. Triggering commits (introduced by)

The behavioral mismatch accumulated over several changes:

- Provider-side auto-insert of a `"Thread {id}"` backlog conversation and `TaskCreated` entry when
  ensuring conversations (e.g. `03bf5428`, `d2bfbbf4`, `7f4cb3b1`).
- Domain workspace initialization and persistence that assumed a default thread (e.g. `fe03cf36`,
  `1b03b633`).

## 7. Fix commits (resolved/mitigated by)

Not committed yet (current working tree).

## 8. Reproduction steps

1. Add a git project in Luban.
2. Create a new workdir (git worktree) for that project.
3. Observe the task list:
   - Expected: one new task (or tasks created explicitly by the client).
   - Actual: an additional `"Thread 1"` task appears in `backlog`.

## 9. Resolution

- Provider: remove implicit task/thread creation on read paths (missing conversations now return
  “not found” instead of silently creating placeholders).
- Domain: allow workspaces to have zero threads initially; only create a thread on explicit
  `create_task` (or equivalent action).
- Compatibility: keep legacy conversation import/repair, but scope it to workspaces that actually
  have legacy conversation data, and avoid creating placeholder tasks for empty workdirs.
- Migration: add a schema migration to delete clearly-empty `"Thread 1"` placeholder tasks when the
  workspace already has other threads.

## 10. Lessons learned

- “Ensure” helpers must be carefully scoped; **read-only APIs should not create state** unless the
  contract explicitly says so.
- Cross-layer invariants (UI ↔ domain ↔ persistence) must be documented and contract-tested,
  especially around “empty state” semantics.

## 11. Prevention / action items

- Add a regression test that verifies `GET /api/workdirs/{id}/tasks` does not create tasks for an
  empty workspace.
- Keep contracts explicit about empty task lists and forbid provider-side side effects on read.
- Prefer a single source of truth for “workspace may start empty” and enforce it in both mock and
  provider implementations.
