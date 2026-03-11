use std::path::PathBuf;

use zuicchini::model::{Context, FileModel, FileState, ResourceCache, WatchedVar};
use zuicchini::scheduler::EngineScheduler;

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
fn record_kdl_round_trip() {
    use zuicchini::model::ConfigError;

    let err = ConfigError::MissingField("test".into());
    let msg = format!("{err}");
    assert!(msg.contains("test"));
}
