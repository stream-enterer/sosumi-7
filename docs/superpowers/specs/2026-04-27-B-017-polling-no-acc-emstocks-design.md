# B-017-polling-no-acc-emstocks — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-017-polling-no-acc-emstocks.md`
**Pattern:** P-007-polling-accessor-missing
**Scope:** emstocks — 3 rows (`emStocksFetchPricesDialog-62`, `emStocksFilePanel-34`, `emStocksFileModel-41`)
**Mechanical-vs-judgement:** balanced. Two of the three rows are mechanically identical to B-016 (D-006 first-Cycle init + `IsSignaled` branch over an external accessor). The third (`emStocksFileModel-41`) is a self-subscribe to a `SaveTimer` signal whose accessor must be rebuilt around the already-ported `TimerCentral` — a one-shot judgement call (`Instant`-based shim → `emTimer`-based timer signal) that is then mechanical to wire.

## Goal & scope

Replace per-Cycle polling in three emstocks sites with subscribe-driven Cycle reactions. All three rows apply **D-005-poll-replacement-shape** (direct subscribe; reaction collapses into the Cycle body) wired through **D-006-subscribe-shape** (first-Cycle init block calling `ectx.connect(...)` gated on `subscribed_init: bool`, then `IsSignaled` branches at top of Cycle). **D-004-stocks-application-strategy** is cited as a non-decision — Phase 3 already clustered these three pattern-coherent rows; "design once, apply mechanically" falls out per row.

The three rows:

| Row | C++ ref | Rust target | Subscribed signals (per C++ ctor / Cycle) |
|---|---|---|---|
| `emStocksFetchPricesDialog-62` | `emStocksFetchPricesDialog.cpp:62` | `emStocksFetchPricesDialog.rs:91` (`Cycle`) | `Fetcher.GetChangeSignal()` (cpp:62) — single signal |
| `emStocksFilePanel-34` | `emStocksFilePanel.cpp:34` | `emStocksFilePanel.rs:354` (`Cycle`) | `GetVirFileStateSignal()` (cpp:34) — single signal in row scope |
| `emStocksFileModel-41` | `emStocksFileModel.cpp:41` | `emStocksFileModel.rs:62` (`CheckSaveTimer`) | `SaveTimer.GetSignal()` (cpp:21,34) — self-subscribe to owned timer signal |

## Decisions cited

- **D-005-poll-replacement-shape** — primary citation for the reaction model. Direct subscribe; the Rust `Cycle` body for each site is invoked by the engine when the subscribed signal fires, and re-reads the triggering producer state inline. Mirrors C++ `Cycle` shape exactly: subscribe in ctor, `IsSignaled` branches at top of `Cycle`, react.
- **D-006-subscribe-shape** — wiring shape for all three rows. First-Cycle init block calling `ectx.connect(sig, ectx.id())` for each subscribed signal, gated on `subscribed_init: bool`, then `IsSignaled` checks, then the existing reactions. Per D-006 §"why first-Cycle init mirrors C++," the Rust port issues the equivalent `connect` calls on the first `Cycle` invocation because `ConstructCtx` does not expose `connect`. The C++ `AddWakeUpSignal` call sites in this bucket all live in panel/dialog/model ctors — first-Cycle init is the closest port.
- **D-004-stocks-application-strategy** — confirms design-once / apply-mechanically across the three in-bucket rows. No special cross-stocks coordination logic is added here; the per-row designs below stand alone.

D-001, D-002, D-003 do not apply directly: no type-mismatched accessors in scope; no `Rc<RefCell>` shim consumers; the row tagged "missing accessor" in the bucket sketch (`emStocksFileModel-41`) is **not** gap-blocked at execution time once D-006 init wires the timer (see Audit-Data Corrections §3 below).

## Audit-data corrections

Re-validation against the actual Rust + C++ source surfaced three bucket-sketch claims that need correction. None of these moves rows out of B-017.

1. **`emTimer::TimerCentral` IS ported.** The bucket sketch flagged `emStocksFileModel-41` as potentially upstream-blocked because "`emTimer::TimerCentral` is unported." This is stale. `crates/emcore/src/emTimer.rs` ports `TimerCentral` (`pub(crate)`) and `crates/emcore/src/emScheduler.rs:402-418` exposes the operations on `Scheduler`: `create_timer(signal: SignalId) -> TimerId`, `start_timer(id, ms, periodic)`, `restart_timer(id, ms, periodic)`, `cancel_timer(id, abort_signal)`, `is_running(id)`. Active consumers exist at `emcore/src/emMiniIpc.rs:350,406` and `crates/eaglemode/tests/integration/signals.rs:82-83`. **The save-timer signal CAN be allocated and fired through the existing infrastructure.** No `emTimer` (public C++-style wrapper class) exists in Rust — the C++ `emTimer` class is replaced by direct `Scheduler::{create,start,...}_timer` calls returning `TimerId` + a caller-owned `SignalId`. That's the Rust ownership-model adaptation already in tree. B-017 conforms to it; no new wrapper. (Below-surface adaptation, not a divergence — observable behavior: same fire timing, same signal contract.)

2. **`emStocksFileModel-41`'s "accessor missing" tag is half-true.** The C++ `SaveTimer.GetSignal()` accessor is owned *by the timer object*, not by `emStocksFileModel`. The Rust port currently uses `Option<Instant>` instead of an `emTimer`, so there is no `SignalId` field at all. The fix is therefore not "expose a delegating accessor" but "switch the timer mechanism to `Scheduler::create_timer` and store the returned `SignalId`." The signal becomes self-owned by the model (mirrors C++ ownership: the model owns the `SaveTimer`, which owns its signal). No external accessor is added; the signal stays internal. This row's "missing accessor" tag in the audit reflects "no `SignalId` reachable from anywhere," which is true under the `Instant` shim.

3. **`emStocksFetchPricesDialog-62` accessor: `Fetcher.GetChangeSignal()` is owned by B-001's G3.** B-001's design (`docs/superpowers/specs/2026-04-27-B-001-no-wire-emstocks-design.md` §G3, design committed as 456fa5f7) ports `emStocksPricesFetcher::GetChangeSignal()` with this exact shape: add `change_signal: SignalId` field to the fetcher struct, allocate in `new(...)` (which must take `&mut C: ConstructCtx` or equivalent), add `GetChangeSignal()` accessor, fire from every internal state transition that the C++ original signals (consult `emStocksPricesFetcher.cpp:70,134,264,272`). The B-001 reconciliation note explicitly flags G3 as "ported per D-003 but has no in-bucket consumer. If C++ has an `AddWakeUpSignal(...PricesFetcher.GetChangeSignal())` site the audit missed, it's a B-001 amendment candidate." **B-017's `emStocksFetchPricesDialog-62` is exactly that consumer.** This creates a soft cross-bucket prereq edge B-001 G3 → B-017 row 1.

4. **`emStocksFilePanel-34` accessor: `emFilePanel::GetVirFileStateSignal()` is owned by B-004's G1.** Identical structural relationship to B-016 (which subscribes the three sibling fileman panels to the same accessor). B-004 G1 ports the `vir_file_state_signal: SignalId` field on `emFilePanel`, the `GetVirFileStateSignal()` accessor, and the four mutator fires (`SetFileModel`, `set_custom_error`, `clear_custom_error`, `cycle_inner`). **B-017's `emStocksFilePanel-34` consumes the accessor on the embedded `file_panel: emFilePanel` field** (`emStocksFilePanel.rs:402`). This creates a hard cross-bucket prereq edge B-004 G1 → B-017 row 2 (identical shape to the B-016 → B-004 G1 edge).

These corrections do not move any rows out of B-017.

## Cross-bucket prereq edges

- **B-004 G1 → B-017 row 2 (`emStocksFilePanel-34`). Soft, blocking.** B-017 row 2 cannot land until `emFilePanel::GetVirFileStateSignal()` exists. B-004 G1 ports it and is independently shippable; the B-016 design depends on the same accessor. PR staging: B-004 G1 first (lands its own `emImageFile-117` consumer); B-016 + B-017 row 2 follow as independent subscribers.
- **B-001 G3 → B-017 row 1 (`emStocksFetchPricesDialog-62`). Soft, blocking.** B-017 row 1 cannot land until `emStocksPricesFetcher::GetChangeSignal()` exists. B-001 G3 ports it; B-001's reconciliation note already flags this as the missing in-bucket consumer site, so the edge is anticipated.
  - **Working-memory amendment candidate (no action requested in this design):** the B-001 reconciliation note suggests "B-001 amendment candidate" if a missed C++ subscribe site exists. The site does exist (`emStocksFetchPricesDialog.cpp:62`); however, the audit row is in B-017's row scope, not B-001's. The consumer wire stays in B-017; only the *accessor port* prereq is owned by B-001. No re-bucketing.
- **B-017 row 3 (`emStocksFileModel-41`) has no cross-bucket prereq edge.** The `SaveTimer.GetSignal()` accessor is allocated locally in `emStocksFileModel::new` via `Scheduler::create_timer` (already-ported infra). No upstream accessor needed.
- **B-017 has no outbound prereq edges.** No other bucket depends on B-017 completing.

## Accessor groups

Following the B-001 / B-008 / B-016 organising convention. Two accessor groups span this bucket; both are *consumed* (one ported elsewhere, one allocated in-row). One additional in-row signal (the SaveTimer signal) is owned by row 3.

### G-ext1 — `emFilePanel::GetVirFileStateSignal` (consumed; ported by B-004 G1)

**Status.** Ported by B-004 G1 (soft prereq). B-017 consumes the accessor on `emStocksFilePanel.file_panel`; it does not port it.

**Accessor surface (post-B-004 G1):**

```rust
impl emFilePanel {
    pub fn GetVirFileStateSignal(&self) -> SignalId { self.vir_file_state_signal }
}
```

**Subscriber wired by B-017:** row 2 (`emStocksFilePanel-34`).

### G-ext2 — `emStocksPricesFetcher::GetChangeSignal` (consumed; ported by B-001 G3)

**Status.** Ported by B-001 G3 (soft prereq). B-017 consumes the accessor on the dialog's owned `fetcher` field.

**Accessor surface (post-B-001 G3):**

```rust
impl emStocksPricesFetcher {
    pub fn GetChangeSignal(&self) -> SignalId { self.change_signal }
}
```

**Subscriber wired by B-017:** row 1 (`emStocksFetchPricesDialog-62`).

### G-int1 — SaveTimer signal on `emStocksFileModel` (allocated in-row 3)

**Status.** Not an accessor. The signal is internally owned by the model; no `Get*Signal` method is added. Mirrors C++ `SaveTimer.GetSignal()` ownership (the model owns the timer, the timer owns the signal — `IsSignaled(SaveTimer.GetSignal())` is a self-subscribe in `emStocksFileModel::Cycle`).

**Allocation surface (added by B-017 row 3):**

```rust
pub struct emStocksFileModel {
    pub file_model: emRecFileModel<emStocksRec>,
    pub PricesFetchingDialog: emCrossPtr<emStocksFetchPricesDialog>,
    /// SaveTimer signal id, allocated via Scheduler::create_signal in `new(cc)`.
    save_timer_signal: SignalId,
    /// SaveTimer handle in TimerCentral, allocated via Scheduler::create_timer in `new(cc)`.
    save_timer_id: TimerId,
    subscribed_init: bool,
}
```

`new` signature changes to take a `ConstructCtx` (or equivalent providing `&mut Scheduler`) — needed to call `create_signal` + `create_timer`. The existing `Option<Instant>` field (`save_timer_deadline`) is removed; the `OnRecChanged` trigger calls `Scheduler::start_timer(self.save_timer_id, AUTOSAVE_DELAY_MS, false)` instead of computing a deadline.

## Wiring-shape application (D-006)

### `emStocksFetchPricesDialog::Cycle` (row `emStocksFetchPricesDialog-62`)

**C++ ref:** ctor `emStocksFetchPricesDialog.cpp:62` (`AddWakeUpSignal(Fetcher.GetChangeSignal())`); reaction `emStocksFetchPricesDialog.cpp:71-87` (Cycle: `IsSignaled(Fetcher.GetChangeSignal()) → UpdateControls() + if (Fetcher.HasFinished()) { ... ; Finish(0); }; return emDialog::Cycle();`).

**Rust target:** `crates/emstocks/src/emStocksFetchPricesDialog.rs:91` (existing `Cycle` body that unconditionally calls `UpdateControls` + checks `fetcher.HasFinished()`).

**Field changes:**

```rust
pub struct emStocksFetchPricesDialog {
    pub(crate) fetcher: emStocksPricesFetcher,
    pub(crate) label_text: String,
    pub(crate) progress_bar: ProgressBarPanel,
    pub(crate) finished: bool,
    pub(crate) finish_error: String,
    /// Cached SignalId from emStocksPricesFetcher::GetChangeSignal, captured
    /// at first-Cycle init time. None until subscribed_init flips true.
    fetcher_change_sig: Option<SignalId>,
    subscribed_init: bool,
}
```

**Cycle wiring (D-006 first-Cycle init):**

```rust
pub fn Cycle(&mut self, ectx: &mut EngineCtx<'_>) -> bool {
    // First-Cycle init: subscribe to Fetcher.GetChangeSignal.
    if !self.subscribed_init {
        let sig = self.fetcher.GetChangeSignal();
        ectx.connect(sig, ectx.id());
        self.fetcher_change_sig = Some(sig);
        self.subscribed_init = true;
    }

    // IsSignaled-gated reaction (C++ emStocksFetchPricesDialog.cpp:73-86).
    let fetcher_fired = self
        .fetcher_change_sig
        .map_or(false, |s| ectx.IsSignaled(s));

    if fetcher_fired {
        self.UpdateControls();
        if self.fetcher.HasFinished() {
            let error = self.fetcher.GetError();
            if !error.is_empty() {
                self.finish_error = error.to_string();
            }
            self.finished = true;
            return false;
        }
    }

    // Mirror C++ `return emDialog::Cycle();` — no Rust analogue today
    // (the dialog struct does not embed an emDialog base). Preserve the
    // current "active while not finished" return value.
    !self.finished
}
```

**Signature change:** `Cycle` now takes `&mut EngineCtx<'_>`. All callers in the codebase (currently only in tests + `emStocksFilePanel::Cycle:365`) must thread `ectx` through. `emStocksFilePanel::Cycle` already has `ectx` in hand at the call site.

**Reaction:** `UpdateControls` + `HasFinished` cleanup logic preserved verbatim, but only on `fetcher_fired`. The redundant per-frame `UpdateControls` invocation is eliminated. Matches C++ behavior exactly: only updates when the fetcher signals change.

**Existing tests touched:** Five tests in the file invoke `dialog.Cycle()` without an `EngineCtx`. They must be updated to use a test harness providing `ectx` (mirrors B-005/B-016 test patterns), or refactored to call `UpdateControls` directly when the test goal is to inspect post-update label state. Two tests (`dialog_cycle_finishes_immediately_when_no_stocks`, `dialog_cycle_returns_true_when_in_progress`) test the Cycle return value and signal-driven reaction; rewrite them to fire `fetcher.change_signal` via the harness, then call Cycle.

### `emStocksFilePanel::Cycle` (row `emStocksFilePanel-34`)

**C++ ref:** ctor `emStocksFilePanel.cpp:34` (`AddWakeUpSignal(GetVirFileStateSignal())`); reaction `emStocksFilePanel.cpp:60-66` (Cycle: `busy=emFilePanel::Cycle(); if (IsSignaled(GetVirFileStateSignal())) UpdateControls(); return busy;`).

**Rust target:** `crates/emstocks/src/emStocksFilePanel.rs:349-387` (existing `Cycle` body).

**Field changes:**

```rust
pub struct emStocksFilePanel {
    pub bg_color: emColor,
    pub config: emStocksConfig,
    pub fetch_dialog: Option<emStocksFetchPricesDialog>,
    pub list_box: Option<emStocksListBox>,
    pub model: emStocksFileModel,
    pub file_panel: emFilePanel,
    /// Cached SignalId from emFilePanel::GetVirFileStateSignal (post-B-004 G1).
    vir_file_state_sig: Option<SignalId>,
    subscribed_init: bool,
}
```

**Cycle wiring (D-006 first-Cycle init):**

```rust
fn Cycle(
    &mut self,
    ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
    _ctx: &mut PanelCtx,
) -> bool {
    // First-Cycle init: subscribe to file_panel.GetVirFileStateSignal.
    if !self.subscribed_init {
        let sig = self.file_panel.GetVirFileStateSignal();
        ectx.connect(sig, ectx.id());
        self.vir_file_state_sig = Some(sig);
        self.subscribed_init = true;
    }

    // IsSignaled-gated reaction. C++ Cycle calls UpdateControls (no-op in Rust:
    // the existing body's "list_box materialization on Loaded" IS the rust
    // equivalent of UpdateControls). Mirror the gate exactly.
    let vfs_fired = self
        .vir_file_state_sig
        .map_or(false, |s| ectx.IsSignaled(s));

    let mut state_changed = false;
    if vfs_fired {
        let old_state = self.file_panel.GetVirFileState();
        self.file_panel.refresh_vir_file_state();
        let new_state = self.file_panel.GetVirFileState();
        state_changed = old_state != new_state;
        if state_changed && new_state.is_good() && self.list_box.is_none() {
            self.list_box = Some(emStocksListBox::new());
        }
    }

    // Save-timer poll is now signal-driven by row 3 (G-int1); the model
    // self-subscribes to its SaveTimer signal in its own Cycle. The
    // model.CheckSaveTimer() call here is removed (was a per-frame poll
    // that the Instant-shim required). Verify by reading emStocksFileModel
    // post-row-3 to confirm the timer-signal Cycle path supersedes it.
    // If emStocksFileModel exposes no Cycle path post-row-3 (i.e., the model
    // is not a registered engine), keep the call as a manual tick instead;
    // implementer decision pending row 3 final shape.
    // self.model.CheckSaveTimer();   // <-- remove pending row 3 review

    // Poll fetch dialog — preserved as-is for now. Row 1 makes the dialog
    // signal-driven internally, but the parent panel still owns the dialog
    // lifecycle and must re-poll Cycle when the engine wakes the parent.
    // The C++ original similarly polls the dialog from outside; this is not
    // a P-007 site at the panel level.
    if let Some(ref mut dialog) = self.fetch_dialog {
        if !dialog.Cycle(ectx) {
            self.fetch_dialog = None;
        }
    }

    // ListBox confirmation-dialog state machine.
    // C-1 RESOLUTION (Adversarial Review 2026-05-01): the previous shape
    //     lb.Cycle(ectx, model.GetWritableRec(), config)
    // cannot survive row 3's ectx-threaded `GetWritableRec` (two `&mut ectx`
    // borrows would coexist). Sequence the borrows: take a non-ectx mutable
    // borrow of the rec first (rec write does NOT call into the scheduler;
    // it only sets a `dirty: bool` flag — see row 3 §Mutator changes), drive
    // lb.Cycle, then call `model.touch_save_timer(ectx)` AFTER lb.Cycle
    // returns to advance the SaveTimer. This keeps `&mut ectx` exclusive
    // across the start_timer call and decouples the timer side-effect from
    // the rec-write side-effect.
    let list_box_busy = {
        let Self {
            list_box,
            model,
            config,
            ..
        } = self;
        if let Some(lb) = list_box.as_mut() {
            // GetWritableRec is now split: rec-mutation only, no scheduler.
            // It sets `model.dirty = true` and returns &mut emStocksRec.
            let rec = model.GetWritableRec();
            lb.Cycle(ectx, rec, config)
        } else {
            false
        }
    };

    // Drive the SaveTimer if any rec write happened during lb.Cycle.
    // This is the ectx-using half of the previous unified GetWritableRec.
    if self.model.dirty_since_last_touch() {
        self.model.touch_save_timer(ectx);
    }

    state_changed || self.fetch_dialog.is_some() || list_box_busy
}
```

**Reaction:** `refresh_vir_file_state` + `list_box` lazy-init gated on `vfs_fired`, mirroring C++'s `IsSignaled(GetVirFileStateSignal()) → UpdateControls()`. The pre-fix unconditional `old_state vs new_state` compare-and-react is replaced by the signal gate. Observable timing matches C++ — Cycle is invoked when `vir_file_state_signal` fires (per B-004 G1 mutator audit).

**Out-of-scope adjacency:** the `model.CheckSaveTimer()` call site at line 361 today exists because the `Instant`-based shim has no signal to drive `Cycle`. Row 3's signal-driven model Cycle replaces this. The implementer must decide between two shapes:
- **(a)** Remove the call from `emStocksFilePanel::Cycle`; have the model register itself as an engine and self-Cycle on its own SaveTimer signal. Mirrors C++ exactly (`emStocksFileModel : emEngine` with its own `Cycle()` triggered by `IsSignaled(SaveTimer.GetSignal())`).
- **(b)** Keep the call as a manual tick from the parent if the model is not a registered engine.

**Recommendation: (a).** The C++ contract is "the model is its own engine; the file panel has no responsibility for ticking the save timer." Mirroring this is the load-bearing structure. The implementer must verify the registration path; if the registration ergonomics are awkward (no precedent in emstocks for a model engine), fall back to (b) and document the divergence (RUST_ONLY: structural — the engine-registration path for emstocks models is not yet exercised). Either way, **the per-frame `CheckSaveTimer` poll is eliminated** — it becomes signal-gated, satisfying P-007 at the model level.

### `emStocksFileModel::Cycle` (row `emStocksFileModel-41`)

**C++ ref:** ctor `emStocksFileModel.cpp:21` (`AddWakeUpSignal(SaveTimer.GetSignal())`); reaction `emStocksFileModel.cpp:33-38` (Cycle: `if (IsSignaled(SaveTimer.GetSignal())) Save(true); return emRecFileModel::Cycle();`).

**Rust target:** `crates/emstocks/src/emStocksFileModel.rs:62` (`CheckSaveTimer` — currently called per-frame from `emStocksFilePanel::Cycle:361`).

**Field changes (replacing the `Option<Instant>` shim):**

```rust
use emcore::emTimer::TimerId;
use emcore::emSignal::SignalId;

pub struct emStocksFileModel {
    pub file_model: emRecFileModel<emStocksRec>,
    pub PricesFetchingDialog: emCrossPtr<emStocksFetchPricesDialog>,
    /// Signal fired by the SaveTimer when the autosave delay elapses.
    /// Mirrors C++ SaveTimer.GetSignal() — owned internally; no accessor.
    save_timer_signal: SignalId,
    /// TimerCentral handle for the save timer.
    save_timer_id: TimerId,
    /// D-006 first-Cycle init flag.
    subscribed_init: bool,
}
```

**Constructor change:** `new(path)` becomes `new<C: ConstructCtx>(cc: &mut C, path: PathBuf)` (or equivalent providing `&mut Scheduler`). Allocates the signal + timer:

```rust
pub fn new<C: ConstructCtx>(cc: &mut C, path: PathBuf) -> Self {
    let save_timer_signal = cc.scheduler_mut().create_signal();
    let save_timer_id = cc.scheduler_mut().create_timer(save_timer_signal);
    Self {
        file_model: emRecFileModel::new(path),
        PricesFetchingDialog: emCrossPtr::new(),
        save_timer_signal,
        save_timer_id,
        subscribed_init: false,
    }
}
```

**Mutator changes (split-borrow shape, per Adversarial Review C-1 resolution):**

The C++ unified site `SaveTimer.Start(15000)` is split in Rust into two half-mutators to avoid the C-1 borrow conflict at `emStocksFilePanel.rs:380`:

1. **Rec-mutation half** — `GetWritableRec(&mut self) -> &mut emStocksRec`. Sets `self.dirty = true` and returns the rec. Does **not** touch the scheduler. Borrow shape: takes `&mut self` only — composes with `lb.Cycle(ectx, rec, config)` because `ectx` is borrowed disjointly from `model`.
2. **Timer-arming half** — `touch_save_timer(&mut self, ectx: &mut EngineCtx<'_>)`. Calls `ectx.scheduler_mut().start_timer(self.save_timer_id, 15000, false)`. Called **after** `lb.Cycle` returns, gated on `self.dirty_since_last_touch()` (a getter that returns `true` iff `dirty` is set and clears the per-touch latch).

`OnRecChanged` (mirrors C++ `OnRecChanged: SaveTimer.Start(15000)`) calls both halves in sequence — it has `&mut ectx` available at its existing call site (the `emRecFileModel` change handler), so the unified shape is preserved there.

The split is a forced divergence (language-forced — Rust's aliasing rules forbid the C++ unified `SaveTimer.Start` inside `lb.Cycle(ectx, GetWritableRec(), config)`). Mark with `DIVERGED:` at both halves citing C-1.

**Cycle wiring (D-006 first-Cycle init + IsSignaled):**

`emStocksFileModel` must implement `emEngine::Cycle` (currently does not — `CheckSaveTimer` is called externally). Add the impl:

```rust
impl emEngine for emStocksFileModel {
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>) -> bool {
        if !self.subscribed_init {
            ectx.connect(self.save_timer_signal, ectx.id());
            self.subscribed_init = true;
        }

        if ectx.IsSignaled(self.save_timer_signal) {
            // C++ emStocksFileModel.cpp:35 → Save(true).
            self.file_model.Save();
        }

        // C++ returns `emRecFileModel::Cycle()`; the Rust composed `file_model`
        // is not itself an engine today — propagate `false` (no busy state)
        // unless emRecFileModel post-port grows a Cycle.
        false
    }
}
```

`CheckSaveTimer` (the public method called from `emStocksFilePanel::Cycle:361`) is **removed**. Its behavior is subsumed by the signal-driven Cycle. The previous return-value contract (`true` if save occurred) is dropped — no caller used it.

**Engine registration:** `emStocksFileModel` must be registered with the scheduler so its Cycle runs when the SaveTimer signal fires. This is a small piece of new infrastructure for emstocks (no precedent today). The implementer adds an `ensure_engine_registered(&Rc<RefCell<Self>>, &mut Scheduler)` helper analogous to `emDirModel::ensure_engine_registered` (referenced in B-016 design's `emDirPanel::Cycle` lazy block). The owning `emStocksFilePanel::Cycle` calls it inside the existing lazy-init path (paired with B-001's `Acquire`/registration pattern, see B-001 §G1 sequencing).

If the engine-registration ergonomics block this (no `emEngine` infrastructure for embedded `emStocksFileModel`), fall back to: keep `CheckSaveTimer` as a manual tick called from `emStocksFilePanel::Cycle`, but switch its body to consult `Scheduler::is_running(self.save_timer_id)` + a `Scheduler::IsSignaled` check on `save_timer_signal`. This still satisfies P-007 (the per-frame `Instant::now() >= deadline` poll is replaced by a signal check), at the cost of mirroring C++ structure less faithfully. **Default path is engine registration.**

**Drop guard:** the existing `Drop` impl saves on shutdown if a deadline was set. Replace the check `if self.save_timer_deadline.is_some()` with `if scheduler.is_running(self.save_timer_id)`. The `Drop` impl needs scheduler access — either store a back-reference (`Weak<RefCell<Scheduler>>` is not in the codebase pattern; CLAUDE.md prefers IDs through `EngineCtx`) or accept the slight semantic widening (always save on drop if there is unflushed data, gated on a `dirty: bool` flag mirrored from `OnRecChanged`). Implementer decision; flagged below.

## Implementation sequencing

1. **Land prereqs first** — both soft-blocking:
   - **B-004 G1** (ports `emFilePanel::vir_file_state_signal` + accessor + four mutator fires). Independently shippable; B-016 also depends on it.
   - **B-001 G3** (ports `emStocksPricesFetcher::change_signal` + accessor + fires from the four C++ `Signal(ChangeSignal)` sites at `emStocksPricesFetcher.cpp:70,134,264,272`).
2. **B-017 row 3 — `emStocksFileModel-41` first.** Lands the largest structural change (constructor signature, engine registration, mutator threading). Independent of rows 1 and 2.
3. **B-017 row 1 — `emStocksFetchPricesDialog-62`.** Adds `fetcher_change_sig` + `subscribed_init` fields; rewrites Cycle per D-006. Updates 3-5 tests for the new `Cycle(&mut ectx)` signature.
4. **B-017 row 2 — `emStocksFilePanel-34`.** Adds `vir_file_state_sig` + `subscribed_init` fields; rewrites Cycle per D-006; removes (or keeps, per row 3 sequencing decision) the `model.CheckSaveTimer()` call.

Rows 1, 2, 3 are independent of each other once the prereqs land and can ship in any order or as one combined PR.

**Per-row PR-staging recommendation (D-004 deferred item):** **all three rows in one PR.** Three small mechanical changes; combined diff is reviewable; no inter-row pilot needed (the pattern is already validated by B-016 for the same vir-file-state shape). If review pressure pushes for staging, row 3 is the natural pilot — it's the largest structural change and validates the Scheduler-timer-on-model pattern (a first for emstocks).

## Verification strategy

C++ → Rust observable contract: each polled re-read becomes signal-driven Cycle invocation; observable behavior identical (state mutation matches; what changes is invocation timing — no longer per-frame).

**Pre-fix observable behavior:**
- `emStocksFetchPricesDialog::Cycle` calls `UpdateControls` every frame even when the fetcher state hasn't changed; the `HasFinished` re-check is also per-frame. Live behavior accidentally works due to dialog over-scheduling.
- `emStocksFilePanel::Cycle` re-reads `vir_file_state` every frame via `old_state vs new_state` compare; identical drift shape to F010 root cause (`emFileLinkPanel` per B-016).
- `emStocksFileModel::CheckSaveTimer` polled every frame from the parent; `Instant::now()` comparison is the trigger, not a signal — so any future scheduler optimization that stops over-scheduling the parent panel could indefinitely defer the autosave.

**Post-fix observable behavior:** all three sites fire their existing reactions on the appropriate signal arrival, independent of any other wake source. Matches C++ `Cycle` invocation cadence exactly.

**New test file:** additions to existing emstocks tests, or new `crates/emstocks/tests/polling_b017.rs`. RUST_ONLY: dependency-forced — no C++ test analogue, mirrors B-005/B-008/B-015/B-016 test rationale.

Test pattern per row:

```rust
// Row 1: emStocksFetchPricesDialog — fire change_signal, assert UpdateControls ran.
let mut h = Harness::new();
let mut dialog = emStocksFetchPricesDialog::new(&mut h.cc(), "", "", "");
dialog.AddStockIds(&["AAPL".into()]);
h.run_cycle(|ectx| dialog.Cycle(ectx)); // first-Cycle init wires the subscribe
// Pre-fire: label_text reflects pre-update state.
h.fire(dialog.fetcher.GetChangeSignal());
h.run_cycle(|ectx| dialog.Cycle(ectx));
assert_eq!(dialog.label_text, "AAPL"); // UpdateControls ran on the fire

// Row 2: emStocksFilePanel — fire VirFileStateSignal, assert refresh ran.
let mut h = Harness::new();
let mut panel = emStocksFilePanel::new();
h.run_cycle(|ectx| panel.Cycle(ectx, &mut PanelCtx::default())); // init
panel.set_vfs_good_for_test(); // helper that mutates vir_file_state + fires the signal
h.run_cycle(|ectx| panel.Cycle(ectx, &mut PanelCtx::default()));
assert!(panel.list_box.is_some()); // lazy materialization gated on vfs_fired

// Row 3: emStocksFileModel — fire SaveTimer signal, assert Save ran.
let mut h = Harness::new();
let mut model = emStocksFileModel::new(&mut h.cc(), PathBuf::from("/tmp/test.emStocks"));
h.scheduler.start_timer(model.save_timer_id, 0, false); // fires immediately
h.run_cycle(|ectx| model.Cycle(ectx));
// Assert Save was attempted (test the file_model state transition).
assert!(/* model.file_model state reflects Save attempt */);
```

**Four-question audit-trail evidence per row:** (1) signal connected? — D-006 init block calls `ectx.connect(sig, ectx.id())`. (2) Cycle observes? — `IsSignaled` branch on cached SignalId. (3) reaction fires documented mutator? — assertions above (`UpdateControls`/`label_text`, `refresh_vir_file_state`/`list_box.is_some`, `Save`). (4) C++ branch order preserved? — code review against `emStocksFetchPricesDialog.cpp:71-87`, `emStocksFilePanel.cpp:60-66`, `emStocksFileModel.cpp:33-38`.

## Out-of-scope adjacency

- **`emStocksFetchPricesDialog`'s parent dialog framework (`emDialog::Cycle`).** The current Rust dialog struct does not embed an `emDialog` base — `Cycle` returns a "still active" boolean directly. C++ falls through to `emDialog::Cycle()` for base-class wake handling (modal-state machinery). When the Rust `emDialog` base lands, `Cycle`'s return becomes `... && emDialog::Cycle(...)`. Out of B-017 scope; document inline.
- **`emStocksFilePanel`'s `last_config_gen` shim.** No analogue here — `emStocksFilePanel` does not subscribe to `Config->GetChangeSignal` in C++. The config-change subscribes for emstocks live elsewhere (G2 in B-001).
- **The `emRecFileModel<T>` base's own change/file-state signals.** Out of B-017 row scope; B-001 G1 owns the `emStocksFileModel::GetChangeSignal` delegating accessor (which forwards to `emFileModel::GetChangeSignal` on the embedded base). B-017 does not consume that accessor.
- **`emStocksFetchPricesDialog`'s `fetcher.GetError()` / `GetCurrentStockId()` re-reads inside `UpdateControls`.** These remain unconditional reads inside the gated body — the gate ensures `UpdateControls` only runs when the fetcher signaled change, which is when those getters can have changed. Mirrors C++ exactly.

## Resolutions from Adversarial Review (2026-05-01)

The original "open questions" §1-§4 are resolved here by the adversarial review; §5 remains open.

1. **I-2 resolved — `emStocksFileModel::new` ctx-threading callsite enumeration.** Threading `&mut C: ConstructCtx` through `emStocksFileModel::new` ripples to the following exact callsites; the implementer must update each as part of row 3:
   - `crates/emstocks/src/emStocksFilePanel.rs:401` — `emStocksFilePanel::new()` constructs the embedded model. Either (a) propagate `cc` to `emStocksFilePanel::new<C: ConstructCtx>(cc: &mut C)` or (b) defer model construction to a lazy first-Cycle init. **Default: (a) propagate** — matches B-001 G3 / B-004 G1 precedent.
   - `crates/emstocks/src/emStocksFpPlugin.rs:30` — `emStocksFpPluginFunc(_ctx: &mut dyn ConstructCtx, ...)`. The plugin already has `_ctx`; rename to `ctx` and forward to `emStocksFilePanel::new(ctx, ...)`.
   - Six tests in `emStocksFilePanel.rs:448-502` that call `emStocksFilePanel::new()` without a ctx. Update each to construct via the test harness's `cc()` (mirrors B-005/B-016 test harness pattern).
2. **I-3 resolved — `emStocksFileModel` ownership shape: by-value + proxy-engine.** `Rc<RefCell<emStocksFileModel>>` is rejected because it does not fit CLAUDE.md §Ownership justifications (a)/(b). Instead: the **owning `emStocksFilePanel` registers itself (or a thin proxy engine it owns) as the scheduler engine** for the SaveTimer signal, and forwards Cycle into `self.model` via `&mut`. The model stays held by value at `emStocksFilePanel::model`. This dissolves I-2's wrapping concern and avoids amending CLAUDE.md. The `emDirModel::ensure_engine_registered` precedent at `crates/emfileman/src/emDirModel.rs:293` is **not** mirrored; document the divergence with a `RUST_ONLY:` (language-forced — `Rc<RefCell<>>` charter does not admit scheduler engine wrappers) comment at the registration site.
3. **I-4 resolved — `dirty: bool` clear-points.** Add `dirty: bool` to `emStocksFileModel`. **Set sites:** every `OnRecChanged` invocation, every `GetWritableRec` invocation. **Clear sites (must be exhaustive):**
   - Inside `Save` (after the filesystem write succeeds).
   - Inside `SaveIfNeeded` (the no-op path leaves `dirty` unchanged; the save path clears).
   - At the end of the `Cycle` `Save(true)` branch (post-`Save` call).
   - Inside `Drop`'s on-shutdown save (final clear, defensive).
   The `dirty_since_last_touch()` getter used by C-1's split-borrow shape returns `dirty && !timer_already_armed_this_dirty_window` — implement as a paired latch (`dirty_unobserved: bool` set by mutators, cleared by `touch_save_timer`). Without exhaustive clears, Drop will redundantly write the file (Adversarial Review I-4).
4. **I-1 resolved (cross-bucket) — `emStocksPricesFetcher` upstream subscribes.** B-001 G3's mutator-fire enumeration (cpp:70/134/264/272) covers the *internal* state-transition fires of `Signal(ChangeSignal)`. C++ `emStocksPricesFetcher.cpp:38-39` ALSO calls `AddWakeUpSignal(FileModel->GetChangeSignal())` and `AddWakeUpSignal(FileModel->GetFileStateSignal())` in the fetcher ctor — the fetcher is itself an engine subscribed to FileModel signals. **B-001 G3 may not cover these upstream subscribes** (B-001's design treats G3 as the accessor port; the *consumer wiring* of the fetcher's own Cycle to FileModel signals is implicit-at-best). **Coordination:** see `docs/superpowers/plans/2026-05-01-B-001-no-wire-emstocks.md` — B-001 G3's scope MAY need to widen to include (a) porting the fetcher's own `Cycle` body, (b) first-Cycle init wiring for `FileModel::GetChangeSignal` + `GetFileStateSignal`, and (c) the fires at cpp:70/134/264/272 driven by those upstream events. Until B-001 G3 widens, B-017 row 1 is silently undertested: the dialog will subscribe to `Fetcher.GetChangeSignal()` correctly, but the fetcher will never *fire* that signal in response to FileModel state changes, so the dialog's `UpdateControls` will not run for those transitions. **Action:** flag this as a B-001 plan amendment candidate before B-001 G3 lands. B-017 row 1 cannot reach `merged` until B-001 G3 either widens or explicitly documents the gap.

### Open question (still open)

5. **Test harness signature.** B-005's harness exposes `Harness::fire(SignalId)` and `Harness::run_cycle(closure)`. Confirm the harness is reachable from emstocks tests; if not, add a minimal local harness mirroring `crates/emfileman/tests/` patterns. Row 2's `set_vfs_good_for_test()` (Adversarial Review M-1) is a new public test-only setter on `emStocksFilePanel` — name it `pub(crate) fn set_vfs_good_for_test(&mut self, ectx: &mut EngineCtx<'_>)` and have it mutate `vir_file_state` + fire the cached `vir_file_state_sig`.

## Open items deferred to working-memory session

1. **No new D-### proposed.** This bucket reuses D-005 (reaction model), D-006 (wiring shape), D-004 (cluster-application strategy non-decision). The two accessor ports it consumes are owned by B-004 G1 and B-001 G3 (already designed). The one in-row signal allocation (`save_timer_signal`) uses already-ported `Scheduler::create_timer` infrastructure — no new global decision is surfaced.
2. **D-005 open question §1 (subscribe-arity per consumer).** Confirmed: each B-017 consumer subscribes to a single signal in its row scope (Fetcher.ChangeSignal / VirFileStateSignal / SaveTimer.Signal respectively). C++ ctors do not subscribe to additional signals at the cited rows.
3. **Cross-bucket prereq edge B-001 G3 → B-017 row 1.** Add to `work-order.md` DAG. B-017 row 1 cannot reach `merged` before B-001 G3's accessor is in tree. (B-001 already designed as 456fa5f7.)
4. **Cross-bucket prereq edge B-004 G1 → B-017 row 2.** Add to `work-order.md` DAG. Identical structural shape to the existing B-004 G1 → B-016 edge.
5. **Bucket-sketch claim "emTimer::TimerCentral unported" is stale.** TimerCentral is ported (`crates/emcore/src/emTimer.rs`) and exposed via Scheduler; the bucket sketch should be amended to remove that flag. Working-memory session may strike that line from the B-017 sketch.
6. **B-001 reconciliation note hit:** the "G3 has no in-bucket consumer; B-001 amendment candidate" flag is satisfied by B-017 row 1. No re-bucketing is needed (the consumer site is a P-007 row by audit pattern; B-001 is a P-001 bucket). Working-memory session can mark the B-001 reconciliation flag resolved.
7. **No row reclassifications.** All three rows verified `polling, accessor missing` at audit time. Post-prereqs, rows 1 + 2 become P-006-shaped (polling, accessor present); row 3 becomes a self-allocated-accessor variant. Bucket retains its P-007 tag because audit-time accessor status defines the bucket.

## Success criteria

- `emStocksFetchPricesDialog`, `emStocksFilePanel`, and `emStocksFileModel` each subscribe to their respective signal in a D-006 first-Cycle init block.
- Each site's `Cycle` body runs its existing reaction (`UpdateControls`/`refresh_vir_file_state`/`Save`) only when the subscribed signal fires.
- `emStocksFileModel::CheckSaveTimer` (the per-frame poll method) is removed; the per-frame `model.CheckSaveTimer()` call in `emStocksFilePanel::Cycle:361` is gone (or replaced per row 3 fallback shape).
- `emStocksFileModel` is registered as an engine and self-Cycles on its `save_timer_signal` (or fallback path documented).
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- New tests cover: each of the three sites' subscribe + signal-driven reaction.
- B-017 status in `work-order.md` flips `pending → designed` on commit; per-row commits flip to `merged` as they land (after both B-001 G3 and B-004 G1 land).
- **(Adversarial Review M-2)** Row 1 eliminates the pre-fix unconditional per-frame `UpdateControls` invocation; this is an intended observable change that *fixes* a real signal-drift relative to C++ (which gates `UpdateControls` inside `IsSignaled(Fetcher.GetChangeSignal())`). Not a regression.
- **(Adversarial Review M-3)** Row 1's `Cycle` return semantics: `false` means "dialog finished, parent should drop." Verified at `emStocksFilePanel.rs:365` — caller treats `false` as "drop the dialog" (`self.fetch_dialog = None`). The post-fix shape preserves this contract; the only difference is that the `false` transition now occurs on a fetcher-signal-driven Cycle rather than every frame.
- **(Adversarial Review I-4)** `dirty: bool` is cleared in all four sites enumerated in §"Resolutions" item 3. Implementer must verify exhaustiveness before merge.

## Adversarial Review — 2026-05-01

### Summary
- Critical: 1 | Important: 4 | Minor: 3 | Notes: 2

### Findings

**C-1 (Critical): Borrow conflict between `lb.Cycle(ectx, model.GetWritableRec(), config)` and the new save-timer mutator path.** At `emStocksFilePanel.rs:380`, ListBox.Cycle is invoked with `model.GetWritableRec()` as a reborrow argument; `GetWritableRec` today sets `save_timer_deadline = Some(...)` (rs:45-47). After row 3, `GetWritableRec` must call `ectx.scheduler_mut().start_timer(...)`, requiring `&mut ectx` — but `ectx` is *also* the first argument of `lb.Cycle`. Two `&mut` borrows of ectx cannot coexist. Design’s "thread `ectx` into mutators" prescription (§Mutator changes) does not address this callsite. The fix is non-trivial: either (a) hoist the timer-start to the panel’s Cycle after `lb.Cycle` returns (using a `dirty` flag mirrored from the rec write), or (b) change `lb.Cycle`’s signature to take `&mut emStocksRec` separately and have the panel call `model.touch_save_timer(ectx)` after lb.Cycle returns. Pick before dispatch.

**I-1 (Important): `emStocksPricesFetcher` C++ is itself an engine subscribed to FileModel signals (`emStocksPricesFetcher.cpp:38-39: AddWakeUpSignal(FileModel->GetChangeSignal()/GetFileStateSignal())`); the Rust port has no FileModel reference and no Cycle.** Row 1’s plan only consumes `Fetcher.GetChangeSignal()` from the dialog side. But the C++ fetcher *fires* its own ChangeSignal as a downstream reaction to those upstream subscribes. If B-001 G3 enumerates only the four "Signal(ChangeSignal)" sites at cpp:70/134/264/272 without porting the upstream subscribe, the fetcher’s firing schedule diverges and the dialog will never see UpdateControls run for state transitions that C++ drives from FileModel changes. Confirm B-001 G3 covers the upstream subscribes too (cpp:38-39), or document the gap and the test that would catch it.

**I-2 (Important): `emStocksFilePanel::new()` callsite cannot satisfy `emStocksFileModel::new<C: ConstructCtx>` without touching `emStocksFpPlugin`.** `emStocksFilePanel::new()` at `emStocksFilePanel.rs:395` constructs the embedded model via `emStocksFileModel::new(PathBuf::from(""))` (rs:401). After row 3, `new` requires `&mut C: ConstructCtx`. `emStocksFpPlugin::emStocksFpPluginFunc` (rs:17-30) has `_ctx: &mut dyn ConstructCtx` available but currently passes nothing — both `emStocksFilePanel::new` and the plugin call have to thread ctx. Six tests at rs:448-502 also call `emStocksFilePanel::new()` with no ctx. Design’s open-question §1 acknowledges this in the abstract; explicit callsite enumeration is missing.

**I-3 (Important): `Rc<RefCell<emStocksFileModel>>` wrapping (open-question §2) does not cleanly fit the CLAUDE.md §Ownership justifications.** CLAUDE.md admits `Rc<RefCell<T>>` only for "(a) cross-closure reference held by winit/wgpu callbacks, or (b) context-registry typed singleton." Engine-registration via the scheduler is neither. emDirModel’s precedent (`emDirModel::ensure_engine_registered` at `emfileman/src/emDirModel.rs:293`) does take `&Rc<RefCell<emDirModel>>` but predates the current rule. Either (a) document a third charter category at the design level (and update CLAUDE.md), or (b) keep `emStocksFileModel` by-value and use a different engine-registration pattern (e.g., a thin proxy engine the panel owns that calls into the model via `&mut`). Picking by-value also dissolves I-2.

**I-4 (Important): `Drop`-time save semantics shift.** C++ destructor checks `SaveTimer.IsRunning()` (cpp:46-49). The proposed Rust `dirty: bool` flag is set by every mutator; that fires on Drop even if Save was already executed by a prior signal-driven Cycle (since `dirty` would need to be cleared on Save). Design names this option (a) but doesn’t spec when `dirty` clears. Clear it inside `Save`/`SaveIfNeeded`, and in the Cycle’s `Save(true)` branch — verify all three sites in the implementation. Without this, Drop may double-save (observable: redundant filesystem write).

### Minor

**M-1:** Test pattern for row 2 (`panel.set_vfs_good_for_test()`) doesn’t exist; B-016’s analogue uses a Harness helper. Adding a public test-only setter is itself a new API surface — fine, but the design should name it.

**M-2:** Design says "the redundant per-frame `UpdateControls` invocation is eliminated" (row 1) — confirm this is intended observable change vs. C++. C++ only calls UpdateControls inside `IsSignaled(...)` branch. Pre-fix Rust calls it unconditionally. Post-fix matches C++; that is *fixing* a real signal-drift, not a regression. Note explicitly in the success-criteria block.

**M-3:** `emStocksFetchPricesDialog.rs:91` `Cycle` returns `bool` — the design changes the meaning from "active-while-not-finished" to "active-while-not-finished, after possibly transitioning to finished on this signal." Existing callers in `emStocksFilePanel::Cycle:365` interpret `false` as "drop the dialog." Verify no caller treats a single-Cycle false transition specially.

### Notes

**N-1:** No double-counting between B-017 and B-001. B-001 contains `emStocksFilePanel-255` (different cpp line, different row); B-017 contains `emStocksFilePanel-34` only. The C++ AddWakeUpSignal census in `emStocksFilePanel.cpp` is exactly two sites (34, 255), and they are split correctly by audit pattern. No missed B-017 rows in `src/emStocks/*.cpp`: the three row sites are the only AddWakeUpSignal occurrences whose subscribe is *missing-accessor* shaped (others are P-001/P-002 in B-001).

**N-2:** D-008/D-009 compliance: row 3’s in-row `save_timer_signal` allocation uses `Scheduler::create_signal` + `create_timer` (not lazy-Cell as D-008 prescribes for accessor-flips); this is acceptable because D-008 governs *external* accessor allocation shape and row 3 is internal/self-subscribed. D-009 (no polling intermediary) is satisfied by row 3 — the `Option<Instant>` field is an intermediary-style poll (`Instant::now() >= deadline`) and is removed.

### Recommended Pre-Implementation Actions

1. **Resolve C-1 borrow conflict** (one-paragraph spec append) before implementer dispatch. Pick option (a) panel-side `dirty`-driven `touch_save_timer(ectx)` after lb.Cycle, or (b) lb.Cycle signature change.
2. **Verify B-001 G3 covers cpp:38-39 upstream subscribes** (I-1) by reading the B-001 design’s G3 mutator-fire enumeration and the C++ fetcher Cycle. If B-001 G3 omits, file an amendment before B-001 lands.
3. **Decide ownership shape** for `emStocksFileModel` (I-3): by-value + proxy-engine vs. `Rc<RefCell<>>` + charter amendment. Default: by-value, smaller blast radius, no CLAUDE.md change.
4. **Enumerate `emStocksFilePanel::new()` ctx-threading callsites** (I-2): plugin func, six tests. Update plan.
5. **Spec `dirty` clear-points** (I-4): `Save`, `SaveIfNeeded`, post-`Save(true)` in Cycle. Add to success criteria.

## Amendment Log — 2026-05-01

Folded the Adversarial Review (above) into the design body; the review block is preserved verbatim. Changes:

- **C-1 (Critical) resolved** in §"Wiring-shape application" → row 2 (`emStocksFilePanel::Cycle`): replaced the conflicting `lb.Cycle(ectx, model.GetWritableRec(), config)` shape with a sequenced split-borrow. `GetWritableRec` is now mutation-only (sets `dirty`, no scheduler); a separate `model.touch_save_timer(ectx)` call after `lb.Cycle` returns advances the SaveTimer. Inline comment at the callsite cites C-1.
- **C-1 propagation** in §"Wiring-shape application" → row 3 (`emStocksFileModel::Cycle`) §"Mutator changes": rewrote the mutator section to spec the split (`GetWritableRec` rec-mutation half + `touch_save_timer` ectx half); marked the split as language-forced divergence (`DIVERGED:` annotation prescribed at both halves).
- **I-1 (Important) flagged** in new §"Resolutions" item 4: added cross-reference to `docs/superpowers/plans/2026-05-01-B-001-no-wire-emstocks.md`. B-001 G3 may not cover the fetcher's upstream subscribes at `emStocksPricesFetcher.cpp:38-39` (`FileModel->GetChangeSignal()` + `GetFileStateSignal()`); without those, B-017 row 1 is silently undertested. **Coordination deferral**: B-017 row 1 blocked until B-001 G3 widens or documents the gap. Filed as B-001 amendment candidate.
- **I-2 (Important) resolved** in §"Resolutions" item 1: enumerated the exact ripple of threading `&mut C: ConstructCtx` through `emStocksFileModel::new` — `emStocksFilePanel::new()` at `emStocksFilePanel.rs:401`, `emStocksFpPlugin::emStocksFpPluginFunc` at `emStocksFpPlugin.rs:30`, six tests at `emStocksFilePanel.rs:448-502`.
- **I-3 (Important) resolved** in §"Resolutions" item 2: ownership shape is **by-value + proxy-engine** (panel registers itself or a thin proxy as the scheduler engine; model stays held by value). Rejects the prior `Rc<RefCell<emStocksFileModel>>` default. No CLAUDE.md amendment required.
- **I-4 (Important) resolved** in §"Resolutions" item 3 + Success Criteria: enumerated the four `dirty: bool` clear-points (`Save`, `SaveIfNeeded`, post-`Save(true)` in Cycle, Drop). Spec'd the `dirty_unobserved: bool` paired latch consumed by `dirty_since_last_touch()`.
- **M-1 addressed** in §"Open question (still open)" item 5: named the test-only setter `pub(crate) fn set_vfs_good_for_test(&mut self, ectx: &mut EngineCtx<'_>)`.
- **M-2 noted** in Success Criteria as an intended observable fix (not a regression).
- **M-3 verified** in Success Criteria: `emStocksFilePanel.rs:365` caller semantics for `dialog.Cycle() == false` preserved.

**Dispatch status:** Dispatch-ready for rows 2 and 3 once prereqs (B-004 G1) land. **Row 1 has a coordination deferral** pending B-001 G3 scope-widening on the fetcher's upstream FileModel subscribes (I-1) — must be resolved (or explicitly accepted as a known undertest) before row 1 dispatch.
