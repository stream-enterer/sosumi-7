use std::rc::Rc;

use crate::foundation::{Color, Rect};
use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::{LineCap, LineJoin, Painter, Stroke, BORDER_EDGES_ONLY};

use super::border::{Border, OuterBorderType};
use super::look::Look;
use super::toolkit_images::with_toolkit_images;

/// CheckBox widget — Margin border with ShownBoxed paint path.
/// Matches C++ `emCheckBox` (which extends `emCheckButton` extends `emButton`).
///
/// C++ constructor chain:
///   emButton: OBT_INSTRUMENT_MORE_ROUND, LabelInBorder=false, ALIGN_CENTER
///   emCheckBox overrides: OBT_MARGIN, ALIGN_LEFT, ShownBoxed=true
pub struct CheckBox {
    border: Border,
    look: Rc<Look>,
    checked: bool,
    pressed: bool,
    box_pressed: bool,
    last_w: f64,
    last_h: f64,
    pub on_check: Option<Box<dyn FnMut(bool)>>,
}

impl CheckBox {
    pub fn new(label: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::Margin)
                .with_caption(label)
                .with_label_in_border(false)
                .with_label_alignment(crate::render::TextAlignment::Left)
                .with_how_to(true),
            look,
            checked: false,
            pressed: false,
            box_pressed: false,
            last_w: 0.0,
            last_h: 0.0,
            on_check: None,
        }
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Compute the box + label geometry from the content rect (C++ lines 235-260).
    /// Returns (bx0, by0, bw0, lx, ly, lw, lh) where bx0/by0/bw0 are the outer
    /// box dimensions and lx/ly/lw/lh are the label area.
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

    /// Paint using the C++ ShownBoxed path (emButton.cpp:233-341).
    ///
    /// Layout: small checkbox box on the left, label text on the right.
    /// The box contains: InputBgColor face → checkmark symbol → CheckBox image overlay.
    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64) {
        self.last_w = w;
        self.last_h = h;
        // Paint outer border (Margin = transparent spacing only).
        self.border
            .paint_border(painter, w, h, &self.look, false, true);

        // C++ DoButton ShownBoxed: GetContentRect, then compute box + label geometry.
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
        let fr = bw * 50.0 / 380.0;

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

        // Paint face (InputBgColor).
        let face_color = self.look.input_bg_color;
        painter.paint_round_rect(fx, fy, fw, fh, fr, face_color);
        painter.set_canvas_color(face_color);

        // Paint check symbol if checked (C++ PaintBoxSymbol, emButton.cpp:160-184).
        if self.checked {
            let check_color = self.look.input_fg_color;
            let verts = [
                (fx + fw * 0.2, fy + fh * 0.6),
                (fx + fw * 0.4, fy + fh * 0.8),
                (fx + fw * 0.8, fy + fh * 0.2),
            ];
            let mut stroke = Stroke::new(check_color, fw * 0.16);
            stroke.join = LineJoin::Round;
            stroke.cap = LineCap::Round;
            painter.paint_solid_polyline(&verts, &stroke, false, Color::TRANSPARENT);
        }

        // Paint checkbox image overlay (C++ lines 318-331).
        // BoxPressed → CheckBoxPressed image, else → CheckBox image.
        with_toolkit_images(|img| {
            let box_img = if self.box_pressed {
                &img.check_box_pressed
            } else {
                &img.check_box
            };
            painter.paint_image_full(bx, by, bw, bh, box_img, 255, Color::TRANSPARENT);
        });

        // C++ lines 333-340: Pressed && !BoxPressed → GroupInnerBorder overlay.
        if self.pressed && !self.box_pressed {
            // Outer hit-test radius: r = h * 0.2 (C++ line 276).
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
    /// Tests whether (mx, my) is within the checkbox square's face area.
    /// Coordinates are in normalized panel space (0..1, 0..tallness).
    fn box_hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let cr = self.border.content_rect(1.0, tallness, &self.look);
        let (_bx0, _by0, bw0, _lx, _ly, _lw, _lh) = self.box_label_geometry(&cr);

        // Inset for image area: d = bw * 0.13 (C++ line 262).
        let d = bw0 * 0.13;
        let bx = cr.x + d;
        let by = cr.y + (cr.h - bw0) * 0.5 + d;
        let bw = bw0 - 2.0 * d;
        let bh = bw;

        // Face inset (C++ line 268-274).
        let d2 = bw * 30.0 / 380.0;
        let fx = bx + d2;
        let fy = by + d2;
        let fw = bw - 2.0 * d2;
        let fh = bh - 2.0 * d2;
        let fr = bw * 50.0 / 380.0;

        // C++ rounded-rect hit test on face rect.
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
                            "    [CheckBox {:?}] Press mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed_before={}",
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
                            "    [CheckBox {:?}] Release mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed={} box_pressed={} checked_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed, self.box_pressed, self.checked
                        );
                    }
                    if !hit {
                        return false;
                    }
                    if self.pressed {
                        self.pressed = false;
                        self.box_pressed = false;
                        self.toggle();
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
                        self.toggle();
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
        // Mouse clicks require paint for hit test; use Space (keyboard).
        cb.input(&InputEvent::press(InputKey::Space));
        assert!(!cb.is_checked()); // Not toggled yet on press
        cb.input(&InputEvent::release(InputKey::Space));
        assert!(cb.is_checked()); // Toggled on release
        cb.input(&InputEvent::press(InputKey::Space));
        cb.input(&InputEvent::release(InputKey::Space));
        assert!(!cb.is_checked());
    }

    #[test]
    fn pressed_state_tracks_press_release() {
        let look = Look::new();
        let mut cb = CheckBox::new("Enable", look);
        assert!(!cb.pressed);
        cb.input(&InputEvent::press(InputKey::Space));
        assert!(cb.pressed);
        cb.input(&InputEvent::release(InputKey::Space));
        assert!(!cb.pressed);
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
