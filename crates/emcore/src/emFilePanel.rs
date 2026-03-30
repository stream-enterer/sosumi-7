use std::cell::RefCell;
use std::rc::Rc;

use crate::emColor::emColor;
use crate::emFileModel::{FileModelState, FileState};
use crate::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use crate::emPanelCtx::PanelCtx;
use crate::emPainter::{emPainter, TextAlignment, VAlign};

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
/// Port of C++ `emFilePanel`. Observes a `FileModelState` and paints status
/// information. Derived types should override `paint` to render the actual
/// content when the virtual file state is good.
pub struct emFilePanel {
    model: Option<Rc<RefCell<dyn FileModelState>>>,
    custom_error: Option<String>,
    last_vir_file_state: VirtualFileState,
    pub(crate) cached_memory_limit: u64,
    pub(crate) cached_priority: f64,
    pub(crate) cached_in_active_path: bool,
}

impl Default for emFilePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl emFilePanel {
    pub fn new() -> Self {
        Self {
            model: None,
            custom_error: None,
            last_vir_file_state: VirtualFileState::NoFileModel,
            cached_memory_limit: u64::MAX,
            cached_priority: 0.0,
            cached_in_active_path: false,
        }
    }

    /// Port of C++ emFilePanel::SetFileModel.
    pub fn SetFileModel(&mut self, model: Option<Rc<RefCell<dyn FileModelState>>>) {
        self.model = model;
        let new_state = self.compute_vir_file_state();
        self.last_vir_file_state = new_state;
    }

    /// Whether a model is attached.
    pub fn GetFileModel(&self) -> bool {
        self.model.is_some()
    }

    pub fn set_custom_error(&mut self, message: &str) {
        self.custom_error = Some(message.to_string());
        self.last_vir_file_state = self.compute_vir_file_state();
    }

    pub fn clear_custom_error(&mut self) {
        self.custom_error = None;
        self.last_vir_file_state = self.compute_vir_file_state();
    }

    pub fn GetCustomError(&self) -> Option<&str> {
        self.custom_error.as_deref()
    }


    /// Return the cached virtual file state.
    pub fn GetVirFileState(&self) -> VirtualFileState {
        self.last_vir_file_state.clone()
    }

    /// Re-compute VirtualFileState from model. Called after model state changes
    /// in tests; in production, Cycle() does this.
    pub fn refresh_vir_file_state(&mut self) {
        self.last_vir_file_state = self.compute_vir_file_state();
    }

    fn compute_vir_file_state(&self) -> VirtualFileState {
        if let Some(ref msg) = self.custom_error {
            return VirtualFileState::CustomError(msg.clone());
        }
        let Some(ref model_rc) = self.model else {
            return VirtualFileState::NoFileModel;
        };
        let model = model_rc.borrow();
        let memory_need = model.get_memory_need();
        if memory_need > self.cached_memory_limit {
            return VirtualFileState::TooCostly;
        }
        match model.GetFileState() {
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

    /// Inner cycle logic. Returns true if VirtualFileState changed.
    /// Port of C++ emFilePanel::Cycle.
    pub(crate) fn cycle_inner(&mut self) -> bool {
        let new_state = self.compute_vir_file_state();
        if new_state != self.last_vir_file_state {
            self.last_vir_file_state = new_state;
            true
        } else {
            false
        }
    }

    /// Port of C++ emFilePanel::IsContentReady.
    /// Returns (ready, readying).
    pub fn IsContentReady(&self) -> (bool, bool) {
        match &self.last_vir_file_state {
            VirtualFileState::Waiting
            | VirtualFileState::Loading { .. }
            | VirtualFileState::Saving => (false, true),
            VirtualFileState::Loaded | VirtualFileState::Unsaved => (true, false),
            _ => (false, false),
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
            VirtualFileState::LoadError(ref error_text) => {
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
                    error_text,
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
            VirtualFileState::SaveError(ref error_text) => {
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
                    error_text,
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

    fn Cycle(&mut self, _ctx: &mut PanelCtx) -> bool {
        self.cycle_inner()
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
        if flags.contains(NoticeFlags::MEMORY_LIMIT_CHANGED) {
            self.cached_memory_limit = state.memory_limit;
        }
        if flags.contains(NoticeFlags::UPDATE_PRIORITY_CHANGED) {
            self.cached_priority = state.priority;
        }
        if flags.intersects(NoticeFlags::ACTIVE_CHANGED | NoticeFlags::VIEW_FOCUS_CHANGED) {
            self.cached_in_active_path = state.in_active_path;
        }
    }

    fn IsHopeForSeeking(&self) -> bool {
        self.GetVirFileState().IsHopeForSeeking()
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        self.paint_status(painter, w, h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emFileModel::emFileModel;
    use crate::emSignal::SignalId;
    use std::path::PathBuf;

    fn make_panel_with_model() -> (emFilePanel, Rc<RefCell<emFileModel<String>>>) {
        let model = Rc::new(RefCell::new(emFileModel::new(
            PathBuf::from("/tmp/test"),
            SignalId::default(),
            SignalId::default(),
        )));
        let mut panel = emFilePanel::new();
        panel.SetFileModel(Some(model.clone() as Rc<RefCell<dyn FileModelState>>));
        (panel, model)
    }

    #[test]
    fn vfs_no_model() {
        let panel = emFilePanel::new();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::NoFileModel);
    }

    #[test]
    fn vfs_with_model_waiting() {
        let (panel, _model) = make_panel_with_model();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Waiting);
    }

    #[test]
    fn vfs_custom_error_overrides() {
        let (mut panel, model) = make_panel_with_model();
        model.borrow_mut().complete_load("data".to_string());
        panel.refresh_vir_file_state();
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
        let (mut panel, model) = make_panel_with_model();
        model.borrow_mut().complete_load("data".to_string());
        model.borrow_mut().CalcMemoryNeed(1000);
        panel.cached_memory_limit = 500;
        panel.refresh_vir_file_state();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::TooCostly);
    }

    #[test]
    fn vfs_good_states() {
        let (mut panel, model) = make_panel_with_model();
        model.borrow_mut().complete_load("data".to_string());
        panel.refresh_vir_file_state();
        assert!(panel.GetVirFileState().is_good());

        model.borrow_mut().SetUnsavedState();
        panel.refresh_vir_file_state();
        assert!(panel.GetVirFileState().is_good());

        // Reset to waiting by creating a new model
        let model2 = Rc::new(RefCell::new(emFileModel::<String>::new(
            PathBuf::from("/tmp/test2"),
            SignalId::default(),
            SignalId::default(),
        )));
        panel.SetFileModel(Some(model2 as Rc<RefCell<dyn FileModelState>>));
        assert!(!panel.GetVirFileState().is_good());
    }

    #[test]
    fn is_opaque_for_errors() {
        let (mut panel, model) = make_panel_with_model();
        model.borrow_mut().fail_load("err".to_string());
        panel.refresh_vir_file_state();
        assert!(panel.IsOpaque());

        model.borrow_mut().complete_load("data".to_string());
        panel.refresh_vir_file_state();
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
        let (mut panel, model) = make_panel_with_model();

        // Waiting (initial state)
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Waiting);

        // Loading — emFileModel doesn't have a direct set_loading, so test via
        // the compute path with a model that reports Loading. We test the other
        // states that are reachable through the public API.

        // Loaded
        model.borrow_mut().complete_load("data".to_string());
        panel.refresh_vir_file_state();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Loaded);

        // Unsaved
        model.borrow_mut().SetUnsavedState();
        panel.refresh_vir_file_state();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Unsaved);

        // Saving
        model.borrow_mut().Save();
        panel.refresh_vir_file_state();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Saving);

        // SaveError
        model.borrow_mut().fail_save("e".to_string());
        panel.refresh_vir_file_state();
        assert_eq!(
            panel.GetVirFileState(),
            VirtualFileState::SaveError("e".to_string())
        );

        // TooCostly
        model.borrow_mut().mark_too_costly();
        panel.refresh_vir_file_state();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::TooCostly);

        // LoadError — need a fresh model since fail_load works from Waiting
        let model2 = Rc::new(RefCell::new(emFileModel::<String>::new(
            PathBuf::from("/tmp/test2"),
            SignalId::default(),
            SignalId::default(),
        )));
        panel.SetFileModel(Some(model2.clone() as Rc<RefCell<dyn FileModelState>>));
        model2.borrow_mut().fail_load("e".to_string());
        panel.refresh_vir_file_state();
        assert_eq!(
            panel.GetVirFileState(),
            VirtualFileState::LoadError("e".to_string())
        );
    }

    #[test]
    fn canvas_color_error_states() {
        let error_color = emColor::rgb(128, 0, 0);

        let (mut panel, model) = make_panel_with_model();
        model.borrow_mut().fail_load("err".to_string());
        panel.refresh_vir_file_state();
        assert_eq!(panel.GetCanvasColor(), error_color);

        // SaveError — need fresh model
        let model2 = Rc::new(RefCell::new(emFileModel::<String>::new(
            PathBuf::from("/tmp/test2"),
            SignalId::default(),
            SignalId::default(),
        )));
        panel.SetFileModel(Some(model2.clone() as Rc<RefCell<dyn FileModelState>>));
        model2.borrow_mut().complete_load("data".to_string());
        model2.borrow_mut().SetUnsavedState();
        model2.borrow_mut().Save();
        model2.borrow_mut().fail_save("err".to_string());
        panel.refresh_vir_file_state();
        assert_eq!(panel.GetCanvasColor(), error_color);

        panel.set_custom_error("custom");
        assert_eq!(panel.GetCanvasColor(), error_color);

        panel.clear_custom_error();
        model2.borrow_mut().complete_load("data".to_string());
        panel.refresh_vir_file_state();
        assert_eq!(panel.GetCanvasColor(), emColor::TRANSPARENT);
    }

    #[test]
    fn custom_error_priority() {
        // Custom error overrides TooCostly + memory limit exceeded.
        let (mut panel, model) = make_panel_with_model();
        model.borrow_mut().mark_too_costly();
        model.borrow_mut().CalcMemoryNeed(1000);
        panel.cached_memory_limit = 500;
        panel.refresh_vir_file_state();
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
    fn set_file_model_connects_and_disconnects() {
        let (mut panel, _model) = make_panel_with_model();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Waiting);

        panel.SetFileModel(None);
        assert_eq!(panel.GetVirFileState(), VirtualFileState::NoFileModel);
    }

    #[test]
    fn cycle_detects_state_change() {
        let (mut panel, model) = make_panel_with_model();
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Waiting);

        model.borrow_mut().complete_load("data".to_string());
        let changed = panel.cycle_inner();
        assert!(changed);
        assert_eq!(panel.GetVirFileState(), VirtualFileState::Loaded);

        let changed = panel.cycle_inner();
        assert!(!changed);
    }

    #[test]
    fn notice_updates_cached_memory_limit() {
        let (mut panel, _model) = make_panel_with_model();
        let mut state = PanelState::default_for_test();
        state.memory_limit = 2048;
        panel.notice(NoticeFlags::MEMORY_LIMIT_CHANGED, &state);
        assert_eq!(panel.cached_memory_limit, 2048);
    }

    #[test]
    fn notice_updates_cached_priority() {
        let (mut panel, _model) = make_panel_with_model();
        let mut state = PanelState::default_for_test();
        state.priority = 0.75;
        panel.notice(NoticeFlags::UPDATE_PRIORITY_CHANGED, &state);
        assert!((panel.cached_priority - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn notice_updates_cached_in_active_path() {
        let (mut panel, _model) = make_panel_with_model();
        let mut state = PanelState::default_for_test();
        state.in_active_path = true;
        panel.notice(NoticeFlags::ACTIVE_CHANGED, &state);
        assert!(panel.cached_in_active_path);
    }

    #[test]
    fn is_hope_for_seeking_delegates() {
        let (panel, _model) = make_panel_with_model();
        assert!(panel.IsHopeForSeeking());
    }

    #[test]
    fn is_content_ready_by_state() {
        let (mut panel, model) = make_panel_with_model();
        // Waiting → not ready, readying
        assert_eq!(panel.IsContentReady(), (false, true));

        // Loaded → ready
        model.borrow_mut().complete_load("data".to_string());
        panel.refresh_vir_file_state();
        assert_eq!(panel.IsContentReady(), (true, false));

        // Reset to test error state
        let model2 = Rc::new(RefCell::new(emFileModel::<String>::new(
            PathBuf::from("/tmp/test2"),
            SignalId::default(),
            SignalId::default(),
        )));
        panel.SetFileModel(Some(model2.clone() as Rc<RefCell<dyn FileModelState>>));
        model2.borrow_mut().fail_load("err".to_string());
        panel.refresh_vir_file_state();
        assert_eq!(panel.IsContentReady(), (false, false));
    }
}
