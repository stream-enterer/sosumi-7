use std::cell::RefCell;
use std::rc::Rc;

use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::Painter;

use super::look::Look;
use super::radio_button::RadioGroup;

const CIRCLE_SIZE: f64 = 9.0;
const CIRCLE_LABEL_GAP: f64 = 4.0;

/// Small radio button variant — circle indicator with label text.
pub struct RadioBox {
    label: String,
    look: Rc<Look>,
    group: Rc<RefCell<RadioGroup>>,
    index: usize,
}

impl RadioBox {
    pub fn new(label: &str, look: Rc<Look>, group: Rc<RefCell<RadioGroup>>, index: usize) -> Self {
        Self {
            label: label.to_string(),
            look,
            group,
            index,
        }
    }

    pub fn is_selected(&self) -> bool {
        self.group.borrow().selected() == Some(self.index)
    }

    pub fn paint(&self, painter: &mut Painter, _w: f64, _h: f64) {
        let cx = CIRCLE_SIZE / 2.0;
        let cy = CIRCLE_SIZE / 2.0;
        let r = CIRCLE_SIZE / 2.0;

        // Outer circle
        painter.paint_ellipse(cx, cy, r, r, self.look.input_bg_color);

        // Border ring — approximate with a slightly larger ellipse underneath
        painter.paint_ellipse(cx, cy, r, r, self.look.border_tint());
        painter.paint_ellipse(cx, cy, r - 1.0, r - 1.0, self.look.input_bg_color);

        // Filled dot when selected
        if self.is_selected() {
            painter.paint_ellipse(cx, cy, r - 2.5, r - 2.5, self.look.input_hl_color);
        }

        // Label
        if !self.label.is_empty() {
            // TODO(font): paint text here
        }
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        match event.key {
            InputKey::MouseLeft if event.variant == InputVariant::Release => {
                self.group.borrow_mut().select(self.index);
                true
            }
            InputKey::Space if event.variant == InputVariant::Release => {
                self.group.borrow_mut().select(self.index);
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
            CIRCLE_SIZE
        } else {
            CIRCLE_SIZE + CIRCLE_LABEL_GAP + self.label.len() as f64 * 7.0 // TODO(font): measure_text stub
        };
        (w, CIRCLE_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radio_box_selection() {
        let look = Look::new();
        let group = RadioGroup::new();

        let mut rb0 = RadioBox::new("X", look.clone(), group.clone(), 0);
        let mut rb1 = RadioBox::new("Y", look, group.clone(), 1);

        assert!(!rb0.is_selected());
        assert!(!rb1.is_selected());

        rb0.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(rb0.is_selected());
        assert!(!rb1.is_selected());

        rb1.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(!rb0.is_selected());
        assert!(rb1.is_selected());
    }
}
