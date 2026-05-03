# B2.1 — Blink-cycle chain investigation

## Status

Spec for the follow-up to B2 (`docs/superpowers/specs/2026-05-03-blink-focus-path-design.md`),
which closed without a fix when its Phase 0 capture showed the focus-path
bookkeeping is correct (`in_active_path=t, window_focused=t` at notice
dispatch on the actual click target) yet zero `WAKE` entries fire from
the focused-branch `wake_up_panel` call site, and zero `BLINK_CYCLE`
entries appear post-click. Findings are at
`docs/scratch/2026-05-03-blink-trace-results.md`. The instr-branch tag
`instr-blink-2026-05-03` archives B2's instrumentation work.

## Problem

When the user clicks into a TextFieldPanel, the chain from "FOCUS_CHANGED
notice arrives at the panel handler" to "cursor blink visible" breaks
*somewhere between the handler entry and the visible pixel*. B2's Phase
0 ruled out the *focus-path bookkeeping* layer (the upstream
`set_active_panel` / `SetFocused` ordering is correct; both flags are
true at notice dispatch). The break must be in one of the layers below:

- **Layer A** — notice handler body (does `behavior.notice` actually
  run, does the FOCUS_CHANGED branch enter, does `wake_up_panel` get
  invoked)?
- **Layer B** — `EngineCtx::wake_up_panel` itself, which has two
  silent-no-op early-return guards (`panel.engine_id == None`,
  `self.scheduler == None`) that bypass `sched.wake_up()` without
  emitting any log entry.
- **Layer C** — engine wake → `Cycle` dispatch (does `DoTimeSlice` pick
  up the woken engine; does `PanelCycleEngine::Cycle` route to the right
  panel behavior)?
- **Layer D** — `Cycle` internals (does the blink-state machine
  advance; does `request_invalidate_self` fire; does the paint pipeline
  drain the dirty bit)?

The existing instrumentation (`WAKE`, `BLINK_CYCLE`, `INVAL_DRAIN`,
etc.) covers Layers C/D unconditionally — so the chain's silence in C/D
is *consequential*, not independent. The break is in A or B. None of
the 10 distinguishable failure modes inside A and B are currently
distinguishable in the log; the break could be at any of them.

## Goal

Identify *which specific link* of layers A–D fails. Land a small,
scoped fix (≤30 LOC across at most 2 files) and a regression test that
asserts the bug class cannot silently recur. If the fix budget is
exceeded or the verdict points outside the candidate space, escalate to
B2.2.

## Non-goals

- D1 (`set_active_panel` missing `WakeUpUpdateEngine`) — already
  deferred to B1; note at
  `docs/scratch/2026-05-03-set-active-panel-missing-wake.md`.
- OD3-paint regression test (deeper paint-pipeline assertion). Deferred
  to a follow-up; the BLINK_CYCLE-presence test already catches the
  dominant bug classes.
- Other widgets with the same focus → wake pattern (B2's D3); covered
  structurally by the regression test.
- Re-architecting the analyzer's path-trace heuristic beyond the
  click-target picking fix (B2's D2, deeper part).
- Architectural alternatives for engine binding outside the ≤30 LOC
  budget (the C5–C7 alternatives sketched during brainstorming);
  trigger re-brainstorm B2.2 if the verdict requires them.

---

## Architecture overview

Mirrors B2's measure-then-fix structure:

```
Phase 0: Instrument + Capture + Analyze
  └─ outcome ∈ 9 bins read off a layer-by-layer truth table

Dispatch (verdict-keyed):
  OB2 / OB3 / OC-DISPATCH / OD2 → Phase 1 with concrete fix sketch
  OC-NOPICKUP, OD3              → Phase 1 if WAKE.engine_type / tile
                                   evidence is clear; otherwise B2.2
  OA1 / OB1 / OD-OK             → Re-brainstorm B2.2

Phase 1 (verdict-dispatched, only for fix-eligible bins):
  - Apply ≤30 LOC fix per the bin's diagnostic procedure
  - Add regression test in crates/emcore/tests/blink_focus_path.rs
  - Verify with manual recapture (BLINK_CYCLE entries appear with
    flipped=t toggling)
  - Land on main as a normal merge

Out of scope (deferred): see Non-goals.
```

**Branches.**

- Phase 0 instrumentation: `instr/blink-cycle-2026-05-03`, cut from
  existing tag `instr-blink-2026-05-03` (B2's archived instr branch).
  Never merges to main.
- Phase 1 fix: `fix/blink-cycle-chain-2026-05-03`, cut from main.
  Standard merge.
- Findings doc commits directly to main as documentation.

**Prediction commitment** (advisor's calibration check). Before
measurement: **50% OB2, 25% OB3, 10% OC-NOPICKUP, 5% OC-DISPATCH, 5%
OD2, 3% OD3, 2% other (OA1/OB1/OD-OK)**. After Phase 0 lands the actual
outcome, record the actual verdict in the Phase 0 Results appendix
below for retrospective calibration.

Reasoning behind these priors: the existing capture's chain dies before
WAKE, which constrains the candidate space to layers A/B. Inside that
space, OB2 dominates because the deferred-binding contract in
`init_panel_view` (gates engine binding on `sched=Some`, with comment
"will register once one is available") has a known fragility window —
if the deferred re-call never fires, `panel.engine_id` stays None at
notice time. OB3 is plausible at lower probability because the notice
dispatch's PanelCtx uses `with_sched_reach_optional_roots` (the
"_optional" suggests a code path with `scheduler=None`). OB2 and OB3 may
be the same root cause (scheduler-context propagation) manifesting
differently — see Phase 1 dispatch.

---

## Phase 0 — Instrumentation design

### Three new structured log lines

Same `crates/emcore/src/emInstr.rs` `write_line()` infra as the existing
A1/A2/B2 instrumentation.

**Line 1: `HANDLER_ENTRY`**

Emitted once per `TextFieldPanel::notice` invocation when
`flags.intersects(NoticeFlags::FOCUS_CHANGED)`, at the *end* of the
FOCUS_CHANGED block (after `self.is_focused` assignment, regardless of
whether the `if self.is_focused` branch executed).

```
HANDLER_ENTRY|wall_us=...|panel_id=...|impl=<test|prod>|flags=0xf0|is_focused_path=t|branch_taken=t
```

Sites:

- `crates/emtest/src/emTestPanel.rs::TextFieldPanel::notice` (line ~246
  on instr-blink-2026-05-03; field name `self.widget`).
- `crates/emcore/src/emColorFieldFieldPanel.rs::TextFieldPanel::notice`
  (line ~148; field name `self.text_field`).

The `impl` field is hardcoded as a string literal at each site
(`"emTestPanel::TextFieldPanel"` vs
`"emColorFieldFieldPanel::TextFieldPanel"`) so the analyzer can
distinguish which impl ran without depending on `type_name_of_val`.

`is_focused_path` is the result of `state.in_focused_path()` at the
handler call. `branch_taken` is the value of `self.is_focused` after
assignment (which equals `is_focused_path`). They're redundant by code,
but together they sanity-check that the formula evaluated as expected
and that no impl-specific override was applied.

**Line 2: `WUP_RESULT`**

Single emit at the end of `EngineCtx::wake_up_panel`
(`crates/emcore/src/emEngineCtx.rs:847`). Requires restructuring the
function from chained early-returns into a single-exit form so all
guard outcomes hit the same emit point. Adds `#[track_caller]` so the
emit can include the call site (mirrors `wake_up_engine`'s existing
caller-line capture).

```
WUP_RESULT|wall_us=...|panel_id=...|caller=<file:line>|panel_found=t|engine_id=Some(EngineId(...))|scheduler_some=t|wake_dispatched=t
```

`engine_id` is `{:?}` formatted (renders as `Some(EngineId(...))` or
`None`). `panel_found`, `scheduler_some`, `wake_dispatched` are derived
booleans. `wake_dispatched=t` iff `sched.wake_up(eid)` was actually
invoked — its complement of t/f for the other booleans names the
specific guard that fired.

**Line 3: `CYCLE_ENTRY`**

Two insertion points in `crates/emcore/src/emPanelCycleEngine.rs`,
immediately before each `behavior.Cycle(&mut ectx, &mut pctx)` call:
the Toplevel path (line ~122) and the SubView path (line ~237).

```
CYCLE_ENTRY|wall_us=...|engine_id=...|panel_id=...|behavior_type=<typename>
```

`behavior_type` via `std::any::type_name_of_val(&*behavior)`, falling
back to a string literal if the toolchain rejects the call (`recipient_type_owned`-style
prelude; mirrors B2's NOTICE_FC_DECODE pattern).

### Analyzer changes

Extend `scripts/analyze_hang.py`:

1. **Parse the three new line types** (parallel to B2's `parse_notice_fc_decode` / `parse_set_active_result` / `parse_set_focused_result`).
2. **Revise click-target heuristic.** The current path-trace heuristic
   picks the focus target from the first ACTIVATE / focus-transition
   event after the open marker, which mis-targets layout-driven blanket
   flushes. Replace with: `target = target_panel_id of the latest
   SET_ACTIVE_RESULT|window_focused=t between MARKER pair`. One-line
   fix.
3. **Add B2.1 verdict emission.** For the click target, find the *first*
   `NOTICE_FC_DECODE` with `iap=t && wf=t` after the matching
   `SET_ACTIVE_RESULT`. From that decisive event, scan forward in time
   through `HANDLER_ENTRY → WUP_RESULT → WAKE → CYCLE_ENTRY →
   BLINK_CYCLE → INVAL_DRAIN(drained=t)`. The first absent layer (or the
   first present layer with a failing field) names the verdict bin per
   the truth table below.

### Capture procedure

Same as B2: manual GUI session, SIGUSR1 markers around a 30-second
focused TextField hold, on the `instr/blink-cycle-2026-05-03` branch
release build. Reuse `scripts/run_blink_capture.sh` with output path
`/tmp/em_instr.blink-cycle.log`.

---

## Phase 0 → outcome dispatch

### Truth table (read by analyzer)

After locating the decisive `NOTICE_FC_DECODE`, the analyzer reads the
chain forward in time. The first absent layer (or the first present
layer with a failing field value) names the bin:

| Bin | Detection criteria | Phase 1 path |
|---|---|---|
| **OA1** | NOTICE_FC_DECODE present, HANDLER_ENTRY absent | Re-brainstorm B2.2 (panic / vtable) |
| **OB1** | HANDLER_ENTRY (`branch_taken=t`), WUP_RESULT `panel_found=f` | Re-brainstorm B2.2 (impossible by code; 🚨) |
| **OB2** | WUP_RESULT `engine_id_some=f` | Phase 1 fix (engine binding lifecycle) |
| **OB3** | WUP_RESULT `engine_id_some=t`, `scheduler_some=f` | Phase 1 fix (PanelCtx scheduler propagation) |
| **OC-NOPICKUP** | WUP_RESULT `wake_dispatched=t`, WAKE present, CYCLE_ENTRY absent | Phase 1 fix *if* WAKE.engine_type evidence is clear (`<unregistered>` or wrong-type); re-brainstorm B2.2 *if* engine_type=PanelCycleEngine (DoTimeSlice scheduler internals; out of budget) |
| **OC-DISPATCH** | CYCLE_ENTRY present, BLINK_CYCLE absent | Phase 1 fix (PanelCycleEngine routing) |
| **OD2** | BLINK_CYCLE present, `flipped=f` always | Phase 1 fix (cycle_blink timer logic) |
| **OD3** | INVAL_DRAIN with `drained=f` for the panel after a `flipped=t` event | Phase 1 fix *if* dirty-tile evidence is clear; re-brainstorm B2.2 *if* paint pipeline involvement is unclear |
| **OD-OK** | Full chain present in logs (BLINK_CYCLE flipped=t, INVAL_DRAIN drained=t, RENDER follows), but no visible blink | Re-brainstorm B2.2 (paint content / capture procedure / perception; non-structural) |

### Aggregation rule

If multiple focus-change events occur in one capture, the *decisive*
event is the **first** `NOTICE_FC_DECODE` for the click target after
the matching `SET_ACTIVE_RESULT|window_focused=t` with `iap=t && wf=t`.
"First" rather than "last" because we measure the
click → establishment chain, not the steady-state behavior under
subsequent blur/refocus.

If multiple distinct verdicts occur across events (e.g., one OB2 and
one OD2), the analyzer reports all and flags as ambiguous; dispatch
treats ambiguous as re-brainstorm B2.2.

### Stop condition

Phase 0 ends when one capture produces a clear verdict. If the first
capture is inconclusive (e.g., the analyzer cannot identify a click
target — capture procedure was wrong), retry up to 2 more times, then
escalate to user as a procedure problem.

---

## Phase 1 — Verdict-dispatched fix shapes

### OB2 — `panel.engine_id` is None at notice dispatch

**What this means.** When `wake_up_panel` is called from the focused
branch of the notice handler, the panel has no engine bound. The
`init_panel_view` registration site
(`crates/emcore/src/emPanelTree.rs:685-702`) gates engine creation on
`sched=Some` and `has_view=true`, deferring registration with the
comment "will register once one is available". For PanelId(125v1) at
notice time, the deferred re-registration never fired.

**Diagnostic procedure.**

1. From the capture, identify the `init_panel_view` call(s) for the
   click target panel id. (Currently no instrumentation; if needed,
   add a short-lived `INIT_PANEL_VIEW` log line emitting `panel_id`,
   `has_view`, `sched_some`, `registered=t/f` as part of the diagnostic
   pass on the instr branch.)
2. Determine which guard(s) triggered the deferral (no view? no
   scheduler?).
3. Trace upstream to find the call path that *should* re-call
   `init_panel_view` once the gating condition resolves but doesn't.
4. **Read the C++ ground truth**: `emPanel::PrivLayout` and
   `emPanel::Notice` in `~/Projects/eaglemode-0.96.4/src/emCore/emPanel.cpp`
   for engine-lifecycle semantics. C++'s "panel IS engine" pattern
   means binding is implicit at construction; the Rust port's deferred
   binding is a forced-divergence (separate engine object) that needs
   an explicit re-trigger somewhere.

**Candidate fix sites.**

| Site | Mechanism | Pro | Con | Likely budget |
|---|---|---|---|---|
| **C1** | Find the missing `init_panel_view(_, Some(sched))` re-call — trace which path created the panel and why deferred re-registration never fired; add the missing call. | Surgical; preserves existing deferred-binding architecture; mirrors design intent. | Requires capture-grounded diagnosis (which path was taken). | ≤15 LOC at one site |
| **C2** | Eager re-call from `emView::set_active_panel` — ensure `init_panel_view(panel_id, Some(sched))` has run when a panel becomes active. | Single, well-defined trigger; covers all panel types. | Adds binding work to every activation (idempotency check needed); set_active_panel shouldn't manage engine lifecycle (architectural smell). | ≤10 LOC |
| **C3** | Lazy re-call inside `wake_up_panel` — when `panel.engine_id == None`, attempt registration via reachable scheduler, then retry. | Self-healing; localized. | Overloads `wake_up_panel`'s contract ("wake or no-op" → "wake or fix-and-wake"); engine-type-specific knowledge leaks into a generic helper. | ≤15 LOC |
| **C4** | Re-call from `TextFieldPanel::notice` on FOCUS_CHANGED — ensure binding before `wake_up_panel`. | Localized to affected widget. | Pushes lifecycle concerns into widget code; doesn't fix root cause for other latent-affected panels. | ≤10 LOC, but per-widget |

**Ruled out by structure:** binding at panel construction (scheduler
not reliably available at `create_child` time — that's why the deferred
contract exists); binding inside widget code (`emTextFieldWidget`
doesn't reach `panel.engine_id`).

**Recommended procedure.** Try C1 first — capture-driven diagnosis
should pin the missing re-call site if one exists. If diagnosis is
inconclusive (no specific missing-call site identifiable), fall back to
C2 as a general safety net. C3/C4 are last resorts (architectural smell
or per-widget patch).

**Concomitant check (OB2 ↔ OB3 linkage).** Both bins are
scheduler-context propagation problems. If the verdict is OB2, the
implementer should also collect WUP_RESULT.scheduler_some evidence
across the capture; if OB3-style "scheduler=None at notice" also
appears, the fix may unify into a single scheduler-propagation cleanup
upstream. Keep the budget envelope unchanged either way.

**Files touched.** `crates/emcore/src/emPanelTree.rs` (init_panel_view
caller patch), and possibly `crates/emcore/src/emView.rs` or
`crates/emcore/src/emEngineCtx.rs` depending on the chosen site.

**Budget.** ≤30 LOC, 1-2 commits. Escalate to re-brainstorm B2.2 if
diagnosis reveals a larger fix is needed.

### OB3 — `scheduler` is None at notice dispatch

**What this means.** The PanelCtx built at notice dispatch
(`emView::handle_notice_one`, the
`PanelCtx::with_sched_reach_optional_roots(...)` call) was constructed
without a scheduler. The `wake_up_panel` call's third guard fires.

**Diagnostic procedure.**

1. From `WUP_RESULT.caller`, confirm the call originates from
   `TextFieldPanel::notice` (line 255 in test, line ~158 in production).
2. Inspect `handle_notice_one` to determine why
   `with_sched_reach_optional_roots` was passed `None` for sched. Likely
   causes: caller chain has no scheduler in scope; an older caller did
   not propagate it; the `_optional` API was chosen for a different
   reason.
3. **Read C++ ground truth**: `emPanel::HandleNotice` and notice
   dispatch in `~/Projects/eaglemode-0.96.4/src/emCore/emPanel.cpp`.
   C++ has implicit access via the panel-engine inheritance; Rust must
   thread it explicitly.

**Fix shape.** Replace `_optional_roots` with `_roots` (mandatory
scheduler) at the notice-dispatch call site, threading the scheduler
through from the caller. If the caller chain doesn't have a scheduler
in scope, trace upstream to find where it was dropped.

**Files touched.** `crates/emcore/src/emView.rs` (handle_notice_one and
its callers if needed). Possibly `crates/emcore/src/emEngineCtx.rs`
(PanelCtx constructors).

**Budget.** ≤15 LOC.

### OC-NOPICKUP — `wake_up_panel` succeeded but no Cycle dispatch

**What this means.** WUP_RESULT shows wake_dispatched=t. WAKE log
appears. CYCLE_ENTRY does not. The `engine_type` in the WAKE entry
disambiguates the sub-cause.

**Diagnostic procedure.**

1. Read `WAKE.engine_type` for the cursor-blink wake.
   - `<unregistered>` → engine was deregistered while panel.engine_id
     stayed Some (stale binding).
   - Type ≠ `PanelCycleEngine` → wrong engine type bound to panel.
   - Type = `PanelCycleEngine` → DoTimeSlice didn't pick up the woken
     engine (scheduler queue management bug).
2. Cross-reference with `REGISTER` lines for the engine id to
   confirm the binding state.

**Fix shape (per sub-case).**

- **Stale binding**: fix the deregistration path to clear
  `panel.engine_id` when the engine is deregistered.
- **Wrong-type binding**: trace where the panel's engine_id was set to
  a non-PCE engine; fix the registration site.
- **DoTimeSlice case**: **likely re-brainstorm B2.2** — DoTimeSlice
  internals are deep scheduler code; ≤30 LOC fix unrealistic without
  a focused investigation.

**Files touched.** `crates/emcore/src/emPanelTree.rs` (deregistration
patch) or `crates/emcore/src/emPanelCycleEngine.rs` (registration site)
or escalation.

**Budget.** ≤30 LOC for the first two; escalate the third.

### OC-DISPATCH — Cycle ran but on the wrong behavior

**What this means.** CYCLE_ENTRY is present but BLINK_CYCLE is not.
PanelCycleEngine dispatched to `behavior.Cycle` but the behavior wasn't
the TextFieldPanel for the click target.

**Diagnostic procedure.**

1. Read `CYCLE_ENTRY.behavior_type`. If it doesn't match the panel id
   you expected, the routing inside PanelCycleEngine resolved the
   wrong panel/behavior.
2. Inspect the panel-tree lookup logic in `PanelCycleEngine::Cycle` for
   the matching path (Toplevel or SubView).

**Fix shape.** Fix the panel/behavior lookup so it resolves to the
right target. Likely a scope-mismatch or panel-id-vs-engine-id
confusion.

**Files touched.** `crates/emcore/src/emPanelCycleEngine.rs`.

**Budget.** ≤15 LOC.

### OD2 — Blink-state machine doesn't tick

**What this means.** BLINK_CYCLE entries appear but `flipped=f` always.
The `cycle_blink` state machine on `emTextFieldWidget` runs but the
clock comparison or threshold never triggers a flip.

**Diagnostic procedure.**

1. Inspect `cycle_blink` (`crates/emcore/src/emTextFieldWidget.rs:2360`+,
   per A2 mapping). Compare clock-source semantics against C++
   `emTextField::Cycle` (`emTextField.cpp:306-340`).
2. Likely culprits: the `Instant::now()` vs `emGetClockMS` choice may
   produce a different clock; the threshold value may be off-by-one;
   the conditional structure may differ.

**Fix shape.** Align the timer logic with C++. Specific: C++ uses
`emUInt64 clk = emGetClockMS()` and compares `clk >= CursorBlinkTime +
1000` and `clk >= CursorBlinkTime + 500`. The Rust port likely uses
`Instant::now().duration_since(...)`; align the units and thresholds
exactly.

**Files touched.** `crates/emcore/src/emTextFieldWidget.rs`.

**Budget.** ≤15 LOC.

### OD3 — Paint pipeline drops the invalidation

**What this means.** BLINK_CYCLE fires with `flipped=t`,
`request_invalidate_self` is invoked, but the corresponding
INVAL_DRAIN shows `drained=f` (or no INVAL_DRAIN appears for the panel
at all).

**Diagnostic procedure.**

1. Confirm `take_invalidate_self_request` is being called and its
   return value is `t` for the panel.
2. Trace from PanelCycleEngine's drain path through the window's redraw
   cycle. Possible drops: dirty-tile tracking misses the panel's rect,
   or the redraw cycle skips a frame, or the paint cache occludes.

**Fix shape.** Depends on where the trace lands. Likely in
`emPanelCycleEngine.rs` (drain logic) or `emWindow.rs` (paint pipeline
dirty-tile tracking).

**Files touched.** `crates/emcore/src/emPanelCycleEngine.rs` and/or
`crates/emcore/src/emWindow.rs`.

**Budget.** ≤30 LOC; if the trace points into deep windowing internals
(wgpu / winit interaction), escalate to re-brainstorm B2.2.

### Escalation paths (no Phase 1 fix in B2.1)

- **OA1** (handler not invoked despite NOTICE_FC_DECODE): possible
  causes are panic, take_behavior race, or vtable mismatch (cdylib
  trap). Read the HANDLER_ENTRY log absence pattern + cross-ref with
  process state. Modality: deeper Rust-debug investigation, possibly
  with valgrind / sanitizers. Out of B2.1 scope.
- **OB1** (panel disappeared mid-handler): impossible by code in the
  current call chain (no yield point between handler entry and
  wake_up_panel). If observed, indicates panel-tree corruption; modality
  is deep memory/threading investigation. Out of B2.1 scope.
- **OD-OK** (chain logs as healthy, no visible blink): three
  sub-causes — paint content, focus mis-targeting in the capture, or
  user perception. Modality: visual debugging (DUMP_GOLDEN-style frame
  capture), capture-procedure verification, or independent confirmation
  with another observer. Different toolset entirely; out of B2.1 scope.

---

## Regression test design

### Test name and shape

`focused_text_field_engine_wakes_and_cycles` in
`crates/emcore/tests/blink_focus_path.rs`. Mirrors B2's Task 14
structure but asserts a different invariant.

The test exercises the *real* `TextFieldPanel` (not a probe behavior),
because the bug class B2.1 targets — engine binding lifecycle, cycle
dispatch, paint invalidation — lives below the behavior layer. A probe
behavior would not exercise the same wake/cycle infrastructure paths.

```rust
#[test]
fn focused_text_field_engine_wakes_and_cycles() {
    use emcore::emView::emView;

    let mut h = TestViewHarness::new();

    // Build tree: root → real TextFieldPanel.
    let root = h.tree.create_root_deferred_view("root");
    h.tree.get_mut(root).unwrap().focusable = true;
    h.tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    let tf_id = h.tree.create_child(root, "tf", None);
    h.tree.get_mut(tf_id).unwrap().focusable = true;
    h.tree.Layout(tf_id, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    h.tree.set_behavior(tf_id, /* real TextFieldPanel ctor */);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 640.0, 480.0);
    { let mut sc = h.sched_ctx(); view.Update(&mut h.tree, &mut sc); }

    // Window focus + active-panel transition.
    view.SetFocused(&mut h.tree, true);
    { let mut sc = h.sched_ctx(); view.set_active_panel(&mut h.tree, tf_id, false, &mut sc); }
    { let mut sc = h.sched_ctx(); view.Update(&mut h.tree, &mut sc); }

    // Pump scheduler for ~2 seconds of simulated time.
    let cycle_log = capture_cycle_log(|| h.run_time_slices(/* until at least one BLINK_CYCLE.flipped=t */));

    // Assertions.
    assert!(cycle_log.iter().any(|e| e.kind == "BLINK_CYCLE" && e.panel_id == tf_id),
        "no BLINK_CYCLE entry for the focused TextField — engine never cycled");
    assert!(cycle_log.iter().any(|e| e.kind == "BLINK_CYCLE" && e.flipped),
        "BLINK_CYCLE entries appeared but flipped=t never observed — cycle_blink timer logic broken");
}
```

### Why this assertion shape

The "BLINK_CYCLE present with at least one `flipped=t`" assertion
catches all of OB2, OB3, OC-NOPICKUP, OC-DISPATCH, OD2 as regressions
— they all break the property "the focused TextField produces blink
cycle log entries with state transitions".

OD3 (paint pipeline drops the invalidation) is *not* caught by this
test; the chain reaches BLINK_CYCLE but the dirty bit is dropped
downstream. A pixel-chain assertion (asserting a `Painter` mock saw a
cursor on/off pair within a window) would catch OD3 — deferred to a
follow-up.

### Sanity check requirement (mirrors B2 Task 16 Step 5)

The test must FAIL when the Phase 1 fix is reverted. If it passes
without the fix, the test isn't exercising the bug class and is
worthless as a regression guard — investigate before proceeding.

### Implementer caveats

- TestViewHarness already exists (see `crates/emcore/src/test_view_harness.rs`).
  If `cargo test --test blink_focus_path` fails to compile due to
  missing `test-support` feature, add `emcore = { path = ".", features
  = ["test-support"] }` to dev-dependencies (mirrors B2 Task 14 Step 2
  guidance).
- "Capture cycle log" needs a small test-only hook on `emInstr` to
  buffer log lines into a per-test vector instead of (or in addition
  to) writing them to fd 9. If absent, add a 5-LOC capture flag to
  `emInstr.rs` gated on `#[cfg(test)]`. Out-of-budget surface is
  contained.
- The test requires `run_time_slices` or equivalent on TestViewHarness
  to pump multiple time slices. If TestViewHarness lacks this, add it
  as part of the test-support API; should be ≤15 LOC.

---

## Out of scope / deferred

(Already enumerated under Non-goals; this section reaffirms.)

- **D1** (`set_active_panel` missing `WakeUpUpdateEngine`) — B1's
  territory. Note: `docs/scratch/2026-05-03-set-active-panel-missing-wake.md`.
- **D3** (other widgets with same pattern) — covered structurally by
  the regression test.
- **OD3-paint regression test** (deeper paint-pipeline assertion) —
  follow-up.
- **OB2-architectural** (C5–C7 alternatives outside ≤30 LOC budget) —
  re-brainstorm B2.2.
- **Analyzer's deeper path-trace heuristic** (beyond click-target
  picking) — tooling task.
- **OA1 / OB1 / OD-OK escalation modalities** — re-brainstorm B2.2 if
  observed.

---

## File structure

### Phase 0 (lives on `instr/blink-cycle-2026-05-03`, never merges to main)

| Path | Action |
|---|---|
| `crates/emtest/src/emTestPanel.rs` | Modify (HANDLER_ENTRY in test TextFieldPanel::notice) |
| `crates/emcore/src/emColorFieldFieldPanel.rs` | Modify (HANDLER_ENTRY in production TextFieldPanel::notice) |
| `crates/emcore/src/emEngineCtx.rs` | Modify (WUP_RESULT in wake_up_panel; restructure to single-exit; add #[track_caller]) |
| `crates/emcore/src/emPanelCycleEngine.rs` | Modify (CYCLE_ENTRY at both behavior.Cycle sites) |
| `scripts/analyze_hang.py` | Modify (parse 3 new lines; revise click-target heuristic; emit B2.1 verdict per 9-bin truth table) |
| `scripts/test_analyze_hang.py` | Modify (truth-table coverage tests for all 9 bins) |

### Phase 0 documentation (lives on main)

| Path | Action |
|---|---|
| `docs/scratch/2026-05-03-blink-cycle-results.md` | Create (B2.1 verdict + recapture findings + prediction calibration) |

### Phase 1 (lives on `fix/blink-cycle-chain-2026-05-03`, merges to main)

| Path | Action |
|---|---|
| 1-3 files in `crates/emcore/src/` (verdict-dispatched) | Modify (OB2: emPanelTree.rs/emEngineCtx.rs; OB3: emView.rs; OC-*: emPanelCycleEngine.rs/emScheduler.rs; OD2: emTextFieldWidget.rs) |
| `crates/emcore/tests/blink_focus_path.rs` | Create (regression test) |

---

## Branch lifecycle

```
main (a9bcd1d2)
 │
 ├─[cut]─→ instr/blink-cycle-2026-05-03   (Phase 0; from tag instr-blink-2026-05-03)
 │           │
 │           ├─ commit: HANDLER_ENTRY, WUP_RESULT, CYCLE_ENTRY (3 commits)
 │           ├─ commit: analyzer extension (parsers, heuristic fix, verdict)
 │           ├─ commit: analyzer unit tests
 │           │
 │           └─ <capture happens here, runs analyzer>
 │           └─ <findings doc lands directly on main, NOT on this branch>
 │
 ├─ direct commits on main: B2.1 findings doc + dispatch decision
 │
 └─[cut]─→ fix/blink-cycle-chain-2026-05-03   (Phase 1; from main)
             │
             ├─ commit: verdict-dispatched fix (1-2 commits)
             ├─ commit: regression test
             │
             └─[merge]─→ main
```

The Phase 0 instrumentation branch is **never merged**. After Phase 0
lands a verdict, tag the branch (e.g., `instr-blink-cycle-2026-05-03`)
for archival.

The findings doc commits directly to main, separately from any branch
— same pattern as A1/A2/B2.

---

## Exit conditions

B2.1 is "done" when ALL of these hold:

1. Phase 0 capture produced a verdict from the 9-bin truth table.
2. If the verdict is in {OB2, OB3, OC-DISPATCH, OD2,
   OC-NOPICKUP-with-clear-engine_type, OD3-with-clear-tile-evidence}:
   fix landed on main; regression test passes; manual recapture
   confirms BLINK_CYCLE entries appear post-fix with `flipped=t`
   toggling.
3. If the verdict is in {OA1, OB1, OD-OK,
   OC-NOPICKUP-DoTimeSlice-case, OD3-deep-paint-internals}: B2.1
   closes with no fix landed; B2.2 brainstorm initiated.
4. Findings doc committed to main.
5. Phase 0 Results appendix below filled in with actual outcome +
   prediction calibration.

---

## Phase 0 results appendix (filled in during execution)

> Template; fill in after Phase 0 capture.

**Capture log:** `<path>`
**Capture wall-time window:** `<start>` to `<end>` (`<duration>` s)

**Click target (latest SET_ACTIVE_RESULT|window_focused=t):**
- `target_panel_id`: `<...>`
- `wall_us`: `<...>`

**Decisive NOTICE_FC_DECODE event:**
- `wall_us`: `<...>`
- `panel_id`: `<...>`
- `behavior_type`: `<...>`
- `in_active_path`: `<t|f>`
- `window_focused`: `<t|f>`

**Chain trace post-decisive:**
- `HANDLER_ENTRY`: `<present|absent>` (if present: `branch_taken=<t|f>`)
- `WUP_RESULT`: `<present|absent>` (if present: `panel_found=<t|f>`,
  `engine_id=<Some|None>`, `scheduler_some=<t|f>`,
  `wake_dispatched=<t|f>`)
- `WAKE` (cursor-blink): `<present|absent>` (if present:
  `engine_type=<...>`)
- `CYCLE_ENTRY`: `<present|absent>` (if present:
  `behavior_type=<...>`)
- `BLINK_CYCLE`: `<present|absent>` (if present: `flipped` ever `t`?
  `<yes|no>`)
- `INVAL_DRAIN`: `<present|absent>` (if present: `drained=<t|f>`)

**Verdict bin:** `<OA1|OB1|OB2|OB3|OC-NOPICKUP|OC-DISPATCH|OD2|OD3|OD-OK>`

**Prediction calibration:**
- Pre-measurement priors: 50% OB2, 25% OB3, 10% OC-NOPICKUP, 5%
  OC-DISPATCH, 5% OD2, 3% OD3, 2% other.
- Actual: `<...>`
- Retrospective: `<one-line note>`

**Next phase dispatched:** `<Phase 1 (fix shape: ...)|B2.2 re-brainstorm>`
