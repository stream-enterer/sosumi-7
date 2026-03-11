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
fn update_retries_load_error() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.request_load();
    m.fail_load("network error".to_string());
    assert!(matches!(m.state(), FileState::LoadError(_)));
    m.update();
    assert!(matches!(m.state(), &FileState::Waiting));
}

#[test]
fn update_retries_too_costly() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.mark_too_costly();
    m.update();
    assert!(matches!(m.state(), &FileState::Waiting));
}

#[test]
fn update_unloads_out_of_date() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.request_load();
    m.complete_load("data".to_string());
    m.set_file_date(1000, 512);
    m.check_out_of_date(2000, 512); // marks out_of_date = true
    m.update();
    assert!(matches!(m.state(), &FileState::Waiting));
    assert!(m.data().is_none());
}

#[test]
fn update_keeps_loaded_if_fresh() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.request_load();
    m.complete_load("data".to_string());
    m.set_file_date(1000, 512);
    m.check_out_of_date(1000, 512); // same date, not out of date
    m.update();
    assert!(matches!(m.state(), &FileState::Loaded));
    assert!(m.data().is_some());
}
