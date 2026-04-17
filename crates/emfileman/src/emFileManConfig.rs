use std::cell::RefCell;
use std::rc::Rc;

use emcore::emConfigModel::emConfigModel;
use emcore::emContext::emContext;
use emcore::emInstallInfo::{emGetInstallPath, InstallDirType};
use emcore::emRec::{RecError, RecStruct};
use emcore::emRecRecord::Record;
use emcore::emSignal::SignalId;
use slotmap::Key as _;

/// DIVERGED: C++ uses anonymous enum constants inside `emFileManConfig`.
/// Rust uses a standalone enum for type safety.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SortCriterion {
    ByName = 0,
    ByEnding = 1,
    ByClass = 2,
    ByVersion = 3,
    ByDate = 4,
    BySize = 5,
}

impl SortCriterion {
    fn to_ident(self) -> &'static str {
        match self {
            Self::ByName => "sort_by_name",
            Self::ByEnding => "sort_by_ending",
            Self::ByClass => "sort_by_class",
            Self::ByVersion => "sort_by_version",
            Self::ByDate => "sort_by_date",
            Self::BySize => "sort_by_size",
        }
    }

    fn from_ident(s: &str) -> Result<Self, RecError> {
        match s {
            "sort_by_name" => Ok(Self::ByName),
            "sort_by_ending" => Ok(Self::ByEnding),
            "sort_by_class" => Ok(Self::ByClass),
            "sort_by_version" => Ok(Self::ByVersion),
            "sort_by_date" => Ok(Self::ByDate),
            "sort_by_size" => Ok(Self::BySize),
            _ => Err(RecError::InvalidValue {
                field: "SortCriterion".to_string(),
                message: format!("unknown sort criterion: {s}"),
            }),
        }
    }
}

/// DIVERGED: C++ uses anonymous enum constants inside `emFileManConfig`.
/// Rust uses a standalone enum for type safety.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum NameSortingStyle {
    PerLocale = 0,
    CaseSensitive = 1,
    CaseInsensitive = 2,
}

impl NameSortingStyle {
    fn to_ident(self) -> &'static str {
        match self {
            Self::PerLocale => "nss_per_locale",
            Self::CaseSensitive => "nss_case_sensitive",
            Self::CaseInsensitive => "nss_case_insensitive",
        }
    }

    fn from_ident(s: &str) -> Result<Self, RecError> {
        match s {
            "nss_per_locale" => Ok(Self::PerLocale),
            "nss_case_sensitive" => Ok(Self::CaseSensitive),
            "nss_case_insensitive" => Ok(Self::CaseInsensitive),
            _ => Err(RecError::InvalidValue {
                field: "NameSortingStyle".to_string(),
                message: format!("unknown name sorting style: {s}"),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct emFileManConfigData {
    pub sort_criterion: SortCriterion,
    pub name_sorting_style: NameSortingStyle,
    pub sort_directories_first: bool,
    pub show_hidden_files: bool,
    pub theme_name: String,
    pub autosave: bool,
}

impl Default for emFileManConfigData {
    fn default() -> Self {
        Self {
            sort_criterion: SortCriterion::ByName,
            name_sorting_style: NameSortingStyle::PerLocale,
            sort_directories_first: false,
            show_hidden_files: false,
            theme_name: String::new(),
            autosave: true,
        }
    }
}

impl Record for emFileManConfigData {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let sort_criterion = match rec.get_ident("sortcriterion") {
            Some(s) => SortCriterion::from_ident(s)?,
            None => SortCriterion::ByName,
        };
        let name_sorting_style = match rec.get_ident("namesortingstyle") {
            Some(s) => NameSortingStyle::from_ident(s)?,
            None => NameSortingStyle::PerLocale,
        };
        let sort_directories_first = rec.get_bool("sortdirectoriesfirst").unwrap_or(false);
        let show_hidden_files = rec.get_bool("showhiddenfiles").unwrap_or(false);
        let theme_name = rec.get_str("themename").unwrap_or("").to_string();
        let autosave = rec.get_bool("autosave").unwrap_or(true);

        Ok(Self {
            sort_criterion,
            name_sorting_style,
            sort_directories_first,
            show_hidden_files,
            theme_name,
            autosave,
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut rec = RecStruct::new();
        rec.set_ident("SortCriterion", self.sort_criterion.to_ident());
        rec.set_ident("NameSortingStyle", self.name_sorting_style.to_ident());
        rec.set_bool("SortDirectoriesFirst", self.sort_directories_first);
        rec.set_bool("ShowHiddenFiles", self.show_hidden_files);
        rec.set_str("ThemeName", &self.theme_name);
        rec.set_bool("Autosave", self.autosave);
        rec
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

/// Model wrapper for emFileManConfig.
/// Port of C++ `emFileManConfig` (extends emConfigModel).
pub struct emFileManConfig {
    config_model: emConfigModel<emFileManConfigData>,
}

impl emFileManConfig {
    pub fn Acquire(ctx: &Rc<emContext>) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>("", || {
            let signal_id = SignalId::null();
            // C++ emFileManConfig uses emGetInstallPath(USER_CONFIG,
            // "emFileMan", "config.rec") then TryLoadOrInstall.
            let path =
                emGetInstallPath(InstallDirType::UserConfig, "emFileMan", Some("config.rec"))
                    .unwrap_or_default();
            let mut config_model =
                emConfigModel::new(emFileManConfigData::default(), path, signal_id)
                    .with_format_name("emFileManConfig");
            if let Err(e) = config_model.TryLoadOrInstall() {
                log::warn!(
                    "emFileManConfig::Acquire: TryLoadOrInstall({:?}) failed: {}",
                    config_model.GetInstallPath(),
                    e
                );
            }
            Self { config_model }
        })
    }

    pub fn GetFormatName(&self) -> &str {
        "emFileManConfig"
    }

    pub fn GetChangeSignal(&self) -> SignalId {
        self.config_model.GetChangeSignal()
    }

    pub fn GetSortCriterion(&self) -> SortCriterion {
        self.config_model.GetRec().sort_criterion
    }

    pub fn SetSortCriterion(&mut self, sc: SortCriterion) {
        self.config_model.modify(|d| d.sort_criterion = sc);
    }

    pub fn GetNameSortingStyle(&self) -> NameSortingStyle {
        self.config_model.GetRec().name_sorting_style
    }

    pub fn SetNameSortingStyle(&mut self, nss: NameSortingStyle) {
        self.config_model.modify(|d| d.name_sorting_style = nss);
    }

    pub fn GetSortDirectoriesFirst(&self) -> bool {
        self.config_model.GetRec().sort_directories_first
    }

    pub fn SetSortDirectoriesFirst(&mut self, b: bool) {
        self.config_model.modify(|d| d.sort_directories_first = b);
    }

    pub fn GetShowHiddenFiles(&self) -> bool {
        self.config_model.GetRec().show_hidden_files
    }

    pub fn SetShowHiddenFiles(&mut self, b: bool) {
        self.config_model.modify(|d| d.show_hidden_files = b);
    }

    pub fn GetThemeName(&self) -> &str {
        &self.config_model.GetRec().theme_name
    }

    pub fn SetThemeName(&mut self, name: &str) {
        self.config_model
            .modify(|d| d.theme_name = name.to_string());
    }

    pub fn GetAutosave(&self) -> bool {
        self.config_model.GetRec().autosave
    }

    pub fn SetAutosave(&mut self, b: bool) {
        self.config_model.modify(|d| d.autosave = b);
    }

    pub fn IsUnsaved(&self) -> bool {
        self.config_model.IsUnsaved()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let c = emFileManConfigData::default();
        assert_eq!(c.sort_criterion, SortCriterion::ByName);
        assert_eq!(c.name_sorting_style, NameSortingStyle::PerLocale);
        assert!(!c.sort_directories_first);
        assert!(!c.show_hidden_files);
        assert!(c.theme_name.is_empty());
        assert!(c.autosave);
    }

    #[test]
    fn record_round_trip() {
        let c = emFileManConfigData {
            sort_criterion: SortCriterion::ByDate,
            name_sorting_style: NameSortingStyle::CaseInsensitive,
            sort_directories_first: true,
            show_hidden_files: true,
            theme_name: "Glass1".to_string(),
            autosave: false,
        };

        let rec = c.to_rec();
        let c2 = emFileManConfigData::from_rec(&rec).unwrap();

        assert_eq!(c2.sort_criterion, SortCriterion::ByDate);
        assert_eq!(c2.name_sorting_style, NameSortingStyle::CaseInsensitive);
        assert!(c2.sort_directories_first);
        assert!(c2.show_hidden_files);
        assert_eq!(c2.theme_name, "Glass1");
        assert!(!c2.autosave);
    }

    #[test]
    fn sort_criterion_values_match_cpp() {
        assert_eq!(SortCriterion::ByName as i32, 0);
        assert_eq!(SortCriterion::ByEnding as i32, 1);
        assert_eq!(SortCriterion::ByClass as i32, 2);
        assert_eq!(SortCriterion::ByVersion as i32, 3);
        assert_eq!(SortCriterion::ByDate as i32, 4);
        assert_eq!(SortCriterion::BySize as i32, 5);
    }

    #[test]
    fn config_model_acquire_singleton() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let c1 = emFileManConfig::Acquire(&ctx);
        let c2 = emFileManConfig::Acquire(&ctx);
        assert!(Rc::ptr_eq(&c1, &c2));
    }

    // Note: test `config_model_getters_match_defaults` was removed when
    // `Acquire` started calling `TryLoadOrInstall` (matching C++). With disk
    // load, getters reflect the user's on-disk config rather than struct
    // defaults. Default-value assertions live in `default_values` (which
    // tests the struct directly, without Acquire).

    #[test]
    fn config_model_setters_round_trip() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let cfg = emFileManConfig::Acquire(&ctx);
        {
            let mut cfg = cfg.borrow_mut();
            cfg.SetSortCriterion(SortCriterion::BySize);
            cfg.SetNameSortingStyle(NameSortingStyle::CaseInsensitive);
            cfg.SetSortDirectoriesFirst(true);
            cfg.SetShowHiddenFiles(true);
            cfg.SetThemeName("Glass1");
            cfg.SetAutosave(false);
        }
        let cfg = cfg.borrow();
        assert_eq!(cfg.GetSortCriterion(), SortCriterion::BySize);
        assert_eq!(cfg.GetNameSortingStyle(), NameSortingStyle::CaseInsensitive);
        assert!(cfg.GetSortDirectoriesFirst());
        assert!(cfg.GetShowHiddenFiles());
        assert_eq!(cfg.GetThemeName(), "Glass1");
        assert!(!cfg.GetAutosave());
    }
}
