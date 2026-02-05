# Postmortems

This directory contains engineering postmortems for user-visible bugs, CI regressions, and correctness issues.

## Severity scale

- **Sev-1 (Critical)**: data loss, security incident, or persistent corruption; no safe workaround.
- **Sev-2 (High)**: major feature broken for typical users; limited workaround.
- **Sev-3 (Medium)**: partial degradation, flaky behavior, or CI instability; workaround exists.
- **Sev-4 (Low)**: cosmetic or low-impact papercut.

## Index (by incident)
- `2026-02-startup-blocked-by-auto-archive.md`

- `2026-02-duplicate-thread-1-backlog-task.md`
- `2026-02-task-list-project-avatar-inconsistency.md`
  - Related fix commits: `c06feb1`
- `2026-01-terminal-clipboard-shortcuts.md`
  - Related fix commits: `a3b4fd0`, `b7120c4`, `a343f74`, `397fc88`
- `2026-01-ghostty-web-clipboard-overwrite.md`
  - Upstream issue: `coder/ghostty-web#108`
  - Related fix commits: `a3b4fd0`, `b7120c4`, `a343f74`, `397fc88`
- `2026-01-e2e-parallel-flakiness.md`
  - Related fix commits: `0489744` (and stabilization commit `6051e9d`)
- `2026-01-engine-missing-effect-handlers.md`
  - Related fix commits: `6481652`, `b4e52ae`
- `2026-01-streaming-markdown-missing.md`
  - Related fix commits: `3c17e36`
- `2026-01-chat-model-effort-noop.md`
  - Related fix commits: `c267663`
- `2026-01-ws-connected-stale.md`
  - Related fix commits: `2e19684`
- `2026-01-tauri-zoom-capability.md`
  - Related fix commits: `2aed57d`
- `2026-01-kanban-main-worktree-missing.md`
  - Related fix commits: `9b30a77`
- `2026-01-project-dedupe-and-github-reuse.md`
  - Related fix commits: `c3eed50`, `1a2981e`
- `2026-01-task-intent-inference-output.md`
  - Related fix commits: `ba8c800`
- `2026-01-task-prompts-repo-specific-assumptions.md`
  - Related fix commits: `7611c8b`, `9189cfd`
- `2026-01-thinking-steps-miscount.md`
  - Related fix commits: `5555663`, `bc6d13d`
- `2026-01-gpui-blocks-reflow.md`
  - Related fix commits: `fc68958`, `4ee5542`
- `2026-01-gpui-terminal-cursor-visibility.md`
  - Related fix commits: `7502750`

## When to write a new postmortem

Write a new postmortem when fixing:

- any **Sev-1** or **Sev-2** issue
- any bug that required multiple iterations to fix safely
- any regression that broke CI or release workflows

## Template (copy/paste)

Create a new file named `YYYY-MM-<short-title>.md` and fill in:

1. **Summary**
2. **Severity**
3. **Impact**
4. **Detection**
5. **Root cause**
6. **Triggering commits (introduced by)**
7. **Fix commits (resolved/mitigated by)**
8. **Reproduction steps**
9. **Resolution**
10. **Lessons learned**
11. **Prevention / action items**
