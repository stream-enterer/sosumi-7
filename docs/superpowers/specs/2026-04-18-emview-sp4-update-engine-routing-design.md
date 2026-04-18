# SP4 — emView::Update engine-only routing + Phase-8 test promotion

**Date:** 2026-04-18
**Sub-project:** SP4 of the emView subsystem closeout (see `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` §8.0).
**Closes:** §8.1 item 14 (scheduler re-entrant borrow) and item 11 (Phase-8 test promotion). Item 14 blocks item 11; one combined spec.
**Scope boundary:** align `emView::Update` dispatch with C++'s single-caller model, fix the resulting re-entrant borrow by construction, and promote `test_phase8_popup_close_signal_zooms_out` to a single-engine end-to-end run. Does **not** touch notice dispatch (SP5) or `emContext` threading (SP7).

---

## 1. Background

### 1.1 C++ reference (ground truth)

- `emView::Update()` is called from exactly one site: `UpdateEngineClass::Cycle()` at `emView.cpp:2523`.
- Every other code path that needs `Update` to run schedules it via `UpdateEngine->WakeUp()` (`emView.cpp:84, 173, 307, 1013, 1288, 1805`, and the ctor at `:84`).
- `emView`'s ctor wakes the engine immediately (`emView.cpp:84`) so the first frame runs.

### 1.2 Rust drift (items to fix)

1. **Direct non-engine call.** `emGUIFramework::about_to_wait:594` calls `win.view_mut().update(tree)` unconditionally every frame, bypassing the scheduler. C++ has no such path. The Rust wrapper `emView::update()` at `emView.rs:3845` exists *only* to serve this site; it wraps `self.Update(tree)` + a post-hoc `SetActivePanelBestPossible` fixup.
2. **Missing ctor-time wake.** `attach_to_scheduler` (`emView.rs:3044`) registers `UpdateEngineClass` but does not wake it. C++ `emView::emView` does (`emView.cpp:84`).
3. **Re-entrant scheduler borrow.** `emView::Update` at `emView.rs:2343` calls `self.scheduler.borrow().is_signaled_for_engine(close_sig, eng_id)`. When `Update` is reached through `DoTimeSlice → UpdateEngineClass::Cycle`, the caller (`emGUIFramework.rs:490`) holds `sched.borrow_mut()` across the entire slice; the inner `borrow()` panics. Today this is latent because the engine is never woken into that code path (path 1 runs `Update` outside the slice). It becomes live the moment anything wakes the update engine while the slice is running — or as soon as we unify on the engine path (this spec).
4. **Bare-view test unreachable from engine.** `UpdateEngineClass::Cycle` resolves the view via `ctx.windows.get(&self.window_id)`; bare-view tests have no window registered, so a principled single-engine promotion of `test_phase8_popup_close_signal_zooms_out` can't receive the cycle call.

Items 1, 2, and 3 are a single defect viewed from three angles: Rust runs `Update` outside its one C++ caller, and its *one* correct caller has a scheduler-borrow hazard. Fix the caller model and the hazard is structural.

### 1.3 Classification under CLAUDE.md

- Item 1 (`:594` direct call): **Rust-inertia drift**. Not forced — the engine infrastructure works. Not preserved design intent — C++ does the opposite. Must be removed.
- Item 2 (missing ctor wake): **silent drift from C++ ctor**. Must be added.
- Item 3 (re-entrant borrow): a **latent bug** produced by 1 + 2 together; dissolves when both are fixed.
- Item 4 (bare-view test): **forced, for tests only.** Production `UpdateEngineClass::Cycle` correctly requires a window; fixing this at the engine level would diverge from C++. Resolution lands in the test, not the engine.

---

## 2. Design decisions

### 2.1 Route `emView::Update` through the engine only

Delete `emGUIFramework::about_to_wait:594`'s `win.view_mut().update(tree)` call. `Update` runs only via `UpdateEngineClass::Cycle`, which already runs inside `DoTimeSlice` at `emGUIFramework.rs:491`. Every C++ `WakeUp()` site already has a Rust `WakeUpUpdateEngine()` counterpart (`emView.rs:1757, 1845, 2088, 3372, 3790`), so mutation paths already schedule the engine correctly.

The `emView::update()` wrapper (`emView.rs:3845-3859`) goes away. Its two pieces:
- `self.Update(tree)` — now reached via engine only.
- Post-hoc `SetActivePanelBestPossible(tree)` gated on active-panel invariants — this is drift. C++ calls `SetActivePanelBestPossible()` at three sites (`emView.cpp:780, 800, 901`: end of `Scroll`, `Zoom`, `ZoomOut`), not after `Update`. Resolution: append `SetActivePanelBestPossible(tree)` to the end of Rust's `Scroll` (`emView.rs:1123`), `Zoom` (`:1086`), and `ZoomOut` (`:1251`), each with a cited C++ line. The Rust-only `need_reselect`/`viewport_changed` gates in the wrapper drop; C++ has no such guard, the method is cheap, and the guard masks inherited drift.

### 2.2 Wake the update engine at attach time

Append `self.WakeUpUpdateEngine();` to the end of `attach_to_scheduler` (after `self.update_engine_id = Some(engine_id);` at `emView.rs:3067`). Matches C++ `emView::emView:84`. Ensures the first `DoTimeSlice` cycles `Update` at least once.

### 2.3 Replace the scheduler-borrow with `EngineCtx::IsSignaled`

`Update` signature becomes:
```rust
pub fn Update(&mut self, tree: &mut PanelTree, ctx: &mut super::emEngine::EngineCtx<'_>)
```

Inside, the popup-close probe at `:2336-2347` changes from:
```rust
sched.borrow().is_signaled_for_engine(close_sig, eng_id)
```
to:
```rust
ctx.IsSignaled(close_sig)
```

This is correct by construction: `UpdateEngineClass::Cycle` passes `ctx` whose `ctx.engine_id == self.update_engine_id`, and `EngineCtx::IsSignaled` compares `signal.clock > engines[ctx.engine_id].clock` — exactly what `is_signaled_for_engine(close_sig, update_engine_id)` computes today. No behavior change; the `self.scheduler` field is not touched from inside `Update`.

The `BUG` comment block at `emView.rs:2324-2335` and the `update_engine_id` field-read at `:2340` are deleted.

Rejected alternatives:
- *Cache signal state on `emView`.* (Doc §8.1 item 14 option b.) Adds a new field with no C++ analogue. `EngineCtx::IsSignaled` is already the C++-faithful mechanism (`emEngine::IsSignaled` is C++ `emView.cpp`'s own API); using it is strictly less drift.
- *Full cascade through ~25 test call sites.* (Doc §8.1 item 14 option a, unmodified.) Reaches every `view.Update(&mut tree)` test site. Unnecessary: all those tests are pinned to §2.5's test-helper refactor anyway; giving them an `EngineCtx` via a thin helper costs less than threading 25 call-site changes by hand.

### 2.4 Collapse `view.update(tree)` test call sites

The ~25 tests at `emView.rs:4691…5223` currently call `view.Update(&mut tree)` (no wrapper, no scheduler). With the new signature they need an `EngineCtx`. Provide a test-only helper on `emView`:

```rust
#[cfg(any(test, feature = "test-support"))]
pub fn pump_update_for_test(&mut self, tree: &mut PanelTree)
```

Which, when the view has a scheduler attached, wakes the update engine and runs one `DoTimeSlice` — exercising the engine path faithfully. When the view has no scheduler (a handful of tests construct bare views), it constructs a throwaway `EngineCtx` against an empty `EngineCtxInner` and calls `Update` directly. The helper is gated behind the existing `test-support` feature (already used for `pump_visiting_va`).

All 25 call sites flip from `view.Update(&mut tree)` to `view.pump_update_for_test(&mut tree)`. Mechanical rename.

### 2.5 Phase-8 test promotion

`test_phase8_popup_close_signal_zooms_out` currently asserts across two engines (Half A + Half B with a dummy engine), per closeout doc §5.1 item 5. Rewrite as a single run:

1. Build a scheduler + minimal `emWindow` (test harness — C++ needs one too) + `emView` attached.
2. Push a popup (sets `PopupWindow`, connects `close_signal` → update engine via `SwapViewPorts`).
3. Fire the popup's `close_signal`.
4. Call `scheduler.DoTimeSlice(&mut tree, &mut windows)` once.
5. Assert `popped_up == false` and the zoom state matches the post-`ZoomOut` expectation.

The "minimal `emWindow`" requirement is item 4 from §1.2. Add a test-only `emWindow::new_for_test(scheduler, ...)` constructor under the existing `test-support` feature. It produces a window with no GPU/winit surface — just enough to satisfy `ctx.windows.get(&window_id)` in `UpdateEngineClass::Cycle` and expose `view_mut()`. The integration test owns the HashMap it passes into `DoTimeSlice`.

The current inline test at `emView.rs` is replaced, not extended. Delete Half A + Half B; the new single-engine test supersedes both.

### 2.6 Scope non-goals

- Do not touch `VisitingVAEngineClass::Cycle` (same window-lookup pattern; not blocking anything in SP4).
- Do not add non-`emEngine` wake paths.
- Do not restructure `SetActivePanelBestPossible`'s call semantics beyond the audit in §2.1.

---

## 3. Blast radius

| Touch | Count | Nature |
|---|---|---|
| `emGUIFramework::about_to_wait:594` | 1 line | Delete direct `update()` call |
| `emView::update` wrapper | 1 method | Delete after audit |
| `emView::Update` signature | 1 method + 25 call sites | Add `ctx` param; call sites flip to `pump_update_for_test` helper |
| `attach_to_scheduler` | 1 line | Add `self.WakeUpUpdateEngine()` |
| Popup-close probe | ~12 lines | Replace scheduler borrow with `ctx.IsSignaled` |
| `pump_update_for_test` | new method | ~20 lines |
| `emWindow::new_for_test` | new test-only ctor | ~30 lines |
| Phase-8 test rewrite | 1 test | ~50 lines net |
| `SetActivePanelBestPossible` relocation into `Scroll`/`Zoom`/`ZoomOut` | 3 lines | Mechanical append per C++ `emView.cpp:780, 800, 901` |

Expected total: ~150 lines changed, mostly mechanical. One `DIVERGED:` removal (the `BUG` comment at `:2324-2335`). No new `DIVERGED:` markers.

---

## 4. Risks

| Risk | Mitigation |
|---|---|
| Moving `SetActivePanelBestPossible` from post-`Update` to end-of-`Scroll`/`Zoom`/`ZoomOut` changes ordering (notice dispatch vs. active-panel reselection) in a way a test/golden depends on | C++ itself uses end-of-mutator ordering; any divergence exposed is Rust drift being closed. Phase 4 runs nextest + golden; investigate any new failure under that frame |
| Tests that silently depend on `Update` running outside the scheduler (e.g., when the view is not attached) break | `pump_update_for_test` handles the no-scheduler case by constructing a throwaway `EngineCtx`. Phase 4 runs the full nextest + golden suite |
| `pump_update_for_test`'s no-scheduler path diverges from the scheduler path | Both paths call `Update` with an `EngineCtx` whose `engine_id` matches the update engine. The no-scheduler path uses a stub `EngineCtxInner` where `ctx.IsSignaled` returns `false` — correct, because no signals can be pending without a scheduler |
| Waking the engine at `attach_to_scheduler` changes observable frame-one behavior | C++ does this at ctor; any observable difference is a pre-existing divergence we're closing, not introducing |
| Popup teardown path in production depends on engine not being woken | The W3 closeout verified popup teardown runs end-to-end through the engine (§3.6 R5); the wake is already expected |

---

## 5. Success criteria

1. `cargo check` + `cargo clippy -- -D warnings` clean.
2. `cargo-nextest ntr` — 2429/2429 (baseline) or higher (SP4 adds one test, replaces two halves of another — net +0 or +1).
3. `cargo test --test golden -- --test-threads=1` — 237/243 (baseline parity; same 6 pre-existing failures).
4. `timeout 20 cargo run --release --bin eaglemode` exits 124/143 (stayed alive).
5. `grep -n "BUG (tracked as" crates/emcore/src/emView.rs` returns nothing (§8.1 item 14 marker deleted).
6. `grep -n "win.view_mut().update(tree)" crates/emcore/src/emGUIFramework.rs` returns nothing.
7. `grep -n "fn update\b" crates/emcore/src/emView.rs` returns nothing (wrapper deleted).
8. `test_phase8_popup_close_signal_zooms_out` runs `DoTimeSlice` exactly once and is documented as single-engine.

---

## 6. Out of scope — deferred to successor sub-projects

- **Notice dispatch per-view** (SP5): `emGUIFramework.rs:517-522`'s `pixel_tallness` single-window shortcut stays.
- **emContext threading** (SP7): `emView::new` signature untouched.
- **W3 surface de-dup** (SP6): unchanged.

End of SP4 design.
