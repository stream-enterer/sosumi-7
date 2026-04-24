# Debug Harness Branch-Per-Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Update `docs/debug/run_debug.md` so the harness creates a `fix/F###` branch for code changes and commits only state (ISSUES.json, scratchpads) to `main`, leaving `main` clean and each fix independently reviewable.

**Architecture:** Single file edit to `run_debug.md`. No source code changes. The harness is a prompt document read and executed by an LLM — the "implementation" is rewriting the protocol steps with precise git commands and clear branching logic.

**Tech Stack:** Markdown, git CLI commands embedded in the protocol document.

---

### Task 1: Rewrite Step 1 — dirty tree recovery

The current Step 1 only handles one case (uncommitted changes → commit everything to current branch). Under the new design it needs to handle two cases: interrupted on a `fix/*` branch, or interrupted on `main`.

**Files:**
- Modify: `docs/debug/run_debug.md`

- [ ] **Step 1: Replace the Step 1 text**

Find the current Step 1 in `docs/debug/run_debug.md`:

```
### Step 1 — Check for dirty working tree

Run `git status`. If there are uncommitted changes of any kind (scratchpad, source files, ISSUES.json, or any combination), stage everything currently modified and commit with message `debug: recover uncommitted work from interrupted iteration`. Then continue to Step 2.
```

Replace with:

```
### Step 1 — Check for dirty working tree

Run `git status` and `git branch --show-current`.

**If on a `fix/*` branch with uncommitted changes:** stage all modified files and commit to the fix branch with message `debug: recover uncommitted work from interrupted iteration`. Then run `git checkout main` and continue to Step 2.

**If on `main` with uncommitted changes:** stage all modified files and commit to `main` with message `debug: recover uncommitted work from interrupted iteration`. Then continue to Step 2.

**If working tree is clean:** continue to Step 2 regardless of which branch you are on. If you are on a `fix/*` branch (interrupted after code work but before switching back), run `git checkout main` first.
```

- [ ] **Step 2: Verify the file saved correctly**

```bash
grep -n "fix/\*" docs/debug/run_debug.md
```

Expected: at least 2 matches (the two branch cases in Step 1).

- [ ] **Step 3: Commit**

```bash
git add docs/debug/run_debug.md
git commit -m "harness: update Step 1 dirty-tree recovery for branch-per-fix"
```

---

### Task 2: Rewrite Step 2 — issue selection with branch guard

The current Step 2 selects an issue by status/kind/priority only. It needs to also check whether a `fix/F###` branch already exists for a candidate issue, and skip it if so.

**Files:**
- Modify: `docs/debug/run_debug.md`

- [ ] **Step 1: Replace the Step 2 text**

Find the current Step 2 in `docs/debug/run_debug.md`:

```
### Step 2 — Select the target issue

Check the user message that invoked this harness for a phrase matching `for issue [A-Z]\d{3}` (e.g. "for issue F001"). If found, use that ID.

Otherwise, select from `docs/debug/ISSUES.json` using this priority order:
1. Among issues with status `investigating` or `root-cause-found` and kind `fix`, pick the highest priority one (`high` before `medium` before `low`). These resume in-progress work first.
2. Among `open` issues of kind `fix`, pick the highest priority one.

Issues of kind `design` or `perf` are out of scope for this harness. Skip them.

If no eligible issue exists, output the completion promise and stop:

```
<promise>DEBUG_PASS_COMPLETE</promise>
```
```

Replace with:

```
### Step 2 — Select the target issue

Check the user message that invoked this harness for a phrase matching `for issue [A-Z]\d{3}` (e.g. "for issue F001"). If found, use that ID — but still apply the branch-guard check below before proceeding.

Otherwise, select from `docs/debug/ISSUES.json` using this priority order, skipping any issue that fails the branch-guard check:

1. Among issues with status `investigating` or `root-cause-found` and kind `fix`, pick the highest priority one (`high` before `medium` before `low`). These resume in-progress work first.
2. Among `open` issues of kind `fix`, pick the highest priority one.

Issues of kind `design` or `perf` are out of scope for this harness. Skip them.

**Branch-guard check:** For each candidate issue with ID `F###`, run:
```
git branch --list "fix/F###"
```
If this returns a non-empty result, a prior run already completed the code work for this issue and the fix branch is waiting for human review. Skip this issue and move to the next candidate.

An issue is eligible if and only if:
- Kind is `fix`
- Status is `open`, `investigating`, or `root-cause-found`
- No `fix/F###` branch exists

If no eligible issue exists, output the completion promise and stop:

```
<promise>DEBUG_PASS_COMPLETE</promise>
```
```

- [ ] **Step 2: Verify**

```bash
grep -n "branch-guard\|git branch --list" docs/debug/run_debug.md
```

Expected: at least 2 matches.

- [ ] **Step 3: Commit**

```bash
git add docs/debug/run_debug.md
git commit -m "harness: add branch-guard to Step 2 issue selection"
```

---

### Task 3: Insert Step 2.5 — create the fix branch

After the issue is selected, the harness needs to create a `fix/F###` branch before doing any code work.

**Files:**
- Modify: `docs/debug/run_debug.md`

- [ ] **Step 1: Insert Step 2.5 after Step 2**

Locate the line `### Step 3 — Load investigation state` and insert the following new section immediately before it:

```
### Step 2.5 — Create fix branch

Now that an eligible issue is selected (ID = `F###`), create the fix branch off current `main` HEAD:

```
git checkout -b fix/F###
```

All code changes (source files, test files) for this issue will be committed to this branch. ISSUES.json and investigation scratchpad files are committed to `main`, not to this branch — that separation is enforced in Step 7.

Do not create the branch if it already exists (the branch-guard in Step 2 prevents reaching this step in that case).
```

- [ ] **Step 2: Verify**

```bash
grep -n "Step 2.5\|fix branch" docs/debug/run_debug.md
```

Expected: matches for "Step 2.5" and "fix branch".

- [ ] **Step 3: Commit**

```bash
git add docs/debug/run_debug.md
git commit -m "harness: insert Step 2.5 — create fix/F### branch after issue selection"
```

---

### Task 4: Rewrite Step 7 — split commit into code (branch) + state (main)

The current Step 7 commits everything in one commit to the current branch. Under the new design it must: commit code to `fix/F###`, switch back to `main`, commit state to `main`.

**Files:**
- Modify: `docs/debug/run_debug.md`

- [ ] **Step 1: Replace the Step 7 text**

Find the current Step 7 in `docs/debug/run_debug.md`:

```
### Step 7 — Commit

Commit once at the end of each iteration. The commit message summarises all transitions that occurred during the iteration.

Format: `debug(<ID>): <summary of what happened>`

If the commit contains only scratchpad and ISSUES.json changes (no source files), append `(scratchpad only)` to the message.

Examples:
- `debug(F001): phase 1 complete — root cause in emGUIFramework Focused handler (scratchpad only)`
- `debug(F001): hypothesis 1 ruled out — input_state not the cause (scratchpad only)`
- `debug(F001): root cause confirmed, fix committed — emGUIFramework Focused resets VIF state`
- `debug(F001): fix committed, needs manual verification`
- `debug(F001): wip — gathering phase 2 evidence (scratchpad only)`

Always stage: the scratchpad file, ISSUES.json, and any modified source files.

Update `head_sha` in the scratchpad frontmatter to the new HEAD SHA immediately after committing (`git rev-parse HEAD`).
```

Replace with:

```
### Step 7 — Commit

Commits are split between the fix branch (code) and `main` (state). Execute in this order:

**Step 7a — Commit code to fix branch (if any source files changed):**

You should currently be on `fix/F###`. Stage and commit only source and test files:

```
git add <source files> <test files>
git commit -m "fix(F###): <summary of what the fix does>"
```

Do not stage ISSUES.json or `docs/debug/investigations/` here. If no source files changed this iteration (investigation-only work), skip 7a.

**Step 7b — Switch to main and commit state:**

```
git checkout main
git add docs/debug/ISSUES.json docs/debug/investigations/F###.md
git commit -m "debug(F###): <summary of what happened>"
```

Format for the state commit message:

`debug(<ID>): <summary of what happened>`

Append `(scratchpad only)` if no code was committed in 7a.

Examples:
- `debug(F001): phase 1 complete — root cause in emGUIFramework Focused handler (scratchpad only)`
- `debug(F001): hypothesis 1 ruled out — input_state not the cause (scratchpad only)`
- `debug(F001): fix committed, needs manual verification`
- `debug(F001): wip — gathering phase 2 evidence (scratchpad only)`

**Step 7c — Update head_sha:**

After the `main` commit, update `head_sha` in the scratchpad frontmatter to the new HEAD SHA:

```
git rev-parse HEAD
```

Write that SHA into `docs/debug/investigations/F###.md` frontmatter and commit:

```
git add docs/debug/investigations/F###.md
git commit -m "debug(F###): update head_sha (scratchpad only)"
```

The iteration always ends on `main`.
```

- [ ] **Step 2: Verify**

```bash
grep -n "Step 7a\|Step 7b\|Step 7c\|git checkout main" docs/debug/run_debug.md
```

Expected: matches for 7a, 7b, 7c, and the checkout command.

- [ ] **Step 3: Commit**

```bash
git add docs/debug/run_debug.md
git commit -m "harness: rewrite Step 7 — split commits between fix branch and main"
```

---

### Task 5: Verify the complete updated harness reads coherently

Read through the full updated `run_debug.md` and confirm:
- Step 1 handles both dirty-on-fix-branch and dirty-on-main cases
- Step 2 includes the branch-guard check
- Step 2.5 creates the branch
- Steps 3–6 are unchanged (investigation phases, scratchpad lifecycle)
- Step 7 splits commits correctly and ends on `main`
- Step 8 terminal state check is unchanged
- No step refers to committing ISSUES.json on the fix branch
- No step refers to committing source files on `main`

**Files:**
- Read: `docs/debug/run_debug.md`

- [ ] **Step 1: Read the full file**

```bash
cat docs/debug/run_debug.md
```

- [ ] **Step 2: Check for contradictions**

```bash
# Confirm ISSUES.json is never staged on the fix branch
grep -n "git add.*ISSUES" docs/debug/run_debug.md
```

Expected: only appears inside Step 7b (after `git checkout main`).

```bash
# Confirm source files are never staged on main
grep -n "git add.*crates" docs/debug/run_debug.md
```

Expected: only appears inside Step 7a (on the fix branch).

- [ ] **Step 3: Final commit**

```bash
git add docs/debug/run_debug.md
git commit -m "harness: branch-per-fix complete — final coherence check"
```

Only commit if any last-minute prose fixes were needed. If the file is already clean from prior tasks, skip this commit.
