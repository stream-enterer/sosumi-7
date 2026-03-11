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
fn file_date_same_is_not_out_of_date() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.set_file_date(1000, 512);
    assert!(!m.check_out_of_date(1000, 512));
}

#[test]
fn file_date_different_mtime_is_out_of_date() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.set_file_date(1000, 512);
    assert!(m.check_out_of_date(2000, 512));
}

#[test]
fn file_date_different_size_is_out_of_date() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.set_file_date(1000, 512);
    assert!(m.check_out_of_date(1000, 1024));
}

#[test]
fn file_date_both_different_is_out_of_date() {
    let (change, update) = make_signals();
    let mut m: FileModel<String> = FileModel::new(PathBuf::from("t.dat"), change, update);
    m.set_file_date(1000, 512);
    assert!(m.check_out_of_date(2000, 1024));
}
