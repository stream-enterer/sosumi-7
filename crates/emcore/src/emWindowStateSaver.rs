use std::path::PathBuf;

use winit::window::WindowId;

use crate::emConfigModel::emConfigModel;
use crate::emEngine::{emEngine, EngineCtx};
use crate::emRec::{RecError, RecStruct};
use crate::emRecRecord::Record;
use crate::emSignal::SignalId;
use crate::emWindow::WindowFlags;

/// Persisted window state matching C++ emWindowStateSaver::ModelClass fields.
///
/// Uses f64 for coordinates (ViewX/ViewY/ViewWidth/ViewHeight) to match the
/// C++ emDoubleRec members.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowGeometry {
    pub ViewX: f64,
    pub ViewY: f64,
    pub ViewWidth: f64,
    pub ViewHeight: f64,
    pub Maximized: bool,
    pub Fullscreen: bool,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            ViewX: 100.0,
            ViewY: 100.0,
            ViewWidth: 1280.0,
            ViewHeight: 720.0,
            Maximized: false,
            Fullscreen: false,
        }
    }
}

impl Record for WindowGeometry {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        Ok(Self {
            ViewX: rec
                .get_double("ViewX")
                .ok_or_else(|| RecError::MissingField("ViewX".into()))?,
            ViewY: rec
                .get_double("ViewY")
                .ok_or_else(|| RecError::MissingField("ViewY".into()))?,
            ViewWidth: rec
                .get_double("ViewWidth")
                .ok_or_else(|| RecError::MissingField("ViewWidth".into()))?,
            ViewHeight: rec
                .get_double("ViewHeight")
                .ok_or_else(|| RecError::MissingField("ViewHeight".into()))?,
            Maximized: rec.get_bool("Maximized").unwrap_or(false),
            Fullscreen: rec.get_bool("Fullscreen").unwrap_or(false),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();
        s.set_double("ViewX", self.ViewX);
        s.set_double("ViewY", self.ViewY);
        s.set_double("ViewWidth", self.ViewWidth);
        s.set_double("ViewHeight", self.ViewHeight);
        s.set_bool("Maximized", self.Maximized);
        s.set_bool("Fullscreen", self.Fullscreen);
        s
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

/// Saves and restores window geometry via an emConfigModel.
///
/// Port of C++ emWindowStateSaver (emWindowStateSaver.h/cpp). An engine
/// that listens to window flags, geometry, and focus signals. When the
/// window is focused and state changes, it saves geometry to an emRec
/// config file. On construction it restores saved geometry.
pub struct emWindowStateSaver {
    model: emConfigModel<WindowGeometry>,
    window_id: WindowId,
    flags_signal: SignalId,
    focus_signal: SignalId,
    geometry_signal: SignalId,
    AllowRestoreFullscreen: bool,
    /// Cached normal-mode geometry, preserved when maximized/fullscreen.
    /// Matches C++ OwnNormalX/Y/W/H.
    OwnNormalX: f64,
    OwnNormalY: f64,
    OwnNormalW: f64,
    OwnNormalH: f64,
}

impl emWindowStateSaver {
    /// Create a new window state saver.
    ///
    /// Port of C++ emWindowStateSaver constructor. Loads or creates the
    /// config file at `file_path`, then restores geometry to the window.
    ///
    /// Arguments:
    /// - `window_id`: The window whose state is saved/restored.
    /// - `file_path`: Path to the emRec config file.
    /// - `flags_signal`: Window's flags signal (GetWindowFlagsSignal).
    /// - `focus_signal`: Window's focus signal (GetFocusSignal).
    /// - `geometry_signal`: Window's geometry signal (GetGeometrySignal).
    /// - `allow_restore_fullscreen`: Whether to restore fullscreen mode.
    /// - `change_signal`: Signal ID for the config model's change tracking.
    pub fn new(
        window_id: WindowId,
        file_path: PathBuf,
        flags_signal: SignalId,
        focus_signal: SignalId,
        geometry_signal: SignalId,
        allow_restore_fullscreen: bool,
        change_signal: SignalId,
    ) -> Self {
        let defaults = WindowGeometry::default();
        let mut model = emConfigModel::new(defaults, file_path, change_signal)
            .with_format_name("emWindowState");

        // Load or install config (C++ PostConstruct + LoadOrInstall).
        if let Err(e) = model.TryLoadOrInstall() {
            log::warn!("emWindowStateSaver: failed to load config: {e}");
        }

        let rec = model.GetRec();
        Self {
            OwnNormalX: rec.ViewX,
            OwnNormalY: rec.ViewY,
            OwnNormalW: rec.ViewWidth,
            OwnNormalH: rec.ViewHeight,
            model,
            window_id,
            flags_signal,
            focus_signal,
            geometry_signal,
            AllowRestoreFullscreen: allow_restore_fullscreen,
        }
    }

    /// Restore saved geometry to the window.
    ///
    /// Port of C++ emWindowStateSaver::Restore. Validates saved geometry
    /// against monitor bounds, then applies position, size, and window flags.
    pub fn Restore(
        &self,
        window: &mut crate::emWindow::emWindow,
        screen: &crate::emScreen::emScreen,
    ) {
        let rec = self.model.GetRec();

        let x = rec.ViewX;
        let y = rec.ViewY;
        let mut w = rec.ViewWidth;
        let mut h = rec.ViewHeight;
        let maximized = rec.Maximized;
        let fullscreen = self.AllowRestoreFullscreen && rec.Fullscreen;

        let size_valid = w >= 32.0 && h >= 32.0;
        let mut pos_valid = false;

        if size_valid {
            // Determine monitor for maximized/fullscreen placement.
            let monitor = if maximized || fullscreen {
                screen
                    .GetMonitorIndexOfRect(x as i32, y as i32, w as u32, h as u32)
                    .unwrap_or(0)
            } else {
                0
            };

            let (mx, my, mw, mh) = screen.GetMonitorRect(monitor);
            let (bl, bt, br, bb) = window.GetBorderSizes();
            let bl = bl as f64;
            let bt = bt as f64;
            let br = br as f64;
            let bb = bb as f64;

            if w > mw - bl - br {
                w = mw - bl - br;
            }
            if h > mh - bt - bb {
                h = mh - bt - bb;
            }

            if w >= 32.0 && h >= 32.0 {
                // Check that at least 95% of the window is visible on the monitor.
                let cw = (x + w).min(mx + mw) - x.max(mx);
                let ch = (y + h).min(my + mh) - y.max(my);
                let area = cw.max(0.0) * ch.max(0.0);
                pos_valid = area >= w * h * 0.95;
            }
        }

        // Apply position for maximized/fullscreen (C++ emWindowStateSaver.cpp:135).
        if pos_valid && (maximized || fullscreen) {
            window.SetViewPos(x, y);
        }

        // Apply size if valid.
        if size_valid && w >= 32.0 && h >= 32.0 {
            window.SetViewSize(w, h);
        }

        // Apply window flags.
        let mut flags = window.flags;
        if maximized {
            flags |= WindowFlags::MAXIMIZED;
        } else {
            flags -= WindowFlags::MAXIMIZED;
        }
        if fullscreen {
            flags |= WindowFlags::FULLSCREEN;
        } else {
            flags -= WindowFlags::FULLSCREEN;
        }
        window.SetWindowFlags(flags);
    }

    /// Get the stored geometry record.
    pub fn model(&self) -> &emConfigModel<WindowGeometry> {
        &self.model
    }
}

/// Port of C++ emWindowStateSaver::Cycle.
///
/// Wakes on flags, geometry, or focus signals. If the window is focused,
/// saves current geometry.
impl emEngine for emWindowStateSaver {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        let signaled = ctx.IsSignaled(self.flags_signal)
            || ctx.IsSignaled(self.focus_signal)
            || ctx.IsSignaled(self.geometry_signal);

        if signaled {
            if let Some(window) = ctx.windows.get(&self.window_id).map(|rc| rc.borrow()) {
                if window.view().IsFocused() {
                    // Need mutable self for Save, but we only need an immutable
                    // window reference. Clone the relevant data to avoid borrow conflict.
                    let flags = window.flags;
                    let pos = window.winit_window().outer_position().unwrap_or_default();
                    let size = window.winit_window().inner_size();

                    if !flags.contains(WindowFlags::MAXIMIZED)
                        && !flags.contains(WindowFlags::FULLSCREEN)
                    {
                        self.OwnNormalX = pos.x as f64;
                        self.OwnNormalY = pos.y as f64;
                        self.OwnNormalW = size.width as f64;
                        self.OwnNormalH = size.height as f64;
                    }

                    let geo = WindowGeometry {
                        ViewX: self.OwnNormalX,
                        ViewY: self.OwnNormalY,
                        ViewWidth: self.OwnNormalW,
                        ViewHeight: self.OwnNormalH,
                        Maximized: flags.contains(WindowFlags::MAXIMIZED),
                        Fullscreen: flags.contains(WindowFlags::FULLSCREEN),
                    };
                    self.model.Set(geo);

                    if let Err(e) = self.model.Save() {
                        log::warn!("emWindowStateSaver: failed to save config: {e}");
                    }
                }
            }
        }

        false
    }
}

#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_WindowGeometry_IsSetToDefault() {
        let mut self_val = WindowGeometry {
            ViewX: kani::any::<f64>(),
            ViewY: kani::any::<f64>(),
            ViewWidth: kani::any::<f64>(),
            ViewHeight: kani::any::<f64>(),
            Maximized: kani::any::<bool>(),
            Fullscreen: kani::any::<bool>(),
        };
        let _r = self_val.IsSetToDefault();
    }

    #[kani::proof]
    fn kani_private_WindowGeometry_SetToDefault() {
        let mut self_val = WindowGeometry {
            ViewX: kani::any::<f64>(),
            ViewY: kani::any::<f64>(),
            ViewWidth: kani::any::<f64>(),
            ViewHeight: kani::any::<f64>(),
            Maximized: kani::any::<bool>(),
            Fullscreen: kani::any::<bool>(),
        };
        let _r = self_val.SetToDefault();
    }
}
