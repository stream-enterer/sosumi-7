use std::path::{Path, PathBuf};

use crate::scheduler::SignalId;

/// Loading/saving state for a file-backed model.
///
/// Matches the C++ emFileModel state machine with all 8 states.
#[derive(Debug, Clone, PartialEq)]
pub enum FileState {
    /// Waiting to be loaded (initial state).
    Waiting,
    /// Currently loading with progress (0.0 to 100.0).
    Loading { progress: f64 },
    /// Successfully loaded.
    Loaded,
    /// Data has been modified and not yet saved.
    Unsaved,
    /// Currently saving.
    Saving,
    /// Load failed with an error message.
    LoadError(String),
    /// Save failed with an error message.
    SaveError(String),
    /// Loading would exceed the memory limit.
    TooCostly,
}

/// Trait for file model loading/saving operations.
///
/// Port of C++ emFileModel's protected pure virtual methods. Derived models
/// implement this to define how data is loaded from and saved to disk.
pub trait FileModelOps {
    /// Reset data and free all memory. Port of C++ `ResetData()`.
    fn reset_data(&mut self);

    /// Initialize loading. Port of C++ `TryStartLoading()`.
    /// Called once when transitioning from Waiting to Loading.
    fn try_start_loading(&mut self) -> Result<(), String>;

    /// Continue loading one step. Returns true when complete.
    /// Port of C++ `TryContinueLoading()`.
    fn try_continue_loading(&mut self) -> Result<bool, String>;

    /// Finalize loading after last continue returns true.
    /// Port of C++ `QuitLoading()`.
    fn quit_loading(&mut self);

    /// Initialize saving. Port of C++ `TryStartSaving()`.
    fn try_start_saving(&mut self) -> Result<(), String>;

    /// Continue saving one step. Returns true when complete.
    /// Port of C++ `TryContinueSaving()`.
    fn try_continue_saving(&mut self) -> Result<bool, String>;

    /// Finalize saving. Port of C++ `QuitSaving()`.
    fn quit_saving(&mut self);

    /// Calculate total bytes needed/allocated for current data.
    /// Port of C++ `CalcMemoryNeed()`.
    fn calc_memory_need(&self) -> u64;

    /// Calculate loading/saving progress (0.0-100.0).
    /// Port of C++ `CalcFileProgress()`.
    fn calc_file_progress(&self) -> f64;
}

/// A file-backed data model with a loading state machine.
///
/// The loading/saving lifecycle is driven by the caller (typically a scheduler
/// engine). The abstract loading/saving operations are implemented via the
/// `FileModelOps` trait.
pub struct FileModel<T> {
    data: Option<T>,
    path: PathBuf,
    state: FileState,
    change_signal: SignalId,
    memory_limit: usize,
    memory_need: u64,
    file_progress: f64,
    last_mtime: u64,
    last_size: u64,
    out_of_date: bool,
    ignore_update_signal: bool,
    update_signal: SignalId,
}

impl<T> FileModel<T> {
    pub fn new(path: PathBuf, signal_id: SignalId, update_signal: SignalId) -> Self {
        Self {
            data: None,
            path,
            state: FileState::Waiting,
            change_signal: signal_id,
            memory_limit: usize::MAX,
            memory_need: 0,
            file_progress: 0.0,
            last_mtime: 0,
            last_size: 0,
            out_of_date: false,
            ignore_update_signal: false,
            update_signal,
        }
    }

    pub fn state(&self) -> &FileState {
        &self.state
    }

    pub fn data(&self) -> Option<&T> {
        self.data.as_ref()
    }

    pub fn data_mut(&mut self) -> Option<&mut T> {
        self.data.as_mut()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn change_signal(&self) -> SignalId {
        self.change_signal
    }

    pub fn set_memory_limit(&mut self, limit: usize) {
        self.memory_limit = limit;
    }

    pub fn memory_limit(&self) -> usize {
        self.memory_limit
    }

    pub fn progress(&self) -> f64 {
        match &self.state {
            FileState::Loading { progress } => *progress,
            FileState::Loaded | FileState::Unsaved => 100.0,
            _ => 0.0,
        }
    }

    /// Begin loading. Transitions from `Waiting` to `Loading`.
    /// Also allows retry from `LoadError` and `TooCostly`.
    pub fn request_load(&mut self) -> bool {
        match &self.state {
            FileState::Waiting | FileState::LoadError(_) | FileState::TooCostly => {
                self.state = FileState::Loading { progress: 0.0 };
                true
            }
            _ => false,
        }
    }

    /// Set loading progress (0.0 to 100.0).
    pub fn set_progress(&mut self, progress: f64) {
        if matches!(self.state, FileState::Loading { .. }) {
            self.state = FileState::Loading { progress };
        }
    }

    /// Complete loading with the loaded data.
    pub fn complete_load(&mut self, data: T) {
        self.data = Some(data);
        self.state = FileState::Loaded;
    }

    /// Fail loading with an error message.
    pub fn fail_load(&mut self, error: String) {
        self.state = FileState::LoadError(error);
    }

    /// Mark the data as too costly to load.
    pub fn mark_too_costly(&mut self) {
        self.state = FileState::TooCostly;
    }

    /// Mark data as modified (unsaved).
    pub fn mark_unsaved(&mut self) {
        if matches!(self.state, FileState::Loaded) {
            self.state = FileState::Unsaved;
        }
    }

    /// Begin saving. Transitions from `Unsaved` to `Saving`.
    pub fn request_save(&mut self) -> bool {
        match &self.state {
            FileState::Unsaved | FileState::SaveError(_) => {
                self.state = FileState::Saving;
                true
            }
            _ => false,
        }
    }

    /// Complete saving.
    pub fn complete_save(&mut self) {
        if matches!(self.state, FileState::Saving) {
            self.state = FileState::Loaded;
        }
    }

    /// Fail saving with an error message.
    pub fn fail_save(&mut self, error: String) {
        self.state = FileState::SaveError(error);
    }

    /// Reset to `Waiting` and clear data.
    pub fn reset(&mut self) -> bool {
        if self.state == FileState::Waiting && self.data.is_none() {
            return false;
        }
        self.data = None;
        self.state = FileState::Waiting;
        true
    }

    /// Port of C++ `emFileModel::GetMemoryNeed`.
    /// Returns the last calculated memory need value.
    pub fn get_memory_need(&self) -> u64 {
        self.memory_need
    }

    /// Port of C++ `emFileModel::Update`.
    /// Retry failed loads, unload out-of-date files.
    pub fn update(&mut self) {
        match &self.state {
            FileState::LoadError(_) | FileState::TooCostly => {
                self.state = FileState::Waiting;
            }
            FileState::Loaded => {
                if self.out_of_date {
                    self.data = None;
                    self.state = FileState::Waiting;
                    self.out_of_date = false;
                }
            }
            _ => {}
        }
    }

    /// Port of C++ `emFileModel::ResetData`.
    /// Clear data and return to Waiting state. Resets all tracking fields.
    pub fn reset_data(&mut self) {
        self.data = None;
        self.state = FileState::Waiting;
        self.memory_need = 0;
        self.file_progress = 0.0;
        self.out_of_date = false;
    }

    /// Port of C++ `emFileModel::ClearSaveError`.
    /// Transition from SaveError back to Unsaved.
    pub fn clear_save_error(&mut self) {
        if matches!(self.state, FileState::SaveError(_)) {
            self.state = FileState::Unsaved;
        }
    }

    /// Port of C++ `emFileModel::CalcMemoryNeed`.
    /// Update and return the cached memory need value.
    pub fn update_memory_need(&mut self, need: u64) {
        self.memory_need = need;
    }

    /// Port of C++ `emFileModel::CalcFileProgress`.
    /// Update and return the cached file progress value.
    pub fn update_file_progress(&mut self, progress: f64) {
        self.file_progress = progress;
    }

    /// Port of C++ `emFileModel::TryFetchDate`.
    /// Store file metadata for freshness checking.
    pub fn set_file_date(&mut self, mtime: u64, size: u64) {
        self.last_mtime = mtime;
        self.last_size = size;
    }

    /// Port of C++ `emFileModel::IsOutOfDate`.
    /// Check if the stored file metadata differs from current.
    pub fn check_out_of_date(&mut self, current_mtime: u64, current_size: u64) -> bool {
        let out_of_date = self.last_mtime != current_mtime || self.last_size != current_size;
        self.out_of_date = out_of_date;
        out_of_date
    }

    /// Port of C++ `emFileModel::GetIgnoreUpdateSignal`.
    pub fn ignore_update_signal(&self) -> bool {
        self.ignore_update_signal
    }

    /// Port of C++ `emFileModel::SetIgnoreUpdateSignal`.
    pub fn set_ignore_update_signal(&mut self, ignore: bool) {
        self.ignore_update_signal = ignore;
    }

    /// Port of C++ `emFileModel::AcquireUpdateSignalModel`.
    /// Returns the update signal ID.
    pub fn update_signal(&self) -> SignalId {
        self.update_signal
    }
}
