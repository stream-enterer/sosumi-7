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

### 2.3 Cache popup-close signal state in `UpdateEngineClass::Cycle`

**C++ context.** C++ `emView` is itself an `emEngine` (via `class emView : public emContext` and `class emContext : public emEngine`, `emContext.h:44`). `emView::Update` at `emView.cpp:1299` calls `IsSignaled(close_signal)` against `emView`'s *own* engine clock — not `UpdateEngine`'s. The connection at `emView.cpp:1642` (`UpdateEngine->AddWakeUpSignal(close_signal)`) separately arranges for the engine to wake.

**Rust forced divergence.** Rust's `emView` is not an `emEngine` (no `emContext` threading yet; tracked as SP7). Rust substitutes `is_signaled_for_engine(close_sig, update_engine_id)` — observationally equivalent as long as the update engine's clock tracks the view's notional "engine tick" — but this requires an `&EngineScheduler` borrow from inside `Update`, producing the re-entrant panic.

**Resolution.** Move the signal probe *out* of `Update` and *into* `UpdateEngineClass::Cycle`, which already holds an `&mut EngineCtx` — no scheduler borrow needed. `Cycle` writes the result to a transient view field; `Update` reads and clears it. Update's signature does not change.

Field added to `emView`:
```rust
/// Set by `UpdateEngineClass::Cycle` from `ctx.IsSignaled(close_signal)`
/// before calling `Update`; read and cleared at the top of `Update`.
/// Stands in for C++ `IsSignaled(PopupWindow->GetCloseSignal())` in
/// `emView::Update` (emView.cpp:1299). DIVERGED: C++ emView inherits
/// from emEngine (via emContext), so the IsSignaled call there is
/// against emView's own clock. Rust emView is not an emEngine (SP7
/// will revisit); the nearest correct clock is UpdateEngine's, and
/// UpdateEngineClass::Cycle is the natural site to observe it.
pub(crate) close_signal_pending: bool,
```

`UpdateEngineClass::Cycle` body becomes:
```rust
fn Cycle(&mut self, ctx: &mut super::emEngine::EngineCtx<'_>) -> bool {
    if let Some(win_rc) = ctx.windows.get(&self.window_id) {
        let win_rc = Rc::clone(win_rc);
        let mut win = win_rc.borrow_mut();
        let view = win.view_mut();
        // Mirror C++ emView.cpp:1299 popup-close probe, using the
        // UpdateEngine's clock as the observational stand-in for
        // emView's own clock (Rust emView is not an emEngine; see
        // close_signal_pending doc comment).
        if let Some(popup) = view.PopupWindow.as_ref() {
            let close_sig = popup.borrow().close_signal;
            view.close_signal_pending = ctx.IsSignaled(close_sig);
        }
        view.Update(ctx.tree);
    }
    false
}
```

`emView::Update` popup-close block at `:2336-2347` becomes:
```rust
let popup_closed = std::mem::take(&mut self.close_signal_pending);
if popup_closed {
    self.ZoomOut(tree);
}
```

The `BUG` comment block at `emView.rs:2324-2335` and the `update_engine_id` field-read at `:2340` are deleted. `self.scheduler` is no longer touched from inside `Update`.

Rejected alternatives:
- *Add `ctx: &mut EngineCtx` to `Update` signature.* Cascades through 141 call sites across 20+ files. All would need `Option<&mut ctx>` plumbing or a stub-ctx test helper. The cached-field path is behaviorally identical with ~5 touched lines of production code.
- *Keep the `self.scheduler.borrow()` path but scope the outer `borrow_mut()` so the slice releases the scheduler during `Cycle`.* Breaks `DoTimeSlice`'s invariants (it needs the borrow for the whole slice) and fights the C++ design rather than mirroring it.

### 2.4 Test call sites unchanged

With §2.3's cached-field approach, `Update`'s signature does not change. All 141 `.Update(&mut tree)` call sites across tests and production remain as-is. Tests that do not attach a scheduler will simply never observe `close_signal_pending == true` (no engine ever writes it), which is correct — those tests don't exercise popup close.

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
- Do not restructure `SetActivePanelBestPossible`'s call semantics beyond the relocation into `Scroll`/`Zoom`/`ZoomOut` specified in §2.1.

---

## 3. Blast radius

| Touch | Count | Nature |
|---|---|---|
| `emGUIFramework::about_to_wait:594` | 1 line | Delete direct `update()` call |
| `emView::update` wrapper | 1 method | Delete after audit |
| `emView::Update` signature | unchanged | Cached-field approach — no cascade |
| `emView::close_signal_pending` field | new | 1 line + doc |
| `UpdateEngineClass::Cycle` | ~10 lines | Write `close_signal_pending` from `ctx.IsSignaled` before calling `Update` |
| `attach_to_scheduler` | 1 line | Add `self.WakeUpUpdateEngine()` |
| Popup-close probe in `Update` | ~12 lines → 4 lines | Replace scheduler borrow with `mem::take(&mut self.close_signal_pending)` |
| `emWindow::new_for_test` | new test-only ctor | ~30 lines |
| Phase-8 test rewrite | 1 test | ~50 lines net |
| `SetActivePanelBestPossible` relocation into `Scroll`/`Zoom`/`ZoomOut` | 3 lines | Mechanical append per C++ `emView.cpp:780, 800, 901` |

Expected total: ~100 lines changed. One `BUG` comment removed (`:2324-2335`). One new `DIVERGED:` comment added (on `close_signal_pending` — documents the emEngine-inheritance substitution, replacing the current `BUG` marker).

---

## 4. Risks

| Risk | Mitigation |
|---|---|
| Moving `SetActivePanelBestPossible` from post-`Update` to end-of-`Scroll`/`Zoom`/`ZoomOut` changes ordering (notice dispatch vs. active-panel reselection) in a way a test/golden depends on | C++ itself uses end-of-mutator ordering; any divergence exposed is Rust drift being closed. Phase 4 runs nextest + golden; investigate any new failure under that frame |
| Tests that relied on `emView::update()` wrapper's post-hoc `SetActivePanelBestPossible` fail once the wrapper is deleted | Mitigated by §2.1 relocation of that call into `Scroll`/`Zoom`/`ZoomOut`; any remaining failure means the test was exercising drift and should be rewritten to match C++ ordering |
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
