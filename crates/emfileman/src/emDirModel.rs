use crate::emDirEntry::emDirEntry;
use std::path::PathBuf;

enum LoadingPhase {
    Idle,
    ReadingNames {
        dir_iter: std::fs::ReadDir,
        dir_path: PathBuf,
        names: Vec<String>,
    },
    Sorting {
        dir_path: PathBuf,
        names: Vec<String>,
    },
    LoadingEntries {
        dir_path: PathBuf,
        names: Vec<String>,
        next_idx: usize,
    },
    Done,
}

pub struct emDirModelData {
    entries: Vec<emDirEntry>,
    name_count: usize,
    loading_phase: LoadingPhase,
}

impl emDirModelData {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            name_count: 0,
            loading_phase: LoadingPhase::Idle,
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self.loading_phase, LoadingPhase::Done)
    }

    pub fn try_start_loading_from(&mut self, path: &str) -> Result<(), String> {
        let dir_path = PathBuf::from(path);
        let dir_iter = std::fs::read_dir(&dir_path)
            .map_err(|e| format!("Failed to open directory {:?}: {}", dir_path, e))?;
        self.loading_phase = LoadingPhase::ReadingNames {
            dir_iter,
            dir_path,
            names: Vec::new(),
        };
        self.name_count = 0;
        self.entries.clear();
        Ok(())
    }

    pub fn try_continue_loading(&mut self) -> Result<bool, String> {
        let phase = std::mem::replace(&mut self.loading_phase, LoadingPhase::Idle);
        match phase {
            LoadingPhase::ReadingNames {
                mut dir_iter,
                dir_path,
                mut names,
            } => match dir_iter.next() {
                Some(Ok(entry)) => {
                    if let Some(name) = entry.file_name().to_str() {
                        names.push(name.to_string());
                    }
                    self.loading_phase = LoadingPhase::ReadingNames {
                        dir_iter,
                        dir_path,
                        names,
                    };
                    Ok(false)
                }
                Some(Err(e)) => {
                    self.loading_phase = LoadingPhase::ReadingNames {
                        dir_iter,
                        dir_path,
                        names,
                    };
                    Err(format!("Error reading directory entry: {}", e))
                }
                None => {
                    // Done reading names, transition to sorting
                    self.loading_phase = LoadingPhase::Sorting { dir_path, names };
                    Ok(false)
                }
            },
            LoadingPhase::Sorting {
                dir_path,
                mut names,
            } => {
                // Sort names
                names.sort();
                // Remove duplicates
                names.dedup();
                self.name_count = names.len();
                if names.is_empty() {
                    self.loading_phase = LoadingPhase::Done;
                } else {
                    self.loading_phase = LoadingPhase::LoadingEntries {
                        dir_path,
                        names,
                        next_idx: 0,
                    };
                }
                Ok(false)
            }
            LoadingPhase::LoadingEntries {
                dir_path,
                names,
                next_idx,
            } => {
                if next_idx < names.len() {
                    let dir_str = dir_path.to_str().unwrap_or("");
                    let entry = emDirEntry::from_parent_and_name(dir_str, &names[next_idx]);
                    self.entries.push(entry);
                    self.loading_phase = LoadingPhase::LoadingEntries {
                        dir_path,
                        names,
                        next_idx: next_idx + 1,
                    };
                    Ok(false)
                } else {
                    self.loading_phase = LoadingPhase::Done;
                    Ok(true)
                }
            }
            LoadingPhase::Done => {
                self.loading_phase = LoadingPhase::Done;
                Ok(true)
            }
            LoadingPhase::Idle => {
                self.loading_phase = LoadingPhase::Idle;
                Ok(true)
            }
        }
    }

    pub fn quit_loading(&mut self) {
        self.loading_phase = LoadingPhase::Done;
    }

    pub fn reset_data(&mut self) {
        self.entries.clear();
        self.name_count = 0;
        self.loading_phase = LoadingPhase::Idle;
    }

    pub fn calc_memory_need(&self) -> u64 {
        self.name_count as u64 * 8192
    }

    pub fn calc_file_progress(&self) -> f64 {
        match &self.loading_phase {
            LoadingPhase::ReadingNames { names, .. } => {
                let nc = names.len() as f64;
                20.0 * (1.0 - 10.0 / (10.0 + nc.sqrt()))
            }
            LoadingPhase::Sorting { .. }
            | LoadingPhase::LoadingEntries { .. }
            | LoadingPhase::Done => {
                if self.name_count > 0 {
                    20.0 + 80.0 * self.entries.len() as f64 / self.name_count as f64
                } else {
                    100.0
                }
            }
            LoadingPhase::Idle => 100.0,
        }
    }

    pub fn IsOutOfDate(&self) -> bool {
        true
    }

    pub fn GetEntryCount(&self) -> usize {
        self.entries.len()
    }

    pub fn GetEntry(&self, index: usize) -> &emDirEntry {
        &self.entries[index]
    }

    pub fn GetEntryIndex(&self, name: &str) -> Option<usize> {
        self.entries
            .binary_search_by(|e| e.GetName().cmp(name))
            .ok()
    }

    pub fn name_count(&self) -> usize {
        self.name_count
    }
}

impl Default for emDirModelData {
    fn default() -> Self {
        Self::new()
    }
}

/// Directory model wrapper.
/// Port of C++ `emDirModel` (extends emFileModel).
///
/// DIVERGED: (language-forced) Does not compose emFileModel<T> because emFileModel requires
/// SignalId and update_signal from the scheduler, which are not needed for
/// the data-layer-only port. Wraps emDirModelData directly. The panel layer
/// drives the loading state machine by calling these methods in its Cycle.
/// Does not implement the FileModelState trait (which returns `&FileState`),
/// but provides `get_file_state()` returning an owned FileState and
/// `is_loaded()` for convenience.
pub struct emDirModel {
    data: emDirModelData,
    path: String,
}

impl emDirModel {
    pub fn Acquire(
        ctx: &std::rc::Rc<emcore::emContext::emContext>,
        name: &str,
    ) -> std::rc::Rc<std::cell::RefCell<Self>> {
        ctx.acquire::<Self>(name, || Self {
            data: emDirModelData::new(),
            path: name.to_string(),
        })
    }

    pub fn GetEntryCount(&self) -> usize {
        self.data.GetEntryCount()
    }

    pub fn GetEntry(&self, index: usize) -> &emDirEntry {
        self.data.GetEntry(index)
    }

    pub fn GetEntryIndex(&self, name: &str) -> Option<usize> {
        self.data.GetEntryIndex(name)
    }

    pub fn IsOutOfDate(&self) -> bool {
        self.data.IsOutOfDate()
    }

    pub fn name_count(&self) -> usize {
        self.data.name_count()
    }

    pub fn GetFilePath(&self) -> &str {
        &self.path
    }

    pub fn reset_data(&mut self) {
        self.data.reset_data();
    }

    pub fn try_start_loading(&mut self) -> Result<(), String> {
        self.data.try_start_loading_from(&self.path)
    }

    pub fn try_continue_loading(&mut self) -> Result<bool, String> {
        self.data.try_continue_loading()
    }

    pub fn quit_loading(&mut self) {
        self.data.quit_loading();
    }

    pub fn calc_memory_need(&self) -> u64 {
        self.data.calc_memory_need()
    }

    pub fn calc_file_progress(&self) -> f64 {
        self.data.calc_file_progress()
    }

    pub fn is_loaded(&self) -> bool {
        self.data.is_loaded()
    }

    pub fn get_file_state(&self) -> emcore::emFileModel::FileState {
        match &self.data.loading_phase {
            LoadingPhase::Idle => emcore::emFileModel::FileState::Waiting,
            LoadingPhase::ReadingNames { .. }
            | LoadingPhase::Sorting { .. }
            | LoadingPhase::LoadingEntries { .. } => emcore::emFileModel::FileState::Loading {
                progress: self.data.calc_file_progress(),
            },
            LoadingPhase::Done => emcore::emFileModel::FileState::Loaded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn initial_state() {
        let m = emDirModelData::new();
        assert_eq!(m.GetEntryCount(), 0);
    }

    #[test]
    fn load_tmp_directory() {
        let mut m = emDirModelData::new();
        m.try_start_loading_from("/tmp").unwrap();
        while !m.try_continue_loading().unwrap() {}
        m.quit_loading();
        assert!(m.GetEntryCount() > 0);
        // Entries are sorted by name
        for i in 1..m.GetEntryCount() {
            assert!(m.GetEntry(i - 1).GetName() <= m.GetEntry(i).GetName());
        }
    }

    #[test]
    fn get_entry_index_binary_search() {
        let mut m = emDirModelData::new();
        m.try_start_loading_from("/tmp").unwrap();
        while !m.try_continue_loading().unwrap() {}
        m.quit_loading();

        if m.GetEntryCount() > 0 {
            let name = m.GetEntry(0).GetName().to_string();
            assert_eq!(m.GetEntryIndex(&name), Some(0));
        }
        assert_eq!(m.GetEntryIndex("__nonexistent_emfileman__"), None);
    }

    #[test]
    fn deduplication() {
        let mut m = emDirModelData::new();
        m.try_start_loading_from("/tmp").unwrap();
        while !m.try_continue_loading().unwrap() {}
        m.quit_loading();

        for i in 1..m.GetEntryCount() {
            assert_ne!(m.GetEntry(i - 1).GetName(), m.GetEntry(i).GetName());
        }
    }

    #[test]
    fn memory_need_scales_with_entries() {
        let mut m = emDirModelData::new();
        m.try_start_loading_from("/tmp").unwrap();
        while !m.try_continue_loading().unwrap() {}
        m.quit_loading();
        assert_eq!(m.calc_memory_need(), m.name_count() as u64 * 8192);
    }

    #[test]
    fn model_acquire_same_path_returns_same() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let m1 = emDirModel::Acquire(&ctx, "/tmp");
        let m2 = emDirModel::Acquire(&ctx, "/tmp");
        assert!(Rc::ptr_eq(&m1, &m2));
    }

    #[test]
    fn model_delegates_entry_accessors() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let model = emDirModel::Acquire(&ctx, "/tmp");
        let model = model.borrow();
        assert_eq!(model.GetEntryCount(), 0);
        assert!(model.GetEntryIndex("anything").is_none());
    }

    #[test]
    fn model_file_model_ops_wiring() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let model = emDirModel::Acquire(&ctx, "/tmp");
        let mut model = model.borrow_mut();
        model.reset_data();
        assert_eq!(model.GetEntryCount(), 0);
    }

    #[test]
    fn is_out_of_date_always_true() {
        let m = emDirModelData::new();
        assert!(m.IsOutOfDate());
    }

    #[test]
    fn progress_calculation() {
        let m = emDirModelData::new();
        assert!((m.calc_file_progress() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn is_loaded_lifecycle() {
        let mut m = emDirModelData::new();
        assert!(!m.is_loaded());
        m.try_start_loading_from("/tmp").unwrap();
        assert!(!m.is_loaded());
        while !m.try_continue_loading().unwrap() {}
        // Done but not quit yet — still in Done phase
        assert!(m.is_loaded());
    }

    #[test]
    fn get_file_state_maps_phases() {
        use emcore::emFileModel::FileState;
        let ctx = emcore::emContext::emContext::NewRoot();
        let model = emDirModel::Acquire(&ctx, "/tmp");

        // Initially Idle → Waiting
        {
            let m = model.borrow();
            assert!(matches!(m.get_file_state(), FileState::Waiting));
        }

        // Start loading → Loading
        {
            let mut m = model.borrow_mut();
            m.try_start_loading().unwrap();
            assert!(matches!(m.get_file_state(), FileState::Loading { .. }));
        }

        // Continue until done → Loaded
        loop {
            let mut m = model.borrow_mut();
            if m.try_continue_loading().unwrap() {
                break;
            }
        }
        {
            let m = model.borrow();
            assert!(matches!(m.get_file_state(), FileState::Loaded));
            assert!(m.is_loaded());
        }
    }

    #[test]
    fn reset_data_clears_entries() {
        let mut m = emDirModelData::new();
        m.try_start_loading_from("/tmp").unwrap();
        while !m.try_continue_loading().unwrap() {}
        m.quit_loading();
        let count = m.GetEntryCount();
        assert!(count > 0);
        m.reset_data();
        assert_eq!(m.GetEntryCount(), 0);
    }
}
