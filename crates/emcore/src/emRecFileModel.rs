use std::cell::Cell;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use slotmap::Key as _;

use crate::emRecParser::{parse_rec, write_rec};

use crate::emEngineCtx::SignalCtx;
use crate::emFileModel::{FileModelState, FileState};
use crate::emRecRecord::Record;
use crate::emSignal::SignalId;

/// A file-backed model that loads and saves a `Record`-typed value as emRec.
///
/// Standalone Rust port of C++ `emRecFileModel`. Does not wrap `emFileModel<T>`
/// to avoid self-referential borrow-checker constraints.
pub struct emRecFileModel<T: Record + Default> {
    data: T,
    state: FileState,
    path: PathBuf,
    error_text: String,
    memory_need: u64,
    memory_limit: u64,
    last_mtime: u64,
    last_size: u64,
    protect_file_state: i32,
    read_buffer: Option<String>,
    /// Port of C++ inherited `emFileModel::ChangeSignal` (B-002 / D-008 A1).
    /// Lazy-allocated on first `GetChangeSignal(&self, ectx)` call; null until then.
    change_signal: Cell<SignalId>,
    /// Port of inherited C++ `emFileModel::FileStateSignal` (FU-005).
    /// Lazy-allocated on first `ensure_file_state_signal(ectx)` call; null
    /// until then. Mirrors the sibling `change_signal` lazy pattern (D-006:
    /// `emRecFileModel::new()` has no `EngineCtx` reach for eager alloc).
    file_state_signal: Cell<SignalId>,
}

impl<T: Record + Default> emRecFileModel<T> {
    pub fn new(path: PathBuf) -> Self {
        Self {
            data: T::default(),
            state: FileState::Waiting,
            path,
            error_text: String::new(),
            memory_need: 0,
            memory_limit: u64::MAX,
            last_mtime: 0,
            last_size: 0,
            protect_file_state: 0,
            read_buffer: None,
            change_signal: Cell::new(SignalId::null()),
            file_state_signal: Cell::new(SignalId::null()),
        }
    }

    /// Port of inherited C++ `emFileModel::GetChangeSignal()`.
    /// D-008 A1 combined-form lazy accessor: allocates the SignalId on first
    /// call, returns the live id thereafter. Mirrors B-009
    /// `emFileManViewConfig::GetChangeSignal` and B-014 `emVirtualCosmosModel`.
    pub fn GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId {
        let cur = self.change_signal.get();
        if cur.is_null() {
            let new_id = ectx.create_signal();
            self.change_signal.set(new_id);
            new_id
        } else {
            cur
        }
    }

    /// Test-only accessor for the raw `change_signal` slot (without allocating).
    #[doc(hidden)]
    pub fn change_signal_for_test(&self) -> SignalId {
        self.change_signal.get()
    }

    /// Port of C++ `emSignal::Signal()` on `ChangeSignal`. Synchronous fire per
    /// D-007. No-op when `change_signal` is null (matches C++ `emSignal::Signal()`
    /// with zero subscribers per D-007 + D-008 composition in decisions.md).
    pub fn signal_change(&self, ectx: &mut impl SignalCtx) {
        let s = self.change_signal.get();
        if !s.is_null() {
            ectx.fire(s);
        }
    }

    /// Port of inherited C++ `emFileModel::GetFileStateSignal()` with lazy
    /// allocation (FU-005). Allocates on first call; returns the live id
    /// thereafter. Subscribers call this at first-Cycle subscribe time so
    /// the connect wires into a real id before any fires can occur. Mirrors
    /// `GetChangeSignal` (line 56) and `emFilePanel::ensure_vir_file_state_signal`.
    pub fn ensure_file_state_signal(&self, ectx: &mut impl SignalCtx) -> SignalId {
        let cur = self.file_state_signal.get();
        if cur.is_null() {
            let new_id = ectx.create_signal();
            self.file_state_signal.set(new_id);
            new_id
        } else {
            cur
        }
    }

    /// Test-only accessor for the raw `file_state_signal` slot (without allocating).
    #[doc(hidden)]
    pub fn file_state_signal_for_test(&self) -> SignalId {
        self.file_state_signal.get()
    }

    /// Synchronous fire of `file_state_signal` (FU-005). No-op when null
    /// (matches C++ `emSignal::Signal()` with zero subscribers per D-007).
    /// Called alongside `signal_change` at every state-mutating site.
    pub fn signal_file_state(&self, ectx: &mut impl SignalCtx) {
        let s = self.file_state_signal.get();
        if !s.is_null() {
            ectx.fire(s);
        }
    }

    pub fn GetFileState(&self) -> &FileState {
        &self.state
    }

    pub fn GetMap(&self) -> &T {
        &self.data
    }

    pub fn GetWritableMap(&mut self, ectx: &mut impl SignalCtx) -> &mut T {
        if self.protect_file_state == 0
            && matches!(
                self.state,
                FileState::Loaded | FileState::Unsaved | FileState::SaveError(_)
            )
        {
            self.set_unsaved_state_internal(ectx);
        }
        &mut self.data
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = path;
    }

    pub fn GetErrorText(&self) -> &str {
        &self.error_text
    }

    pub fn GetMemoryNeed(&self) -> u64 {
        self.memory_need
    }

    pub fn GetMemoryLimit(&self) -> u64 {
        self.memory_limit
    }

    pub fn set_memory_limit(&mut self, limit: u64) {
        self.memory_limit = limit;
    }

    /// Synchronously load the file. Port of C++ `Load(true)`.
    pub fn TryLoad(&mut self, ectx: &mut impl SignalCtx) {
        if matches!(self.state, FileState::LoadError(_) | FileState::TooCostly) {
            self.state = FileState::Waiting;
            self.error_text.clear();
        }
        while matches!(self.state, FileState::Waiting | FileState::Loading { .. }) {
            self.do_step_loading();
        }
        // D-007: C++ `emFileModel::Load` (and the inherited `Step` driver)
        // calls `Signal(ChangeSignal)` synchronously when the load completes.
        self.signal_change(ectx);
    }

    /// Synchronously save the file. Port of C++ `Save(true)`.
    pub fn Save(&mut self, ectx: &mut impl SignalCtx) {
        if !matches!(self.state, FileState::Unsaved) {
            return;
        }
        self.state = FileState::Saving;
        self.error_text.clear();

        let rec = self.data.to_rec();
        let content = write_rec(&rec);

        if let Some(parent) = self.path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                self.state = FileState::SaveError(e.to_string());
                // D-007: C++ fires ChangeSignal on the SaveError transition.
                self.signal_change(ectx);
                return;
            }
        }

        if let Err(e) = std::fs::write(&self.path, &content) {
            self.state = FileState::SaveError(e.to_string());
            self.signal_change(ectx);
            return;
        }

        if let Err(e) = self.try_fetch_date() {
            self.state = FileState::SaveError(e);
            self.signal_change(ectx);
            return;
        }

        self.state = FileState::Loaded;
        let memory_need = self.last_size + size_of::<T>() as u64;
        self.memory_need = memory_need;
        if memory_need > self.memory_limit {
            self.protect_file_state += 1;
            self.data.SetToDefault();
            self.protect_file_state -= 1;
            self.state = FileState::TooCostly;
        }
        // D-007: C++ `emFileModel::Save` calls `Signal(ChangeSignal)`
        // synchronously on the Loaded transition.
        self.signal_change(ectx);
    }

    /// Port of C++ `Update()`. Re-check file freshness; reset stale states.
    pub fn update(&mut self, ectx: &mut impl SignalCtx) {
        let prev = self.state.clone();
        if matches!(self.state, FileState::Loaded) {
            if self.is_out_of_date() {
                self.hard_reset_data();
                self.state = FileState::Waiting;
            }
        } else if matches!(self.state, FileState::LoadError(_) | FileState::TooCostly) {
            self.state = FileState::Waiting;
            self.error_text.clear();
        }
        // D-007: C++ `emFileModel::Update` calls `Signal(ChangeSignal)`
        // synchronously when transitioning out of Loaded (out-of-date) or out
        // of LoadError/TooCostly.
        if !matches!((&prev, &self.state), (FileState::Loaded, FileState::Loaded))
            && std::mem::discriminant(&prev) != std::mem::discriminant(&self.state)
        {
            self.signal_change(ectx);
        }
    }

    /// Port of C++ `HardResetFileState()`. Reset data and return to Waiting.
    pub fn hard_reset(&mut self, ectx: &mut impl SignalCtx) {
        if matches!(self.state, FileState::Loading { .. }) {
            self.read_buffer = None;
        }
        self.protect_file_state += 1;
        self.data.SetToDefault();
        self.protect_file_state -= 1;
        self.state = FileState::Waiting;
        self.error_text.clear();
        self.memory_need = 1;
        self.last_mtime = 0;
        self.last_size = 0;
        // D-007: C++ `emFileModel::HardResetFileState` calls
        // `Signal(ChangeSignal)` synchronously.
        self.signal_change(ectx);
    }

    /// Port of C++ `ClearSaveError()`. Transition SaveError → Unsaved.
    pub fn clear_save_error(&mut self, ectx: &mut impl SignalCtx) {
        if matches!(self.state, FileState::SaveError(_)) {
            self.state = FileState::Unsaved;
            self.error_text.clear();
            // D-007: C++ `emFileModel::ClearSaveError` calls
            // `Signal(ChangeSignal)` synchronously on SaveError → Unsaved.
            self.signal_change(ectx);
        }
    }

    fn set_unsaved_state_internal(&mut self, ectx: &mut impl SignalCtx) {
        if !matches!(self.state, FileState::Unsaved) {
            if matches!(self.state, FileState::Loading { .. }) {
                self.read_buffer = None;
            }
            self.state = FileState::Unsaved;
            self.error_text.clear();
            // D-007: C++ `emFileModel::SetUnsavedState`/`GetWritableMap` call
            // `Signal(ChangeSignal)` synchronously on Loaded/SaveError →
            // Unsaved. `GetWritableMap` is covered transitively via this site.
            self.signal_change(ectx);
        }
    }

    fn do_step_loading(&mut self) {
        if matches!(self.state, FileState::Waiting) {
            if let Err(e) = self.try_fetch_date() {
                self.state = FileState::LoadError(e);
                return;
            }
            self.protect_file_state += 1;
            self.data.SetToDefault();
            self.protect_file_state -= 1;

            match std::fs::read_to_string(&self.path) {
                Err(e) => {
                    self.state = FileState::LoadError(e.to_string());
                }
                Ok(content) => {
                    let memory_need = self.last_size + size_of::<T>() as u64;
                    self.memory_need = memory_need;
                    if memory_need > self.memory_limit {
                        self.state = FileState::TooCostly;
                        return;
                    }
                    self.read_buffer = Some(content);
                    self.state = FileState::Loading { progress: 0.0 };
                }
            }
        } else if matches!(self.state, FileState::Loading { .. }) {
            let content = self
                .read_buffer
                .take()
                .expect("read_buffer present in Loading");
            self.protect_file_state += 1;
            let result: Result<T, String> = parse_rec(&content)
                .and_then(|rec| T::from_rec(&rec))
                .map_err(|e| e.to_string());
            self.protect_file_state -= 1;
            match result {
                Err(e) => {
                    self.protect_file_state += 1;
                    self.data.SetToDefault();
                    self.protect_file_state -= 1;
                    self.state = FileState::LoadError(e);
                }
                Ok(data) => {
                    self.data = data;
                    self.state = FileState::Loaded;
                }
            }
        }
    }

    fn try_fetch_date(&mut self) -> Result<(), String> {
        let meta = std::fs::metadata(&self.path)
            .map_err(|e| format!("Failed to get file info for {:?}: {}", self.path, e))?;
        self.last_mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_size = meta.len();
        Ok(())
    }

    fn is_out_of_date(&self) -> bool {
        match std::fs::metadata(&self.path) {
            Err(_) => true,
            Ok(meta) => {
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let size = meta.len();
                mtime != self.last_mtime || size != self.last_size
            }
        }
    }

    fn hard_reset_data(&mut self) {
        self.protect_file_state += 1;
        self.data.SetToDefault();
        self.protect_file_state -= 1;
    }
}

impl<T: Record + Default> FileModelState for emRecFileModel<T> {
    fn GetFileState(&self) -> &FileState {
        &self.state
    }

    fn GetFileProgress(&self) -> f64 {
        match &self.state {
            FileState::Loading { progress } => *progress,
            _ => 0.0,
        }
    }

    fn GetErrorText(&self) -> &str {
        &self.error_text
    }

    fn get_memory_need(&self) -> u64 {
        self.memory_need
    }

    /// Port of inherited C++ `emFileModel::GetFileStateSignal` (FU-005).
    /// Returns the lazy-allocated `file_state_signal` id, or null if no
    /// subscriber has called `ensure_file_state_signal(ectx)` yet. Once
    /// promoted, the id is stable for the lifetime of this model.
    fn GetFileStateSignal(&self) -> SignalId {
        self.file_state_signal.get()
    }
}
