use zuicchini::model::FileModelOps;

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
