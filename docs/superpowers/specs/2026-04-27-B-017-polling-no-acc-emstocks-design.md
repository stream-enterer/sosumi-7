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

    // ListBox confirmation-dialog state machine — unchanged (out of B-017 scope).
    let list_box_busy = {
        let Self {
            list_box,
            model,
            config,
            ..
        } = self;
        if let Some(lb) = list_box.as_mut() {
            lb.Cycle(ectx, model.GetWritableRec(), config)
        } else {
            false
        }
    };

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

**Mutator changes:** every site that previously set `save_timer_deadline = Some(Instant::now() + AUTOSAVE_DELAY)` now calls `scheduler.start_timer(self.save_timer_id, 15000, false)`. Sites:
- `OnRecChanged` (mirrors C++ `OnRecChanged: SaveTimer.Start(15000)`).
- `GetWritableRec` (the existing path that lazily starts the timer).

The implementer must thread `&mut EngineCtx` (or `&mut Scheduler`) into both. If signature churn is too wide, an alternative: keep the existing `OnRecChanged` shape but defer the `start_timer` call to the next `Cycle` invocation via a `pending_start_timer: bool` flag. **Default: thread `ectx`** — the equivalent change is already required for emFileModel mutator sites per B-004 G1's pattern.

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

## Open questions for the implementer

1. **`emStocksFileModel::new` constructor-signature change.** Threading `&mut C: ConstructCtx` (or `&mut Scheduler`) through `new` propagates to every call site that constructs the model — currently `emStocksFilePanel::new:401` and any test code. Audit those sites; mirror B-001 G3 / B-004 G1's analogous signature changes for shape precedent.
2. **`emStocksFileModel` engine registration.** No precedent today for an embedded `emStocksFileModel` registering itself as an engine. The `emDirModel::ensure_engine_registered` pattern (referenced in B-016) is the closest analogue; adapt it. If the registration call has to happen in `emStocksFilePanel::Cycle` (lazy, on first wake), the model needs to be wrapped in `Rc<RefCell<>>` — currently held by value at `emStocksFilePanel::model`. Either change the field to `Rc<RefCell<emStocksFileModel>>` (justified: cross-engine reference held by scheduler callback registry — falls under CLAUDE.md §Ownership rule (a)) or use a static thread-local registry pattern. Default: `Rc<RefCell<>>`, with the justification comment cited.
3. **`Drop` impl for `emStocksFileModel`.** Current `Drop` saves if a deadline was set. Post-port, `Drop` cannot easily access the scheduler (no back-pointer per CLAUDE.md). Two options: (a) add a `dirty: bool` flag set by every mutator and consulted in `Drop` to call `Save` directly (no signal involvement); (b) accept that on-drop save happens via the scheduler's normal Cycle path before drop runs (rely on graceful shutdown). C++ depends on the destructor running, so option (a) is the closer mirror. Default: (a).
4. **`emStocksPricesFetcher` mutator-fire enumeration.** B-001 G3's design lists the C++ Signal call sites at lines 70/134/264/272 of `emStocksPricesFetcher.cpp`. The B-001 implementer must enumerate the equivalent Rust mutator sites and add `Signal(self.change_signal)` calls (via `ectx`). If B-001 G3's implementation defers any of those fires, B-017 row 1's tests will reveal the gap (the `UpdateControls`-on-progress assertion would silently pass at zero progress). Coordinate via working-memory session.
5. **Test harness signature.** B-005's harness exposes `Harness::fire(SignalId)` and `Harness::run_cycle(closure)`. Confirm the harness is reachable from emstocks tests; if not, add a minimal local harness mirroring `crates/emfileman/tests/` patterns.

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
