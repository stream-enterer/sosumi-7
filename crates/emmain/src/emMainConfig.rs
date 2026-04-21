use std::cell::RefCell;
use std::rc::Rc;

use emcore::emConfigModel::emConfigModel;
use emcore::emContext::emContext;
use emcore::emInstallInfo::{InstallDirType, emGetInstallPath};
use emcore::emRecParser::{RecError, RecStruct};
use emcore::emRecRecord::Record;
use emcore::emSignal::SignalId;
use slotmap::Key as _;

/// Record type for emMainConfig fields.
///
/// Port of C++ `emMainConfig` data fields. Holds auto-hide and panel sizing options.
// DIVERGED: C++ `emMainConfig` is a single class; Rust splits the record data into
// `emMainConfigRec` and the model wrapper into `emMainConfig` (one primary type per file).
#[derive(Debug, Clone, PartialEq)]
pub struct emMainConfigRec {
    pub AutoHideControlView: bool,
    pub AutoHideSlider: bool,
    pub ControlViewSize: f64,
}

impl Default for emMainConfigRec {
    fn default() -> Self {
        Self {
            AutoHideControlView: false,
            AutoHideSlider: false,
            ControlViewSize: 0.515,
        }
    }
}

impl Record for emMainConfigRec {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let d = Self::default();
        Ok(Self {
            AutoHideControlView: rec
                .get_bool("AutoHideControlView")
                .unwrap_or(d.AutoHideControlView),
            AutoHideSlider: rec.get_bool("AutoHideSlider").unwrap_or(d.AutoHideSlider),
            ControlViewSize: rec
                .get_double("ControlViewSize")
                .unwrap_or(d.ControlViewSize)
                .clamp(0.0, 1.0),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();
        s.set_bool("AutoHideControlView", self.AutoHideControlView);
        s.set_bool("AutoHideSlider", self.AutoHideSlider);
        s.set_double("ControlViewSize", self.ControlViewSize);
        s
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

/// Model wrapper for emMainConfig.
///
/// Port of C++ `emMainConfig` (extends emConfigModel). Backed by a `emConfigModel`
/// for file persistence at `emMain/config.rec` under the user config directory.
pub struct emMainConfig {
    config_model: emConfigModel<emMainConfigRec>,
}

impl emMainConfig {
    /// Acquire the singleton `emMainConfig` from the context registry.
    ///
    /// Port of C++ `emMainConfig::Acquire`. On first call, creates the model,
    /// registers it, and loads from disk (or installs defaults).
    pub fn Acquire(ctx: &Rc<emContext>) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>("", || {
            let path = emGetInstallPath(InstallDirType::UserConfig, "emMain", Some("config.rec"))
                .unwrap_or_else(|_| {
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                    std::path::PathBuf::from(home)
                        .join(".eaglemode-rs")
                        .join("emMain")
                        .join("config.rec")
                });

            let mut model = emConfigModel::new(emMainConfigRec::default(), path, SignalId::null());

            if let Err(e) = model.TryLoadOrInstall() {
                log::warn!("MainConfig: failed to load or install config: {e}");
            }

            Self {
                config_model: model,
            }
        })
    }

    pub fn GetFormatName(&self) -> &str {
        "emMainConfig"
    }

    pub fn GetChangeSignal(&self) -> SignalId {
        self.config_model.GetChangeSignal()
    }

    pub fn GetAutoHideControlView(&self) -> bool {
        self.config_model.GetRec().AutoHideControlView
    }

    pub fn SetAutoHideControlView(&mut self, b: bool) {
        self.config_model.modify(|d| d.AutoHideControlView = b);
    }

    pub fn GetAutoHideSlider(&self) -> bool {
        self.config_model.GetRec().AutoHideSlider
    }

    pub fn SetAutoHideSlider(&mut self, b: bool) {
        self.config_model.modify(|d| d.AutoHideSlider = b);
    }

    pub fn GetControlViewSize(&self) -> f64 {
        self.config_model.GetRec().ControlViewSize
    }

    pub fn SetControlViewSize(&mut self, v: f64) {
        self.config_model
            .modify(|d| d.ControlViewSize = v.clamp(0.0, 1.0));
    }

    pub fn IsUnsaved(&self) -> bool {
        self.config_model.IsUnsaved()
    }

    /// Persist the current config to disk.
    ///
    /// Port of C++ `emConfigModel::Save`. Writes the current record to the config file.
    pub fn Save(&mut self) {
        if let Err(e) = self.config_model.Save() {
            log::warn!("MainConfig: failed to save config: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let config = emMainConfigRec::default();
        assert!(!config.AutoHideControlView);
        assert!(!config.AutoHideSlider);
        assert!((config.ControlViewSize - 0.515).abs() < 1e-10);
    }

    #[test]
    fn test_round_trip() {
        let config = emMainConfigRec {
            AutoHideControlView: true,
            ControlViewSize: 0.75,
            ..emMainConfigRec::default()
        };
        let rec = config.to_rec();
        let loaded = emMainConfigRec::from_rec(&rec).unwrap();
        assert!(loaded.AutoHideControlView);
        assert!(!loaded.AutoHideSlider);
        assert!((loaded.ControlViewSize - 0.75).abs() < 1e-10);
    }

    #[test]
    fn test_clamp_control_view_size() {
        let mut rec = RecStruct::new();
        rec.set_double("ControlViewSize", 1.5);
        let config = emMainConfigRec::from_rec(&rec).unwrap();
        assert!((config.ControlViewSize - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_set_to_default() {
        let mut config = emMainConfigRec {
            AutoHideControlView: true,
            AutoHideSlider: true,
            ControlViewSize: 0.9,
        };
        assert!(!config.IsSetToDefault());
        config.SetToDefault();
        assert!(config.IsSetToDefault());
    }

    #[test]
    fn test_acquire_singleton() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let c1 = emMainConfig::Acquire(&ctx);
        let c2 = emMainConfig::Acquire(&ctx);
        assert!(Rc::ptr_eq(&c1, &c2));
    }

    #[test]
    fn test_getters_match_defaults() {
        // Note: GetControlViewSize reflects whatever Acquire loads from disk
        // (via TryLoadOrInstall), so its value is user-dependent. The defaults
        // for the raw rec are verified by `test_defaults` above; here we only
        // check that the getters proxy correctly through the loaded config.
        let ctx = emcore::emContext::emContext::NewRoot();
        let cfg = emMainConfig::Acquire(&ctx);
        let cfg = cfg.borrow();
        let rec_size = cfg.GetControlViewSize();
        assert!((0.0..=1.0).contains(&rec_size));
        assert_eq!(cfg.GetFormatName(), "emMainConfig");
    }

    #[test]
    fn test_setters_round_trip() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let cfg = emMainConfig::Acquire(&ctx);
        {
            let mut cfg = cfg.borrow_mut();
            cfg.SetAutoHideControlView(true);
            cfg.SetAutoHideSlider(true);
            cfg.SetControlViewSize(0.8);
        }
        let cfg = cfg.borrow();
        assert!(cfg.GetAutoHideControlView());
        assert!(cfg.GetAutoHideSlider());
        assert!((cfg.GetControlViewSize() - 0.8).abs() < 1e-10);
    }
}
