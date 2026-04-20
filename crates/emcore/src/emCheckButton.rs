use std::rc::Rc;

use crate::emColor::emColor;
use crate::emCursor::emCursor;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::{emPainter, BORDER_EDGES_ONLY};
use crate::emPanel::PanelState;
use crate::emPanel::Rect;

use super::emBorder::{emBorder, OuterBorderType};
use crate::emBorder::with_toolkit_images;
use crate::emButton::HOWTO_BUTTON;
use crate::emEngineCtx::{ConstructCtx, PanelCtx, WidgetCallback};
use crate::emLook::emLook;
use crate::emSignal::SignalId;

/// Toggle button widget — visually depressed when checked.
pub struct emCheckButton {
    border: emBorder,
    look: Rc<emLook>,
    checked: bool,
    pressed: bool,
    /// Cached enabled state from the last paint call. Gates input handling.
    enabled: bool,
    last_w: f64,
    last_h: f64,
    // DIVERGED: GetCheckSignal — replaced by callback field `on_check`
    // DIVERGED: CheckChanged — folded into `on_check` callback invocation
    pub on_check: Option<WidgetCallback<bool>>,
    /// Allocated at construction per C++ `emCheckButton::GetCheckSignal()`.
    /// B3.4b allocates; B3.4c fires in Input; B3.4d cleans up DIVERGED blocks.
    pub check_signal: SignalId,
}

impl emCheckButton {
    pub fn new<C: ConstructCtx>(ctx: &mut C, caption: &str, look: Rc<emLook>) -> Self {
        let mut border = emBorder::new(OuterBorderType::InstrumentMoreRound)
            .with_caption(caption)
            .with_label_in_border(false)
            .with_how_to(true);
        // C++ emButton constructor: SetLabelAlignment(EM_ALIGN_CENTER)
        border.SetLabelAlignment(crate::emPainter::TextAlignment::Center);
        Self {
            border,
            look,
            checked: false,
            pressed: false,
            enabled: true,
            last_w: 0.0,
            last_h: 0.0,
            on_check: None,
            check_signal: ctx.create_signal(),
        }
    }

    /// Set the border description text. Matches C++ `emCheckButton::SetDescription`.
    pub fn SetDescription(&mut self, desc: &str) {
        self.border.description = desc.to_string();
    }

    pub fn IsChecked(&self) -> bool {
        self.checked
    }

    pub fn SetChecked(&mut self, checked: bool, ctx: &mut PanelCtx<'_>) {
        if self.checked != checked {
            self.checked = checked;
            if let Some(cb) = self.on_check.as_mut() {
                if let Some(mut sched) = ctx.as_sched_ctx() {
                    cb(self.checked, &mut sched);
                }
            }
        }
    }

    /// Paint using the non-boxed C++ DoButton path (emButton.cpp:343-421).
    ///
    /// emCheckButton renders as a normal button face with centered label.
    /// When checked (ShownChecked=true), the label is slightly shrunk and
    /// a ButtonChecked overlay is painted instead of the normal emButton overlay.
    pub fn Paint(
        &mut self,
        painter: &mut emPainter,
        w: f64,
        h: f64,
        enabled: bool,
        pixel_scale: f64,
    ) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;
        self.border.how_to_text = self.GetHowTo(enabled, true);
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
        let canvas_color = painter.GetCanvasColor();

        // C++ DoButton non-boxed path — emButton.cpp:345-422
        let (cr, r) = self.border.GetContentRoundRect(w, h, &self.look);
        let x = cr.x;
        let y = cr.y;
        let cw = cr.w;
        let ch = cr.h;
        let r = r.max(cw.min(ch) * self.border.border_scaling * 0.223);

        let d = (1.0 - (264.0 - 14.0) / 264.0) * r;
        let fx = x + d;
        let fy = y + d;
        let fw = cw - 2.0 * d;
        let fh = ch - 2.0 * d;
        let fr = r - d;

        let face_color = self.look.button_bg_color;
        painter.PaintRoundRect(fx, fy, fw, fh, fr, fr, face_color, canvas_color);
        painter.SetCanvasColor(face_color);

        let d = fw.min(fh) * 0.1;
        let dx = (r * 0.7).max(d);
        let dy = (r * 0.4).max(d);
        let mut lx = fx + dx;
        let mut ly = fy + dy;
        let mut lw = fw - 2.0 * dx;
        let mut lh = fh - 2.0 * dy;
        if self.pressed || self.checked {
            let d = if self.pressed { 0.98 } else { 0.983 };
            lx += (1.0 - d) * 0.5 * lw;
            lw *= d;
            ly += (1.0 - d) * 0.5 * lh;
            lh *= d;
        }
        let mut color = self.look.button_fg_color;
        if !enabled {
            color = color.GetTransparented(75.0);
        }
        self.border.paint_label_colored(
            painter,
            Rect::new(lx, ly, lw, lh),
            &self.look,
            color,
            true,
        );

        with_toolkit_images(|img| {
            if self.pressed {
                painter.PaintBorderImage(
                    x,
                    y,
                    cw,
                    ch,
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
            } else if self.checked {
                painter.PaintBorderImage(
                    x,
                    y,
                    cw,
                    ch,
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
                painter.PaintBorderImage(
                    x,
                    y,
                    cw + (658.0 - 648.0) / 264.0 * r,
                    ch + (658.0 - 648.0) / 264.0 * r,
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
    /// Tests against the face rect (content rect with face inset).
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
                    // C++ emButton.cpp:101: IsViewed check on release.
                    if !state.viewed {
                        self.pressed = false;
                        return true;
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
                        self.toggle(ctx);
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

    /// Whether this check button provides how-to help text.
    /// Matches C++ `emCheckButton::HasHowTo` (inherited from emButton, always true).
    pub fn HasHowTo(&self) -> bool {
        true
    }

    /// Help text describing how to use this check button.
    ///
    /// Chains the border's base how-to with button + check-button specific
    /// sections. Matches C++ `emCheckButton::GetHowTo`.
    pub fn GetHowTo(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.GetHowTo(enabled, focusable);
        text.push_str(HOWTO_BUTTON);
        text.push_str(HOWTO_CHECK_BUTTON);
        if self.checked {
            text.push_str(HOWTO_CHECKED);
        } else {
            text.push_str(HOWTO_NOT_CHECKED);
        }
        text
    }

    // DIVERGED: Clicked — renamed to toggle (private); C++ Clicked is protected virtual
    fn toggle(&mut self, ctx: &mut PanelCtx<'_>) {
        self.checked = !self.checked;
        if let Some(cb) = self.on_check.as_mut() {
            if let Some(mut sched) = ctx.as_sched_ctx() {
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
    use std::cell::RefCell;

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
    }
    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: crate::emContext::emContext::NewRoot(),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
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
    fn toggle_state() {
        let look = emLook::new();
        let mut init = TestInit::new();
        let mut btn = emCheckButton::new(&mut init.ctx(), "Toggle", look);
        let ps = default_panel_state();
        let is = default_input_state();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        assert!(!btn.IsChecked());
        // Enter is instant: toggles on press, no release needed.
        btn.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(btn.IsChecked()); // Toggled immediately on press
        btn.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!btn.IsChecked());
    }

    #[test]
    fn pressed_state_tracks_press_release() {
        // Enter is instant — no visual press state. Verify pressed stays false.
        let look = emLook::new();
        let mut init = TestInit::new();
        let mut btn = emCheckButton::new(&mut init.ctx(), "CB", look);
        let ps = default_panel_state();
        let is = default_input_state();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        assert!(!btn.pressed);
        btn.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!btn.pressed); // Enter toggles instantly, no press state
        assert!(btn.IsChecked()); // But the toggle did happen
    }

    #[test]
    #[ignore = "B3.3: callback requires scheduler reach; B3.4 restores dispatch"]
    fn callback_receives_state() {
        let look = emLook::new();
        let states = Rc::new(RefCell::new(Vec::new()));
        let states_clone = states.clone();

        let mut init = TestInit::new();
        let mut btn = emCheckButton::new(&mut init.ctx(), "CB", look);
        btn.on_check = Some(Box::new(
            move |checked, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                states_clone.borrow_mut().push(checked);
            },
        ));
        let ps = default_panel_state();
        let is = default_input_state();

        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        // Enter is instant: each press fires the callback immediately.
        btn.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        btn.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert_eq!(*states.borrow(), vec![true, false]);
    }
}
