# Debug Harness — Branch-Per-Fix Design

**Date:** 2026-04-24
**Status:** Approved

## Problem

The current debug harness (`docs/debug/run_debug.md`) commits all work — code changes, ISSUES.json
updates, and investigation scratchpads — directly to `main`. This prevents independent review and
merging of individual fixes, and caused a violation in practice where two fixes landed on `main`
in a single ralph-loop run with no way to separate them after the fact.

## Design

### Invariant

The harness always begins and ends each iteration on `main`. The ralph-loop re-feeds the same
prompt on each iteration; this invariant ensures it always starts fresh from the correct branch.

### What lives where

**`fix/F###` branches (code only):**
- Source file changes (`crates/...`)
- Test file changes
- Nothing else

**`main` (state only):**
- `docs/debug/ISSUES.json` — the single source of truth for issue status
- `docs/debug/investigations/F###.md` — investigation scratchpads, committed incrementally
  throughout the investigation so progress survives interruption

### Branch lifecycle per iteration

1. Harness selects an eligible issue (see Issue Selection below)
2. `git checkout -b fix/F###` off current `main` HEAD
3. All code work and code-only commits happen on `fix/F###`
4. Fix ready: `git checkout main`
5. Commit ISSUES.json status update + investigation scratchpad to `main`
6. Iteration ends on `main`

The fix branch is left in place. The human reviews and merges when satisfied.

### Issue selection and branch guard

Before selecting an issue, the harness checks for an existing `fix/F###` branch:

```
git branch --list "fix/F###"
```

**Eligibility rules (in priority order):**

1. Status `investigating` or `root-cause-found`, kind `fix`, no `fix/F###` branch exists → resume
2. Status `open`, kind `fix`, no `fix/F###` branch exists → fresh start
3. Anything else → skip:
   - Status is `needs-manual-verification`, `closed`, `needs-design`, or `blocked`
   - A `fix/F###` branch already exists (prior run completed code work, awaiting human merge)
   - Kind is `design` or `perf`

### Merge and cleanup

The harness **never** merges branches and **never** deletes branches. The human owns all merges.
When the human confirms a fix works at runtime, they tell the agent in natural language; the agent
merges the fix branch to `main` and closes the issue in ISSUES.json only after explicit human
go-ahead.

### Commit conventions

Unchanged from current protocol:

- Code commits on `fix/F###`: `fix(F###): ...`
- State commits on `main`: `debug(F###): ...` / `chore: ...`

### Note on `fixed_in_commit`

This field records the SHA of the fix commit, which lives on `fix/F###` and is not reachable
from `main`'s history until the branch is merged. This is expected — it tells the human which
commit to find on the branch. The field remains useful as a reference.

## Changes to `run_debug.md`

The following steps change:

**Step 2 (issue selection):** Add branch-guard check. After selecting the target issue ID, run
`git branch --list "fix/F###"`. If the branch exists, skip to the next eligible issue.

**New Step 2.5 (create branch):** After confirming the issue is eligible and no branch exists,
run `git checkout -b fix/F###`. All subsequent code work happens on this branch.

**Step 7 (commit):** Split into two commits:
- Code commit on `fix/F###` (source + test files)
- Switch to `main`: `git checkout main`
- State commit on `main` (ISSUES.json + investigation scratchpad)
- Iteration ends on `main`

**Step 1 (dirty tree check):** Two recovery cases:
- If on a `fix/*` branch with uncommitted changes: commit all dirty files to the fix branch
  (recovery commit), then switch to `main` and continue.
- If on `main` with uncommitted changes: commit all dirty files directly to `main`
  (handles interruption between the branch-switch-back and the state commit).
In both cases, use message `debug: recover uncommitted work from interrupted iteration`.

All other steps (phases 1–4, scratchpad lifecycle, terminal state check) are unchanged.
