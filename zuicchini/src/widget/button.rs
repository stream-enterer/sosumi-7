use std::rc::Rc;

use crate::foundation::{Color, Rect};
use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::{Painter, BORDER_EDGES_ONLY};

use super::border::{Border, OuterBorderType};
use super::look::Look;
use super::toolkit_images::with_toolkit_images;

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
    pub on_press_state: Option<Box<dyn FnMut(bool)>>,
}

impl Button {
    pub fn new(caption: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::InstrumentMoreRound)
                .with_caption(caption)
                .with_label_in_border(false),
            look,
            pressed: false,
            hovered: false,
            last_w: 0.0,
            last_h: 0.0,
            on_click: None,
            on_press_state: None,
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

        // C++ DoBorder paints the border first, then DoButton paints the face
        // inside the content round rect.
        self.border
            .paint_border(painter, w, h, &self.look, false, true);

        // C++ emButton::DoButton gets content round rect, then insets the face
        // by d = (1 - 250/264) * r = (14/264) * r.
        let (cr, r) = self.border.content_round_rect(w, h, &self.look);
        let r = r.max(cr.w.min(cr.h) * self.border.border_scaling * 0.223);
        let d = (14.0 / 264.0) * r;
        let fx = cr.x + d;
        let fy = cr.y + d;
        let fw = cr.w - 2.0 * d;
        let fh = cr.h - 2.0 * d;
        let fr = (r - d).max(0.0);
        painter.paint_round_rect(fx, fy, fw, fh, fr, face_color);
        painter.set_canvas_color(face_color);

        // C++ DoButton: PaintLabel inside the face area with padding.
        let d_min = fw.min(fh) * 0.1;
        let dx = (r * 0.7).max(d_min);
        let dy = (r * 0.4).max(d_min);
        let mut lx = fx + dx;
        let mut ly = fy + dy;
        let mut lw = fw - 2.0 * dx;
        let mut lh = fh - 2.0 * dy;
        if self.pressed {
            let s = 0.98;
            lx += (1.0 - s) * 0.5 * lw;
            lw *= s;
            ly += (1.0 - s) * 0.5 * lh;
            lh *= s;
        }
        self.border.paint_label_colored(
            painter,
            Rect::new(lx, ly, lw, lh),
            &self.look,
            self.look.button_fg_color,
            true,
        );

        // C++ DoButton paints button image overlay on top of the face.
        with_toolkit_images(|img| {
            if self.pressed {
                painter.paint_border_image(
                    cr.x,
                    cr.y,
                    cr.w,
                    cr.h,
                    360.0 / 264.0 * r,
                    374.0 / 264.0 * r,
                    r, // C++ 264.0/264.0 = 1.0
                    r, // C++ 264.0/264.0 = 1.0
                    &img.button_pressed,
                    360,
                    374,
                    264,
                    264,
                    255,
                    Color::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            } else {
                // Normal button: image extends slightly beyond content rect.
                let extra = (658.0 - 648.0) / 264.0 * r;
                painter.paint_border_image(
                    cr.x,
                    cr.y,
                    cr.w + extra,
                    cr.h + extra,
                    278.0 / 264.0 * r,
                    278.0 / 264.0 * r,
                    278.0 / 264.0 * r,
                    278.0 / 264.0 * r,
                    &img.button,
                    278,
                    278,
                    278,
                    278,
                    255,
                    Color::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            }
        });
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
                    if let Some(cb) = &mut self.on_press_state {
                        cb(true);
                    }
                    true
                }
                InputVariant::Release => {
                    if self.pressed {
                        self.pressed = false;
                        if let Some(cb) = &mut self.on_press_state {
                            cb(false);
                        }
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
                    if let Some(cb) = &mut self.on_press_state {
                        cb(true);
                    }
                    true
                }
                InputVariant::Release => {
                    if self.pressed {
                        self.pressed = false;
                        if let Some(cb) = &mut self.on_press_state {
                            cb(false);
                        }
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
        let th = 13.0;
        let tw = Painter::measure_text_width(&self.border.caption, th);
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
