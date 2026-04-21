use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::emRecParser::{parse_rec, write_rec};

use crate::emFileModel::FileState;
use crate::emRecRecord::Record;

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
        }
    }

    pub fn GetFileState(&self) -> &FileState {
        &self.state
    }

    pub fn GetMap(&self) -> &T {
        &self.data
    }

    pub fn GetWritableMap(&mut self) -> &mut T {
        if self.protect_file_state == 0
            && matches!(
                self.state,
                FileState::Loaded | FileState::Unsaved | FileState::SaveError(_)
            )
        {
            self.set_unsaved_state_internal();
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
    pub fn TryLoad(&mut self) {
        if matches!(self.state, FileState::LoadError(_) | FileState::TooCostly) {
            self.state = FileState::Waiting;
            self.error_text.clear();
        }
        while matches!(self.state, FileState::Waiting | FileState::Loading { .. }) {
            self.do_step_loading();
        }
    }

    /// Synchronously save the file. Port of C++ `Save(true)`.
    pub fn Save(&mut self) {
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
                return;
            }
        }

        if let Err(e) = std::fs::write(&self.path, &content) {
            self.state = FileState::SaveError(e.to_string());
            return;
        }

        if let Err(e) = self.try_fetch_date() {
            self.state = FileState::SaveError(e);
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
    }

    /// Port of C++ `Update()`. Re-check file freshness; reset stale states.
    pub fn update(&mut self) {
        if matches!(self.state, FileState::Loaded) {
            if self.is_out_of_date() {
                self.hard_reset_data();
                self.state = FileState::Waiting;
            }
        } else if matches!(self.state, FileState::LoadError(_) | FileState::TooCostly) {
            self.state = FileState::Waiting;
            self.error_text.clear();
        }
    }

    /// Port of C++ `HardResetFileState()`. Reset data and return to Waiting.
    pub fn hard_reset(&mut self) {
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
    }

    /// Port of C++ `ClearSaveError()`. Transition SaveError → Unsaved.
    pub fn clear_save_error(&mut self) {
        if matches!(self.state, FileState::SaveError(_)) {
            self.state = FileState::Unsaved;
            self.error_text.clear();
        }
    }

    fn set_unsaved_state_internal(&mut self) {
        if !matches!(self.state, FileState::Unsaved) {
            if matches!(self.state, FileState::Loading { .. }) {
                self.read_buffer = None;
            }
            self.state = FileState::Unsaved;
            self.error_text.clear();
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
