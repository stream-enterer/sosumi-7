// Port of C++ emAutoplayControlPanel (emAutoplay.h:333-398).
//
// DIVERGED: (language-forced) C++ emAutoplayControlPanel extends emPackGroup and uses signal-based
// wiring (AddWakeUpSignal/IsSignaled). Rust uses emPackGroup for layout/border,
// Rc<Cell<...>> flags for action communication (polled by the parent), and
// on_click/on_check/on_value callbacks on widgets.

use std::cell::Cell;
use std::rc::Rc;

use emcore::emBorder::{InnerBorderType, OuterBorderType, emBorder};
use emcore::emButton::emButton;
use emcore::emCheckBox::emCheckBox;
use emcore::emCheckButton::emCheckButton;
use emcore::emColor::emColor;
use emcore::emCursor::emCursor;
use emcore::emEngineCtx::PanelCtx;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLinearLayout::emLinearLayout;
use emcore::emLook::emLook;
use emcore::emPackLayout::emPackLayout;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emScalarField::emScalarField;
use emcore::emTiling::ChildConstraint;

// ── Duration table and conversion ───────────────────────────────────────────

/// Port of C++ DurationTable in emAutoplay.cpp.
const DURATION_TABLE_MS: &[i32] = &[
    500, 1000, 2000, 3000, 5000, 10000, 15000, 30000, 60000, 120000,
];

/// Convert a scalar-field value (0..900) to milliseconds by interpolating in
/// `DURATION_TABLE_MS`.
///
/// Port of C++ `emAutoplayControlPanel::DurationValueToMS`.
pub fn DurationValueToMS(value: i64) -> i32 {
    let n = DURATION_TABLE_MS.len();
    let step = 900.0 / (n as f64 - 1.0);
    let pos = value as f64 / step;
    let idx = pos.floor() as usize;
    if idx >= n - 1 {
        return DURATION_TABLE_MS[n - 1];
    }
    let frac = pos - idx as f64;
    let a = DURATION_TABLE_MS[idx] as f64;
    let b = DURATION_TABLE_MS[idx + 1] as f64;
    (a + frac * (b - a)).round() as i32
}

/// Convert milliseconds back to a scalar-field value (0..900) by binary search.
///
/// Port of C++ `emAutoplayControlPanel::DurationMSToValue`.
pub fn DurationMSToValue(ms: i32) -> i64 {
    // Binary search over the value domain [0, 900].
    let mut lo: i64 = 0;
    let mut hi: i64 = 900;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if DurationValueToMS(mid) < ms {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Port of C++ `emAutoplayControlPanel::DurationTextOfValue`.
/// Converts scalar value to human-readable seconds string.
fn DurationTextOfValue(value: i64, _mark_interval: u64) -> String {
    let seconds = DurationValueToMS(value) as f64 / 1000.0;
    // C++ uses %g format which strips trailing zeros
    if seconds == seconds.floor() {
        format!("{}", seconds as i64)
    } else {
        format!("{}", seconds)
    }
}

// ── AutoplayFlags ──────────────────────────────────────────────────────────
// DIVERGED: (language-forced) C++ uses AddWakeUpSignal/IsSignaled. Rust uses Rc<Cell<...>>
// flags set by widget callbacks and polled by the parent panel's Cycle.

/// Shared state for autoplay control actions.
pub struct AutoplayFlags {
    /// Autoplay toggle was clicked (new checked state).
    pub toggle: Cell<Option<bool>>,
    /// Previous button clicked.
    pub prev: Cell<bool>,
    /// Next button clicked.
    pub next: Cell<bool>,
    /// Continue Last Autoplay button clicked.
    pub continue_last: Cell<bool>,
    /// Duration slider value changed (new value 0..900).
    pub duration_value: Cell<Option<f64>>,
    /// Recursive checkbox toggled (new checked state).
    pub recursive: Cell<Option<bool>>,
    /// Loop checkbox toggled (new checked state).
    pub loop_toggle: Cell<Option<bool>>,
    /// Autoplay progress (0.0 to 1.0), shared with AutoplayCheckButtonPanel.
    pub progress: Rc<Cell<f64>>,
}

impl Default for AutoplayFlags {
    fn default() -> Self {
        Self {
            toggle: Cell::new(None),
            prev: Cell::new(false),
            next: Cell::new(false),
            continue_last: Cell::new(false),
            duration_value: Cell::new(None),
            recursive: Cell::new(None),
            loop_toggle: Cell::new(None),
            progress: Rc::new(Cell::new(0.0)),
        }
    }
}

// ── PanelBehavior wrappers ─────────────────────────────────────────────────
// These wrap emcore widgets for use as PanelBehavior children.

/// PanelBehavior wrapper for emButton.
struct AutoplayButtonPanel {
    button: emButton,
}

impl PanelBehavior for AutoplayButtonPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.button
            .Paint(painter, canvas_color, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.button.Input(event, state, input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.button.GetCursor()
    }

    fn get_title(&self) -> Option<String> {
        Some(self.button.GetCaption().to_string())
    }
}

/// PanelBehavior wrapper for emCheckButton (autoplay toggle).
/// DIVERGED: (language-forced) C++ AutoplayButton overrides PaintLabel to draw a progress bar.
/// Rust draws the progress bar overlay after painting the check button.
struct AutoplayCheckButtonPanel {
    check_button: emCheckButton,
    progress: Rc<Cell<f64>>,
}

impl PanelBehavior for AutoplayCheckButtonPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.check_button
            .Paint(painter, canvas_color, w, h, state.enabled, pixel_scale);

        // BLOCKED: C++ AutoplayButton::PaintLabel draws ellipse-arc progress indicator using PaintEllipseArc.
        // Rust uses a simple rectangle overlay — AutoplayButton::PaintLabel has not been ported yet.
        let progress = self.progress.get();
        if progress > 0.0 {
            let bar_color = emColor::from_packed(0x00AA0080); // green, 50% alpha
            painter.PaintRect(0.0, 0.0, w * progress, h, bar_color, emColor::TRANSPARENT);
        }
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.check_button.Input(event, state, input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.check_button.GetCursor()
    }
}

/// PanelBehavior wrapper for emCheckBox.
struct AutoplayCheckBoxPanel {
    check_box: emCheckBox,
}

impl PanelBehavior for AutoplayCheckBoxPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.check_box
            .Paint(painter, canvas_color, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.check_box.Input(event, state, input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.check_box.GetCursor()
    }
}

/// PanelBehavior wrapper for emScalarField.
struct AutoplayScalarFieldPanel {
    scalar_field: emScalarField,
}

impl PanelBehavior for AutoplayScalarFieldPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.scalar_field
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.scalar_field.Input(event, state, input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.scalar_field.GetCursor()
    }
}

// ── Settings sub-panel ─────────────────────────────────────────────────────
// Port of C++ lSettings: emPackGroup("settings", "Autoplay Settings")

/// Inner pack group for "Autoplay Settings" containing duration, recursive, loop.
struct SettingsPanel {
    layout: emPackLayout,
    border: emBorder,
    look: Rc<emLook>,
    children_created: bool,
    flags: Rc<AutoplayFlags>,
}

impl SettingsPanel {
    fn new(look: Rc<emLook>, flags: Rc<AutoplayFlags>) -> Self {
        Self {
            layout: emPackLayout::new(),
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Autoplay Settings"),
            look,
            children_created: false,
            flags,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::clone(&self.look);

        // ── SfDuration ──
        // C++ SetPrefChildTallness(0, 0.15), SetChildWeight(0, 1.0)
        let flags = Rc::clone(&self.flags);
        let mut sf = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emScalarField::new(&mut sched, 0.0, 900.0, Rc::clone(&look))
        };
        sf.SetCaption("Duration");
        sf.SetDescription(
            "Number of seconds autoplay shall show each\n\
             item that has no playback function.",
        );
        sf.SetEditable(true);
        sf.SetScaleMarkIntervals(&[100, 20, 5]);
        sf.SetTextOfValueFunc(Box::new(DurationTextOfValue));
        sf.on_value = Some(Box::new(
            move |v, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                flags.duration_value.set(Some(v));
            },
        ));
        let sf_id = ctx.create_child_with(
            "duration",
            Box::new(AutoplayScalarFieldPanel { scalar_field: sf }),
        );
        self.layout.set_child_constraint(
            sf_id,
            ChildConstraint {
                weight: 1.0,
                preferred_tallness: 0.15,
                ..Default::default()
            },
        );

        // ── CbRecursive ──
        // C++ SetPrefChildTallness(1, 0.15), SetChildWeight(1, 0.75)
        let flags = Rc::clone(&self.flags);
        let mut cb_recursive = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(&mut sched, "Recursive", Rc::clone(&look))
        };
        cb_recursive.SetDescription("Whether autoplay shall play subdirectories recursively.");
        cb_recursive.on_check = Some(Box::new(
            move |checked, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                flags.recursive.set(Some(checked));
            },
        ));
        let recursive_id = ctx.create_child_with(
            "recursive",
            Box::new(AutoplayCheckBoxPanel {
                check_box: cb_recursive,
            }),
        );
        self.layout.set_child_constraint(
            recursive_id,
            ChildConstraint {
                weight: 0.75,
                preferred_tallness: 0.15,
                ..Default::default()
            },
        );

        // ── CbLoop ──
        // C++ SetPrefChildTallness(2, 0.15), SetChildWeight(2, 0.75)
        let flags = Rc::clone(&self.flags);
        let mut cb_loop = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(&mut sched, "Loop", Rc::clone(&look))
        };
        cb_loop.SetDescription(
            "Whether autoplay shall start from the beginning\n\
             after reaching the end.",
        );
        cb_loop.on_check = Some(Box::new(
            move |checked, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                flags.loop_toggle.set(Some(checked));
            },
        ));
        let loop_id = ctx.create_child_with(
            "loop",
            Box::new(AutoplayCheckBoxPanel { check_box: cb_loop }),
        );
        self.layout.set_child_constraint(
            loop_id,
            ChildConstraint {
                weight: 0.75,
                preferred_tallness: 0.15,
                ..Default::default()
            },
        );

        self.children_created = true;
    }
}

impl PanelBehavior for SettingsPanel {
    fn get_title(&self) -> Option<String> {
        Some("Autoplay Settings".to_string())
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border.paint_border(
            painter,
            w,
            h,
            &self.look,
            state.is_focused(),
            state.enabled,
            pixel_scale,
        );
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.create_children(ctx);
        }
        let aux_id = emcore::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn auto_expand(&self) -> bool {
        true
    }
}

// ── PrevNext sub-panel ─────────────────────────────────────────────────────
// Port of C++ lPrevNext: emLinearLayout("prev_next")

/// Linear layout for Previous / Next buttons.
struct PrevNextPanel {
    layout: emLinearLayout,
    children_created: bool,
    look: Rc<emLook>,
    flags: Rc<AutoplayFlags>,
}

impl PrevNextPanel {
    fn new(look: Rc<emLook>, flags: Rc<AutoplayFlags>) -> Self {
        // C++ SetOrientationThresholdTallness(0.7)
        Self {
            layout: emLinearLayout::adaptive(0.7),
            children_created: false,
            look,
            flags,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::clone(&self.look);

        // ── BtPrev ──
        let flags = Rc::clone(&self.flags);
        let mut btn_prev = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Previous", Rc::clone(&look))
        };
        btn_prev.SetDescription(
            "Skip to previous autoplay item. This also works\n\
             when autoplay is off, for a manual show.\n\n\
             Hotkey: Shift+F12, backward button of the mouse",
        );
        btn_prev.on_click = Some(Box::new(
            move |(), _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                flags.prev.set(true);
            },
        ));
        ctx.create_child_with("prev", Box::new(AutoplayButtonPanel { button: btn_prev }));

        // ── BtNext ──
        let flags = Rc::clone(&self.flags);
        let mut btn_next = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Next", Rc::clone(&look))
        };
        btn_next.SetDescription(
            "Skip to next autoplay item. This also works\n\
             when autoplay is off, for a manual show.\n\n\
             Hotkeys: F12, forward button of the mouse",
        );
        btn_next.on_click = Some(Box::new(
            move |(), _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                flags.next.set(true);
            },
        ));
        ctx.create_child_with("next", Box::new(AutoplayButtonPanel { button: btn_next }));

        self.children_created = true;
    }
}

impl PanelBehavior for PrevNextPanel {
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.create_children(ctx);
        }
        let cc = ctx.GetCanvasColor();
        ctx.set_all_children_canvas_color(cc);
        self.layout.do_layout_skip(ctx, None, None);
    }
}

// ── emAutoplayControlPanel ──────────────────────────────────────────────────

/// Control panel for autoplay settings.
///
/// Port of C++ `emAutoplayControlPanel` (extends `emPackGroup`).
/// C++ widget tree:
///   emPackGroup (Caption="Autoplay")
///     ├─ child 0: AutoplayButton (emCheckButton) — weight 1.0, tallness 0.7
///     ├─ child 1: lPrevNext (emLinearLayout) — weight 0.64, tallness 0.4
///     │   ├─ BtPrev: emButton "Previous"
///     │   └─ BtNext: emButton "Next"
///     ├─ child 2: BtContinueLast (emButton) — weight 0.17, tallness 0.7
///     └─ child 3: lSettings (emPackGroup "Autoplay Settings") — weight 0.28, tallness 0.4
///         ├─ SfDuration: emScalarField — weight 1.0, tallness 0.15
///         ├─ CbRecursive: emCheckBox — weight 0.75, tallness 0.15
///         └─ CbLoop: emCheckBox — weight 0.75, tallness 0.15
pub struct emAutoplayControlPanel {
    layout: emPackLayout,
    border: emBorder,
    look: Rc<emLook>,
    children_created: bool,
    flags: Rc<AutoplayFlags>,
}

impl emAutoplayControlPanel {
    pub fn new(look: Rc<emLook>, flags: Rc<AutoplayFlags>) -> Self {
        Self {
            layout: emPackLayout::new(),
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Autoplay"),
            look,
            children_created: false,
            flags,
        }
    }

    /// Access the shared flags (for parent to poll).
    pub fn flags(&self) -> &Rc<AutoplayFlags> {
        &self.flags
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::clone(&self.look);
        let flags = Rc::clone(&self.flags);

        // ── child 0: BtAutoplay (emCheckButton) ──
        // C++ SetChildWeight(0, 1.0), SetPrefChildTallness(0, 0.7)
        let toggle_flags = Rc::clone(&flags);
        let mut btn_autoplay = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckButton::new(&mut sched, "Autoplay", Rc::clone(&look))
        };
        btn_autoplay.SetDescription(
            "Start or stop autoplay.\n\n\
             The autoplay function shows or plays things one after the other. This\n\
             is useful as a slideshow of picture files or for playing back multiple\n\
             audio or video files. Autoplay always starts at the focused panel (i.e.\n\
             the thing you have zoomed in) and follows the visual order.\n\n\
             Hotkey: Ctrl+F12",
        );
        btn_autoplay.on_check = Some(Box::new(
            move |checked, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                toggle_flags.toggle.set(Some(checked));
            },
        ));
        let autoplay_id = ctx.create_child_with(
            "autoplay",
            Box::new(AutoplayCheckButtonPanel {
                check_button: btn_autoplay,
                progress: Rc::clone(&flags.progress),
            }),
        );
        self.layout.set_child_constraint(
            autoplay_id,
            ChildConstraint {
                weight: 1.0,
                preferred_tallness: 0.7,
                ..Default::default()
            },
        );

        // ── child 1: lPrevNext (emLinearLayout) ──
        // C++ SetChildWeight(1, 0.64), SetPrefChildTallness(1, 0.4)
        let prev_next = Box::new(PrevNextPanel::new(Rc::clone(&look), Rc::clone(&flags)));
        let prev_next_id = ctx.create_child_with("prev_next", prev_next);
        self.layout.set_child_constraint(
            prev_next_id,
            ChildConstraint {
                weight: 0.64,
                preferred_tallness: 0.4,
                ..Default::default()
            },
        );

        // ── child 2: BtContinueLast (emButton) ──
        // C++ SetChildWeight(2, 0.17), SetPrefChildTallness(2, 0.7)
        let cont_flags = Rc::clone(&flags);
        let mut btn_cont = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Continue Last Autoplay", Rc::clone(&look))
        };
        btn_cont.SetDescription(
            "Start autoplay where it has stopped for the last time.\n\n\
             Hotkey: Shift+Ctrl+F12",
        );
        btn_cont.on_click = Some(Box::new(
            move |(), _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                cont_flags.continue_last.set(true);
            },
        ));
        let cont_id =
            ctx.create_child_with("cont", Box::new(AutoplayButtonPanel { button: btn_cont }));
        self.layout.set_child_constraint(
            cont_id,
            ChildConstraint {
                weight: 0.17,
                preferred_tallness: 0.7,
                ..Default::default()
            },
        );

        // ── child 3: lSettings (emPackGroup "Autoplay Settings") ──
        // C++ SetChildWeight(3, 0.28), SetPrefChildTallness(3, 0.4)
        let settings = Box::new(SettingsPanel::new(Rc::clone(&look), Rc::clone(&flags)));
        let settings_id = ctx.create_child_with("settings", settings);
        self.layout.set_child_constraint(
            settings_id,
            ChildConstraint {
                weight: 0.28,
                preferred_tallness: 0.4,
                ..Default::default()
            },
        );

        self.children_created = true;
    }
}

impl PanelBehavior for emAutoplayControlPanel {
    fn get_title(&self) -> Option<String> {
        Some("Autoplay".to_string())
    }

    fn IsOpaque(&self) -> bool {
        true
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border.paint_border(
            painter,
            w,
            h,
            &self.look,
            state.is_focused(),
            state.enabled,
            pixel_scale,
        );
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.create_children(ctx);
        }
        let aux_id = emcore::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn auto_expand(&self) -> bool {
        true
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_panel_new() {
        let look = Rc::new(emLook::default());
        let flags = Rc::new(AutoplayFlags::default());
        let panel = emAutoplayControlPanel::new(look, flags);
        assert_eq!(panel.get_title(), Some("Autoplay".to_string()));
    }

    #[test]
    fn test_autoplay_flags_default() {
        let flags = AutoplayFlags::default();
        assert!(flags.toggle.get().is_none());
        assert!(!flags.prev.get());
        assert!(!flags.next.get());
        assert!(!flags.continue_last.get());
        assert!(flags.duration_value.get().is_none());
        assert!(flags.recursive.get().is_none());
        assert!(flags.loop_toggle.get().is_none());
    }

    #[test]
    fn test_autoplay_flags_roundtrip() {
        let flags = Rc::new(AutoplayFlags::default());
        flags.toggle.set(Some(true));
        assert_eq!(flags.toggle.take(), Some(true));
        assert!(flags.toggle.get().is_none());

        flags.prev.set(true);
        assert!(flags.prev.take());
        assert!(!flags.prev.get());
    }

    #[test]
    fn test_duration_value_to_ms() {
        // Value 0 → first table entry
        assert_eq!(DurationValueToMS(0), 500);
        // Value 900 → last table entry
        assert_eq!(DurationValueToMS(900), 120000);
        // Value 100 → second entry (index 1)
        assert_eq!(DurationValueToMS(100), 1000);
        // Value 450 → midpoint of table
        assert_eq!(DurationValueToMS(450), 7500);
    }

    #[test]
    fn test_duration_ms_to_value() {
        // Inverse of known values
        assert_eq!(DurationMSToValue(500), 0);
        assert_eq!(DurationMSToValue(120000), 900);
        assert_eq!(DurationMSToValue(1000), 100);
    }

    #[test]
    fn test_duration_roundtrip() {
        for v in (0..=900).step_by(100) {
            let ms = DurationValueToMS(v);
            let v2 = DurationMSToValue(ms);
            assert_eq!(
                DurationValueToMS(v2),
                ms,
                "round-trip failed for value {v}: ms={ms}, v2={v2}"
            );
        }
    }

    #[test]
    fn test_duration_text_of_value() {
        // 0 → 500ms → 0.5 seconds
        assert_eq!(DurationTextOfValue(0, 0), "0.5");
        // 100 → 1000ms → 1 second
        assert_eq!(DurationTextOfValue(100, 0), "1");
        // 900 → 120000ms → 120 seconds
        assert_eq!(DurationTextOfValue(900, 0), "120");
    }
}
