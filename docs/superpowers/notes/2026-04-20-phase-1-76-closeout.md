# Phase 1.76 — `PanelBehavior::Input` throwaway scheduler elimination — Closeout

**Branch:** `port-rewrite/phase-1-76`
**Commits:** `20ddfa5c` (bootstrap) … `0861b098` (E039 JSON resolved) … final closeout.
**Status:** COMPLETE — all C1–C11 invariants SAT. No deferrals — E039 resolved.

## Summary

Phase 1.76 closes JSON entry E039, the sole `deferred-phase-1-76` item left by Phase 1.75.
The `throwaway_sched_input` local `EngineScheduler` at `emSubViewPanel.rs:301` — which silently
dropped all wakes emitted by `set_active_panel` and `Update` during mouse/touch input dispatch —
is eliminated. `PanelBehavior::Input` now carries a `ctx: &mut PanelCtx` parameter matching the
`notice(&mut PanelCtx)` pattern established in Phase 1.75 Task 5. The cascade updated the trait
default, 19 production overrides, and ~25 test overrides across 29 files; `emWindow::dispatch_input`
constructs a `PanelCtx::with_scheduler` per panel. Wakes emitted during sub-view input dispatch now
reach the real outer scheduler, matching C++ behavior. All gates held at baseline (nextest 2454/0/9,
goldens 237/6 identical failure set, clippy clean, fmt clean); no metric shifted.

## Delta from baseline

| metric              | baseline | exit | delta |
|---------------------|---------:|-----:|------:|
| nextest passed      |     2454 | 2454 |     0 |
| nextest failed      |        0 |    0 |     0 |
| nextest skipped     |        9 |    9 |     0 |
| goldens passed      |      237 |  237 |     0 |
| goldens failed      |        6 |    6 |     0 |
| rc_refcell_total    |      283 |  283 |     0 |
| diverged_total      |      176 |  176 |     0 |
| rust_only_total     |       17 |   17 |     0 |
| idiom_total         |        0 |    0 |     0 |
| try_borrow_total    |        0 |    0 |     0 |

See `2026-04-20-phase-1-76-exit.md` for per-metric notes.

## JSON entries closed

- **E039** — `PanelBehavior::Input` throwaway `EngineScheduler::new()`. Resolved by KEYSTONE Task 2
  commit `48466f41`. `resolution_commit` set in `2026-04-19-port-divergence-raw-material.json`.

No other `deferred-phase-1-76` entries remained in JSON: `rg '"status": "deferred-phase-1-76"'` → 0 matches.

## Invariants verified

Phase 1.76 new invariants:

| ID | Status |
|----|--------|
| I-1.76-throwaway — no production throwaway scheduler in `emSubViewPanel.rs` | SAT |
| I-1.76-signature — `PanelBehavior::Input` trait default carries `_ctx: &mut PanelCtx` | SAT |
| I-1.76-cascade — all production call sites pass `&mut PanelCtx` (clippy clean) | SAT |
| I-1.76-no-new-hacks — `#[allow(...)` count outside whitelist unchanged at 22 | SAT |

Phase 1.75 carry-forwards: I1, I1c, I1d, I-Y3-dispatch, I-T3a, I-T3b, I-T3c,
I-Spec-3.3-clarified, Task-10, Task-11 — **all SAT** (evidence in exit doc).

Phase 1 carry-forwards: I1a, I1b, I6 — **all SAT**.

## No deferrals

No new deferred entries surfaced during Phase 1.76. The descoped items from the plan
(emColorField::Input inherent helper, emView::Input inherent method, emViewAnimator trait Input)
were confirmed non-issues during Task 2 audit — none fire signals or wake engines during Input.

## Next phase

Phase 2 — view/window composition and back-ref migration. Phase 2's B4 predecessor check
accepts `port-rewrite-phase-1-76-complete` as the second COMPLETE phase in the series
(after `port-rewrite-phase-1-75-complete`).
