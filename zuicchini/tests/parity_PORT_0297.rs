use zuicchini::model::FileModelOps;

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
