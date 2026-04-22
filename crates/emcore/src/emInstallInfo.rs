use std::fmt;
use std::path::{Path, PathBuf};

/// Directory type for Eagle Mode installation paths.
///
/// Port of C++ `emInstallDirType`. Each variant maps to a specific
/// directory in the Eagle Mode installation layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstallDirType {
    /// Binary directory (`<em>/bin`).
    Bin,
    /// Include directory (`<em>/include/<prj>`).
    Include,
    /// Library directory (`<em>/lib`).
    Lib,
    /// HTML documentation (`<em>/doc/html`).
    HtmlDoc,
    /// PostScript documentation (`<em>/doc/ps`).
    PsDoc,
    /// Per-user configuration (`~/.eaglemode-rs/<prj>` or `$EM_USER_CONFIG_DIR/<prj>`).
    ///
    /// DIVERGED: (language-forced) C++ uses `~/.eaglemode`. eaglemode-rs uses `~/.eaglemode-rs`
    /// so the two builds do not share a config directory: Rust's serialized
    /// files are not bit-identical to C++'s (notably missing the emRec format
    /// header), and letting them overwrite each other breaks the C++ install.
    UserConfig,
    /// System-wide configuration (`<em>/etc/<prj>`).
    HostConfig,
    /// Temporary directory (`$TMPDIR` or `/tmp`).
    Tmp,
    /// Resources directory (`<em>/res/<prj>`).
    Res,
    /// User home directory (`$HOME`).
    Home,
    /// PDF documentation (`<em>/doc/pdf`).
    PdfDoc,
}

/// Errors from install path resolution.
#[derive(Debug)]
pub enum InstallInfoError {
    /// A required environment variable is not set.
    EnvNotSet(String),
    /// An I/O error occurred (e.g., reading version files).
    IoError(std::io::Error),
    /// This platform is not supported.
    UnsupportedPlatform,
}

impl fmt::Display for InstallInfoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnvNotSet(var) => write!(f, "environment variable {var} is not set"),
            Self::IoError(e) => write!(f, "I/O error: {e}"),
            Self::UnsupportedPlatform => write!(f, "unsupported platform"),
        }
    }
}

impl std::error::Error for InstallInfoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for InstallInfoError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

/// Resolve an Eagle Mode installation path.
///
/// Port of C++ `emGetInstallPath`. Maps directory types to filesystem paths
/// using environment variables `EM_DIR`, `HOME`, `TMPDIR`, and
/// `EM_USER_CONFIG_DIR`.
///
/// # Arguments
/// - `idt` — the directory type to resolve
/// - `prj` — project name (used by `Include`, `UserConfig`, `HostConfig`, `Res`)
/// - `sub_path` — optional sub-path appended to the result
///
/// # Errors
/// Returns `InstallInfoError::EnvNotSet` if a required environment variable is
/// missing. Returns `InstallInfoError::UnsupportedPlatform` on non-Linux.
#[cfg(target_os = "linux")]
pub fn emGetInstallPath(
    idt: InstallDirType,
    prj: &str,
    sub_path: Option<&str>,
) -> Result<PathBuf, InstallInfoError> {
    assert!(!prj.is_empty(), "project name must not be empty");

    let base = match idt {
        InstallDirType::Bin => em_dir()?.join("bin"),
        InstallDirType::Include => em_dir()?.join("include").join(prj),
        InstallDirType::Lib => em_dir()?.join("lib"),
        InstallDirType::HtmlDoc => em_dir()?.join("doc").join("html"),
        InstallDirType::PsDoc => em_dir()?.join("doc").join("ps"),
        InstallDirType::PdfDoc => em_dir()?.join("doc").join("pdf"),
        InstallDirType::UserConfig => user_config_dir()?.join(prj),
        InstallDirType::HostConfig => em_dir()?.join("etc").join(prj),
        InstallDirType::Tmp => tmp_dir(),
        InstallDirType::Res => em_dir()?.join("res").join(prj),
        InstallDirType::Home => home_dir()?,
    };

    let path = match sub_path {
        Some(sp) if !sp.is_empty() => base.join(sp),
        _ => base,
    };

    Ok(path)
}

#[cfg(not(target_os = "linux"))]
pub fn emGetInstallPath(
    _idt: InstallDirType,
    _prj: &str,
    _sub_path: Option<&str>,
) -> Result<PathBuf, InstallInfoError> {
    Err(InstallInfoError::UnsupportedPlatform)
}

/// Resolve a configuration directory, preferring user config if versions match.
///
/// Port of C++ `emGetConfigDirOverloadable`. Reads `version` files from both
/// the host and user configuration directories for `prj`. If both exist and
/// their contents match, returns the user config path; otherwise returns the
/// host config path.
///
/// On version mismatch, a warning is logged (the C++ shows a dialog, but
/// eaglemode-rs doesn't have dialog infrastructure wired yet).
///
/// # Arguments
/// - `prj` — project name
/// - `sub_dir` — optional sub-directory appended to the result
#[cfg(target_os = "linux")]
pub fn emGetConfigDirOverloadable(
    prj: &str,
    sub_dir: Option<&str>,
) -> Result<PathBuf, InstallInfoError> {
    let host_dir = emGetInstallPath(InstallDirType::HostConfig, prj, None)?;
    let user_dir = emGetInstallPath(InstallDirType::UserConfig, prj, None)?;

    let result_dir = match (read_version_file(&host_dir), read_version_file(&user_dir)) {
        (Some(host_ver), Some(user_ver)) => {
            if host_ver == user_ver {
                user_dir
            } else {
                log::warn!(
                    "Config version mismatch for {prj}: \
                     host config has version {host_ver:?}, \
                     user config has version {user_ver:?}. \
                     Using host config directory."
                );
                host_dir
            }
        }
        _ => host_dir,
    };

    let path = match sub_dir {
        Some(sd) if !sd.is_empty() => result_dir.join(sd),
        _ => result_dir,
    };

    Ok(path)
}

#[cfg(not(target_os = "linux"))]
pub fn emGetConfigDirOverloadable(
    _prj: &str,
    _sub_dir: Option<&str>,
) -> Result<PathBuf, InstallInfoError> {
    Err(InstallInfoError::UnsupportedPlatform)
}

// ── Private helpers ─────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn em_dir() -> Result<PathBuf, InstallInfoError> {
    std::env::var("EM_DIR")
        .map(PathBuf::from)
        .map_err(|_| InstallInfoError::EnvNotSet("EM_DIR".to_string()))
}

#[cfg(target_os = "linux")]
fn home_dir() -> Result<PathBuf, InstallInfoError> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| InstallInfoError::EnvNotSet("HOME".to_string()))
}

#[cfg(target_os = "linux")]
fn tmp_dir() -> PathBuf {
    std::env::var("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

#[cfg(target_os = "linux")]
fn user_config_dir() -> Result<PathBuf, InstallInfoError> {
    if let Ok(dir) = std::env::var("EM_USER_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = home_dir()?;
    // DIVERGED: (language-forced) C++ `emGetInstallPath` uses `.eaglemode`; see `UserConfig` doc.
    Ok(home.join(".eaglemode-rs"))
}

#[cfg(target_os = "linux")]
fn read_version_file(dir: &Path) -> Option<String> {
    let path = dir.join("version");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}
