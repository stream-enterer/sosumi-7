use std::cell::RefCell;
use std::rc::Rc;

use crate::emContext::emContext;
use crate::emSignal::SignalId;

/// Information about a physical monitor.
#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub scale_factor: f64,
    pub primary: bool,
}

/// Tracks available monitors and virtual desktop bounds.
///
/// Matches C++ emScreen: a model that provides monitor geometry, DPI, and
/// the list of open windows. Installed into a `emContext` so panels can
/// find it via `lookup_inherited`.
pub struct emScreen {
    monitors: Vec<MonitorInfo>,
    /// Virtual desktop bounding box (x, y, w, h).
    pub virtual_bounds: (i32, i32, u32, u32),
    /// Signal fired when monitor geometry changes.
    geometry_signal: SignalId,
    /// Signal fired when the window list changes.
    windows_signal: SignalId,
    /// Whether programmatic cursor warping is supported (X11 only).
    can_warp: bool,
}

impl emScreen {
    /// Populate from winit's available monitors.
    ///
    /// `geometry_signal` and `windows_signal` should be freshly allocated
    /// signal IDs from the scheduler.
    pub fn from_event_loop(
        event_loop: &winit::event_loop::ActiveEventLoop,
        geometry_signal: SignalId,
        windows_signal: SignalId,
    ) -> Self {
        let mut monitors = Vec::new();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        let mut first = true;
        for handle in event_loop.available_monitors() {
            let pos = handle.position();
            let size = handle.size();
            let scale = handle.scale_factor();
            let name = handle.name();

            let info = MonitorInfo {
                name,
                position: (pos.x, pos.y),
                size: (size.width, size.height),
                scale_factor: scale,
                primary: first,
            };
            first = false;

            min_x = min_x.min(pos.x);
            min_y = min_y.min(pos.y);
            max_x = max_x.max(pos.x + size.width as i32);
            max_y = max_y.max(pos.y + size.height as i32);

            monitors.push(info);
        }

        let virtual_bounds = if monitors.is_empty() {
            (0, 0, 1920, 1080)
        } else {
            (min_x, min_y, (max_x - min_x) as u32, (max_y - min_y) as u32)
        };

        let can_warp = std::env::var("WAYLAND_DISPLAY").is_err();

        Self {
            monitors,
            virtual_bounds,
            geometry_signal,
            windows_signal,
            can_warp,
        }
    }

    pub fn monitors(&self) -> &[MonitorInfo] {
        &self.monitors
    }

    pub fn primary(&self) -> Option<&MonitorInfo> {
        self.monitors.iter().find(|m| m.primary)
    }

    /// Return the DPI (dots per inch) of the primary monitor.
    ///
    /// Matches C++ emScreen::GetDPI (pure virtual). Uses the primary monitor's
    /// scale_factor to compute logical DPI. Returns 96.0 as the base DPI
    /// multiplied by the scale factor, following the convention that 1.0 scale
    /// = 96 DPI.
    pub fn GetDPI(&self) -> f64 {
        let scale = self.primary().map(|m| m.scale_factor).unwrap_or(1.0);
        96.0 * scale
    }

    /// Whether the mouse pointer can be moved programmatically.
    ///
    /// Matches C++ emScreen::CanMoveMousePointer. Returns true on X11
    /// where winit's set_cursor_position is supported.
    pub fn CanMoveMousePointer(&self) -> bool {
        self.can_warp
    }

    /// Move the mouse pointer by (dx, dy) pixels.
    ///
    /// DIVERGED: (language-forced) C++ emScreen inherits from emWindow and implements this directly.
    /// In Rust, emScreen is a monitor model without a window reference. The actual
    /// implementation is on `emWindow::MoveMousePointer` which uses winit's
    /// `set_cursor_position`. Callers with window access should use that instead.
    pub fn MoveMousePointer(&self, _dx: f64, _dy: f64) {
        // No-op: emScreen doesn't hold a window reference in Rust.
        // Use emWindow::MoveMousePointer for the working implementation.
    }

    /// Emit an acoustic warning beep via libcanberra (Linux) or no-op (other).
    ///
    /// Matches C++ emScreen::Beep.
    pub fn Beep(&self) {
        super::emWindowPlatform::system_beep();
    }

    /// Return the bounding rect (x, y, w, h) of monitor `index`.
    ///
    /// Matches C++ emScreen::GetMonitorRect. Falls back to primary monitor
    /// bounds (or virtual_bounds) if the index is out of range.
    pub fn GetMonitorRect(&self, index: usize) -> (f64, f64, f64, f64) {
        let m = self.monitors.get(index).or_else(|| self.primary());
        match m {
            Some(info) => (
                info.position.0 as f64,
                info.position.1 as f64,
                info.size.0 as f64,
                info.size.1 as f64,
            ),
            None => (
                self.virtual_bounds.0 as f64,
                self.virtual_bounds.1 as f64,
                self.virtual_bounds.2 as f64,
                self.virtual_bounds.3 as f64,
            ),
        }
    }

    /// Find the monitor with maximum overlap area with the given rect.
    pub fn GetMonitorIndexOfRect(&self, x: i32, y: i32, w: u32, h: u32) -> Option<usize> {
        let rx1 = x as i64;
        let ry1 = y as i64;
        let rx2 = rx1 + w as i64;
        let ry2 = ry1 + h as i64;

        let mut best_idx = None;
        let mut best_area: i64 = 0;

        for (i, m) in self.monitors.iter().enumerate() {
            let mx1 = m.position.0 as i64;
            let my1 = m.position.1 as i64;
            let mx2 = mx1 + m.size.0 as i64;
            let my2 = my1 + m.size.1 as i64;

            let ox = (rx2.min(mx2) - rx1.max(mx1)).max(0);
            let oy = (ry2.min(my2) - ry1.max(my1)).max(0);
            let area = ox * oy;

            if area > best_area {
                best_area = area;
                best_idx = Some(i);
            }
        }

        best_idx
    }

    /// emLook up the screen installed in the given context or an ancestor.
    ///
    /// Matches C++ emScreen::LookupInherited. The screen is registered
    /// under the type `emScreen` with name `""`.
    pub fn LookupInherited(context: &emContext) -> Option<Rc<RefCell<emScreen>>> {
        context.LookupInherited::<emScreen>("")
    }

    /// Signal fired when monitor geometry (bounds, DPI, count) changes.
    ///
    /// Matches C++ emScreen::GetGeometrySignal.
    pub fn GetGeometrySignal(&self) -> SignalId {
        self.geometry_signal
    }

    /// Signal fired when the set of open windows changes.
    ///
    /// Matches C++ emScreen::GetWindowsSignal.
    pub fn GetWindowsSignal(&self) -> SignalId {
        self.windows_signal
    }

    /// Register this screen in a `emContext` so it can be found via
    /// `lookup_inherited`.
    ///
    /// Matches C++ emScreen::Install (protected). Should be called once
    /// on the root context at startup.
    pub fn Install(screen: Rc<RefCell<emScreen>>, context: &emContext) {
        context.register_model::<emScreen>("", screen);
    }

    /// Fire the geometry-changed signal.
    ///
    /// Matches C++ emScreen::SignalGeometrySignal (protected).
    /// The caller must pass this signal ID to the scheduler to actually
    /// fire it.
    pub fn SignalGeometrySignal(&self) -> SignalId {
        self.geometry_signal
    }

    /// Stub: create a window port for a new window.
    ///
    /// Matches C++ emScreen::CreateWindowPort (pure virtual, protected).
    /// In eaglemode-rs the window port is created directly by `emWindow::create`,
    /// so this is a no-op placeholder for API parity.
    pub fn CreateWindowPort(&self) {
        // No-op: window ports are created inline by emWindow::create.
    }

    pub fn LeaveFullscreenModes(
        &self,
        windows: &mut std::collections::HashMap<winit::window::WindowId, super::emWindow::emWindow>,
        except: Option<winit::window::WindowId>,
    ) {
        use crate::emWindow::WindowFlags;

        for (id, win) in windows.iter_mut() {
            if win.flags.contains(WindowFlags::FULLSCREEN) && Some(*id) != except {
                let new_flags = win.flags & !WindowFlags::FULLSCREEN;
                win.SetWindowFlags(new_flags);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_screen(monitors: Vec<MonitorInfo>) -> emScreen {
        use slotmap::SlotMap;
        // Create dummy signal IDs for tests.
        let mut signals: SlotMap<SignalId, ()> = SlotMap::with_key();
        let gs = signals.insert(());
        let ws = signals.insert(());
        emScreen {
            monitors,
            virtual_bounds: (0, 0, 3840, 1080),
            geometry_signal: gs,
            windows_signal: ws,
            can_warp: true,
        }
    }

    #[test]
    fn monitor_index_of_rect_single() {
        let screen = make_screen(vec![MonitorInfo {
            name: None,
            position: (0, 0),
            size: (1920, 1080),
            scale_factor: 1.0,
            primary: true,
        }]);
        assert_eq!(screen.GetMonitorIndexOfRect(100, 100, 200, 200), Some(0));
    }

    #[test]
    fn monitor_index_of_rect_picks_max_overlap() {
        let screen = make_screen(vec![
            MonitorInfo {
                name: None,
                position: (0, 0),
                size: (1920, 1080),
                scale_factor: 1.0,
                primary: true,
            },
            MonitorInfo {
                name: None,
                position: (1920, 0),
                size: (1920, 1080),
                scale_factor: 1.0,
                primary: false,
            },
        ]);
        // Mostly on monitor 1 (right)
        assert_eq!(screen.GetMonitorIndexOfRect(1900, 0, 200, 100), Some(1));
        // Mostly on monitor 0 (left)
        assert_eq!(screen.GetMonitorIndexOfRect(1800, 0, 200, 100), Some(0));
    }

    #[test]
    fn monitor_index_of_rect_no_overlap() {
        let screen = make_screen(vec![MonitorInfo {
            name: None,
            position: (0, 0),
            size: (1920, 1080),
            scale_factor: 1.0,
            primary: true,
        }]);
        assert_eq!(screen.GetMonitorIndexOfRect(2000, 2000, 100, 100), None);
    }

    #[test]
    fn monitor_index_of_rect_empty_monitors() {
        let screen = make_screen(vec![]);
        assert_eq!(screen.GetMonitorIndexOfRect(0, 0, 100, 100), None);
    }
}
