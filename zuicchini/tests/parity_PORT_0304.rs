use std::path::PathBuf;

use zuicchini::model::{FileModel, FileState};
use zuicchini::scheduler::EngineScheduler;

fn make_signals() -> (
    zuicchini::scheduler::SignalId,
    zuicchini::scheduler::SignalId,
) {
    let mut sched = EngineScheduler::new();
    (sched.create_signal(), sched.create_signal())
}

#[test]
fn clear_save_error_transitions_to_unsaved() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.request_load();
    m.complete_load("data".to_string());
    m.mark_unsaved();
    m.request_save();
    m.fail_save("disk full".to_string());
    assert!(matches!(m.state(), FileState::SaveError(_)));
    m.clear_save_error();
    assert!(matches!(m.state(), &FileState::Unsaved));
}

#[test]
fn clear_save_error_noop_in_waiting() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.clear_save_error();
    assert!(matches!(m.state(), &FileState::Waiting));
}

#[test]
fn clear_save_error_noop_in_loaded() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.request_load();
    m.complete_load("data".to_string());
    m.clear_save_error();
    assert!(matches!(m.state(), &FileState::Loaded));
}
