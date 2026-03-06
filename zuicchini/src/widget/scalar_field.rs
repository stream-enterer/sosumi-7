use std::rc::Rc;

use crate::foundation::Rect;
use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::font_cache::FontCache;
use crate::render::Painter;

use super::border::{Border, InnerBorderType, OuterBorderType};
use super::look::Look;

/// Numeric input with scale bar.
pub struct ScalarField {
    border: Border,
    look: Rc<Look>,
    value: f64,
    min: f64,
    max: f64,
    precision: usize,
    dragging: bool,
    drag_start_x: f64,
    drag_start_value: f64,
    /// Cached width from the last paint call (like Eagle Mode's
    /// GetContentRoundRect pattern — widgets query dimensions during input).
    last_w: f64,
    pub on_value: Option<Box<dyn FnMut(f64)>>,
}

impl ScalarField {
    pub fn new(min: f64, max: f64, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::Rect).with_inner(InnerBorderType::InputField),
            look,
            value: min,
            min,
            max,
            precision: 2,
            dragging: false,
            drag_start_x: 0.0,
            drag_start_value: 0.0,
            last_w: 0.0,
            on_value: None,
        }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn set_value(&mut self, val: f64) {
        self.value = val.clamp(self.min, self.max);
    }

    pub fn set_precision(&mut self, precision: usize) {
        self.precision = precision;
    }

    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64) {
        self.last_w = w;
        self.border.paint_border(painter, w, h, &self.look, false);

        let Rect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.border.content_rect(w, h, &self.look);
        let range = self.max - self.min;

        if range > 0.0 {
            // Fill bar
            let fill_frac = (self.value - self.min) / range;
            let fill_w = cw * fill_frac;
            painter.paint_rect(cx, cy, fill_w, ch, self.look.input_hl_color);

            // Scale marks
            let mark_count = 10;
            for i in 1..mark_count {
                let mx = cx + cw * i as f64 / mark_count as f64;
                painter.paint_rect(mx, cy, 1.0, ch, self.look.disabled_fg());
            }
        }

        // Value text
        let text = format!("{:.prec$}", self.value, prec = self.precision);
        let size_px = FontCache::quantize_size(FontCache::DEFAULT_SIZE_PX);
        let tw = painter.font_cache().measure_text(&text, 0, size_px).0;
        let tx = cx + (cw - tw) / 2.0;
        let ty = cy + (ch - FontCache::DEFAULT_SIZE_PX) / 2.0;
        painter.paint_text(
            tx,
            ty,
            &text,
            FontCache::DEFAULT_SIZE_PX,
            self.look.input_fg_color,
        );
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        let Rect { w: cw, .. } = self.border.content_rect(self.last_w, 0.0, &self.look);
        let range = self.max - self.min;

        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    self.dragging = true;
                    self.drag_start_x = event.mouse_x;
                    self.drag_start_value = self.value;
                    true
                }
                InputVariant::Release => {
                    self.dragging = false;
                    true
                }
                InputVariant::Repeat | InputVariant::Move => {
                    if self.dragging && cw > 0.0 {
                        let dx = event.mouse_x - self.drag_start_x;
                        let dv = dx / cw * range;
                        let new_val = (self.drag_start_value + dv).clamp(self.min, self.max);
                        if (new_val - self.value).abs() > f64::EPSILON {
                            self.value = new_val;
                            self.fire_change();
                        }
                    }
                    true
                }
            },
            InputKey::ArrowRight if event.variant == InputVariant::Press => {
                let step = range / 100.0;
                self.set_value(self.value + step);
                self.fire_change();
                true
            }
            InputKey::ArrowLeft if event.variant == InputVariant::Press => {
                let step = range / 100.0;
                self.set_value(self.value - step);
                self.fire_change();
                true
            }
            _ => false,
        }
    }

    pub fn get_cursor(&self) -> Cursor {
        Cursor::ResizeEW
    }

    pub fn preferred_size(&self, _font_cache: &FontCache) -> (f64, f64) {
        let cw = 100.0;
        let ch = FontCache::DEFAULT_SIZE_PX + 4.0;
        self.border.preferred_size_for_content(cw, ch)
    }

    fn fire_change(&mut self) {
        if let Some(cb) = &mut self.on_value {
            cb(self.value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn value_clamping() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);

        sf.set_value(50.0);
        assert!((sf.value() - 50.0).abs() < 0.001);

        sf.set_value(-10.0);
        assert!((sf.value() - 0.0).abs() < 0.001);

        sf.set_value(200.0);
        assert!((sf.value() - 100.0).abs() < 0.001);
    }

    #[test]
    fn arrow_key_stepping() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        sf.set_value(50.0);

        // Cache dimensions (paint would do this in real usage)
        sf.last_w = 200.0;

        sf.input(&InputEvent::press(InputKey::ArrowRight));
        assert!(sf.value() > 50.0);

        sf.input(&InputEvent::press(InputKey::ArrowLeft));
        // Should be roughly back to 50
        assert!((sf.value() - 50.0).abs() < 0.01);
    }

    #[test]
    fn callback_on_change() {
        let look = Look::new();
        let values = Rc::new(RefCell::new(Vec::new()));
        let val_clone = values.clone();

        let mut sf = ScalarField::new(0.0, 10.0, look);
        sf.set_value(5.0);
        sf.last_w = 200.0;
        sf.on_value = Some(Box::new(move |v| {
            val_clone.borrow_mut().push(v);
        }));

        sf.input(&InputEvent::press(InputKey::ArrowRight));
        assert_eq!(values.borrow().len(), 1);
        assert!(values.borrow()[0] > 5.0);
    }
}
