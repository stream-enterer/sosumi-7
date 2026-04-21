use std::path::PathBuf;

use emcore::emContext::emContext;
use emcore::emRecParser::RecStruct;

use emcore::emFileModel::{emFileModel, FileModelOps, FileState};

use emcore::emRecParser::RecError;

use emcore::emRecFileModel::emRecFileModel;

use emcore::emRecRecord::Record;

use emcore::emRes::ResourceCache;

use emcore::emScheduler::EngineScheduler;
use emcore::emVarModel::WatchedVar;

// ── Shared test record ──────────────────────────────────────────────────────

#[derive(Default, Clone, PartialEq, Debug)]
struct TestRecord {
    name: String,
    count: i32,
}

impl Record for TestRecord {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        Ok(Self {
            name: rec.get_str("name").unwrap_or("").to_string(),
            count: rec.get_int("count").unwrap_or(0),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut r = RecStruct::new();
        r.set_str("name", &self.name);
        r.set_int("count", self.count);
        r
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        self.name.is_empty() && self.count == 0
    }
}

fn write_test_rec(path: &std::path::Path, name: &str, count: i32) {
    let mut r = RecStruct::new();
    r.set_str("name", name);
    r.set_int("count", count);
    let content = emcore::emRecParser::write_rec(&r);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

// ── emRecFileModel tests ───────────────────────────────────────────────────────

fn make_signal() -> emcore::emSignal::SignalId {
    let mut sched = EngineScheduler::new();
    sched.create_signal()
}

#[test]
fn watched_var_fires_on_change() {
    let sig = make_signal();
    let mut var = WatchedVar::new(10, sig);

    assert!(!var.Set(10), "same value should return false");
    assert!(var.Set(20), "different value should return true");
    assert_eq!(*var.GetRec(), 20);
}

#[test]
fn resource_cache_deduplication() {
    let mut cache = ResourceCache::<String>::new();
    let a = cache.GetOrInsertWith("key", || "value".into());
    let b = cache.GetOrInsertWith("key", || "other".into());
    assert!(std::rc::Rc::ptr_eq(&a, &b));
    assert_eq!(cache.len(), 1);
}

#[test]
fn resource_cache_purge_unused() {
    let mut cache = ResourceCache::<String>::new();
    let _held = cache.GetOrInsertWith("keep", || "kept".into());
    let _dropped = cache.GetOrInsertWith("drop", || "gone".into());
    drop(_dropped);
    cache.PurgeUnused();
    assert_eq!(cache.len(), 1);
    assert!(cache.GetRec("keep").is_some());
    assert!(cache.GetRec("drop").is_none());
}

#[test]
fn context_parent_child_tree() {
    let root = emContext::NewRoot();
    assert!(root.GetParentContext().is_none());
    assert_eq!(root.child_count(), 0);

    let child = emContext::NewChild(&root);
    assert_eq!(root.child_count(), 1);
    assert!(child.GetParentContext().is_some());
    assert!(std::rc::Rc::ptr_eq(
        &child.GetParentContext().unwrap(),
        &root
    ));
}

#[test]
fn context_children_are_weak() {
    // Children stored as Weak references -- dropping the child Rc
    // should reduce the GetParentContext's child_count.
    let root = emContext::NewRoot();
    let child = emContext::NewChild(&root);
    assert_eq!(root.child_count(), 1);
    drop(child);
    // Weak ref is now dead
    assert_eq!(root.child_count(), 0);
}

#[test]
fn file_model_state_machine() {
    let sig = make_signal();
    let mut fm = emFileModel::<Vec<u8>>::new(PathBuf::from("/tmp/test"), sig, sig);

    assert_eq!(*fm.GetFileState(), FileState::Waiting);
    assert_eq!(fm.GetFileProgress(), 0.0);

    // Waiting -> Loading
    assert!(fm.Load());
    assert!(matches!(*fm.GetFileState(), FileState::Loading { .. }));

    // Loading -> LoadError
    fm.fail_load("test error".into());
    assert!(matches!(*fm.GetFileState(), FileState::LoadError(_)));

    // LoadError -> Loading (retry)
    assert!(fm.Load());
    assert!(matches!(*fm.GetFileState(), FileState::Loading { .. }));

    // Loading -> Loaded
    fm.complete_load(vec![1, 2, 3]);
    assert_eq!(*fm.GetFileState(), FileState::Loaded);
    assert_eq!(fm.GetMap().unwrap(), &vec![1, 2, 3]);
    assert_eq!(fm.GetFileProgress(), 100.0);

    // Loaded -> Unsaved
    fm.SetUnsavedState();
    assert_eq!(*fm.GetFileState(), FileState::Unsaved);

    // Unsaved -> Saving
    assert!(fm.Save());
    assert_eq!(*fm.GetFileState(), FileState::Saving);

    // Saving -> Loaded (Save complete)
    fm.complete_save();
    assert_eq!(*fm.GetFileState(), FileState::Loaded);

    // Reset
    assert!(fm.HardResetFileState());
    assert_eq!(*fm.GetFileState(), FileState::Waiting);
    assert!(fm.GetMap().is_none());
}

#[test]
fn file_model_too_costly() {
    let sig = make_signal();
    let mut fm = emFileModel::<String>::new(PathBuf::from("/tmp/test"), sig, sig);

    fm.mark_too_costly();
    assert_eq!(*fm.GetFileState(), FileState::TooCostly);

    // Can retry from TooCostly
    assert!(fm.Load());
    assert!(matches!(*fm.GetFileState(), FileState::Loading { .. }));
}

#[test]
fn rec_round_trip() {
    use emcore::emRecParser::RecError;

    let missing = RecError::MissingField("test".into());
    assert!(format!("{missing}").contains("test"));

    let invalid = RecError::InvalidValue {
        field: "count".into(),
        message: "must be positive".into(),
    };
    assert!(format!("{invalid}").contains("count"));
    assert!(format!("{invalid}").contains("must be positive"));
}

// ── emRecFileModel tests ───────────────────────────────────────────────────────

#[test]
fn rec_file_model_load_roundtrip() {
    let dir = std::env::temp_dir().join("eaglemode_rfm_1");
    let path = dir.join("test.rec");
    write_test_rec(&path, "hello", 42);

    let mut m = emRecFileModel::<TestRecord>::new(path);
    m.TryLoad();

    assert_eq!(*m.GetFileState(), FileState::Loaded);
    assert_eq!(m.GetMap().name, "hello");
    assert_eq!(m.GetMap().count, 42);
}

#[test]
fn rec_file_model_load_error_missing() {
    let path = PathBuf::from("/tmp/eaglemode_rfm_no_such_file_xyz.rec");
    let mut m = emRecFileModel::<TestRecord>::new(path);
    m.TryLoad();
    assert!(matches!(*m.GetFileState(), FileState::LoadError(_)));
}

#[test]
fn rec_file_model_load_error_bad_rec() {
    let dir = std::env::temp_dir().join("eaglemode_rfm_3");
    let path = dir.join("bad.rec");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&path, b"{{not valid rec content!!!").unwrap();

    let mut m = emRecFileModel::<TestRecord>::new(path);
    m.TryLoad();
    assert!(matches!(*m.GetFileState(), FileState::LoadError(_)));
}

#[test]
fn rec_file_model_save_roundtrip() {
    let dir = std::env::temp_dir().join("eaglemode_rfm_4");
    let path = dir.join("save.rec");
    write_test_rec(&path, "original", 1);

    let mut m = emRecFileModel::<TestRecord>::new(path.clone());
    m.TryLoad();
    assert_eq!(*m.GetFileState(), FileState::Loaded);

    m.GetWritableMap().name = "modified".to_string();
    m.GetWritableMap().count = 99;
    assert_eq!(*m.GetFileState(), FileState::Unsaved);

    m.Save();
    assert_eq!(*m.GetFileState(), FileState::Loaded);

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("modified"));
    assert!(content.contains("99"));
}

#[test]
fn rec_file_model_out_of_date() {
    let dir = std::env::temp_dir().join("eaglemode_rfm_5");
    let path = dir.join("ood.rec");
    write_test_rec(&path, "v1", 1);

    let mut m = emRecFileModel::<TestRecord>::new(path.clone());
    m.TryLoad();
    assert_eq!(*m.GetFileState(), FileState::Loaded);

    // Overwrite with significantly different size to avoid mtime collision
    let big = "x".repeat(4096);
    std::fs::write(&path, big.as_bytes()).unwrap();

    m.update();
    assert_eq!(*m.GetFileState(), FileState::Waiting);
}

#[test]
fn rec_file_model_hard_reset() {
    let dir = std::env::temp_dir().join("eaglemode_rfm_6");
    let path = dir.join("reset.rec");
    write_test_rec(&path, "data", 7);

    let mut m = emRecFileModel::<TestRecord>::new(path);
    m.TryLoad();
    assert_eq!(*m.GetFileState(), FileState::Loaded);

    m.hard_reset();

    assert_eq!(*m.GetFileState(), FileState::Waiting);
    assert!(m.GetMap().IsSetToDefault());
}

#[test]
fn rec_file_model_clear_save_error() {
    let dir = std::env::temp_dir().join("eaglemode_rfm_7");
    let path = dir.join("valid.rec");
    write_test_rec(&path, "x", 0);

    let mut m = emRecFileModel::<TestRecord>::new(path.clone());
    m.TryLoad();
    assert_eq!(*m.GetFileState(), FileState::Loaded);

    // Mark unsaved via data_mut()
    m.GetWritableMap().count = 5;
    assert_eq!(*m.GetFileState(), FileState::Unsaved);

    // Redirect to unwritable path (GetParentContext is a regular file)
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"").unwrap();
    let bad_path = blocker.join("sub.rec");
    m.set_path(bad_path);

    m.Save();
    assert!(
        matches!(*m.GetFileState(), FileState::SaveError(_)),
        "expected SaveError, got {:?}",
        m.GetFileState()
    );

    m.clear_save_error();
    assert_eq!(*m.GetFileState(), FileState::Unsaved);
}

#[test]
fn rec_file_model_memory_limit() {
    let dir = std::env::temp_dir().join("eaglemode_rfm_8");
    let path = dir.join("mem.rec");
    write_test_rec(&path, "big", 1);

    let mut m = emRecFileModel::<TestRecord>::new(path);
    m.set_memory_limit(1);
    m.TryLoad();

    assert_eq!(*m.GetFileState(), FileState::TooCostly);
}

#[test]
fn rec_file_model_protect_file_state() {
    let dir = std::env::temp_dir().join("eaglemode_rfm_9");
    let path = dir.join("protect.rec");
    write_test_rec(&path, "protected", 3);

    let mut m = emRecFileModel::<TestRecord>::new(path);
    m.TryLoad();

    // Loading internally guards data mutations with protect_file_state,
    // so the state after a clean TryLoad must be Loaded, not Unsaved.
    assert_eq!(*m.GetFileState(), FileState::Loaded);
}

// ── emFileModel<T> lifecycle tests ─────────────────────────────────────────────

struct MemOps {
    start_called: bool,
    continue_called: bool,
    quit_loading_called: bool,
    reset_called: bool,
    save_start_called: bool,
    save_continue_called: bool,
    quit_saving_called: bool,
    continue_result: Result<bool, String>,
    save_continue_result: Result<bool, String>,
}

impl MemOps {
    fn new() -> Self {
        Self {
            start_called: false,
            continue_called: false,
            quit_loading_called: false,
            reset_called: false,
            save_start_called: false,
            save_continue_called: false,
            quit_saving_called: false,
            continue_result: Ok(true),
            save_continue_result: Ok(true),
        }
    }
}

impl FileModelOps for MemOps {
    fn reset_data(&mut self) {
        self.reset_called = true;
    }
    fn try_start_loading(&mut self) -> Result<(), String> {
        self.start_called = true;
        Ok(())
    }
    fn try_continue_loading(&mut self) -> Result<bool, String> {
        self.continue_called = true;
        self.continue_result.clone()
    }
    fn quit_loading(&mut self) {
        self.quit_loading_called = true;
    }
    fn try_start_saving(&mut self) -> Result<(), String> {
        self.save_start_called = true;
        Ok(())
    }
    fn try_continue_saving(&mut self) -> Result<bool, String> {
        self.save_continue_called = true;
        self.save_continue_result.clone()
    }
    fn quit_saving(&mut self) {
        self.quit_saving_called = true;
    }
    fn calc_memory_need(&self) -> u64 {
        0
    }
    fn calc_file_progress(&self) -> f64 {
        0.0
    }
}

fn make_temp_file(subdir: &str) -> PathBuf {
    let path = std::env::temp_dir().join(subdir).join("fm.tmp");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"placeholder").unwrap();
    path
}

#[test]
fn file_model_step_loading() {
    let sig = make_signal();
    let path = make_temp_file("eaglemode_fm_10");
    let mut fm = emFileModel::<()>::new(path, sig, sig);
    let mut ops = MemOps::new();

    // First step: Waiting → Loading
    let changed = fm.step_loading(&mut ops);
    assert!(changed);
    assert!(matches!(*fm.GetFileState(), FileState::Loading { .. }));
    assert!(ops.start_called);
    assert!(!ops.continue_called);

    // Second step: Loading → Loaded (continue returns Ok(true))
    let changed = fm.step_loading(&mut ops);
    assert!(changed);
    assert_eq!(*fm.GetFileState(), FileState::Loaded);
    assert!(ops.continue_called);
    assert!(ops.quit_loading_called);
}

#[test]
fn file_model_step_saving() {
    let sig = make_signal();
    let path = make_temp_file("eaglemode_fm_11");
    let mut fm = emFileModel::<()>::new(path, sig, sig);

    // Reach Loaded state manually
    fm.complete_load(());
    assert_eq!(*fm.GetFileState(), FileState::Loaded);
    fm.SetUnsavedState();
    assert_eq!(*fm.GetFileState(), FileState::Unsaved);

    let mut ops = MemOps::new();

    // First step: Unsaved → Saving
    let changed = fm.step_saving(&mut ops);
    assert!(changed);
    assert_eq!(*fm.GetFileState(), FileState::Saving);
    assert!(ops.save_start_called);

    // Second step: Saving → Loaded (continue returns Ok(true))
    let changed = fm.step_saving(&mut ops);
    assert!(changed);
    assert_eq!(*fm.GetFileState(), FileState::Loaded);
    assert!(ops.save_continue_called);
    assert!(ops.quit_saving_called);
}

#[test]
fn file_model_hard_reset_file_state() {
    let sig = make_signal();
    let path = make_temp_file("eaglemode_fm_12");
    let mut fm = emFileModel::<()>::new(path, sig, sig);
    fm.complete_load(());
    assert_eq!(*fm.GetFileState(), FileState::Loaded);

    let mut ops = MemOps::new();
    fm.hard_reset_file_state(&mut ops);

    assert_eq!(*fm.GetFileState(), FileState::Waiting);
    assert!(ops.reset_called);
}

#[test]
fn file_model_set_unsaved_state_aborts_loading() {
    let sig = make_signal();
    let path = make_temp_file("eaglemode_fm_13");
    let mut fm = emFileModel::<()>::new(path, sig, sig);
    let mut ops = MemOps::new();

    // Step once: Waiting → Loading
    fm.step_loading(&mut ops);
    assert!(matches!(*fm.GetFileState(), FileState::Loading { .. }));

    // set_unsaved_state should abort loading and move to Unsaved
    fm.set_unsaved_state(&mut ops);
    assert_eq!(*fm.GetFileState(), FileState::Unsaved);
    assert!(ops.quit_loading_called);
}
