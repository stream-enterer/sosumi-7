use std::rc::Rc;

use crate::foundation::Color;
use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::Painter;

use super::border::{Border, OuterBorderType};
use super::look::Look;
use super::toolkit_images::with_toolkit_images;

/// CheckBox widget — InstrumentMoreRound border with checkbox image overlay.
/// Matches C++ `emCheckBox` (which extends `emCheckButton` extends `emButton`).
pub struct CheckBox {
    border: Border,
    look: Rc<Look>,
    checked: bool,
    pub on_check: Option<Box<dyn FnMut(bool)>>,
}

impl CheckBox {
    pub fn new(label: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::InstrumentMoreRound).with_caption(label),
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

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true);

        // C++ DoButton: content round rect, face, then checkbox image.
        let (cr, r) = self.border.content_round_rect(w, h, &self.look);
        let d = (1.0 - 250.0 / 264.0) * r;
        let fr = (r - d).max(0.0);
        let face_color = self.look.button_bg_color;
        painter.paint_round_rect(
            cr.x + d,
            cr.y + d,
            cr.w - 2.0 * d,
            cr.h - 2.0 * d,
            fr,
            face_color,
        );
        painter.set_canvas_color(face_color);

        // C++ DoButton ShowBox: paint checkbox image inside content rect.
        let bw = cr.w.min(cr.h) * 0.7;
        let bx = cr.x + (cr.w - bw) * 0.5;
        let by = cr.y + (cr.h - bw) * 0.5;
        with_toolkit_images(|img| {
            let image = if self.checked {
                &img.check_box_pressed
            } else {
                &img.check_box
            };
            painter.paint_image_full(bx, by, bw, bw, image, 255, Color::TRANSPARENT);
        });
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
        let th = 13.0;
        let tw = Painter::measure_text_width(&self.border.caption, th);
        self.border.preferred_size_for_content(tw + 8.0, th + 4.0)
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
        assert!(w > 0.0, "Should have positive width");
        assert!(h > 0.0, "Should have positive height");
    }
}
