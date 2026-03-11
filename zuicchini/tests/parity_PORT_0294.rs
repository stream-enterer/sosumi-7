use std::path::PathBuf;

use zuicchini::model::FileModel;
use zuicchini::scheduler::EngineScheduler;

fn make_signals() -> (
    zuicchini::scheduler::SignalId,
    zuicchini::scheduler::SignalId,
) {
    let mut sched = EngineScheduler::new();
    (sched.create_signal(), sched.create_signal())
}

#[test]
fn get_memory_need_default_zero() {
    let (change, update) = make_signals();
    let m: FileModel<Vec<u8>> = FileModel::new(PathBuf::from("test.dat"), change, update);
    assert_eq!(m.get_memory_need(), 0);
}

#[test]
fn get_memory_need_after_update() {
    let (change, update) = make_signals();
    let mut m: FileModel<Vec<u8>> = FileModel::new(PathBuf::from("test.dat"), change, update);
    m.update_memory_need(1024);
    assert_eq!(m.get_memory_need(), 1024);
}
