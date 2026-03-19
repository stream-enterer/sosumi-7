use std::rc::Rc;

use crate::foundation::{Color, Rect};
use crate::input::{Cursor, InputEvent, InputKey, InputState, InputVariant};
use crate::panel::PanelState;
use crate::render::{Painter, BORDER_EDGES_ONLY};

use super::border::{Border, OuterBorderType};
use super::look::Look;
use super::toolkit_images::with_toolkit_images;

/// Clickable button widget.
pub struct Button {
    border: Border,
    look: Rc<Look>,
    pressed: bool,
    /// When true, clicking this button does not send an End-of-Interaction
    /// signal. Matches C++ `emButton::NoEOI`.
    no_eoi: bool,
    /// Visual appearance flags matching C++ `emButton` bit-fields.
    /// These control which visual style is used when painting.
    shown_checked: bool,
    shown_boxed: bool,
    shown_radioed: bool,
    /// Cached enabled state from the last paint call. Gates input handling.
    enabled: bool,
    /// Cached dimensions from the last paint call.
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
                .with_label_in_border(false)
                .with_how_to(true),
            look,
            pressed: false,
            no_eoi: false,
            shown_checked: false,
            shown_boxed: false,
            shown_radioed: false,
            enabled: true,
            last_w: 0.0,
            last_h: 0.0,
            on_click: None,
            on_press_state: None,
        }
    }

    pub fn set_caption(&mut self, text: &str) {
        self.border.caption = text.to_string();
    }

    /// Whether clicking this button is not an "End Of Interaction".
    /// If false (the default), an EOI signal would be sent on every click.
    /// Matches C++ `emButton::IsNoEOI`.
    pub fn is_no_eoi(&self) -> bool {
        self.no_eoi
    }

    /// Set whether clicking this button triggers an End-of-Interaction.
    /// Matches C++ `emButton::SetNoEOI`.
    pub fn set_no_eoi(&mut self, no_eoi: bool) {
        self.no_eoi = no_eoi;
    }

    /// Set the border description text. Matches C++ `emButton::SetDescription`.
    pub fn set_description(&mut self, desc: &str) {
        self.border.description = desc.to_string();
    }

    /// Whether the button is visually shown as checked.
    /// Matches C++ `emButton::IsShownChecked`.
    pub fn is_shown_checked(&self) -> bool {
        self.shown_checked
    }

    /// Set whether the button is visually shown as checked.
    /// Matches C++ `emButton::SetShownChecked`.
    pub fn set_shown_checked(&mut self, checked: bool) {
        self.shown_checked = checked;
    }

    /// Whether the button is visually shown with a checkbox-style box.
    /// Matches C++ `emButton::IsShownBoxed`.
    pub fn is_shown_boxed(&self) -> bool {
        self.shown_boxed
    }

    /// Set whether the button is visually shown with a checkbox-style box.
    /// Matches C++ `emButton::SetShownBoxed`.
    pub fn set_shown_boxed(&mut self, boxed: bool) {
        self.shown_boxed = boxed;
    }

    /// Whether the button is visually shown as a radio button.
    /// Matches C++ `emButton::IsShownRadioed`.
    pub fn is_shown_radioed(&self) -> bool {
        self.shown_radioed
    }

    /// Set whether the button is visually shown as a radio button.
    /// Matches C++ `emButton::SetShownRadioed`.
    pub fn set_shown_radioed(&mut self, radioed: bool) {
        self.shown_radioed = radioed;
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Round-rect hit test for the button face area.
    ///
    /// Returns true if (`mx`, `my`) is inside the button's rounded-rect face.
    /// Matches C++ `emButton::CheckMouse` for the non-boxed path: tests
    /// against the face inset (fx, fy, fw, fh) with corner radius `fr`.
    pub fn check_mouse(&self, mx: f64, my: f64) -> bool {
        let w = self.last_w;
        let h = self.last_h;
        if w <= 0.0 || h <= 0.0 {
            return false;
        }
        let (cr, r) = self.border.content_round_rect(w, h, &self.look);
        let r = r.max(cr.w.min(cr.h) * self.border.border_scaling * 0.223);
        let d = (14.0 / 264.0) * r;
        let fx = cr.x + d;
        let fy = cr.y + d;
        let fw = cr.w - 2.0 * d;
        let fh = cr.h - 2.0 * d;
        let fr = (r - d).max(0.0);
        // C++ round-rect hit test: distance to inset rect expanded by radius
        let dx = ((fx - mx).max(mx - fx - fw) + fr).max(0.0);
        let dy = ((fy - my).max(my - fy - fh) + fr).max(0.0);
        dx * dx + dy * dy <= fr * fr
    }

    /// Whether this button provides how-to help text.
    /// Matches C++ `emButton::HasHowTo` (always returns true).
    pub fn has_how_to(&self) -> bool {
        true
    }

    /// Help text describing how to use this button.
    ///
    /// Chains the border's base how-to (preface + disabled/focus) with the
    /// button-specific sections. Matches C++ `emButton::GetHowTo`.
    pub fn get_how_to(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.get_howto(enabled, focusable);
        text.push_str(HOWTO_BUTTON);
        if !self.no_eoi {
            text.push_str(HOWTO_EOI_BUTTON);
        }
        text
    }

    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, enabled: bool) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;
        // C++ emButton.cpp:361: always ButtonBgColor. Pressed/checked visual
        // comes from overlay image, not face color change.
        let face_color = self.look.button_bg_color;

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
        // C++ emButton.cpp:377-382: Pressed → 0.98, ShownChecked → 0.983.
        if self.pressed || self.shown_checked {
            let s = if self.pressed { 0.98 } else { 0.983 };
            lx += (1.0 - s) * 0.5 * lw;
            lw *= s;
            ly += (1.0 - s) * 0.5 * lh;
            lh *= s;
        }
        let label_color = if enabled {
            self.look.button_fg_color
        } else {
            let c = self.look.button_fg_color;
            c.with_alpha((c.a() as u16 * 64 / 255) as u8)
        };
        self.border.paint_label_colored(
            painter,
            Rect::new(lx, ly, lw, lh),
            &self.look,
            label_color,
            true,
        );

        // C++ DoButton paints button image overlay on top of the face.
        // Priority: Pressed → ButtonPressed, ShownChecked → ButtonChecked, else → Button.
        with_toolkit_images(|img| {
            if self.pressed {
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
            } else if self.shown_checked {
                // C++ emButton.cpp:402-409: ButtonChecked overlay.
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

    /// Rounded-rect hit test matching C++ `emButton::CheckMouse` non-boxed path.
    /// Tests against the face rect (content rect with face inset), not the raw
    /// content rect. Mouse coords are in normalized panel space.
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

    pub fn input(&mut self, event: &InputEvent, state: &PanelState, _input_state: &InputState) -> bool {
        if !self.enabled {
            return false;
        }
        let trace = super::trace_input_enabled();
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
                        let cap = &self.border.caption;
                        eprintln!(
                            "    [Button {:?}] Press mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed_before={}",
                            cap, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed
                        );
                    }
                    if !hit {
                        return false;
                    }
                    self.pressed = true;
                    if let Some(cb) = &mut self.on_press_state {
                        cb(true);
                    }
                    true
                }
                InputVariant::Release => {
                    // C++ clears Pressed unconditionally on release, then
                    // gates Click() on hit test. This prevents stuck pressed
                    // state when mouse moves away between press and release.
                    if !self.pressed {
                        return false;
                    }
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        let cap = &self.border.caption;
                        eprintln!(
                            "    [Button {:?}] Release mouse=({:.4},{:.4}) hit={} pressed_before={}",
                            cap, event.mouse_x, event.mouse_y, hit, self.pressed
                        );
                    }
                    self.pressed = false;
                    if let Some(cb) = &mut self.on_press_state {
                        cb(false);
                    }
                    // C++ emButton.cpp:101-109: CheckMouse && IsEnabled && IsViewed,
                    // then PanelToViewX/Y against ClipRect.
                    if hit && state.viewed {
                        // Panel-to-view transform (C++ emPanel.h:1019-1027):
                        //   viewX = panelX * ViewedWidth + ViewedX
                        //   viewY = panelY * ViewedWidth / PixelTallness + ViewedY
                        let vr = &state.viewed_rect;
                        let cr = &state.clip_rect;
                        let vmx = event.mouse_x * vr.w + vr.x;
                        let vmy = event.mouse_y * vr.w / state.pixel_tallness + vr.y;
                        if vmx >= cr.x && vmx < cr.x + cr.w && vmy >= cr.y && vmy < cr.y + cr.h {
                            if let Some(cb) = &mut self.on_click {
                                cb();
                            }
                        }
                    }
                    true
                }
                _ => false,
            },
            // C++ emButton.cpp:113-119: Enter only, instant Click(), no press
            // visual state. C++ does NOT handle Space for buttons.
            // C++ emButton.cpp:113-119: Enter only, instant Click(), no press
            // visual state. Gated on (IsNoMod || IsShiftMod).
            InputKey::Enter
                if event.variant == InputVariant::Press
                    && !event.alt
                    && !event.meta
                    && !event.ctrl =>
            {
                if let Some(cb) = &mut self.on_click {
                    cb();
                }
                true
            }
            _ => false,
        }
    }

    /// Programmatically fire the click callback.
    ///
    /// Matches C++ `emButton::Click(shift)`: gates on IsEnabled(),
    /// fires ClickSignal, calls Clicked(). EOI signal not implemented.
    pub fn click(&mut self) {
        if !self.enabled {
            return;
        }
        if let Some(cb) = &mut self.on_click {
            cb();
        }
    }

    pub fn get_cursor(&self) -> Cursor {
        // C++ emButton doesn't override GetCursor — uses default panel cursor.
        Cursor::Normal
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let th = 13.0;
        let tw = Painter::measure_text_width(&self.border.caption, th);
        self.border.preferred_size_for_content(tw + 8.0, th + 4.0)
    }
}

/// C++ `emButton::HowToButton`.
pub(crate) const HOWTO_BUTTON: &str = "\n\n\
    BUTTON\n\n\
    This is a button. Buttons can be triggered to perform an application defined\n\
    function.\n\n\
    In order to trigger a button, move the mouse pointer over the button and click\n\
    the left mouse button. The function is triggered when releasing the mouse\n\
    button, but only if the mouse pointer is still over the button.\n\n\
    Alternatively, a button can be triggered by giving it the focus and pressing the\n\
    Enter key.\n";

/// C++ `emButton::HowToEOIButton`.
const HOWTO_EOI_BUTTON: &str = "\n\n\
    EOI BUTTON\n\n\
    This is an End Of Interaction button. The exact behavior is application defined,\n\
    but it usually means that if the button is in a view that has popped up, the\n\
    view is popped down automatically when the button is triggered. If you want to\n\
    bypass that, hold the Shift key while triggering the button.\n";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::Rect;
    use crate::panel::PanelId;
    use slotmap::Key as _;
    use std::cell::RefCell;

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
        }
    }

    fn default_input_state() -> InputState {
        InputState::new()
    }

    #[test]
    fn button_press_release_fires_callback() {
        let look = Look::new();
        let fired = Rc::new(RefCell::new(false));
        let fired_clone = fired.clone();

        let mut btn = Button::new("Click", look);
        btn.on_click = Some(Box::new(move || {
            *fired_clone.borrow_mut() = true;
        }));
        let ps = default_panel_state();
        let is = default_input_state();

        // C++ Enter: instant Click() on press, no visual press state.
        btn.input(&InputEvent::press(InputKey::Enter), &ps, &is);
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
        let ps = default_panel_state();
        let is = default_input_state();

        // C++: only Enter activates, instant on press. Space is not handled.
        btn.input(&InputEvent::press(InputKey::Enter), &ps, &is);
        assert_eq!(*count.borrow(), 1);
        // Space should NOT activate
        btn.input(&InputEvent::press(InputKey::Space), &ps, &is);
        assert_eq!(*count.borrow(), 1);
    }

    #[test]
    fn button_cursor_is_hand() {
        let look = Look::new();
        let btn = Button::new("X", look);
        assert_eq!(btn.get_cursor(), Cursor::Normal);
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

    #[test]
    fn no_eoi_default_false() {
        let look = Look::new();
        let btn = Button::new("Test", look);
        assert!(!btn.is_no_eoi());
    }

    #[test]
    fn set_no_eoi() {
        let look = Look::new();
        let mut btn = Button::new("Test", look);
        btn.set_no_eoi(true);
        assert!(btn.is_no_eoi());
        btn.set_no_eoi(false);
        assert!(!btn.is_no_eoi());
    }

    #[test]
    fn has_howto_always_true() {
        let look = Look::new();
        let btn = Button::new("OK", look);
        assert!(btn.has_how_to());
    }

    #[test]
    fn howto_includes_eoi_by_default() {
        let look = Look::new();
        let btn = Button::new("OK", look);
        let text = btn.get_how_to(true, true);
        assert!(text.contains("BUTTON"));
        assert!(text.contains("EOI BUTTON"));
        // Should also include border preface and focus sections
        assert!(text.contains("How to use this panel"));
        assert!(text.contains("FOCUS"));
    }

    #[test]
    fn howto_excludes_eoi_when_no_eoi() {
        let look = Look::new();
        let mut btn = Button::new("OK", look);
        btn.set_no_eoi(true);
        let text = btn.get_how_to(true, true);
        assert!(text.contains("BUTTON"));
        assert!(!text.contains("EOI BUTTON"));
    }

    #[test]
    fn howto_includes_disabled_when_not_enabled() {
        let look = Look::new();
        let btn = Button::new("OK", look);
        let text = btn.get_how_to(false, false);
        assert!(text.contains("DISABLED"));
        assert!(!text.contains("FOCUS"));
    }

    #[test]
    fn check_mouse_zero_size_returns_false() {
        let look = Look::new();
        let btn = Button::new("X", look);
        assert!(!btn.check_mouse(0.0, 0.0));
    }

    #[test]
    fn check_mouse_center_returns_true() {
        use crate::foundation::Image;
        let look = Look::new();
        let mut btn = Button::new("X", look);
        // Simulate paint to cache dimensions
        let mut img = Image::new(200, 100, 4);
        let mut painter = Painter::new(&mut img);
        btn.paint(&mut painter, 200.0, 100.0, true);
        // Center of the button should hit
        assert!(btn.check_mouse(100.0, 50.0));
    }

    #[test]
    fn check_mouse_outside_returns_false() {
        use crate::foundation::Image;
        let look = Look::new();
        let mut btn = Button::new("X", look);
        let mut img = Image::new(200, 100, 4);
        let mut painter = Painter::new(&mut img);
        btn.paint(&mut painter, 200.0, 100.0, true);
        // Well outside the button bounds
        assert!(!btn.check_mouse(-50.0, -50.0));
        assert!(!btn.check_mouse(300.0, 200.0));
    }
}
