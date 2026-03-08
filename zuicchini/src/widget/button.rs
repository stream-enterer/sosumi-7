use std::rc::Rc;

use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::Painter;

use super::border::{Border, OuterBorderType};
use super::look::Look;

/// Clickable button widget.
pub struct Button {
    border: Border,
    look: Rc<Look>,
    pressed: bool,
    hovered: bool,
    /// Cached dimensions for hover hit testing.
    last_w: f64,
    last_h: f64,
    pub on_click: Option<Box<dyn FnMut()>>,
}

impl Button {
    pub fn new(caption: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::RoundRect).with_caption(caption),
            look,
            pressed: false,
            hovered: false,
            last_w: 0.0,
            last_h: 0.0,
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

    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64) {
        self.last_w = w;
        self.last_h = h;
        let face_color = if self.pressed {
            self.look.button_pressed()
        } else if self.hovered {
            self.look.button_hover()
        } else {
            self.look.button_bg_color
        };

        painter.paint_round_rect(1.0, 1.0, w - 2.0, h - 2.0, 3.0, face_color);
        self.border
            .paint_border(painter, w, h, &self.look, false, true);
    }

    /// Update hover state based on mouse position within button bounds.
    fn update_hover(&mut self, mx: f64, my: f64) {
        self.hovered = mx >= 0.0 && mx <= self.last_w && my >= 0.0 && my <= self.last_h;
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        // Update hover on any event with mouse coordinates
        if event.variant == InputVariant::Move {
            self.update_hover(event.mouse_x, event.mouse_y);
            return false;
        }

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

    /// Programmatically fire the click callback.
    pub fn click(&mut self) {
        if let Some(cb) = &mut self.on_click {
            cb();
        }
    }

    pub fn get_cursor(&self) -> Cursor {
        Cursor::Hand
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let tw = self.border.caption.len() as f64 * 7.0; // TODO(font): measure_text stub
        let th = 13.0;
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

    #[test]
    fn click_fires_callback() {
        let look = Look::new();
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();

        let mut btn = Button::new("Go", look);
        btn.on_click = Some(Box::new(move || {
            *count_clone.borrow_mut() += 1;
        }));

        btn.click();
        btn.click();
        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn click_without_callback_is_noop() {
        let look = Look::new();
        let mut btn = Button::new("Go", look);
        btn.click(); // should not panic
    }
}
