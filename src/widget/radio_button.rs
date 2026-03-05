use std::cell::RefCell;
use std::rc::Rc;

use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::font_cache::FontCache;
use crate::render::Painter;

use super::border::{Border, OuterBorderType};
use super::look::Look;

/// Shared state for a group of radio buttons enforcing mutual exclusion.
pub struct RadioGroup {
    selected: Option<usize>,
    pub on_select: Option<Box<dyn FnMut(usize)>>,
}

impl RadioGroup {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            selected: None,
            on_select: None,
        }))
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn select(&mut self, index: usize) {
        self.selected = Some(index);
        if let Some(cb) = &mut self.on_select {
            cb(index);
        }
    }
}

/// Radio button widget — mutually exclusive selection within a group.
pub struct RadioButton {
    border: Border,
    look: Rc<Look>,
    group: Rc<RefCell<RadioGroup>>,
    index: usize,
}

impl RadioButton {
    pub fn new(
        caption: &str,
        look: Rc<Look>,
        group: Rc<RefCell<RadioGroup>>,
        index: usize,
    ) -> Self {
        Self {
            border: Border::new(OuterBorderType::RoundRect).with_caption(caption),
            look,
            group,
            index,
        }
    }

    pub fn is_selected(&self) -> bool {
        self.group.borrow().selected == Some(self.index)
    }

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        let face_color = if self.is_selected() {
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
        let tw = FontCache::measure_text(&self.border.caption) as f64;
        let th = FontCache::GLYPH_HEIGHT as f64;
        self.border.preferred_size_for_content(tw + 8.0, th + 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radio_group_mutual_exclusion() {
        let look = Look::new();
        let group = RadioGroup::new();

        let mut r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = RadioButton::new("B", look.clone(), group.clone(), 1);
        let mut r2 = RadioButton::new("C", look, group.clone(), 2);

        assert!(!r0.is_selected());
        assert!(!r1.is_selected());
        assert!(!r2.is_selected());

        r0.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(r0.is_selected());
        assert!(!r1.is_selected());

        r2.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(!r0.is_selected());
        assert!(r2.is_selected());

        r1.input(&InputEvent::release(InputKey::Space));
        assert!(!r0.is_selected());
        assert!(r1.is_selected());
        assert!(!r2.is_selected());
    }

    #[test]
    fn radio_group_callback() {
        let group = RadioGroup::new();
        let selections = Rc::new(RefCell::new(Vec::new()));
        let sel_clone = selections.clone();
        group.borrow_mut().on_select = Some(Box::new(move |idx| {
            sel_clone.borrow_mut().push(idx);
        }));

        let look = Look::new();
        let mut r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = RadioButton::new("B", look, group.clone(), 1);

        r0.input(&InputEvent::release(InputKey::MouseLeft));
        r1.input(&InputEvent::release(InputKey::MouseLeft));
        assert_eq!(*selections.borrow(), vec![0, 1]);
    }
}
