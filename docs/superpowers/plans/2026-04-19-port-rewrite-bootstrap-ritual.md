# Port Ownership Rewrite — Shared Bootstrap & Closeout Ritual

> **For agentic workers:** This is not an executable plan by itself. It defines the preamble (Bootstrap) and postamble (Closeout) that every phase plan in the port-rewrite series cites verbatim. Do not skip any step. Steps use `- [ ]` syntax so an executor can tick them off per phase.

**Companion spec:** `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md`
**Companion raw material:** `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json`

**Scope.** This ritual exists so a series of phase plans (Phase 1, 2, 3, 4a, 4b, 4c, 4d, 5) can run back-to-back unattended with deterministic entry and exit state. Each phase plan includes two headers — `## Bootstrap (per shared ritual)` and `## Closeout (per shared ritual)` — whose bodies reference steps B1–B12 and C1–C11 defined here.

---

## Bootstrap — steps B1–B12

Run at the *very first line* of every phase plan, before any task.

- [ ] **B1. Read the spec.** Read `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` from the top. Do not skim. You will reference specific section numbers (§3.1, §4 D4.4, etc.) throughout.

- [ ] **B2. Read the raw-material JSON.** Read `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json`. Note the `id` field of every entry this phase closes (listed in the phase plan's header). You will verify resolution at Closeout.

- [ ] **B3. Read the scheduler workaround ledger.** Read `docs/superpowers/notes/2026-04-19-scheduler-refcell-workaround-ledger.md`. It enumerates the 12 B01 mechanisms whose deletion is gated by Phase 1; later phases inherit the clean state.

- [ ] **B4. Read the prior-phase closeout note (if any).** Locate `docs/superpowers/notes/2026-04-19-phase-<N-1>-closeout.md`. If it exists:
    - Verify `## Status` line reads `COMPLETE — all C1–C11 checks passed`.
    - If it reads anything else, STOP the plan immediately. Write a new note `docs/superpowers/notes/2026-04-19-phase-<N>-bootstrap-blocked.md` with the prior-closeout state and halt. Do not proceed under an incomplete predecessor.
    - If the phase is Phase 1, skip this step (no predecessor).

- [ ] **B5. Verify clean working tree.**

    Run: `git status --porcelain`
    Expected output: empty.
    If non-empty, STOP. Write `docs/superpowers/notes/2026-04-19-phase-<N>-bootstrap-blocked.md` recording the dirty files and halt.

- [ ] **B6. Verify main branch alignment.**

    Run: `git rev-parse --abbrev-ref HEAD`
    Expected: `main`. (Phase branches are created in B9.)
    If on any other branch: STOP. Do not proceed.

- [ ] **B7. Record entry baselines.**

    Run each and capture stdout:
    ```bash
    cargo-nextest ntr 2>&1 | tail -5
    cargo test --test golden -- --test-threads=1 2>&1 | tail -5
    cargo clippy --all-targets --all-features 2>&1 | tail -3
    rg -c 'Rc<RefCell<' crates/ | awk -F: '{sum += $2} END {print "rc_refcell_total:", sum}'
    rg -c 'DIVERGED:' crates/ | awk -F: '{sum += $2} END {print "diverged_total:", sum}'
    rg -c 'RUST_ONLY:' crates/ | awk -F: '{sum += $2} END {print "rust_only_total:", sum}'
    rg -c 'IDIOM:' crates/ | awk -F: '{sum += $2} END {print "idiom_total:", sum}'
    rg -c 'try_borrow' crates/ | awk -F: '{sum += $2} END {print "try_borrow_total:", sum}'
    ```

    Write the captured numbers to `docs/superpowers/notes/2026-04-19-phase-<N>-baseline.md` under headings `nextest`, `goldens`, `clippy`, `rc_refcell_total`, `diverged_total`, `rust_only_total`, `idiom_total`, `try_borrow_total`.

    **Ground-truth reference (measured 2026-04-19, pre-Phase-1):** `rc_refcell_total=284`, `try_borrow_total=11`, `diverged_total=177`, `rust_only_total=16`, `idiom_total=1`. Phase 1's Bootstrap baseline should match these within a small drift window. If Phase 1's measured baseline diverges from these reference numbers by > 10% on any metric, STOP and investigate — the spec/plan numerics may need re-baselining, or recent unrelated work shifted the tree. Later phases inherit the prior phase's exit numbers, not these reference numbers.

- [ ] **B8. Verify baseline is green.**

    Inspect the `nextest` and `goldens` captures from B7. Nextest must show `0 failed`. Goldens must show `237 passed; 6 failed` (the known baseline from the 2026-04-18 emview closeout) OR strictly better (more passed, no regressions in previously-passing tests). Clippy must exit 0.

    If any check is worse than baseline, STOP. The predecessor phase shipped a regression; do not build on top of it.

- [ ] **B9. Create phase branch.**

    Run: `git checkout -b port-rewrite/phase-<N>`
    (Replace `<N>` with the phase number; use `phase-4a`, `phase-4b`, etc. for sub-phases.)

- [ ] **B10. Create phase ledger file.**

    Write `docs/superpowers/notes/2026-04-19-phase-<N>-ledger.md` with frontmatter:
    ```markdown
    # Phase <N> — <short title> — Ledger

    **Started:** <YYYY-MM-DD HH:MM local>
    **Branch:** port-rewrite/phase-<N>
    **Baseline:** see 2026-04-19-phase-<N>-baseline.md
    **Spec sections:** <list from phase plan header>
    **JSON entries to close:** <list from phase plan header>

    ## Task log

    <empty — tasks append here as they complete>
    ```

- [ ] **B11. Commit bootstrap artifacts.**

    Run:
    ```bash
    git add docs/superpowers/notes/2026-04-19-phase-<N>-baseline.md docs/superpowers/notes/2026-04-19-phase-<N>-ledger.md
    git commit -m "phase-<N>: bootstrap — baseline captured, ledger opened"
    ```

- [ ] **B12. Announce phase start.** Output to the user (or log, in unattended mode):

    `Phase <N> bootstrap complete. Branch: port-rewrite/phase-<N>. Baseline: <nextest count>, <golden count>. Beginning Task 1.`

---

## Closeout — steps C1–C11

Run after the final task of every phase plan, before merging back.

- [ ] **C1. Run full gate.** All three must pass:

    ```bash
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo-nextest ntr
    cargo test --test golden -- --test-threads=1
    ```

    Capture stdout. If any fails, STOP. Fix the failure in a new commit, then re-run C1. Do not proceed to C2 on red.

- [ ] **C2. Record exit counts.**

    Same commands as B7. Write to `docs/superpowers/notes/2026-04-19-phase-<N>-exit.md`.

- [ ] **C3. Diff baseline vs exit.**

    Compute (exit − baseline) for each metric. Write the diff under a `## Delta` section of the exit note:
    - `nextest`: must be ≥ 0 (new tests may be added; never removed on net).
    - `goldens passed`: must be ≥ baseline.
    - `goldens failed`: must be ≤ baseline.
    - Phase-specific metric targets (stated in the phase plan's Closeout — e.g. "Phase 1: `try_borrow_total` delta ≤ −40").

- [ ] **C4. Verify phase-specific invariants.** Each phase plan lists invariants I-list that must hold at exit (grep-enforceable assertions — e.g. "zero `Rc<RefCell<EngineScheduler>>` matches anywhere in `crates/`"). Run each assertion and record pass/fail.

- [ ] **C5. Verify JSON entries closed.** For each `id` this phase committed to closing (from the phase plan header), add a line to `docs/superpowers/notes/2026-04-19-phase-<N>-exit.md` under `## JSON entries closed` citing: `<id>: <commit sha that closed it> — <one-line evidence>`. Use the JSON entry's `how_to_verify` field where present.

- [ ] **C6. Update the raw-material JSON.** For each closed entry, mark its `status` field to `resolved-phase-<N>` and add a `resolution_commit` field. Commit the JSON change in a dedicated commit: `phase-<N>: mark JSON entries <list> resolved`.

- [ ] **C7. Write closeout note.**

    Write `docs/superpowers/notes/2026-04-19-phase-<N>-closeout.md`:

    ```markdown
    # Phase <N> — <title> — Closeout

    **Branch:** port-rewrite/phase-<N>
    **Commits:** <sha-range>
    **Status:** COMPLETE — all C1–C11 checks passed

    ## Summary

    <3–5 sentences of what shipped and why it's green>

    ## Delta from baseline

    <paste from exit.md>

    ## JSON entries closed

    <list of ids with resolution commits>

    ## Spec sections implemented

    <list of §-numbers from spec>

    ## Invariants verified

    <list of I-assertions run, each with pass/fail>

    ## Next phase

    Phase <N+1> — see `docs/superpowers/plans/2026-04-19-port-rewrite-phase-<N+1>-*.md`.
    ```

- [ ] **C8. Commit closeout artifacts.**

    ```bash
    git add docs/superpowers/notes/2026-04-19-phase-<N>-exit.md docs/superpowers/notes/2026-04-19-phase-<N>-closeout.md
    git commit -m "phase-<N>: closeout — all gates green, entries resolved"
    ```

- [ ] **C9. Merge phase branch into main.** No rebase; preserve the phase's commit history.

    ```bash
    git checkout main
    git merge --no-ff port-rewrite/phase-<N> -m "Merge phase-<N>: <short title>"
    ```

- [ ] **C10. Tag phase completion.**

    ```bash
    git tag port-rewrite-phase-<N>-complete
    ```

- [ ] **C11. Announce phase complete.** Output:

    `Phase <N> closeout complete. All gates green. Tagged port-rewrite-phase-<N>-complete. Next phase may begin.`

    In unattended mode, the next phase's plan may now run Bootstrap.

---

## Failure-handling conventions

Each step above that says "STOP" is a hard halt. The executor must:

1. Write the halt diagnostic to `docs/superpowers/notes/2026-04-19-phase-<N>-bootstrap-blocked.md` (during Bootstrap) or `docs/superpowers/notes/2026-04-19-phase-<N>-gate-failed.md` (during Closeout).
2. Emit the halt message to the user/log.
3. Not attempt to repair or bypass. Unattended mode is sequential; later phases depend on all earlier phases' Closeouts being COMPLETE.

Mid-phase failures (inside a Task, not in the ritual) are handled per the phase plan's Task instructions. A phase plan may include recovery steps inline, but the ritual itself does not.

---

## Why this ritual exists

The unattended workflow has three requirements the ritual enforces:

1. **Deterministic entry.** B4/B5/B6/B8 guarantee a phase never starts on a dirty or red tree.
2. **Traceable exit.** C2/C5/C6 make every phase's resolution verifiable; no "I think I fixed that" lingering.
3. **Cliff-shaped transitions.** C9's `--no-ff` merge + C10's tag produce a visible step in the git history, and C1's gate guarantees each cliff is green. A later phase reading `git log main` can trace "phase 1 here, phase 2 here" and bisect against phase boundaries.

Skipping any step breaks the chain. The ritual is deliberately mechanical so an executor (human or agent) can run it without judgment calls.

---

## Claimed-safe simplifications *not* taken

To make the rationale explicit:

- **Not using submodules or worktrees per phase.** The workflow assumes a single working tree and a sequential phase chain. A parallel-branch strategy would require different coordination and was out of scope when the spec was written.
- **Not using CI for gates.** Local `cargo` runs are authoritative. CI may run in parallel but its result is not consulted by this ritual. This is because the workflow is meant to run inside an agent session, not a CI runner.
- **Not using issue-tracker integration.** Entry resolution lives in the JSON, not an external tracker. This is a deliberate choice for the port subsystem; future phases may revisit.
