use std::path::PathBuf;

use zuicchini::model::{FileModel, FileModelOps, FileState};
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

struct TestLoader {
    steps: u32,
    current: u32,
    loaded: bool,
}

impl FileModelOps for TestLoader {
    fn reset_data(&mut self) {
        self.current = 0;
        self.loaded = false;
    }
    fn try_start_loading(&mut self) -> Result<(), String> {
        self.current = 0;
        Ok(())
    }
    fn try_continue_loading(&mut self) -> Result<bool, String> {
        self.current += 1;
        Ok(self.current >= self.steps)
    }
    fn quit_loading(&mut self) {
        self.loaded = true;
    }
    fn try_start_saving(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn try_continue_saving(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn quit_saving(&mut self) {}
    fn calc_memory_need(&self) -> u64 {
        256
    }
    fn calc_file_progress(&self) -> f64 {
        if self.steps == 0 {
            100.0
        } else {
            (self.current as f64 / self.steps as f64) * 100.0
        }
    }
}

#[test]
fn loading_lifecycle() {
    let mut loader = TestLoader {
        steps: 3,
        current: 0,
        loaded: false,
    };
    loader.try_start_loading().expect("start loading");
    assert!(!loader.try_continue_loading().expect("step 1")); // step 1
    assert!(!loader.try_continue_loading().expect("step 2")); // step 2
    assert!(loader.try_continue_loading().expect("step 3")); // step 3 = done
    loader.quit_loading();
    assert!(loader.loaded);
}

#[test]
fn loading_error() {
    struct FailLoader;
    impl FileModelOps for FailLoader {
        fn reset_data(&mut self) {}
        fn try_start_loading(&mut self) -> Result<(), String> {
            Err("cannot open file".to_string())
        }
        fn try_continue_loading(&mut self) -> Result<bool, String> {
            Ok(true)
        }
        fn quit_loading(&mut self) {}
        fn try_start_saving(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn try_continue_saving(&mut self) -> Result<bool, String> {
            Ok(true)
        }
        fn quit_saving(&mut self) {}
        fn calc_memory_need(&self) -> u64 {
            0
        }
        fn calc_file_progress(&self) -> f64 {
            0.0
        }
    }

    let mut loader = FailLoader;
    let result = loader.try_start_loading();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "cannot open file");
}

struct TestSaver {
    save_count: u32,
    saved: bool,
}

impl FileModelOps for TestSaver {
    fn reset_data(&mut self) {}
    fn try_start_loading(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn try_continue_loading(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn quit_loading(&mut self) {}
    fn try_start_saving(&mut self) -> Result<(), String> {
        self.save_count = 0;
        Ok(())
    }
    fn try_continue_saving(&mut self) -> Result<bool, String> {
        self.save_count += 1;
        Ok(self.save_count >= 2)
    }
    fn quit_saving(&mut self) {
        self.saved = true;
    }
    fn calc_memory_need(&self) -> u64 {
        0
    }
    fn calc_file_progress(&self) -> f64 {
        0.0
    }
}

#[test]
fn saving_lifecycle() {
    let mut saver = TestSaver {
        save_count: 0,
        saved: false,
    };
    saver.try_start_saving().expect("start saving");
    assert!(!saver.try_continue_saving().expect("step 1")); // step 1
    assert!(saver.try_continue_saving().expect("step 2")); // step 2 = done
    saver.quit_saving();
    assert!(saver.saved);
}

struct MemModel {
    data_size: u64,
}

impl FileModelOps for MemModel {
    fn reset_data(&mut self) {
        self.data_size = 0;
    }
    fn try_start_loading(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn try_continue_loading(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn quit_loading(&mut self) {}
    fn try_start_saving(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn try_continue_saving(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn quit_saving(&mut self) {}
    fn calc_memory_need(&self) -> u64 {
        self.data_size
    }
    fn calc_file_progress(&self) -> f64 {
        0.0
    }
}

#[test]
fn calc_memory_need_returns_data_size() {
    let m = MemModel { data_size: 4096 };
    assert_eq!(m.calc_memory_need(), 4096);
}

#[test]
fn reset_data_clears_memory_need() {
    let mut m = MemModel { data_size: 4096 };
    m.reset_data();
    assert_eq!(m.data_size, 0);
    assert_eq!(m.calc_memory_need(), 0);
}

struct ProgModel {
    done: f64,
    total: f64,
}

impl FileModelOps for ProgModel {
    fn reset_data(&mut self) {}
    fn try_start_loading(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn try_continue_loading(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn quit_loading(&mut self) {}
    fn try_start_saving(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn try_continue_saving(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn quit_saving(&mut self) {}
    fn calc_memory_need(&self) -> u64 {
        0
    }
    fn calc_file_progress(&self) -> f64 {
        if self.total == 0.0 {
            0.0
        } else {
            (self.done / self.total) * 100.0
        }
    }
}

#[test]
fn calc_file_progress_percentage() {
    let m = ProgModel {
        done: 50.0,
        total: 200.0,
    };
    assert!((m.calc_file_progress() - 25.0).abs() < 0.01);
}

#[test]
fn calc_file_progress_zero_total() {
    let m = ProgModel {
        done: 0.0,
        total: 0.0,
    };
    assert!((m.calc_file_progress()).abs() < 0.01);
}

#[test]
fn calc_file_progress_complete() {
    let m = ProgModel {
        done: 200.0,
        total: 200.0,
    };
    assert!((m.calc_file_progress() - 100.0).abs() < 0.01);
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
