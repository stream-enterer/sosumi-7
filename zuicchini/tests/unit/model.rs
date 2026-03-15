use std::path::PathBuf;

use zuicchini::foundation::RecStruct;
use zuicchini::model::{
    Context, FileModel, FileModelOps, FileState, RecError, RecFileModel, Record, ResourceCache,
    WatchedVar,
};
use zuicchini::scheduler::EngineScheduler;

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

    fn set_to_default(&mut self) {
        *self = Self::default();
    }

    fn is_default(&self) -> bool {
        self.name.is_empty() && self.count == 0
    }
}

fn write_test_rec(path: &std::path::Path, name: &str, count: i32) {
    let mut r = RecStruct::new();
    r.set_str("name", name);
    r.set_int("count", count);
    let content = zuicchini::foundation::write_rec(&r);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

// ── RecFileModel tests ───────────────────────────────────────────────────────

fn make_signal() -> zuicchini::scheduler::SignalId {
    let mut sched = EngineScheduler::new();
    sched.create_signal()
}

#[test]
fn watched_var_fires_on_change() {
    let sig = make_signal();
    let mut var = WatchedVar::new(10, sig);

    assert!(!var.set(10), "same value should return false");
    assert!(var.set(20), "different value should return true");
    assert_eq!(*var.get(), 20);
}

#[test]
fn resource_cache_deduplication() {
    let mut cache = ResourceCache::<String>::new();
    let a = cache.get_or_insert_with("key", || "value".into());
    let b = cache.get_or_insert_with("key", || "other".into());
    assert!(std::rc::Rc::ptr_eq(&a, &b));
    assert_eq!(cache.len(), 1);
}

#[test]
fn resource_cache_purge_unused() {
    let mut cache = ResourceCache::<String>::new();
    let _held = cache.get_or_insert_with("keep", || "kept".into());
    let _dropped = cache.get_or_insert_with("drop", || "gone".into());
    drop(_dropped);
    cache.purge_unused();
    assert_eq!(cache.len(), 1);
    assert!(cache.get("keep").is_some());
    assert!(cache.get("drop").is_none());
}

#[test]
fn context_parent_child_tree() {
    let root = Context::new_root();
    assert!(root.parent().is_none());
    assert_eq!(root.child_count(), 0);

    let child = Context::new_child(&root);
    assert_eq!(root.child_count(), 1);
    assert!(child.parent().is_some());
    assert!(std::rc::Rc::ptr_eq(&child.parent().unwrap(), &root));
}

#[test]
fn context_children_are_weak() {
    // Children stored as Weak references -- dropping the child Rc
    // should reduce the parent's child_count.
    let root = Context::new_root();
    let child = Context::new_child(&root);
    assert_eq!(root.child_count(), 1);
    drop(child);
    // Weak ref is now dead
    assert_eq!(root.child_count(), 0);
}

#[test]
fn file_model_state_machine() {
    let sig = make_signal();
    let mut fm = FileModel::<Vec<u8>>::new(PathBuf::from("/tmp/test"), sig, sig);

    assert_eq!(*fm.state(), FileState::Waiting);
    assert_eq!(fm.progress(), 0.0);

    // Waiting -> Loading
    assert!(fm.request_load());
    assert!(matches!(*fm.state(), FileState::Loading { .. }));

    // Loading -> LoadError
    fm.fail_load("test error".into());
    assert!(matches!(*fm.state(), FileState::LoadError(_)));

    // LoadError -> Loading (retry)
    assert!(fm.request_load());
    assert!(matches!(*fm.state(), FileState::Loading { .. }));

    // Loading -> Loaded
    fm.complete_load(vec![1, 2, 3]);
    assert_eq!(*fm.state(), FileState::Loaded);
    assert_eq!(fm.data().unwrap(), &vec![1, 2, 3]);
    assert_eq!(fm.progress(), 100.0);

    // Loaded -> Unsaved
    fm.mark_unsaved();
    assert_eq!(*fm.state(), FileState::Unsaved);

    // Unsaved -> Saving
    assert!(fm.request_save());
    assert_eq!(*fm.state(), FileState::Saving);

    // Saving -> Loaded (save complete)
    fm.complete_save();
    assert_eq!(*fm.state(), FileState::Loaded);

    // Reset
    assert!(fm.reset());
    assert_eq!(*fm.state(), FileState::Waiting);
    assert!(fm.data().is_none());
}

#[test]
fn file_model_too_costly() {
    let sig = make_signal();
    let mut fm = FileModel::<String>::new(PathBuf::from("/tmp/test"), sig, sig);

    fm.mark_too_costly();
    assert_eq!(*fm.state(), FileState::TooCostly);

    // Can retry from TooCostly
    assert!(fm.request_load());
    assert!(matches!(*fm.state(), FileState::Loading { .. }));
}

#[test]
fn rec_round_trip() {
    use zuicchini::model::RecError;

    let missing = RecError::MissingField("test".into());
    assert!(format!("{missing}").contains("test"));

    let invalid = RecError::InvalidValue {
        field: "count".into(),
        message: "must be positive".into(),
    };
    assert!(format!("{invalid}").contains("count"));
    assert!(format!("{invalid}").contains("must be positive"));
}

// ── RecFileModel tests ───────────────────────────────────────────────────────

#[test]
fn rec_file_model_load_roundtrip() {
    let dir = std::env::temp_dir().join("zuicchini_rfm_1");
    let path = dir.join("test.rec");
    write_test_rec(&path, "hello", 42);

    let mut m = RecFileModel::<TestRecord>::new(path);
    m.load();

    assert_eq!(*m.state(), FileState::Loaded);
    assert_eq!(m.data().name, "hello");
    assert_eq!(m.data().count, 42);
}

#[test]
fn rec_file_model_load_error_missing() {
    let path = PathBuf::from("/tmp/zuicchini_rfm_no_such_file_xyz.rec");
    let mut m = RecFileModel::<TestRecord>::new(path);
    m.load();
    assert!(matches!(*m.state(), FileState::LoadError(_)));
}

#[test]
fn rec_file_model_load_error_bad_rec() {
    let dir = std::env::temp_dir().join("zuicchini_rfm_3");
    let path = dir.join("bad.rec");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&path, b"{{not valid rec content!!!").unwrap();

    let mut m = RecFileModel::<TestRecord>::new(path);
    m.load();
    assert!(matches!(*m.state(), FileState::LoadError(_)));
}

#[test]
fn rec_file_model_save_roundtrip() {
    let dir = std::env::temp_dir().join("zuicchini_rfm_4");
    let path = dir.join("save.rec");
    write_test_rec(&path, "original", 1);

    let mut m = RecFileModel::<TestRecord>::new(path.clone());
    m.load();
    assert_eq!(*m.state(), FileState::Loaded);

    m.data_mut().name = "modified".to_string();
    m.data_mut().count = 99;
    assert_eq!(*m.state(), FileState::Unsaved);

    m.save();
    assert_eq!(*m.state(), FileState::Loaded);

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("modified"));
    assert!(content.contains("99"));
}

#[test]
fn rec_file_model_out_of_date() {
    let dir = std::env::temp_dir().join("zuicchini_rfm_5");
    let path = dir.join("ood.rec");
    write_test_rec(&path, "v1", 1);

    let mut m = RecFileModel::<TestRecord>::new(path.clone());
    m.load();
    assert_eq!(*m.state(), FileState::Loaded);

    // Overwrite with significantly different size to avoid mtime collision
    let big = "x".repeat(4096);
    std::fs::write(&path, big.as_bytes()).unwrap();

    m.update();
    assert_eq!(*m.state(), FileState::Waiting);
}

#[test]
fn rec_file_model_hard_reset() {
    let dir = std::env::temp_dir().join("zuicchini_rfm_6");
    let path = dir.join("reset.rec");
    write_test_rec(&path, "data", 7);

    let mut m = RecFileModel::<TestRecord>::new(path);
    m.load();
    assert_eq!(*m.state(), FileState::Loaded);

    m.hard_reset();

    assert_eq!(*m.state(), FileState::Waiting);
    assert!(m.data().is_default());
}

#[test]
fn rec_file_model_clear_save_error() {
    let dir = std::env::temp_dir().join("zuicchini_rfm_7");
    let path = dir.join("valid.rec");
    write_test_rec(&path, "x", 0);

    let mut m = RecFileModel::<TestRecord>::new(path.clone());
    m.load();
    assert_eq!(*m.state(), FileState::Loaded);

    // Mark unsaved via data_mut()
    m.data_mut().count = 5;
    assert_eq!(*m.state(), FileState::Unsaved);

    // Redirect to unwritable path (parent is a regular file)
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"").unwrap();
    let bad_path = blocker.join("sub.rec");
    m.set_path(bad_path);

    m.save();
    assert!(
        matches!(*m.state(), FileState::SaveError(_)),
        "expected SaveError, got {:?}",
        m.state()
    );

    m.clear_save_error();
    assert_eq!(*m.state(), FileState::Unsaved);
}

#[test]
fn rec_file_model_memory_limit() {
    let dir = std::env::temp_dir().join("zuicchini_rfm_8");
    let path = dir.join("mem.rec");
    write_test_rec(&path, "big", 1);

    let mut m = RecFileModel::<TestRecord>::new(path);
    m.set_memory_limit(1);
    m.load();

    assert_eq!(*m.state(), FileState::TooCostly);
}

#[test]
fn rec_file_model_protect_file_state() {
    let dir = std::env::temp_dir().join("zuicchini_rfm_9");
    let path = dir.join("protect.rec");
    write_test_rec(&path, "protected", 3);

    let mut m = RecFileModel::<TestRecord>::new(path);
    m.load();

    // Loading internally guards data mutations with protect_file_state,
    // so the state after a clean load must be Loaded, not Unsaved.
    assert_eq!(*m.state(), FileState::Loaded);
}

// ── FileModel<T> lifecycle tests ─────────────────────────────────────────────

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
    let path = make_temp_file("zuicchini_fm_10");
    let mut fm = FileModel::<()>::new(path, sig, sig);
    let mut ops = MemOps::new();

    // First step: Waiting → Loading
    let changed = fm.step_loading(&mut ops);
    assert!(changed);
    assert!(matches!(*fm.state(), FileState::Loading { .. }));
    assert!(ops.start_called);
    assert!(!ops.continue_called);

    // Second step: Loading → Loaded (continue returns Ok(true))
    let changed = fm.step_loading(&mut ops);
    assert!(changed);
    assert_eq!(*fm.state(), FileState::Loaded);
    assert!(ops.continue_called);
    assert!(ops.quit_loading_called);
}

#[test]
fn file_model_step_saving() {
    let sig = make_signal();
    let path = make_temp_file("zuicchini_fm_11");
    let mut fm = FileModel::<()>::new(path, sig, sig);

    // Reach Loaded state manually
    fm.complete_load(());
    assert_eq!(*fm.state(), FileState::Loaded);
    fm.mark_unsaved();
    assert_eq!(*fm.state(), FileState::Unsaved);

    let mut ops = MemOps::new();

    // First step: Unsaved → Saving
    let changed = fm.step_saving(&mut ops);
    assert!(changed);
    assert_eq!(*fm.state(), FileState::Saving);
    assert!(ops.save_start_called);

    // Second step: Saving → Loaded (continue returns Ok(true))
    let changed = fm.step_saving(&mut ops);
    assert!(changed);
    assert_eq!(*fm.state(), FileState::Loaded);
    assert!(ops.save_continue_called);
    assert!(ops.quit_saving_called);
}

#[test]
fn file_model_hard_reset_file_state() {
    let sig = make_signal();
    let path = make_temp_file("zuicchini_fm_12");
    let mut fm = FileModel::<()>::new(path, sig, sig);
    fm.complete_load(());
    assert_eq!(*fm.state(), FileState::Loaded);

    let mut ops = MemOps::new();
    fm.hard_reset_file_state(&mut ops);

    assert_eq!(*fm.state(), FileState::Waiting);
    assert!(ops.reset_called);
}

#[test]
fn file_model_set_unsaved_state_aborts_loading() {
    let sig = make_signal();
    let path = make_temp_file("zuicchini_fm_13");
    let mut fm = FileModel::<()>::new(path, sig, sig);
    let mut ops = MemOps::new();

    // Step once: Waiting → Loading
    fm.step_loading(&mut ops);
    assert!(matches!(*fm.state(), FileState::Loading { .. }));

    // set_unsaved_state should abort loading and move to Unsaved
    fm.set_unsaved_state(&mut ops);
    assert_eq!(*fm.state(), FileState::Unsaved);
    assert!(ops.quit_loading_called);
}
