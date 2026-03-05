use std::path::PathBuf;

use crate::model::{ConfigError, ConfigModel, Record};
use crate::scheduler::SignalId;

/// Persisted window geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 1280,
            height: 720,
            maximized: false,
        }
    }
}

impl Record for WindowGeometry {
    fn from_kdl(node: &kdl::KdlNode) -> Result<Self, ConfigError> {
        let x = node
            .get("x")
            .and_then(|e| e.as_integer())
            .map(|v| v as i32)
            .ok_or_else(|| ConfigError::MissingField("x".into()))?;
        let y = node
            .get("y")
            .and_then(|e| e.as_integer())
            .map(|v| v as i32)
            .ok_or_else(|| ConfigError::MissingField("y".into()))?;
        let width = node
            .get("width")
            .and_then(|e| e.as_integer())
            .map(|v| v as u32)
            .ok_or_else(|| ConfigError::MissingField("width".into()))?;
        let height = node
            .get("height")
            .and_then(|e| e.as_integer())
            .map(|v| v as u32)
            .ok_or_else(|| ConfigError::MissingField("height".into()))?;
        let maximized = node
            .get("maximized")
            .and_then(|e| e.as_bool())
            .unwrap_or(false);

        Ok(Self {
            x,
            y,
            width,
            height,
            maximized,
        })
    }

    fn to_kdl(&self) -> kdl::KdlNode {
        let mut node = kdl::KdlNode::new("window-geometry");
        node.push(kdl::KdlEntry::new_prop("x", self.x as i128));
        node.push(kdl::KdlEntry::new_prop("y", self.y as i128));
        node.push(kdl::KdlEntry::new_prop("width", self.width as i128));
        node.push(kdl::KdlEntry::new_prop("height", self.height as i128));
        node.push(kdl::KdlEntry::new_prop("maximized", self.maximized));
        node
    }

    fn set_to_default(&mut self) {
        *self = Self::default();
    }

    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Saves and restores window geometry via a ConfigModel.
pub struct WindowStateSaver {
    model: ConfigModel<WindowGeometry>,
}

impl WindowStateSaver {
    pub fn new(path: PathBuf, signal_id: SignalId) -> Self {
        Self {
            model: ConfigModel::new(WindowGeometry::default(), path, signal_id),
        }
    }

    /// Save the current window position/size.
    pub fn save_from(&mut self, window: &super::zui_window::ZuiWindow) {
        let pos = window.winit_window.outer_position().unwrap_or_default();
        let size = window.winit_window.inner_size();
        let maximized = window.winit_window.is_maximized();

        self.model.set(WindowGeometry {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
            maximized,
        });
    }

    /// Get the stored geometry for restoring.
    pub fn geometry(&self) -> &WindowGeometry {
        self.model.get()
    }

    pub fn model(&self) -> &ConfigModel<WindowGeometry> {
        &self.model
    }
}
