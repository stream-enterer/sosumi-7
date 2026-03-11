use zuicchini::model::FileModelOps;

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
