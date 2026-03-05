use std::rc::Rc;

use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::font_cache::FontCache;
use crate::render::Painter;

use super::border::{Border, OuterBorderType};
use super::look::Look;

/// Toggle button widget — visually depressed when checked.
pub struct CheckButton {
    border: Border,
    look: Rc<Look>,
    checked: bool,
    pub on_check: Option<Box<dyn FnMut(bool)>>,
}

impl CheckButton {
    pub fn new(caption: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::RoundRect).with_caption(caption),
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
        let face_color = if self.checked {
            self.look.button_press_color
        } else {
            self.look.button_color
        };
        painter.paint_round_rect(1.0, 1.0, w - 2.0, h - 2.0, 3.0, face_color);
        self.border.paint_border(painter, w, h, &self.look, false);
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
        let tw = FontCache::measure_text(&self.border.caption).0 as f64;
        let th = FontCache::GLYPH_HEIGHT as f64;
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
    use std::cell::RefCell;

    #[test]
    fn toggle_state() {
        let look = Look::new();
        let mut btn = CheckButton::new("Toggle", look);
        assert!(!btn.is_checked());
        btn.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(btn.is_checked());
        btn.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(!btn.is_checked());
    }

    #[test]
    fn callback_receives_state() {
        let look = Look::new();
        let states = Rc::new(RefCell::new(Vec::new()));
        let states_clone = states.clone();

        let mut btn = CheckButton::new("CB", look);
        btn.on_check = Some(Box::new(move |checked| {
            states_clone.borrow_mut().push(checked);
        }));

        btn.input(&InputEvent::release(InputKey::Space));
        btn.input(&InputEvent::release(InputKey::Space));
        assert_eq!(*states.borrow(), vec![true, false]);
    }
}
