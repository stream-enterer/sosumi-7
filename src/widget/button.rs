use std::rc::Rc;

use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::font_cache::FontCache;
use crate::render::Painter;

use super::border::{Border, OuterBorderType};
use super::look::Look;

/// Clickable button widget.
pub struct Button {
    border: Border,
    look: Rc<Look>,
    pressed: bool,
    hovered: bool,
    pub on_click: Option<Box<dyn FnMut()>>,
}

impl Button {
    pub fn new(caption: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::RoundRect).with_caption(caption),
            look,
            pressed: false,
            hovered: false,
            on_click: None,
        }
    }

    pub fn set_caption(&mut self, text: &str) {
        self.border.caption = text.to_string();
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        let face_color = if self.pressed {
            self.look.button_press_color
        } else if self.hovered {
            self.look.button_hover_color
        } else {
            self.look.button_color
        };

        painter.paint_round_rect(1.0, 1.0, w - 2.0, h - 2.0, 3.0, face_color);
        self.border.paint_border(painter, w, h, &self.look, false);
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    self.pressed = true;
                    true
                }
                InputVariant::Release => {
                    if self.pressed {
                        self.pressed = false;
                        if let Some(cb) = &mut self.on_click {
                            cb();
                        }
                    }
                    true
                }
                _ => false,
            },
            InputKey::Enter | InputKey::Space => match event.variant {
                InputVariant::Press => {
                    self.pressed = true;
                    true
                }
                InputVariant::Release => {
                    if self.pressed {
                        self.pressed = false;
                        if let Some(cb) = &mut self.on_click {
                            cb();
                        }
                    }
                    true
                }
                _ => false,
            },
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn button_press_release_fires_callback() {
        let look = Look::new();
        let fired = Rc::new(RefCell::new(false));
        let fired_clone = fired.clone();

        let mut btn = Button::new("Click", look);
        btn.on_click = Some(Box::new(move || {
            *fired_clone.borrow_mut() = true;
        }));

        assert!(!btn.is_pressed());
        btn.input(&InputEvent::press(InputKey::MouseLeft));
        assert!(btn.is_pressed());
        btn.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(!btn.is_pressed());
        assert!(*fired.borrow());
    }

    #[test]
    fn button_keyboard_activation() {
        let look = Look::new();
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();

        let mut btn = Button::new("OK", look);
        btn.on_click = Some(Box::new(move || {
            *count_clone.borrow_mut() += 1;
        }));

        btn.input(&InputEvent::press(InputKey::Space));
        btn.input(&InputEvent::release(InputKey::Space));
        btn.input(&InputEvent::press(InputKey::Enter));
        btn.input(&InputEvent::release(InputKey::Enter));
        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn button_cursor_is_hand() {
        let look = Look::new();
        let btn = Button::new("X", look);
        assert_eq!(btn.get_cursor(), Cursor::Hand);
    }
}
