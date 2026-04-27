use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use emcore::emFileModel::{emAbsoluteFileModelClient, emFileModel, FileModelOps, FileState};
use emcore::emScheduler::EngineScheduler;

fn make_change_signal() -> emcore::emSignal::SignalId {
    let mut sched = EngineScheduler::new();
    sched.create_signal()
}

#[test]
fn get_memory_need_default_zero() {
    let change = make_change_signal();
    let m: emFileModel<Vec<u8>> = emFileModel::new(PathBuf::from("test.dat"), change);
    assert_eq!(m.get_memory_need(), 0);
}

#[test]
fn get_memory_need_after_update() {
    let change = make_change_signal();
    let mut m: emFileModel<Vec<u8>> = emFileModel::new(PathBuf::from("test.dat"), change);
    m.CalcMemoryNeed(1024);
    assert_eq!(m.get_memory_need(), 1024);
}

#[test]
fn update_retries_load_error() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.Load();
    m.fail_load("network error".to_string());
    assert!(matches!(m.GetFileState(), FileState::LoadError(_)));
    m.update();
    assert!(matches!(m.GetFileState(), &FileState::Waiting));
}

#[test]
fn update_retries_too_costly() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.mark_too_costly();
    m.update();
    assert!(matches!(m.GetFileState(), &FileState::Waiting));
}

#[test]
fn update_unloads_out_of_date() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.Load();
    m.complete_load("data".to_string());
    m.TryFetchDate(1000, 512);
    m.IsOutOfDate(2000, 512); // marks out_of_date = true
    m.update();
    assert!(matches!(m.GetFileState(), &FileState::Waiting));
    assert!(m.GetMap().is_none());
}

#[test]
fn update_keeps_loaded_if_fresh() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.Load();
    m.complete_load("data".to_string());
    m.TryFetchDate(1000, 512);
    m.IsOutOfDate(1000, 512); // same date, not out of date
    m.update();
    assert!(matches!(m.GetFileState(), &FileState::Loaded));
    assert_eq!(
        m.GetMap().unwrap(),
        "data",
        "loaded data should be preserved after fresh update"
    );
}

#[test]
fn reset_data_clears_everything() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.Load();
    m.complete_load("data".to_string());
    m.CalcMemoryNeed(500);
    m.reset_data();
    assert!(matches!(m.GetFileState(), &FileState::Waiting));
    assert!(m.GetMap().is_none());
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
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.TryFetchDate(1000, 512);
    assert!(!m.IsOutOfDate(1000, 512));
}

#[test]
fn file_date_different_mtime_is_out_of_date() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.TryFetchDate(1000, 512);
    assert!(m.IsOutOfDate(2000, 512));
}

#[test]
fn file_date_different_size_is_out_of_date() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.TryFetchDate(1000, 512);
    assert!(m.IsOutOfDate(1000, 1024));
}

#[test]
fn file_date_both_different_is_out_of_date() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.TryFetchDate(1000, 512);
    assert!(m.IsOutOfDate(2000, 1024));
}

/// B-007 gap-fix: AcquireUpdateSignalModel returns the scheduler's shared broadcast
/// signal, not a per-model signal. Verified via EngineCtx plumbed with file_update_signal.
///
/// The old tests `update_signal_returned` and `update_signal_differs_from_change`
/// verified the pre-fix behavior (per-model update_signal field). They are replaced
/// by the B-007 tests in `crates/emcore/tests/typed_subscribe_b007.rs`.
#[test]
fn ignore_update_signal_default_false() {
    let change = make_change_signal();
    let m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    assert!(!m.GetIgnoreUpdateSignal());
}

#[test]
fn set_ignore_update_signal_true() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.set_ignore_update_signal(true);
    assert!(m.GetIgnoreUpdateSignal());
}

#[test]
fn set_ignore_update_signal_toggle() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.set_ignore_update_signal(true);
    assert!(m.GetIgnoreUpdateSignal());
    m.set_ignore_update_signal(false);
    assert!(!m.GetIgnoreUpdateSignal());
}

#[test]
fn clear_save_error_transitions_to_unsaved() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.Load();
    m.complete_load("data".to_string());
    m.SetUnsavedState();
    m.Save();
    m.fail_save("disk full".to_string());
    assert!(matches!(m.GetFileState(), FileState::SaveError(_)));
    m.clear_save_error();
    assert!(matches!(m.GetFileState(), &FileState::Unsaved));
}

#[test]
fn clear_save_error_noop_in_waiting() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.clear_save_error();
    assert!(matches!(m.GetFileState(), &FileState::Waiting));
}

#[test]
fn clear_save_error_noop_in_loaded() {
    let change = make_change_signal();
    let mut m: emFileModel<String> = emFileModel::new(PathBuf::from("t.dat"), change);
    m.Load();
    m.complete_load("data".to_string());
    m.clear_save_error();
    assert!(matches!(m.GetFileState(), &FileState::Loaded));
}

#[test]
fn absolute_file_model_client_empty() {
    let client: emAbsoluteFileModelClient<Vec<u8>> = emAbsoluteFileModelClient::new();
    assert!(client.GetModel().is_none());
}

#[test]
fn absolute_file_model_client_set_and_get() {
    let model = Rc::new(RefCell::new(vec![1u8, 2, 3]));
    let mut client = emAbsoluteFileModelClient::new();
    client.SetModel(&model);
    assert!(client.GetModel().is_some());
    assert_eq!(*client.GetModel().unwrap().borrow(), vec![1u8, 2, 3]);
}
