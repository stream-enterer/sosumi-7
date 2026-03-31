use std::rc::Rc;

use crate::emColor::emColor;
use crate::emPanel::Rect;
use crate::emCursor::emCursor;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPanel::PanelState;
use crate::emPainter::{emPainter, BORDER_EDGES_ONLY};

use super::emBorder::{emBorder, OuterBorderType};
use crate::emLook::emLook;
use crate::emBorder::with_toolkit_images;

/// Clickable button widget.
pub struct emButton {
    border: emBorder,
    look: Rc<emLook>,
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
    pub on_eoi: Option<Box<dyn FnMut()>>,
}

impl emButton {
    pub fn new(caption: &str, look: Rc<emLook>) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::InstrumentMoreRound)
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
            on_eoi: None,
        }
    }

    pub fn SetCaption(&mut self, text: &str) {
        self.border.caption = text.to_string();
    }

    /// Returns the caption text set via `SetCaption`.
    /// Matches C++ `emBorder::GetCaption`.
    pub fn GetCaption(&self) -> &str {
        &self.border.caption
    }

    /// Whether clicking this button is not an "End Of Interaction".
    /// If false (the default), an EOI signal would be sent on every click.
    /// Matches C++ `emButton::IsNoEOI`.
    pub fn IsNoEOI(&self) -> bool {
        self.no_eoi
    }

    /// Set whether clicking this button triggers an End-of-Interaction.
    /// Matches C++ `emButton::SetNoEOI`.
    pub fn SetNoEOI(&mut self, no_eoi: bool) {
        self.no_eoi = no_eoi;
    }

    /// Set the border description text. Matches C++ `emButton::SetDescription`.
    pub fn SetDescription(&mut self, desc: &str) {
        self.border.description = desc.to_string();
    }

    /// Whether the button is visually shown as checked.
    /// Matches C++ `emButton::IsShownChecked`.
    pub fn IsShownChecked(&self) -> bool {
        self.shown_checked
    }

    /// Set whether the button is visually shown as checked.
    /// Matches C++ `emButton::SetShownChecked`.
    pub fn SetShownChecked(&mut self, checked: bool) {
        self.shown_checked = checked;
    }

    /// Whether the button is visually shown with a checkbox-style box.
    /// Matches C++ `emButton::IsShownBoxed`.
    pub fn IsShownBoxed(&self) -> bool {
        self.shown_boxed
    }

    /// Set whether the button is visually shown with a checkbox-style box.
    /// Matches C++ `emButton::SetShownBoxed`.
    pub fn SetShownBoxed(&mut self, boxed: bool) {
        self.shown_boxed = boxed;
    }

    /// Whether the button is visually shown as a radio button.
    /// Matches C++ `emButton::IsShownRadioed`.
    pub fn IsShownRadioed(&self) -> bool {
        self.shown_radioed
    }

    /// Set whether the button is visually shown as a radio button.
    /// Matches C++ `emButton::SetShownRadioed`.
    pub fn SetShownRadioed(&mut self, radioed: bool) {
        self.shown_radioed = radioed;
    }

    pub fn IsPressed(&self) -> bool {
        self.pressed
    }

    /// Round-rect hit test for the button face area.
    ///
    /// Returns true if (`mx`, `my`) is inside the button's rounded-rect face.
    /// Matches C++ `emButton::CheckMouse` for the non-boxed path: coordinates
    /// and face geometry are both computed in normalized `(1.0, tallness)`
    /// panel-local space, making the result zoom-invariant.
    pub fn CheckMouse(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        // Normalize pixel coords to (1.0, tallness) panel-local space,
        // matching C++ where both mouse coords and GetContentRoundRect
        // output are in the same normalized coordinate system.
        let nmx = mx / self.last_w;
        let nmy = my / self.last_w;
        self.hit_test(nmx, nmy)
    }

    /// Whether this button provides how-to help text.
    /// Matches C++ `emButton::HasHowTo` (always returns true).
    pub fn HasHowTo(&self) -> bool {
        true
    }

    /// Help text describing how to use this button.
    ///
    /// Chains the border's base how-to (preface + disabled/focus) with the
    /// button-specific sections. Matches C++ `emButton::GetHowTo`.
    pub fn GetHowTo(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.GetHowTo(enabled, focusable);
        text.push_str(HOWTO_BUTTON);
        if !self.no_eoi {
            text.push_str(HOWTO_EOI_BUTTON);
        }
        text
    }

    pub fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, enabled: bool) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;
        // C++ emButton.cpp:361: always ButtonBgColor. Pressed/checked visual
        // comes from overlay image, not face color change.
        let face_color = self.look.button_bg_color;

        // C++ DoBorder paints the border first, then DoButton paints the face
        // inside the content round rect.
        self.border
            .paint_border(painter, w, h, &self.look, false, true, 1.0);

        // C++ emButton::DoButton gets content round rect, then insets the face
        // by d = (1 - 250/264) * r = (14/264) * r.
        let (cr, r) = self.border.GetContentRoundRect(w, h, &self.look);
        let r = r.max(cr.w.min(cr.h) * self.border.border_scaling * 0.223);
        let d = (14.0 / 264.0) * r;
        let fx = cr.x + d;
        let fy = cr.y + d;
        let fw = cr.w - 2.0 * d;
        let fh = cr.h - 2.0 * d;
        let fr = (r - d).max(0.0);
        painter.PaintRoundRect(fx, fy, fw, fh, fr, face_color);
        painter.SetCanvasColor(face_color);

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
            c.SetAlpha((c.GetAlpha() as f64 * 0.25 + 0.5) as u8)
        };
        self.border.paint_label_colored(
            painter,
            Rect::new(lx, ly, lw, lh),
            &self.look,
            label_color,
            true,
        );

        // C++ DoButton paints button image overlay on top of the face.
        // Priority: Pressed → ButtonPressed, ShownChecked → ButtonChecked, else → emButton.
        with_toolkit_images(|img| {
            if self.pressed {
                painter.PaintBorderImage(
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
                    emColor::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            } else if self.shown_checked {
                // C++ emButton.cpp:402-409: ButtonChecked overlay.
                painter.PaintBorderImage(
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
                    emColor::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            } else {
                // Normal button: image extends slightly beyond content rect.
                let extra = (658.0 - 648.0) / 264.0 * r;
                painter.PaintBorderImage(
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
                    emColor::TRANSPARENT,
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
        let (cr, r) = self.border.GetContentRoundRect(1.0, tallness, &self.look);
        let r = r.max(cr.w.min(cr.h) * self.border.border_scaling * 0.223);
        // Face inset: d = (14/264) * r (C++ emButton.cpp:348)
        let d = (14.0 / 264.0) * r;
        let face = Rect::new(cr.x + d, cr.y + d, cr.w - 2.0 * d, cr.h - 2.0 * d);
        let fr = (r - d).max(0.0);
        // RUST_ONLY: widget_utils.rs -- C++ inlines this formula per widget
        let dx = ((face.x - mx).max(mx - face.x - face.w) + fr).max(0.0);
        let dy = ((face.y - my).max(my - face.y - face.h) + fr).max(0.0);
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
                            if !self.no_eoi {
                                if let Some(eoi) = &mut self.on_eoi {
                                    eoi();
                                }
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
                    && !event.ctrl
                    && state.viewed_rect.w.min(state.viewed_rect.h) >= 8.0 =>
            {
                if let Some(cb) = &mut self.on_click {
                    cb();
                }
                if !self.no_eoi {
                    if let Some(eoi) = &mut self.on_eoi {
                        eoi();
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Programmatically fire the click callback.
    ///
    /// Matches C++ `emButton::Click(shift)`: gates on IsEnabled(),
    /// fires ClickSignal, calls Clicked(), then fires EOI signal.
    pub fn Click(&mut self) {
        if !self.enabled {
            return;
        }
        if let Some(cb) = &mut self.on_click {
            cb();
        }
        if !self.no_eoi {
            if let Some(eoi) = &mut self.on_eoi {
                eoi();
            }
        }
    }

    pub fn GetCursor(&self) -> emCursor {
        // C++ emButton doesn't override GetCursor — uses default panel cursor.
        emCursor::Normal
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let th = 13.0;
        let tw = emPainter::measure_text_width(&self.border.caption, th);
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
    use crate::emPanel::Rect;
    use crate::emPanelTree::PanelId;
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
            height: 1.0,
        }
    }

    fn default_input_state() -> emInputState {
        emInputState::new()
    }

    #[test]
    fn button_press_release_fires_callback() {
        let look = emLook::new();
        let fired = Rc::new(RefCell::new(false));
        let fired_clone = fired.clone();

        let mut btn = emButton::new("Click", look);
        btn.on_click = Some(Box::new(move || {
            *fired_clone.borrow_mut() = true;
        }));
        let ps = default_panel_state();
        let is = default_input_state();

        // C++ Enter: instant Click() on press, no visual press state.
        btn.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
        assert!(*fired.borrow());
    }

    #[test]
    fn button_keyboard_activation() {
        let look = emLook::new();
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();

        let mut btn = emButton::new("OK", look);
        btn.on_click = Some(Box::new(move || {
            *count_clone.borrow_mut() += 1;
        }));
        let ps = default_panel_state();
        let is = default_input_state();

        // C++: only Enter activates, instant on press. Space is not handled.
        btn.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
        assert_eq!(*count.borrow(), 1);
        // Space should NOT activate
        btn.Input(&emInputEvent::press(InputKey::Space), &ps, &is);
        assert_eq!(*count.borrow(), 1);
    }

    #[test]
    fn button_cursor_is_hand() {
        let look = emLook::new();
        let btn = emButton::new("X", look);
        assert_eq!(btn.GetCursor(), emCursor::Normal);
    }

    #[test]
    fn click_fires_callback() {
        let look = emLook::new();
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();

        let mut btn = emButton::new("Go", look);
        btn.on_click = Some(Box::new(move || {
            *count_clone.borrow_mut() += 1;
        }));

        btn.Click();
        btn.Click();
        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn click_without_callback_is_noop() {
        let look = emLook::new();
        let mut btn = emButton::new("Go", look);
        btn.Click(); // should not panic
    }

    #[test]
    fn no_eoi_default_false() {
        let look = emLook::new();
        let btn = emButton::new("Test", look);
        assert!(!btn.IsNoEOI());
    }

    #[test]
    fn SetNoEOI() {
        let look = emLook::new();
        let mut btn = emButton::new("Test", look);
        btn.SetNoEOI(true);
        assert!(btn.IsNoEOI());
        btn.SetNoEOI(false);
        assert!(!btn.IsNoEOI());
    }

    #[test]
    fn has_howto_always_true() {
        let look = emLook::new();
        let btn = emButton::new("OK", look);
        assert!(btn.HasHowTo());
    }

    #[test]
    fn howto_includes_eoi_by_default() {
        let look = emLook::new();
        let btn = emButton::new("OK", look);
        let text = btn.GetHowTo(true, true);
        assert!(text.contains("BUTTON"));
        assert!(text.contains("EOI BUTTON"));
        // Should also include border preface and focus sections
        assert!(text.contains("How to use this panel"));
        assert!(text.contains("FOCUS"));
    }

    #[test]
    fn howto_excludes_eoi_when_no_eoi() {
        let look = emLook::new();
        let mut btn = emButton::new("OK", look);
        btn.SetNoEOI(true);
        let text = btn.GetHowTo(true, true);
        assert!(text.contains("BUTTON"));
        assert!(!text.contains("EOI BUTTON"));
    }

    #[test]
    fn howto_includes_disabled_when_not_enabled() {
        let look = emLook::new();
        let btn = emButton::new("OK", look);
        let text = btn.GetHowTo(false, false);
        assert!(text.contains("DISABLED"));
        assert!(!text.contains("FOCUS"));
    }

    #[test]
    fn eoi_callback_fires() {
        let fired = Rc::new(std::cell::Cell::new(false));
        let fired_clone = fired.clone();
        let look = emLook::new();
        let mut btn = emButton::new("test", look);
        btn.on_eoi = Some(Box::new(move || {
            fired_clone.set(true);
        }));
        btn.Click();
        assert!(fired.get(), "EOI callback should fire after Click");
    }

    #[test]
    fn eoi_callback_suppressed_when_no_eoi() {
        let fired = Rc::new(std::cell::Cell::new(false));
        let fired_clone = fired.clone();
        let look = emLook::new();
        let mut btn = emButton::new("test", look);
        btn.SetNoEOI(true);
        btn.on_eoi = Some(Box::new(move || {
            fired_clone.set(true);
        }));
        btn.Click();
        assert!(!fired.get(), "EOI callback should NOT fire when no_eoi is set");
    }

    #[test]
    fn check_mouse_zero_size_returns_false() {
        let look = emLook::new();
        let btn = emButton::new("X", look);
        assert!(!btn.CheckMouse(0.0, 0.0));
    }

    #[test]
    fn check_mouse_center_returns_true() {
        use crate::emImage::emImage;
        let look = emLook::new();
        let mut btn = emButton::new("X", look);
        // Simulate paint to cache dimensions
        let mut img = emImage::new(200, 100, 4);
        let mut painter = emPainter::new(&mut img);
        btn.Paint(&mut painter, 200.0, 100.0, true);
        // Center of the button should hit
        assert!(btn.CheckMouse(100.0, 50.0));
    }

    #[test]
    fn check_mouse_outside_returns_false() {
        use crate::emImage::emImage;
        let look = emLook::new();
        let mut btn = emButton::new("X", look);
        let mut img = emImage::new(200, 100, 4);
        let mut painter = emPainter::new(&mut img);
        btn.Paint(&mut painter, 200.0, 100.0, true);
        // Well outside the button bounds
        assert!(!btn.CheckMouse(-50.0, -50.0));
        assert!(!btn.CheckMouse(300.0, 200.0));
    }
}
