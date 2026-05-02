// Port of C++ emStocksFileModel.h / emStocksFileModel.cpp

use std::path::PathBuf;

use emcore::emCrossPtr::emCrossPtr;
use emcore::emEngineCtx::{DropOnlySignalCtx, EngineCtx, SignalCtx};
use emcore::emFileModel::FileState;
use emcore::emRecFileModel::emRecFileModel;
use emcore::emSignal::SignalId;
use emcore::emTimer::TimerId;

use super::emStocksFetchPricesDialog::emStocksFetchPricesDialog;
use super::emStocksRec::emStocksRec;

/// Save delay matching C++ AUTOSAVE_DELAY_MS = 15000.
const AUTOSAVE_DELAY_MS: u64 = 15000;

/// Port of C++ emStocksFileModel.
///
/// DIVERGED: (language-forced) Composition instead of C++ multiple inheritance —
/// Rust has no MI; composition with delegation is the idiomatic equivalent.
///
/// DIVERGED: (language-forced) C++ `emStocksFileModel : public emEngine`
/// owns its `SaveTimer` (`emTimer`) and self-Cycles on `IsSignaled(SaveTimer.GetSignal())`.
/// Rust embeds the model by-value inside `emStocksFilePanel` (CLAUDE.md §Ownership
/// rejects the `Rc<RefCell<emStocksFileModel>>` shape required to register the
/// model independently as a scheduler engine). The owning panel acts as the
/// proxy engine: it allocates the model's `save_timer_signal` + `save_timer_id`
/// in its own first-Cycle init via `ensure_save_timer(ectx, eid)`, subscribes
/// the panel's engine to the signal, and forwards the `IsSignaled(...)` branch
/// into `save_on_timer_fire(ectx)` from the panel's Cycle. Observable contract
/// matches C++: `SaveTimer.Start(15000)` arms the same scheduler timer; firing
/// drives the same `Save(true)` path. Cite: spec
/// `2026-04-27-B-017-polling-no-acc-emstocks-design.md` §"Resolutions" item 2
/// (I-3 by-value + proxy-engine), spec line 374 (engine-registration shape
/// realised via panel-as-proxy because embedded-model has no independent
/// scheduler reach), and decisions.md D-009 (Instant-poll intermediary
/// removed).
pub struct emStocksFileModel {
    pub file_model: emRecFileModel<emStocksRec>,
    pub PricesFetchingDialog: emCrossPtr<emStocksFetchPricesDialog>,
    /// SaveTimer signal. Allocated lazily by `ensure_save_timer` from the
    /// owning panel's first-Cycle init. Mirrors C++ `SaveTimer.GetSignal()`.
    /// Null until the panel first cycles.
    save_timer_signal: SignalId,
    /// SaveTimer handle in `TimerCentral`. Allocated alongside
    /// `save_timer_signal`. `None` until the panel first cycles.
    save_timer_id: Option<TimerId>,
    /// True iff there are pending writes since the last successful Save.
    /// Cleared by Save / SaveIfNeeded / `save_on_timer_fire` / Drop.
    /// Mirrors the implicit "SaveTimer.IsRunning() => unsaved" invariant in C++.
    dirty: bool,
    /// Paired latch consumed by `dirty_since_last_touch`. Set by mutators
    /// alongside `dirty`; cleared by `touch_save_timer` after the timer is
    /// armed. Lets the panel decide whether to call `touch_save_timer(ectx)`
    /// after `lb.Cycle` returns without re-arming the timer every Cycle.
    dirty_unobserved: bool,
}

impl emStocksFileModel {
    /// Create a new file model for the given path.
    ///
    /// Note: `save_timer_signal` and `save_timer_id` are NOT allocated here —
    /// `new` has no `EngineCtx`/`Scheduler` reach. The owning panel allocates
    /// them on first Cycle via `ensure_save_timer`. Until then, mutators that
    /// would arm the timer (`touch_save_timer`) no-op gracefully — the panel
    /// re-checks `dirty_since_last_touch` after each Cycle, so a write that
    /// preceded the panel's first Cycle is still observed and armed at first
    /// Cycle.
    pub fn new(path: PathBuf) -> Self {
        Self {
            file_model: emRecFileModel::new(path),
            PricesFetchingDialog: emCrossPtr::new(),
            save_timer_signal: SignalId::default(),
            save_timer_id: None,
            dirty: false,
            dirty_unobserved: false,
        }
    }

    /// Allocate `save_timer_signal` + `save_timer_id` and connect `engine_id`
    /// to the signal. Idempotent. Called from the owning panel's first-Cycle
    /// `subscribed_init` block. Mirrors C++ ctor `AddWakeUpSignal(SaveTimer.GetSignal())`
    /// (emStocksFileModel.cpp:21) — but executed from the panel because the
    /// embedded model has no scheduler reach until the panel cycles.
    pub fn ensure_save_timer(
        &mut self,
        ectx: &mut EngineCtx<'_>,
        engine_id: emcore::emEngine::EngineId,
    ) {
        if self.save_timer_id.is_some() {
            return;
        }
        let sig = ectx.scheduler.create_signal();
        let tid = ectx.scheduler.create_timer(sig);
        ectx.connect(sig, engine_id);
        self.save_timer_signal = sig;
        self.save_timer_id = Some(tid);
    }

    /// Test/internal accessor for the SaveTimer signal id.
    #[doc(hidden)]
    pub fn save_timer_signal_for_test(&self) -> SignalId {
        self.save_timer_signal
    }

    /// Test accessor for the dirty flag.
    #[doc(hidden)]
    pub fn dirty_for_test(&self) -> bool {
        self.dirty
    }

    /// Test accessor: is the SaveTimer currently armed (running) on the
    /// scheduler? Returns false if the timer has not been allocated yet.
    #[doc(hidden)]
    pub fn is_save_timer_running_for_test(
        &self,
        scheduler: &emcore::emScheduler::EngineScheduler,
    ) -> bool {
        self.save_timer_id
            .map(|tid| scheduler.is_timer_running(tid))
            .unwrap_or(false)
    }

    /// Access the record data.
    pub fn GetRec(&self) -> &emStocksRec {
        self.file_model.GetMap()
    }

    /// Access the record data mutably. Marks data as dirty.
    ///
    /// DIVERGED: (language-forced) Rec-mutation half of the C++ unified
    /// `GetWritableRec()` + `SaveTimer.Start(15000)` site. Splitting the
    /// rec-mutation from the scheduler-touch is required by the borrow
    /// shape at `emStocksFilePanel::Cycle`: that callsite needs
    /// `lb.Cycle(ectx, model.GetWritableRec(...), config)`, and the
    /// `&mut ectx` borrow for `lb.Cycle` cannot coexist with another
    /// `&mut ectx` borrow inside `GetWritableRec`. The save-timer arm is
    /// hoisted to the paired `touch_save_timer(ectx)` half, sequenced
    /// after `lb.Cycle` returns. Cite: Adversarial Review C-1, design
    /// 2026-04-27-B-017 §"Mutator changes".
    pub fn GetWritableRec(&mut self, ectx: &mut impl SignalCtx) -> &mut emStocksRec {
        let rec = self.file_model.GetWritableMap(ectx);
        self.dirty = true;
        self.dirty_unobserved = true;
        rec
    }

    /// Returns `true` if the model has been mutated since the last
    /// `touch_save_timer` and consumes the latch. The next call returns
    /// `false` until another mutator sets `dirty_unobserved` again.
    pub fn dirty_since_last_touch(&mut self) -> bool {
        let observed = self.dirty_unobserved;
        self.dirty_unobserved = false;
        observed
    }

    /// Timer-arming half of the split `GetWritableRec`/`OnRecChanged`. Mirrors
    /// C++ `SaveTimer.Start(15000)`. No-op if the timer has not been allocated
    /// yet (panel has not first-cycled); the dirty-latch persists, so the next
    /// Cycle pass arms the timer.
    ///
    /// DIVERGED: (language-forced) Scheduler-touch half of the split — see
    /// `GetWritableRec` for the borrow-shape rationale.
    pub fn touch_save_timer(&mut self, ectx: &mut EngineCtx<'_>) {
        let Some(tid) = self.save_timer_id else {
            return;
        };
        // Mirror C++ `SaveTimer.Start(15000)`: only start when not already
        // running — re-Start would reset the deadline, which C++ avoids by
        // checking IsRunning() (emStocksFileModel.cpp:OnRecChanged).
        if !ectx.scheduler.is_timer_running(tid) {
            ectx.scheduler.start_timer(tid, AUTOSAVE_DELAY_MS, false);
        }
    }

    /// Called when record data changes. Starts 15-second save timer.
    /// Port of C++ `OnRecChanged`.
    ///
    /// Production callers thread `ectx` through; test callers (which exist
    /// before this hook fires in production) may pass a `DropOnlySignalCtx`
    /// to mark dirty without arming. Mirrors the split established in
    /// `GetWritableRec`/`touch_save_timer`.
    pub fn OnRecChanged(&mut self, ectx: &mut EngineCtx<'_>) {
        self.dirty = true;
        self.dirty_unobserved = true;
        self.touch_save_timer(ectx);
    }

    /// Save when the SaveTimer signal fires. Mirrors C++ Cycle branch
    /// `if (IsSignaled(SaveTimer.GetSignal())) Save(true);`
    /// (emStocksFileModel.cpp:35). Caller (the owning panel) gates this on
    /// `IsSignaled(model.save_timer_signal)`.
    pub fn save_on_timer_fire(&mut self, ectx: &mut impl SignalCtx) {
        self.file_model.Save(ectx);
        // Post-Save(true) clear-point per design §"I-4 resolved".
        self.dirty = false;
        self.dirty_unobserved = false;
    }

    /// Force save if there are unsaved changes.
    pub fn SaveIfNeeded(&mut self, ectx: &mut impl SignalCtx) {
        if self.dirty {
            self.file_model.Save(ectx);
            self.dirty = false;
            self.dirty_unobserved = false;
        }
    }

    /// Delegate to file_model.
    pub fn TryLoad(&mut self, ectx: &mut impl SignalCtx) {
        self.file_model.TryLoad(ectx);
    }

    /// Delegate to file_model.
    pub fn Save(&mut self, ectx: &mut impl SignalCtx) {
        self.file_model.Save(ectx);
        self.dirty = false;
        self.dirty_unobserved = false;
    }

    /// Delegate to file_model.
    pub fn GetFileState(&self) -> &FileState {
        self.file_model.GetFileState()
    }

    /// Delegate to file_model.
    pub fn GetErrorText(&self) -> &str {
        self.file_model.GetErrorText()
    }
}

impl Drop for emStocksFileModel {
    // DIVERGED: (language-forced) Rust `Drop::drop(&mut self)` has no
    // parameters — no `EngineCtx` / `SchedCtx` is reachable by language. C++
    // `~emStocksFileModel` runs synchronously through `this`'s scheduler
    // reference (per-instance `emEngine` ownership), so its `Save` call's
    // `Signal(ChangeSignal)` fires synchronously. The Rust port keeps the
    // last-chance autosave but uses `DropOnlySignalCtx` to drop the ChangeSignal
    // fire on the floor: at drop time the model is being destroyed and any
    // subscriber observers are tearing down with it, so the missed fire has
    // no observable consequence (D-007 §170 single-callsite escape hatch
    // applies — Rust `Drop` is the canonical "genuinely lacks ectx" site).
    fn drop(&mut self) {
        // Drop-time save mirrors C++ `if (SaveTimer.IsRunning()) Save(true);`.
        // `dirty` is the Rust analogue of the SaveTimer.IsRunning predicate
        // (the timer is armed iff there are pending writes).
        if self.dirty {
            let mut null = DropOnlySignalCtx;
            self.file_model.Save(&mut null);
            // Defensive clear; Drop is the last observer of these flags.
            self.dirty = false;
            self.dirty_unobserved = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_model_create() {
        let model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        assert!(model.GetRec().stocks.is_empty());
    }

    #[test]
    fn file_model_prices_dialog_starts_invalid() {
        let model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        assert!(!model.PricesFetchingDialog.is_valid());
    }

    #[test]
    fn file_model_save_timer_signal_null_until_ensure() {
        let model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        // Default `SignalId` is the slotmap "null" key — equal to itself.
        assert_eq!(model.save_timer_signal, SignalId::default());
        assert!(model.save_timer_id.is_none());
    }

    #[test]
    fn get_writable_rec_marks_dirty() {
        // Rec-mutation half: sets dirty/unobserved. Timer not armed (no
        // ectx reach — panel arms post-lb.Cycle).
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        assert!(!model.dirty);
        assert!(!model.dirty_unobserved);
        let mut null = DropOnlySignalCtx;
        let _rec = model.GetWritableRec(&mut null);
        assert!(model.dirty);
        assert!(model.dirty_unobserved);
    }

    #[test]
    fn dirty_since_last_touch_consumes_latch() {
        // Paired latch: dirty_unobserved set on mutate, cleared by getter.
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        assert!(!model.dirty_since_last_touch());
        let mut null = DropOnlySignalCtx;
        let _rec = model.GetWritableRec(&mut null);
        assert!(model.dirty_since_last_touch());
        assert!(
            !model.dirty_since_last_touch(),
            "second read must observe the latch already consumed"
        );
        // dirty (non-latched) stays true until Save clears it.
        assert!(model.dirty);
    }

    #[test]
    fn save_clears_dirty_and_unobserved() {
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        let mut null = DropOnlySignalCtx;
        let _rec = model.GetWritableRec(&mut null);
        assert!(model.dirty);
        assert!(model.dirty_unobserved);
        model.Save(&mut null);
        assert!(!model.dirty);
        assert!(!model.dirty_unobserved);
    }

    #[test]
    fn save_if_needed_clears_dirty() {
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        let mut null = DropOnlySignalCtx;
        let _rec = model.GetWritableRec(&mut null);
        model.SaveIfNeeded(&mut null);
        assert!(!model.dirty);
        assert!(!model.dirty_unobserved);
    }

    #[test]
    fn save_if_needed_no_op_when_clean() {
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        let mut null = DropOnlySignalCtx;
        // Clean model: SaveIfNeeded should not flip any state.
        model.SaveIfNeeded(&mut null);
        assert!(!model.dirty);
    }

    #[test]
    fn save_on_timer_fire_clears_dirty() {
        // Mirrors C++ Cycle's `IsSignaled(SaveTimer) → Save(true)` branch.
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        let mut null = DropOnlySignalCtx;
        let _rec = model.GetWritableRec(&mut null);
        assert!(model.dirty);
        model.save_on_timer_fire(&mut null);
        assert!(!model.dirty);
        assert!(!model.dirty_unobserved);
    }
}
