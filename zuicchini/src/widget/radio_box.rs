use std::cell::RefCell;
use std::rc::Rc;

use crate::foundation::{Color, Rect};
use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::{Painter, BORDER_EDGES_ONLY};

use super::border::{Border, OuterBorderType};
use super::look::Look;
use super::radio_button::RadioGroup;
use super::toolkit_images::with_toolkit_images;

/// Small radio box widget — box indicator with label text.
///
/// C++ `emRadioBox` inherits `emRadioButton : emCheckButton : emButton : emBorder`.
/// Constructor sets: `OBT_MARGIN`, `LabelAlignment=LEFT`, `ShownBoxed=true`,
/// `ShownRadioed=true`.
///
/// Paint uses the C++ DoButton boxed+radioed path (emButton.cpp:233-341):
/// content_rect → box geometry → circular face → radio dot → RadioBox image overlay.
pub struct RadioBox {
    border: Border,
    look: Rc<Look>,
    group: Rc<RefCell<RadioGroup>>,
    index: usize,
    pressed: bool,
    box_pressed: bool,
    last_w: f64,
    last_h: f64,
}

impl RadioBox {
    pub fn new(label: &str, look: Rc<Look>, group: Rc<RefCell<RadioGroup>>, index: usize) -> Self {
        Self {
            border: Border::new(OuterBorderType::Margin)
                .with_caption(label)
                .with_label_in_border(false)
                .with_label_alignment(crate::render::TextAlignment::Left)
                .with_how_to(true),
            look,
            group,
            index,
            pressed: false,
            box_pressed: false,
            last_w: 0.0,
            last_h: 0.0,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn set_index(&mut self, index: usize) {
        self.index = index;
    }

    pub fn is_selected(&self) -> bool {
        self.group.borrow().selected() == Some(self.index)
    }

    pub fn set_checked(&mut self, checked: bool) {
        if checked {
            self.group.borrow_mut().select(self.index);
        } else if self.is_selected() {
            self.group.borrow_mut().set_check_index(None);
        }
    }

    /// Compute the box + label geometry from the content rect (C++ lines 235-260).
    /// Returns (bx0, by0, bw0, lx, ly, lw, lh).
    fn box_label_geometry(
        &self,
        cr: &Rect,
    ) -> (f64, f64, f64, f64, f64, f64, f64) {
        let has_label = self.border.has_label();
        if has_label {
            let label_tallness = self.border.best_label_tallness().max(0.2);
            let mut box_w = label_tallness;
            let mut d = box_w * 0.1;
            let f = (cr.w / (box_w + d + 1.0)).min(cr.h / label_tallness);
            box_w *= f;
            d *= f;
            let lw = cr.w - box_w - d;
            let lh = box_w;
            let lx = cr.x + cr.w - lw;
            let ly = cr.y + (cr.h - lh) * 0.5;
            let bw0 = box_w;
            let bx0 = cr.x;
            let by0 = cr.y + (cr.h - bw0) * 0.5;
            (bx0, by0, bw0, lx, ly, lw, lh)
        } else {
            let bw0 = cr.w.min(cr.h);
            let bx0 = cr.x;
            let by0 = cr.y + (cr.h - bw0) * 0.5;
            (bx0, by0, bw0, cr.x, cr.y, 0.0, 0.0)
        }
    }

    /// Paint using the C++ DoButton ShownBoxed=true, ShownRadioed=true path.
    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64) {
        self.last_w = w;
        self.last_h = h;
        self.border
            .paint_border(painter, w, h, &self.look, false, true);

        let cr = self.border.content_rect(w, h, &self.look);
        let (bx0, by0, bw0, mut lx, mut ly, mut lw, mut lh) =
            self.box_label_geometry(&cr);

        // Inset for image area: d = bw * 0.13 (C++ line 262).
        let d = bw0 * 0.13;
        let mut bx = bx0 + d;
        let by = by0 + d;
        let bw = bw0 - 2.0 * d;
        let bh = bw;

        // Face inset: d = bw * 30/380 (C++ line 268).
        let d2 = bw * 30.0 / 380.0;
        let mut fx = bx + d2;
        let fy = by + d2;
        let fw = bw - 2.0 * d2;
        let fh = bh - 2.0 * d2;
        // C++ line 273: ShownRadioed → fr = fw * 0.5 (fully circular).
        let fr = fw * 0.5;

        // C++ lines 294-300: Pressed && !BoxPressed nudges box/label.
        if self.pressed && !self.box_pressed {
            bx += lw * 0.003;
            fx += lw * 0.003;
            lx += lw * 0.003;
            ly += lh * 0.007;
            lw *= 0.986;
            lh *= 0.986;
        }

        // Paint label to the right of the box.
        if self.border.has_label() {
            self.border
                .paint_label(painter, Rect::new(lx, ly, lw, lh), &self.look, true);
        }

        // Paint face (InputBgColor) — circular for radio.
        let face_color = self.look.input_bg_color;
        painter.paint_round_rect(fx, fy, fw, fh, fr, face_color);
        painter.set_canvas_color(face_color);

        // Paint radio dot if selected (C++ PaintBoxSymbol, lines 161-167).
        // C++ PaintEllipse takes bounding rect (x, y, w, h).
        // Rust paint_ellipse takes center + radii (cx, cy, rx, ry).
        if self.is_selected() {
            let dot_d = fw * 0.25;
            let dot_w = fw - 2.0 * dot_d;
            let dot_h = fh - 2.0 * dot_d;
            painter.paint_ellipse(
                fx + dot_d + dot_w * 0.5,
                fy + dot_d + dot_h * 0.5,
                dot_w * 0.5,
                dot_h * 0.5,
                self.look.input_fg_color,
                Color::TRANSPARENT,
            );
        }

        // Paint radio box image overlay (C++ lines 318-331).
        // BoxPressed → RadioBoxPressed image, else → RadioBox image.
        with_toolkit_images(|img| {
            let box_img = if self.box_pressed {
                &img.radio_box_pressed
            } else {
                &img.radio_box
            };
            painter.paint_image_full(bx, by, bw, bh, box_img, 255, Color::TRANSPARENT);
        });

        // C++ lines 333-340: Pressed && !BoxPressed → GroupInnerBorder overlay.
        if self.pressed && !self.box_pressed {
            let r = cr.h * 0.2;
            with_toolkit_images(|img| {
                painter.paint_border_image(
                    cr.x,
                    cr.y,
                    cr.w,
                    cr.h,
                    r,
                    r,
                    r,
                    r,
                    &img.group_inner_border,
                    225,
                    225,
                    225,
                    225,
                    255,
                    Color::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            });
        }
    }

    /// Rounded-rect hit test matching C++ `emButton::CheckMouse` outer hit.
    fn hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let (rect, r) = self.border.content_round_rect(1.0, tallness, &self.look);
        super::check_mouse_round_rect(mx, my, &rect, r)
    }

    /// Box-specific hit test matching C++ `emButton::CheckMouse` inBox check.
    /// Tests whether (mx, my) is within the radio circle's face area.
    fn box_hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let cr = self.border.content_rect(1.0, tallness, &self.look);
        let (_bx0, _by0, bw0, _lx, _ly, _lw, _lh) = self.box_label_geometry(&cr);

        let d = bw0 * 0.13;
        let bx = cr.x + d;
        let by = cr.y + (cr.h - bw0) * 0.5 + d;
        let bw = bw0 - 2.0 * d;
        let bh = bw;

        let d2 = bw * 30.0 / 380.0;
        let fx = bx + d2;
        let fy = by + d2;
        let fw = bw - 2.0 * d2;
        let fh = bh - 2.0 * d2;
        // ShownRadioed: fr = fw * 0.5 (fully circular).
        let fr = fw * 0.5;

        let dx = ((fx - mx).max(mx - fx - fw) + fr).max(0.0);
        let dy = ((fy - my).max(my - fy - fh) + fr).max(0.0);
        dx * dx + dy * dy <= fr * fr
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        let trace = super::trace_input_enabled();
        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        eprintln!(
                            "    [RadioBox {:?}] Press mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed
                        );
                    }
                    if !hit {
                        return false;
                    }
                    self.pressed = true;
                    self.box_pressed =
                        self.box_hit_test(event.mouse_x, event.mouse_y);
                    true
                }
                InputVariant::Release => {
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        eprintln!(
                            "    [RadioBox {:?}] Release mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed={} box_pressed={} selected_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed, self.box_pressed, self.is_selected()
                        );
                    }
                    if !hit {
                        return false;
                    }
                    if self.pressed {
                        self.pressed = false;
                        self.box_pressed = false;
                        self.group.borrow_mut().select(self.index);
                    }
                    true
                }
                _ => false,
            },
            InputKey::Space => match event.variant {
                InputVariant::Press => {
                    self.pressed = true;
                    true
                }
                InputVariant::Release => {
                    if self.pressed {
                        self.pressed = false;
                        self.box_pressed = false;
                        self.group.borrow_mut().select(self.index);
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
        let th = 13.0;
        let tw = Painter::measure_text_width(&self.border.caption, th);
        self.border.preferred_size_for_content(tw + 8.0, th + 4.0)
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

        // Mouse clicks require paint; use Space for unit test.
        rb0.input(&InputEvent::press(InputKey::Space));
        assert!(!rb0.is_selected()); // Not selected yet on press
        rb0.input(&InputEvent::release(InputKey::Space));
        assert!(rb0.is_selected());
        assert!(!rb1.is_selected());

        rb1.input(&InputEvent::press(InputKey::Space));
        rb1.input(&InputEvent::release(InputKey::Space));
        assert!(!rb0.is_selected());
        assert!(rb1.is_selected());
    }

    #[test]
    fn pressed_state_tracks_press_release() {
        let look = Look::new();
        let group = RadioGroup::new();
        let mut rb = RadioBox::new("X", look, group.clone(), 0);
        assert!(!rb.pressed);
        rb.input(&InputEvent::press(InputKey::Space));
        assert!(rb.pressed);
        rb.input(&InputEvent::release(InputKey::Space));
        assert!(!rb.pressed);
    }
}
