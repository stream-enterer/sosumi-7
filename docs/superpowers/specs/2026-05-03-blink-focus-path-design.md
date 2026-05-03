# Blink focus-path fix — design (B2)

> Spec for the cursor-blink regression on focused TextField panels, identified
> in the engine wake observability investigation (A2). Companion spec to B1
> (`has_awake` cadence) which addresses a different, independent finding from
> the same investigation.

**Status:** spec ready for plan.
**Authority:** C++ source > golden tests > Rust idiom (per `CLAUDE.md`).
**Predecessors:**
- `docs/scratch/2026-05-03-blink-findings.md` — A2 path-trace findings.
- `docs/scratch/2026-05-03-has-awake-findings.md` — A1 findings (separate, for B1).
- `docs/superpowers/specs/2026-05-03-engine-wake-observability-design.md` — investigation spec.

---

## Problem

After the 044408b3 fix (port of `emTextField::Cycle`), TextField cursor blink
is still observably broken on focused panels. The A2 path-trace identified
the chain break inside `TextFieldPanel::notice`: the FOCUS_CHANGED notice
arrives, but the `if self.is_focused { ... ctx.wake_up_panel(id); }` branch
never fires, so the engine never wakes, so `Cycle` never advances the blink
state.

The A2 findings doc proposed a hypothesis: `state.in_focused_path()` returns
`false` on the panel that just gained focus, due to a focused-path
bookkeeping vs. notice-dispatch ordering divergence from C++.

During this spec's exploration, I (the controller) found that:

1. The hypothesis is **supported by the data** — zero WAKE log entries with
   caller-line in the `wake_up_panel(id)` call site range
   (`crates/emtest/src/emTestPanel.rs:240-245`) across the entire 60s capture.
   The branch was never taken.

2. The hypothesis is **not yet supported by code reading** —
   `set_active_panel` (emView.rs:1758-1793) flips `in_active_path` *before*
   queueing notices, and `SetFocused` (emView.rs:802-835) flips
   `window_focused` *before* queueing notices, both matching C++ ordering.
   `PanelState::in_focused_path()` (emPanel.rs:174-176) is a simple AND of
   the two flags.

3. There is a separate, independent divergence in the same vicinity:
   `set_active_panel` (emView.rs:1791-1793) calls
   `tree.queue_notice(id, flags, None)` — `None` for sched, so the
   `add_to_notice_list` wake branch is dead. C++ `SetActivePanel`
   (emView.cpp:307) explicitly calls `UpdateEngine->WakeUp()`. This is real
   and confirmed, but did not cause the blink symptom in the A2 capture
   (the NOTICE entry did appear, so the engine was woken from elsewhere).

The data and code reading disagree. CLAUDE.md authority order says trust
the data. There is therefore an ordering or snapshot bug in the focus-path
system that I have not yet localized by reading. The spec's first phase
measures it directly.

---

## Goal

Restore visible cursor blink on focused TextFields, and add a regression
test that catches the focus-path / engine-wake bug class so it cannot
silently recur.

## Non-goals

- Fix the missing `WakeUpUpdateEngine` divergence in `set_active_panel`
  (D1 — deferred to B1).
- Improve the analyzer's `PanelCycleEngine`→panel mapping heuristic
  (D2 — deferred indefinitely).
- Audit other focus-path-dependent widgets for the same shape
  (D3 — deferred; covered structurally by the new regression test).

---

## Architecture overview

Spec is structured as **measure-then-fix**:

```
Phase 0: Instrument + Capture + Analyze
  └─ outcome ∈ {O1: in_active_path=false at notice time,
                O2: window_focused=false at notice time,
                O3: both true & branch fires,
                O4: notice not delivered}

Dispatch:
  O1 → Phase 1a: ordering fix in set_active_panel / build_panel_state
  O2 → Phase 1b: ordering fix in SubViewPanel::Input / SetFocused
  O3 → Re-brainstorm B2.1; bug is elsewhere (Cycle, InvalidatePainting, etc.)
  O4 → Re-brainstorm B2.1; notice dispatch itself is broken

Phase 1 (a or b only, for O1/O2):
  - Fix the ordering issue (≤30 LOC, 1-2 commits)
  - Add regression test in crates/emcore/tests/
  - Verify with manual recapture (BLINK_CYCLE entries appear)
  - Land on main as a normal merge

Out of scope (deferred): D1, D2, D3.
```

**Branches.**
- Phase 0 instrumentation: `instr/blink-trace-2026-05-03`, cut from existing
  tag `instr-7-loop-chain`. Never merges to main.
- Phase 1 fix: `fix/blink-focus-path-2026-05-03`, cut from main.
  Standard merge.
- Findings docs and deferral notes commit directly to main as documentation.

**Prediction commitment** (advisor's calibration check). Before measurement:
60% O1, 25% O2, 10% O3, 5% O4. After Phase 0 lands the actual outcome,
record the actual verdict in the Phase 0 Results appendix below for
retrospective calibration.

---

## Phase 0 — Instrumentation design

### Three new structured log lines

Same `crates/emcore/src/emInstr.rs` `write_line()` infra as the existing
A1/A2 instrumentation. Lines land in the same `/tmp/em_instr.<tag>.log`
output and the analyzer parser extends naturally.

**Line 1: `NOTICE_FC_DECODE`**

Emitted at the top of every panel's `notice` invocation when
`flags.intersects(FOCUS_CHANGED)`. Emitted unconditionally — no
`if self.is_focused` gate, since that's the very thing being measured.

Location: `emView::handle_notice_one` (emView.rs:~4187), immediately before
`behavior.notice(flags, &state, &mut ctx)`. Tree-level rather than
per-behavior so it captures every panel, ruling out "wrong panel"
confusion.

```
NOTICE_FC_DECODE|wall_us=...|panel_id=...|behavior_type=...|in_active_path=t|window_focused=t|flags=0xf0
```

**Line 2: `SET_ACTIVE_RESULT`**

Emitted at the end of every `set_active_panel` call, both outer-view and
sub-view paths.

Location: `emView::set_active_panel` (emView.rs:~1791), after the
`for id in notice_ids { tree.queue_notice(...) }` loop.

```
SET_ACTIVE_RESULT|wall_us=...|target_panel_id=...|window_focused=t|notice_count=N|sched_some=f
```

`sched_some` captures whether the scheduler was passed to `queue_notice`.
Always `f` per current code, but cheap to log.

**Line 3: `SET_FOCUSED_RESULT`**

Emitted at the end of every `view.SetFocused` call.

Location: `emView::SetFocused` (emView.rs:~835), after the
`for (id, flags) in notice_list` loop.

```
SET_FOCUSED_RESULT|wall_us=...|view_kind=outer|focused=t|panels_notified=N
```

`view_kind` is `outer` or `subview` — derive by checking the view's
`update_engine_id` scope (Toplevel vs SubView), or thread a one-bool
ctor flag.

### Why these three are sufficient

Together they form a closed system that lets the analyzer answer:

1. **Did `set_active_panel` run on the textfield's tree?**
   `SET_ACTIVE_RESULT` with `target_panel_id` matching the clicked TextField.
2. **Was `sub_view.window_focused` true at queue time?**
   `SET_FOCUSED_RESULT|view_kind=subview|focused=t` preceding
   `SET_ACTIVE_RESULT` for the textfield.
3. **What were `in_active_path` and `window_focused` at notice delivery
   time?** `NOTICE_FC_DECODE`.

The four outcomes (O1/O2/O3/O4) become a clean truth-table read off these
three lines plus the existing WAKE entries.

### Analyzer changes

Extend `scripts/analyze_hang.py` `blink` command:

- Parse the three new line types.
- Add a "Phase 0 verdict" section to the report: for each focus-change
  event on the clicked TextField, print the
  `(in_active_path, window_focused, branch_fired)` triple and emit one of
  `{O1, O2, O3, O4}`.
- "Branch fired" detection: presence of a WAKE entry with caller-line in
  the configured range (default: `crates/emtest/src/emTestPanel.rs:240-245`,
  CLI flag to override) within a small wall-time window
  (default 100ms) after the corresponding `NOTICE_FC_DECODE`.

### Capture procedure

Manual GUI session, mirrors the A2 procedure:

1. Build instr branch in release mode:
   `cargo build -p eaglemode --release`.
2. Launch with structured log fd:
   `EM_INSTR_FD=9 cargo run -p eaglemode --release 9>/tmp/em_instr.blink-trace.log`.
3. Wait for window. Send SIGUSR1 marker for "open":
   `kill -USR1 $(pgrep -f eaglemode)`.
4. Click into a test-panel TextField. Wait ~30s with the cursor visible
   (focused TextField, no other interaction).
5. Send SIGUSR1 marker for "close".
6. Run analyzer:
   `python3 scripts/analyze_hang.py blink /tmp/em_instr.blink-trace.log`.

The capture launcher script `scripts/run_blink_capture.sh` from the A2
investigation (already present) can be reused with a different output path.

---

## Phase 0 → outcome dispatch

### Truth table

Read by the analyzer per focus-change event:

| `in_active_path` | `window_focused` | branch fired | outcome |
|---|---|---|---|
| f | * | f | O1 — `in_active_path` stale at notice time |
| t | f | f | O2 — `window_focused` stale at notice time |
| t | t | t | O3 — handler ran; bug elsewhere |
| t | t | f | (impossible by code; if observed, log 🚨 and treat as O3-ambiguous) |
| no NOTICE_FC_DECODE | — | — | O4 — notice not delivered |

### Aggregation rule

If multiple focus-change events occur in one capture:

- The "decisive" event is the last `NOTICE_FC_DECODE` for the clicked
  TextField in the post-click window.
- If multiple distinct outcomes occur across events (e.g., one O1 and one
  O3), the analyzer reports all outcomes and flags the capture as
  ambiguous; dispatch treats ambiguous as O3 (escalate to re-brainstorm).

### Dispatch rules

- **O1** → Proceed to Phase 1a. Cheap fix; do not re-brainstorm.
- **O2** → Proceed to Phase 1b. Cheap fix; do not re-brainstorm.
- **O3** → Stop. Open `docs/scratch/2026-05-03-blink-trace-results.md` with
  the capture findings. Re-invoke `superpowers:brainstorming` for B2.1
  with the data in hand.
- **O4** → Stop. Same as O3, different bug area (notice dispatch).
- **Ambiguous / impossible row** → Treat as O3.

### Stop condition

Phase 0 ends when one capture produces a clear verdict. If the first
capture is inconclusive (e.g., the user clicked but the analyzer found no
FOCUS_CHANGED notice for any TextField — capture procedure was wrong),
retry up to 2 more times, then escalate to user as a procedure problem.

---

## Phase 1a — Fix for outcome O1 (`in_active_path` stale)

**What this means.** At notice dispatch, `state.in_active_path == false`
for the textfield, even though some `set_active_panel` walk should have
set it true. Either (i) the FOCUS_CHANGED notice was queued by a code path
that ran before the path-update walk, or (ii) the path-update happened on
a different tree than the one whose notice fired.

### Diagnostic procedure

1. Find the wall-time of the offending `NOTICE_FC_DECODE` line.
2. Find the most recent `SET_ACTIVE_RESULT` and `SET_FOCUSED_RESULT` lines
   for the same panel/tree before that timestamp.
3. Determine which call queued the FOCUS_CHANGED that was delivered. Two
   suspects:
   - `set_active_panel` queueing FOCUS_CHANGED on a panel whose
     `in_active_path` was just updated. If this fired with
     `in_active_path=true`, then by notice time it should still be true.
     If it isn't, something cleared it between queue and flush.
   - `SetFocused` queueing FOCUS_CHANGED on panels whose `in_active_path`
     was already true at SetFocused time. If SetFocused ran while
     `in_active_path` was still false (e.g., before `set_active_panel`
     walked the path), the notice goes nowhere meaningful; an additional
     FOCUS_CHANGED may need to be queued by `set_active_panel`.
4. Read the offending site, **read the corresponding C++ code**
   (`emView::SetActivePanel`, `emView::SetFocused`, notice dispatch in
   `emPanel.cpp:HandleNotice`), identify the divergence.

### Expected fix shape

Reorder two operations at one call site, OR change a
`queue_notice(id, flags, None)` call to be placed at a different point in
the sequence. Likely candidates:

- `emView::set_active_panel` (lines 1758-1793): ensure path-update
  precedes any FOCUS_CHANGED queueing — already true by reading, so this
  is the unsurprising path.
- `emView::SetFocused` (lines 802-835): the FOCUS_CHANGED filter
  `if panel.in_active_path` reads the tree at SetFocused time, not at
  notice flush time. If SetFocused runs before path-update, the wrong
  set of panels gets FOCUS_CHANGED. Fix: ensure SetFocused either
  re-checks at flush time, or runs after the path-update.
- `SubViewPanel::Input` ordering (lines 296-298 SetFocused, 348
  set_active_panel): swap them, OR add a third call.

### Files touched

`crates/emcore/src/emView.rs` and/or `crates/emcore/src/emSubViewPanel.rs`.

### Budget

≤30 LOC, 1-2 commits. If diagnosis reveals the fix is larger
(e.g., requires changing how `PanelState` snapshots are built), escalate
to re-brainstorm B2.1.

---

## Phase 1b — Fix for outcome O2 (`window_focused` stale)

**What this means.** At notice dispatch, `state.window_focused == false`
for the textfield's view, even though SetFocused should have set it true.
Most likely: the SubView's `window_focused` was not propagated in time, OR
the outer view's input dispatch put the `SubViewPanel::Input` ordering
wrong.

### Diagnostic procedure

1. Find the offending `NOTICE_FC_DECODE` and confirm `window_focused=f`.
2. Look for `SET_FOCUSED_RESULT|view_kind=subview|focused=t|...`
   preceding it. If absent → SetFocused was never called on the sub-view
   with `focused=t`. If present → there's a SECOND SetFocused with
   `focused=f` between it and the NOTICE that we need to find.
3. Walk the input dispatch path:
   `emWindow::dispatch_input` → outer `set_active_panel` →
   `SubViewPanel::Input` (SetFocused, set_active_panel) → outer
   `handle_notice_one` for SVP (which ALSO calls
   `sub_view.SetFocused`) → sub-view `handle_notice_one` for textfield.
   Identify whether one of these SetFocused calls passed `focused=f`
   after a previous one passed `focused=t`.
4. **Read the corresponding C++ code** to ground the right ordering:
   `emSubViewPanel::Input` and `emView::SetFocused` in
   `~/Projects/eaglemode-0.96.4/src/emCore/`.

### Expected fix shape

One of:

- Remove a redundant SetFocused call (e.g., `SubViewPanel::Input`
  lines 296-299 SetFocused is redundant with the SVP::notice handler's
  SetFocused at lines 547-549, and one of them races the other).
- Reorder so the authoritative SetFocused happens last.
- Guard a SetFocused call to skip if state is already correct.

### Files touched

`crates/emcore/src/emSubViewPanel.rs` primarily; possibly
`crates/emcore/src/emView.rs::SetFocused` if a guard needs added.

### Budget

≤30 LOC, 1-2 commits. Same escalation rule as Phase 1a.

---

## Hard rule for both fix branches

The fix MUST be grounded in C++ behavior comparison. CLAUDE.md authority
order: read C++ to confirm the right ordering before changing Rust. The
diagnostic procedure includes "compare against C++" as an explicit step.
If C++ has the same potential ordering issue and gets away with it via
different timing, investigate why — do not just transplant Rust around it.

---

## Regression test design

One new integration test in `crates/emcore/tests/blink_focus_path.rs`
(new file). Targets the focus-gain → notice → engine-wake → Cycle chain
end-to-end without depending on the GUI or sub-view harness — minimum
surface to lock the bug class out.

### Test shape

```rust
#[test]
fn focused_panel_engine_wakes_and_cycles() {
    // 1. Build a minimal emView + PanelTree with a scheduler.
    let (mut tree, mut view, mut sched, root) = test_setup();

    // 2. Add a focusable child panel with an instrumented behavior that
    //    mimics the TextFieldPanel pattern: caches `is_focused` from
    //    NOTICE FOCUS_CHANGED, returns busy=is_focused from Cycle, and
    //    counts both invocations.
    let probe_id = tree.create_child_with(
        root, "probe", Box::new(BlinkProbe::default())
    );
    tree.SetFocusable(probe_id, true);

    // 3. Simulate window focus and active-panel change.
    view.SetFocused(&mut tree, true);
    {
        let mut sc = make_sched_ctx(&mut sched, ...);
        view.set_active_panel(&mut tree, probe_id, false, &mut sc);
    }

    // 4. Drive Update cycles until either the probe has cycled with
    //    is_focused=true, or we hit a tick budget.
    for _tick in 0..10 {
        let mut sc = make_sched_ctx(&mut sched, ...);
        view.Update(&mut tree, &mut sc);
        sched.do_time_slice();
        let probe_state = tree
            .with_behavior_as::<BlinkProbe, _>(probe_id, |b| b.snapshot());
        if probe_state.cycle_count_focused > 0 {
            return; // pass
        }
    }
    panic!(
        "probe never cycled with is_focused=true; \
         notice_fc_count={}, cycle_count_total={}, last_seen_focused={}",
        probe_state.notice_fc_count,
        probe_state.cycle_count_total,
        probe_state.last_seen_focused,
    );
}

#[derive(Default)]
struct BlinkProbe {
    is_focused: bool,
    notice_fc_count: u32,
    cycle_count_total: u32,
    cycle_count_focused: u32,
    last_seen_focused: bool,
}

impl PanelBehavior for BlinkProbe {
    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.notice_fc_count += 1;
            self.is_focused = state.in_focused_path();
            if self.is_focused {
                ctx.wake_up_panel(ctx.id);
            }
        }
    }
    fn Cycle(&mut self, _e: &mut EngineCtx, _p: &mut PanelCtx) -> bool {
        self.cycle_count_total += 1;
        if self.is_focused {
            self.cycle_count_focused += 1;
        }
        self.last_seen_focused = self.is_focused;
        self.is_focused  // busy iff focused, mirrors TextField
    }
    // Paint, Input, etc.: no-ops.
}
```

### Why this test would have caught the bug

Today: `state.in_focused_path()` returns false → `is_focused` stays false
→ `wake_up_panel` skipped → Cycle never runs while focused →
`cycle_count_focused == 0` → assertion fails.

Post-fix: `in_focused_path()` returns true → wake fires → Cycle runs →
`cycle_count_focused > 0` → assertion passes.

The probe behavior is identical in shape to the production
TextFieldPanel notice handler, so any structural ordering bug in the
focus-path system is caught here regardless of which widget surfaces it.

### Conditional second test

Phase 1b additionally adds:

```rust
#[test]
fn focused_panel_in_subview_engine_wakes_and_cycles() {
    // Same as above but the probe lives inside a SubViewPanel. Specifically
    // exercises the outer-set_active_panel → SVP::notice → sub_view.SetFocused
    // → sub_view.set_active_panel → sub_view notice flush chain.
}
```

Phase 1a does not add this — its bug doesn't require sub-view to repro.

### No timing dependence

The test loop has a fixed tick budget, no real-time waits. Deterministic.

### Test infrastructure

Reuses the existing `emcore` test patterns (e.g., the
`set_active_panel_transition_invalidates_highlight` test at emView.rs:7139
already does similar setup). The `BlinkProbe` is a ~50 LOC struct local to
the test file.

---

## Out of scope / deferred

### D1. Missing `WakeUpUpdateEngine` in `emView::set_active_panel`

**What.** C++ `emView::SetActivePanel` (emView.cpp:307) calls
`UpdateEngine->WakeUp()` after queueing notices. Rust
`view.set_active_panel` (emView.rs:1791-1793) calls
`tree.queue_notice(id, flags, None)` — `None` for sched, so the wake-up
branch in `add_to_notice_list` is dead. Notices end up relying on
incidental wakes from elsewhere.

**Why deferred.** Adding the wake-up call increases UpdateEngine wake
cadence, which interacts directly with B1 (the `has_awake==1` 66.7%
offender work). B1 should make the call about whether to add this wake
source.

**Follow-up.** Write a short note at
`docs/scratch/2026-05-03-set-active-panel-missing-wake.md` (~10-20 lines)
capturing the divergence with file:line citations, so B1's brainstorming
has it as documented input. No GitHub issue, no TODO comment in source —
the scratch note is the durable record.

### D2. Analyzer's `PanelCycleEngine` → panel mapping heuristic

**What.** The blink-trace analyzer's report includes "Engine REGISTER for
PanelCycleEngine" rows that fail to match the inner panel id when scope
is a SubViewPanel — `PanelCycleEngine.scope` records the outer
SubViewPanel id (e.g., `PanelId(2v1)`), not the inner panel's id.
Captured in the A2 findings doc as a known limitation.

**Why deferred.** Tooling improvement, not a behavior bug.

**Follow-up.** No action in B2. If a future investigation hits the same
heuristic limitation, address then.

### D3. Same-shape `notice` handlers in other widgets

**What.** Production `TextFieldPanel`
(`emColorFieldFieldPanel.rs:136-149`) and the test-panel copy
(`emTestPanel.rs:233-246`) both follow the same
`is_focused = state.in_focused_path(); if self.is_focused { wake_up_panel(...) }`
pattern. Other widgets (ListBox per `emColorFieldFieldPanel.rs:215-222`)
use a similar pattern but don't wake their engine on focus-gain — possibly
because they don't need to (no blink-like state machine).

**Why deferred.** Only the TextField has the blink behavior that makes
the bug user-visible. Other widgets may have the same latent issue but
the user can't observe it.

**Follow-up.** Phase 1's regression test (the `BlinkProbe` integration
test) catches the bug class generically. Out of B2's scope.

### D4. C++ ordering verification depth

**What.** The diagnostic procedures in Phase 1a / Phase 1b say "compare
against C++ behavior" but don't pre-commit to which exact C++ functions
get re-read. Phase 0's outcome will determine that.

**Why deferred.** Pre-listing C++ read targets for branches we may not
take is wasted effort. The diagnostic procedure names the search area;
the implementer reads the relevant C++ on demand.

**No follow-up needed.** Just calling out that "compare against C++" is
a real step the implementer must do, not a rubber-stamp.

---

## File structure

### Phase 0 (instrumentation, never merges to main)

- Modify: `crates/emcore/src/emView.rs` — add `NOTICE_FC_DECODE`,
  `SET_ACTIVE_RESULT`, `SET_FOCUSED_RESULT` emission sites.
- Modify: `scripts/analyze_hang.py` — extend `blink` command to parse the
  three new line types and emit Phase 0 verdict.
- Create: `docs/scratch/2026-05-03-blink-trace-results.md` — capture
  findings + Phase 0 verdict + prediction-vs-actual calibration.
- Create: `docs/scratch/2026-05-03-set-active-panel-missing-wake.md` —
  D1 deferred-divergence note.

### Phase 1 (fix, merges to main)

- Modify: 1-3 files in `crates/emcore/src/`, depending on Phase 0 outcome
  (`emView.rs`, `emSubViewPanel.rs`, possibly `emPanelTree.rs`).
  Budget ≤30 LOC.
- Create: `crates/emcore/tests/blink_focus_path.rs` — `BlinkProbe`
  regression test (and conditionally the sub-view variant for Phase 1b).

---

## Branch lifecycle

```
main (637d8bf1)
 │
 ├─[cut]─→ instr/blink-trace-2026-05-03   (Phase 0; from tag instr-7-loop-chain)
 │           │
 │           ├─ commit: instrumentation lines
 │           ├─ commit: analyzer extension
 │           │
 │           └─ <capture happens here, runs analyzer>
 │           └─ <findings doc + scratch note land directly on main, NOT on this branch>
 │
 └─[cut]─→ fix/blink-focus-path-2026-05-03   (Phase 1; from main)
             │
             ├─ commit: focus-path ordering fix (1-2 commits)
             ├─ commit: regression test
             ├─ commit: scratch findings + deferral notes (if not already on main)
             │
             └─[merge]─→ main
```

The Phase 0 instrumentation branch is **never merged**. After Phase 0
lands a verdict, tag the branch (e.g., `instr-blink-2026-05-03`) for
archival reference and discard the working copy.

The findings doc (`blink-trace-results.md`) and the deferral note
(`set-active-panel-missing-wake.md`) commit directly to main, separately
from any branch — they're documentation, not instrumentation. Same
pattern as A1/A2 findings docs.

---

## Exit conditions

B2 is "done" when ALL of these hold:

1. Phase 0 capture produced a verdict (O1 / O2 / O3 / O4) recorded in
   the findings doc.
2. If O1 or O2: fix landed on main; regression test passes; manual
   recapture confirms `BLINK_CYCLE` entries appear post-fix.
3. If O3 or O4: B2 closes with no fix landed; B2.1 brainstorm initiated.
4. Both scratch documents (findings, deferral note) committed to main.
5. Phase 0 Results appendix below filled in with actual outcome +
   retrospective prediction calibration.

---

## Phase 0 results appendix (filled in during execution)

> Template; fill in after Phase 0 capture.

**Capture log:** `<path>`
**Capture wall-time window:** `<start>` to `<end>` (`<duration>` s)
**Decisive NOTICE_FC_DECODE event:**
- `wall_us`: `<...>`
- `panel_id`: `<...>`
- `behavior_type`: `<...>`
- `in_active_path`: `<t|f>`
- `window_focused`: `<t|f>`
- `flags`: `<0x...>`

**Branch fired (WAKE entry within window?):** `<t|f>`

**Verdict:** `<O1|O2|O3|O4>`

**Prediction calibration:**
- Pre-measurement priors: 60% O1, 25% O2, 10% O3, 5% O4
- Actual: `<O?>`
- Notes: `<one-line retrospective on the priors vs reality>`

**Next phase dispatched:** `<Phase 1a|Phase 1b|B2.1 re-brainstorm>`
