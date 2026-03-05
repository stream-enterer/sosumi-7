use std::rc::Rc;

use crate::panel::PanelCtx;
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

/// Modal dialog container widget.
pub struct Dialog {
    border: Border,
    look: Rc<Look>,
    buttons: Vec<(String, DialogResult)>,
    result: Option<DialogResult>,
    pub on_finish: Option<DialogFinishCb>,
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
        }
    }

    pub fn add_button(&mut self, label: &str, result: DialogResult) {
        self.buttons.push((label.to_string(), result));
    }

    pub fn result(&self) -> Option<&DialogResult> {
        self.result.as_ref()
    }

    pub fn finish(&mut self, result: DialogResult) {
        self.result = Some(result.clone());
        if let Some(cb) = &mut self.on_finish {
            cb(&result);
        }
    }

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border.paint_border(painter, w, h, &self.look, false);
    }

    /// Layout content area and button row at the bottom.
    pub fn layout_children(&self, ctx: &mut PanelCtx, w: f64, h: f64) {
        let (cx, cy, cw, ch) = self.border.content_rect(w, h, &self.look);
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
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        self.border
            .preferred_size_for_content(200.0, 120.0 + BUTTON_HEIGHT + BOTTOM_MARGIN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

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
    fn dialog_custom_result() {
        let look = Look::new();
        let mut dlg = Dialog::new("Custom", look);
        dlg.add_button("Retry", DialogResult::Custom(42));
        dlg.finish(DialogResult::Custom(42));
        assert_eq!(dlg.result(), Some(&DialogResult::Custom(42)));
    }
}
