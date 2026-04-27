// Port of C++ emAutoplayControlPanel (emAutoplay.h:333-398).
//
// R-A migration (B-003-no-wire-autoplay, 2026-04-27): AutoplayFlags dropped;
// emAutoplayControlPanel now holds Rc<RefCell<emAutoplayViewModel>> and
// implements Cycle per D-006-subscribe-shape. Widget SignalIds are captured at
// child-creation time and used for IsSignaled checks in Cycle.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emBorder::{InnerBorderType, OuterBorderType, emBorder};
use emcore::emButton::emButton;
use emcore::emCheckBox::emCheckBox;
use emcore::emCheckButton::emCheckButton;
use emcore::emColor::emColor;
use emcore::emCursor::emCursor;
use emcore::emEngineCtx::{EngineCtx, PanelCtx};
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLinearLayout::emLinearLayout;
use emcore::emLook::emLook;
use emcore::emPackLayout::emPackLayout;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emPanelTree::PanelId;
use emcore::emScalarField::emScalarField;
use emcore::emSignal::SignalId;
use emcore::emTiling::ChildConstraint;

use crate::emAutoplay::emAutoplayViewModel;

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

// ── Widget signal ID bundle ───────────────────────────────────────────────────

/// SignalId snapshots captured at child-creation time. Used in Cycle for
/// `IsSignaled` checks mirroring C++ `emAutoplayControlPanel::Cycle` fan-out.
struct WidgetSignalIds {
    bt_autoplay_check: SignalId,      // emCheckButton::check_signal
    bt_prev_click: SignalId,          // emButton::click_signal
    bt_next_click: SignalId,          // emButton::click_signal
    bt_continue_last_click: SignalId, // emButton::click_signal
    sf_duration_value: SignalId,      // emScalarField::value_signal
    cb_recursive_check: SignalId,     // emCheckBox::check_signal
    cb_loop_check: SignalId,          // emCheckBox::check_signal
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
    /// Cross-Cycle reference to ViewModel for reading ItemProgress in Paint;
    /// per CLAUDE.md §Ownership (a) — cross-Cycle reference held by panel.
    model: Rc<RefCell<emAutoplayViewModel>>,
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
        let progress = self.model.borrow().GetItemProgress();
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
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.scalar_field
            .Paint(painter, canvas_color, w, h, state.enabled, pixel_scale);
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
    // Widget signal IDs captured at create_children time.
    sf_duration_signal: Option<SignalId>,
    cb_recursive_signal: Option<SignalId>,
    cb_loop_signal: Option<SignalId>,
    // Child panel IDs for reading widget state in Cycle.
    sf_duration_id: Option<PanelId>,
    cb_recursive_id: Option<PanelId>,
    cb_loop_id: Option<PanelId>,
}

impl SettingsPanel {
    fn new(look: Rc<emLook>, _model: Rc<RefCell<emAutoplayViewModel>>) -> Self {
        Self {
            layout: emPackLayout::new(),
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Autoplay Settings"),
            look,
            children_created: false,
            sf_duration_signal: None,
            cb_recursive_signal: None,
            cb_loop_signal: None,
            sf_duration_id: None,
            cb_recursive_id: None,
            cb_loop_id: None,
        }
    }

    /// Returns (sf_duration_signal, cb_recursive_signal, cb_loop_signal) once created.
    fn widget_signal_ids(&self) -> Option<(SignalId, SignalId, SignalId)> {
        match (
            self.sf_duration_signal,
            self.cb_recursive_signal,
            self.cb_loop_signal,
        ) {
            (Some(sf), Some(cbr), Some(cbl)) => Some((sf, cbr, cbl)),
            _ => None,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::clone(&self.look);

        // ── SfDuration ──
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
        // No on_value callback — Cycle reads via IsSignaled(sf_duration_value).
        self.sf_duration_signal = Some(sf.value_signal);
        let sf_id = ctx.create_child_with(
            "duration",
            Box::new(AutoplayScalarFieldPanel { scalar_field: sf }),
        );
        self.sf_duration_id = Some(sf_id);
        self.layout.set_child_constraint(
            sf_id,
            ChildConstraint {
                weight: 1.0,
                preferred_tallness: 0.15,
                ..Default::default()
            },
        );

        // ── CbRecursive ──
        let mut cb_recursive = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(&mut sched, "Recursive", Rc::clone(&look))
        };
        cb_recursive.SetDescription("Whether autoplay shall play subdirectories recursively.");
        // No on_check — Cycle reads via IsSignaled.
        self.cb_recursive_signal = Some(cb_recursive.check_signal);
        let recursive_id = ctx.create_child_with(
            "recursive",
            Box::new(AutoplayCheckBoxPanel {
                check_box: cb_recursive,
            }),
        );
        self.cb_recursive_id = Some(recursive_id);
        self.layout.set_child_constraint(
            recursive_id,
            ChildConstraint {
                weight: 0.75,
                preferred_tallness: 0.15,
                ..Default::default()
            },
        );

        // ── CbLoop ──
        let mut cb_loop = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(&mut sched, "Loop", Rc::clone(&look))
        };
        cb_loop.SetDescription(
            "Whether autoplay shall start from the beginning\n\
             after reaching the end.",
        );
        // No on_check — Cycle reads via IsSignaled.
        self.cb_loop_signal = Some(cb_loop.check_signal);
        let loop_id = ctx.create_child_with(
            "loop",
            Box::new(AutoplayCheckBoxPanel { check_box: cb_loop }),
        );
        self.cb_loop_id = Some(loop_id);
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
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border.paint_border(
            painter,
            canvas_color,
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
    // Widget signal IDs captured at create_children time.
    bt_prev_signal: Option<SignalId>,
    bt_next_signal: Option<SignalId>,
}

impl PrevNextPanel {
    fn new(look: Rc<emLook>, _model: Rc<RefCell<emAutoplayViewModel>>) -> Self {
        // C++ SetOrientationThresholdTallness(0.7)
        Self {
            layout: emLinearLayout::adaptive(0.7),
            children_created: false,
            look,
            bt_prev_signal: None,
            bt_next_signal: None,
        }
    }

    /// Returns (bt_prev_click, bt_next_click) signals once created.
    fn widget_signal_ids(&self) -> Option<(SignalId, SignalId)> {
        match (self.bt_prev_signal, self.bt_next_signal) {
            (Some(bp), Some(bn)) => Some((bp, bn)),
            _ => None,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::clone(&self.look);

        // ── BtPrev ──
        let mut btn_prev = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Previous", Rc::clone(&look))
        };
        btn_prev.SetDescription(
            "Skip to previous autoplay item. This also works\n\
             when autoplay is off, for a manual show.\n\n\
             Hotkey: Shift+F12, backward button of the mouse",
        );
        // No on_click — Cycle reads via IsSignaled.
        self.bt_prev_signal = Some(btn_prev.click_signal);
        ctx.create_child_with("prev", Box::new(AutoplayButtonPanel { button: btn_prev }));

        // ── BtNext ──
        let mut btn_next = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Next", Rc::clone(&look))
        };
        btn_next.SetDescription(
            "Skip to next autoplay item. This also works\n\
             when autoplay is off, for a manual show.\n\n\
             Hotkeys: F12, forward button of the mouse",
        );
        // No on_click — Cycle reads via IsSignaled.
        self.bt_next_signal = Some(btn_next.click_signal);
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
    /// Cross-Cycle reference to ViewModel; per CLAUDE.md §Ownership (a) —
    /// cross-Cycle reference held by panel Cycle body.
    model: Rc<RefCell<emAutoplayViewModel>>,
    /// First-Cycle init flag for D-006-subscribe-shape: subscribe to model signals.
    subscribed_init: bool,
    /// Second-stage flag: subscribe to widget signals after AutoExpand creates children.
    subscribed_widgets: bool,
    /// Widget SignalId snapshots captured at create_children time, assembled once
    /// all sub-panels have completed their first LayoutChildren.
    widget_signals: Option<WidgetSignalIds>,
    // Top-level signal IDs captured at create_children time.
    bt_autoplay_check_signal: Option<SignalId>,
    bt_continue_last_click_signal: Option<SignalId>,
    // Child panel IDs for reading widget state and collecting sub-panel signals.
    autoplay_panel_id: Option<PanelId>,
    prev_next_panel_id: Option<PanelId>,
    settings_panel_id: Option<PanelId>,
}

impl emAutoplayControlPanel {
    pub fn new(look: Rc<emLook>, model: Rc<RefCell<emAutoplayViewModel>>) -> Self {
        Self {
            layout: emPackLayout::new(),
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Autoplay"),
            look,
            children_created: false,
            model,
            subscribed_init: false,
            subscribed_widgets: false,
            widget_signals: None,
            bt_autoplay_check_signal: None,
            bt_continue_last_click_signal: None,
            autoplay_panel_id: None,
            prev_next_panel_id: None,
            settings_panel_id: None,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::clone(&self.look);

        // ── child 0: BtAutoplay (emCheckButton) ──
        // C++ SetChildWeight(0, 1.0), SetPrefChildTallness(0, 0.7)
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
        // No on_check — Cycle reads via IsSignaled(bt_autoplay_check).
        self.bt_autoplay_check_signal = Some(btn_autoplay.check_signal);
        let autoplay_id = ctx.create_child_with(
            "autoplay",
            Box::new(AutoplayCheckButtonPanel {
                check_button: btn_autoplay,
                model: Rc::clone(&self.model),
            }),
        );
        self.autoplay_panel_id = Some(autoplay_id);
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
        let prev_next = Box::new(PrevNextPanel::new(Rc::clone(&look), Rc::clone(&self.model)));
        let prev_next_id = ctx.create_child_with("prev_next", prev_next);
        self.layout.set_child_constraint(
            prev_next_id,
            ChildConstraint {
                weight: 0.64,
                preferred_tallness: 0.4,
                ..Default::default()
            },
        );
        self.prev_next_panel_id = Some(prev_next_id);

        // ── child 2: BtContinueLast (emButton) ──
        // C++ SetChildWeight(2, 0.17), SetPrefChildTallness(2, 0.7)
        let mut btn_cont = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Continue Last Autoplay", Rc::clone(&look))
        };
        btn_cont.SetDescription(
            "Start autoplay where it has stopped for the last time.\n\n\
             Hotkey: Shift+Ctrl+F12",
        );
        // No on_click — Cycle reads via IsSignaled.
        self.bt_continue_last_click_signal = Some(btn_cont.click_signal);
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
        let settings = Box::new(SettingsPanel::new(Rc::clone(&look), Rc::clone(&self.model)));
        let settings_id = ctx.create_child_with("settings", settings);
        self.layout.set_child_constraint(
            settings_id,
            ChildConstraint {
                weight: 0.28,
                preferred_tallness: 0.4,
                ..Default::default()
            },
        );
        self.settings_panel_id = Some(settings_id);

        self.children_created = true;
    }

    /// Try to assemble the full WidgetSignalIds bundle once sub-panels have created
    /// their children (PrevNextPanel and SettingsPanel must both have completed their
    /// first LayoutChildren before their signals are available).
    fn try_collect_widget_signals(&self, ctx: &mut PanelCtx) -> Option<WidgetSignalIds> {
        let bt_check = self.bt_autoplay_check_signal?;
        let bt_cont = self.bt_continue_last_click_signal?;

        let pn_id = self.prev_next_panel_id?;
        let (bt_prev, bt_next) = ctx
            .tree
            .with_behavior_as::<PrevNextPanel, _>(pn_id, |pn| pn.widget_signal_ids())
            .flatten()?;

        let s_id = self.settings_panel_id?;
        let (sf_dur, cb_rec, cb_lp) = ctx
            .tree
            .with_behavior_as::<SettingsPanel, _>(s_id, |sp| sp.widget_signal_ids())
            .flatten()?;

        Some(WidgetSignalIds {
            bt_autoplay_check: bt_check,
            bt_prev_click: bt_prev,
            bt_next_click: bt_next,
            bt_continue_last_click: bt_cont,
            sf_duration_value: sf_dur,
            cb_recursive_check: cb_rec,
            cb_loop_check: cb_lp,
        })
    }

    /// Mirror C++ `emAutoplayControlPanel::UpdateControls` (emAutoplay.cpp:~1394).
    /// Reads Model state and pushes back into widgets.
    /// Stub reaction body: B-003 owns the wire; full UpdateControls port is staged
    /// as a follow-up per design doc §"update_controls / update_progress" note.
    fn update_controls(&mut self) {
        let model = self.model.borrow();
        // Stub: log observable side effect so tests can verify the branch ran.
        log::debug!(
            "emAutoplayControlPanel::update_controls: autoplaying={} duration_ms={} recursive={} loop={}",
            model.IsAutoplaying(),
            model.GetDurationMS(),
            model.IsRecursive(),
            model.IsLoop(),
        );
        // TODO(B-003-follow-up): push model state back into widgets:
        //   bt_autoplay.SetChecked(model.IsAutoplaying())
        //   sf_duration.SetValue(DurationMSToValue(model.GetDurationMS()))
        //   cb_recursive.SetChecked(model.IsRecursive())
        //   cb_loop.SetChecked(model.IsLoop())
    }

    /// Mirror C++ `emAutoplayControlPanel::UpdateProgress` (emAutoplay.cpp:~1467).
    /// Stub reaction body per design doc §"update_controls / update_progress" note.
    fn update_progress(&mut self) {
        let progress = self.model.borrow().GetItemProgress();
        log::debug!("emAutoplayControlPanel::update_progress: progress={progress:.3}");
        // TODO(B-003-follow-up): push progress into AutoplayCheckButtonPanel display.
        // (AutoplayCheckButtonPanel already reads model.GetItemProgress() directly in Paint.)
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
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            state.is_focused(),
            state.enabled,
            pixel_scale,
        );
    }

    /// Port of C++ `emAutoplayControlPanel::Cycle` (emAutoplay.cpp:1183-1222).
    /// D-006-subscribe-shape: first-Cycle init + IsSignaled fan-out.
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        let eid = ectx.id();

        // ── Phase 1: subscribe to model signals on first Cycle (D-006) ──
        // Mirrors C++ emAutoplayControlPanel constructor (emAutoplay.cpp:1171-1172):
        //   AddWakeUpSignal(Model->GetChangeSignal())
        //   AddWakeUpSignal(Model->GetProgressSignal())
        if !self.subscribed_init {
            let change_sig = self.model.borrow().GetChangeSignal(ectx);
            let progress_sig = self.model.borrow().GetProgressSignal(ectx);
            ectx.connect(change_sig, eid);
            ectx.connect(progress_sig, eid);
            self.subscribed_init = true;
        }

        // ── Phase 2: subscribe to widget signals after AutoExpand (D-006) ──
        // Widget children exist only after first LayoutChildren (AutoExpand).
        // PrevNextPanel and SettingsPanel populate their signal fields in
        // their own first LayoutChildren. Try to assemble the bundle each Cycle
        // until all 7 signals are available.
        if !self.subscribed_widgets
            && let Some(sigs) = self.try_collect_widget_signals(ctx)
        {
            ectx.connect(sigs.bt_autoplay_check, eid);
            ectx.connect(sigs.bt_prev_click, eid);
            ectx.connect(sigs.bt_next_click, eid);
            ectx.connect(sigs.bt_continue_last_click, eid);
            ectx.connect(sigs.sf_duration_value, eid);
            ectx.connect(sigs.cb_recursive_check, eid);
            ectx.connect(sigs.cb_loop_check, eid);
            self.widget_signals = Some(sigs);
            self.subscribed_widgets = true;
        }

        // ── IsSignaled fan-out — mirrors C++ Cycle source order ──
        // emAutoplay.cpp:1184-1218
        if let Some(ref sigs) = self.widget_signals {
            if ectx.IsSignaled(sigs.bt_autoplay_check) {
                // Read IsChecked from the AutoplayCheckButtonPanel child.
                let checked = self
                    .autoplay_panel_id
                    .and_then(|id| {
                        ctx.tree
                            .with_behavior_as::<AutoplayCheckButtonPanel, _>(id, |p| {
                                p.check_button.IsChecked()
                            })
                    })
                    .unwrap_or(false);
                self.model.borrow_mut().SetAutoplaying(ectx, checked);
            }
            if ectx.IsSignaled(sigs.bt_prev_click) {
                self.model.borrow_mut().SkipToPreviousItem();
            }
            if ectx.IsSignaled(sigs.bt_next_click) {
                self.model.borrow_mut().SkipToNextItem();
            }
            if ectx.IsSignaled(sigs.bt_continue_last_click) {
                self.model.borrow_mut().ContinueLastAutoplay(ectx);
            }
            if ectx.IsSignaled(sigs.sf_duration_value) {
                let val_opt = self.settings_panel_id.and_then(|sid| {
                    ctx.tree
                        .with_behavior_as::<SettingsPanel, _>(sid, |sp| sp.sf_duration_id)
                        .flatten()
                });
                if let Some(sfid) = val_opt {
                    let val = ctx
                        .tree
                        .with_behavior_as::<AutoplayScalarFieldPanel, _>(sfid, |p| {
                            p.scalar_field.GetValue() as i64
                        });
                    if let Some(val) = val {
                        let ms = DurationValueToMS(val);
                        self.model.borrow_mut().SetDurationMS(ectx, ms);
                    }
                }
            }
            if ectx.IsSignaled(sigs.cb_recursive_check) {
                let id_opt = self.settings_panel_id.and_then(|sid| {
                    ctx.tree
                        .with_behavior_as::<SettingsPanel, _>(sid, |sp| sp.cb_recursive_id)
                        .flatten()
                });
                if let Some(cbid) = id_opt {
                    let checked = ctx
                        .tree
                        .with_behavior_as::<AutoplayCheckBoxPanel, _>(cbid, |p| {
                            p.check_box.IsChecked()
                        });
                    if let Some(checked) = checked {
                        self.model.borrow_mut().SetRecursive(ectx, checked);
                    }
                }
            }
            if ectx.IsSignaled(sigs.cb_loop_check) {
                let id_opt = self.settings_panel_id.and_then(|sid| {
                    ctx.tree
                        .with_behavior_as::<SettingsPanel, _>(sid, |sp| sp.cb_loop_id)
                        .flatten()
                });
                if let Some(cbid) = id_opt {
                    let checked = ctx
                        .tree
                        .with_behavior_as::<AutoplayCheckBoxPanel, _>(cbid, |p| {
                            p.check_box.IsChecked()
                        });
                    if let Some(checked) = checked {
                        self.model.borrow_mut().SetLoop(ectx, checked);
                    }
                }
            }
        }

        // ── React to model signals ──
        // emAutoplay.cpp:1219-1222
        let change_sig = self.model.borrow().change_signal.get();
        let progress_sig = self.model.borrow().progress_signal.get();
        if ectx.IsSignaled(change_sig) {
            self.update_controls();
        }
        if ectx.IsSignaled(progress_sig) {
            self.update_progress();
        }

        false
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
        let model = Rc::new(RefCell::new(emAutoplayViewModel::new()));
        let panel = emAutoplayControlPanel::new(look, model);
        assert_eq!(panel.get_title(), Some("Autoplay".to_string()));
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
