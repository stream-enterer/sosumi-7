# Engine Wake Observability — Design

**Date:** 2026-05-03
**Investigation source:** `docs/scratch/2026-05-03-hang-rootcause-findings.md` §"Pending followups"

## Problem

The hang fix (commit `044408b3`, merged `dc7bcbfd`) resolved the GUI hang but left two flagged followups in the post-fix scratch findings:

1. **`has_awake==1` 66.7% of slices at idle.** Phase E showed the wake flag elevated above the plan's ≥50% investigation trigger. Idle CPU is genuinely quiet (paint dropped to 0.4%), so `has_awake==1` is not a performance symptom — but on a port-fidelity project, divergence from C++ behavior is the bug regardless of CPU consequence. We do not know whether C++ exhibits the same idle wake flag.
2. **Cursor blink regression.** A manual GUI check after merge showed cursor in a TextField does not blink visibly. The `Cycle` port wired in by the fix is in place but evidently not producing visible flips. Multiple candidate causes exist along the `focus → Notice → wake → Cycle → cycle_blink → request_invalidate_self → drain → InvalidatePainting → render` chain.

Both symptoms touch the same subsystem (engine registration, the wake queue, `wake_up_engine`, `PanelCycleEngine`). This investigation produces evidence about that subsystem; fix specs (if any) are spawned by the findings, not designed here.

## Out of Scope

- **Fix design.** No fix code in this scope. The investigation produces two findings docs; fix specs (if warranted) are separate brainstorms triggered by what the findings show.
- **C++ comparison.** A1's question "is `has_awake==1` at idle a Rust-only divergence" requires C++-side measurement and is the *next* spec, not this one. A1 here ends with offender names only.
- **Tile-level RENDER instrumentation beyond what already exists.** `instr/hang-2026-05-02` already brackets paint/present cycles; no extension needed.
- **Production-binary capture as the default path.** Per user preference, A2 captures against test-panel TextFields. A production-binary follow-up is a contingency, not a default.

## Architecture & Scope

One instrumentation diff on `instr/hang-2026-05-02` (no merge to main). Two captures, two findings docs:

- **A1** (idle, 60s, two SIGUSR1 markers): identify engine types staying awake or being externally re-woken at idle. Output: name(s) of offenders. C++ comparison out of scope.
- **A2** (test-panel TextField focused, single ~90s window with one open + one close marker bracketing pre-click idle, click moment, and ~60s focused steady-state): path-trace the `focus → Notice → wake → Cycle → cycle_blink flip → request_invalidate_self → drain → InvalidatePainting → tile dirty → render` chain. Output: identify the broken link.

Investigation ends when both findings docs exist on `main`.

## Instrumentation Surface

Seven new log line types on top of the existing `wall_us`/`SLICE`/`CB`/`AW`/`RENDER`/`MARKER` infrastructure. Existing instrumentation unchanged.

| Line | Where it fires | Fields |
|---|---|---|
| `REGISTER` | `Scheduler::register_engine<E>` once per registration; captures `std::any::type_name::<E>()` at the monomorphized call site, stores `&'static str` in the engine record | `wall_us`, `engine_id`, `engine_type`, `scope` |
| `STAYAWAKE` | `Scheduler::DoTimeSlice` after `let stay_awake = match scope { ... }` | `wall_us`, `slice`, `engine_id`, `engine_type`, `stay_awake=t/f` |
| `WAKE` | `Scheduler::wake_up_engine` (bottom layer; `wake_up_panel` propagates through `#[track_caller]`) | `wall_us`, `slice`, `engine_id`, `engine_type`, `caller=<file:line>` |
| `NOTICE` | Notice dispatch site(s); logs **all** Notices — analyzer filters. Audit task identifies the dispatch points; if there is more than one (e.g., separate paths for cross-panel vs. intra-panel delivery), instrumentation is added at each | `wall_us`, `slice`, `recipient_panel_id`, `recipient_type`, `flags=<bits>` |
| `BLINK_CYCLE` | `TextFieldPanel::Cycle` after the `cycle_blink` call | `wall_us`, `slice`, `engine_id`, `panel_id`, `focused`, `flipped`, `busy` |
| `INVAL_REQ` | `PanelCtx::request_invalidate_self` (added in the fix); `#[track_caller]` captures originator | `wall_us`, `slice`, `engine_id`, `panel_id`, `source=<caller>` |
| `INVAL_DRAIN` | `PanelCycleEngine` drain check (added in the fix) | `wall_us`, `slice`, `engine_id`, `panel_id`, `drained=t/f` |

### Sub-decisions baked in

- **WAKE caller via `#[track_caller]` + `Location::caller()`.** Zero invasive call-site changes; captures original calling frame including indirection through `wake_up_panel`.
- **NOTICE logs all flag types,** not only `FOCUS_CHANGED`. Volume is low at idle (Notice traffic is signal-driven, not continuous). Logging all flags catches unknown-unknown causes — e.g., a Notice arriving without `FOCUS_CHANGED` set when it should have, or a different flag toggling `is_focused` incorrectly.
- **`INVAL_REQ` and `BLINK_CYCLE` both kept,** even though `flipped=true` in `BLINK_CYCLE` should always coincide with an `INVAL_REQ`. Keeping both lets the analyzer detect a refactor regression where `cycle_blink` flips but doesn't request.

### Type-name plumbing — known unknown

This spec assumes `Scheduler::register_engine` takes a generic `E: EngineBehavior + 'static` so `std::any::type_name::<E>()` works at the monomorphized call site. The implementation plan includes an audit task: grep all `register_engine` call sites; if a pre-erased `Box<dyn EngineBehavior>` registration entry point exists, add a `register_engine_dyn(behavior: Box<dyn EngineBehavior>, name: &'static str, scope) -> EngineId` parallel where the caller passes the name. Spec calls this out explicitly so the implementer doesn't quietly guess.

### Volume sanity

Idle 60s capture: a few hundred lines total. Blink 90s capture: similar order, bounded because the user holds still after clicking. Existing shared-FD logging handles this trivially.

## Capture Protocol

Two captures, one log file each, written to `/tmp/em_instr.<phase>.log` via the existing shared-FD plumbing.

### A1 — Idle (`/tmp/em_instr.idle.log`)

1. Build release: `cargo build -p eaglemode --release` on `instr/hang-2026-05-02` at the new tag (see §Branch + Commit Strategy).
2. Launch GUI (existing `scripts/run_hang_capture.sh` or equivalent) with `EM_INSTR_FD` set.
3. Wait until cosmos is fully painted, mouse parked away from any panel boundary, no movement.
4. `kill -USR1 <pid>` (open marker).
5. Wait **60 s**, no input, no mouse movement.
6. `kill -USR1 <pid>` (close marker).
7. Kill GUI cleanly. Verify two `MARKER` lines in log.

### A2 — Test-panel TextField focused (`/tmp/em_instr.blink.log`)

1. Build & launch as above.
2. Navigate to the runtime test panel that exposes TextField widgets (`crates/emtest/src/emTestPanel.rs`). The implementation plan names the specific TextField by its visible label so captures are repeatable across runs.
3. Position so the TextField is fully visible, no mouse movement after this point until click.
4. `kill -USR1 <pid>` (open marker).
5. Wait **5 s** of pre-click idle.
6. Single click into the TextField.
7. Hold **60 s** focused-idle: no typing, no mouse movement. Cursor should visibly blink during this window if the fix is working — the very thing under test.
8. `kill -USR1 <pid>` (close marker).
9. Kill GUI. Verify two `MARKER` lines.

### Why a single window with the click mid-bracket

Bracketing only the steady states (separate "before click" + "after click" captures) drops the focus-change *transition* — and the transition is precisely where multiple candidate causes (Notice delivery, focus-flag computation) get answered. The analyzer identifies the click moment from the event stream and slices the window into pre-click / transition / post-click regions for separate aggregation.

### Validation gates

Run before analysis; fail the capture if violated:

- Exactly two `MARKER` lines.
- At least one `REGISTER` line (sanity check that instrumentation initialized).
- A2 only: at least one `NOTICE` with `FOCUS_CHANGED` flag set within the window. Absence means the click didn't land on a TextField; capture is invalid, re-run.

### Capture script

New `scripts/run_blink_capture.sh` sets `EM_INSTR_FD`, launches the GUI, prints click-timing instructions to stdout, otherwise does nothing automatic. The user sends SIGUSR1 by hand. Existing `scripts/run_hang_capture.sh` covers A1 unchanged.

### Contingency — A2 test/production divergence

If A2's findings doc reads "every step in the path-trace fired correctly" but the user still observes no blink in the default `eaglemode` binary, run an **A2-prod** follow-up capture: same instrumentation tag, repeat protocol against the first reachable TextField in the default binary's startup cosmos. Same analyzer command. This is **not** an instrumentation diff; it's a re-run with a different click target. Lives as an "if needed" task in the implementation plan.

## Analyzer Extensions

Extend `scripts/analyze_hang.py` with two new top-level commands:

```
analyze_hang.py idle <log>   # produces A1 findings doc skeleton
analyze_hang.py blink <log>  # produces A2 findings doc skeleton
```

Both write a Markdown report to stdout; the user pastes into the findings doc, edits the Verdict / Next-steps sections, commits.

### `idle` command

Per-engine-type aggregation across the marker-bracketed window:

| Column | How computed |
|---|---|
| `engine_type` | from `REGISTER` records, indexed by `engine_id` |
| `cycles` | count of `STAYAWAKE` lines per `engine_type` |
| `stay_awake_pct` | `(stay_awake=t count) / cycles` |
| `ext_wakes` | count of `WAKE` lines for that `engine_id` where the most-recent prior `STAYAWAKE` returned `f` (engine was asleep, then re-woken externally) |
| `classification` | `self-perpetuating` if `stay_awake_pct ≥ threshold`; `externally-rewoken` if `ext_wakes ≥ threshold * slice_count`; `episodic` otherwise; `never-awake` if `cycles == 0` |

Threshold default `0.80`, configurable via `--threshold`. Offender list = any non-`episodic`, non-`never-awake` row.

For each externally-rewoken offender, also produce a caller breakdown: group `WAKE.caller` field, count, sort. Names the source that keeps re-arming it.

Output ends with `Next step: spec B1 — compare {offenders} to C++ ground truth.` placeholder.

### `blink` command

Three-stage analysis:

1. **Locate focus-change moment.** Scan post-open-marker events for the first `NOTICE` line with `FOCUS_CHANGED` bit set whose `recipient_type` is `TextFieldPanel`. Record `t_focus`, `recipient_panel_id`, `recipient_engine_id` (resolved by joining `recipient_panel_id` against `REGISTER` records for `PanelCycleEngine` with matching scope).

2. **Path-trace verdict at transition.** For each link, emit `✓` or `✗` with timestamp and the relevant log line, in this fixed order:
   - `NOTICE FOCUS_CHANGED → TextFieldPanel`
   - `WAKE → PanelCycleEngine for that panel`
   - (registration check: was the engine `REGISTER`'d at all? Reported separately if no, since absence implies the wake target doesn't exist.)
   - `STAYAWAKE for that engine within 1 slice of the wake`
   - `BLINK_CYCLE | focused=true`
   - `BLINK_CYCLE | flipped=true at expected ~500ms cadence`
   - `INVAL_REQ from cycle_blink`
   - `INVAL_DRAIN | drained=true`
   - `RENDER:paint` event covering the panel's screen region (cross-reference existing render lines)

   First `✗` is the broken link. Report it with surrounding context.

3. **Steady-state aggregation (post-click region).** Counts of `BLINK_CYCLE` `flipped=true` events, `INVAL_REQ`, `INVAL_DRAIN drained=true`, render paints. Expected: ~120 flips in 60s (one per 500ms). A non-zero but wrong cadence (e.g., 0 flips, or 1200 flips) is itself diagnostic.

Output ends with `Next step: spec B2 — investigate {first broken link}.` placeholder.

### Validation pre-pass

Both commands assert exactly two `MARKER` lines, at least one `REGISTER`, and (for `blink`) at least one `NOTICE FOCUS_CHANGED` to a `TextFieldPanel`. If any fails, the analyzer prints `capture invalid: rerun` and exits non-zero — does not produce a misleading partial report.

### Volume of analyzer changes

~80–120 lines of Python: parsers for 7 new line types, `idle` command, `blink` command.

## Findings Doc Templates

Two findings docs, one per capture, written to `docs/scratch/`:

- `<YYYY-MM-DD>-has-awake-findings.md` (A1)
- `<YYYY-MM-DD>-blink-findings.md` (A2)

Date is the day the capture is run, not the spec date.

### A1 template

```markdown
# has_awake idle findings — YYYY-MM-DD
Capture: /tmp/em_instr.idle.log
Branch: instr/hang-2026-05-02 @ <tag>
Threshold: <X%>

## Window
<slices>, <wall_seconds>s

## Per-engine-type aggregation
<analyzer table — paste verbatim>

## Offenders
<analyzer offender list — paste verbatim>

## External-wake caller breakdown
<one block per externally-rewoken offender — paste from analyzer>

## Verdict
<one paragraph: which engines, classification, whether the user
considers this a candidate-for-fix or wait-for-C++-comparison>

## Next steps
- [ ] Spec B1 — C++ comparison for {offender list}
- [ ] OR: defer; document rationale
```

### A2 template

```markdown
# Blink path-trace findings — YYYY-MM-DD
Capture: /tmp/em_instr.blink.log (test-panel TextField: <name>)
Branch: instr/hang-2026-05-02 @ <tag>

## Window
Marker-open: <wall_us>; click: t_focus=<wall_us>; marker-close: <wall_us>
Pre-click: <Xs>; transition: instantaneous; post-click: <Ys>

## Path-trace verdict (transition)
<analyzer ✓/✗ list with evidence lines — paste verbatim>

## Steady-state aggregation (post-click)
<analyzer counts table — paste verbatim>

## Identified break
<analyzer's "first ✗" — paste, plus one human paragraph interpreting>

## Contingency check
<if every step is ✓ but blink still not visually working, run A2-prod
capture against default eaglemode binary's first reachable TextField;
re-run analyzer; if results differ → hypothesis #9 confirmed (test/prod
divergence). Document outcome here.>

## Next steps
- [ ] Spec B2 — fix targeted at {broken-link layer}
- [ ] OR: A2-prod follow-up capture
- [ ] OR: defer; document rationale
```

### Why doc templates this rigid

1. The findings docs are the *only* artifact this investigation produces. Their structure determines whether the next session — possibly weeks later, possibly a fresh agent — can act on them without re-deriving context. A consistent skeleton with explicit "Next steps" checkboxes makes them self-executable.
2. The "Verdict" / "Identified break" sections are *short* by design. The analyzer produces evidence; the human writes one paragraph of interpretation. This bounds how much the human can drift the conclusion away from what the data actually says.

## Acceptance Criteria & Exit Conditions

### Acceptance criteria

- **A1**: for each `engine_type` observed in the idle capture, classify as `{self-perpetuating, externally-rewoken, episodic, never-awake}`. Offenders = any `engine_type` where `stay_awake=true` ratio ≥ threshold (default 80%, configurable) OR external re-wake ratio ≥ threshold across slices in window. Findings doc names them. **C++ comparison out of scope.**
- **A2**: trace `focus → Notice → wake → Cycle → cycle_blink → request_invalidate_self → drain → InvalidatePainting → render` for the post-click region. Identify the first `✗` link with evidence. **Fix design out of scope.**

### Exit conditions — investigation is done when all hold

1. Two captures committed validated (analyzer's pre-pass exits zero on each).
2. Two findings docs exist on `main`, each with: filled Verdict paragraph, filled Next-steps checklist, capture log path + branch tag referenced.
3. The instrumentation branch's new commits have not been merged to `main` (validate via `git log main..instr-7-loop-chain --oneline` showing only the new commits).

### Hard rules — investigation is not done while any holds

- Either capture invalid (missing markers, missing `REGISTER`, A2 missing `FOCUS_CHANGED`).
- Either findings doc lacks Verdict or Next-steps.
- Any fix code committed (instrumentation drift into fix work is the v2 anti-pattern; spec rejects a "while we're at it" patch).
- A2-prod contingency triggered but not run to completion (if A2 verdict reads "every step ✓ but blink not working," prod follow-up must run before this investigation closes).

### Threshold discipline

`0.80` is a default, not load-bearing. Analyzer accepts `--threshold` for re-classification without re-capturing. Findings doc records the threshold used so the call is reproducible.

## Branch + Commit Strategy

### Instrumentation commits — `instr/hang-2026-05-02`

Two commits on the existing instrumentation branch, in order:

1. `instr: phase A 7-LOOP-CHAIN — engine wake observability (REGISTER/STAYAWAKE/WAKE/NOTICE/BLINK_CYCLE/INVAL_REQ/INVAL_DRAIN)` — Rust source diff. Includes `register_engine` type-name capture, scheduler `STAYAWAKE` line, `wake_up_engine` `#[track_caller]` + `WAKE` line, Notice dispatch `NOTICE` line, `TextFieldPanel::Cycle` `BLINK_CYCLE` line, `request_invalidate_self` `INVAL_REQ` line, `PanelCycleEngine` drain `INVAL_DRAIN` line, and the `register_engine_dyn` parallel entry point if the audit task finds pre-erased registrations.
2. `instr: analyze_hang.py — idle and blink commands` — Python analyzer extension only.

Source change is reviewable independently of the analyzer; if either fails review or build, the other isn't blocked.

After both commits, **tag the branch HEAD** as `instr-7-loop-chain`. Findings docs reference the tag, not the SHA — safer against future history rewrites on the branch.

**No merge to main.** Validate at investigation close via `git log main..instr-7-loop-chain --oneline` showing exactly the two new commits.

### Findings docs — direct to `main`

Match the prior session's pattern (`docs/scratch/2026-05-03-hang-rootcause-findings.md` was committed directly to main). Two small commits to `main`:

- `scratch: A1 has_awake idle findings — engine wake observability capture`
- `scratch: A2 blink path-trace findings — engine wake observability capture`

No long-lived branch needed; findings are self-contained docs that don't depend on the instrumentation source.

### Capture log files — not committed

`/tmp/em_instr.idle.log` and `/tmp/em_instr.blink.log` stay local. Findings docs reference them by path and by the `instr-7-loop-chain` tag. Reproducibility comes from re-running the capture against the tagged branch, not from archival of one-shot logs.

## Risk Register

Forward-looking failure modes and their mitigations, consolidated:

- **Code-reading rabbit hole.** Reading "all C++ engines that could idle awake" without a target is unbounded. *Mitigation:* instrumentation comes first; named engines from instrumentation make any subsequent C++ comparison bounded. C++ comparison is explicitly the next spec, not this one.
- **Investigation drifts into a fix.** "While we're at it" patches violate scope. *Mitigation:* exit condition checks no fix code committed. Fix specs branch off `main`, never from `instr/hang-2026-05-02`.
- **Threshold cherry-picking.** Setting `--threshold` after seeing the data to engineer a desired offender list. *Mitigation:* default `0.80` is documented; findings doc records the actual threshold; deviations require a paragraph of justification.
- **Test/production divergence (hypothesis #9).** A2's test-panel capture might show blink working while the default binary doesn't. *Mitigation:* A2-prod contingency capture, gated by A2's path-trace verdict.
- **Pre-erased Box<dyn> registration breaks type_name plumbing.** *Mitigation:* implementation plan includes an audit task; if pre-erased entries exist, add `register_engine_dyn(behavior, name, scope)` parallel.
- **Notice volume swamps the log.** *Mitigation:* idle Notice traffic is signal-driven and low; if a capture produces unexpected NOTICE volume, that itself is a finding worth reporting.
- **Tag drift if commits get amended.** *Mitigation:* tag is created **after** both commits land and after a working capture validates the instrumentation; if the implementer needs to amend, they re-tag.
- **Statistical underpower.** Phase E's 9 slices in 20s was barely distinguishable from noise. *Mitigation:* 60s captures, 30+ slices target.
- **False ✓ in path-trace.** A line could fire for the wrong reason (e.g., `NOTICE FOCUS_CHANGED` to a different TextField than the one clicked). *Mitigation:* each ✓/✗ in the analyzer output includes the specific log line with `panel_id`/`engine_id` so a human cross-checks.
- **Hidden engine in `episodic` classification.** An engine doing 50% `stay_awake=true` returns wouldn't trigger the offender threshold but might still be a divergence. *Mitigation:* analyzer always prints the full per-engine-type table sorted by `stay_awake_pct` desc; non-offender rows visible to the reviewer.

## Annotations

This investigation adds instrumentation only. All new lines and storage in production code are tagged `RUST_ONLY: (language-forced-utility)` matching the existing `emInstr.rs` plumbing — instrumentation primitives have no C++ analogue because Eagle Mode upstream doesn't ship phase-0 hang-capture infrastructure.

The `register_engine_dyn` parallel entry point (if added by the audit task) is also `RUST_ONLY: (language-forced-utility)`: a side-channel name parameter is required because Rust trait objects do not preserve concrete-type names through vtables, while C++ has full RTTI.

No `DIVERGED:` annotations needed; no behavior changes ship.
