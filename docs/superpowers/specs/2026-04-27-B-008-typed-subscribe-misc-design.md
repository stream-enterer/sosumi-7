# B-008-typed-subscribe-misc — Design

**Date:** 2026-04-27
**Status:** Merged at `133de22e` (2026-04-28; range `c68360ef..133de22e`). Combined-reviewer APPROVED end-of-bucket; +5 tests (2860 → 2865). **Implementer pick on row -104:** panel-side first-Cycle subscribe in `emVirtualCosmosPanel::Cycle` (option `(b)` from open question 4) rather than a standalone `VirtualCosmosUpdateEngine`. Observable contract preserved (broadcast wake → `Reload()` → synthesize `ChangeSignal` fire → `update_children`). The success-criterion that referenced `VirtualCosmosUpdateEngine` is satisfied by the panel-side equivalent.
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-008-typed-subscribe-misc.md`
**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** misc (emMainPanel, emVirtualCosmos), 3 rows
**Mechanical-vs-judgement:** mixed — 2 rows mechanical (D-006 verbatim), 1 row (`emVirtualCosmos-104`) requires the same engine-host structural choice that B-007 made for `LoaderEngine`.

## Goal & scope

Wire the three missing P-002 subscriptions across two emmain files, applying **D-006-subscribe-shape** as the canonical port shape:

- `emMainPanel-67` — `View.GetEOISignal()` subscribe (one-of-interest signal, drives slider auto-hide finalisation per C++ `emMainPanel::Cycle`).
- `emMainPanel-69` — `MainWin.GetWindowFlagsSignal()` subscribe (drives `UpdateFullscreen()` in C++ `emMainPanel::Cycle`).
- `emVirtualCosmos-104` — `FileUpdateSignalModel->Sig` subscribe in `emVirtualCosmosModel` constructor (drives `Reload()` on file-update broadcast). This row inherits B-007's structural twist: `emVirtualCosmosModel` in Rust is not an emEngine, so the subscribe must live on a driving engine, not on the model.

The bucket sketch lists three open questions; this design re-validates each against actual Rust source per the B-006/B-007 precedent (verify accessor claims before treating any row as gap-blocked) and resolves all three. None of the three rows is gap-blocked. See §"Audit-data corrections".

## Decisions cited

- **D-006-subscribe-shape** — primary citation. First-Cycle init block + `IsSignaled` checks at top of Cycle. Same shape applied verbatim from B-005's reference design (`docs/superpowers/specs/2026-04-27-B-005-typed-subscribe-emfileman-design.md`). Per-row deviations called out below.
- **D-003-gap-blocked-fill-vs-stub** — listed by the bucket sketch for all three rows. Re-validation finds:
  - Row 67's `EOISignal` accessor is **present** on `emView` (`crates/emcore/src/emView.rs:481`, public field `EOISignal: Option<SignalId>`). Not gap-blocked.
  - Row 69's `GetWindowFlagsSignal` accessor is **present** at `crates/emcore/src/emWindow.rs:1279` — same finding as B-006/emMainControlPanel-218. Not gap-blocked.
  - Row 104's `emVirtualCosmosModel` is **ported** (`crates/emmain/src/emVirtualCosmos.rs:213`); the `FileUpdateSignalModel` it consumes is the same shared `App::file_update_signal` that B-007 re-points the accessor at. The blocker is not "missing model" — it is the same accessor mis-port that B-007 fixes (`emFileModel::AcquireUpdateSignalModel` returning the per-model signal instead of the shared broadcast). Not gap-blocked in the strong sense; cross-bucket prereq on B-007 step 1 instead.

No other D-### decisions apply (D-001, D-002, D-004, D-005 do not touch this scope).

## Audit-data corrections

The bucket sketch (`B-008-typed-subscribe-misc.md`) raises four open questions; this design re-validates each against actual Rust source:

1. **emWindow flag-change accessor (row 69):** sketch claims "emWindow has WindowFlags bitflags but no SignalId accessor for flag changes." **Wrong.** `pub fn GetWindowFlagsSignal(&self) -> SignalId` exists at `crates/emcore/src/emWindow.rs:1279` and is fired from `emWindow::SetWindowFlags` (`emWindow.rs:1618` block changes the flags). Same audit-time stale gap-blocked tag as B-006/emMainControlPanel-218. Working-memory session should flip row 69's tag from `gap-blocked` to `drifted`.
2. **emVirtualCosmosModel ported? (row 104):** sketch's truncated note ("has no en…") was likely "has no engine." Correct in the structural sense — the model is not an emEngine in Rust — but **the model itself is fully ported** (struct + `Acquire` + `Reload`). The classification is "drift due to the same B-007 broadcast-accessor mis-port" not "model not yet ported." Working-memory session should flip row 104's tag from `gap-blocked` to `drifted` and add a B-007 prereq edge.
3. **emMainPanel-67 EOI signal accessor (row 67):** sketch asks to confirm the view-side accessor exists. Confirmed. `emView::EOISignal: Option<SignalId>` is a public field installed at `RegisterEngines` time (`emView.rs:3354,3362`). Pure consumer-side wiring; not gap-blocked.
4. **Merge with larger P-002 buckets?** The three rows split across two distinct files (emMainPanel.rs and emVirtualCosmos.rs); merging with B-005 (emfileman) or B-006 (emMainControlPanel) would mix unrelated panel scopes for no design payoff. Keep separate. The B-007 prereq edge on row 104 makes the staging dependency explicit; merging into B-007 would inflate B-007's already-substantial scope.

## Per-row design

### Row `emMainPanel-67` — `AddWakeUpSignal(GetControlView().GetEOISignal())`

**C++ ref:** `src/emMain/emMainPanel.cpp:67` (subscribe in ctor), reaction in `emMainPanel::Cycle` (C++ `emMainPanel.cpp` Cycle body checks `IsSignaled(GetControlView().GetEOISignal())` to finalise the auto-hide of the control view after the slider hide-timer expires).

**Rust target:** `crates/emmain/src/emMainPanel.rs:658` (existing `Cycle` body).

**Accessor status:** Present. `emView::EOISignal: Option<SignalId>` is a public field on `emView`, populated in `RegisterEngines` (`emView.rs:3354–3362`). The Rust panel accesses the view through `pctx` (the standard panel-side handle in this codebase).

**Wiring:** Standard D-006 first-Cycle init connect + Cycle `IsSignaled` branch.

**Reaction:** Per C++, EOI signal arrival drives the "control view hide finalisation" path — when the user has clicked into the content area (one-of-interest = end-of-interaction in the control area), the slider/control auto-hide engine finalises the hide. In the Rust port, this corresponds to clearing `slider_hide_timer_start` and asserting `slider_hidden` (the existing 5-second timer block is a separate divergence — see §"Out-of-scope adjacency").

**Field added:** `subscribed_init: bool` on `emMainPanel`. The view's EOI `SignalId` is fetched once in the first-Cycle init via `pctx`-mediated view access; cached on the panel as `eoi_signal: Option<SignalId>` so subsequent Cycle calls can `IsSignaled` against it without re-walking `pctx`.

### Row `emMainPanel-69` — `AddWakeUpSignal(GetWindow()->GetWindowFlagsSignal())`

**C++ ref:** `src/emMain/emMainPanel.cpp:69` (conditional subscribe — only if `GetWindow()` is non-null), reaction in `emMainPanel::Cycle` → `UpdateFullscreen()` (which reads `WF_FULLSCREEN` and updates the panel's internal fullscreen state).

**Rust target:** `crates/emmain/src/emMainPanel.rs:658` (existing `Cycle` body).

**Accessor status:** Present at `crates/emcore/src/emWindow.rs:1279`. Row is **not gap-blocked**.

**Wiring:** Standard D-006 connect + branch. Use `crate::emMainWindow::with_main_window(|mw| mw.GetWindowFlagsSignal())` to fetch the SignalId at first-Cycle init time, cache as `flags_signal: Option<SignalId>` on the panel, then `IsSignaled` against the cached value in Cycle. The C++ "conditional on non-null window" guard maps naturally to the `Option` returned by `with_main_window`.

**Reaction:** Mirror C++ `UpdateFullscreen()` — read `mw.GetWindowFlags().contains(WindowFlags::FULLSCREEN)` and update the panel's `FullscreenOn` field (`fullscreen_on` per Rust naming convention; the field already exists by inspection of the Cycle body — implementer confirms or adds).

### Row `emVirtualCosmos-104` — `AddWakeUpSignal(FileUpdateSignalModel->Sig)` in `emVirtualCosmosModel` constructor

**C++ ref:** `src/emMain/emVirtualCosmos.cpp:101` (acquire `FileUpdateSignalModel` from root context), `:104` (subscribe in model ctor). Reaction in `emVirtualCosmosModel::Cycle` → `Reload()`.

**Rust target:** `crates/emmain/src/emVirtualCosmos.rs:213` (`emVirtualCosmosModel` struct), constructor in `Acquire` at `:228`.

**Structural twist — `emVirtualCosmosModel` is not an emEngine in Rust.** Same dependency-forced shape as B-007 row `-103`: the C++ `emModel : emEngine` base-class chain has no Rust analogue, so `AddWakeUpSignal` cannot live on the model. The closest faithful port is to attach a driving engine that hosts the subscribe.

**Accessor status:** The shared broadcast signal exists (`App::file_update_signal`). The accessor `emFileModel::AcquireUpdateSignalModel` is currently mis-pointed at the per-model signal — fixed by B-007 step 1. **Cross-bucket prereq on B-007 step 1.**

**Design choice:** **Add a `VirtualCosmosUpdateEngine` driving engine** alongside the model, registered in `Acquire` after the model is constructed.

**Reasoning:** B-007 row `-103` set the precedent: when an `emModel` consumer needs to subscribe in Rust, the subscribe lives on a driving engine that holds the model via a weak ref (mirrors `LoaderEngine`'s `model_weak`). The engine subscribes to the broadcast at registration time and on wake calls `model.Reload()`. This is the smallest viable shape that preserves the C++ observable contract (broadcast wake → `Reload`) without replicating the engine-base-class shape that the Rust port has already foregone.

**Engine shape:**

```rust
struct VirtualCosmosUpdateEngine {
    model_weak: Weak<RefCell<emVirtualCosmosModel>>,
    update_signal: SignalId, // cached App::file_update_signal
}

impl emEngine for VirtualCosmosUpdateEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        let Some(model) = self.model_weak.upgrade() else {
            ctx.remove_engine(ctx.engine_id);
            return false;
        };
        if ctx.IsSignaled(self.update_signal) {
            model.borrow_mut().Reload();
        }
        false
    }
}
```

**Field added:** none on the model itself. The engine carries the cached signal id and weak ref.

**Reaction:** `Reload()` already exists at `emVirtualCosmos.rs:251` (Rust port of C++ `emVirtualCosmosModel::Reload`). The engine simply calls it.

**Acquire-time wiring:** in `emVirtualCosmosModel::Acquire` (after the existing `Reload()` initialisation):

```rust
let model_rc: Rc<RefCell<Self>> = /* existing */;
let upd = emFileModel::<()>::AcquireUpdateSignalModel(ctx); // post-B-007 accessor
let engine = VirtualCosmosUpdateEngine {
    model_weak: Rc::downgrade(&model_rc),
    update_signal: upd,
};
let eid = ctx.register_engine(Box::new(engine));
ctx.connect(upd, eid);
```

The `Rc<RefCell<emVirtualCosmosModel>>` already exists (the Acquire returns it). The `Weak` justification is the canonical "(a)-justified `Rc<RefCell<T>>` → paired `Weak<RefCell<T>>`" pattern from CLAUDE.md §Ownership.

## Wiring-shape application (D-006)

### `emMainPanel::Cycle` (rows 67 and 69)

```rust
fn Cycle(
    &mut self,
    ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
    ctx: &mut PanelCtx,
) -> bool {
    if !self.subscribed_init {
        let eid = ectx.id();

        // Row 67: View EOI signal. Mirrors C++ emMainPanel.cpp:67.
        if let Some(sig) = ctx.view().EOISignal {
            ectx.connect(sig, eid);
            self.eoi_signal = Some(sig);
        }

        // Row 69: Window flags signal. Mirrors C++ emMainPanel.cpp:69
        // (conditional on GetWindow() non-null).
        if let Some(sig) = crate::emMainWindow::with_main_window(|mw| mw.GetWindowFlagsSignal()) {
            ectx.connect(sig, eid);
            self.flags_signal = Some(sig);
        }

        self.subscribed_init = true;
    }

    // Row 67 reaction (mirrors C++ Cycle EOI branch).
    if let Some(sig) = self.eoi_signal
        && ectx.IsSignaled(sig)
    {
        // Finalise auto-hide: clear timer and assert hidden state.
        // Per C++ EOI semantics in emMainPanel.cpp.
        self.slider_hidden = true;
        self.slider_hide_timer_start = None;
    }

    // Row 69 reaction (mirrors C++ UpdateFullscreen()).
    if let Some(sig) = self.flags_signal
        && ectx.IsSignaled(sig)
    {
        let is_fs = crate::emMainWindow::with_main_window(|mw| {
            mw.GetWindowFlags().contains(WindowFlags::FULLSCREEN)
        }).unwrap_or(false);
        self.update_fullscreen(is_fs);
    }

    // Existing slider-timer block continues (out-of-scope per §below).
    // ...
    false
}
```

The exact `ctx.view()` accessor name on `PanelCtx` may differ; implementer threads view access through whatever the existing emcore convention is (search for `EOISignal` consumer sites in emcore for the pattern). If no existing pattern reaches the view's `EOISignal` field, add a small `pctx.view_eoi_signal()` helper. Below-surface adaptation; no annotation needed.

### `emVirtualCosmosModel::Acquire` + `VirtualCosmosUpdateEngine`

See §"Per-row design" row 104 — engine struct + Acquire-time registration listed inline.

## Verification strategy

C++ → Rust observable contract: the three signals fire and the documented Cycle reaction runs.

**Pre-fix observable behavior:**
- Row 67: After user clicks into the content area to indicate end-of-interest in the control area, the slider/control auto-hide finalisation does not fire on EOI signal. C++ does (the EOI signal-driven path is what makes the hide feel responsive).
- Row 69: User toggles fullscreen via WM (Alt+Enter, etc.); `emMainPanel`'s `fullscreen_on` state and the derived layout do not update until the next viewing-state notice. C++ updates them via the signal.
- Row 104: User triggers a file-update reload (e.g., via the main control panel reload button or after editing a `.emVcItem` file); `emVirtualCosmosModel` does not call `Reload()` and stale items remain. C++ reloads.

**Post-fix observable behavior:** all three reactions fire on signal arrival.

**New test file:** `crates/emmain/tests/typed_subscribe_b008.rs` (RUST_ONLY: dependency-forced — no C++ test analogue, mirrors B-005/B-006/B-007 test rationale).

Test pattern per row:

```rust
// Row 67
let mut h = Harness::new();
let panel = h.create_main_panel();
panel.borrow_mut().slider_hide_timer_start = Some(std::time::Instant::now());
panel.borrow_mut().slider_hidden = false;
h.fire(h.view().EOISignal.unwrap());
h.run_cycle();
assert!(panel.borrow().slider_hidden);
assert!(panel.borrow().slider_hide_timer_start.is_none());

// Row 69
let mut h = Harness::new();
let panel = h.create_main_panel();
h.window_mut().SetWindowFlags(WindowFlags::FULLSCREEN);
h.run_cycle();
assert!(panel.borrow().fullscreen_on);

// Row 104
let mut h = Harness::new();
let model = emVirtualCosmosModel::Acquire(h.ctx());
let initial_gen = model.borrow().reload_generation();
h.fire(h.app().file_update_signal);
h.run_cycle();
assert_ne!(model.borrow().reload_generation(), initial_gen);
```

The `reload_generation()` accessor for row 104 is a test-only counter incremented on each `Reload()` call (RUST_ONLY: dependency-forced for testability — C++ tests have no analogue). Implementer adds via `#[cfg(test)]` or a small public counter.

**Four-question audit-trail evidence per row:** (1) signal connected? — D-006 init block / engine-register connect. (2) Cycle observes? — `IsSignaled` branch. (3) reaction fires documented mutator? — assertions above. (4) C++ branch order preserved? — code review against C++ `emMainPanel::Cycle` and `emVirtualCosmosModel::Cycle` line ranges.

## Implementation sequencing

1. **B-007 step 1 lands first** (cross-bucket prereq). Until `emFileModel::AcquireUpdateSignalModel` returns the shared `App::file_update_signal`, row 104's subscribe would target the wrong signal. B-008 cannot start row 104 before B-007 step 1 is merged.
2. **Row 67 + 69 (`emMainPanel` D-006 init + Cycle branches).** Add `subscribed_init`, `eoi_signal: Option<SignalId>`, `flags_signal: Option<SignalId>` fields. Add the first-Cycle init block, then the two `IsSignaled` branches. If `update_fullscreen()` does not yet exist on `emMainPanel` as a separate method, extract it from the existing fullscreen-state update path. Land as one commit including row-67 and row-69 tests. Independent of step 1.
3. **Row 104 (`VirtualCosmosUpdateEngine` + Acquire-time registration).** After B-007 step 1 lands. Add the engine struct in `emVirtualCosmos.rs`, register and connect in `Acquire`. Land as one commit including row-104 test.
4. **Working-memory reconciliation:** flip rows 67, 69, and 104 from `gap-blocked` to `drifted` in `inventory-enriched.json`; mark all three rows resolved.

Steps 2 and 3 are independent of each other (after step 1 lands).

## Cross-bucket prereq edges

- **B-007 → B-008 (row 104).** B-007 step 1 corrects `emFileModel::AcquireUpdateSignalModel` to return `App::file_update_signal`. B-008 row 104 consumes that accessor. Without B-007 step 1, the row-104 subscribe binds to the wrong signal and the test would fail. **Hard prereq edge.** B-008 rows 67 and 69 carry no cross-bucket prereqs and can land independently.

## Out-of-scope adjacency

- **`SliderTimer` divergence (C++ `emMainPanel.cpp:68`).** The C++ ctor also subscribes to `SliderTimer.GetSignal()`. Rust replaces the timer-engine with a polled `slider_hide_timer_start: Option<Instant>` checked in Cycle (`emMainPanel.rs:663`). This is a separate P-006/P-007-family divergence outside B-008's row set. Do not touch in B-008. If the SliderTimer port lands later as a typed signal, the row 67 EOI branch will compose cleanly (both fire `slider_hidden = true` for orthogonal reasons; the EOI branch is the responsive-to-user-action path, the SliderTimer branch is the 5-second-elapsed path).
- **Seven button click rows (P-004, owned by emmain rc-shim bucket).** Same shape as B-006's note. B-008 leaves them untouched.

## Open questions for the implementer

1. **`PanelCtx` view access for row 67.** If no existing `pctx.view()` or equivalent reaches the view's `EOISignal` field, add a small `pctx.view_eoi_signal() -> Option<SignalId>` helper rather than threading view access through every panel. Below-surface; pick whichever is least intrusive.
2. **`update_fullscreen()` extraction.** The Rust `emMainPanel` likely already updates fullscreen state somewhere (e.g., in `notice` for `VIEWING_CHANGED`). If a clean `update_fullscreen(is_fs: bool)` method does not exist, extract one in step 2; otherwise call the existing path.
3. **`reload_generation()` test hook for row 104.** Whether to add as `pub` counter or `#[cfg(test)]` accessor; test-fixture choice.
4. **Engine-context for `Acquire` (row 104).** `emVirtualCosmosModel::Acquire` currently takes only `&Rc<emContext>`. To register an engine and call `connect`, it needs an `&mut SchedCtx` or similar. If threading that through `Acquire` callers is invasive, the alternative is to register the `VirtualCosmosUpdateEngine` lazily on first model use (analogous to B-007's `LoaderEngine` registration in `emImageFileModel::register`). Implementer picks; either preserves the observable contract.

## Open items deferred to working-memory session

1. **Re-tag rows 67, 69, and 104:** flip from `gap-blocked` to `drifted` in `inventory-enriched.json`. Per §"Audit-data corrections", none of the three rows is gap-blocked in the strong sense.
2. **Bucket sketch refresh:** the bucket sketch's four open questions are answered above; sketch text can be updated.
3. **Cross-bucket prereq edge:** add `B-007 → B-008` (row 104 only) to `work-order.md`'s prereq DAG. This means B-008 cannot reach `merged` before B-007 step 1 lands — but B-008 rows 67 and 69 can be designed and even partially landed independently.
4. **No new D-### proposed.** This bucket reuses D-006 (wiring shape), D-003 (gap-fill option A — already invoked by B-007 for the same accessor), and B-007's structural-port precedent for engine-host. No cross-cutting decision surfaces here.

## Success criteria

- `emMainPanel` subscribes to view EOI signal and window flags signal in a D-006 first-Cycle init block.
- `emMainPanel::Cycle` contains `IsSignaled` branches for EOI (→ slider-hide finalisation) and flags (→ `update_fullscreen`).
- `VirtualCosmosUpdateEngine` exists, is registered at model `Acquire` time, subscribes to the shared `App::file_update_signal`, and calls `model.Reload()` on wake.
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- New `crates/emmain/tests/typed_subscribe_b008.rs` covers all three rows.
- B-008 status in `work-order.md` flips `pending → designed` (working-memory session reconciliation).
