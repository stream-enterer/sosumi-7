use std::rc::Rc;

use crate::emColor::emColor;
use crate::emCursor::emCursor;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::{emPainter, BORDER_EDGES_ONLY};
use crate::emPanel::PanelState;
use crate::emPanel::Rect;
use crate::emStroke::{emStroke, emStrokeEnd, LineCap, LineJoin, StrokeEndType};

use super::emBorder::{emBorder, OuterBorderType};
use crate::emBorder::with_toolkit_images;
use crate::emButton::{HOWTO_BUTTON, HOWTO_EOI_BUTTON};
use crate::emEngineCtx::{ConstructCtx, PanelCtx, WidgetCallback};
use crate::emLook::emLook;
use crate::emSignal::SignalId;

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
    pub on_check: Option<WidgetCallback<bool>>,
    /// Mirrors emCheckButton's CheckSignal.
    ///
    /// DIVERGED: (language-forced) C++ `emCheckBox` inherits `GetCheckSignal()` from
    /// `emCheckButton` (public inheritance chain emCheckBox → emCheckButton
    /// → emButton → emBorder → emPanel). Rust lacks C++ inheritance, so we
    /// mirror `emCheckButton::CheckSignal` explicitly on `emCheckBox` to
    /// preserve observable signal-dispatch behavior. Spec §3.5 D6.1. Phase-3
    /// B3.4c: allocated and fired at Input-driven toggle sites.
    pub check_signal: SignalId,
}

impl emCheckBox {
    pub fn new<C: ConstructCtx>(ctx: &mut C, label: &str, look: Rc<emLook>) -> Self {
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
            check_signal: ctx.create_signal(),
        }
    }

    /// Set the border description text. Matches C++ `emCheckBox::SetDescription`.
    pub fn SetDescription(&mut self, desc: &str) {
        self.border.description = desc.to_string();
    }

    pub fn IsChecked(&self) -> bool {
        self.checked
    }

    pub fn SetChecked(&mut self, checked: bool, ctx: &mut PanelCtx<'_>) {
        if self.checked != checked {
            self.checked = checked;
            // Mirrors emCheckButton::SetChecked C++ order.
            if let Some(mut sched) = ctx.as_sched_ctx() {
                sched.fire(self.check_signal);
                if let Some(cb) = self.on_check.as_mut() {
                    cb(self.checked, &mut sched);
                }
            }
        }
    }

    /// Test-only setter that bypasses signal firing. Used by B-010 row 299/300/301
    /// integration tests in `tests/rc_shim_b010.rs` to pre-stage `IsChecked()`
    /// state before firing the captured `check_signal` directly. Production code
    /// must use `SetChecked` (which atomically updates state + fires the signal).
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn set_checked_for_test(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Set checked state without firing `check_signal`. Used by `update_output`
    /// in panel groups to sync checkbox display from config after Reset without
    /// causing a feedback loop or requiring `PanelCtx`.
    pub fn set_checked_silent(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Compute the box + label geometry from the content rect (C++ lines 235-260).
    /// Returns (bx0, by0, bw0, lx, ly, lw, lh) where bx0/by0/bw0 are the outer
    /// box dimensions and lx/ly/lw/lh are the label area.
    fn box_label_geometry(&self, cr: &Rect) -> (f64, f64, f64, f64, f64, f64, f64) {
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
    pub fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        enabled: bool,
        pixel_scale: f64,
    ) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;
        self.border.how_to_text = self.GetHowTo(enabled, true);
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            false,
            true,
            pixel_scale,
        );
        let canvas_color = self
            .border
            .content_canvas_color(canvas_color, &self.look, enabled);

        // C++ DoButton ShownBoxed path — emButton.cpp:233-341
        let cr = self.border.GetContentRect(w, h, &self.look);
        let x = cr.x;
        let y = cr.y;
        let cw = cr.w;
        let ch = cr.h;

        let has_label = self.border.HasLabel();
        let (mut bx, mut by, mut bw, mut bh, mut lx, mut ly, mut lw, mut lh);
        if has_label {
            lw = 1.0;
            lh = self.border.GetBestLabelTallness().max(0.2);
            bw = lh;
            let mut d = bw * 0.1;
            let f = (cw / (bw + d + lw)).min(ch / lh);
            bw *= f;
            d *= f;
            lw = cw - bw - d;
            lh = bw;
            lx = x + cw - lw;
            ly = y + (ch - lh) * 0.5;
        } else {
            bw = cw.min(ch);
            lx = x;
            ly = y;
            lw = 1e-100;
            lh = 1e-100;
        }
        bh = bw;
        bx = x;
        by = y + (ch - bh) * 0.5;

        // Image area inset (C++ line 262)
        let d = bw * 0.13;
        bx += d;
        by += d;
        bw -= 2.0 * d;
        bh -= 2.0 * d;

        // Face inset (C++ line 268)
        let d = bw * 30.0 / 380.0;
        let mut fx = bx + d;
        let fy = by + d;
        let fw = bw - 2.0 * d;
        let fh = bh - 2.0 * d;
        let fr = bw * 50.0 / 380.0; // Not radioed → bw*50/380

        let r = ch * 0.2;

        // C++ lines 290-291: label color
        let mut color = self.look.fg_color;
        if !enabled {
            color = color.GetTransparented(75.0);
        }

        // C++ lines 293-308: label with press nudge
        if has_label {
            if self.pressed && !self.box_pressed {
                bx += lw * 0.003;
                fx += lw * 0.003;
                lx += lw * 0.003;
                ly += lh * 0.007;
                lw *= 0.986;
                lh *= 0.986;
            }
            self.border.paint_label_colored(
                painter,
                canvas_color,
                Rect::new(lx, ly, lw, lh),
                &self.look,
                color,
                true,
            );
        }

        // C++ lines 310-312: face
        let face_color = self.look.input_bg_color;
        painter.PaintRoundRect(fx, fy, fw, fh, fr, fr, face_color, canvas_color);
        let canvas_color = face_color;

        // C++ line 314: PaintBoxSymbol (emButton.cpp:160-184) — checkbox checkmark
        if self.checked {
            let check_color = self.look.input_fg_color;
            let verts = [
                (fx + fw * 0.2, fy + fh * 0.6),
                (fx + fw * 0.4, fy + fh * 0.8),
                (fx + fw * 0.8, fy + fh * 0.2),
            ];
            let mut stroke = emStroke::new(check_color, fw * 0.16);
            stroke.cap = LineCap::Round;
            stroke.join = LineJoin::Round;
            stroke.start_end = emStrokeEnd::new(StrokeEndType::Cap);
            stroke.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
            painter.PaintPolyline(&verts, &stroke, false, canvas_color);
        }

        // C++ line 316: disabled overlay
        if !enabled {
            painter.PaintRoundRect(
                fx,
                fy,
                fw,
                fh,
                fr,
                fr,
                emColor::rgba(0x88, 0x88, 0x88, 0xE0),
                emColor::TRANSPARENT,
            );
        }

        // C++ lines 318-331: PaintImage (checkbox image overlay)
        with_toolkit_images(|img| {
            let box_img = if self.box_pressed {
                &img.check_box_pressed
            } else {
                &img.check_box
            };
            painter.paint_image_full(bx, by, bw, bh, box_img, 255, emColor::TRANSPARENT);
        });

        // C++ lines 333-340: Pressed && !BoxPressed → GroupInnerBorder overlay
        if self.pressed && !self.box_pressed {
            with_toolkit_images(|img| {
                painter.PaintBorderImage(
                    x,
                    y,
                    cw,
                    ch,
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
        // RUST_ONLY: (language-forced-utility) widget_utils.rs -- C++ inlines this formula per widget
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

    pub fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        _input_state: &emInputState,
        ctx: &mut crate::emEngineCtx::PanelCtx,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        // RUST_ONLY: (language-forced-utility) widget_utils.rs -- debug trace aid, no C++ equivalent
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
                    self.box_pressed = self.box_hit_test(event.mouse_x, event.mouse_y);
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
                        self.toggle(ctx);
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
                self.toggle(ctx);
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

    /// Internal toggle helper (private). Implements the action C++ does in
    /// its protected virtual `Clicked()` override.
    fn toggle(&mut self, ctx: &mut PanelCtx<'_>) {
        self.checked = !self.checked;
        // Mirrors emCheckButton::SetChecked C++ order
        // (invalidate → Signal(CheckSignal) → CheckChanged).
        if let Some(mut sched) = ctx.as_sched_ctx() {
            sched.fire(self.check_signal);
            if let Some(cb) = self.on_check.as_mut() {
                cb(self.checked, &mut sched);
            }
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
    use crate::emEngineCtx::{DeferredAction, InitCtx, PanelCtx};
    use crate::emPanel::Rect;
    use crate::emPanelTree::{PanelId, PanelTree};
    use crate::emScheduler::EngineScheduler;
    use slotmap::Key as _;

    fn test_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let id = tree.create_root("t", false);
        (tree, id)
    }

    /// Minimal construction-ctx bundle for widget unit tests.
    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<crate::emContext::emContext>,
        pa: Rc<std::cell::RefCell<Vec<crate::emEngineCtx::FrameworkDeferredAction>>>,
    }
    impl Drop for TestInit {
        fn drop(&mut self) {
            // B3.4c: clear pending signals accumulated during Input-path tests
            self.sched.clear_pending_for_tests();
        }
    }

    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: crate::emContext::emContext::NewRoot(),
                pa: Rc::new(std::cell::RefCell::new(Vec::new())),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
                pending_actions: &self.pa,
            }
        }
    }

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
        let mut init = TestInit::new();
        let mut cb = emCheckBox::new(&mut init.ctx(), "Enable", look);
        let ps = default_panel_state();
        let is = default_input_state();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        assert!(!cb.IsChecked());
        // Enter is instant: toggles on press, no release needed.
        cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(cb.IsChecked()); // Toggled immediately on press
        cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!cb.IsChecked());
    }

    #[test]
    fn pressed_state_tracks_press_release() {
        // Enter is instant — no visual press state. Verify pressed stays false.
        let look = emLook::new();
        let mut init = TestInit::new();
        let mut cb = emCheckBox::new(&mut init.ctx(), "Enable", look);
        let ps = default_panel_state();
        let is = default_input_state();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        assert!(!cb.pressed);
        cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!cb.pressed); // Enter toggles instantly, no press state
        assert!(cb.IsChecked()); // But the toggle did happen
    }

    #[test]
    fn check_box_fires_check_signal_on_toggle() {
        let look = emLook::new();
        let mut init = TestInit::new();
        let mut cb = emCheckBox::new(&mut init.ctx(), "Enable", look);
        let sig = cb.check_signal;
        let ps = default_panel_state();
        let is = default_input_state();
        let (mut tree, tid) = test_tree();
        let fw_cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut init.sched,
                &mut init.fw,
                &init.root,
                &fw_cb,
                &init.pa,
            );
            cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        }
        assert!(init.sched.is_pending(sig));
    }

    #[test]
    fn checkbox_preferred_size() {
        let look = emLook::new();
        let mut init = TestInit::new();
        let cb = emCheckBox::new(&mut init.ctx(), "Hi", look);
        let (w, h) = cb.preferred_size();
        assert!(w > 0.0, "Should have positive width");
        assert!(h > 0.0, "Should have positive height");
    }
}
