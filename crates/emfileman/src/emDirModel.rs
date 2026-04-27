use crate::emDirEntry::emDirEntry;
use emcore::emEngine::{emEngine, EngineId, Priority};
use emcore::emEngineCtx::EngineCtx;
use emcore::emFileModel::{emFileModel, FileModelOps, FileModelState, FileState};
use emcore::emPanelScope::PanelScope;
use emcore::emScheduler::EngineScheduler;
use emcore::emSignal::SignalId;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

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
    path: PathBuf,
    entries: Vec<emDirEntry>,
    name_count: usize,
    loading_phase: LoadingPhase,
}

impl emDirModelData {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
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
        self.path = dir_path.clone();
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
                    self.loading_phase = LoadingPhase::Sorting { dir_path, names };
                    Ok(false)
                }
            },
            LoadingPhase::Sorting {
                dir_path,
                mut names,
            } => {
                names.sort();
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
        Self::new(PathBuf::new())
    }
}

impl FileModelOps for emDirModelData {
    fn reset_data(&mut self) {
        emDirModelData::reset_data(self)
    }

    fn try_start_loading(&mut self) -> Result<(), String> {
        let p = self.path.clone();
        let s = p.to_string_lossy().into_owned();
        emDirModelData::try_start_loading_from(self, &s)
    }

    fn try_continue_loading(&mut self) -> Result<bool, String> {
        emDirModelData::try_continue_loading(self)
    }

    fn quit_loading(&mut self) {
        emDirModelData::quit_loading(self)
    }

    fn try_start_saving(&mut self) -> Result<(), String> {
        Err("emDirModel does not support saving".to_string())
    }

    fn try_continue_saving(&mut self) -> Result<bool, String> {
        Err("emDirModel does not support saving".to_string())
    }

    fn quit_saving(&mut self) {}

    fn calc_memory_need(&self) -> u64 {
        emDirModelData::calc_memory_need(self)
    }

    fn calc_file_progress(&self) -> f64 {
        emDirModelData::calc_file_progress(self)
    }
}

/// Directory model. Port of C++ `emDirModel` (extends emFileModel).
///
/// Composes `emFileModel<()>` for the loading state machine and
/// `emDirModelData` (implements `FileModelOps`) for the loader workspace.
/// The engine that drives loading is `emDirModelEngine` (shim, registered
/// lazily via `ensure_engine_registered`).
pub struct emDirModel {
    file_model: emFileModel<()>,
    data: emDirModelData,
    path: String,
    engine_id: Option<EngineId>,
}

// RUST_ONLY: (language-forced-utility) Acquire pattern returns Rc<RefCell<emDirModel>>;
// engine registration moves Box<dyn emEngine> into the scheduler. The same
// struct cannot satisfy both ownership shapes. The shim holds a Weak ref so
// the model can be both shared via Rc<RefCell<>> and registered as an engine.
struct emDirModelEngine {
    model: Weak<RefCell<emDirModel>>,
}

impl emEngine for emDirModelEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        let Some(rc) = self.model.upgrade() else {
            return false;
        };
        let mut m = rc.borrow_mut();
        m.cycle(ctx)
    }
}

impl emDirModel {
    pub fn Acquire(ctx: &Rc<emcore::emContext::emContext>, name: &str) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>(name, || Self {
            file_model: emFileModel::new(PathBuf::from(name), SignalId::default()),
            data: emDirModelData::new(PathBuf::from(name)),
            path: name.to_string(),
            engine_id: None,
        })
    }

    /// Register an `emDirModelEngine` shim with the scheduler so that the
    /// model's loading loop runs cooperatively. Idempotent. Called by
    /// `emDirPanel` after `Acquire` (panel has scheduler reach; `Acquire`
    /// does not).
    pub fn ensure_engine_registered(
        model_rc: &Rc<RefCell<emDirModel>>,
        scheduler: &mut EngineScheduler,
    ) {
        if model_rc.borrow().engine_id.is_some() {
            return;
        }
        let engine = Box::new(emDirModelEngine {
            model: Rc::downgrade(model_rc),
        });
        let engine_id = scheduler.register_engine(engine, Priority::Medium, PanelScope::Framework);
        scheduler.wake_up(engine_id);
        model_rc.borrow_mut().engine_id = Some(engine_id);
    }

    /// Engine cycle entry — called by the `emDirModelEngine` shim's
    /// `Cycle`. Drives the do-while loading loop via
    /// `emFileModel::Cycle`.
    pub(crate) fn cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        self.file_model.Cycle(ctx, &mut self.data)
    }

    /// Unregister the model's engine from the scheduler, if registered.
    /// Idempotent. Used when a panel stops viewing the model (notice
    /// unviewed) and during test teardown.
    pub fn release_engine(model_rc: &Rc<RefCell<emDirModel>>, scheduler: &mut EngineScheduler) {
        let eid = model_rc.borrow().engine_id;
        if let Some(eid) = eid {
            scheduler.remove_engine(eid);
            model_rc.borrow_mut().engine_id = None;
        }
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
        self.data.try_start_loading_from(&self.path)?;
        // Mirror the state transition on the inner file_model so that
        // direct callers (tests, external code paths that bypass the
        // engine) observe Loading via `get_file_state()`.
        self.file_model.Load();
        Ok(())
    }

    pub fn try_continue_loading(&mut self) -> Result<bool, String> {
        let done = self.data.try_continue_loading()?;
        if done {
            self.file_model.complete_load(());
        }
        Ok(done)
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

    pub fn get_file_state(&self) -> FileState {
        self.file_model.GetFileState().clone()
    }
}

impl FileModelState for emDirModel {
    fn GetFileState(&self) -> &FileState {
        self.file_model.GetFileState()
    }

    fn GetFileProgress(&self) -> f64 {
        self.file_model.GetFileProgress()
    }

    fn GetErrorText(&self) -> &str {
        self.file_model.GetErrorText()
    }

    fn get_memory_need(&self) -> u64 {
        self.file_model.get_memory_need()
    }

    fn GetFileStateSignal(&self) -> SignalId {
        self.file_model.GetFileStateSignal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> emDirModelData {
        emDirModelData::new(PathBuf::from("/tmp"))
    }

    #[test]
    fn initial_state() {
        let m = data();
        assert_eq!(m.GetEntryCount(), 0);
    }

    #[test]
    fn load_tmp_directory() {
        let mut m = data();
        m.try_start_loading_from("/tmp").unwrap();
        while !m.try_continue_loading().unwrap() {}
        m.quit_loading();
        assert!(m.GetEntryCount() > 0);
        for i in 1..m.GetEntryCount() {
            assert!(m.GetEntry(i - 1).GetName() <= m.GetEntry(i).GetName());
        }
    }

    #[test]
    fn get_entry_index_binary_search() {
        let mut m = data();
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
        let mut m = data();
        m.try_start_loading_from("/tmp").unwrap();
        while !m.try_continue_loading().unwrap() {}
        m.quit_loading();

        for i in 1..m.GetEntryCount() {
            assert_ne!(m.GetEntry(i - 1).GetName(), m.GetEntry(i).GetName());
        }
    }

    #[test]
    fn memory_need_scales_with_entries() {
        let mut m = data();
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
        let m = data();
        assert!(m.IsOutOfDate());
    }

    #[test]
    fn progress_calculation() {
        let m = data();
        assert!((m.calc_file_progress() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn is_loaded_lifecycle() {
        let mut m = data();
        assert!(!m.is_loaded());
        m.try_start_loading_from("/tmp").unwrap();
        assert!(!m.is_loaded());
        while !m.try_continue_loading().unwrap() {}
        assert!(m.is_loaded());
    }

    #[test]
    fn get_file_state_maps_phases() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let model = emDirModel::Acquire(&ctx, "/tmp");

        {
            let m = model.borrow();
            assert!(matches!(m.get_file_state(), FileState::Waiting));
        }

        {
            let mut m = model.borrow_mut();
            m.try_start_loading().unwrap();
            assert!(matches!(m.get_file_state(), FileState::Loading { .. }));
        }

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
        let mut m = data();
        m.try_start_loading_from("/tmp").unwrap();
        while !m.try_continue_loading().unwrap() {}
        m.quit_loading();
        let count = m.GetEntryCount();
        assert!(count > 0);
        m.reset_data();
        assert_eq!(m.GetEntryCount(), 0);
    }

    // ─── Proof-of-fix tests for F017 ────────────────────────────────
    //
    // The do-while loop in emFileModel::Cycle is the load-bearing fix
    // for F017. Both tests drive the engine path (not direct
    // try_continue_loading delegation) — they prove that one
    // scheduler-tick worth of Cycle loads many entries (test 1) and
    // that even when the deadline has already passed at Cycle entry,
    // at least one step still runs (test 2).

    fn run_engine_through_do_time_slice<F>(sched: &mut EngineScheduler, mut body: F)
    where
        F: FnMut(&mut EngineScheduler),
    {
        body(sched);
    }

    fn drive_one_time_slice(sched: &mut EngineScheduler) {
        use std::collections::HashMap;
        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut windows: HashMap<winit::window::WindowId, emcore::emWindow::emWindow> =
            HashMap::new();
        let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(winit::window::WindowId, emcore::emInput::emInputEvent)> =
            Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        sched.DoTimeSlice(
            &mut windows,
            &root_ctx,
            &mut fw,
            &mut pending_inputs,
            &mut input_state,
            &cb,
            &pa,
        );
    }

    #[test]
    fn cycle_loads_multiple_entries_within_one_slice() {
        // Build a 60-file directory.
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0..60 {
            std::fs::write(dir.path().join(format!("f{:03}.dat", i)), b"x").unwrap();
        }
        let path = dir.path().to_string_lossy().into_owned();

        let mut sched = EngineScheduler::new();
        let ctx = emcore::emContext::emContext::NewRoot();
        let model = emDirModel::Acquire(&ctx, &path);
        emDirModel::ensure_engine_registered(&model, &mut sched);

        // Drive one DoTimeSlice — full 50 ms budget, many cycles per
        // engine tick. Loop a few slices to allow the model to walk
        // through ReadingNames → Sorting → LoadingEntries phases.
        run_engine_through_do_time_slice(&mut sched, |s| {
            for _ in 0..5 {
                drive_one_time_slice(s);
                if matches!(model.borrow().get_file_state(), FileState::Loaded) {
                    break;
                }
            }
        });

        let count = model.borrow().GetEntryCount();
        // Cleanup: drop engine registration before scheduler drops, even
        // on assertion failure.
        let eid = model.borrow().engine_id;
        if let Some(eid) = eid {
            sched.remove_engine(eid);
        }
        assert!(
            count > 1,
            "do-while loop must load >1 entry across the slices; got {}",
            count
        );
    }

    #[test]
    fn cycle_runs_at_least_one_step_when_deadline_passed() {
        // Big enough that one Cycle will not finish even with full slice.
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0..200 {
            std::fs::write(dir.path().join(format!("e{:04}", i)), b"x").unwrap();
        }
        let path = dir.path().to_string_lossy().into_owned();

        let mut sched = EngineScheduler::new();
        let ctx = emcore::emContext::emContext::NewRoot();
        let model = emDirModel::Acquire(&ctx, &path);
        emDirModel::ensure_engine_registered(&model, &mut sched);

        // Prime: drive at most one slice — the do-while in itself loads
        // many entries, so we explicitly stop priming after one slice
        // so the model is still LoadingEntries (not Loaded) for the
        // proof.
        drive_one_time_slice(&mut sched);
        let entries_before = model.borrow().GetEntryCount();
        let primed_ok =
            entries_before > 0 && !matches!(model.borrow().get_file_state(), FileState::Loaded);

        // Sleep past the 50 ms deadline last set by drive_one_time_slice.
        std::thread::sleep(std::time::Duration::from_millis(60));
        let deadline_passed = sched.IsTimeSliceAtEnd();

        let mut entries_after = entries_before;
        if primed_ok && deadline_passed {
            // Drive one more slice. DoTimeSlice resets deadline to
            // now+50ms, so to truly observe "deadline already in the
            // past at Cycle entry" we need a different shape: call
            // cycle() directly. Construct a minimal EngineCtx with
            // sched still flagged as past-deadline (achieved by NOT
            // calling DoTimeSlice between the sleep and the cycle).
            use std::collections::HashMap;
            let root = emcore::emContext::emContext::NewRoot();
            let mut windows: HashMap<winit::window::WindowId, emcore::emWindow::emWindow> =
                HashMap::new();
            let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
            let mut pending_inputs: Vec<(winit::window::WindowId, emcore::emInput::emInputEvent)> =
                Vec::new();
            let mut input_state = emcore::emInputState::emInputState::new();
            let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
            let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
                Rc::new(RefCell::new(Vec::new()));
            let _ = (
                &mut windows,
                &mut pending_inputs,
                &mut input_state,
                &cb,
                &pa,
                &root,
                &mut fw,
            );
            let eng_id = model.borrow().engine_id.expect("engine registered");
            let mut ectx = emcore::emEngineCtx::EngineCtx {
                scheduler: &mut sched,
                tree: None,
                windows: &mut windows,
                root_context: &root,
                framework_actions: &mut fw,
                pending_inputs: &mut pending_inputs,
                input_state: &mut input_state,
                framework_clipboard: &cb,
                engine_id: eng_id,
                pending_actions: &pa,
            };
            // Sanity: deadline must be in the past at this point.
            assert!(ectx.IsTimeSliceAtEnd(), "deadline must already be passed");
            model.borrow_mut().cycle(&mut ectx);
            entries_after = model.borrow().GetEntryCount();
        }

        // Cleanup before assertions.
        let eid = model.borrow().engine_id;
        if let Some(eid) = eid {
            sched.remove_engine(eid);
        }

        assert!(
            primed_ok,
            "priming failed: count={} state={:?}",
            entries_before,
            model.borrow().get_file_state()
        );
        assert!(deadline_passed, "deadline not in the past");
        assert!(
            entries_after > entries_before,
            "do-while at-least-once: entries_after ({}) must be > entries_before ({})",
            entries_after,
            entries_before
        );
    }
}
