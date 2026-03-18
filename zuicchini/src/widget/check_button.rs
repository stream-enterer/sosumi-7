use std::rc::Rc;

use crate::foundation::{Color, Rect};
use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::{Painter, BORDER_EDGES_ONLY};

use super::border::{Border, OuterBorderType};
use super::look::Look;
use super::toolkit_images::with_toolkit_images;

/// Toggle button widget — visually depressed when checked.
pub struct CheckButton {
    border: Border,
    look: Rc<Look>,
    checked: bool,
    pressed: bool,
    last_w: f64,
    last_h: f64,
    pub on_check: Option<Box<dyn FnMut(bool)>>,
}

impl CheckButton {
    pub fn new(caption: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::InstrumentMoreRound)
                .with_caption(caption)
                .with_label_in_border(false)
                .with_how_to(true),
            look,
            checked: false,
            pressed: false,
            last_w: 0.0,
            last_h: 0.0,
            on_check: None,
        }
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        if self.checked != checked {
            self.checked = checked;
            if let Some(cb) = &mut self.on_check {
                cb(self.checked);
            }
        }
    }

    /// Paint using the non-boxed C++ DoButton path (emButton.cpp:343-421).
    ///
    /// CheckButton renders as a normal button face with centered label.
    /// When checked (ShownChecked=true), the label is slightly shrunk and
    /// a ButtonChecked overlay is painted instead of the normal Button overlay.
    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64) {
        self.last_w = w;
        self.last_h = h;
        self.border
            .paint_border(painter, w, h, &self.look, false, true);

        // C++ DoButton non-boxed path: GetContentRoundRect, clamp r.
        let (cr, r) = self.border.content_round_rect(w, h, &self.look);
        let r = r.max(cr.w.min(cr.h) * self.border.border_scaling * 0.223);

        // Face inset: d = (14/264) * r (C++ line 348).
        let d = (14.0 / 264.0) * r;
        let fx = cr.x + d;
        let fy = cr.y + d;
        let fw = cr.w - 2.0 * d;
        let fh = cr.h - 2.0 * d;
        let fr = (r - d).max(0.0);

        // C++ emButton.cpp:361: always ButtonBgColor. Pressed visual from overlay.
        let face_color = self.look.button_bg_color;
        painter.paint_round_rect(fx, fy, fw, fh, fr, face_color);
        painter.set_canvas_color(face_color);

        // Label inside face with padding (C++ lines 370-391).
        let d_min = fw.min(fh) * 0.1;
        let dx = (r * 0.7).max(d_min);
        let dy = (r * 0.4).max(d_min);
        let mut lx = fx + dx;
        let mut ly = fy + dy;
        let mut lw = fw - 2.0 * dx;
        let mut lh = fh - 2.0 * dy;

        // C++ line 377-382: Pressed → 0.98, ShownChecked → 0.983.
        // Pressed takes priority.
        if self.pressed || self.checked {
            let s = if self.pressed { 0.98 } else { 0.983 };
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

        // Button overlay image (C++ lines 393-421).
        // Priority: Pressed → ButtonPressed, ShownChecked → ButtonChecked, else → Button.
        with_toolkit_images(|img| {
            if self.pressed {
                // Pressed: ButtonPressed overlay (C++ lines 393-401).
                painter.paint_border_image(
                    cr.x,
                    cr.y,
                    cr.w,
                    cr.h,
                    360.0 / 264.0 * r,
                    374.0 / 264.0 * r,
                    r,
                    r,
                    &img.button_pressed,
                    360,
                    374,
                    264,
                    264,
                    255,
                    Color::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            } else if self.checked {
                // ShownChecked: ButtonChecked overlay (C++ lines 402-409).
                painter.paint_border_image(
                    cr.x,
                    cr.y,
                    cr.w,
                    cr.h,
                    340.0 / 264.0 * r,
                    374.0 / 264.0 * r,
                    r,
                    r,
                    &img.button_checked,
                    340,
                    374,
                    264,
                    264,
                    255,
                    Color::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            } else {
                // Normal: Button overlay (C++ lines 411-420).
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

    /// Rounded-rect hit test matching C++ `emButton::CheckMouse` non-boxed path.
    /// Tests against the face rect (content rect with face inset).
    fn hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let (cr, r) = self.border.content_round_rect(1.0, tallness, &self.look);
        let r = r.max(cr.w.min(cr.h) * self.border.border_scaling * 0.223);
        // Face inset: d = (14/264) * r (C++ emButton.cpp:348)
        let d = (14.0 / 264.0) * r;
        let face = Rect::new(cr.x + d, cr.y + d, cr.w - 2.0 * d, cr.h - 2.0 * d);
        let fr = (r - d).max(0.0);
        super::check_mouse_round_rect(mx, my, &face, fr)
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        let trace = super::trace_input_enabled();
        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    // C++ emButton.cpp:82: (state.IsNoMod() || state.IsShiftMod())
                    if event.ctrl || event.alt || event.meta {
                        return false;
                    }
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        eprintln!(
                            "    [CheckButton {:?}] Press mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed
                        );
                    }
                    if !hit {
                        return false;
                    }
                    self.pressed = true;
                    true
                }
                InputVariant::Release => {
                    if !self.pressed {
                        return false;
                    }
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        eprintln!(
                            "    [CheckButton {:?}] Release mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed={} checked_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed, self.checked
                        );
                    }
                    self.pressed = false;
                    if hit {
                        self.toggle();
                    }
                    true
                }
                _ => false,
            },
            // C++ emButton.cpp:113-119: Enter only, instant Click().
            InputKey::Enter if event.variant == InputVariant::Press => {
                self.toggle();
                true
            }
            InputKey::Enter => true,
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

    /// Whether this check button provides how-to help text.
    /// Matches C++ `emCheckButton::HasHowTo` (inherited from emButton, always true).
    pub fn has_how_to(&self) -> bool {
        true
    }

    /// Help text describing how to use this check button.
    ///
    /// Chains the border's base how-to with button + check-button specific
    /// sections. Matches C++ `emCheckButton::GetHowTo`.
    pub fn get_how_to(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.get_howto(enabled, focusable);
        text.push_str(HOWTO_CHECK_BUTTON);
        if self.checked {
            text.push_str(HOWTO_CHECKED);
        } else {
            text.push_str(HOWTO_NOT_CHECKED);
        }
        text
    }

    fn toggle(&mut self) {
        self.checked = !self.checked;
        if let Some(cb) = &mut self.on_check {
            cb(self.checked);
        }
    }
}

/// C++ `emCheckButton::HowToCheckButton`.
const HOWTO_CHECK_BUTTON: &str = "\n\n\
    CHECK BUTTON\n\n\
    This button can have checked or unchecked state. Usually this is a yes-or-no\n\
    answer to a question. Whenever the button is triggered, the check state toggles.\n";

/// C++ `emCheckButton::HowToChecked`.
const HOWTO_CHECKED: &str = "\n\n\
    CHECKED\n\n\
    Currently this check button is checked.\n";

/// C++ `emCheckButton::HowToNotChecked`.
const HOWTO_NOT_CHECKED: &str = "\n\n\
    UNCHECKED\n\n\
    Currently this check button is not checked.\n";

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn toggle_state() {
        let look = Look::new();
        let mut btn = CheckButton::new("Toggle", look);
        assert!(!btn.is_checked());
        // Enter is instant: toggles on press, no release needed.
        btn.input(&InputEvent::press(InputKey::Enter));
        assert!(btn.is_checked()); // Toggled immediately on press
        btn.input(&InputEvent::press(InputKey::Enter));
        assert!(!btn.is_checked());
    }

    #[test]
    fn pressed_state_tracks_press_release() {
        // Enter is instant — no visual press state. Verify pressed stays false.
        let look = Look::new();
        let mut btn = CheckButton::new("CB", look);
        assert!(!btn.pressed);
        btn.input(&InputEvent::press(InputKey::Enter));
        assert!(!btn.pressed); // Enter toggles instantly, no press state
        assert!(btn.is_checked()); // But the toggle did happen
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

        // Enter is instant: each press fires the callback immediately.
        btn.input(&InputEvent::press(InputKey::Enter));
        btn.input(&InputEvent::press(InputKey::Enter));
        assert_eq!(*states.borrow(), vec![true, false]);
    }
}
