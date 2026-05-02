// Port of C++ emStocksFileModel.h / emStocksFileModel.cpp

use std::path::PathBuf;
use std::time::{Duration, Instant};

use emcore::emCrossPtr::emCrossPtr;
use emcore::emEngineCtx::{DropOnlySignalCtx, SignalCtx};
use emcore::emFileModel::FileState;
use emcore::emRecFileModel::emRecFileModel;

use super::emStocksFetchPricesDialog::emStocksFetchPricesDialog;
use super::emStocksRec::emStocksRec;

/// Save delay matching C++ AUTOSAVE_DELAY_MS = 15000.
const AUTOSAVE_DELAY: Duration = Duration::from_millis(15000);

/// Port of C++ emStocksFileModel.
/// DIVERGED: (language-forced) Composition instead of C++ multiple inheritance — Rust has no MI; composition with delegation is the idiomatic equivalent.
/// Save timer uses std::time::Instant instead of emTimer — emTimer::TimerCentral is
/// internal to emcore; Instant provides the same delayed-save behavior.
pub struct emStocksFileModel {
    pub file_model: emRecFileModel<emStocksRec>,
    pub PricesFetchingDialog: emCrossPtr<emStocksFetchPricesDialog>,
    save_timer_deadline: Option<Instant>,
}

impl emStocksFileModel {
    /// Create a new file model for the given path.
    pub fn new(path: PathBuf) -> Self {
        Self {
            file_model: emRecFileModel::new(path),
            PricesFetchingDialog: emCrossPtr::new(),
            save_timer_deadline: None,
        }
    }

    /// Access the record data.
    pub fn GetRec(&self) -> &emStocksRec {
        self.file_model.GetMap()
    }

    /// Access the record data mutably. Marks data as changed (starts save timer).
    pub fn GetWritableRec(&mut self, ectx: &mut impl SignalCtx) -> &mut emStocksRec {
        let rec = self.file_model.GetWritableMap(ectx);
        // GetWritableMap already transitions to Unsaved; start save timer too.
        if self.save_timer_deadline.is_none() {
            self.save_timer_deadline = Some(Instant::now() + AUTOSAVE_DELAY);
        }
        rec
    }

    /// Called when record data changes. Starts 15-second save timer.
    /// Port of C++ OnRecChanged.
    pub fn OnRecChanged(&mut self) {
        if self.save_timer_deadline.is_none() {
            self.save_timer_deadline = Some(Instant::now() + AUTOSAVE_DELAY);
        }
    }

    /// Check if save timer has fired and save if needed.
    /// Port of C++ Cycle (save timer part).
    /// Returns true if a save was performed.
    pub fn CheckSaveTimer(&mut self, ectx: &mut impl SignalCtx) -> bool {
        if let Some(deadline) = self.save_timer_deadline {
            if Instant::now() >= deadline {
                self.save_timer_deadline = None;
                self.file_model.Save(ectx);
                return true;
            }
        }
        false
    }

    /// Force save if there are unsaved changes.
    pub fn SaveIfNeeded(&mut self, ectx: &mut impl SignalCtx) {
        if self.save_timer_deadline.is_some() {
            self.save_timer_deadline = None;
            self.file_model.Save(ectx);
        }
    }

    /// Delegate to file_model.
    pub fn TryLoad(&mut self, ectx: &mut impl SignalCtx) {
        self.file_model.TryLoad(ectx);
    }

    /// Delegate to file_model.
    pub fn Save(&mut self, ectx: &mut impl SignalCtx) {
        self.save_timer_deadline = None;
        self.file_model.Save(ectx);
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
        if self.save_timer_deadline.is_some() {
            self.save_timer_deadline = None;
            let mut null = DropOnlySignalCtx;
            self.file_model.Save(&mut null);
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
    fn file_model_on_rec_changed_starts_timer() {
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        assert!(model.save_timer_deadline.is_none());
        model.OnRecChanged();
        assert!(model.save_timer_deadline.is_some());
    }

    #[test]
    fn file_model_check_save_timer_not_expired() {
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        model.OnRecChanged();
        // Timer just started, shouldn't fire yet
        let mut null = DropOnlySignalCtx;
        assert!(!model.CheckSaveTimer(&mut null));
    }

    #[test]
    fn file_model_save_if_needed_clears_timer() {
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        model.OnRecChanged();
        assert!(model.save_timer_deadline.is_some());
        let mut null = DropOnlySignalCtx;
        model.SaveIfNeeded(&mut null);
        assert!(model.save_timer_deadline.is_none());
    }

    #[test]
    fn file_model_get_writable_rec_starts_timer() {
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        assert!(model.save_timer_deadline.is_none());
        let mut null = DropOnlySignalCtx;
        let _rec = model.GetWritableRec(&mut null);
        assert!(model.save_timer_deadline.is_some());
    }
}
