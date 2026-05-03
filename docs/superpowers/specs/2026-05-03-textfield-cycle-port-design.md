# TextField Cycle Port — Design

**Date:** 2026-05-03
**Investigation source:** `docs/scratch/2026-05-03-hang-rootcause-findings.md`

## Problem

`emGUIFramework.rs:1478-1497` unconditionally calls `InvalidatePainting` on the active panel every winit `about_to_wait` (≈vsync). At idle the active panel is the cosmos root `PanelId(1v1)`, so this dirties all 1024 tiles → full tree paint → ~26 ms/frame → 37 fps locked → 100 % main-thread CPU. The block was added in commit `01c3b6a7` ("feat: implement cursor blink for TextField") as a workaround for missing blink-repaint wiring. The accompanying comment claiming C++ parity is incorrect: C++ does *not* invalidate the active panel every frame.

## C++ Ground Truth

`src/emCore/emTextField.cpp:306-340` — `emTextField::Cycle()`:

- Reads `emGetClockMS()`, compares against `CursorBlinkTime`.
- On 500 ms / 1000 ms boundary, flips `CursorBlinkOn` and calls `InvalidatePainting()` (no-arg, whole panel).
- Returns `busy=true` while focused.

Engine semantics (`include/emCore/emEngine.h:222-232`) — `Cycle` returning `true` ⇒ wake on next time slice; no min-cycle interval. The Rust scheduler matches this contract (`emScheduler.rs` engine wake queue).

`emTextField::Notice` (`emTextField.cpp:343-350`) on `NF_FOCUS_CHANGED` calls `RestartCursorBlinking()` and `WakeUp()`.

## Rust Divergence (current)

- `emTextField::cycle_blink` (`emTextField.rs:2339`) does **not** call `InvalidatePainting` on flip — it only mutates state.
- `cycle_blink` is invoked from `TextFieldPanel::Paint` (`emColorFieldFieldPanel.rs:110`), not from a `Cycle` method.
- The Rust panel wrapper has no `Cycle` impl, so flip-driven invalidation never runs through the scheduler.
- The per-frame block in `emGUIFramework.rs` substitutes for the missing wiring at the cost of full-screen invalidation every vsync.

This is a fidelity bug, not a forced divergence. Per Port Ideology, the Rust shape is "idiom adaptation that diverged observably" — must be corrected.

## Design

### Architecture

Port `emTextField::Cycle` to the panel wrapper layer (`TextFieldPanel`) using the existing `PanelBehavior::Cycle` hook (`emPanel.rs:423`) and `PanelCycleEngine` adapter (`emPanelCycleEngine.rs`). Drive blink invalidation through the scheduler instead of the winit loop.

### Components

**1. `emTextField::cycle_blink` — return flip-detection signal.**

Change return value: instead of `bool busy`, return a struct or tuple indicating both `(flipped, busy)`, so the caller knows when to invalidate. Concretely:

```rust
pub struct CycleBlinkResult {
    pub flipped: bool,  // true if cursor_blink_on changed this call
    pub busy: bool,     // true while focused (engine should stay awake)
}
pub fn cycle_blink(&mut self, focused: bool) -> CycleBlinkResult { ... }
```

Existing `bool` callers (Paint sites in test panels and the production panel) will be removed in step 2 — `cycle_blink` is no longer called from Paint.

**2. `TextFieldPanel::Cycle` — drive blink, invalidate on flip.**

In `emColorFieldFieldPanel.rs`, implement the `Cycle` method on the `PanelBehavior` impl:

```rust
fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx) -> bool {
    let focused = pctx.state(self_id).in_focused_path();
    let r = self.text_field.cycle_blink(focused);
    if r.flipped {
        pctx.invalidate_painting(self_id);  // whole-panel, matching C++ no-arg form
    }
    r.busy
}
```

(Exact API for accessing `self_id` and calling `invalidate_painting` follows whatever pattern existing PanelBehavior `Cycle` impls use — see `emFileSelectionBox.rs` / `emImageFileImageFilePanel.rs`.)

Remove `self.text_field.cycle_blink(...)` from `Paint`.

**3. Focus wakeup — `Notice(FOCUS_CHANGED)`.**

Already partially wired: `TextFieldPanel::notice` (`emColorFieldFieldPanel.rs:115`) calls `self.text_field.on_focus_changed`. Extend it to also wake the panel's engine so `Cycle` starts firing:

```rust
fn notice(&mut self, flags: NoticeFlags, state: &PanelState, ctx: &mut PanelCtx) {
    if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
        self.text_field.on_focus_changed(state.in_focused_path());
        ctx.wake_up_panel_self();  // or equivalent — wakes the PanelCycleEngine
    }
}
```

**4. Delete the per-frame workaround.**

Remove `emGUIFramework.rs:1478-1497` (the active-panel invalidation block + INVAL instrumentation). The `request_redraw` chain is no longer needed for blink.

**5. Test panels.**

Apply the same Cycle/Paint refactor to:
- `crates/emtest/src/emTestPanel.rs:207`
- `crates/eaglemode/tests/golden/composition.rs:310`
- `crates/eaglemode/tests/golden/test_panel.rs:265`

These are test-only TextFieldPanel impls; they must mirror production for consistent behavior.

### Data Flow

Before:
```
about_to_wait → InvalidatePainting(active_panel) [unconditional]
              → 1024 tiles dirty → 26 ms paint
```

After (focused TextField):
```
focus_change → Notice → wake_up_panel(self)
            → scheduler queues panel's PanelCycleEngine
            → Cycle (every slice while focused)
              → cycle_blink → flipped? → InvalidatePainting (only on 500 ms boundary)
              → returns busy=true → engine stays in wake queue
```

After (no TextField focused, idle):
```
about_to_wait → DoTimeSlice → no awake engines → no work → no redraw chain
```

### Error Handling

No new error paths. `Cycle` returning `true`/`false` is the existing engine contract. Focus-change wake_up failure is impossible — the panel always has a registered engine.

### Testing

- **Existing golden tests** (`composition.rs`, `test_panel.rs`) cover the visual blink behavior; they must continue to pass after the refactor.
- **Existing unit test** `cursor_blink_cycle` (`emTextField.rs:3825`) covers `cycle_blink` semantics; update for new return type.
- **New unit test:** verify `TextFieldPanel::Cycle` calls `InvalidatePainting` only on flip (not every cycle). Use a mock PanelCtx with an invalidation counter.
- **Existing nextest suite (`cargo-nextest ntr`)** must pass — no regressions.

### Phase-E Verification Gate

After implementation, re-run hang instrumentation (`scripts/run_hang_capture.sh` + SIGUSR1 markers, ~20 s idle window):

- **Pass criterion 1:** `RENDER:paint` < 5 % of wall-clock at idle (down from 99.1 %).
- **Pass criterion 2:** Avg paint per frame < 1 ms at idle (down from 26 ms).
- **Pass criterion 3:** No regression in cursor-blink visual behavior (run `composition` and `test_panel` golden tests).

If `AW.has_awake==1` is still 100 % of slices post-fix: file a followup investigation (separate scope — identify which engine never sleeps). Idle CPU is expected to drop substantially regardless because empty paints are cheap, but `has_awake==0 at idle` is the stronger property and worth tracking.

## Out of Scope

- `emTextField` widget Cycle wiring — the widget is a helper, not an engine; `TextFieldPanel` is the engine layer.
- The `has_awake==1 always` secondary observation — handled as Phase-E followup, not part of this fix.
- Audit of other `cycle_*` helpers called from `Paint` across other panel types — separate concern, tracked only if Phase-E reveals additional issues.

## Annotations

The fix removes a divergence; the new code matches C++ structure. No `DIVERGED:` annotations needed at the fix sites. The deleted block was unannotated; its comment block is also deleted.

## Branch State

- Fix branches off `main`.
- Single commit: code change + test updates + workaround deletion + INVAL instrumentation removal.
- `instr/hang-2026-05-02` (10 commits of instrumentation) abandoned after Phase-E pass.
