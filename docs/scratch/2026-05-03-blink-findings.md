# Blink path-trace findings — 2026-05-03

Capture: `/tmp/em_instr.blink.log` (test-panel TextField, post-fix branch)
Branch: `instr/hang-2026-05-02` @ `instr-7-loop-chain` (`38f1b9e1`)

## Path-trace verdict (transition)

Focus-change identified at +8.44s after open marker
(`PanelId(497v1)`, `emTestPanel::emTestPanel::TextFieldPanel`).

- ✓ **NOTICE FOCUS_CHANGED → TextFieldPanel** —
  `NOTICE|wall_us=77680728|recipient_panel_id=PanelId(497v1)|flags=0xf0`
  (flags 0xf0 = ENABLE_CHANGED|FOCUS_CHANGED|ACTIVE_CHANGED|VIEWING_CHANGED)
- ✗ **WAKE from `notice()` → wake_up_panel** — *zero* WAKE entries from
  `crates/emtest/src/emTestPanel.rs:1555` in the post-focus window
  (77.68s → 146.98s wall, 69.3s span). The test-panel `notice` impl's
  `if self.is_focused { ... ctx.wake_up_panel(id); }` branch did **not**
  fire on the focused panel.
- ✗ Engine `WAKE` to any `PanelCycleEngine` post-focus — 0
- ✗ `STAYAWAKE` for any `PanelCycleEngine` post-focus — 0
- ✗ `BLINK_CYCLE` post-focus — 0 (panel's `Cycle` never ran)
- ✗ `INVAL_REQ` post-focus — 0 (no invalidation request, by consequence)

The `Engine REGISTER for PanelCycleEngine` row in the analyzer's report
is a heuristic limitation: PanelCycleEngine `scope` records the outer
SubViewPanel id (e.g. `PanelId(2v1)`), not the inner panel's id; the
analyzer's match on inner panel id within scope therefore fails. Not
load-bearing here — the chain breaks earlier, at the `notice → wake_up_panel`
edge.

## Identified break

**`TextFieldPanel::notice` does not wake its engine on focus-gain.**

The notice handler receives FOCUS_CHANGED and, per the merged-in fix
(commit `044408b3`), is supposed to:

```rust
if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
    self.is_focused = state.in_focused_path();
    self.widget.on_focus_changed(self.is_focused);
    if self.is_focused {
        self.widget.RestartCursorBlinking();
        let id = ctx.id;
        ctx.wake_up_panel(id);     // <-- this does not fire
    }
}
```

The simplest hypothesis consistent with the data: `state.in_focused_path()`
returned `false` even on the panel that just gained focus, so the focused
branch was skipped and the engine was never woken.

`state.in_focused_path()` walks the panel's path to the root, checking
that every link is a focused child. It can return false for a focus-gain
notice if (a) the focused-path bookkeeping in `emPanelTree` lags behind
the FOCUS_CHANGED notice dispatch, or (b) the `state` snapshot built for
this notice was constructed before the focused-path update. Both are
real possibilities and need to be distinguished.

## Steady-state aggregation (post-click)

| line type | count in 70s focused-idle window |
|---|---:|
| NOTICE (all) | 46,267 |
| WAKE (all) | 12,216 |
| STAYAWAKE (all) | 1,316 |
| INVAL_DRAIN | 306 (all `drained=f`) |
| BLINK_CYCLE | 0 |
| INVAL_REQ | 0 |

The cosmos is not idle (heavy NOTICE/WAKE traffic), but none of it is the
TextField's blink path.

## Contingency check

Not triggered. The path-trace shows a clear ✗ at the notice→wake edge;
no need to run the A2-prod contingency (which is for the
"every step ✓ but blink visually broken" case).

## Verdict

The blink regression that triggered this investigation is caused by the
TextFieldPanel's `notice` handler not waking up its engine on focus-gain.
The most likely root cause is `state.in_focused_path()` returning `false`
when called from the FOCUS_CHANGED notice — either because the notice
dispatch fires before `in_focused_path` is updated, or because the
`PanelState` snapshot is built from stale data.

The fix candidate is therefore **upstream of the notice itself**: ensure
`state.in_focused_path()` is consistent with the FOCUS_CHANGED bit at
notice-dispatch time. Comparison against C++ behavior is needed — in
C++, `emTextField::Notice` reads `IsInFocusedPath()` directly from the
panel object, not from a state snapshot, and that direct read happens
*after* the focus-path bookkeeping completes. The Rust port may have
introduced a snapshot ordering divergence.

## Next steps

- [ ] Add ad-hoc instrumentation: log `(panel_id, in_focused_path)` from
  inside `TextFieldPanel::notice` for FOCUS_CHANGED notices, recapture.
  Confirms or refutes the `in_focused_path() == false` hypothesis directly.
- [ ] Read C++ `emPanel::SignalChildrenInFocusedPath` /
  `emPanel::PrivLayout` and trace when in-focused-path bookkeeping is
  updated relative to FOCUS_CHANGED notice dispatch. Compare against
  Rust `emPanelTree::set_focus` (or equivalent) and the notice-dispatch
  reach.
- [ ] Spec B2 once root cause is named: realign Rust focus-path
  bookkeeping with C++ ordering, or change the test-panel/production
  `TextFieldPanel::notice` to read in-focused-path from a more
  authoritative source.
- [ ] Revisit analyzer's PanelCycleEngine→panel mapping heuristic so the
  next path-trace can name the engine cleanly.
