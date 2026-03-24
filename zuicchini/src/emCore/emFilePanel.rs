use crate::emCore::emColor::emColor;
use crate::emCore::emFileModel::FileState;
use crate::emCore::emPanel::{PanelBehavior, PanelState};
use crate::emCore::emPainter::{emPainter, TextAlignment, VAlign};

/// Extended file state for a file panel, adding custom error and no-model states.
///
/// Port of C++ `emFilePanel::VirtualFileState`. This augments `FileState` with
/// two additional states: `NoFileModel` (no model is attached) and
/// `CustomError` (a custom error message overrides the model state).
#[derive(Clone, Debug, PartialEq)]
pub enum VirtualFileState {
    Waiting,
    Loading { progress: f64 },
    Loaded,
    Unsaved,
    Saving,
    TooCostly,
    LoadError(String),
    SaveError(String),
    NoFileModel,
    CustomError(String),
}

impl VirtualFileState {
    /// Whether the state represents usable content (loaded or unsaved).
    pub fn is_good(&self) -> bool {
        matches!(self, Self::Loaded | Self::Unsaved)
    }

    /// Whether the panel should show a progress/waiting animation.
    pub fn IsHopeForSeeking(&self) -> bool {
        matches!(self, Self::Waiting | Self::Loading { .. } | Self::Saving)
    }
}

/// A panel that displays a file model's content (loading state, error display).
///
/// Port of C++ `emFilePanel`. Observes a `FileState` and paints status
/// information. Derived types should override `paint` to render the actual
/// content when the virtual file state is good.
pub struct emFilePanel {
    file_state: FileState,
    error_text: String,
    memory_need: u64,
    memory_limit: u64,
    custom_error: Option<String>,
    has_model: bool,
}

impl Default for emFilePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl emFilePanel {
    pub fn new() -> Self {
        Self {
            file_state: FileState::Waiting,
            error_text: String::new(),
            memory_need: 0,
            memory_limit: u64::MAX,
            custom_error: None,
            has_model: false,
        }
    }

    /// Create a file panel with a model attached.
    pub fn with_model() -> Self {
        Self {
            file_state: FileState::Waiting,
            error_text: String::new(),
            memory_need: 0,
            memory_limit: u64::MAX,
            custom_error: None,
            has_model: true,
        }
    }

    pub fn GetFileModel(&self) -> bool {
        self.has_model
    }

    pub fn SetFileModel(&mut self, has: bool) {
        self.has_model = has;
    }

    pub fn GetFileState(&self) -> &FileState {
        &self.file_state
    }

    pub fn set_file_state(&mut self, state: FileState) {
        self.file_state = state;
    }

    pub fn GetErrorText(&self) -> &str {
        &self.error_text
    }

    pub fn set_error_text(&mut self, text: &str) {
        self.error_text = text.to_string();
    }

    pub fn GetMemoryNeed(&self) -> u64 {
        self.memory_need
    }

    pub fn set_memory_need(&mut self, need: u64) {
        self.memory_need = need;
    }

    pub fn GetMemoryLimit(&self) -> u64 {
        self.memory_limit
    }

    pub fn set_memory_limit(&mut self, limit: u64) {
        self.memory_limit = limit;
    }

    pub fn set_custom_error(&mut self, message: &str) {
        self.custom_error = Some(message.to_string());
    }

    pub fn clear_custom_error(&mut self) {
        self.custom_error = None;
    }

    pub fn GetCustomError(&self) -> Option<&str> {
        self.custom_error.as_deref()
    }

    /// Compute the virtual file state from current model state and custom error.
    pub fn GetVirFileState(&self) -> VirtualFileState {
        if let Some(ref msg) = self.custom_error {
            return VirtualFileState::CustomError(msg.clone());
        }
        if !self.has_model {
            return VirtualFileState::NoFileModel;
        }
        // If memory need exceeds limit and model is loaded, force TooCostly.
        if self.memory_need > self.memory_limit {
            return VirtualFileState::TooCostly;
        }
        match &self.file_state {
            FileState::Waiting => VirtualFileState::Waiting,
            FileState::Loading { progress } => VirtualFileState::Loading {
                progress: *progress,
            },
            FileState::Loaded => VirtualFileState::Loaded,
            FileState::Unsaved => VirtualFileState::Unsaved,
            FileState::Saving => VirtualFileState::Saving,
            FileState::TooCostly => VirtualFileState::TooCostly,
            FileState::LoadError(e) => VirtualFileState::LoadError(e.clone()),
            FileState::SaveError(e) => VirtualFileState::SaveError(e.clone()),
        }
    }

    /// Paint the file panel status information.
    ///
    /// This renders informational text about the current virtual file state.
    /// Derived panels should check `vir_file_state().is_good()` and render
    /// their content instead of calling this method when the state is good.
    pub fn paint_status(&self, painter: &mut emPainter, w: f64, h: f64) {
        let canvas_color = painter.GetCanvasColor();
        let vfs = self.GetVirFileState();

        match &vfs {
            VirtualFileState::Waiting => {
                paint_status_text(
                    painter,
                    w,
                    h,
                    "Wait...",
                    emColor::rgba(92, 92, 0, 192),
                    canvas_color,
                );
            }
            VirtualFileState::Loading { progress } => {
                let text = format!("Loading: {:.1}%", progress);
                paint_status_text(
                    painter,
                    w,
                    h,
                    &text,
                    emColor::rgba(0, 112, 0, 192),
                    canvas_color,
                );
            }
            VirtualFileState::Loaded => {
                paint_status_text(
                    painter,
                    w,
                    h,
                    "Loaded",
                    emColor::rgba(0, 116, 112, 192),
                    canvas_color,
                );
            }
            VirtualFileState::Unsaved => {
                paint_status_text(
                    painter,
                    w,
                    h,
                    "Unsaved",
                    emColor::rgba(144, 0, 144, 192),
                    canvas_color,
                );
            }
            VirtualFileState::Saving => {
                paint_status_text(
                    painter,
                    w,
                    h,
                    "Saving...",
                    emColor::rgba(0, 112, 0, 192),
                    canvas_color,
                );
            }
            VirtualFileState::TooCostly => {
                paint_status_text(
                    painter,
                    w,
                    h,
                    "Costly",
                    emColor::rgba(112, 64, 64, 192),
                    canvas_color,
                );
            }
            VirtualFileState::LoadError(ref _e) => {
                let bg = emColor::rgb(128, 0, 0);
                painter.PaintRect(0.0, 0.0, w, h, bg, canvas_color);
                painter.PaintTextBoxed(
                    0.05 * w,
                    h * 0.15,
                    0.9 * w,
                    h * 0.1,
                    "Loading Failed",
                    h * 0.1,
                    emColor::rgb(204, 136, 0),
                    bg,
                    TextAlignment::Center,
                    VAlign::Center,
                    TextAlignment::Left,
                    1.0,
                    false,
                    0.0,
                );
                painter.PaintTextBoxed(
                    0.05 * w,
                    h * 0.3,
                    0.9 * w,
                    h * 0.4,
                    &self.error_text,
                    h * 0.4,
                    emColor::rgb(255, 255, 0),
                    bg,
                    TextAlignment::Center,
                    VAlign::Center,
                    TextAlignment::Left,
                    1.0,
                    false,
                    0.0,
                );
            }
            VirtualFileState::SaveError(ref _e) => {
                let bg = emColor::rgb(128, 0, 0);
                painter.PaintRect(0.0, 0.0, w, h, bg, canvas_color);
                painter.PaintTextBoxed(
                    0.05 * w,
                    h * 0.15,
                    0.9 * w,
                    h * 0.3,
                    "Saving Failed",
                    h * 0.3,
                    emColor::rgb(255, 0, 0),
                    bg,
                    TextAlignment::Center,
                    VAlign::Center,
                    TextAlignment::Left,
                    1.0,
                    false,
                    0.0,
                );
                painter.PaintTextBoxed(
                    0.05 * w,
                    h * 0.5,
                    0.9 * w,
                    h * 0.3,
                    &self.error_text,
                    h * 0.3,
                    emColor::rgb(255, 255, 0),
                    bg,
                    TextAlignment::Center,
                    VAlign::Center,
                    TextAlignment::Left,
                    1.0,
                    false,
                    0.0,
                );
            }
            VirtualFileState::CustomError(ref msg) => {
                let bg = emColor::rgb(128, 0, 0);
                painter.PaintRect(0.0, 0.0, w, h, bg, canvas_color);
                painter.PaintTextBoxed(
                    0.05 * w,
                    h * 0.15,
                    0.9 * w,
                    h * 0.2,
                    "Error",
                    h * 0.2,
                    emColor::rgb(221, 0, 0),
                    bg,
                    TextAlignment::Center,
                    VAlign::Center,
                    TextAlignment::Left,
                    1.0,
                    false,
                    0.0,
                );
                painter.PaintTextBoxed(
                    0.05 * w,
                    h * 0.45,
                    0.9 * w,
                    h * 0.3,
                    msg,
                    h * 0.4,
                    emColor::rgb(255, 255, 0),
                    bg,
                    TextAlignment::Center,
                    VAlign::Center,
                    TextAlignment::Left,
                    1.0,
                    false,
                    0.0,
                );
            }
            VirtualFileState::NoFileModel => {
                paint_status_text(
                    painter,
                    w,
                    h,
                    "No file model",
                    emColor::rgba(128, 0, 0, 192),
                    canvas_color,
                );
            }
        }
    }

}

/// Paint a centered status text over the full panel area.
fn paint_status_text(
    painter: &mut emPainter,
    w: f64,
    h: f64,
    text: &str,
    color: emColor,
    canvas_color: emColor,
) {
    painter.PaintTextBoxed(
        0.0,
        0.0,
        w,
        h,
        text,
        h / 6.0,
        color,
        canvas_color,
        TextAlignment::Center,
        VAlign::Center,
        TextAlignment::Left,
        1.0,
        false,
        0.0,
    );
}

impl PanelBehavior for emFilePanel {
    fn IsOpaque(&self) -> bool {
        matches!(
            self.GetVirFileState(),
            VirtualFileState::LoadError(_)
                | VirtualFileState::SaveError(_)
                | VirtualFileState::CustomError(_)
        )
    }

    fn GetCanvasColor(&self) -> emColor {
        match self.GetVirFileState() {
            VirtualFileState::LoadError(_)
            | VirtualFileState::SaveError(_)
            | VirtualFileState::CustomError(_) => emColor::rgb(128, 0, 0),
            _ => emColor::TRANSPARENT,
        }
    }

    fn GetIconFileName(&self) -> Option<String> {
        Some("file.tga".to_string())
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        self.paint_status(painter, w, h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vfs_no_model() {
        let panel = emFilePanel::new();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::NoFileModel);
    }

    #[test]
    fn vfs_with_model_waiting() {
        let panel = emFilePanel::with_model();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Waiting);
    }

    #[test]
    fn vfs_custom_error_overrides() {
        let mut panel = emFilePanel::with_model();
        panel.set_file_state(FileState::Loaded);
        panel.set_custom_error("custom problem");
        assert_eq!(
            panel.GetVirFileState(),
            VirtualFileState::CustomError("custom problem".to_string())
        );
        panel.clear_custom_error();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Loaded);
    }

    #[test]
    fn vfs_too_costly_when_over_limit() {
        let mut panel = emFilePanel::with_model();
        panel.set_file_state(FileState::Loaded);
        panel.set_memory_need(1000);
        panel.set_memory_limit(500);
        assert_eq!(panel.GetVirFileState(), VirtualFileState::TooCostly);
    }

    #[test]
    fn vfs_good_states() {
        let mut panel = emFilePanel::with_model();
        panel.set_file_state(FileState::Loaded);
        assert!(panel.GetVirFileState().is_good());

        panel.set_file_state(FileState::Unsaved);
        assert!(panel.GetVirFileState().is_good());

        panel.set_file_state(FileState::Waiting);
        assert!(!panel.GetVirFileState().is_good());
    }

    #[test]
    fn is_opaque_for_errors() {
        let mut panel = emFilePanel::with_model();
        panel.set_file_state(FileState::LoadError("err".to_string()));
        assert!(panel.IsOpaque());

        panel.set_file_state(FileState::Loaded);
        assert!(!panel.IsOpaque());
    }

    #[test]
    fn hope_for_seeking() {
        assert!(VirtualFileState::Waiting.IsHopeForSeeking());
        assert!(VirtualFileState::Loading { progress: 50.0 }.IsHopeForSeeking());
        assert!(VirtualFileState::Saving.IsHopeForSeeking());
        assert!(!VirtualFileState::Loaded.IsHopeForSeeking());
    }

    #[test]
    fn vfs_all_states_map() {
        let mut panel = emFilePanel::with_model();

        panel.set_file_state(FileState::Waiting);
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Waiting);

        panel.set_file_state(FileState::Loading { progress: 50.0 });
        assert_eq!(
            panel.GetVirFileState(),
            VirtualFileState::Loading { progress: 50.0 }
        );

        panel.set_file_state(FileState::Loaded);
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Loaded);

        panel.set_file_state(FileState::Unsaved);
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Unsaved);

        panel.set_file_state(FileState::Saving);
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Saving);

        panel.set_file_state(FileState::TooCostly);
        assert_eq!(panel.GetVirFileState(), VirtualFileState::TooCostly);

        panel.set_file_state(FileState::LoadError("e".to_string()));
        assert_eq!(
            panel.GetVirFileState(),
            VirtualFileState::LoadError("e".to_string())
        );

        panel.set_file_state(FileState::SaveError("e".to_string()));
        assert_eq!(
            panel.GetVirFileState(),
            VirtualFileState::SaveError("e".to_string())
        );
    }

    #[test]
    fn canvas_color_error_states() {
        let error_color = emColor::rgb(128, 0, 0);

        let mut panel = emFilePanel::with_model();
        panel.set_file_state(FileState::LoadError("err".to_string()));
        assert_eq!(panel.GetCanvasColor(), error_color);

        panel.set_file_state(FileState::SaveError("err".to_string()));
        assert_eq!(panel.GetCanvasColor(), error_color);

        panel.set_custom_error("custom");
        assert_eq!(panel.GetCanvasColor(), error_color);

        panel.clear_custom_error();
        panel.set_file_state(FileState::Loaded);
        assert_eq!(panel.GetCanvasColor(), emColor::TRANSPARENT);
    }

    #[test]
    fn custom_error_priority() {
        // Custom error overrides TooCostly + memory limit exceeded.
        let mut panel = emFilePanel::with_model();
        panel.set_file_state(FileState::TooCostly);
        panel.set_memory_need(1000);
        panel.set_memory_limit(500);
        panel.set_custom_error("msg");
        assert_eq!(
            panel.GetVirFileState(),
            VirtualFileState::CustomError("msg".to_string())
        );

        // Custom error overrides even NoFileModel.
        let mut panel = emFilePanel::new();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::NoFileModel);
        panel.set_custom_error("msg");
        assert_eq!(
            panel.GetVirFileState(),
            VirtualFileState::CustomError("msg".to_string())
        );
    }

    #[test]
    fn memory_accessors_roundtrip() {
        let mut panel = emFilePanel::new();
        panel.set_memory_need(42);
        assert_eq!(panel.GetMemoryNeed(), 42);

        panel.set_memory_limit(100);
        assert_eq!(panel.GetMemoryLimit(), 100);
    }
}
