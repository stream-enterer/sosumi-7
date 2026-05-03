# Blink path-trace Phase 0 results — 2026-05-03

Capture: `/tmp/em_instr.blink-trace.log` (9.5 MB, 2 markers)
Branch: `instr/blink-trace-2026-05-03` @ `71260017`
Run: 2026-05-03 12:51 (open marker wall_us=19288047, close marker wall_us=87398740)

## Headline verdict (analyzer)

**O1** — `in_active_path` stale at notice time.

Decisive event picked by the analyzer's path-trace heuristic:
- `wall_us=35453391`, `panel_id=PanelId(19v5)`, `behavior_type=emTestPanel::emTestPanel::TextFieldPanel`
- `in_active_path=False`, `window_focused=False`, `flags=0x3ff`, `branch_fired=False`

Per the spec dispatch table, O1 → Phase 1a (fix in `set_active_panel` /
`build_panel_state`).

## Re-analysis (manual) — verdict is misleading

The analyzer picked PanelId(19v5) as the focus-transition target, but
that panel is part of a **layout-driven blanket FOCUS_CHANGED flush** at
wall_us≈35453000–35453500 affecting *every* panel in the test sub-view
with `iap=f, wf=f` regardless of behavior type. It is not the user's
click event.

The user's actual click landed at **wall_us=48655320** on
**PanelId(125v1)** — see `SET_ACTIVE_RESULT|target_panel_id=PanelId(125v1)|window_focused=t|notice_count=12`,
with the matching FOCUS_CHANGED notice for that panel at:

```
NOTICE_FC_DECODE|wall_us=48655524|panel_id=PanelId(125v1)|behavior_type=emTestPanel::emTestPanel::TextFieldPanel|in_active_path=t|window_focused=t|flags=0xf0
```

So at the *real* click target the focus-path bookkeeping is **correct
at notice dispatch time**: `iap=t`, `wf=t`, hence
`state.in_focused_path()=true`. The notice handler's `if self.is_focused`
branch *should* run.

**Yet zero `WAKE` entries from `emTestPanel.rs:255` (the
`ctx.wake_up_panel(id)` call in the focused branch) appear in the entire
log.** The only WAKEs from `emTestPanel.rs` come from lines 1555, 2370,
3200 — none of which is the cursor-blink wake site. This means the
focused-branch wake call is either not running, or it is running and
silently no-opping inside `wake_up_panel`.

`wake_up_panel` has two early-return guards (`emEngineCtx.rs:847`):

```rust
pub fn wake_up_panel(&mut self, id: PanelId) {
    let Some(panel) = self.tree.GetRec(id) else { return; };
    let Some(eid) = panel.engine_id else { return; };       // ← silent
    if let Some(sched) = self.scheduler.as_deref_mut() {    // ← silent
        sched.wake_up(eid);
    }
}
```

If `panel.engine_id` is `None` (the cursor-blink engine is not yet
registered for this panel) or `self.scheduler` is `None` (the
notice-dispatch `PanelCtx` was built without a scheduler), the wake
silently no-ops without producing a `WAKE` log line.

Reframing the actual data: `iap=t && wf=t && !branch_fired` matches the
truth-table row **O3-AMBIG**, not O1. Per the spec, O3-AMBIG → "STOP:
re-brainstorm B2.1 (impossible-row outcome; investigate)".

## Branch fired (WAKE within 100ms window)?

**For the analyzer's chosen target (PanelId(19v5)):** `f`
**For the actual click target (PanelId(125v1)):** `f`

Either way: `branch_fired=False`.

## Verdict

- **As reported by the analyzer:** O1
- **As supported by the data when retargeted to the user click:** O3-AMBIG

Per B2 spec dispatch:
- O1 → proceed to Phase 1a
- O2 → proceed to Phase 1b
- O3 / O3-AMBIG / O4 → STOP, re-brainstorm B2.1

The data fits O3-AMBIG. Proceeding to Phase 1a on the analyzer's stated
verdict would be acting on a wrong target panel and a stale-flush event
that is not the bug under investigation.

## Prediction calibration (advisor's check)

- Pre-measurement priors: 60% O1, 25% O2, 10% O3, 5% O4
- Headline (analyzer): O1 — within the modal prior; falsely calibrated.
- Real (after retargeting): O3-AMBIG — within the long tail (~10%).
- Retrospective: code reading suggested both `iap` and `wf` should be
  true at notice dispatch. The data confirms this for the actual click
  target. The bug is *downstream* of the focus-path system, in the
  notice handler → `wake_up_panel` → engine registration chain, not in
  the focus-path bookkeeping. The 60%-O1 prior was wrong. The original
  A2 hypothesis ("`state.in_focused_path()` returned false") is also
  wrong — it returns true; the bug is one layer further down.

## Analyzer-heuristic finding (B2 follow-up)

The analyzer's blink-command path-trace identifies the focus-transition
target by scanning ACTIVATE/transition events early in the post-marker
window. It locked onto a layout-driven FOCUS_CHANGED flush at
wall_us=35453000–35453500 instead of the user's click at
wall_us=48655320. This is a re-discovery of the same heuristic
limitation flagged in the A2 findings (PanelCycleEngine scope mapping)
and should be tracked for analyzer hardening, but it is not load-bearing
for the B2 investigation now that the data has been read manually.

## Full analyzer report

```
## Path-trace verdict (transition)

Focus-change identified at +16165.3ms (`PanelId(19v5)`, `emTestPanel::emTestPanel::TextFieldPanel`).

- ✓ **NOTICE FOCUS_CHANGED → TextFieldPanel** — `NOTICE|wall_us=35453390|recipient_panel_id=PanelId(19v5)|flags=0x3ff`
- ✗ **Engine REGISTER for PanelCycleEngine** — no REGISTER record matches target panel

## Identified break

First ✗: **Engine REGISTER for PanelCycleEngine**.

_Next step: spec B2 — investigate Engine REGISTER for PanelCycleEngine._


## Phase 0 verdict: O1
  decisive_event: wall_us=35453391, panel_id=PanelId(19v5), behavior_type=emTestPanel::emTestPanel::TextFieldPanel
    in_active_path=False, window_focused=False, flags=0x3ff, branch_fired=False

## Phase 0 dispatch
→ Phase 1a: in_active_path stale; fix in set_active_panel/build_panel_state
```

## Supporting raw events (manual reanalysis)

```
SET_ACTIVE_RESULT|wall_us=48655320|target_panel_id=PanelId(125v1)|window_focused=t|notice_count=12|sched_some=f
NOTICE|wall_us=48655523|recipient_panel_id=PanelId(125v1)|recipient_type=emTestPanel::emTestPanel::TextFieldPanel|flags=0xf0
NOTICE_FC_DECODE|wall_us=48655524|panel_id=PanelId(125v1)|behavior_type=emTestPanel::emTestPanel::TextFieldPanel|in_active_path=t|window_focused=t|flags=0xf0
[next 100ms: 8 WAKE entries; none from emTestPanel.rs:255]
```

**Next phase dispatched:** B2.1 re-brainstorm (skip Tasks 12–17).

## Handoff to B2.1 brainstorm

Phase 0 verdict (when read from the actual user click event, not the
analyzer's heuristic-chosen target) is **O3-AMBIG**, which means the
focus-path bookkeeping system is NOT the bug. B2 closes here without a
fix landed.

### What we learned

Phase 0 successfully ruled out:

- `in_active_path` stale at notice dispatch (it is `t` at the real
  click target).
- `window_focused` stale at notice dispatch (it is `t` at the real
  click target).
- `state.in_focused_path()` returning false despite both flags being
  true (the formula is `in_active_path && window_focused`, both `t`).

Phase 0 also surfaced:

- A **silent-no-op trap in `emEngineCtx::wake_up_panel`**
  (`crates/emcore/src/emEngineCtx.rs:847`): two early-return guards
  (`panel.engine_id == None`, `self.scheduler == None`) skip the wake
  without emitting a `WAKE` log line. This is the most likely bug
  vector for the blink regression.
- A **path-trace heuristic limitation in
  `scripts/analyze_hang.py`**: the blink command's transition detector
  picked a layout-driven blanket FOCUS_CHANGED flush as the focus
  event, not the user's click. Re-targeting the verdict logic to "the
  panel reported by `SET_ACTIVE_RESULT` with `window_focused=t` after
  the open marker" would pick the right target.
- The **B1 D1 deferral** (set_active_panel missing
  WakeUpUpdateEngine) — see
  `docs/scratch/2026-05-03-set-active-panel-missing-wake.md`.

### What remains unknown

- Whether `panel.engine_id` is actually `None` for the focused
  TextFieldPanel at notice dispatch (i.e., the cursor-blink engine is
  not yet registered when the FOCUS_CHANGED notice fires). Likely
  cause: `RestartCursorBlinking` registers the engine *during* the
  notice handler but the wake call uses a stale read of
  `panel.engine_id`.
- Whether `PanelCtx::scheduler` is `Some` at notice dispatch time.
  Notice delivery in `handle_notice_one` builds a
  `PanelCtx::with_sched_reach_optional_roots(...)` — the
  "sched-optional" suggests this can be None depending on the call
  path.
- Why the previous A2 hypothesis ("`state.in_focused_path()` returns
  false") was ever supported by data — this run shows it returns true.
  Either A2's path-trace heuristic also picked the wrong target, or
  the bug is intermittent.

### Next step

Invoke `superpowers:brainstorming` for **B2.1** with this findings doc
and the capture log (`/tmp/em_instr.blink-trace.log`) as input. The
candidate space for B2.1 is much smaller than B2's:

1. `wake_up_panel` early-return on `panel.engine_id == None` — verify
   by adding instrumentation logging both guards' outcomes.
2. `wake_up_panel` early-return on `scheduler == None` — same.
3. `RestartCursorBlinking` not registering the cursor-blink engine on
   the panel before `wake_up_panel` is called.
4. `RestartCursorBlinking` registering an engine that is not bound to
   `panel.engine_id` (so wake target is wrong even when an engine
   exists).

The first two are easy to falsify with a 2-line instrumentation patch
on `instr/blink-trace-2026-05-03`. (3) and (4) require reading C++
`emTextField::RestartCursorBlinking` and the Rust port's engine
registration flow.
