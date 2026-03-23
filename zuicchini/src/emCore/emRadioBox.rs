use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::emCore::emColor::emColor;
use crate::emCore::rect::Rect;
use crate::emCore::emCursor::emCursor;
use crate::emCore::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emCore::emInputState::emInputState;
use crate::emCore::emPanel::PanelState;
use crate::emCore::emPainter::{emPainter, BORDER_EDGES_ONLY};

use super::emBorder::{emBorder, OuterBorderType};
use crate::emCore::emLook::emLook;
use crate::emCore::emRadioButton::RadioGroup;
use crate::emCore::toolkit_images::with_toolkit_images;

/// Small radio box widget — box indicator with label text.
///
/// C++ `emRadioBox` inherits `emRadioButton : emCheckButton : emButton : emBorder`.
/// Constructor sets: `OBT_MARGIN`, `LabelAlignment=LEFT`, `ShownBoxed=true`,
/// `ShownRadioed=true`.
///
/// Paint uses the C++ DoButton boxed+radioed path (emButton.cpp:233-341):
/// content_rect → box geometry → circular face → radio dot → emRadioBox image overlay.
pub struct emRadioBox {
    border: emBorder,
    look: Rc<emLook>,
    group: Rc<RefCell<RadioGroup>>,
    index_cell: Rc<Cell<usize>>,
    pressed: bool,
    box_pressed: bool,
    /// Cached enabled state from the last paint call. Gates input handling.
    enabled: bool,
    last_w: f64,
    last_h: f64,
}

impl emRadioBox {
    pub fn new(label: &str, look: Rc<emLook>, group: Rc<RefCell<RadioGroup>>, _index: usize) -> Self {
        let index_cell = group.borrow_mut().register();
        Self {
            border: emBorder::new(OuterBorderType::Margin)
                .with_caption(label)
                .with_label_in_border(false)
                .with_label_alignment(crate::emCore::emPainter::TextAlignment::Left)
                .with_how_to(true),
            look,
            group,
            index_cell,
            pressed: false,
            box_pressed: false,
            enabled: true,
            last_w: 0.0,
            last_h: 0.0,
        }
    }

    pub fn index(&self) -> usize {
        self.index_cell.get()
    }

    pub fn set_index(&mut self, index: usize) {
        self.index_cell.set(index);
    }

    pub fn is_selected(&self) -> bool {
        self.group.borrow().GetChecked() == Some(self.index_cell.get())
    }

    pub fn set_checked(&mut self, checked: bool) {
        if checked {
            self.group.borrow_mut().SetChecked(self.index_cell.get());
        } else if self.is_selected() {
            self.group.borrow_mut().SetCheckIndex(None);
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
    pub fn paint(&mut self, painter: &mut emPainter, w: f64, h: f64, enabled: bool) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;
        self.border
            .paint_border(painter, w, h, &self.look, false, true, 1.0);

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
                .paint_label(painter, Rect::new(lx, ly, lw, lh), &self.look, enabled);
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
                emColor::TRANSPARENT,
            );
        }

        // Paint radio box image overlay (C++ lines 318-331).
        // BoxPressed → RadioBoxPressed image, else → emRadioBox image.
        with_toolkit_images(|img| {
            let box_img = if self.box_pressed {
                &img.radio_box_pressed
            } else {
                &img.radio_box
            };
            painter.paint_image_full(bx, by, bw, bh, box_img, 255, emColor::TRANSPARENT);
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
                    emColor::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            });
        }

        // C++ DoButton: disabled gray overlay for boxed+radioed path.
        // PaintRoundRect(fx, fy, fw, fh, fr, fr, 0x888888E0).
        if !enabled {
            painter.paint_round_rect(fx, fy, fw, fh, fr, emColor::rgba(0x88, 0x88, 0x88, 0xE0));
        }
    }

    /// Rounded-rect hit test matching C++ `emButton::CheckMouse` boxed path.
    /// Uses content_rect with r = h * 0.2 (C++ emButton.cpp:276).
    fn hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let rect = self.border.content_rect(1.0, tallness, &self.look);
        let r = rect.h * 0.2;
        super::widget_utils::check_mouse_round_rect(mx, my, &rect, r)
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

    pub fn input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        let trace = super::widget_utils::trace_input_enabled();
        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    // C++ emButton.cpp:82: (state.IsNoMod() || state.IsShiftMod())
                    if event.ctrl || event.alt || event.meta {
                        return false;
                    }
                    // C++ emButton.cpp:84: GetViewCondition(VCT_MIN_EXT) >= 8.0
                    let min_ext = state.viewed_rect.w.min(state.viewed_rect.h);
                    if min_ext < 8.0 {
                        return false;
                    }
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
                    if !self.pressed {
                        return false;
                    }
                    // C++ emButton.cpp:101: IsViewed check on release.
                    if !state.viewed {
                        self.pressed = false;
                        self.box_pressed = false;
                        return true;
                    }
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        eprintln!(
                            "    [RadioBox {:?}] Release mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed={} box_pressed={} selected_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed, self.box_pressed, self.is_selected()
                        );
                    }
                    self.pressed = false;
                    self.box_pressed = false;
                    if hit {
                        self.group.borrow_mut().SetChecked(self.index_cell.get());
                    }
                    true
                }
                _ => false,
            },
            // C++ emButton.cpp:113-119: Enter only, instant Click().
            // Gated on (IsNoMod || IsShiftMod).
            InputKey::Enter
                if event.variant == InputVariant::Press
                    && !event.alt
                    && !event.meta
                    && !event.ctrl
                    && state.viewed_rect.w.min(state.viewed_rect.h) >= 8.0 =>
            {
                self.group.borrow_mut().SetChecked(self.index_cell.get());
                true
            }
            _ => false,
        }
    }

    pub fn get_cursor(&self) -> emCursor {
        emCursor::Normal
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let th = 13.0;
        let tw = emPainter::measure_text_width(&self.border.caption, th);
        self.border.preferred_size_for_content(tw + 8.0, th + 4.0)
    }
}

impl Drop for emRadioBox {
    fn drop(&mut self) {
        self.group.borrow_mut().deregister(&self.index_cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emCore::rect::Rect;
    use crate::emCore::emPanelTree::PanelId;
    use slotmap::Key as _;

    fn default_panel_state() -> PanelState {
        PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        }
    }

    fn default_input_state() -> emInputState {
        emInputState::new()
    }

    #[test]
    fn radio_box_selection() {
        let look = emLook::new();
        let group = RadioGroup::new();

        let mut rb0 = emRadioBox::new("X", look.clone(), group.clone(), 0);
        let mut rb1 = emRadioBox::new("Y", look, group.clone(), 1);
        let ps = default_panel_state();
        let is = default_input_state();

        assert!(!rb0.is_selected());
        assert!(!rb1.is_selected());

        // Enter is instant: selects on press, no release needed.
        rb0.input(&emInputEvent::press(InputKey::Enter), &ps, &is);
        assert!(rb0.is_selected()); // Selected immediately on press
        assert!(!rb1.is_selected());

        rb1.input(&emInputEvent::press(InputKey::Enter), &ps, &is);
        assert!(!rb0.is_selected());
        assert!(rb1.is_selected());
    }

    #[test]
    fn pressed_state_tracks_press_release() {
        // Enter is instant — no visual press state. Verify pressed stays false.
        let look = emLook::new();
        let group = RadioGroup::new();
        let mut rb = emRadioBox::new("X", look, group.clone(), 0);
        let ps = default_panel_state();
        let is = default_input_state();
        assert!(!rb.pressed);
        rb.input(&emInputEvent::press(InputKey::Enter), &ps, &is);
        assert!(!rb.pressed); // Enter selects instantly, no press state
        assert!(rb.is_selected()); // But the selection did happen
    }
}
