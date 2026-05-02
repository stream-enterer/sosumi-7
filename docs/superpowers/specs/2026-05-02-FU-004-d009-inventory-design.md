# FU-004 — D-009 polling-intermediary inventory (verified)

**Bucket:** [FU-004](../../debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-004-d009-polling-sweep.md)
**Date:** 2026-05-02
**Scope:** tree-wide enumeration + classification, **no code changes.**
**Prereqs:** none.

## Summary

The FU-004 bucket file specified a discovery-led sweep with three phases: enumerate, classify, remediate. A first-pass brainstorm tried to produce a remediation plan, but verifications revealed the bucket file's "known starting point" (emCoreConfigPanel reset closure) does not appear in current source, and the most-prominent grep hit (`emBookmarkButton::pending_click_fire`) is a vestigial placeholder for unimplemented Input rather than a polling intermediary.

**This spec scopes FU-004 to the inventory deliverable only.** The work is to grep exhaustively, verify each candidate against C++ and Rust source, classify each, and publish a row table. **No remediation is specified here.** Remediation, if any candidates classify as D-007 candidates after verification, gets a follow-up spec written against the verified inventory.

## Deliverable

A row table appended to the FU-004 bucket file (or to a sibling `FU-004-inventory.md` if the bucket file gets too long), containing for each candidate:

| Field | Description |
|---|---|
| **Site** | Rust file:line of the field declaration. |
| **Shape** | One of: `Cell<bool>`-flag drained in Cycle, `RefCell<Option<...>>` queue, closure registry, vestigial-placeholder, in-progress-migration. |
| **Setter sites** | List of call sites that mutate the field (Rust file:line each). |
| **Drain site** | Where the field is read/cleared (typically Cycle/cycle/LayoutChildren). |
| **C++ counterpart** | C++ file:line of the analogous field/method, or "none." |
| **C++ shape** | Whether C++ uses the same polling pattern, fires synchronously, or has no analogue. |
| **Classification** | One of: `D-007 candidate`, `C++-mirrored`, `forced retention`, `vestigial`, `out-of-scope`. |
| **Evidence** | One-line summary of why the classification holds. |
| **Action** | If `D-007 candidate`: scoped to follow-up spec. If `C++-mirrored`: add an explanatory comment if not already present. If `forced retention`: add `DIVERGED:` annotation. If `vestigial`: scratch-dump entry with cleanup trigger. If `out-of-scope`: cross-reference the owning effort. |

## Work units

Three units, two commits (one for the row table, one for any in-place comment/annotation additions surfaced by classification).

### Unit 1 — Exhaustive enumeration (research)

**Output:** raw candidate list (no commit; informs Unit 2).

Greps to run, in order, accumulating distinct hits:

1. **Cell-flag fields:**
   ```
   grep -rn "^\s*(pub )?(pub\(crate\) )?[a-z_]*: Cell<bool>" crates/*/src/
   grep -rn "^\s*(pub )?(pub\(crate\) )?[a-z_]*: Cell<Option<" crates/*/src/
   grep -rn "^\s*(pub )?(pub\(crate\) )?[a-z_]*: Cell<usize>" crates/*/src/
   grep -rn "^\s*(pub )?(pub\(crate\) )?[a-z_]*: Cell<u64>" crates/*/src/
   ```
2. **Plain-bool flag fields with names that signal pending state:**
   ```
   grep -rn "^\s*(pub )?(pub\(crate\) )?(pending_|do_|to_|needs_|update_)[a-z_]*: bool" crates/*/src/
   ```
3. **RefCell-Option queue fields:**
   ```
   grep -rn "^\s*(pub )?(pub\(crate\) )?[a-z_]*: RefCell<Option<" crates/*/src/
   ```
4. **Closure-registry shapes:**
   ```
   grep -rn "Vec<Box<dyn Fn" crates/*/src/
   grep -rn "Rc<RefCell<Vec<Box<dyn" crates/*/src/
   ```
5. **`.take()` / `.replace(false)` / `.replace(None)` calls inside `fn Cycle` / `fn cycle` bodies:**
   ```
   # heuristic — read each Cycle/cycle body, look for drain patterns
   grep -B2 -A30 "fn Cycle\b\|fn cycle\b" crates/*/src/ | grep -E "\.(take|replace)\("
   ```

Filter out:
- Test code (`#[cfg(test)]` modules, `for_test` helpers).
- Local variables (only field declarations count).
- `pending_actions` on App (justified, used by FU-002).
- `pending_inputs` on test harness / scheduler (test infra / justified).
- `pending_signals` on Scheduler (internal queue, not a panel-side polling intermediary).

The **draft** results from the brainstorm pass (to be re-verified, not trusted as-is):
- `emBookmarkButton::pending_click_fire` (likely vestigial).
- `emTextField` pending_*_fire cluster (in-progress B3.4c/d migration).
- `emVirtualCosmosItemPanel::update_needed` (likely C++-mirrored).
- `emFileLinkPanel::do_update` + `dir_entry_up_to_date` (likely C++-mirrored — B-016 fidelity).
- `emMainWindow::to_close` (verified C++-mirrored: emMainWindow.cpp:163,184).
- `emDialog::pending_result` (out-of-scope: Phase 3.6 Task 4 will delete `Cycle` and call sites).
- `emPriSchedAgent::callback` / `emRecListener::callback` / `emDialog` callback chains (likely C++-mirrored — verify per pattern).

### Unit 2 — Per-candidate verification

For each enumerated candidate, perform exactly these checks:

1. **Read the field declaration's surrounding doc comment** for stated rationale (often cites C++ line numbers or audit IDs).
2. **Enumerate all setter sites** (`grep -n "self\.<field>\s*=\|self\.<field>\.set\|self\.<field>\.replace\(true"`).
   - For each setter, identify whether the calling context has access to a `SignalCtx` / `PanelCtx` / `EngineCtx` (i.e., whether synchronous fire is feasible).
3. **Identify the drain site(s).**
4. **Read the cited C++ counterpart.** If the doc doesn't cite a C++ file:line, search C++ headers for analogously-named fields (`grep -n "<FieldName>" ~/Projects/eaglemode-0.96.4/include/<ScopeDir>/*.h`).
5. **Compare shapes:**
   - C++ uses the same polling shape (set-and-cycle-drain) → `C++-mirrored`.
   - C++ fires synchronously at the mutation site → Rust diverged → `D-007 candidate`.
   - C++ has no analogue (Rust-only) → context-dependent classification.
6. **Classify and record evidence** (one-line summary citing file:line for the deciding observation).

Special cases the verification must handle:

- **Vestigial-placeholder pattern.** Field exists with no production setter (only test hooks). Classify as `vestigial`; action is "scratch-dump entry with cleanup trigger" (the trigger is whoever finishes the unimplemented feature).
- **In-progress migration pattern.** Field is part of a documented incremental D-007 conversion (e.g., emTextField's B3.4c/d). Classify as `in-progress-migration`; action is "cross-reference the migration's owning track; not an FU-004 concern."
- **Closure registries.** Many of these are C++-mirrored callback chains (emRecListener, dialog callbacks). Verify each individually; a registry is only D-009 if it's drained by a Cycle and the C++ analogue dispatches synchronously.

### Unit 3 — Publish row table + in-place adjustments

**Output:** one commit (or two if the in-place changes are large) doing both:

1. **Append the verified row table to the FU-004 bucket file** (or create `…/followups/FU-004-inventory.md` and link from the bucket file's body if size exceeds ~80 lines).
2. **For `C++-mirrored` candidates without an explanatory doc-comment**, add one (~3 lines explaining the C++ shape match). For `forced retention` candidates, add a `DIVERGED:` annotation with the forced category cite. No behavior change in either case.
3. **For `vestigial` candidates**, add scratch-dump entries to `docs/scratch/2026-05-02-future-work-dump.md` with the cleanup trigger.

If the row table classifies any candidates as `D-007 candidate` after honest verification, **do not remediate in this commit.** Note them in the table; remediation gets its own follow-up spec.

## Phase ordering

1. **Phase 1 — Enumeration.** Run greps; produce raw candidate list. Output: scratch notes, no commit.
2. **Phase 2 — Verification.** For each candidate, apply the per-candidate verification process. Output: classified row table draft.
3. **Phase 3 — Publication.** Single commit appending row table + adding in-place comments / annotations / scratch entries.
4. **Phase 4 — Reconciliation.** Update FU-004 bucket file's body with a closure section linking to the inventory. If the inventory is empty (zero D-007 candidates after verification), explicitly note this and recommend FU-004 be marked closed.

## Acceptance criteria

- Row table published with one row per enumerated candidate.
- Every row has a recorded classification with one-line evidence citing file:line.
- Every `C++-mirrored` field has a doc-comment in source (existing or added) explaining the C++ shape match.
- Every `forced retention` field has a `DIVERGED:` annotation with category cite.
- Every `vestigial` field has a scratch-dump entry with cleanup trigger.
- Every `D-007 candidate` is recorded for follow-up spec; not remediated in this work.
- `cargo xtask annotations` clean; `cargo-nextest ntr` green.

## Risk notes

- **Bucket may be empty.** Honest verification could produce a row table where every candidate classifies as `C++-mirrored`, `vestigial`, `in-progress-migration`, or `out-of-scope`. That outcome is a valid result: FU-004's value is then the verification record itself, which prevents future contributors from re-flagging the same patterns. The bucket-file closure section should explicitly state this if it happens.
- **Verification depth varies.** Some candidates (`pending_click_fire`) reveal their classification after one read of the surrounding code. Others (emTextField cluster, with 30+ call sites) require fanning out into setter-by-setter ctx-availability mapping. If a candidate's verification grows past a single focused reading session, classify as "needs deeper audit" and split it into its own follow-up rather than expanding this bucket's scope.
- **Grep heuristics will miss things.** The pattern is structural, not syntactic. The greps in Unit 1 are starting points; manual reads of Cycle bodies for drain patterns will catch fields that don't match the naming heuristics. Phase 1's "filter out" list captures known false positives.

## Out of scope

- All code remediation. If verification surfaces D-007 candidates, those go in a follow-up spec.
- New regression-prevention tooling (CI check / clippy lint / pre-commit hook for new D-009 shapes).
- emTextField B3.4c/d migration progress — that's its own ongoing concern.
- emDialog `pending_result` — Phase 3.6 Task 4 territory.
- Tooling to auto-classify candidates — verification is a manual read process.

## References

- CLAUDE.md §"Polling intermediaries" — D-009 rule statement.
- Tier-B work-order log — buckets B-015/B-016/B-017 each removed concrete D-009 instances; their resolution patterns inform classification here.
- Bucket file: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-004-d009-polling-sweep.md`.
- Brainstorm scratch: `docs/scratch/2026-05-02-future-work-dump.md` — captures un-verified draft candidates and the scoping decision (β).
