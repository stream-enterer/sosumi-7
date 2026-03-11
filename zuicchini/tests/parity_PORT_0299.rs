use zuicchini::model::FileModelOps;

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
