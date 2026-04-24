# Debug Harness

This file is the prompt for a ralph-loop debugging pass. It is read and executed by an LLM agent. It is not documentation for humans.

**Skill usage:** The only skill invoked in this harness is `superpowers:systematic-debugging` at Step 4. Do not invoke any other skill at any point.

---

## How to Start a Debug Run

```
/ralph-loop "Read and execute the protocol in docs/debug/run_debug.md" --completion-promise "DEBUG_PASS_COMPLETE" --max-iterations 20
```

To target a specific issue by ID:

```
/ralph-loop "Read and execute the protocol in docs/debug/run_debug.md for issue F001" --completion-promise "DEBUG_PASS_COMPLETE" --max-iterations 20
```

Without an explicit ID, the harness selects the highest-priority open or resumable issue automatically.

---

## Agent Protocol

**Scope constraint: this harness processes exactly one issue per invocation. Once a terminal state is reached for the selected issue, no further work is performed — not even reading the next issue. Stop immediately.**

Execute the steps below in order every iteration. Do not skip steps. Do not reorder them.

### Step 1 — Check for dirty working tree

Run `git status` and `git branch --show-current`.

**If on a `fix/*` branch with uncommitted changes:** stage all modified files and commit to the fix branch with message `debug: recover uncommitted work from interrupted iteration`. Then run `git checkout main` and continue to Step 2.

**If on `main` with uncommitted changes:** stage all modified files and commit to `main` with message `debug: recover uncommitted work from interrupted iteration`. Then continue to Step 2.

**If working tree is clean:** continue to Step 2 regardless of which branch you are on. If you are on a `fix/*` branch (interrupted after code work but before switching back), run `git checkout main` first.

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
If this returns a non-empty result AND the issue's status is `needs-manual-verification` or `closed`, the fix is complete and awaiting human review — skip this issue and move to the next candidate.

If the branch exists AND status is `investigating` or `root-cause-found`, this is a resumable in-progress fix — the issue is eligible and Step 2.5 will check out the existing branch rather than creating a new one.

An issue is eligible if and only if:
- Kind is `fix`
- Status is `open`, `investigating`, or `root-cause-found`
- If status is `needs-manual-verification` or `closed`: always skip (regardless of branch state)

If no eligible issue exists, output the completion promise and stop:

```
<promise>DEBUG_PASS_COMPLETE</promise>
```

### Step 2.5 — Create or resume fix branch

Check whether `fix/F###` already exists:

```
git branch --list "fix/F###"
```

**If the branch does not exist** (fresh start or first iteration on this issue):
```
git checkout -b fix/F###
```

**If the branch already exists** (resuming a multi-iteration fix — status is `investigating` or `root-cause-found`):
```
git checkout fix/F###
```

All code changes (source files, test files) for this issue will be committed to this branch. ISSUES.json and investigation scratchpad files are committed to `main`, not to this branch — that separation is enforced in Step 7.

### Step 3 — Load investigation state

Check whether `investigation_file` is set on the selected issue in ISSUES.json, and whether that file exists on disk.

**If a scratchpad exists (resuming):**
- Read the scratchpad fully.
- Before executing anything, verify internal consistency: all phases prior to `current_phase` must have `complete: true`. If any prior phase has `complete: false`, the scratchpad is inconsistent from a crashed iteration — resume from the last phase that has `complete: true` rather than from `current_phase`.
- Check `head_sha` in the scratchpad against current HEAD (`git rev-parse HEAD`). If they differ, run `git diff <scratchpad_head_sha>..HEAD -- <files listed in next_steps>`. For each file that changed: re-read it, verify the intended action in `next_steps` is still feasible. If the target symbol or structure still exists at a new line, update the line reference. If the structure has fundamentally changed, mark the step `STALE: <reason>` and regenerate it from the current phase goals before executing.
- Execute the first unchecked item in `next_steps`.

**If no scratchpad exists (fresh start):**
- Create `docs/debug/investigations/<ID>.md` using the template in the *Scratchpad Template* section of this document.
- Set `investigation_file` in ISSUES.json to the scratchpad path.
- Set `status` to `investigating` in ISSUES.json.
- Proceed to Step 4 to invoke the systematic-debugging skill and begin Phase 1.

### Step 4 — Execute systematic debugging

Invoke the `superpowers:systematic-debugging` skill now using the Skill tool before doing any debugging work. Follow it exactly, with these project-specific additions and overrides:

- **C++ reference:** the ground truth for expected behaviour is at `~/Projects/eaglemode-0.96.4/`. Use it when tracing root causes.
- **Phase gates:** after each phase completes, set `complete: true` in the scratchpad frontmatter, update `current_phase`, and commit (see Step 7) before beginning the next phase.
- **Blocked trigger:** treat 3+ *distinct* ruled-out hypotheses (each with concrete evidence cited from Phase 1 or Phase 2) as the blocked condition. Route to Step 6.
- **Failing test case override:** the skill's Phase 4 mandates creating a failing test case first. For issues whose `repro` involves launching the app or visual/runtime observation (i.e. not coverable by `cargo-nextest`), skip the failing-test step and proceed directly to the fix. The `needs-manual-verification` terminal state handles final validation.
- **Failed fix handling:** if a fix is committed and tests break, do not revert. Commit the broken state as a WIP commit. The next iteration inherits the broken tests and either fixes them or rules out the hypothesis and tries another approach.

Record all evidence, hypotheses, and outcomes in the scratchpad under the appropriate phase headings. Every `RULED OUT` entry must cite a specific evidence entry from Phase 1 or Phase 2 in the scratchpad.

### Step 5 — Update ISSUES.json

After any meaningful state change, update ISSUES.json:

- **Root cause confirmed:** set `status` to `root-cause-found`, update `fix_note` with the confirmed root cause and fix direction, set `root_cause_file` to the path of the root-cause narrative you are about to write.
- **Fix lands:** run `cargo check`, `cargo clippy -- -D warnings`, and `cargo-nextest ntr`. If all pass, determine the terminal status using this rule: if the issue's `repro` field describes launching the app or visual/runtime observation, set `status` to `needs-manual-verification`; otherwise set `status` to `closed`. Populate `fixed_in_commit` and `fixed_date`. Note: for `needs-manual-verification` issues, passing tests are necessary but not sufficient — the fix may not work in practice. The human confirms at runtime and closes the issue.
- **Investigation reveals a design problem:** if at any point investigation confirms that the fix requires a human architectural or planning decision — not just a difficult code change, but a decision about *what* the right approach is — reclassify: set `kind` to `design`, set `status` to `needs-design`, update `fix_note` with exactly what the design session must decide. Also update the issue's shape: add a `details` block with `"audit_source": null` (the `design` kind requires it). This exit requires Phase 1 to be complete; you must have documented evidence for why autonomous resolution is impossible. Do not use `needs-design` to escape a hard investigation.
- **Blocked:** set `status` to `blocked`, populate `blocked_question` with the precise question a human must answer to unblock investigation.

If you discover a new issue during investigation, add it to ISSUES.json with status `open` and kind `fix` (default — the harness will reclassify if needed). Recording it is in scope. Investigating it is not — leave it for a future run.

### Step 6 — Blocked exit

If 3+ distinct hypotheses have been ruled out with concrete evidence and you cannot proceed:
1. Update the scratchpad: record all ruled-out hypotheses and what remains unknown.
2. Update ISSUES.json: set `status` to `blocked`, set `blocked_question` to the precise question that must be answered.
3. Update `fix_note` with a summary of what was tried.
4. Commit (Step 7).
5. Output the completion promise. This must be the absolute last line of your output — nothing after it:

```
<promise>DEBUG_PASS_COMPLETE</promise>
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

Format for the state commit message: `debug(<ID>): <summary of what happened>`

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

### Step 8 — Terminal state check

After committing, check if a terminal state has been reached:

- `closed` — fix committed and fully verified by automated tests
- `needs-manual-verification` — fix committed, human runtime verification required
- `root-cause-found` — root cause documented but fix is too large for this run; investigation complete for now
- `needs-design` — investigation confirmed the fix requires human architectural or planning decisions; kind reclassified to design
- `blocked` — investigation stalled, human input required

In all five cases, output the completion promise **after** the commit. This must be the absolute last line of your output — output nothing after it, do not summarise, do not plan next steps:

```
<promise>DEBUG_PASS_COMPLETE</promise>
```

If no terminal state has been reached, do not output the promise. The ralph-loop stop hook will re-feed this prompt and the next iteration will continue from Step 1.

---

## Scratchpad Template

Create this file at `docs/debug/investigations/<ID>.md` on fresh start.

```markdown
---
issue_id: <ID>
current_phase: 1
head_sha: <output of `git rev-parse HEAD`>
phases:
  1_root_cause_investigation:
    complete: false
    gate: "Root cause chain traced to a specific file:line. Evidence recorded in Phase 1 section. No fix proposed."
  2_pattern_analysis:
    complete: false
    gate: "C++ reference behaviour confirmed at ~/Projects/eaglemode-0.96.4/. Difference from Rust identified and recorded."
  3_hypothesis_and_testing:
    complete: false
    gate: "Root cause confirmed by a passing hypothesis. At least one hypothesis tested and recorded with CONFIRMED or RULED OUT outcome."
  4_implementation:
    complete: false
    gate: "Fix committed. cargo check, cargo clippy -- -D warnings, and cargo-nextest ntr all pass."
---

## Issue

<Copy title and description from ISSUES.json>

## Next Steps

1. <First specific action: file to read, command to run, line to check>

## Phase 1 — Root Cause Investigation

(empty — fill as gathered)

## Phase 2 — Pattern Analysis

(empty — fill when phase 1 gate passes)

## Phase 3 — Hypotheses

(empty — fill when phase 2 gate passes)
<!-- Format: HYPOTHESIS: <description> / OUTCOME: CONFIRMED | RULED OUT: <reason> / EVIDENCE: <ref to Phase 1 or Phase 2 entry> -->

## Root Cause

(empty — fill when phase 3 gate passes)
```

**Phase gate discipline:** Before advancing from one phase to the next, verify the gate condition is met. Set `complete: true` in the frontmatter and commit before starting the next phase. Do not begin Phase 2 with Phase 1's gate unmet. If a gate cannot be met because investigation is stalled, go to Step 6 (Blocked). If a gate cannot be met because the fix requires a human decision, follow the `needs-design` path in Step 5.

**next_steps discipline:** Each item must be specific enough to execute without interpretation. Not "check the VIF chain" — instead "read `crates/emcore/src/emVIF.rs:100-150` to verify input forwarding on WindowEvent::Focused". Check off completed items with `[x]`. Add new items as they are discovered. The first unchecked item is always what the next iteration executes first.

---

## Root Cause Narrative Template

When root cause is confirmed, write `docs/debug/investigations/<ID>-root-cause.md`:

```markdown
# <ID> — <Title> — Root Cause

**Date:** <YYYY-MM-DD>
**Issue:** <one-line description>

## Root Cause Chain

<Trace from symptom back to source. Reference file:line. Compare against C++ where relevant.>

## Fix

<What was changed and why it resolves the root cause.>

## Verification

<How the fix was verified: test names, cargo commands, or manual steps required.>
```
