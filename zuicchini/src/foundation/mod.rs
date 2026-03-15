mod alignment;
mod at_matrix;
mod checksum;
mod clip_rects;
mod color;
mod em_rec;
mod fixed;
mod image;
mod install_info;
mod mini_ipc;
mod process;
mod rect;
mod tga;

pub use alignment::ContentAlignment;
pub use at_matrix::AffineMatrix;
pub use checksum::{calc_adler32, calc_crc32, calc_hash_code};
pub use clip_rects::{ClipRect, ClipRects};
pub use color::{Color, ColorParseError};
pub use em_rec::{
    parse_rec, parse_rec_with_format, write_rec, write_rec_with_format, RecError, RecStruct,
    RecValue,
};
pub use fixed::Fixed12;
pub use image::Image;
pub use install_info::{
    get_config_dir_overloadable, get_install_path, InstallDirType, InstallInfoError,
};
#[cfg(target_os = "linux")]
pub use mini_ipc::MiniIpcServer;
pub use mini_ipc::{decode_message, encode_message, MiniIpcClient, MiniIpcError};
pub use process::{PipeResult, Process, ProcessError, StartFlags};
pub use rect::{PixelRect, Rect};
pub use tga::{load_tga, TgaError};

use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag: whether fatal errors should be displayed graphically.
static FATAL_ERROR_GRAPHICAL: AtomicBool = AtomicBool::new(false);

/// Enable or disable graphical display of fatal errors.
///
/// Matches C++ emSetFatalErrorGraphical. When enabled, a future fatal-error
/// handler could show a dialog instead of just logging to stderr.
/// Currently only stores the flag; no graphical dialog is implemented yet.
pub fn set_fatal_error_graphical(enable: bool) {
    FATAL_ERROR_GRAPHICAL.store(enable, Ordering::Relaxed);
}

/// Query whether fatal errors should be displayed graphically.
pub fn is_fatal_error_graphical() -> bool {
    FATAL_ERROR_GRAPHICAL.load(Ordering::Relaxed)
}
