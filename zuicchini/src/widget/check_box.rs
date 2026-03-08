use std::rc::Rc;

use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::Painter;

use super::look::Look;

/// Small checkbox variant — square indicator with label text.
pub struct CheckBox {
    label: String,
    look: Rc<Look>,
    checked: bool,
    pub on_check: Option<Box<dyn FnMut(bool)>>,
}

const BOX_SIZE: f64 = 9.0;
const BOX_LABEL_GAP: f64 = 4.0;

impl CheckBox {
    pub fn new(label: &str, look: Rc<Look>) -> Self {
        Self {
            label: label.to_string(),
            look,
            checked: false,
            on_check: None,
        }
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    pub fn paint(&self, painter: &mut Painter, _w: f64, _h: f64) {
        let y_off = 0.0;

        // Draw box outline
        painter.paint_rect(0.0, y_off, BOX_SIZE, BOX_SIZE, self.look.input_bg_color);
        painter.paint_rect_outlined(
            0.0,
            y_off,
            BOX_SIZE,
            BOX_SIZE,
            &crate::render::Stroke::new(self.look.border_tint(), 1.0),
        );

        // Draw checkmark when checked
        if self.checked {
            let c = self.look.input_hl_color;
            // Two lines forming a checkmark
            painter.paint_line(2.0, y_off + 4.0, 4.0, y_off + 7.0, c);
            painter.paint_line(4.0, y_off + 7.0, 7.0, y_off + 1.0, c);
        }

        // Draw label text to the right
        if !self.label.is_empty() {
            // TODO(font): paint text here
        }
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        match event.key {
            InputKey::MouseLeft if event.variant == InputVariant::Release => {
                self.toggle();
                true
            }
            InputKey::Space if event.variant == InputVariant::Release => {
                self.toggle();
                true
            }
            _ => false,
        }
    }

    pub fn get_cursor(&self) -> Cursor {
        Cursor::Hand
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let w = if self.label.is_empty() {
            BOX_SIZE
        } else {
            BOX_SIZE + BOX_LABEL_GAP + self.label.len() as f64 * 7.0 // TODO(font): measure_text stub
        };
        (w, BOX_SIZE)
    }

    fn toggle(&mut self) {
        self.checked = !self.checked;
        if let Some(cb) = &mut self.on_check {
            cb(self.checked);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkbox_toggle() {
        let look = Look::new();
        let mut cb = CheckBox::new("Enable", look);
        assert!(!cb.is_checked());
        cb.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(cb.is_checked());
        cb.input(&InputEvent::release(InputKey::Space));
        assert!(!cb.is_checked());
    }

    #[test]
    fn checkbox_preferred_size() {
        let look = Look::new();
        let cb = CheckBox::new("Hi", look);
        let (w, h) = cb.preferred_size();
        // 9 (box) + 4 (gap) + text width stub
        assert!(w > 13.0, "Should include box + gap + text width");
        assert_eq!(h, 9.0);
    }
}
