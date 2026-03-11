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
fn ignore_update_signal_default_false() {
    let (change, update) = make_signals();
    let m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    assert!(!m.ignore_update_signal());
}

#[test]
fn set_ignore_update_signal_true() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.set_ignore_update_signal(true);
    assert!(m.ignore_update_signal());
}

#[test]
fn set_ignore_update_signal_toggle() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.set_ignore_update_signal(true);
    assert!(m.ignore_update_signal());
    m.set_ignore_update_signal(false);
    assert!(!m.ignore_update_signal());
}
