# B-007-typed-subscribe-emcore — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-007-typed-subscribe-emcore.md`
**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** emcore, 3 rows
**Mechanical-vs-judgement:** mixed — 1 row mechanical (D-006 verbatim), 2 rows require an upstream-gap fix on the shared `emFileModel::UpdateSignal` broadcast model before the consumer wire is meaningful.

## Goal & scope

Wire the three missing P-002 subscriptions in emcore for file-update broadcast and per-model change observation, applying **D-006-subscribe-shape** as the canonical port shape. Two of the three rows additionally require fixing a latent semantic divergence on `emFileModel::AcquireUpdateSignalModel` (currently returns the *per-model* `update_signal` instead of the *shared root-context* broadcast `SignalId`); per **D-003** that fix is in-bucket because the broadcast infrastructure is already ported (`App::file_update_signal`) and the fix is to expose it through the existing accessor rather than to port a missing model.

The three rows together cover: post-mtime-broadcast invalidate-listing (`emFileSelectionBox`), per-image-model-change reaction (`emImageFile`/`emImageFilePanel`), and self-triggered reload after the broadcast (`emFileModel::Cycle`).

## Decisions cited

- **D-006-subscribe-shape** — primary citation. First-Cycle init block + `IsSignaled` checks at top of Cycle. Same shape applied verbatim from B-005's reference design (`docs/superpowers/specs/2026-04-27-B-005-typed-subscribe-emfileman-design.md`). Per-row deviations called out below.
- **D-003-gap-blocked-fill-vs-stub** — applies to rows `emFileSelectionBox-64` and `emFileModel-103`. Per the bucket sketch's open question, the reference here is to the **shared** `emFileModel::UpdateSignal` broadcast model. Verification (see §"Audit-data corrections" below) finds the shared signal IS ported (`App::file_update_signal` at `crates/emcore/src/emGUIFramework.rs:227`, fired from `emMainWindow::Cycle` at `crates/emmain/src/emMainWindow.rs:389`). What is missing is the accessor wiring — `emFileModel::AcquireUpdateSignalModel` returns the wrong signal. Per D-003 option A ("fill the gap, scoped per bucket"), the fix is in-bucket: re-point the existing accessor at the shared signal.

No other D-### decisions apply (D-001, D-002, D-004, D-005 do not touch this scope).

## Audit-data corrections

The bucket sketch lists two open questions about whether the upstream broadcast model is ported. Following the B-006 precedent (verify accessor claims against actual Rust code before treating any row as gap-blocked), this design re-validates both:

1. **`FileModelsUpdateSignalModel` (referenced by `emFileSelectionBox-64`).** In C++ this is a per-instance `emRef<emSigModel>` field on `emFileSelectionBox` (`emFileSelectionBox.h:295`) acquired via `emFileModel::AcquireUpdateSignalModel(GetRootContext())`. The "model" itself is just `emSigModel::Acquire(rootContext, "emFileModel::UpdateSignal")` — a shared root-scoped signal-only model. **Rust port status:** the broadcast signal is ported as `App::file_update_signal` (a `SignalId` allocated on the global scheduler and fired from `emMainWindow::Cycle` for the reload action). The signal exists. Only the accessor (`AcquireUpdateSignalModel`) is wrong: it returns a per-model `update_signal` rather than the shared broadcast.
2. **`UpdateSignalModel` (referenced by `emFileModel-103`).** Same underlying object as (1) — both C++ sites acquire the same shared `emSigModel("emFileModel::UpdateSignal")`. Same Rust status: shared signal exists at framework scope; per-model accessor is mis-wired.

**Conclusion:** Neither row is gap-blocked in the strong sense (no missing model). Both are blocked by a latent semantic mis-port of `AcquireUpdateSignalModel`. The fix is in-bucket per D-003 option A. The bucket sketch's "fill in scope or escalate" question resolves to **fill in scope**.

The bucket sketch's gap-blocked tag for these two rows should be downgraded by the working-memory session from "gap-blocked" to "drifted" (the upstream gap is non-blocking once the accessor is re-wired). See §"Open items deferred to working-memory session".

## Per-row design

### Row `emFileSelectionBox-64` — `AddWakeUpSignal(FileModelsUpdateSignalModel->Sig)`

**C++ ref:** `src/emCore/emFileSelectionBox.cpp:39` (acquire), `:64` (subscribe), `:392` (Cycle reaction → `InvalidateListing()`).

**Rust target:** `crates/emcore/src/emFileSelectionBox.rs:1494` (existing `Cycle` body).

**Accessor status after gap-fill:** Present. The new shape exposes `App::file_update_signal` through a `Framework`-scope accessor (see §"Gap-fill" below).

**Wiring:** Standard D-006 first-Cycle init connect + Cycle `IsSignaled` branch.

**Reaction:** `self.invalidate_listing()` — already exists at line 1064.

**Field added:** `subscribed_init: bool`. (No `FileModelsUpdateSignalModel` field needed — the signal is read once in the first-Cycle init via the framework accessor.)

### Row `emImageFile-139` — `AddWakeUpSignal(((emImageFileModel*)GetFileModel())->GetChangeSignal())` in `emImageFilePanel::SetFileModel`

**C++ ref:** `src/emCore/emImageFile.cpp:120-141` (`SetFileModel`: remove old subscribe, swap model, add new subscribe). The companion Cycle reaction in C++ `emImageFilePanel::Cycle` re-reads the image from the (now-loaded) model.

**Rust target:** `crates/emcore/src/emImageFileImageFilePanel.rs` (the `emImageFilePanel` SPLIT out of `emImageFile.h`). The audit row anchor `emImageFile.rs:85` points at `emImageFileModel::GetChangeSignal` (the accessor side), but the *consumer-side* drift lives in the panel file. The accessor exists at `emImageFile.rs:113`.

**Pure consumer-side wire-up.** No upstream-gap fill — `emImageFileModel::GetChangeSignal()` already returns a per-model `SignalId`. This row is mechanical D-006.

**Wiring:** Add `subscribed_init: bool` to `emImageFilePanel`. First-Cycle init block reads the currently-bound image model (if any) and connects to its `GetChangeSignal()`. Cycle body adds an `IsSignaled` branch that invalidates the cached image and triggers a re-read.

**Subtlety — model swap.** C++ `SetFileModel` does the subscribe/unsubscribe pair atomically with the model swap. In the Rust port, `emFilePanel::SetFileModel` (the parent) does not currently expose this hook. Two options:

- **A. Override `SetFileModel` on `emImageFilePanel`.** Mirrors C++ structure exactly. Requires that `PanelBehavior` (or the panel composition layer) admit derived-class override of `SetFileModel`. The current Rust `emFilePanel::SetFileModel` is on the `emFilePanel` struct, and `emImageFilePanel` composes (not inherits). The composing wrapper can expose its own `SetFileModel(model)` that (1) disconnects from old model's signal via the engine context, (2) calls `self.file_panel.SetFileModel(model)`, (3) connects to new model's signal.
- **B. Re-evaluate model binding in Cycle.** First-Cycle init plus a "current model id" cache; on every Cycle, if the bound model changed since last seen, disconnect from old / connect to new. Simpler, no new public surface, but adds per-cycle bookkeeping.

**Choice:** **A**, with the caveat that `SetFileModel` on `emImageFilePanel` requires a `&mut SchedCtx` (or `&mut EngineCtx`) in its signature in order to call `connect`/`disconnect`. C++ achieves this through ambient engine context on the base class; Rust must thread it explicitly. If existing `emImageFilePanel::SetFileModel` callers don't have an engine context handy, fall back to **B** — document the choice on the implementation commit. This is below-surface adaptation; the observable contract (subscribe to the bound model's `GetChangeSignal`) is identical either way.

**Reaction:** Mirror C++ `emImageFilePanel::Cycle`. The Rust panel currently relies on `cycle_inner` (state-poll); add an explicit `IsSignaled(model.GetChangeSignal())` branch that invalidates `current_image` and (via `pctx`) requests a paint re-fetch.

### Row `emFileModel-103` — `AddWakeUpSignal(UpdateSignalModel->Sig)` in `emFileModel::SetIgnoreUpdateSignal`

**C++ ref:** `src/emCore/emFileModel.cpp:101-103` (subscribe inside `SetIgnoreUpdateSignal(false)` branch), `:233-235` (Cycle reaction → `Update()`).

**Rust target:** `crates/emcore/src/emFileModel.rs:483` (existing `Cycle` body). The audit row anchors at line 483 — the Cycle site, not the SetIgnoreUpdateSignal site.

**Structural twist — `emFileModel` is not an emEngine in Rust.** In C++, `emFileModel : emModel : emEngine`, so `AddWakeUpSignal` calls the engine framework directly. In Rust, `emFileModel<T>` is a plain struct; its `Cycle` is dispatched by an external engine (`LoaderEngine` for image files; equivalents elsewhere) that holds the model via `Rc<RefCell<>>`. The subscribe call therefore lives on the *driving engine*, not on the model. This is below-surface adaptation forced by Rust's lack of virtual base-class engine inheritance — the C++ `emModel : emEngine` pattern is **dependency-forced** here in the sense that no Rust port of that base-class shape exists in scope. The DIVERGED block at `emFileModel.rs:40-41` already documents the broader divergence.

**Implication:** the connect call belongs on `LoaderEngine` (and any future per-model engine). For the B-007 scope this is `emImageFile::LoaderEngine` — which is one-shot and exits after firing, so a typical first-Cycle init pattern doesn't apply directly. The closest faithful port is:

- At `emImageFileModel::register` (where `LoaderEngine` is constructed and registered, `emImageFile.rs:87-91`), add a `connect(app.file_update_signal, eid)` call after `register_engine`. The shared broadcast then wakes `LoaderEngine` whenever any reload is requested; on wake, `LoaderEngine` calls `model.update()` (the existing Rust port of `emFileModel::Update` at `emFileModel.rs:272`), which retries failed loads / unloads out-of-date files exactly per C++ `emFileModel::Cycle:233-235`.
- Replace the current "remove self after first run" behavior in `LoaderEngine::Cycle` (line 237: `ctx.remove_engine(engine_id)`) with a **persistent-after-load** mode: stay registered after the initial load completes, gated on the broadcast subscribe so it only wakes on reload events. (The "remove engine" path is preserved for the `model_weak.upgrade()` failure case.)

**Field added on `LoaderEngine`:** `update_signal: SignalId` (the cached broadcast id).

**Reaction (in `LoaderEngine::Cycle`):** if `IsSignaled(self.update_signal)` and the model's `ignore_update_signal == false`, call `model.update()`. If the resulting state requires a re-load (`FileState::Waiting`), proceed through the existing load path. The existing `change_signal` fire on state change is preserved.

This row's design is the most structurally substantial of the three; per Port Ideology it preserves the C++ observable contract (broadcast wake → `Update()` → optional reload) without replicating the engine-base-class shape that the Rust port has already foregone.

## Gap-fill — `emFileModel::AcquireUpdateSignalModel`

**Current state:** returns `self.update_signal` (a per-model `SignalId`). Callers receive a signal that only the model itself ever fires — the C++ shared-broadcast contract is silently broken.

**Fix:** re-point the accessor at the framework-scope shared signal. The accessor signature changes from `&self` to take a context handle that exposes `app.file_update_signal`. Two viable shapes:

- **A1. Static accessor.** Mirror C++ `static emRef<emSigModel> AcquireUpdateSignalModel(emRootContext&)`. Rust signature: `pub fn AcquireUpdateSignalModel<C: ConstructCtx>(ctx: &C) -> SignalId` (or equivalent with `SchedCtx`/`EngineCtx`). Returns `app.file_update_signal`. Drop the per-model `update_signal` field (it serves no callers in production — only `AcquireUpdateSignalModel` reads it, and `emImageFileModel::register` allocates one but nothing fires it).
- **A2. Instance accessor with framework lookup.** Keep the `&self` signature but resolve via a global framework reference. Rejected — Rust port has no ambient root-context lookup for plain structs.

**Choice:** **A1**. The `update_signal` field on `emFileModel<T>` becomes dead and is removed in the same commit; `emFileModel::new` drops the `update_signal: SignalId` parameter; `emImageFileModel::register` drops the `let update_signal = ctx.create_signal();` line.

**Annotation:** the current `pub fn AcquireUpdateSignalModel(&self) -> SignalId` is **not** annotated as DIVERGED — it is a fidelity bug masquerading as a port. The fix removes the bug; no DIVERGED annotation needed at the new accessor (it matches C++ shape).

## Wiring-shape application (D-006)

### `emFileSelectionBox::Cycle` (row `-64`)

```rust
fn Cycle(
    &mut self,
    ectx: &mut crate::emEngineCtx::EngineCtx<'_>,
    ctx: &mut PanelCtx,
) -> bool {
    if !self.subscribed_init {
        let eid = ectx.id();
        // Row -64: shared file-update broadcast. Mirrors C++
        // emFileSelectionBox.cpp:64 (AddWakeUpSignal(FileModelsUpdateSignalModel->Sig)).
        let upd = emFileModel::<()>::AcquireUpdateSignalModel(ectx);
        ectx.connect(upd, eid);
        self.subscribed_init = true;
    }

    // Row -64 reaction (mirrors C++ Cycle 392).
    let upd = emFileModel::<()>::AcquireUpdateSignalModel(ectx);
    if ectx.IsSignaled(upd) {
        self.invalidate_listing();
    }

    // Existing event-drain block continues unchanged.
    // ...
    false
}
```

The `<()>` type-args on the static method is the Rust expression for "I just want the static accessor; I don't care about T." Implementer may prefer a free function `emFileModel::AcquireUpdateSignalModel<C: ConstructCtx>(ctx: &C)` to avoid the turbofish; below-surface choice.

### `emImageFilePanel::Cycle` (row `-139`)

```rust
fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx) -> bool {
    if !self.subscribed_init {
        if let Some(model_rc) = self.file_panel.model.as_ref() {
            // Row -139: per-model change signal. Mirrors C++
            // emImageFile.cpp:139 (AddWakeUpSignal(model.GetChangeSignal())).
            let sig = model_rc.borrow().GetChangeSignal();
            ectx.connect(sig, ectx.id());
            self.subscribed_model_id = Some(/* opaque key */);
        }
        self.subscribed_init = true;
    }

    // Re-bind on model swap (option B fallback).
    let current_model_key = self.file_panel.model.as_ref().map(/* key */);
    if current_model_key != self.subscribed_model_id {
        // disconnect old, connect new — mirrors C++ SetFileModel pair.
        // Code preserved here in Cycle if option A (override SetFileModel)
        // was infeasible; otherwise this block is empty.
    }

    if let Some(model_rc) = self.file_panel.model.as_ref() {
        let sig = model_rc.borrow().GetChangeSignal();
        if ectx.IsSignaled(sig) {
            self.current_image = None; // invalidate cache
            pctx.invalidate_painting(/* self id */);
        }
    }

    self.file_panel.cycle_inner();
    false
}
```

### `LoaderEngine::Cycle` (row `-103`, structural-port variant)

```rust
impl emEngine for LoaderEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        // Row -103: broadcast-wake → Update(). Mirrors C++ emFileModel.cpp:233-235.
        let upd = self.update_signal; // cached at register time
        let woken_by_broadcast = ctx.IsSignaled(upd);

        let Some(model_rc) = self.model_weak.upgrade() else {
            ctx.remove_engine(ctx.engine_id);
            return false;
        };

        if woken_by_broadcast {
            let mut m = model_rc.borrow_mut();
            if !m.file_model().GetIgnoreUpdateSignal() {
                m.file_model_mut().update();
            }
        }

        // Existing initial-load path (unchanged) follows here.
        // After load completes the engine STAYS registered so the broadcast
        // can wake it again. The remove_engine path is reached only when
        // model_weak fails to upgrade.
        // ...
        false
    }
}
```

## Verification strategy

C++ → Rust observable contract: the three signals fire and the documented Cycle reaction runs.

**Pre-fix observable behavior:**
- Row `-64`: User triggers a file-update reload (e.g., via the main control panel reload button); `emFileSelectionBox` does NOT re-list the directory. C++ does.
- Row `-139`: A bound `emImageFileModel` re-loads (e.g., after a broadcast); the `emImageFilePanel`'s cached image stays stale. C++ refreshes.
- Row `-103`: A reload is triggered via the broadcast; `emFileModel::Update()` is never called on listening models; out-of-date or failed-load models stay stuck. C++ recovers.

**Post-fix observable behavior:** all three reactions fire on signal arrival.

**New test file:** `crates/emcore/tests/typed_subscribe_b007.rs` (RUST_ONLY: dependency-forced — no C++ test analogue, mirrors B-005 / B-006 test rationale).

Test pattern per row:

```rust
// Row -64
let mut h = Harness::new();
let panel = h.create_file_selection_box();
let initial_listing_gen = panel.borrow().listing_gen();
h.fire(h.app().file_update_signal);
h.run_cycle();
assert_ne!(panel.borrow().listing_gen(), initial_listing_gen);

// Row -139
let mut h = Harness::new();
let (panel, model) = h.create_image_file_panel_with_model();
let initial_image = panel.borrow().current_image_handle();
model.borrow_mut().fail_load("synthetic".into());
h.fire(model.borrow().GetChangeSignal());
h.run_cycle();
assert_ne!(panel.borrow().current_image_handle(), initial_image);

// Row -103
let mut h = Harness::new();
let model = h.create_image_file_model();
model.borrow_mut().file_model_mut().fail_load("io".into());
h.fire(h.app().file_update_signal);
h.run_cycle();
assert_eq!(model.borrow().file_model().GetFileState(), &FileState::Waiting);
```

**Four-question audit-trail evidence per row:** (1) signal connected? — D-006 init / `LoaderEngine::register` connect. (2) Cycle observes? — `IsSignaled` branch. (3) reaction fires documented mutator? — assertions above. (4) C++ branch order preserved? — code review against the cited C++ Cycle line ranges.

## Implementation sequencing

1. **Gap-fix prereq (rows `-64` and `-103`).** Re-point `AcquireUpdateSignalModel` at `App::file_update_signal`; remove the dead per-model `update_signal` field and constructor parameter; update `emImageFileModel::register` to drop the `let update_signal = ctx.create_signal();` allocation. Land as one commit. Verify with `cargo check` and existing tests (no behavioral change yet — the per-model signal had no consumers).
2. **Row `-103` (LoaderEngine persistent + broadcast subscribe).** Cache `app.file_update_signal` on `LoaderEngine`; convert from one-shot to persistent; add the `IsSignaled(upd) → model.update()` branch. Land as one commit including the `tests/typed_subscribe_b007.rs` row-103 test.
3. **Row `-64` (`emFileSelectionBox` D-006 init + Cycle branch).** Add `subscribed_init`, first-Cycle connect to the broadcast, `IsSignaled → invalidate_listing`. Land as one commit including row-64 test.
4. **Row `-139` (`emImageFilePanel` D-006 init + Cycle branch + model-swap handling).** Per §"Per-row design" choice between option A (override `SetFileModel`) or option B (re-bind in Cycle). Land as one commit including row-139 test.
5. **Working-memory reconciliation:** flip rows `-64` and `-103` from `gap-blocked` to `drifted` in `inventory-enriched.json`; mark all three rows resolved.

Steps 1–4 have a hard ordering: 1 is a prereq for 2 and 3 (both consume the corrected accessor); 2/3/4 are independent of each other after step 1.

## Cross-bucket prereq edges

**None.** No row in B-007 depends on another bucket's deliverable. The gap-fix on `AcquireUpdateSignalModel` is in-bucket (D-003 option A); the `LoaderEngine` structural-port variant for row `-103` is in-bucket because `LoaderEngine` itself lives in `emImageFile.rs`.

If a future bucket ports an additional `emFileModel`-derived type (e.g., `emRecFileModel`), that bucket inherits the same persistent-engine + broadcast-subscribe pattern established here for `LoaderEngine`. That is "future-bucket reuse," not a B-007 prereq edge.

## Open questions for the implementer

1. **`emImageFilePanel::SetFileModel` override (option A vs B in row `-139`).** If the call sites for `emImageFilePanel::SetFileModel` don't have an engine context to thread (currently the parent `emFilePanel::SetFileModel` takes none), fall back to option B (Cycle-time re-bind). Document the choice in the commit message; either shape preserves the observable contract.
2. **`emFileModel::new` parameter removal.** Step 1 removes the `update_signal: SignalId` parameter from `emFileModel::new`. All callers (`emImageFileModel::new`, any test fixtures) need their call sites updated. Mechanical; no decision required.
3. **`LoaderEngine` "stay registered after load" energy cost.** The persistent-after-load mode means `LoaderEngine` stays in the scheduler's engine registry indefinitely (until model drop). The cost is one entry per loaded file model. C++ has the same cost via `emModel : emEngine` lifetime. Acceptable; no decision required.
4. **Row `-139` audit anchor relocation.** The audit row anchor `emImageFile.rs:85` points at the accessor side, but the consumer-side fix lands in `emImageFileImageFilePanel.rs`. Working-memory session may want to relocate the anchor for clarity post-remediation. Bookkeeping, not design.

## Open items deferred to working-memory session

1. **Re-tag rows `-64` and `-103`:** flip from `gap-blocked` to `drifted` in `inventory-enriched.json`. Per §"Audit-data corrections" the upstream broadcast model IS ported (`App::file_update_signal`); the gap was the accessor mis-port, which is fixed in-bucket per D-003.
2. **Bucket sketch refresh:** the bucket sketch's two open questions about gap-blocked status are answered above; the sketch text can be updated to reflect that neither row is gap-blocked.
3. **No new D-### proposed.** This bucket reuses D-006 (wiring shape) and D-003 (gap-fill option A); no cross-cutting decision surfaces here.

## Success criteria

- `AcquireUpdateSignalModel` returns the shared `App::file_update_signal`; per-model `update_signal` field and constructor parameter removed.
- `LoaderEngine` subscribes to the broadcast at registration, calls `model.update()` on wake, and stays registered until model drop.
- `emFileSelectionBox::Cycle` contains a D-006 init block and an `IsSignaled(broadcast) → invalidate_listing` branch.
- `emImageFilePanel` subscribes to the bound model's `GetChangeSignal` (via SetFileModel override or Cycle re-bind) and invalidates the cached image on signal.
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- New `crates/emcore/tests/typed_subscribe_b007.rs` covers all three rows.
- B-007 status in `work-order.md` flips `pending → designed` (working-memory session reconciliation).
