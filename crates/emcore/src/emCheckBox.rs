use std::rc::Rc;

use crate::emColor::emColor;
use crate::emPanel::Rect;
use crate::emCursor::emCursor;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPanel::PanelState;
use crate::emPainter::{emPainter, BORDER_EDGES_ONLY};
use crate::emStroke::{LineJoin, emStroke, emStrokeEnd, StrokeEndType};

use super::emBorder::{emBorder, OuterBorderType};
use crate::emLook::emLook;
use crate::emBorder::with_toolkit_images;
use crate::emButton::{HOWTO_BUTTON, HOWTO_EOI_BUTTON};

/// emCheckBox widget — Margin border with ShownBoxed paint path.
/// Matches C++ `emCheckBox` (which extends `emCheckButton` extends `emButton`).
///
/// C++ constructor chain:
///   emButton: OBT_INSTRUMENT_MORE_ROUND, LabelInBorder=false, ALIGN_CENTER
///   emCheckBox overrides: OBT_MARGIN, ALIGN_LEFT, ShownBoxed=true
pub struct emCheckBox {
    border: emBorder,
    look: Rc<emLook>,
    checked: bool,
    pressed: bool,
    box_pressed: bool,
    /// Cached enabled state from the last paint call. Gates input handling.
    enabled: bool,
    last_w: f64,
    last_h: f64,
    // DIVERGED: GetCheckSignal — replaced by callback field `on_check` (inherited from emCheckButton)
    // DIVERGED: CheckChanged — folded into `on_check` callback invocation
    pub on_check: Option<Box<dyn FnMut(bool)>>,
}

impl emCheckBox {
    pub fn new(label: &str, look: Rc<emLook>) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::Margin)
                .with_caption(label)
                .with_label_in_border(false)
                .with_label_alignment(crate::emPainter::TextAlignment::Left)
                .with_how_to(true),
            look,
            checked: false,
            pressed: false,
            box_pressed: false,
            enabled: true,
            last_w: 0.0,
            last_h: 0.0,
            on_check: None,
        }
    }

    pub fn IsChecked(&self) -> bool {
        self.checked
    }

    pub fn SetChecked(&mut self, checked: bool) {
        if self.checked != checked {
            self.checked = checked;
            if let Some(cb) = &mut self.on_check {
                cb(self.checked);
            }
        }
    }

    /// Compute the box + label geometry from the content rect (C++ lines 235-260).
    /// Returns (bx0, by0, bw0, lx, ly, lw, lh) where bx0/by0/bw0 are the outer
    /// box dimensions and lx/ly/lw/lh are the label area.
    fn box_label_geometry(
        &self,
        cr: &Rect,
    ) -> (f64, f64, f64, f64, f64, f64, f64) {
        let has_label = self.border.HasLabel();
        if has_label {
            let label_tallness = self.border.GetBestLabelTallness().max(0.2);
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
    /// The box contains: InputBgColor face → checkmark symbol → emCheckBox image overlay.
    pub fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, enabled: bool, pixel_scale: f64) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;
        // Paint outer border (Margin = transparent spacing only).
        self.border.how_to_text = self.GetHowTo(enabled, true);
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);

        // C++ DoButton ShownBoxed: GetContentRect, then compute box + label geometry.
        let cr = self.border.GetContentRect(w, h, &self.look);
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
        if self.border.HasLabel() {
            self.border
                .paint_label(painter, Rect::new(lx, ly, lw, lh), &self.look, enabled);
        }

        // Paint face (InputBgColor).
        let face_color = self.look.input_bg_color;
        painter.PaintRoundRect(fx, fy, fw, fh, fr, face_color, painter.GetCanvasColor());
        painter.SetCanvasColor(face_color);

        // Paint check symbol if checked (C++ PaintBoxSymbol, emButton.cpp:160-184).
        if self.checked {
            let check_color = self.look.input_fg_color;
            let verts = [
                (fx + fw * 0.2, fy + fh * 0.6),
                (fx + fw * 0.4, fy + fh * 0.8),
                (fx + fw * 0.8, fy + fh * 0.2),
            ];
            let mut stroke = emStroke::new(check_color, fw * 0.16);
            stroke.join = LineJoin::Round;
            stroke.start_end = emStrokeEnd::new(StrokeEndType::Cap);
            stroke.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
            painter.PaintSolidPolyline(&verts, &stroke, false, face_color);
        }

        // Paint checkbox image overlay (C++ lines 318-331).
        // BoxPressed → CheckBoxPressed image, else → emCheckBox image.
        with_toolkit_images(|img| {
            let box_img = if self.box_pressed {
                &img.check_box_pressed
            } else {
                &img.check_box
            };
            painter.paint_image_full(bx, by, bw, bh, box_img, 255, emColor::TRANSPARENT);
        });

        // C++ lines 333-340: Pressed && !BoxPressed → GroupInnerBorder overlay.
        if self.pressed && !self.box_pressed {
            // Outer hit-test radius: r = h * 0.2 (C++ line 276).
            let r = cr.h * 0.2;
            with_toolkit_images(|img| {
                painter.PaintBorderImage(
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

        // C++ DoButton: disabled gray overlay for boxed path.
        // PaintRoundRect(fx, fy, fw, fh, fr, fr, 0x888888E0).
        if !enabled {
            painter.PaintRoundRect(fx, fy, fw, fh, fr, emColor::rgba(0x88, 0x88, 0x88, 0xE0), emColor::TRANSPARENT);
        }
    }

    /// Rounded-rect hit test matching C++ `emButton::CheckMouse` boxed path.
    /// Uses content_rect with r = h * 0.2 (C++ emButton.cpp:276).
    fn hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let rect = self.border.GetContentRect(1.0, tallness, &self.look);
        let r = rect.h * 0.2;
        // RUST_ONLY: widget_utils.rs -- C++ inlines this formula per widget
        let dx = ((rect.x - mx).max(mx - rect.x - rect.w) + r).max(0.0);
        let dy = ((rect.y - my).max(my - rect.y - rect.h) + r).max(0.0);
        dx * dx + dy * dy <= r * r
    }

    /// Box-specific hit test matching C++ `emButton::CheckMouse` inBox check.
    /// Tests whether (mx, my) is within the checkbox square's face area.
    /// Coordinates are in normalized panel space (0..1, 0..tallness).
    fn box_hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let cr = self.border.GetContentRect(1.0, tallness, &self.look);
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

    pub fn Input(&mut self, event: &emInputEvent, state: &PanelState, _input_state: &emInputState) -> bool {
        if !self.enabled {
            return false;
        }
        // RUST_ONLY: widget_utils.rs -- debug trace aid, no C++ equivalent
        let trace = {
            use std::sync::OnceLock;
            static ENABLED: OnceLock<bool> = OnceLock::new();
            *ENABLED.get_or_init(|| std::env::var("TRACE_INPUT").is_ok())
        };
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
                            "    [CheckBox {:?}] Release mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed={} box_pressed={} checked_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed, self.box_pressed, self.checked
                        );
                    }
                    self.pressed = false;
                    self.box_pressed = false;
                    if hit {
                        self.toggle();
                    }
                    true
                }
                _ => false,
            },
            // C++ emButton.cpp:113-119: Enter only, instant Click().
            // Gated on (IsNoMod || IsShiftMod). No press visual, no Space.
            InputKey::Enter
                if event.variant == InputVariant::Press
                    && !event.alt
                    && !event.meta
                    && !event.ctrl
                    && state.viewed_rect.w.min(state.viewed_rect.h) >= 8.0 =>
            {
                self.toggle();
                true
            }
            _ => false,
        }
    }

    pub fn GetCursor(&self) -> emCursor {
        emCursor::Normal
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let th = 13.0;
        let tw = emPainter::measure_text_width(&self.border.caption, th);
        self.border.preferred_size_for_content(tw + 8.0, th + 4.0)
    }

    /// Help text describing how to use this checkbox.
    ///
    /// Chains border → button → check-button sections.
    /// C++ emCheckBox inherits `emCheckButton::GetHowTo`.
    pub fn GetHowTo(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.GetHowTo(enabled, focusable);
        text.push_str(HOWTO_BUTTON);
        text.push_str(HOWTO_EOI_BUTTON);
        text.push_str(HOWTO_CHECK_BUTTON);
        if self.checked {
            text.push_str(HOWTO_CHECKED);
        } else {
            text.push_str(HOWTO_NOT_CHECKED);
        }
        text
    }

    // DIVERGED: Clicked — renamed to toggle (private); C++ Clicked is protected virtual
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
    use crate::emPanel::Rect;
    use crate::emPanelTree::PanelId;
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
    fn checkbox_toggle() {
        let look = emLook::new();
        let mut cb = emCheckBox::new("Enable", look);
        let ps = default_panel_state();
        let is = default_input_state();
        assert!(!cb.IsChecked());
        // Enter is instant: toggles on press, no release needed.
        cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
        assert!(cb.IsChecked()); // Toggled immediately on press
        cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
        assert!(!cb.IsChecked());
    }

    #[test]
    fn pressed_state_tracks_press_release() {
        // Enter is instant — no visual press state. Verify pressed stays false.
        let look = emLook::new();
        let mut cb = emCheckBox::new("Enable", look);
        let ps = default_panel_state();
        let is = default_input_state();
        assert!(!cb.pressed);
        cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
        assert!(!cb.pressed); // Enter toggles instantly, no press state
        assert!(cb.IsChecked()); // But the toggle did happen
    }

    #[test]
    fn checkbox_preferred_size() {
        let look = emLook::new();
        let cb = emCheckBox::new("Hi", look);
        let (w, h) = cb.preferred_size();
        assert!(w > 0.0, "Should have positive width");
        assert!(h > 0.0, "Should have positive height");
    }
}
