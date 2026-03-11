use std::path::PathBuf;

use zuicchini::model::FileModel;
use zuicchini::scheduler::EngineScheduler;

#[test]
fn update_signal_returned() {
    let mut sched = EngineScheduler::new();
    let change = sched.create_signal();
    let update = sched.create_signal();
    let m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    assert_eq!(m.update_signal(), update);
}

#[test]
fn update_signal_differs_from_change() {
    let mut sched = EngineScheduler::new();
    let change = sched.create_signal();
    let update = sched.create_signal();
    let m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    assert_ne!(m.change_signal(), m.update_signal());
}
