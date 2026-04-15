use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use emcore::emContext::emContext;
use emcore::emFileModel::FileState;
use emcore::emInstallInfo::{emGetInstallPath, InstallDirType};
use emcore::emRec::{RecError, RecStruct};
use emcore::emRecFileModel::emRecFileModel;
use emcore::emRecRecord::Record;

/// Base path type for emFileLink resolution.
///
/// Port of C++ anonymous enum constants in `emFileLinkModel`.
/// Values match C++ BPT_* constants exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum BasePathType {
    #[default]
    None       =  0,
    Bin        =  1,
    Include    =  2,
    Lib        =  3,
    HtmlDoc    =  4,
    PdfDoc     =  5,
    PsDoc      =  6,
    UserConfig =  7,
    HostConfig =  8,
    Tmp        =  9,
    Res        = 10,
    Home       = 11,
}

impl BasePathType {
    fn to_ident(self) -> &'static str {
        match self {
            Self::None       => "none",
            Self::Bin        => "bin",
            Self::Include    => "include",
            Self::Lib        => "lib",
            Self::HtmlDoc    => "htmldoc",
            Self::PdfDoc     => "pdfdoc",
            Self::PsDoc      => "psdoc",
            Self::UserConfig => "userconfig",
            Self::HostConfig => "hostconfig",
            Self::Tmp        => "tmp",
            Self::Res        => "res",
            Self::Home       => "home",
        }
    }

    fn from_ident(s: &str) -> Result<Self, RecError> {
        match s {
            "none"       => Ok(Self::None),
            "bin"        => Ok(Self::Bin),
            "include"    => Ok(Self::Include),
            "lib"        => Ok(Self::Lib),
            "htmldoc"    => Ok(Self::HtmlDoc),
            "pdfdoc"     => Ok(Self::PdfDoc),
            "psdoc"      => Ok(Self::PsDoc),
            "userconfig" => Ok(Self::UserConfig),
            "hostconfig" => Ok(Self::HostConfig),
            "tmp"        => Ok(Self::Tmp),
            "res"        => Ok(Self::Res),
            "home"       => Ok(Self::Home),
            _ => Err(RecError::InvalidValue {
                field: "BasePathType".to_string(),
                message: format!("unknown base path type: {s}"),
            }),
        }
    }
}

/// Data record for `.emFileLink` files.
///
/// Port of C++ `emFileLinkModel` record fields. Format name: `"emFileLink"`.
#[derive(Clone, Debug, PartialEq)]
pub struct emFileLinkData {
    pub base_path_type: BasePathType,
    pub base_path_project: String,
    pub path: String,
    pub have_dir_entry: bool,
}

impl Default for emFileLinkData {
    fn default() -> Self {
        Self {
            base_path_type: BasePathType::None,
            base_path_project: String::new(),
            path: String::new(),
            have_dir_entry: false,
        }
    }
}

impl emFileLinkData {
    /// Resolve the full filesystem path for this link.
    ///
    /// Port of C++ `emFileLinkModel::GetFullPath()`.
    ///
    /// When `base_path_type` is `None`, the base path is the parent directory
    /// of `file_path` (the path to the `.emFileLink` file itself). Otherwise,
    /// `emGetInstallPath` is called with the appropriate `InstallDirType`.
    ///
    /// The `self.path` field is then joined onto the base path.
    pub fn GetFullPath(&self, file_path: &str) -> String {
        let prj = if self.base_path_project.is_empty() {
            "unknown"
        } else {
            &self.base_path_project
        };

        let base = match self.base_path_type {
            BasePathType::None => {
                // C++: emGetParentPath(GetFilePath())
                let p = std::path::Path::new(file_path);
                p.parent()
                    .map(|d| d.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
            }
            BasePathType::Bin        => emGetInstallPath(InstallDirType::Bin,        prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::Include    => emGetInstallPath(InstallDirType::Include,    prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::Lib        => emGetInstallPath(InstallDirType::Lib,        prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::HtmlDoc    => emGetInstallPath(InstallDirType::HtmlDoc,    prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::PdfDoc     => emGetInstallPath(InstallDirType::PdfDoc,     prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::PsDoc      => emGetInstallPath(InstallDirType::PsDoc,      prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::UserConfig => emGetInstallPath(InstallDirType::UserConfig, prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::HostConfig => emGetInstallPath(InstallDirType::HostConfig, prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::Tmp        => emGetInstallPath(InstallDirType::Tmp,        prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::Res        => emGetInstallPath(InstallDirType::Res,        prj, None).unwrap_or_else(|_| PathBuf::new()),
            BasePathType::Home       => emGetInstallPath(InstallDirType::Home,       prj, None).unwrap_or_else(|_| PathBuf::new()),
        };

        // C++: emGetAbsolutePath(Path.Get(), basePath) — join base + path
        let joined = if self.path.is_empty() {
            base
        } else {
            base.join(&self.path)
        };

        joined.to_string_lossy().into_owned()
    }
}

impl Record for emFileLinkData {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let base_path_type = match rec.get_ident("basepathtype") {
            Some(s) => BasePathType::from_ident(s)?,
            None => BasePathType::None,
        };
        let base_path_project = rec
            .get_str("basepathproject")
            .unwrap_or("")
            .to_string();
        let path = rec.get_str("path").unwrap_or("").to_string();
        let have_dir_entry = rec.get_bool("havedirentry").unwrap_or(false);

        Ok(Self {
            base_path_type,
            base_path_project,
            path,
            have_dir_entry,
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut rec = RecStruct::new();
        rec.set_ident("BasePathType", self.base_path_type.to_ident());
        rec.set_str("BasePathProject", &self.base_path_project);
        rec.set_str("Path", &self.path);
        rec.set_bool("HaveDirEntry", self.have_dir_entry);
        rec
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

/// Model wrapper for `.emFileLink` files.
///
/// Port of C++ `emFileLinkModel` (extends `emRecFileModel`).
pub struct emFileLinkModel {
    rec_model: emRecFileModel<emFileLinkData>,
}

impl emFileLinkModel {
    pub fn Acquire(ctx: &Rc<emContext>, name: &str, _common: bool) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>(name, || Self {
            rec_model: emRecFileModel::new(PathBuf::from(name)),
        })
    }

    pub fn GetFormatName(&self) -> &str {
        "emFileLink"
    }

    pub fn GetFileState(&self) -> &FileState {
        self.rec_model.GetFileState()
    }

    pub fn GetFullPath(&self) -> String {
        let data = self.rec_model.GetMap();
        let file_path = self.rec_model.path().to_string_lossy();
        data.GetFullPath(&file_path)
    }

    pub fn GetBasePathType(&self) -> BasePathType {
        self.rec_model.GetMap().base_path_type
    }

    pub fn GetBasePathProject(&self) -> &str {
        &self.rec_model.GetMap().base_path_project
    }

    pub fn GetPath(&self) -> &str {
        &self.rec_model.GetMap().path
    }

    pub fn GetHaveDirEntry(&self) -> bool {
        self.rec_model.GetMap().have_dir_entry
    }

    /// Load the model synchronously if not yet loaded.
    /// Returns true if the model is now in Loaded state.
    pub fn ensure_loaded(&mut self) -> bool {
        if matches!(self.rec_model.GetFileState(), FileState::Waiting) {
            self.rec_model.TryLoad();
        }
        matches!(self.rec_model.GetFileState(), FileState::Loaded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emRecRecord::Record;

    #[test]
    fn default_values() {
        let m = emFileLinkData::default();
        assert_eq!(m.base_path_type, BasePathType::None);
        assert!(m.base_path_project.is_empty());
        assert!(m.path.is_empty());
        assert!(!m.have_dir_entry);
    }

    #[test]
    fn record_round_trip() {
        let mut m = emFileLinkData::default();
        m.base_path_type = BasePathType::Res;
        m.base_path_project = "emFileMan".to_string();
        m.path = "themes".to_string();
        m.have_dir_entry = true;

        let rec = m.to_rec();
        let m2 = emFileLinkData::from_rec(&rec).unwrap();

        assert_eq!(m2.base_path_type, BasePathType::Res);
        assert_eq!(m2.base_path_project, "emFileMan");
        assert_eq!(m2.path, "themes");
        assert!(m2.have_dir_entry);
    }

    #[test]
    fn base_path_type_values_match_cpp() {
        assert_eq!(BasePathType::None as i32, 0);
        assert_eq!(BasePathType::Bin as i32, 1);
        assert_eq!(BasePathType::Include as i32, 2);
        assert_eq!(BasePathType::Lib as i32, 3);
        assert_eq!(BasePathType::HtmlDoc as i32, 4);
        assert_eq!(BasePathType::PdfDoc as i32, 5);
        assert_eq!(BasePathType::PsDoc as i32, 6);
        assert_eq!(BasePathType::UserConfig as i32, 7);
        assert_eq!(BasePathType::HostConfig as i32, 8);
        assert_eq!(BasePathType::Tmp as i32, 9);
        assert_eq!(BasePathType::Res as i32, 10);
        assert_eq!(BasePathType::Home as i32, 11);
    }

    #[test]
    fn get_full_path_none_type() {
        let m = emFileLinkData {
            base_path_type: BasePathType::None,
            base_path_project: String::new(),
            path: "subdir/file.txt".to_string(),
            have_dir_entry: false,
        };
        let full = m.GetFullPath("/some/dir/link.emFileLink");
        // None type uses parent of file_path as base
        assert!(full.ends_with("subdir/file.txt"));
        assert!(full.starts_with("/some/dir/"));
    }

    #[test]
    fn model_acquire_returns_same_instance() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let m1 = emFileLinkModel::Acquire(&ctx, "/tmp/test.emFileLink", true);
        let m2 = emFileLinkModel::Acquire(&ctx, "/tmp/test.emFileLink", true);
        assert!(Rc::ptr_eq(&m1, &m2));
    }

    #[test]
    fn model_acquire_different_names() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let m1 = emFileLinkModel::Acquire(&ctx, "/tmp/a.emFileLink", true);
        let m2 = emFileLinkModel::Acquire(&ctx, "/tmp/b.emFileLink", true);
        assert!(!Rc::ptr_eq(&m1, &m2));
    }

    #[test]
    fn model_get_format_name() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let m = emFileLinkModel::Acquire(&ctx, "/tmp/test.emFileLink", true);
        assert_eq!(m.borrow().GetFormatName(), "emFileLink");
    }

    #[test]
    fn model_delegates_to_data() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let m = emFileLinkModel::Acquire(&ctx, "/tmp/test.emFileLink", true);
        let m = m.borrow();
        assert_eq!(m.GetBasePathType(), BasePathType::None);
        assert_eq!(m.GetBasePathProject(), "");
        assert_eq!(m.GetPath(), "");
        assert!(!m.GetHaveDirEntry());
    }
}
