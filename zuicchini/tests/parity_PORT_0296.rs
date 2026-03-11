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
fn reset_data_clears_everything() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.request_load();
    m.complete_load("data".to_string());
    m.update_memory_need(500);
    m.reset_data();
    assert!(matches!(m.state(), &FileState::Waiting));
    assert!(m.data().is_none());
    assert_eq!(m.get_memory_need(), 0);
}
