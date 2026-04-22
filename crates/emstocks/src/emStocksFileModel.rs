// Port of C++ emStocksFileModel.h / emStocksFileModel.cpp

use std::path::PathBuf;
use std::time::{Duration, Instant};

use emcore::emCrossPtr::emCrossPtr;
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
    pub fn GetWritableRec(&mut self) -> &mut emStocksRec {
        let rec = self.file_model.GetWritableMap();
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
    pub fn CheckSaveTimer(&mut self) -> bool {
        if let Some(deadline) = self.save_timer_deadline {
            if Instant::now() >= deadline {
                self.save_timer_deadline = None;
                self.file_model.Save();
                return true;
            }
        }
        false
    }

    /// Force save if there are unsaved changes.
    pub fn SaveIfNeeded(&mut self) {
        if self.save_timer_deadline.is_some() {
            self.save_timer_deadline = None;
            self.file_model.Save();
        }
    }

    /// Delegate to file_model.
    pub fn TryLoad(&mut self) {
        self.file_model.TryLoad();
    }

    /// Delegate to file_model.
    pub fn Save(&mut self) {
        self.save_timer_deadline = None;
        self.file_model.Save();
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
    fn drop(&mut self) {
        if self.save_timer_deadline.is_some() {
            self.save_timer_deadline = None;
            self.file_model.Save();
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
        assert!(!model.CheckSaveTimer());
    }

    #[test]
    fn file_model_save_if_needed_clears_timer() {
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        model.OnRecChanged();
        assert!(model.save_timer_deadline.is_some());
        model.SaveIfNeeded();
        assert!(model.save_timer_deadline.is_none());
    }

    #[test]
    fn file_model_get_writable_rec_starts_timer() {
        let mut model = emStocksFileModel::new(PathBuf::from("/tmp/test.emStocks"));
        assert!(model.save_timer_deadline.is_none());
        let _rec = model.GetWritableRec();
        assert!(model.save_timer_deadline.is_some());
    }
}
