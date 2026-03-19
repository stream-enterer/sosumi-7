use std::rc::Rc;

use crate::foundation::Rect;
use crate::input::{InputEvent, InputKey, InputState, InputVariant};
use crate::panel::{PanelCtx, PanelState};
use crate::render::Painter;

use super::border::{Border, OuterBorderType};
use super::look::Look;

/// Result of a dialog interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogResult {
    Ok,
    Cancel,
    Custom(u32),
}

type DialogFinishCb = Box<dyn FnMut(&DialogResult)>;
type DialogCheckFinishCb = Box<dyn FnMut(&DialogResult) -> bool>;

/// Modal dialog container widget.
pub struct Dialog {
    border: Border,
    look: Rc<Look>,
    buttons: Vec<(String, DialogResult)>,
    result: Option<DialogResult>,
    pub on_finish: Option<DialogFinishCb>,
    pub on_check_finish: Option<DialogCheckFinishCb>,
    auto_delete: bool,
}

const BUTTON_HEIGHT: f64 = 22.0;
const BUTTON_SPACING: f64 = 4.0;
const BOTTOM_MARGIN: f64 = 4.0;

impl Dialog {
    pub fn new(title: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::PopupRoot).with_caption(title),
            look,
            buttons: Vec::new(),
            result: None,
            on_finish: None,
            on_check_finish: None,
            auto_delete: false,
        }
    }

    pub fn add_button(&mut self, label: &str, result: DialogResult) {
        self.buttons.push((label.to_string(), result));
    }

    pub fn result(&self) -> Option<&DialogResult> {
        self.result.as_ref()
    }

    pub fn finish(&mut self, result: DialogResult) {
        if let Some(cb) = &mut self.on_check_finish {
            if !cb(&result) {
                return;
            }
        }
        self.result = Some(result.clone());
        if let Some(cb) = &mut self.on_finish {
            cb(&result);
        }
    }

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true);
    }

    /// Layout content area and button row at the bottom.
    pub fn layout_children(&self, ctx: &mut PanelCtx, w: f64, h: f64) {
        let Rect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.border.content_rect(w, h, &self.look);
        let children = ctx.children();

        if children.is_empty() {
            return;
        }

        let button_count = self.buttons.len();
        let content_children = children.len().saturating_sub(button_count);

        // Layout content children above button row
        let content_h = ch - BUTTON_HEIGHT - BOTTOM_MARGIN;
        for (i, &child) in children.iter().take(content_children).enumerate() {
            let child_h = content_h / content_children.max(1) as f64;
            ctx.layout_child(child, cx, cy + i as f64 * child_h, cw, child_h);
        }

        // Layout button row at the bottom
        if button_count > 0 {
            let total_btn_w = cw;
            let btn_w =
                (total_btn_w - (button_count as f64 - 1.0) * BUTTON_SPACING) / button_count as f64;
            let btn_y = cy + ch - BUTTON_HEIGHT;

            for (i, &child) in children.iter().skip(content_children).enumerate() {
                let btn_x = cx + i as f64 * (btn_w + BUTTON_SPACING);
                ctx.layout_child(child, btn_x, btn_y, btn_w, BUTTON_HEIGHT);
            }
        }

        // Propagate content canvas color to children.
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        self.border
            .preferred_size_for_content(200.0, 120.0 + BUTTON_HEIGHT + BOTTOM_MARGIN)
    }

    /// Access a button label and result by index.
    ///
    /// Port of C++ `emDialog::GetButton`.
    pub fn get_button(&self, index: usize) -> Option<&(String, DialogResult)> {
        self.buttons.get(index)
    }

    /// Find the first button whose result matches `result`.
    ///
    /// Port of C++ `emDialog::GetButtonForResult`.
    pub fn get_button_for_result(&self, result: &DialogResult) -> Option<&(String, DialogResult)> {
        self.buttons.iter().find(|(_, r)| r == result)
    }

    /// Convenience accessor for the first `Ok` button.
    pub fn ok_button(&self) -> Option<&(String, DialogResult)> {
        self.get_button_for_result(&DialogResult::Ok)
    }

    /// Convenience accessor for the first `Cancel` button.
    pub fn cancel_button(&self) -> Option<&(String, DialogResult)> {
        self.get_button_for_result(&DialogResult::Cancel)
    }

    /// Enable automatic deletion when the dialog closes.
    pub fn enable_auto_deletion(&mut self) {
        self.auto_delete = true;
    }

    /// Check if auto-deletion is enabled.
    pub fn is_auto_deletion_enabled(&self) -> bool {
        self.auto_delete
    }

    /// Static convenience to create a message dialog (returns a configured `Dialog`).
    ///
    /// Port of C++ `emDialog::ShowMessage`.
    pub fn show_message(text: &str, look: Rc<Look>) -> Self {
        let mut dlg = Self::new(text, look);
        dlg.add_button("OK", DialogResult::Ok);
        dlg
    }

    /// Handle keyboard input for the dialog.
    ///
    /// Port of C++ `emDlg::DlgPanel::Input`:
    /// - Enter (no modifiers) → finish with `DialogResult::Ok`
    /// - Escape (no modifiers) → finish with `DialogResult::Cancel`
    pub fn input(
        &mut self,
        event: &InputEvent,
        _state: &PanelState,
        _input_state: &InputState,
    ) -> bool {
        if event.variant != InputVariant::Press {
            return false;
        }
        if event.ctrl || event.alt || event.meta {
            return false;
        }
        match event.key {
            InputKey::Enter => {
                self.finish(DialogResult::Ok);
                true
            }
            InputKey::Escape => {
                self.finish(DialogResult::Cancel);
                true
            }
            _ => false,
        }
    }

    /// Check if the dialog should close (i.e. a result has been set).
    ///
    /// Port of C++ `emDialog::CheckFinish`.
    pub fn check_finish(&self) -> bool {
        self.result.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::Rect;
    use crate::panel::PanelId;
    use slotmap::Key as _;
    use std::cell::RefCell;

    fn default_panel_state() -> PanelState {
        PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
        }
    }

    fn default_input_state() -> InputState {
        InputState::new()
    }

    #[test]
    fn dialog_finish_fires_callback() {
        let look = Look::new();
        let results = Rc::new(RefCell::new(Vec::new()));
        let res_clone = results.clone();

        let mut dlg = Dialog::new("Test", look);
        dlg.add_button("OK", DialogResult::Ok);
        dlg.add_button("Cancel", DialogResult::Cancel);
        dlg.on_finish = Some(Box::new(move |r| {
            res_clone.borrow_mut().push(r.clone());
        }));

        assert!(dlg.result().is_none());
        dlg.finish(DialogResult::Ok);
        assert_eq!(dlg.result(), Some(&DialogResult::Ok));
        assert_eq!(*results.borrow(), vec![DialogResult::Ok]);
    }

    #[test]
    fn check_finish_can_veto() {
        let look = Look::new();
        let mut dlg = Dialog::new("Veto", look);
        dlg.on_check_finish = Some(Box::new(|r| *r != DialogResult::Cancel));

        dlg.finish(DialogResult::Cancel);
        assert!(dlg.result().is_none(), "veto should prevent finish");

        dlg.finish(DialogResult::Ok);
        assert_eq!(dlg.result(), Some(&DialogResult::Ok));
    }

    #[test]
    fn dialog_custom_result() {
        let look = Look::new();
        let mut dlg = Dialog::new("Custom", look);
        dlg.add_button("Retry", DialogResult::Custom(42));
        dlg.finish(DialogResult::Custom(42));
        assert_eq!(dlg.result(), Some(&DialogResult::Custom(42)));
    }

    #[test]
    fn enter_finishes_with_ok() {
        let look = Look::new();
        let mut dlg = Dialog::new("Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let consumed = dlg.input(&InputEvent::press(InputKey::Enter), &ps, &is);
        assert!(consumed);
        assert_eq!(dlg.result(), Some(&DialogResult::Ok));
    }

    #[test]
    fn escape_finishes_with_cancel() {
        let look = Look::new();
        let mut dlg = Dialog::new("Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let consumed = dlg.input(&InputEvent::press(InputKey::Escape), &ps, &is);
        assert!(consumed);
        assert_eq!(dlg.result(), Some(&DialogResult::Cancel));
    }

    #[test]
    fn enter_with_modifier_is_ignored() {
        let look = Look::new();
        let mut dlg = Dialog::new("Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let mut ev = InputEvent::press(InputKey::Enter);
        ev.ctrl = true;
        let consumed = dlg.input(&ev, &ps, &is);
        assert!(!consumed);
        assert!(dlg.result().is_none());
    }

    #[test]
    fn release_event_is_ignored() {
        let look = Look::new();
        let mut dlg = Dialog::new("Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let consumed = dlg.input(&InputEvent::release(InputKey::Enter), &ps, &is);
        assert!(!consumed);
        assert!(dlg.result().is_none());
    }
}
