# Hang root-cause findings — 2026-05-03

Investigation of GUI hang first surfaced after merging notice-dispatch-reach-loss fix `206afed0`. User report: "main thread pinning at 99.5% CPU after specific rapid zoom actions." Idle baseline is also pathological — 37 fps full-screen repaint at idle, ~26 ms paint per frame.

Branch with instrumentation: `instr/hang-2026-05-02` (do not merge).
Plan: `docs/superpowers/plans/2026-05-02-hang-instrumentation-plan.md` rev 3.

## What was actually wrong

`crates/emcore/src/emGUIFramework.rs:1483-1486`:

```rust
let active_id = win.view().GetActivePanel();
if let Some(active_id) = active_id {
    win.view_mut().InvalidatePainting(&mut sc, &tree, active_id);
}
```

This runs once per `about_to_wait` (i.e., once per scheduler tick / once per frame) and unconditionally invalidates the active panel.

Comment claims C++ parity: "matches C++ where Input() is called for all viewed panels on every frame, and emTextField invalidates itself when the blink timer fires." This is incorrect parity. C++ calls `Input()` per frame; `InvalidatePainting()` is called by the cursor-blink path *only when blink state actually flips* (and on a sub-rect). The Rust version unconditionally invalidates the whole active panel.

At startup the active panel is the cosmos root (`PanelId(1v1)`), so the per-frame invalidation marks the entire 32×32 tile grid dirty → `view.Paint(tree, …)` walks the whole panel tree → 25 ms tree-paint per frame → 37 fps locked → 100% main thread.

## Evidence

19.4 s window between two USR1 markers (`MARKER` lines in `/tmp/em_instr.phase0.log`):

| Bucket | Count | Total | % | Avg |
|---|---:|---:|---:|---:|
| `RENDER:paint` | 722 | 19.20 s | **99.1%** | 26.6 ms |
| `RENDER:present` | 722 | 0.15 s | 0.8% | 207 µs |
| `CB:about_to_wait` | 722 | 6.5 ms | 0.0% | 8 µs |
| `SLICE:scheduler` | 722 | 1.1 ms | 0.0% | 1 µs |

Per-render breakdown after Phase A instrumentation:

- `dirty_tiles = 1024 / 1024` on every frame (full-screen invalidation).
- `path = viewport_buffer` consistently (because >50% of tiles dirty).
- `tree_paint_us` ≈ 25 000 (the `view.Paint` tree walk).
- `dirty_detect_us` ≈ 0, `upload_blit_us` ≈ 1 700.
- `INVAL` lines: 696/696 with `active_id = PanelId(1v1)`.
- `AW.has_awake = 1` on 1093/1093 slices (the `request_redraw` chain at `emGUIFramework.rs:1307` is fed by an always-awake engine — secondary issue, see below).

Scheduler-internal bug class (the original v2 hypothesis) is refuted: `SLICE:scheduler` is 0.0% of wall-clock; `cycled` per slice maxes at 280; the v2 reconciliation invariant `drain == carry_in + fire + timer` held across every slice.

## Fix candidates (Phase D pending)

(a) **Delete the per-frame block.** Rely on each panel's own `Input` / `Cycle` to call `InvalidatePainting` on a sub-rect when its blink state actually changes — this is the documented C++ contract.

(b) **Gate on a blink-state flag.** Only invalidate when the cursor or some clock-driven state actually toggled this frame. Smallest diff.

(c) **Restrict the rect.** Pass the cursor's bounding rect, not the whole panel. Bounds the cost without addressing the design issue. Still a per-frame invalidation, just a smaller one.

Not yet decided. Recommendation in chat was (a). Phase D will branch off main, single commit, no instrumentation; Phase E is one human run on the rebuilt fix branch (binary pass/fail).

## Secondary observation — engine stays awake forever

`has_awake_engines() == true` on 100% of `about_to_wait` calls, including post-fix this might persist independently. The `if has_awake { request_redraw }` chain at `emGUIFramework.rs:1307` will keep firing redraws as long as some engine is awake. Worth checking after the primary fix lands whether idle CPU still has a redraw chain (i.e., is there an engine that never goes to sleep?).

If yes, Phase A row 7-LOOP-CHAIN: log per-slice the set of engines whose `Cycle` returned `stay_awake = true`, identify the offender.

## Plan-process notes (lessons folded into rev 3)

The first attempt (v2) instrumented the scheduler under an inherited "wake firehose" premise from a prior compaction summary. Phase 0 v2 captured a hang reproduction; data refuted the premise (scheduler wall-clock = 1.5%); v2's verdict matrix had no row for "the bug is not in the scheduler" and v2's hot-slice acceptance gate would have rejected the real data as "rerun." Rev 3 instruments at the system boundary first and ranks chokepoints by percentage of wall-clock between user-driven `SIGUSR1` markers, with no magic thresholds.

## Branch state

- `instr/hang-2026-05-02` — 10 commits ahead of `main`, all instrumentation. Kept for the two followups below.
- `verify/hang-2026-05-03` — fix cherry-picked onto instrumentation branch (commit `48add5ce`). Used for Phase E capture; kept for blink-regression diagnosis.
- `fix/hang-2026-05-03-textfield-cycle` — merged to `main` as `dc7bcbfd` (commit `044408b3`), branch deleted.

## Phase E result (2026-05-03)

Idle 20-second window post-fix:

| Metric | Pre-fix | Post-fix |
|---|---:|---:|
| `RENDER:paint` % wall-clock | 99.1 % | 0.4 % |
| Paint events | 722 | 6 |
| Slices in window | 1093 | 9 |
| `has_awake==1` frequency | 100 % | 66.7 % |

Hang resolved. Idle CPU dropped from saturated to genuinely quiet.

## Pending followups

### 1. Cursor blink regression

Manual GUI check after merge: cursor in a TextField does not blink at all.

The `Cycle` port is in place (commit `044408b3`) but evidently isn't producing visible flips. Likely causes to investigate, ranked by likelihood:

- `FOCUS_CHANGED` Notice never fires for the TextField the user clicked into → `is_focused` stays false → `Cycle` returns `busy=false` early → engine sleeps → no cycling.
- `wake_up_panel(id)` fires but the panel's `PanelCycleEngine` was never registered (or got unregistered) → wake is a no-op.
- `Cycle` runs but `cycle_blink` consistently takes the `< 500ms` branch because `cursor_blink_time` keeps getting reset (e.g., `RestartCursorBlinking` called every Notice or every Cycle).
- `request_invalidate_self()` is set, drained, and `view.InvalidatePainting` is called — but the dirty rect isn't reaching the renderer (different bug, not blink-specific).

`verify/hang-2026-05-03` has the full instrumentation (CB, RENDER, INVAL was deleted but the SLICE/AW lines remain). Re-running capture while clicked into a TextField should pinpoint which step is failing.

### 2. `has_awake==1` 66.7 % of slices at idle

Down from 100 %, but the plan's followup criterion is "≥ 50 % triggers investigation." Some engine returns `true` from `Cycle` (or stays in a wake queue) without an obvious driver. Candidates: cosmos root panel behavior, view animator, an update engine, a startup engine that never registers a sleep condition.

Phase A row 7-LOOP-CHAIN (from the original instrumentation plan rev 3) covers this: log per-slice the set of engines whose `Cycle` returned `stay_awake = true`, identify the offender. `instr/hang-2026-05-02` already has the wiring; the LOOP-CHAIN row needs to be added.

Both followups can use the same instrumentation branch.
