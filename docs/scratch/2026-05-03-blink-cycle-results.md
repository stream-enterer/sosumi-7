# Blink-cycle chain Phase 0 results — 2026-05-03 (B2.1)

Capture: `/tmp/em_instr.blink-cycle.log` (15.3 MB, 4.7-hour total elapsed; 4-hour AFK gap between SIGUSR1 open marker and the actual user click)
Branch: `instr/blink-cycle-2026-05-03` @ `47250afe`
Run: 2026-05-03 (afternoon launch, evening interaction)

## Click target

- `target_panel_id`: `PanelId(497v1)` (emTestPanel::TextFieldPanel — `tf1` or `tf2`)
- `wall_us` of SET_ACTIVE_RESULT: `17032624265` (= 17032.624s; `window_focused=t`, `sched_some=f`)

The B2.1 click-target picker chose the latest `SET_ACTIVE_RESULT|window_focused=t` between markers. Two candidates qualified (`PanelId(3v1)` at `17032624166` and `PanelId(497v1)` at `17032624265`); `PanelId(497v1)` wins by latest-wall_us.

> **Aside (not the B2.1 verdict, but flagged for B2.2):** the chosen `SET_ACTIVE_RESULT` reports `sched_some=f`. That is the OB3 candidate the spec named ("scheduler missing at set_active_panel time") but at a *different call site* than the cursor-blink chain. B2.1's chain-trace runs forward from NOTICE_FC_DECODE; B2.2 should consider whether `set_active_panel`'s `sched_some=f` is causally linked to the OA1 outcome below.

## Decisive NOTICE_FC_DECODE event

- `wall_us`: `17032624929` (decisive = first NOTICE_FC_DECODE for click target with iap=t,wf=t after click)
- `panel_id`: `PanelId(497v1)`
- `behavior_type`: `emTestPanel::emTestPanel::TextFieldPanel`
- `in_active_path`: `t`
- `window_focused`: `t`
- `flags`: `0xf0`

## Chain trace post-decisive (5-second window, then extended to full focus-hold window)

5-second post-decisive (T_DEC=17032624929 to T_DEC+5s=22032624929):

| Line type | Count |
|---|---|
| HANDLER_ENTRY | 0 |
| WUP_RESULT | 0 |
| WAKE | 0 |
| CYCLE_ENTRY | 0 |
| BLINK_CYCLE | 0 |
| INVAL_DRAIN | 0 |

Extended to the full 46-second focus-hold window (T_DEC to T_CLOSE=17078179578):

| Line type | Count | Notes |
|---|---|---|
| HANDLER_ENTRY | **0** | *No notice handler body ran for any TextFieldPanel for the entire 46s.* |
| WUP_RESULT | **0** | *No `wake_up_panel` call ever issued during the focus hold.* |
| CYCLE_ENTRY | **0** | *No `behavior.Cycle(...)` invocation during the focus hold.* |
| BLINK_CYCLE | **0** | *No cycle_blink ran (consequence of CYCLE_ENTRY=0).* |
| WAKE | 11 | All for `emcore::emView::UpdateEngineClass` and `InputDispatchEngine` — **no PanelCycleEngine WAKE**. |
| NOTICE_FC_DECODE | 12 | 2 of these targeted TextFieldPanel (PanelId(497v1) at decisive + at focus-loss `wall_us=17065073845`). |
| INVAL_DRAIN | 0 | (consequence of CYCLE_ENTRY=0) |

Whole-log scope of HANDLER_ENTRY: **all 237 emissions occurred in the first 68 seconds of the process.** After `wall_us=68658526`, zero HANDLER_ENTRY ever fired again — not for the AFK period, not for the click event, not for the focus hold.

## Verdict

**OA1** — NOTICE_FC_DECODE present, HANDLER_ENTRY absent.

The 12 NOTICE_FC_DECODE events post-decisive prove the notice-dispatch path *reached* the dispatcher's FOCUS_CHANGED decode point (emView.rs:4243). 2 of those targeted `emTestPanel::TextFieldPanel`, yet no HANDLER_ENTRY fired from the impl (which sits at emTestPanel.rs:265 and emColorFieldFieldPanel.rs:158, immediately inside `if flags.intersects(FOCUS_CHANGED)`).

Sites between NOTICE_FC_DECODE emit (emView.rs:4244) and HANDLER_ENTRY emit (inside the TextFieldPanel impl):
- `behavior.notice(flags, &state, &mut ctx)` — sequential, no early-return.

Possibilities for behavior.notice running but not reaching HANDLER_ENTRY:
1. The dispatched `behavior` is NOT a TextFieldPanel impl despite `behavior_type` reporting it as such (vtable trap, similar to but not identical to the prior `project_isactive_bug.md` cdylib trap — which was resolved by listing emtest in `[dependencies]`).
2. `behavior.notice` is called but on a stale cdylib copy whose source predates Task 2.
3. The dispatch panics/aborts silently between NOTICE_FC_DECODE emit and the FOCUS_CHANGED branch entry — no panic stack would be in this log without an `eprintln!`.

(Verified: `strings target/release/deps/libemTestPanel.so | grep HANDLER_ENTRY` shows the format string IS in the loaded cdylib, ruling out direct staleness in the binary on disk. But "loaded into running process via dlopen" vs. "newer file on disk" can diverge if dlopen happened earlier.)

## Dispatch

`Re-brainstorm B2.2 (handler not invoked: panic / vtable / wrong impl)`

Per the B2.1 spec § Phase 0 outcome dispatch: OA1 routes to Task 22 (B2.2 re-brainstorm), not to a Phase 1 fix.

The B2.1 truth table successfully isolated the bug to the BEFORE-handler region; the chain analysis between NOTICE_FC_DECODE emit and HANDLER_ENTRY emit must be probed by B2.2.

## Prediction calibration (advisor's check)

- **Pre-measurement priors** (from spec Phase 0): 50% OB2, 25% OB3, 10% OC-NOPICKUP, 5% OC-DISPATCH, 5% OD2, 3% OD3, 2% other.
- **Actual**: OA1 (which falls in the 2% "other" prior).
- **Retrospective**:
  - The prior was *miscalibrated* on the location of the break. I expected the chain to break at engine binding (OB2) or cycle dispatch (OC-DISPATCH); it actually broke before HANDLER_ENTRY ran at all.
  - The OA1 bin was not weighted as "low likelihood" arbitrarily — the priors assumed the FU-005-class fixes (NOTICE delivery to TextField) had landed. Both *did* land. So OA1 hitting indicates a **second**, distinct break in the dispatch chain that B2's instrumentation could not see.
  - The B2.1 instrumentation worked as designed: it surfaced "HANDLER_ENTRY count 0 in the active window despite NOTICE_FC_DECODE present" — a precise, mechanically-verifiable outcome that would have been ambiguous in B2.
  - **Lesson for B2.2**: the bug class lives in the gap between `behavior_type` lookup (used by NOTICE_FC_DECODE) and the actual `behavior.notice` dispatch call. Investigations should probe: (a) is the in-memory `behavior` for PanelId(497v1) at click time really an `emTestPanel::TextFieldPanel`? (b) does `behavior.notice` reach the impl's body, or is some intermediate wrapper consuming the notice silently?
  - **Aside open thread**: SET_ACTIVE_RESULT.sched_some=f at the click moment is a separate observable departure from the FU-005 expected state. B2.2 should weigh whether that's causal or coincidental.

## Whole-process activity profile

- HANDLER_ENTRY range: `1565545` (1.56s) → `68658526` (68.7s). **Zero events after 68.7 seconds.**
- WUP_RESULT range: 660 → 17017668890 (17017s). 62 events total; some during pre-AFK initialization, some during click.
- CYCLE_ENTRY range: 355455 (0.36s) → 17030861815 (17030s). 367 events; 290 of them on `PanelId(3v1)` (an emSubViewPanel). **Zero CYCLE_ENTRY events for PanelId(497v1) (the TextFieldPanel) ever in the log.**
- BLINK_CYCLE: **0 in the entire 4.7-hour log.** (cycle_blink never ran; no engine ever Cycled the TextFieldPanel.)

The "0 CYCLE_ENTRY ever for the click target" is consistent with OA1: if HANDLER_ENTRY never fires, RestartCursorBlinking + wake_up_panel never run, no engine wake for that panel's PanelCycleEngine, no Cycle, no BLINK_CYCLE.

CYCLE_ENTRY's `behavior_type` field reads `dyn emcore::emPanel::PanelBehavior` for all 367 events — `std::any::type_name_of_val(&*behavior)` returns the trait-object type name, not the concrete type. This means the analyzer cannot use `behavior_type` to distinguish which concrete impl was Cycled at each site; B2.2 may need a different mechanism (e.g., behavior pointer address logged + cross-referenced) for that distinction.

## Full analyzer report

Saved at `/tmp/blink-cycle-report.txt`. Excerpt of relevant tail:

```
## Phase 0 verdict: O1
  decisive_event: wall_us=17019261600, panel_id=PanelId(71v1), behavior_type=emcore::emColorFieldFieldPanel::TextFieldPanel
    in_active_path=False, window_focused=False, flags=0x3ff, branch_fired=False

## Phase 0 dispatch
→ Phase 1a: in_active_path stale; fix in set_active_panel/build_panel_state

## B2.1 verdict: OA1
  evidence: NOTICE_FC_DECODE present, HANDLER_ENTRY absent
## B2.1 dispatch: Re-brainstorm B2.2 (handler not invoked: panic / vtable / wrong impl)
```

Note: B2's path-trace verdict (`O1`) targeted `PanelId(71v1)` (the production TextField under control-panel area) per the legacy heuristic. B2.1's revised picker chose `PanelId(497v1)` (the test-panel TextField the user actually clicked into, in the test cosmos area). Both targets receive zero HANDLER_ENTRY in the active window, so the OA1 verdict is robust regardless of target-picker choice.

**Next phase dispatched:** Task 22 — B2.2 re-brainstorm. Per spec Phase 0 outcome dispatch, the OA1 bin (handler not invoked despite NOTICE_FC_DECODE present) is out of B2.1's ≤30 LOC fix budget; the candidate space for B2.2 is the gap between `behavior_type` lookup and `behavior.notice()` invocation in `emView.rs:4204-4245`. Tasks 13-21 are skipped (no Phase 1 fix lands in B2.1).

---

## Handoff to B2.2 brainstorm

Phase 0 verdict was OA1, which means **the notice-dispatch chain reaches NOTICE_FC_DECODE emit (emView.rs:4243) but does not reach the TextFieldPanel impl's body (HANDLER_ENTRY emit at emTestPanel.rs:265 / emColorFieldFieldPanel.rs:158)** in the focus-hold window. B2.1 closes here without a fix landed.

### What we learned

- B2.1's 3-line instrumentation (HANDLER_ENTRY, WUP_RESULT, CYCLE_ENTRY) cleanly distinguished the 9-bin truth table; the capture identified OA1 unambiguously.
- The bug class is upstream of the post-handler chain. *All* of {OB1, OB2, OB3, OC-NOPICKUP, OC-DISPATCH, OD2, OD3, OD-OK} are ruled out — none required: HANDLER_ENTRY is absent, so no downstream link is reachable.
- **All 237 HANDLER_ENTRY events in the entire 4.7-hour log occurred in the first 68 seconds of process lifetime.** After `wall_us=68658526` there is a ~17000-second gap with zero HANDLER_ENTRY emissions, including across the user's actual click and 30-second focus hold.
- 12 NOTICE_FC_DECODE events fired in the focus-hold window, 2 of them targeting `emTestPanel::TextFieldPanel` (PanelId(497v1)); zero HANDLER_ENTRY for any panel followed.
- The cdylib's instrumentation IS compiled in (`strings target/release/deps/libemTestPanel.so | grep HANDLER_ENTRY` confirms presence), so this is not the prior `project_isactive_bug.md` cdylib-staleness trap directly.

### What remains unknown

The four candidate root causes for B2.2:

1. **Vtable corruption / wrong impl bound at click time.** The `behavior` for PanelId(497v1) reports `behavior_type=emTestPanel::emTestPanel::TextFieldPanel` at NOTICE_FC_DECODE time (emView.rs:4204 reads `tree.behavior_type_name(id)`). But that's a *type-name string*, not the actual `Box<dyn PanelBehavior>` vtable. Possibility: the type name lookup and the actual stored behavior diverge (e.g., type_name cached at registration but behavior swapped via take_behavior/put_behavior pair).
2. **Silent panic between NOTICE_FC_DECODE and behavior.notice.** Lines 4244 and 4245 are sequential; a panic between them would kill the panel handler chain. Investigation: add an EMIT_DELIVERED line right after `behavior.notice(...)` returns to detect whether the call returned at all.
3. **Cdylib in-memory copy vs. on-disk file divergence.** dlopen can pin an in-memory image of the .so. If anything (test runner, `cargo build`, etc.) recompiled the .so between dlopen and click, behavior could diverge from disk. We did NOT explicitly verify that the running cdylib was loaded AFTER the most recent emtest source change.
4. **Activity drop-off pattern correlates with something else.** All HANDLER_ENTRY events stop at ~68s. What system event happens around 68 seconds into the process? (Possibly: panel-tree settles, activations stop firing FOCUS_CHANGED, panels go into a "warm" steady state.) If the 4-hour AFK plus the click triggered a notice-delivery code path that was never exercised in the first 68s, that path may have a permanent bug.

The aside on `SET_ACTIVE_RESULT.sched_some=f` at click time is a separate observable that B2.2 should triangulate — it might be coincident, or it might block notice-delivery causally.

### What B2.2 instrumentation should add

Beyond what B2.1 already covers:

- **DELIVERED line** immediately after `behavior.notice(...)` returns at emView.rs:4246 — distinguishes "behavior.notice ran but didn't enter FOCUS_CHANGED branch" vs. "behavior.notice never ran".
- **BEHAVIOR_PTR line** at the dispatch site that logs the actual vtable pointer of the dispatched `behavior` — distinguishes "right type name, wrong vtable" trap.
- **PROCESS_STATE / SCHEDULER_STATE line** at periodic intervals — surfaces whether the activity drop-off at 68s is caused by some event-loop state change.

### Next step

Invoke `superpowers:brainstorming` for B2.2 with this findings doc and the capture log as input. The candidate space is much smaller than B2.1's — only 4 candidates above, each with a specific instrumentation strategy.
