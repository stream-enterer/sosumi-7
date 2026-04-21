use std::path::PathBuf;

use emcore::emConfigModel::emConfigModel;
use emcore::emRecParser::RecError;
use emcore::emRecParser::{parse_rec, write_rec, RecStruct};
use emcore::emRecRecord::Record;
use emcore::emScheduler::EngineScheduler;

/// Test record type that exercises all field types RecStruct supports.
#[derive(Clone, Debug, PartialEq)]
struct TestConfig {
    name: String,
    count: i32,
    ratio: f64,
    enabled: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            count: 0,
            ratio: 1.0,
            enabled: false,
        }
    }
}

impl Record for TestConfig {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        Ok(Self {
            name: rec
                .get_str("name")
                .ok_or_else(|| RecError::MissingField("name".into()))?
                .to_string(),
            count: rec
                .get_int("count")
                .ok_or_else(|| RecError::MissingField("count".into()))?,
            ratio: rec
                .get_double("ratio")
                .ok_or_else(|| RecError::MissingField("ratio".into()))?,
            enabled: rec.get_bool("enabled").unwrap_or(false),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();
        s.set_str("name", &self.name);
        s.set_int("count", self.count);
        s.set_double("ratio", self.ratio);
        s.set_bool("enabled", self.enabled);
        s
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

fn tmp_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("eaglemode_test_config");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir.join(name)
}

#[test]
fn config_model_round_trip_save_load() {
    let path = tmp_path("round_trip.rec");
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();

    let original = TestConfig {
        name: "hello world".to_string(),
        count: 42,
        ratio: 1.25,
        enabled: true,
    };

    // Save
    let mut model = emConfigModel::new(original.clone(), path.clone(), sig);
    model.Save().expect("save should succeed");
    assert!(!model.IsUnsaved());

    // Load into a fresh model
    let mut model2 = emConfigModel::new(TestConfig::default(), path.clone(), sig);
    model2.TryLoad().expect("load should succeed");
    assert_eq!(model2.GetRec(), &original);
    assert!(!model2.IsUnsaved());

    // Cleanup
    let _ = std::fs::remove_file(&path);
}

#[test]
fn config_model_load_or_install_creates_default() {
    let path = tmp_path("install_default.rec");
    let _ = std::fs::remove_file(&path); // ensure clean state
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();

    let mut model = emConfigModel::new(TestConfig::default(), path.clone(), sig);
    model
        .TryLoadOrInstall()
        .expect("load_or_install should succeed");
    assert!(path.exists(), "file should be created");
    assert_eq!(model.GetRec(), &TestConfig::default());

    // Cleanup
    let _ = std::fs::remove_file(&path);
}

#[test]
fn config_model_load_or_install_reads_existing() {
    let path = tmp_path("install_existing.rec");
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();

    // Write a custom config first
    let custom = TestConfig {
        name: "custom".to_string(),
        count: 99,
        ratio: 4.5,
        enabled: true,
    };
    let mut writer = emConfigModel::new(custom.clone(), path.clone(), sig);
    writer.Save().expect("save");

    // TryLoadOrInstall should read existing, not overwrite
    let mut model = emConfigModel::new(TestConfig::default(), path.clone(), sig);
    model.TryLoadOrInstall().expect("load_or_install");
    assert_eq!(model.GetRec(), &custom);

    // Cleanup
    let _ = std::fs::remove_file(&path);
}

#[test]
fn config_model_set_marks_dirty() {
    let path = tmp_path("dirty.rec");
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();

    let mut model = emConfigModel::new(TestConfig::default(), path, sig);
    assert!(!model.IsUnsaved());
    model.Set(TestConfig {
        name: "changed".to_string(),
        ..TestConfig::default()
    });
    assert!(model.IsUnsaved());
}

#[test]
fn config_model_modify_marks_dirty() {
    let path = tmp_path("modify.rec");
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();

    let mut model = emConfigModel::new(TestConfig::default(), path, sig);
    model.modify(|c| c.count = 7);
    assert!(model.IsUnsaved());
    assert_eq!(model.GetRec().count, 7);
}

#[test]
fn config_model_reset_to_default() {
    let path = tmp_path("reset.rec");
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();

    let custom = TestConfig {
        name: "non-default".to_string(),
        count: 100,
        ratio: 9.9,
        enabled: true,
    };
    let mut model = emConfigModel::new(custom, path, sig);
    model.SetToDefault();
    assert!(model.IsUnsaved());
    assert_eq!(model.GetRec(), &TestConfig::default());
}

#[test]
fn record_round_trip_through_rec_text() {
    let original = TestConfig {
        name: "serialized".to_string(),
        count: -5,
        ratio: 0.001,
        enabled: false,
    };
    let rec = original.to_rec();
    let text = write_rec(&rec);
    let parsed = parse_rec(&text).expect("parse should succeed");
    let restored = TestConfig::from_rec(&parsed).expect("from_rec should succeed");
    assert_eq!(restored, original);
}
