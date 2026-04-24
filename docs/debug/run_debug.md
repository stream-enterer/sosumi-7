# Debug Harness

This file is the prompt for a debugging pass. It is read and executed by an LLM agent. It is not documentation for humans.

**Skill usage:** The only skill invoked in this harness is `superpowers:systematic-debugging` at Step 4. Do not invoke any other skill at any point.

---

## Agent Protocol

**Scope constraint: this harness processes exactly one issue per invocation. Once a terminal state is reached for the selected issue, no further work is performed — not even reading the next issue. Stop immediately.**

Steps 1–3 run once at the start of the invocation. Steps 4–8 drive the phase work: proceed through Phases 1–4, invoking Step 5 (ISSUES.json update) and Step 7 (commit) at each phase boundary, and checking Step 8 after every commit. Stop when Step 8 observes a terminal state or Step 6 routes to a blocked exit. Do not skip steps. Do not reorder them.

All work happens on `main`. The harness does not create, check out, or merge branches.

### Step 1 — Check for dirty working tree

Run `git status`.

If the working tree has uncommitted changes, stage all modified files and commit to `main` with message `debug: recover uncommitted work from interrupted prior run`. Then continue to Step 2. If the tree is clean, continue to Step 2.

### Step 2 — Select the target issue

Check the user message that invoked this harness for a phrase matching `for issue [A-Z]\d{3}` (e.g. "for issue F001"). If found, use that ID (subject to eligibility below).

Otherwise, select from `docs/debug/ISSUES.json` using this priority order:
1. Among issues with status `investigating` or `root-cause-found` and kind `fix`, pick the highest priority one (`high` before `medium` before `low`). These resume in-progress work first.
2. Among `open` issues of kind `fix`, pick the highest priority one.

Issues of kind `design` or `perf` are out of scope for this harness. Skip them.

An issue is eligible if and only if:
- Kind is `fix`
- Status is `open`, `investigating`, or `root-cause-found`

Skip any issue whose status is `needs-manual-verification` or `closed`.

If no eligible issue exists, stop immediately.

### Step 3 — Load investigation state

Check whether `investigation_file` is set on the selected issue in ISSUES.json, and whether that file exists on disk.

**If a scratchpad exists (resuming):**
- Read the scratchpad fully.
- Before executing anything, verify internal consistency: all phases prior to `current_phase` must have `complete: true`. If any prior phase has `complete: false`, the scratchpad is inconsistent from a crashed prior run — resume from the last phase that has `complete: true` rather than from `current_phase`.
- **Rollback detection:** If all four phases have `complete: true` but the current status in ISSUES.json is `investigating`, this means a runtime-verified fix was rejected by the human and rolled back. Do not re-execute Phase 4. Instead, re-enter Phase 3: form a new hypothesis based on the runtime failure, add it to the Phase 3 section of the scratchpad (with the failure as evidence), and proceed to a new implementation attempt. Update `current_phase` to `3` in the scratchpad frontmatter and set Phase 4's `complete` back to `false`.
- Check `head_sha` in the scratchpad against current HEAD on `main` (`git rev-parse HEAD`). If they differ, run `git diff <scratchpad_head_sha>..HEAD -- <files listed in next_steps>`. For each file that changed: re-read it, verify the intended action in `next_steps` is still feasible. If the target symbol or structure still exists at a new line, update the line reference. If the structure has fundamentally changed, mark the step `STALE: <reason>` and regenerate it from the current phase goals before executing.
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
- **Failed fix handling:** if a fix is committed and tests break, do not revert. Commit the broken state as a WIP commit. Subsequent phase work inherits the broken tests and either fixes them or rules out the hypothesis and tries another approach.

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
5. Stop immediately after the commit. Do not summarise, do not plan next steps.

### Step 7 — Commit

All commits land on `main`.

**Step 7a — Commit phase work:**

Stage source files, test files, `docs/debug/ISSUES.json`, and the scratchpad together, and commit:

```
git add <source files> <test files> docs/debug/ISSUES.json docs/debug/investigations/F###.md
git commit -m "debug(F###): <summary of what happened>"
```

Commit message format: `debug(<ID>): <summary of what happened>`. If no source files changed during this phase (investigation-only work), append `(scratchpad only)`.

Examples:
- `debug(F001): phase 1 complete — root cause in emGUIFramework Focused handler (scratchpad only)`
- `debug(F001): hypothesis 1 ruled out — input_state not the cause (scratchpad only)`
- `debug(F001): fix committed, needs manual verification`
- `debug(F001): wip — gathering phase 2 evidence (scratchpad only)`

**Step 7b — Update head_sha:**

Run `git rev-parse HEAD` and write the result into the scratchpad frontmatter as `head_sha`. Commit:

```
git add docs/debug/investigations/F###.md
git commit -m "debug(F###): update head_sha (scratchpad only)"
```

This keeps the scratchpad's `head_sha` aligned with `main` HEAD between phases so the stale check in Step 3 triggers only when commits are made to `main` outside the harness.

### Step 8 — Terminal state check

After committing, check if a terminal state has been reached:

- `closed` — fix committed and fully verified by automated tests
- `needs-manual-verification` — fix committed, human runtime verification required
- `root-cause-found` — root cause documented but fix is too large for this run; investigation complete for now
- `needs-design` — investigation confirmed the fix requires human architectural or planning decisions; kind reclassified to design
- `blocked` — investigation stalled, human input required

In all five cases, stop immediately after the commit. Do not summarise, do not plan next steps.

If no terminal state has been reached, return to Step 4 and continue the phase work.

---

## Scratchpad Template

Create this file at `docs/debug/investigations/<ID>.md` on fresh start.

```markdown
---
issue_id: <ID>
current_phase: 1
head_sha: <output of `git rev-parse HEAD`>
phases:
  1_root_cause_investigation: { complete: false }
  2_pattern_analysis: { complete: false }
  3_hypothesis_and_testing: { complete: false }
  4_implementation: { complete: false }
---

## Issue

<Copy title and description from ISSUES.json>

## Next Steps

1. <First specific action: file to read, command to run, line to check>

## Phase 1 — Root Cause Investigation

(empty — fill as gathered. Cite evidence as `E1.N (path:line)`; end with a
numbered root-cause chain from symptom to source.)

## Phase 2 — Pattern Analysis

(empty — cite C++ reference at `~/Projects/eaglemode-0.96.4/` and name the
Rust divergence. One or two `E2.N` entries, not a third summary entry.)

## Phase 3 — Hypotheses

(empty — one bullet per hypothesis, format below. No prose restatement of
the gate; the frontmatter `complete: true` is the gate.)
<!-- Format: - HYPOTHESIS: <description> / OUTCOME: CONFIRMED | RULED OUT — <reason> / EVIDENCE: E1.N, E2.N -->
```

**Phase gates** — a phase's `complete: true` means its gate is met. Gates:

1. Root cause chain traced to a specific file:line, evidence recorded.
2. C++ reference behaviour confirmed, Rust divergence named.
3. At least one hypothesis tested with CONFIRMED or RULED OUT outcome.
4. Fix committed. `cargo check`, `cargo clippy -- -D warnings`, `cargo-nextest ntr` all pass.

Advance only when the current gate is met. Do not write a "Phase N gate"
prose section inside the scratchpad — the frontmatter `complete` flag is
sufficient. If a gate cannot be met because investigation is stalled, go
to Step 6 (Blocked). If a gate cannot be met because the fix requires a
human decision, follow the `needs-design` path in Step 5.

**Do not duplicate the root cause.** Phase 1's root-cause chain is the
single source of truth. Do not add a trailing "## Root Cause" section,
and do not restate the fix in the scratchpad — the commit message and
ISSUES.json `fix_note` already carry it.

**next_steps discipline:** Each item must be specific enough to execute without interpretation. Not "check the VIF chain" — instead "read `crates/emcore/src/emVIF.rs:100-150` to verify input forwarding on WindowEvent::Focused". Check off completed items with `[x]`. Add new items as they are discovered. The first unchecked item is always the next action to execute, whether within this invocation or on resume after a crash.

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
