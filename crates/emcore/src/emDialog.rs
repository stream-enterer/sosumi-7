use std::rc::Rc;

use crate::emEngineCtx::PanelCtx;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::emPainter;
use crate::emPanel::PanelState;
use crate::emPanel::Rect;

use super::emBorder::{emBorder, OuterBorderType};
use crate::emLook::emLook;

/// Result of a dialog interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogResult {
    Ok,
    Cancel,
    Custom(u32),
}

type DialogFinishCb = crate::emEngineCtx::WidgetCallbackRef<DialogResult>;
// DIVERGED-B3.3: `DialogCheckFinishCb` returns `bool` (veto semantics),
// which is structurally incompatible with both `WidgetCallback<Args>` and
// `WidgetCallbackRef<T>` (both return `()`). The divergence is the return
// value, not the payload lifetime. Remains a plain `Box<dyn FnMut>`.
type DialogCheckFinishCb = Box<dyn FnMut(&DialogResult) -> bool>;

/// Modal dialog container widget.
pub struct emDialog {
    border: emBorder,
    look: Rc<emLook>,
    buttons: Vec<(String, DialogResult)>,
    result: Option<DialogResult>,
    pub on_finish: Option<DialogFinishCb>,
    pub on_check_finish: Option<DialogCheckFinishCb>,
    auto_delete: bool,
}

const BUTTON_HEIGHT: f64 = 22.0;
const BUTTON_SPACING: f64 = 4.0;
const BOTTOM_MARGIN: f64 = 4.0;

impl emDialog {
    pub fn new(title: &str, look: Rc<emLook>) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::PopupRoot).with_caption(title),
            look,
            buttons: Vec::new(),
            result: None,
            on_finish: None,
            on_check_finish: None,
            auto_delete: false,
        }
    }

    pub fn AddCustomButton(&mut self, label: &str, result: DialogResult) {
        self.buttons.push((label.to_string(), result));
    }

    /// Update the dialog title (border caption).
    pub fn SetRootTitle(&mut self, title: &str) {
        self.border.SetCaption(title);
    }

    /// Update the label of the first button whose result matches `result`.
    pub fn set_button_label_for_result(&mut self, result: &DialogResult, label: &str) {
        if let Some((lbl, _)) = self.buttons.iter_mut().find(|(_, r)| r == result) {
            *lbl = label.to_string();
        }
    }

    /// Get the look used by this dialog.
    pub fn look(&self) -> &Rc<emLook> {
        &self.look
    }

    pub fn GetResult(&self) -> Option<&DialogResult> {
        self.result.as_ref()
    }

    pub fn Finish(&mut self, result: DialogResult) {
        if let Some(cb) = &mut self.on_check_finish {
            if !cb(&result) {
                return;
            }
        }
        self.result = Some(result.clone());
        // DIVERGED-B3.4b: `on_finish` is now `WidgetCallbackRef<DialogResult>`
        // requiring a `SchedCtx`, but `Finish` is a public API called from
        // non-sched-reach paths. B3.4b/c will restore dispatch via async
        // signals routed through the dialog close path.
        let _ = &self.on_finish;
    }

    pub fn Paint(&self, painter: &mut emPainter, w: f64, h: f64, pixel_scale: f64) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
    }

    /// Layout content area and button row at the bottom.
    pub fn LayoutChildren(&self, ctx: &mut PanelCtx, w: f64, h: f64) {
        let Rect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.border.GetContentRect(w, h, &self.look);
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
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        self.border
            .preferred_size_for_content(200.0, 120.0 + BUTTON_HEIGHT + BOTTOM_MARGIN)
    }

    /// Access a button label and result by index.
    ///
    /// Port of C++ `emDialog::GetButton`.
    pub fn GetButton(&self, index: usize) -> Option<&(String, DialogResult)> {
        self.buttons.get(index)
    }

    /// Find the first button whose result matches `result`.
    ///
    /// Port of C++ `emDialog::GetButtonForResult`.
    pub fn GetButtonForResult(&self, result: &DialogResult) -> Option<&(String, DialogResult)> {
        self.buttons.iter().find(|(_, r)| r == result)
    }

    /// Convenience accessor for the first `Ok` button.
    pub fn GetOKButton(&self) -> Option<&(String, DialogResult)> {
        self.GetButtonForResult(&DialogResult::Ok)
    }

    /// Convenience accessor for the first `Cancel` button.
    pub fn GetCancelButton(&self) -> Option<&(String, DialogResult)> {
        self.GetButtonForResult(&DialogResult::Cancel)
    }

    /// Enable automatic deletion when the dialog closes.
    pub fn EnableAutoDeletion(&mut self) {
        self.auto_delete = true;
    }

    /// Check if auto-deletion is enabled.
    pub fn IsAutoDeletionEnabled(&self) -> bool {
        self.auto_delete
    }

    /// Static convenience to create a message dialog (returns a configured `emDialog`).
    ///
    /// Port of C++ `emDialog::ShowMessage`.
    pub fn ShowMessage(text: &str, look: Rc<emLook>) -> Self {
        let mut dlg = Self::new(text, look);
        dlg.AddCustomButton("OK", DialogResult::Ok);
        dlg
    }

    /// Handle keyboard input for the dialog.
    ///
    /// Port of C++ `emDlg::DlgPanel::Input`:
    /// - Enter (no modifiers) → finish with `DialogResult::Ok`
    /// - Escape (no modifiers) → finish with `DialogResult::Cancel`
    pub fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        if event.variant != InputVariant::Press {
            return false;
        }
        if event.ctrl || event.alt || event.meta {
            return false;
        }
        match event.key {
            InputKey::Enter => {
                self.Finish(DialogResult::Ok);
                true
            }
            InputKey::Escape => {
                self.Finish(DialogResult::Cancel);
                true
            }
            _ => false,
        }
    }

    /// Check if the dialog should close (i.e. a result has been set).
    ///
    /// Port of C++ `emDialog::CheckFinish`.
    pub fn CheckFinish(&self) -> bool {
        self.result.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanel::Rect;
    use crate::emPanelTree::{PanelId, PanelTree};
    use slotmap::Key as _;
    use std::cell::RefCell;

    fn test_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let id = tree.create_root("t", false);
        (tree, id)
    }

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
            height: 1.0,
        }
    }

    fn default_input_state() -> emInputState {
        emInputState::new()
    }

    #[test]
    #[ignore = "B3.4b: on_finish deferred dispatch; B3.4c restores via signal"]
    fn dialog_finish_fires_callback() {
        let look = emLook::new();
        let results = Rc::new(RefCell::new(Vec::new()));
        let res_clone = results.clone();

        let mut dlg = emDialog::new("Test", look);
        dlg.AddCustomButton("OK", DialogResult::Ok);
        dlg.AddCustomButton("Cancel", DialogResult::Cancel);
        dlg.on_finish = Some(Box::new(
            move |r: &DialogResult, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                res_clone.borrow_mut().push(r.clone());
            },
        ));

        assert!(dlg.GetResult().is_none());
        dlg.Finish(DialogResult::Ok);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
        assert_eq!(*results.borrow(), vec![DialogResult::Ok]);
    }

    #[test]
    fn check_finish_can_veto() {
        let look = emLook::new();
        let mut dlg = emDialog::new("Veto", look);
        dlg.on_check_finish = Some(Box::new(|r| *r != DialogResult::Cancel));

        dlg.Finish(DialogResult::Cancel);
        assert!(dlg.GetResult().is_none(), "veto should prevent finish");

        dlg.Finish(DialogResult::Ok);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
    }

    #[test]
    fn dialog_custom_result() {
        let look = emLook::new();
        let mut dlg = emDialog::new("Custom", look);
        dlg.AddCustomButton("Retry", DialogResult::Custom(42));
        dlg.Finish(DialogResult::Custom(42));
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Custom(42)));
    }

    #[test]
    fn enter_finishes_with_ok() {
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new("Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let consumed = dlg.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(consumed);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
    }

    #[test]
    fn escape_finishes_with_cancel() {
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new("Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let consumed = dlg.Input(&emInputEvent::press(InputKey::Escape), &ps, &is, &mut ctx);
        assert!(consumed);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Cancel));
    }

    #[test]
    fn enter_with_modifier_is_ignored() {
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new("Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let mut ev = emInputEvent::press(InputKey::Enter);
        ev.ctrl = true;
        let consumed = dlg.Input(&ev, &ps, &is, &mut ctx);
        assert!(!consumed);
        assert!(dlg.GetResult().is_none());
    }

    #[test]
    fn release_event_is_ignored() {
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new("Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let consumed = dlg.Input(&emInputEvent::release(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!consumed);
        assert!(dlg.GetResult().is_none());
    }

    #[test]
    fn add_custom_button_lookup() {
        let look = emLook::new();
        let mut dlg = emDialog::new("Test", look);
        dlg.AddCustomButton("Apply", DialogResult::Custom(7));
        let btn = dlg.GetButtonForResult(&DialogResult::Custom(7));
        assert!(btn.is_some());
        let (label, _) = btn.unwrap();
        assert_eq!(label, "Apply");
    }

    #[test]
    fn set_button_label() {
        let look = emLook::new();
        let mut dlg = emDialog::new("Test", look);
        dlg.AddCustomButton("OK", DialogResult::Ok);
        dlg.set_button_label_for_result(&DialogResult::Ok, "Accept");
        let btn = dlg.GetButtonForResult(&DialogResult::Ok);
        assert!(btn.is_some());
        let (label, _) = btn.unwrap();
        assert_eq!(label, "Accept");
    }

    #[test]
    fn auto_deletion_toggle() {
        let look = emLook::new();
        let dlg = emDialog::new("Test", look);
        assert!(!dlg.IsAutoDeletionEnabled());
        let mut dlg = dlg;
        dlg.EnableAutoDeletion();
        assert!(dlg.IsAutoDeletionEnabled());
    }

    #[test]
    fn check_finish_lifecycle() {
        let look = emLook::new();
        let mut dlg = emDialog::new("Test", look);
        assert!(!dlg.CheckFinish());
        dlg.Finish(DialogResult::Ok);
        assert!(dlg.CheckFinish());
    }

    #[test]
    fn set_root_title() {
        let look = emLook::new();
        let mut dlg = emDialog::new("Old Title", look);
        dlg.SetRootTitle("New Title");
        // Verify the dialog still functions after title change.
        dlg.Finish(DialogResult::Ok);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
    }
}
