use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::SystemTime;

use crate::emSignal::SignalId;

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

/// Port of C++ emFileModelClient. Panels implement this to participate
/// in model memory/priority decisions.
pub trait FileModelClient {
    fn get_memory_limit(&self) -> u64;
    fn get_priority(&self) -> f64;
    fn is_reload_annoying(&self) -> bool;
}

/// Read-only view of file model state, erasing the data type T.
/// DIVERGED: C++ emFileModel base class — Rust uses trait for type erasure
/// since emFileModel<T> is generic.
pub trait FileModelState {
    fn GetFileState(&self) -> &FileState;
    fn GetFileProgress(&self) -> f64;
    fn GetErrorText(&self) -> &str;
    fn get_memory_need(&self) -> u64;
    fn GetFileStateSignal(&self) -> SignalId;
}

impl<T> FileModelState for emFileModel<T> {
    fn GetFileState(&self) -> &FileState {
        &self.state
    }
    fn GetFileProgress(&self) -> f64 {
        self.GetFileProgress()
    }
    fn GetErrorText(&self) -> &str {
        &self.error_text
    }
    fn get_memory_need(&self) -> u64 {
        self.memory_need
    }
    fn GetFileStateSignal(&self) -> SignalId {
        self.change_signal
    }
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
pub struct emFileModel<T> {
    data: Option<T>,
    path: PathBuf,
    state: FileState,
    error_text: String,
    change_signal: SignalId,
    memory_limit: usize,
    memory_need: u64,
    file_progress: f64,
    last_mtime: u64,
    last_size: u64,
    out_of_date: bool,
    ignore_update_signal: bool,
    update_signal: SignalId,
    clients: Vec<Weak<RefCell<dyn FileModelClient>>>,
    memory_limit_invalid: bool,
    priority_invalid: bool,
}

impl<T> emFileModel<T> {
    pub fn new(path: PathBuf, signal_id: SignalId, update_signal: SignalId) -> Self {
        Self {
            data: None,
            path,
            state: FileState::Waiting,
            error_text: String::new(),
            change_signal: signal_id,
            memory_limit: usize::MAX,
            memory_need: 0,
            file_progress: 0.0,
            last_mtime: 0,
            last_size: 0,
            out_of_date: false,
            ignore_update_signal: false,
            update_signal,
            clients: Vec::new(),
            memory_limit_invalid: true,
            priority_invalid: true,
        }
    }

    pub fn GetFileState(&self) -> &FileState {
        &self.state
    }

    pub fn GetMap(&self) -> Option<&T> {
        self.data.as_ref()
    }

    pub fn GetWritableMap(&mut self) -> Option<&mut T> {
        self.data.as_mut()
    }

    pub fn GetFilePath(&self) -> &Path {
        &self.path
    }

    pub fn GetFileStateSignal(&self) -> SignalId {
        self.change_signal
    }

    pub fn set_memory_limit(&mut self, limit: usize) {
        self.memory_limit = limit;
    }

    pub fn GetMemoryLimit(&self) -> usize {
        self.memory_limit
    }

    pub fn GetFileProgress(&self) -> f64 {
        match &self.state {
            FileState::Loading { progress } => *progress,
            FileState::Loaded | FileState::Unsaved => 100.0,
            _ => 0.0,
        }
    }

    /// Begin loading. Transitions from `Waiting` to `Loading`.
    /// Also allows retry from `LoadError` and `TooCostly`.
    pub fn Load(&mut self) -> bool {
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
    pub fn SetUnsavedState(&mut self) {
        if matches!(self.state, FileState::Loaded) {
            self.state = FileState::Unsaved;
        }
    }

    /// Begin saving. Transitions from `Unsaved` to `Saving`.
    pub fn Save(&mut self) -> bool {
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
    pub fn HardResetFileState(&mut self) -> bool {
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
    pub fn CalcMemoryNeed(&mut self, need: u64) {
        self.memory_need = need;
    }

    /// Port of C++ `emFileModel::CalcFileProgress`.
    /// Update and return the cached file progress value.
    pub fn CalcFileProgress(&mut self, progress: f64) {
        self.file_progress = progress;
    }

    /// Port of C++ `emFileModel::TryFetchDate`.
    /// Store file metadata for freshness checking.
    pub fn TryFetchDate(&mut self, mtime: u64, size: u64) {
        self.last_mtime = mtime;
        self.last_size = size;
    }

    /// Port of C++ `emFileModel::IsOutOfDate`.
    /// Check if the stored file metadata differs from current.
    pub fn IsOutOfDate(&mut self, current_mtime: u64, current_size: u64) -> bool {
        let out_of_date = self.last_mtime != current_mtime || self.last_size != current_size;
        self.out_of_date = out_of_date;
        out_of_date
    }

    /// Port of C++ `emFileModel::GetIgnoreUpdateSignal`.
    pub fn GetIgnoreUpdateSignal(&self) -> bool {
        self.ignore_update_signal
    }

    /// Port of C++ `emFileModel::SetIgnoreUpdateSignal`.
    pub fn set_ignore_update_signal(&mut self, ignore: bool) {
        self.ignore_update_signal = ignore;
    }

    /// Port of C++ `emFileModel::AcquireUpdateSignalModel`.
    /// Returns the update signal ID.
    pub fn AcquireUpdateSignalModel(&self) -> SignalId {
        self.update_signal
    }

    pub fn GetErrorText(&self) -> &str {
        &self.error_text
    }

    fn try_fetch_date(&self) -> Result<(u64, u64), String> {
        let meta = std::fs::metadata(&self.path)
            .map_err(|e| format!("Failed to get file info for {:?}: {}", self.path, e))?;
        let mtime_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let size = meta.len();
        Ok((mtime_secs, size))
    }

    /// Port of C++ `StepLoading()`. Returns true if state changed.
    pub fn step_loading<O: FileModelOps>(&mut self, ops: &mut O) -> bool {
        let mut ready = false;
        let mut state_changed = false;

        if matches!(self.state, FileState::Loading { .. }) {
            match ops.try_continue_loading() {
                Err(e) => {
                    ops.quit_loading();
                    ops.reset_data();
                    self.state = FileState::LoadError(e);
                    return true;
                }
                Ok(done) => ready = done,
            }
        } else if matches!(self.state, FileState::Waiting) {
            match self.try_fetch_date() {
                Err(e) => {
                    self.state = FileState::LoadError(e);
                    return true;
                }
                Ok((m, s)) => {
                    self.last_mtime = m;
                    self.last_size = s;
                }
            }
            ops.reset_data();
            self.state = FileState::Loading { progress: 0.0 };
            if let Err(e) = ops.try_start_loading() {
                ops.quit_loading();
                ops.reset_data();
                self.state = FileState::LoadError(e);
                return true;
            }
            state_changed = true;
        } else {
            return false;
        }

        let memory_need = ops.calc_memory_need().max(1);
        self.memory_need = memory_need;
        if memory_need > self.memory_limit as u64 {
            ops.quit_loading();
            ops.reset_data();
            self.state = FileState::TooCostly;
            return true;
        }
        if !ready {
            return state_changed;
        }
        ops.quit_loading();
        self.state = FileState::Loaded;
        true
    }

    /// Port of C++ `StepSaving()`. Returns true if state changed.
    pub fn step_saving<O: FileModelOps>(&mut self, ops: &mut O) -> bool {
        if matches!(self.state, FileState::Saving) {
            match ops.try_continue_saving() {
                Err(e) => {
                    ops.quit_saving();
                    self.state = FileState::SaveError(e);
                    return true;
                }
                Ok(false) => return false,
                Ok(true) => {}
            }
            ops.quit_saving();
            match self.try_fetch_date() {
                Err(e) => {
                    self.state = FileState::SaveError(e);
                    return true;
                }
                Ok((m, s)) => {
                    self.last_mtime = m;
                    self.last_size = s;
                }
            }
            self.state = FileState::Loaded;
            let memory_need = ops.calc_memory_need().max(1);
            self.memory_need = memory_need;
            if memory_need > self.memory_limit as u64 {
                ops.reset_data();
                self.state = FileState::TooCostly;
            }
            return true;
        }
        if matches!(self.state, FileState::Unsaved) {
            self.state = FileState::Saving;
            self.error_text.clear();
            if let Err(e) = ops.try_start_saving() {
                ops.quit_saving();
                self.state = FileState::SaveError(e);
                return true;
            }
            return true;
        }
        false
    }

    /// Port of C++ `Load(bool immediately)`.
    pub fn load<O: FileModelOps>(&mut self, ops: &mut O, immediately: bool) {
        if matches!(self.state, FileState::Waiting | FileState::Loading { .. }) {
            self.step_loading(ops);
            if immediately {
                while matches!(self.state, FileState::Loading { .. }) {
                    self.step_loading(ops);
                }
            }
        }
    }

    /// Port of C++ `Save(bool immediately)`.
    pub fn save<O: FileModelOps>(&mut self, ops: &mut O, immediately: bool) {
        if matches!(self.state, FileState::Unsaved | FileState::Saving) {
            self.step_saving(ops);
            if immediately {
                while matches!(self.state, FileState::Saving) {
                    self.step_saving(ops);
                }
            }
        }
    }

    /// Port of C++ `HardResetFileState()`.
    pub fn hard_reset_file_state<O: FileModelOps>(&mut self, ops: &mut O) {
        if matches!(self.state, FileState::Loading { .. }) {
            ops.quit_loading();
            ops.reset_data();
        } else if matches!(self.state, FileState::Saving) {
            ops.quit_saving();
            ops.reset_data();
        } else if matches!(
            self.state,
            FileState::Loaded | FileState::Unsaved | FileState::SaveError(_)
        ) {
            ops.reset_data();
        }
        self.state = FileState::TooCostly;
        self.memory_need = 1;
        self.error_text.clear();
        if self.memory_limit as u64 >= 1 {
            self.state = FileState::Waiting;
        }
    }

    /// Port of C++ `SetUnsavedState()`.
    pub fn set_unsaved_state<O: FileModelOps>(&mut self, ops: &mut O) {
        if self.state != FileState::Unsaved {
            if matches!(self.state, FileState::Loading { .. }) {
                ops.quit_loading();
            } else if matches!(self.state, FileState::Saving) {
                ops.quit_saving();
            }
            self.state = FileState::Unsaved;
            self.error_text.clear();
        }
    }

    /// Register a client to participate in memory/priority decisions.
    /// Port of C++ `emFileModel::AddClient`.
    pub fn AddClient(&mut self, client: &Rc<RefCell<dyn FileModelClient>>) {
        self.clients.push(Rc::downgrade(client));
        self.memory_limit_invalid = true;
        self.priority_invalid = true;
    }

    /// Unregister a client. Port of C++ `emFileModel::RemoveClient`.
    pub fn RemoveClient(&mut self, client: &Rc<RefCell<dyn FileModelClient>>) {
        let ptr = Rc::as_ptr(client);
        self.clients.retain(|w| {
            w.upgrade()
                .is_some_and(|rc| !std::ptr::eq(Rc::as_ptr(&rc), ptr))
        });
        self.memory_limit_invalid = true;
        self.priority_invalid = true;
    }

    /// Number of live (non-dropped) clients.
    pub fn client_count(&self) -> usize {
        self.clients
            .iter()
            .filter(|w| w.upgrade().is_some())
            .count()
    }

    /// Recompute memory limit as max across all clients.
    /// Port of C++ `emFileModel::UpdateMemoryLimit`.
    pub fn UpdateMemoryLimit(&mut self) {
        self.clients.retain(|w| w.upgrade().is_some());
        let new_limit = self
            .clients
            .iter()
            .filter_map(|w| w.upgrade())
            .map(|c| c.borrow().get_memory_limit())
            .max()
            .unwrap_or(0);
        self.memory_limit = new_limit as usize;
        self.memory_limit_invalid = false;
    }

    /// Recompute priority as max across all clients.
    /// Port of C++ `emFileModel::UpdatePriority`.
    pub fn UpdatePriority(&mut self) -> f64 {
        self.clients.retain(|w| w.upgrade().is_some());
        let max_pri = self
            .clients
            .iter()
            .filter_map(|w| w.upgrade())
            .map(|c| c.borrow().get_priority())
            .fold(0.0_f64, f64::max);
        self.priority_invalid = false;
        max_pri
    }

    /// Port of C++ `emFileModel::IsAnyClientReloadAnnoying`.
    pub fn IsAnyClientReloadAnnoying(&self) -> bool {
        self.clients
            .iter()
            .filter_map(|w| w.upgrade())
            .any(|c| c.borrow().is_reload_annoying())
    }

    pub fn is_memory_limit_invalid(&self) -> bool {
        self.memory_limit_invalid
    }

    pub fn is_priority_invalid(&self) -> bool {
        self.priority_invalid
    }

    pub fn InvalidateMemoryLimit(&mut self) {
        self.memory_limit_invalid = true;
    }

    pub fn InvalidatePriority(&mut self) {
        self.priority_invalid = true;
    }
}

/// Port of C++ emAbsoluteFileModelClient.
/// DIVERGED: Uses Weak<RefCell<T>> instead of raw pointer + emRef.
/// Rust has no emFileModelClient base class; this is a standalone
/// wrapper that tracks a file model's lifetime via Weak reference.
pub struct emAbsoluteFileModelClient<T> {
    model: Option<Weak<RefCell<T>>>,
}

impl<T> emAbsoluteFileModelClient<T> {
    /// Create an empty client with no model.
    pub fn new() -> Self {
        Self { model: None }
    }

    /// Set the tracked model. Port of C++ `SetModel`.
    pub fn SetModel(&mut self, model: &Rc<RefCell<T>>) {
        self.model = Some(Rc::downgrade(model));
    }

    /// Clear the tracked model. Port of C++ `ClearModel`.
    pub fn ClearModel(&mut self) {
        self.model = None;
    }

    /// Returns the model if still alive, None if dropped.
    /// Port of C++ `GetModel`.
    pub fn GetModel(&self) -> Option<Rc<RefCell<T>>> {
        self.model.as_ref().and_then(|w| w.upgrade())
    }
}

impl<T> Default for emAbsoluteFileModelClient<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockClient {
        memory_limit: u64,
        priority: f64,
        reload_annoying: bool,
    }

    impl MockClient {
        fn new(memory_limit: u64, priority: f64, reload_annoying: bool) -> Self {
            Self {
                memory_limit,
                priority,
                reload_annoying,
            }
        }
    }

    impl FileModelClient for MockClient {
        fn get_memory_limit(&self) -> u64 {
            self.memory_limit
        }
        fn get_priority(&self) -> f64 {
            self.priority
        }
        fn is_reload_annoying(&self) -> bool {
            self.reload_annoying
        }
    }

    fn make_model() -> emFileModel<()> {
        emFileModel::new(
            PathBuf::from("/tmp/test.dat"),
            SignalId::default(),
            SignalId::default(),
        )
    }

    fn make_client(limit: u64, priority: f64, annoying: bool) -> Rc<RefCell<dyn FileModelClient>> {
        Rc::new(RefCell::new(MockClient::new(limit, priority, annoying)))
    }

    #[test]
    fn add_remove_client() {
        let mut model = make_model();
        let client = make_client(1024, 1.0, false);

        model.AddClient(&client);
        assert_eq!(model.client_count(), 1);

        model.RemoveClient(&client);
        assert_eq!(model.client_count(), 0);
    }

    #[test]
    fn add_client_invalidates_flags() {
        let mut model = make_model();
        model.memory_limit_invalid = false;
        model.priority_invalid = false;

        let client = make_client(1024, 1.0, false);
        model.AddClient(&client);

        assert!(model.is_memory_limit_invalid());
        assert!(model.is_priority_invalid());
    }

    #[test]
    fn remove_client_invalidates_flags() {
        let mut model = make_model();
        let client = make_client(1024, 1.0, false);
        model.AddClient(&client);
        model.memory_limit_invalid = false;
        model.priority_invalid = false;

        model.RemoveClient(&client);
        assert!(model.is_memory_limit_invalid());
        assert!(model.is_priority_invalid());
    }

    #[test]
    fn dead_client_cleaned_from_count() {
        let mut model = make_model();
        {
            let client = make_client(1024, 1.0, false);
            model.AddClient(&client);
            assert_eq!(model.client_count(), 1);
        }
        // client dropped
        assert_eq!(model.client_count(), 0);
    }

    #[test]
    fn update_memory_limit_takes_max() {
        let mut model = make_model();
        let c1 = make_client(100, 0.0, false);
        let c2 = make_client(500, 0.0, false);
        let c3 = make_client(200, 0.0, false);
        model.AddClient(&c1);
        model.AddClient(&c2);
        model.AddClient(&c3);

        model.UpdateMemoryLimit();

        assert_eq!(model.GetMemoryLimit(), 500);
        assert!(!model.is_memory_limit_invalid());
    }

    #[test]
    fn update_memory_limit_cleans_dead_refs() {
        let mut model = make_model();
        let c1 = make_client(100, 0.0, false);
        model.AddClient(&c1);
        {
            let c2 = make_client(999, 0.0, false);
            model.AddClient(&c2);
        }
        // c2 is dead now

        model.UpdateMemoryLimit();

        assert_eq!(model.GetMemoryLimit(), 100);
        assert_eq!(model.client_count(), 1);
    }

    #[test]
    fn update_memory_limit_no_clients() {
        let mut model = make_model();
        model.UpdateMemoryLimit();
        assert_eq!(model.GetMemoryLimit(), 0);
    }

    #[test]
    fn update_priority_takes_max() {
        let mut model = make_model();
        let c1 = make_client(0, 1.5, false);
        let c2 = make_client(0, 3.7, false);
        let c3 = make_client(0, 2.0, false);
        model.AddClient(&c1);
        model.AddClient(&c2);
        model.AddClient(&c3);

        let pri = model.UpdatePriority();

        assert!((pri - 3.7).abs() < f64::EPSILON);
        assert!(!model.is_priority_invalid());
    }

    #[test]
    fn update_priority_cleans_dead_refs() {
        let mut model = make_model();
        let c1 = make_client(0, 1.0, false);
        model.AddClient(&c1);
        {
            let c2 = make_client(0, 99.0, false);
            model.AddClient(&c2);
        }

        let pri = model.UpdatePriority();

        assert!((pri - 1.0).abs() < f64::EPSILON);
        assert_eq!(model.client_count(), 1);
    }

    #[test]
    fn update_priority_no_clients() {
        let mut model = make_model();
        let pri = model.UpdatePriority();
        assert!((pri - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn is_any_client_reload_annoying_true() {
        let mut model = make_model();
        let c1 = make_client(0, 0.0, false);
        let c2 = make_client(0, 0.0, true);
        model.AddClient(&c1);
        model.AddClient(&c2);

        assert!(model.IsAnyClientReloadAnnoying());
    }

    #[test]
    fn is_any_client_reload_annoying_false() {
        let mut model = make_model();
        let c1 = make_client(0, 0.0, false);
        let c2 = make_client(0, 0.0, false);
        model.AddClient(&c1);
        model.AddClient(&c2);

        assert!(!model.IsAnyClientReloadAnnoying());
    }

    #[test]
    fn is_any_client_reload_annoying_no_clients() {
        let model = make_model();
        assert!(!model.IsAnyClientReloadAnnoying());
    }

    #[test]
    fn file_model_state_trait_object() {
        let model = make_model();
        let state: &dyn FileModelState = &model;
        assert_eq!(*state.GetFileState(), FileState::Waiting);
        assert!((state.GetFileProgress() - 0.0).abs() < f64::EPSILON);
        assert_eq!(state.GetErrorText(), "");
        assert_eq!(state.get_memory_need(), 0);
    }

    #[test]
    fn invalidate_methods() {
        let mut model = make_model();
        model.memory_limit_invalid = false;
        model.priority_invalid = false;

        model.InvalidateMemoryLimit();
        assert!(model.is_memory_limit_invalid());

        model.InvalidatePriority();
        assert!(model.is_priority_invalid());
    }

    #[test]
    fn scheduler_drives_loading_via_callback() {
        use crate::emPriSchedAgent::PriSchedModel;
        use crate::emScheduler::EngineScheduler;

        let mut sched = EngineScheduler::new();
        let mut ps_model = PriSchedModel::new(&mut sched);

        let model: Rc<RefCell<emFileModel<String>>> = Rc::new(RefCell::new(emFileModel::new(
            PathBuf::from("/dev/null"),
            SignalId::default(),
            SignalId::default(),
        )));

        // Create a GotAccess callback that drives loading
        let m = Rc::clone(&model);
        let agent = ps_model.add_agent(
            1.0,
            Box::new(move || {
                let mut model = m.borrow_mut();
                model.complete_load("loaded".to_string());
            }),
        );

        ps_model.RequestAccess(agent, &mut sched);
        {
            use crate::emWindow::emWindow;
            use std::collections::HashMap;
            use winit::window::WindowId;
            let mut windows: HashMap<WindowId, emWindow> = HashMap::new();
            let __root_ctx = crate::emContext::emContext::NewRoot();
            let mut __fw: Vec<_> = Vec::new();
            let mut __pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
                Vec::new();
            let mut __input_state = crate::emInputState::emInputState::new();
            let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
                std::cell::RefCell::new(None);
            let __pa: std::rc::Rc<std::cell::RefCell<Vec<crate::emGUIFramework::DeferredAction>>> =
                std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            sched.DoTimeSlice(
                &mut windows,
                &__root_ctx,
                &mut __fw,
                &mut __pending_inputs,
                &mut __input_state,
                &__cb,
                &__pa,
            );
        }

        assert!(ps_model.HasAccess(agent));
        assert_eq!(*model.borrow().GetFileState(), FileState::Loaded);
        assert_eq!(model.borrow().GetMap(), Some(&"loaded".to_string()));

        ps_model.ReleaseAccess(agent, &mut sched);
        ps_model.remove(&mut sched);
    }
}
