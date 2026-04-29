use std::cell::{Cell, RefCell};
use std::rc::Rc;

use slotmap::Key as _;

use crate::emColor::emColor;
use crate::emCoreConfig::emCoreConfig;
use crate::emCursor::emCursor;
use crate::emEngineCtx::{EngineCtx, PanelCtx};
use crate::emLinearLayout::emLinearLayout;
use crate::emPainter::emPainter;
use crate::emPanel::{PanelBehavior, PanelState};
use crate::emPanelTree::PanelId;
use crate::emRasterLayout::emRasterLayout;
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emRecNodeConfigModel::emRecNodeConfigModel;
use crate::emSignal::SignalId;
use crate::emTiling::{AlignmentH, AlignmentV, ChildConstraint, Spacing};

use super::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use super::emColorFieldFieldPanel::{ButtonPanel, CheckBoxPanel, LabelPanel};
use crate::emButton::emButton;
use crate::emCheckBox::emCheckBox;
use crate::emLabel::emLabel;
use crate::emLook::emLook;
use crate::emScalarField::emScalarField;
use crate::emTunnel::emTunnel;

// ---------------------------------------------------------------------------
// Pure conversion functions (C++ emCoreConfigPanel.cpp factor field logic)
// ---------------------------------------------------------------------------

/// emScalarField value (-200..+200) to config domain value.
fn factor_val_to_cfg(value: f64, cfg_min: f64, cfg_max: f64) -> f64 {
    let m = if value >= 0.0 { cfg_max } else { 1.0 / cfg_min };
    m.sqrt().powf(value / 100.0)
}

/// Config domain value to emScalarField value (-200..+200), rounded.
fn factor_cfg_to_val(d: f64, cfg_min: f64, cfg_max: f64) -> f64 {
    let m = if d >= 1.0 { cfg_max } else { 1.0 / cfg_min };
    let v = d.ln() / m.sqrt().ln() * 100.0;
    if v >= 0.0 {
        (v + 0.5).floor()
    } else {
        (v - 0.5).ceil()
    }
}

/// Text for factor fields (C++ lines 118-141).
fn factor_text_of_value(
    value: i64,
    mark_interval: u64,
    minimum_means_disabled: bool,
    cfg_min: f64,
    cfg_max: f64,
) -> String {
    if mark_interval >= 100 {
        match value {
            -200 => {
                if minimum_means_disabled {
                    "Disabled"
                } else {
                    "Minimal"
                }
            }
            -100 => "Reduced",
            0 => "Default",
            100 => "Increased",
            200 => "Extreme",
            _ => "?",
        }
        .to_string()
    } else if mark_interval >= 10 {
        format!("{:.2}", factor_val_to_cfg(value as f64, cfg_min, cfg_max))
    } else {
        format!("{:.3}", factor_val_to_cfg(value as f64, cfg_min, cfg_max))
    }
}

/// Memory MB to emScalarField value (log2 space).
fn mem_cfg_to_val(mb: i32) -> f64 {
    (mb as f64).ln() / 2.0_f64.ln() * 100.0
}

/// emScalarField value to memory MB (log2 space).
fn mem_val_to_cfg(val: f64) -> i32 {
    (2.0_f64.powf(val / 100.0) + 0.5) as i32
}

/// Text for memory field.
fn mem_text_of_value(value: i64, mark_interval: u64) -> String {
    let d = 2.0_f64.powf(value as f64 / 100.0);
    if mark_interval < 100 && d < 64.0 {
        format!("{:.1}", d)
    } else {
        format!("{}", (d + 0.5) as i32)
    }
}

/// Text for downscale quality field.
fn downscale_text(value: i64, _mark_interval: u64) -> String {
    if value < 1 {
        "Nearest\nPixel".to_string()
    } else {
        format!("{}x{}", value, value)
    }
}

/// Text for upscale quality field.
fn upscale_text(value: i64, _mark_interval: u64) -> String {
    match value {
        0 => "Nearest\nPixel",
        1 => "Area Sampling\n(Antialiased\nNearest Pixel)",
        2 => "Bilinear",
        3 => "Bicubic",
        4 => "Lanczos",
        5 => "Adaptive",
        _ => "?",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Factory helper: build a FactorFieldPanel with factor-field config.
// Replaces the previous ScalarFieldPanel { scalar_field } literal construction
// for config-bound scalar fields in emCoreConfigPanel.
// ---------------------------------------------------------------------------

/// Config-bound scalar field panel. Equivalent to C++ `FactorField`:
/// `emScalarField + emRecListener`. Subscribes to `config_sig` in its first
/// Cycle and calls `set_value_silent` on signal — display-only update that
/// does not trigger the `on_value` feedback loop.
struct FactorFieldPanel {
    scalar_field: emScalarField,
    /// Value signal of the specific `emDoubleRec`/`emIntRec` this panel mirrors.
    /// `SignalId::null()` = no self-update wiring.
    config_sig: SignalId,
    /// Closure returning the current config value in slider units.
    get_config_val: Option<Box<dyn Fn() -> f64>>,
    subscribed_to_config: bool,
}

impl PanelBehavior for FactorFieldPanel {
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

    /// D-006 first-Cycle init: subscribe to config_sig, react by calling
    /// set_value_silent (no on_value feedback, no value_signal fire).
    ///
    /// DIVERGED: (language-forced) 1-cycle delay vs C++ emRecListener::OnRecChanged
    /// which fires synchronously inside emRec::Changed().
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, _ctx: &mut PanelCtx) -> bool {
        if !self.subscribed_to_config && !self.config_sig.is_null() {
            let eid = ectx.id();
            ectx.connect(self.config_sig, eid);
            self.subscribed_to_config = true;
        }
        if !self.config_sig.is_null() && ectx.IsSignaled(self.config_sig) {
            if let Some(ref get_val) = self.get_config_val {
                self.scalar_field.set_value_silent(get_val());
            }
        }
        false
    }

    fn Input(
        &mut self,
        event: &crate::emInput::emInputEvent,
        state: &PanelState,
        input_state: &crate::emInputState::emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.scalar_field.Input(event, state, input_state, ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.scalar_field.GetCursor()
    }
}

#[allow(clippy::too_many_arguments)]
fn make_factor_field(
    ctx: &mut PanelCtx<'_>,
    caption: &str,
    description: &str,
    look: Rc<emLook>,
    cfg_min: f64,
    cfg_max: f64,
    cfg_value: f64,
    minimum_means_disabled: bool,
    field_sig: SignalId,
    get_val: Box<dyn Fn() -> f64>,
) -> FactorFieldPanel {
    let mut sched = ctx
        .as_sched_ctx()
        .expect("make_factor_field requires scheduler-reach PanelCtx");
    let mut sf = emScalarField::new(&mut sched, -200.0, 200.0, look);
    sf.SetCaption(caption);
    sf.border_mut().description = description.to_string();
    sf.set_initial_value(factor_cfg_to_val(cfg_value, cfg_min, cfg_max));
    sf.SetScaleMarkIntervals(&[100, 10]);
    sf.SetTextBoxTallness(0.3);
    sf.border_mut().SetBorderScaling(1.5);
    let (min, max, dis) = (cfg_min, cfg_max, minimum_means_disabled);
    sf.SetTextOfValueFunc(Box::new(move |v, mi| {
        factor_text_of_value(v, mi, dis, min, max)
    }));
    FactorFieldPanel {
        scalar_field: sf,
        config_sig: field_sig,
        get_config_val: Some(get_val),
        subscribed_to_config: false,
    }
}

// ---------------------------------------------------------------------------
// Leaf Groups
// ---------------------------------------------------------------------------

/// Keyboard control group — 2 factor fields.
struct KBGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: emBorder,
    layout: emRasterLayout,
}

impl KBGroup {
    fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Keyboard Control"),
            layout: emRasterLayout::new()
                .with_preferred_tallness(0.2)
                .with_spacing(Spacing {
                    margin_left: 0.05,
                    margin_top: 0.1,
                    margin_right: 0.05,
                    margin_bottom: 0.1,
                    ..Default::default()
                }),
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        let (zoom_sig, zoom_init, scroll_sig, scroll_init) = {
            let cfg = self.config.borrow();
            let c = cfg.GetRec();
            (
                c.KeyboardZoomSpeed.listened_signal(),
                *c.KeyboardZoomSpeed.GetValue(),
                c.KeyboardScrollSpeed.listened_signal(),
                *c.KeyboardScrollSpeed.GetValue(),
            )
        };

        let zoom_config = Rc::clone(&self.config);
        let mut zoom = make_factor_field(
            ctx,
            "Keyboard zoom speed",
            "Speed of zooming by keyboard",
            self.look.clone(),
            0.25,
            4.0,
            zoom_init,
            false,
            zoom_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *zoom_config.borrow().GetRec().KeyboardZoomSpeed.GetValue(),
                    0.25,
                    4.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        zoom.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.KeyboardZoomSpeed.SetValue(cfg_val, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("zoom", Box::new(zoom));

        let scroll_config = Rc::clone(&self.config);
        let mut scroll = make_factor_field(
            ctx,
            "Keyboard scroll speed",
            "Speed of scrolling by keyboard",
            self.look.clone(),
            0.25,
            4.0,
            scroll_init,
            false,
            scroll_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *scroll_config
                        .borrow()
                        .GetRec()
                        .KeyboardScrollSpeed
                        .GetValue(),
                    0.25,
                    4.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        scroll.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.KeyboardScrollSpeed.SetValue(cfg_val, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("scroll", Box::new(scroll));
    }
}

impl PanelBehavior for KBGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            state.enabled,
            true,
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100),
        );
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        let gen = self.generation.get();
        if gen != self.last_generation && ctx.child_count() > 0 {
            for id in ctx.children() {
                ctx.delete_child(id);
            }
            self.last_generation = gen;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let aux_id = crate::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

/// Miscellaneous mouse settings group — 3 checkboxes (stick / emu / pan).
///
/// Visibility: `pub(crate)` in production; gated up to `pub` under the
/// `test-support` feature so the row 299/300/301 integration tests in
/// `tests/rc_shim_b010.rs` can construct and drive it directly. Production
/// callers reach it only through `MouseGroup::create_children`.
#[cfg(any(test, feature = "test-support"))]
pub struct MouseMiscGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    stick_possible: bool,
    border: emBorder,
    layout: emRasterLayout,
    // B-010 rows 299/300/301: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    // Config aggregate subscribe for update_output after Reset.
    // Distinct from subscribed_init which gates per-checkbox wakeup subscriptions.
    subscribed_to_config: bool,
    config_sig: SignalId,
    stick_sig: SignalId,
    emu_sig: SignalId,
    pan_sig: SignalId,
    stick_id: Option<PanelId>,
    emu_id: Option<PanelId>,
    pan_id: Option<PanelId>,
}

#[cfg(not(any(test, feature = "test-support")))]
pub(crate) struct MouseMiscGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    stick_possible: bool,
    border: emBorder,
    layout: emRasterLayout,
    // B-010 rows 299/300/301: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    // Config aggregate subscribe for update_output after Reset.
    // Distinct from subscribed_init which gates per-checkbox wakeup subscriptions.
    subscribed_to_config: bool,
    config_sig: SignalId,
    stick_sig: SignalId,
    emu_sig: SignalId,
    pan_sig: SignalId,
    stick_id: Option<PanelId>,
    emu_id: Option<PanelId>,
    pan_id: Option<PanelId>,
}

impl MouseMiscGroup {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
        stick_possible: bool,
    ) -> Self {
        let gen = generation.get();
        let config_sig = config.borrow().GetChangeSignal();
        Self {
            config_sig,
            config,
            look,
            generation,
            last_generation: gen,
            stick_possible,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Miscellaneous mouse settings"),
            layout: emRasterLayout::new().with_preferred_tallness(0.04),
            subscribed_init: false,
            subscribed_to_config: false,
            stick_sig: SignalId::null(),
            emu_sig: SignalId::null(),
            pan_sig: SignalId::null(),
            stick_id: None,
            emu_id: None,
            pan_id: None,
        }
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
        stick_possible: bool,
    ) -> Self {
        let gen = generation.get();
        let config_sig = config.borrow().GetChangeSignal();
        Self {
            config_sig,
            config,
            look,
            generation,
            last_generation: gen,
            stick_possible,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Miscellaneous mouse settings"),
            layout: emRasterLayout::new().with_preferred_tallness(0.04),
            subscribed_init: false,
            subscribed_to_config: false,
            stick_sig: SignalId::null(),
            emu_sig: SignalId::null(),
            pan_sig: SignalId::null(),
            stick_id: None,
            emu_id: None,
            pan_id: None,
        }
    }

    /// Test-only accessors for the captured check_signals. Used by
    /// `tests/rc_shim_b010.rs` to fire each checkbox's signal directly without
    /// exposing the `pub(crate)` `CheckBoxPanel` adapter.
    #[cfg(any(test, feature = "test-support"))]
    pub fn stick_sig_for_test(&self) -> SignalId {
        self.stick_sig
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn emu_sig_for_test(&self) -> SignalId {
        self.emu_sig
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn pan_sig_for_test(&self) -> SignalId {
        self.pan_sig
    }

    /// Test-only helper: pre-stage a child checkbox's `IsChecked()` state via
    /// the typed downcast `with_behavior_as::<CheckBoxPanel, _>`. Bypasses
    /// signal firing — the test fires the captured signal explicitly via
    /// `sched.fire(stick_sig_for_test())` after staging.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_stick_checked_for_test(
        &self,
        tree: &mut crate::emPanelTree::PanelTree,
        checked: bool,
    ) {
        let id = self.stick_id.expect("stick_id set in create_children");
        tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
            p.check_box.set_checked_for_test(checked);
        });
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_emu_checked_for_test(
        &self,
        tree: &mut crate::emPanelTree::PanelTree,
        checked: bool,
    ) {
        let id = self.emu_id.expect("emu_id set in create_children");
        tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
            p.check_box.set_checked_for_test(checked);
        });
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_pan_checked_for_test(
        &self,
        tree: &mut crate::emPanelTree::PanelTree,
        checked: bool,
    ) {
        let id = self.pan_id.expect("pan_id set in create_children");
        tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
            p.check_box.set_checked_for_test(checked);
        });
    }

    /// D4: propagate current config values back to each checkbox without
    /// triggering the checkbox's own CheckSignal (uses `set_checked_silent`).
    /// Called when `config_sig` fires, so the UI reflects external config
    /// changes (e.g., another panel or process writing to the config file).
    fn update_output(&self, ctx: &mut PanelCtx) {
        let (stick_val, emu_val, pan_val) = {
            let cfg = self.config.borrow();
            let c = cfg.GetRec();
            (
                self.stick_possible && *c.StickMouseWhenNavigating.GetValue(),
                *c.EmulateMiddleButton.GetValue(),
                *c.PanFunction.GetValue(),
            )
        };
        if let Some(id) = self.stick_id {
            ctx.tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
                p.check_box.set_checked_silent(stick_val);
            });
        }
        if let Some(id) = self.emu_id {
            ctx.tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
                p.check_box.set_checked_silent(emu_val);
            });
        }
        if let Some(id) = self.pan_id {
            ctx.tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
                p.check_box.set_checked_silent(pan_val);
            });
        }
    }

    /// Visibility: `pub(crate)` in production; gated up to `pub` under the
    /// `test-support` feature so row 299/300/301 integration tests can drive
    /// child creation without invoking the full `LayoutChildren` path.
    #[cfg(any(test, feature = "test-support"))]
    pub fn create_children(&mut self, ctx: &mut PanelCtx) {
        self.create_children_impl(ctx);
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn create_children(&mut self, ctx: &mut PanelCtx) {
        self.create_children_impl(ctx);
    }

    fn create_children_impl(&mut self, ctx: &mut PanelCtx) {
        let (stick_init, emu_init, pan_init) = {
            let cfg = self.config.borrow();
            let c = cfg.GetRec();
            (
                *c.StickMouseWhenNavigating.GetValue(),
                *c.EmulateMiddleButton.GetValue(),
                *c.PanFunction.GetValue(),
            )
        };

        // C++ emCoreConfigPanel.cpp:295: StickBox->SetEnableSwitch(StickPossible)
        // Disabled when the screen cannot move the mouse pointer.
        let mut stick = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(
                &mut sched,
                "Stick mouse\nwhen navigating",
                self.look.clone(),
            )
        };
        stick.SetChecked(stick_init, ctx);
        // B-010 row 299: capture check_signal BEFORE the checkbox moves into
        // the child behavior (SignalId is Copy).
        self.stick_sig = stick.check_signal;
        let stick_id = ctx.create_child_with("stick", Box::new(CheckBoxPanel { check_box: stick }));
        self.stick_id = Some(stick_id);
        if !self.stick_possible {
            ctx.tree
                .SetEnableSwitch(stick_id, false, ctx.scheduler.as_deref_mut());
        }

        let mut emu = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(&mut sched, "Emulate\nmiddle button", self.look.clone())
        };
        emu.SetChecked(emu_init, ctx);
        // B-010 row 300: capture check_signal BEFORE child move.
        self.emu_sig = emu.check_signal;
        let emu_id = ctx.create_child_with("emu", Box::new(CheckBoxPanel { check_box: emu }));
        self.emu_id = Some(emu_id);

        let mut pan = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(&mut sched, "Pan\nfunction", self.look.clone())
        };
        pan.SetChecked(pan_init, ctx);
        // B-010 row 301: capture check_signal BEFORE child move.
        self.pan_sig = pan.check_signal;
        let pan_id = ctx.create_child_with("pan", Box::new(CheckBoxPanel { check_box: pan }));
        self.pan_id = Some(pan_id);
    }
}

impl PanelBehavior for MouseMiscGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            state.enabled,
            true,
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100),
        );
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        let gen = self.generation.get();
        if gen != self.last_generation && ctx.child_count() > 0 {
            for id in ctx.children() {
                ctx.delete_child(id);
            }
            self.last_generation = gen;
            // B-010: child PanelIds and signals are stale after rebuild;
            // re-subscribe on the next Cycle once children are recreated.
            self.subscribed_init = false;
            self.stick_id = None;
            self.emu_id = None;
            self.pan_id = None;
            self.stick_sig = SignalId::null();
            self.emu_sig = SignalId::null();
            self.pan_sig = SignalId::null();
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let aux_id = crate::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    /// B-010 rows 299/300/301: D-006 first-Cycle init + IsSignaled subscribe shape.
    ///
    /// Mirrors C++ `emCoreConfigPanel::MouseMiscGroup::Cycle`
    /// (emCoreConfigPanel.cpp:~245-275): subscribes to each checkbox's
    /// CheckSignal and on `IsSignaled(...)` propagates `IsChecked()` to the
    /// corresponding config field + Save (if changed). Branches in C++ source
    /// order: stick → emu → pan.
    fn Cycle(&mut self, ectx: &mut crate::emEngineCtx::EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        // Config aggregate subscribe — distinct from per-checkbox subscribed_init.
        if !self.subscribed_to_config {
            ectx.connect(self.config_sig, ectx.id());
            self.subscribed_to_config = true;
        }
        if ectx.IsSignaled(self.config_sig) {
            self.update_output(ctx);
        }

        if !self.subscribed_init
            && self.stick_id.is_some()
            && self.emu_id.is_some()
            && self.pan_id.is_some()
        {
            let eid = ectx.id();
            ectx.connect(self.stick_sig, eid);
            ectx.connect(self.emu_sig, eid);
            ectx.connect(self.pan_sig, eid);
            self.subscribed_init = true;
            ectx.wake_up(eid);
        }

        // C++ guards stick branch with `&& StickPossible` (emCoreConfigPanel.cpp:~252).
        if self.stick_possible && !self.stick_sig.is_null() && ectx.IsSignaled(self.stick_sig) {
            let checked = ctx
                .tree
                .with_behavior_as::<CheckBoxPanel, _>(
                    self.stick_id.expect("stick_id set in create_children"),
                    |p| p.check_box.IsChecked(),
                )
                .unwrap_or(false);
            let mut cm = self.config.borrow_mut();
            let mut sched = ctx.as_sched_ctx().expect("sched");
            cm.modify(
                |c, sc| {
                    if *c.StickMouseWhenNavigating.GetValue() != checked {
                        c.StickMouseWhenNavigating.SetValue(checked, sc);
                    }
                },
                &mut sched,
            );
            let _ = cm.TrySave(false);
        }

        if !self.emu_sig.is_null() && ectx.IsSignaled(self.emu_sig) {
            let checked = ctx
                .tree
                .with_behavior_as::<CheckBoxPanel, _>(
                    self.emu_id.expect("emu_id set in create_children"),
                    |p| p.check_box.IsChecked(),
                )
                .unwrap_or(false);
            let mut cm = self.config.borrow_mut();
            let mut sched = ctx.as_sched_ctx().expect("sched");
            cm.modify(
                |c, sc| {
                    if *c.EmulateMiddleButton.GetValue() != checked {
                        c.EmulateMiddleButton.SetValue(checked, sc);
                    }
                },
                &mut sched,
            );
            let _ = cm.TrySave(false);
        }

        if !self.pan_sig.is_null() && ectx.IsSignaled(self.pan_sig) {
            let checked = ctx
                .tree
                .with_behavior_as::<CheckBoxPanel, _>(
                    self.pan_id.expect("pan_id set in create_children"),
                    |p| p.check_box.IsChecked(),
                )
                .unwrap_or(false);
            let mut cm = self.config.borrow_mut();
            let mut sched = ctx.as_sched_ctx().expect("sched");
            cm.modify(
                |c, sc| {
                    if *c.PanFunction.GetValue() != checked {
                        c.PanFunction.SetValue(checked, sc);
                    }
                },
                &mut sched,
            );
            let _ = cm.TrySave(false);
        }

        false
    }
}

/// Kinetic effects group — 4 factor fields.
struct KineticGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: emBorder,
    layout: emRasterLayout,
}

impl KineticGroup {
    fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Kinetic Effects"),
            layout: emRasterLayout::new()
                .with_preferred_tallness(0.2)
                .with_spacing(Spacing {
                    margin_left: 0.05,
                    margin_top: 0.1,
                    margin_right: 0.05,
                    margin_bottom: 0.1,
                    ..Default::default()
                }),
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        let (
            kinetic_sig,
            kinetic_init,
            mag_radius_sig,
            mag_radius_init,
            mag_speed_sig,
            mag_speed_init,
            visit_sig,
            visit_init,
        ) = {
            let cfg = self.config.borrow();
            let c = cfg.GetRec();
            (
                c.KineticZoomingAndScrolling.listened_signal(),
                *c.KineticZoomingAndScrolling.GetValue(),
                c.MagnetismRadius.listened_signal(),
                *c.MagnetismRadius.GetValue(),
                c.MagnetismSpeed.listened_signal(),
                *c.MagnetismSpeed.GetValue(),
                c.VisitSpeed.listened_signal(),
                *c.VisitSpeed.GetValue(),
            )
        };

        // KineticZoomingAndScrolling
        let kinetic_config = Rc::clone(&self.config);
        let mut kinetic = make_factor_field(
            ctx,
            "Kinetic zooming and scrolling",
            "Whether and how much to have kinetic effects on zooming and scrolling",
            self.look.clone(),
            0.25,
            2.0,
            kinetic_init,
            true,
            kinetic_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *kinetic_config
                        .borrow()
                        .GetRec()
                        .KineticZoomingAndScrolling
                        .GetValue(),
                    0.25,
                    2.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        kinetic.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.25, 2.0);
                let mut cm = config.borrow_mut();
                cm.modify(
                    |c, sc| c.KineticZoomingAndScrolling.SetValue(cfg_val, sc),
                    sched,
                );
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("KineticZoomingAndScrolling", Box::new(kinetic));

        // MagnetismRadius
        let mag_radius_config = Rc::clone(&self.config);
        let mut mag_radius = make_factor_field(
            ctx,
            "Magnetism radius",
            "Maximum radius for magnetism to snap the focus to nearby panels",
            self.look.clone(),
            0.25,
            4.0,
            mag_radius_init,
            true,
            mag_radius_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *mag_radius_config
                        .borrow()
                        .GetRec()
                        .MagnetismRadius
                        .GetValue(),
                    0.25,
                    4.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        mag_radius.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.MagnetismRadius.SetValue(cfg_val, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("MagnetismRadius", Box::new(mag_radius));

        // MagnetismSpeed
        let mag_speed_config = Rc::clone(&self.config);
        let mut mag_speed = make_factor_field(
            ctx,
            "Magnetism speed",
            "Speed of the magnetism movement",
            self.look.clone(),
            0.25,
            4.0,
            mag_speed_init,
            false,
            mag_speed_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *mag_speed_config.borrow().GetRec().MagnetismSpeed.GetValue(),
                    0.25,
                    4.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        mag_speed.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.MagnetismSpeed.SetValue(cfg_val, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("MagnetismSpeed", Box::new(mag_speed));

        // VisitSpeed
        let visit_config = Rc::clone(&self.config);
        let mut visit = make_factor_field(
            ctx,
            "Visit speed",
            "Speed of the visit animation",
            self.look.clone(),
            0.1,
            10.0,
            visit_init,
            false,
            visit_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *visit_config.borrow().GetRec().VisitSpeed.GetValue(),
                    0.1,
                    10.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        visit.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.1, 10.0);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.VisitSpeed.SetValue(cfg_val, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("VisitSpeed", Box::new(visit));
    }
}

impl PanelBehavior for KineticGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            state.enabled,
            true,
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100),
        );
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        let gen = self.generation.get();
        if gen != self.last_generation && ctx.child_count() > 0 {
            for id in ctx.children() {
                ctx.delete_child(id);
            }
            self.last_generation = gen;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let aux_id = crate::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

/// Max megabytes per view group — label + scalar field.
struct MaxMemGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: emBorder,
    layout: emLinearLayout,
}

impl MaxMemGroup {
    fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Max Megabytes Per View"),
            layout: emLinearLayout::vertical(),
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let label_text =
            "Here you can set the maximum allowed memory consumption per view (or window) in\n\
            megabytes. This mainly plays a role when viewing extravagant files like\n\
            high-resolution image files. The higher the maximum allowed memory consumption,\n\
            the earlier the files are shown and the more extravagant files are shown at all.\n\
            \n\
            IMPORTANT: This is just a guideline for the program. The internal algorithms\n\
            around this are working with heuristics and they are far from being exact. In\n\
            very seldom situations, a view may consume much more memory (factor two or so).\n\
            \n\
            RECOMMENDATION: The value should not be greater than a quarter of the total\n\
            system memory (RAM). Examples: 4096MB RAM => 1024MB; 8192MB RAM => 2048MB. This\n\
            is just a rough recommendation for an average system and user. It depends on the\n\
            number of windows you open, and on the memory consumption through other running\n\
            programs.\n\
            \n\
            WARNING: If you set a too large value, everything may work fine for a long time,\n\
            but one day it could happen you zoom into something and the whole system gets\n\
            extremely slow, or it even hangs, in lack of free memory.\n\
            \n\
            NOTE: After changing the value, you may have to restart the program for the\n\
            change to take effect. Or zoom out from all panels once.";
        let label = emLabel::new(label_text, self.look.clone());
        let label_id = ctx.create_child_with("label", Box::new(LabelPanel { label }));
        self.layout.set_child_constraint(
            label_id,
            ChildConstraint {
                weight: 5.0,
                ..Default::default()
            },
        );

        let mem_layout_id = ctx.create_child_with(
            "memfield",
            Box::new(MemFieldLayoutPanel::new(
                Rc::clone(&self.config),
                self.look.clone(),
            )),
        );
        self.layout.set_child_constraint(
            mem_layout_id,
            ChildConstraint {
                weight: 1.0,
                ..Default::default()
            },
        );
    }
}

impl PanelBehavior for MaxMemGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            state.enabled,
            true,
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100),
        );
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        let gen = self.generation.get();
        if gen != self.last_generation && ctx.child_count() > 0 {
            for id in ctx.children() {
                ctx.delete_child(id);
            }
            self.last_generation = gen;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let aux_id = crate::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

/// Bare emLinearLayout wrapping the memory emScalarField.
///
/// Visibility: `pub(crate)` in production; gated up to `pub` under the
/// `test-support` feature so the row 563 integration test in
/// `tests/rc_shim_b010.rs` can construct and drive it directly. Production
/// callers reach it only through `MaxMemGroup::create_children`.
#[cfg(any(test, feature = "test-support"))]
pub struct MemFieldLayoutPanel {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    layout: emLinearLayout,
    // B-010 row 563: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    mem_sig: SignalId,
    mem_id: Option<PanelId>,
}

#[cfg(not(any(test, feature = "test-support")))]
pub(crate) struct MemFieldLayoutPanel {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    layout: emLinearLayout,
    // B-010 row 563: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    mem_sig: SignalId,
    mem_id: Option<PanelId>,
}

impl MemFieldLayoutPanel {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>, look: Rc<emLook>) -> Self {
        Self {
            config,
            look,
            layout: emLinearLayout::horizontal().with_spacing(Spacing {
                margin_left: 0.02,
                margin_top: 0.05,
                margin_right: 0.05,
                margin_bottom: 0.0,
                ..Default::default()
            }),
            subscribed_init: false,
            mem_sig: SignalId::null(),
            mem_id: None,
        }
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
    ) -> Self {
        Self {
            config,
            look,
            layout: emLinearLayout::horizontal().with_spacing(Spacing {
                margin_left: 0.02,
                margin_top: 0.05,
                margin_right: 0.05,
                margin_bottom: 0.0,
                ..Default::default()
            }),
            subscribed_init: false,
            mem_sig: SignalId::null(),
            mem_id: None,
        }
    }

    /// Test-only accessor for the captured value_signal. Used by
    /// `tests/rc_shim_b010.rs` to fire the scalar field's signal directly
    /// without exposing the `pub(crate)` `ScalarFieldPanel` adapter.
    #[cfg(any(test, feature = "test-support"))]
    pub fn mem_sig_for_test(&self) -> SignalId {
        self.mem_sig
    }

    /// Test-only helper: pre-stage the child scalar field's `GetValue()` via
    /// the typed downcast `with_behavior_as::<FactorFieldPanel, _>`. Bypasses
    /// signal firing — the test fires the captured signal explicitly via
    /// `sched.fire(mem_sig_for_test())` after staging.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_mem_value_for_test(&self, tree: &mut crate::emPanelTree::PanelTree, value: f64) {
        let id = self.mem_id.expect("mem_id set in create_children");
        tree.with_behavior_as::<FactorFieldPanel, _>(id, |p| {
            p.scalar_field.set_value_for_test(value);
        });
    }

    /// Visibility: `pub(crate)` in production; gated up to `pub` under the
    /// `test-support` feature so row 563 integration test can drive child
    /// creation without invoking the full `LayoutChildren` path.
    #[cfg(any(test, feature = "test-support"))]
    pub fn create_children(&mut self, ctx: &mut PanelCtx) {
        self.create_children_impl(ctx);
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn create_children(&mut self, ctx: &mut PanelCtx) {
        self.create_children_impl(ctx);
    }

    fn create_children_impl(&mut self, ctx: &mut PanelCtx) {
        let (mem_sig, init_mb) = {
            let cfg = self.config.borrow();
            let c = cfg.GetRec();
            (
                c.MaxMegabytesPerView.listened_signal(),
                *c.MaxMegabytesPerView.GetValue() as i32,
            )
        };

        // Memory field: log2 space, range 8..16384 → ~300..1400 in val space
        let min_val = mem_cfg_to_val(8);
        let max_val = mem_cfg_to_val(16384);
        let mut sf = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emScalarField::new(&mut sched, min_val, max_val, self.look.clone())
        };
        sf.SetCaption("Max megabytes per view");
        sf.set_initial_value(mem_cfg_to_val(init_mb));
        sf.SetScaleMarkIntervals(&[100, 10]);
        sf.SetTextBoxTallness(0.3);
        sf.border_mut().SetBorderScaling(1.5);
        sf.SetTextOfValueFunc(Box::new(mem_text_of_value));

        // B-010 row 563: capture value_signal BEFORE the field moves into the
        // child behavior (SignalId is Copy).
        self.mem_sig = sf.value_signal;
        let update_config = Rc::clone(&self.config);
        let mem_id = ctx.create_child_with(
            "mem",
            Box::new(FactorFieldPanel {
                scalar_field: sf,
                config_sig: mem_sig,
                get_config_val: Some(Box::new(move || {
                    mem_cfg_to_val(
                        *update_config
                            .borrow()
                            .GetRec()
                            .MaxMegabytesPerView
                            .GetValue() as i32,
                    )
                })),
                subscribed_to_config: false,
            }),
        );
        self.mem_id = Some(mem_id);
    }
}

impl PanelBehavior for MemFieldLayoutPanel {
    fn Paint(
        &mut self,
        _painter: &mut emPainter,
        _canvas_color: emColor,
        _w: f64,
        _h: f64,
        _state: &PanelState,
    ) {
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        self.layout.do_layout_skip(ctx, None, None);
    }

    /// B-010 row 563: D-006 first-Cycle init + IsSignaled subscribe shape.
    ///
    /// Mirrors C++ `emCoreConfigPanel::MaxMemGroup::Cycle`
    /// (emCoreConfigPanel.cpp:503-519): subscribes to MemField's ValueSignal
    /// and on `IsSignaled(...)` propagates `GetValue()` through
    /// `mem_val_to_cfg` to `Config->MaxMegabytesPerView` + Save (if changed).
    fn Cycle(&mut self, ectx: &mut crate::emEngineCtx::EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        if !self.subscribed_init && self.mem_id.is_some() {
            let eid = ectx.id();
            ectx.connect(self.mem_sig, eid);
            self.subscribed_init = true;
            ectx.wake_up(eid);
        }

        if !self.mem_sig.is_null() && ectx.IsSignaled(self.mem_sig) {
            let val = ctx
                .tree
                .with_behavior_as::<FactorFieldPanel, _>(
                    self.mem_id.expect("mem_id set in create_children"),
                    |p| p.scalar_field.GetValue(),
                )
                .unwrap_or(0.0);
            let mb = mem_val_to_cfg(val).clamp(8, 16384) as i64;
            let mut cm = self.config.borrow_mut();
            let mut sched = ctx.as_sched_ctx().expect("sched");
            cm.modify(
                |c, sc| {
                    if *c.MaxMegabytesPerView.GetValue() != mb {
                        c.MaxMegabytesPerView.SetValue(mb, sc);
                    }
                },
                &mut sched,
            );
            let _ = cm.TrySave(false);
        }

        false
    }
}

// ---------------------------------------------------------------------------
// Composite Groups
// ---------------------------------------------------------------------------

/// Inner tunnel wrapping MaxMemGroup (child_tallness=0.7).
struct MaxMemInnerTunnelPanel {
    tunnel: emTunnel,
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
}

impl MaxMemInnerTunnelPanel {
    fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let mut tunnel = emTunnel::new(look.clone())
            .with_caption("Please read all text\nbefore changing this setting!");
        tunnel.SetChildTallness(0.7);
        Self {
            tunnel,
            config,
            look,
            generation,
        }
    }
}

impl PanelBehavior for MaxMemInnerTunnelPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.tunnel
            .paint_tunnel(painter, canvas_color, w, h, pixel_scale);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        if ctx.child_count() == 0 {
            ctx.create_child_with(
                "maxMemGroup",
                Box::new(MaxMemGroup::new(
                    Rc::clone(&self.config),
                    self.look.clone(),
                    Rc::clone(&self.generation),
                )),
            );
        }

        let rect = ctx.layout_rect();
        let cr = self
            .tunnel
            .GetChildRect(rect.w, rect.h, ctx.GetCanvasColor());
        if let Some(&child) = ctx.children().first() {
            ctx.layout_child(child, cr.x, cr.y, cr.w, cr.h);
            ctx.tree
                .SetCanvasColor(child, cr.canvas_color, ctx.scheduler.as_deref_mut());
        }
    }
}

/// Outer tunnel wrapping MaxMemInnerTunnelPanel (child_tallness=0.3, border_scaling=1.5).
struct MaxMemTunnelPanel {
    tunnel: emTunnel,
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
}

impl MaxMemTunnelPanel {
    fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let mut tunnel = emTunnel::new(look.clone());
        tunnel.SetChildTallness(0.3);
        tunnel.border_mut().SetBorderScaling(1.5);
        Self {
            tunnel,
            config,
            look,
            generation,
        }
    }
}

impl PanelBehavior for MaxMemTunnelPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.tunnel
            .paint_tunnel(painter, canvas_color, w, h, pixel_scale);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        if ctx.child_count() == 0 {
            ctx.create_child_with(
                "innerTunnel",
                Box::new(MaxMemInnerTunnelPanel::new(
                    Rc::clone(&self.config),
                    self.look.clone(),
                    Rc::clone(&self.generation),
                )),
            );
        }

        let rect = ctx.layout_rect();
        let cr = self
            .tunnel
            .GetChildRect(rect.w, rect.h, ctx.GetCanvasColor());
        if let Some(&child) = ctx.children().first() {
            ctx.layout_child(child, cr.x, cr.y, cr.w, cr.h);
            ctx.tree
                .SetCanvasColor(child, cr.canvas_color, ctx.scheduler.as_deref_mut());
        }
    }
}

/// CPU group — MaxRenderThreads scalar field + AllowSIMD checkbox.
///
/// Visibility: `pub(crate)` in production; gated up to `pub` under the
/// `test-support` feature so the row 746/755 integration tests in
/// `tests/rc_shim_b010.rs` can construct and drive it directly. Production
/// callers reach it only through `PerformanceGroup::create_children`.
#[cfg(any(test, feature = "test-support"))]
pub struct CpuGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: emBorder,
    layout: emLinearLayout,
    // B-010 rows 746/755: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    // Config aggregate subscribe for update_output after Reset.
    // Distinct from subscribed_init which gates per-checkbox wakeup subscriptions.
    subscribed_to_config: bool,
    config_sig: SignalId,
    threads_sig: SignalId,
    simd_sig: SignalId,
    threads_id: Option<PanelId>,
    simd_id: Option<PanelId>,
}

#[cfg(not(any(test, feature = "test-support")))]
pub(crate) struct CpuGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: emBorder,
    layout: emLinearLayout,
    // B-010 rows 746/755: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    // Config aggregate subscribe for update_output after Reset.
    // Distinct from subscribed_init which gates per-checkbox wakeup subscriptions.
    subscribed_to_config: bool,
    config_sig: SignalId,
    threads_sig: SignalId,
    simd_sig: SignalId,
    threads_id: Option<PanelId>,
    simd_id: Option<PanelId>,
}

impl CpuGroup {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        let config_sig = config.borrow().GetChangeSignal();
        let mut border = emBorder::new(OuterBorderType::Instrument)
            .with_inner(InnerBorderType::Group)
            .with_caption("CPU");
        border.SetBorderScaling(1.5);
        Self {
            config_sig,
            config,
            look,
            generation,
            last_generation: gen,
            border,
            layout: emLinearLayout::vertical().with_spacing(Spacing {
                inner_v: 0.1,
                ..Default::default()
            }),
            subscribed_init: false,
            subscribed_to_config: false,
            threads_sig: SignalId::null(),
            simd_sig: SignalId::null(),
            threads_id: None,
            simd_id: None,
        }
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        let config_sig = config.borrow().GetChangeSignal();
        let mut border = emBorder::new(OuterBorderType::Instrument)
            .with_inner(InnerBorderType::Group)
            .with_caption("CPU");
        border.SetBorderScaling(1.5);
        Self {
            config_sig,
            config,
            look,
            generation,
            last_generation: gen,
            border,
            layout: emLinearLayout::vertical().with_spacing(Spacing {
                inner_v: 0.1,
                ..Default::default()
            }),
            subscribed_init: false,
            subscribed_to_config: false,
            threads_sig: SignalId::null(),
            simd_sig: SignalId::null(),
            threads_id: None,
            simd_id: None,
        }
    }

    /// Test-only accessors for the captured signals. Used by
    /// `tests/rc_shim_b010.rs` to fire each child's signal directly without
    /// exposing the `pub(crate)` `ScalarFieldPanel`/`CheckBoxPanel` adapters.
    #[cfg(any(test, feature = "test-support"))]
    pub fn threads_sig_for_test(&self) -> SignalId {
        self.threads_sig
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn simd_sig_for_test(&self) -> SignalId {
        self.simd_sig
    }

    /// Test-only helper: pre-stage the threads scalar field's `GetValue()` via
    /// the typed downcast. Bypasses signal firing — the test fires the
    /// captured signal explicitly via `sched.fire(threads_sig_for_test())`
    /// after staging.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_threads_value_for_test(&self, tree: &mut crate::emPanelTree::PanelTree, value: f64) {
        let id = self.threads_id.expect("threads_id set in create_children");
        tree.with_behavior_as::<FactorFieldPanel, _>(id, |p| {
            p.scalar_field.set_value_for_test(value);
        });
    }

    /// Test-only helper: pre-stage the SIMD checkbox's `IsChecked()` via the
    /// typed downcast.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_simd_checked_for_test(
        &self,
        tree: &mut crate::emPanelTree::PanelTree,
        checked: bool,
    ) {
        let id = self.simd_id.expect("simd_id set in create_children");
        tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
            p.check_box.set_checked_for_test(checked);
        });
    }

    /// D4: propagate current config values back to the SIMD checkbox without
    /// triggering its own CheckSignal (uses `set_checked_silent`).
    /// Called when `config_sig` fires, so the UI reflects external config changes.
    fn update_output(&self, ctx: &mut PanelCtx) {
        let simd_val = *self.config.borrow().GetRec().AllowSIMD.GetValue();
        if let Some(id) = self.simd_id {
            ctx.tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
                p.check_box.set_checked_silent(simd_val);
            });
        }
    }

    /// Visibility: `pub(crate)` in production; gated up to `pub` under the
    /// `test-support` feature so row 746/755 integration tests can drive
    /// child creation without invoking the full `LayoutChildren` path.
    #[cfg(any(test, feature = "test-support"))]
    pub fn create_children(&mut self, ctx: &mut PanelCtx) {
        self.create_children_impl(ctx);
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn create_children(&mut self, ctx: &mut PanelCtx) {
        self.create_children_impl(ctx);
    }

    fn create_children_impl(&mut self, ctx: &mut PanelCtx) {
        let (threads_sig, threads_init, simd_init) = {
            let cfg = self.config.borrow();
            let c = cfg.GetRec();
            (
                c.MaxRenderThreads.listened_signal(),
                *c.MaxRenderThreads.GetValue() as f64,
                *c.AllowSIMD.GetValue(),
            )
        };

        // MaxRenderThreads: range 1-32
        let mut sf = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emScalarField::new(&mut sched, 1.0, 32.0, self.look.clone())
        };
        sf.SetCaption("Max render threads");
        sf.set_initial_value(threads_init);
        sf.SetScaleMarkIntervals(&[1]);
        sf.border_mut().outer = OuterBorderType::None;
        sf.border_mut().inner = InnerBorderType::InputField;
        sf.border_mut().SetBorderScaling(1.5);

        // B-010 row 746: capture value_signal BEFORE the field moves into
        // the child behavior (SignalId is Copy).
        self.threads_sig = sf.value_signal;
        let update_config = Rc::clone(&self.config);
        let threads_id = ctx.create_child_with(
            "MaxRenderThreads",
            Box::new(FactorFieldPanel {
                scalar_field: sf,
                config_sig: threads_sig,
                get_config_val: Some(Box::new(move || {
                    *update_config.borrow().GetRec().MaxRenderThreads.GetValue() as f64
                })),
                subscribed_to_config: false,
            }),
        );
        self.threads_id = Some(threads_id);
        self.layout.set_child_constraint(
            threads_id,
            ChildConstraint {
                weight: 4.0,
                ..Default::default()
            },
        );

        // AllowSIMD checkbox
        let mut cb = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(&mut sched, "Allow SIMD", self.look.clone())
        };
        cb.SetChecked(simd_init, ctx);
        // B-010 row 755: capture check_signal BEFORE child move.
        self.simd_sig = cb.check_signal;
        let simd_id = ctx.create_child_with("allowSIMD", Box::new(CheckBoxPanel { check_box: cb }));
        self.simd_id = Some(simd_id);
    }
}

impl PanelBehavior for CpuGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            state.enabled,
            true,
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100),
        );
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        let gen = self.generation.get();
        if gen != self.last_generation && ctx.child_count() > 0 {
            for id in ctx.children() {
                ctx.delete_child(id);
            }
            self.last_generation = gen;
            // B-010: child PanelIds and signals are stale after rebuild;
            // re-subscribe on the next Cycle once children are recreated.
            self.subscribed_init = false;
            self.threads_id = None;
            self.simd_id = None;
            self.threads_sig = SignalId::null();
            self.simd_sig = SignalId::null();
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let aux_id = crate::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    /// B-010 rows 746/755: D-006 first-Cycle init + IsSignaled subscribe shape.
    ///
    /// Mirrors C++ `emCoreConfigPanel::PerformanceGroup::Cycle`
    /// (emCoreConfigPanel.cpp:667-686): subscribes to MaxRenderThreadsField's
    /// ValueSignal and AllowSIMDBox's CheckSignal; on `IsSignaled(...)`
    /// propagates to `Config->MaxRenderThreads` / `Config->AllowSIMD` + Save
    /// (if changed). Branches in C++ source order: threads → SIMD.
    fn Cycle(&mut self, ectx: &mut crate::emEngineCtx::EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        // Config aggregate subscribe — distinct from per-checkbox subscribed_init.
        if !self.subscribed_to_config {
            ectx.connect(self.config_sig, ectx.id());
            self.subscribed_to_config = true;
        }
        if ectx.IsSignaled(self.config_sig) {
            self.update_output(ctx);
        }

        if !self.subscribed_init && self.threads_id.is_some() && self.simd_id.is_some() {
            let eid = ectx.id();
            ectx.connect(self.threads_sig, eid);
            ectx.connect(self.simd_sig, eid);
            self.subscribed_init = true;
            ectx.wake_up(eid);
        }

        if !self.threads_sig.is_null() && ectx.IsSignaled(self.threads_sig) {
            let val = ctx
                .tree
                .with_behavior_as::<FactorFieldPanel, _>(
                    self.threads_id.expect("threads_id set in create_children"),
                    |p| p.scalar_field.GetValue(),
                )
                .unwrap_or(0.0);
            let threads = ((val + 0.5) as i64).clamp(1, 32);
            let mut cm = self.config.borrow_mut();
            let mut sched = ctx.as_sched_ctx().expect("sched");
            cm.modify(
                |c, sc| {
                    if *c.MaxRenderThreads.GetValue() != threads {
                        c.MaxRenderThreads.SetValue(threads, sc);
                    }
                },
                &mut sched,
            );
            let _ = cm.TrySave(false);
        }

        if !self.simd_sig.is_null() && ectx.IsSignaled(self.simd_sig) {
            let checked = ctx
                .tree
                .with_behavior_as::<CheckBoxPanel, _>(
                    self.simd_id.expect("simd_id set in create_children"),
                    |p| p.check_box.IsChecked(),
                )
                .unwrap_or(false);
            let mut cm = self.config.borrow_mut();
            let mut sched = ctx.as_sched_ctx().expect("sched");
            cm.modify(
                |c, sc| {
                    if *c.AllowSIMD.GetValue() != checked {
                        c.AllowSIMD.SetValue(checked, sc);
                    }
                },
                &mut sched,
            );
            let _ = cm.TrySave(false);
        }

        false
    }
}

/// Mirrors C++ `emCoreConfigPanel::PerformanceGroup::InvalidatePaintingOfAllWindows`
/// (emCoreConfigPanel.cpp:828-843). Walks every window registered with the
/// engine and marks its compositor cache fully dirty so the next frame
/// repaints. Called from the Downscale/Upscale Cycle branches when the
/// corresponding config field changes.
fn InvalidatePaintingOfAllWindows(ectx: &mut crate::emEngineCtx::EngineCtx<'_>) {
    #[cfg(any(test, feature = "test-support"))]
    {
        INVALIDATE_PAINTING_OF_ALL_WINDOWS_CALLS.with(|c| c.set(c.get() + 1));
    }
    for w in ectx.windows.values_mut() {
        w.invalidate();
    }
}

// Test-only counter incremented every time `InvalidatePaintingOfAllWindows`
// fires. Used by `tests/rc_shim_b010.rs::row_791_…` to observe that the
// upscale Cycle branch invokes the helper without requiring a populated
// window registry in the test harness.
#[cfg(any(test, feature = "test-support"))]
thread_local! {
    pub static INVALIDATE_PAINTING_OF_ALL_WINDOWS_CALLS: Cell<u64> = const { Cell::new(0) };
}

/// Performance group — tunnel, CPU group, 2 quality scalar fields.
///
/// Visibility: `pub(crate)` in production; gated up to `pub` under the
/// `test-support` feature so the row 773/791 integration tests in
/// `tests/rc_shim_b010.rs` can construct and drive it directly. Production
/// callers reach it only through the root `emCoreConfigPanel` AutoExpand path.
#[cfg(any(test, feature = "test-support"))]
pub struct PerformanceGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: emBorder,
    layout: emRasterLayout,
    // B-010 rows 773/791: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    downscale_sig: SignalId,
    upscale_sig: SignalId,
    downscale_id: Option<PanelId>,
    upscale_id: Option<PanelId>,
}

#[cfg(not(any(test, feature = "test-support")))]
pub(crate) struct PerformanceGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: emBorder,
    layout: emRasterLayout,
    // B-010 rows 773/791: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    downscale_sig: SignalId,
    upscale_sig: SignalId,
    downscale_id: Option<PanelId>,
    upscale_id: Option<PanelId>,
}

impl PerformanceGroup {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Graphics Performance vs. Quality"),
            layout: emRasterLayout::new()
                .with_preferred_tallness(0.2)
                .with_spacing(Spacing {
                    margin_left: 0.05,
                    margin_top: 0.1,
                    margin_right: 0.05,
                    margin_bottom: 0.1,
                    ..Default::default()
                }),
            subscribed_init: false,
            downscale_sig: SignalId::null(),
            upscale_sig: SignalId::null(),
            downscale_id: None,
            upscale_id: None,
        }
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Graphics Performance vs. Quality"),
            layout: emRasterLayout::new()
                .with_preferred_tallness(0.2)
                .with_spacing(Spacing {
                    margin_left: 0.05,
                    margin_top: 0.1,
                    margin_right: 0.05,
                    margin_bottom: 0.1,
                    ..Default::default()
                }),
            subscribed_init: false,
            downscale_sig: SignalId::null(),
            upscale_sig: SignalId::null(),
            downscale_id: None,
            upscale_id: None,
        }
    }

    /// Test-only accessors for the captured downscale/upscale signals.
    #[cfg(any(test, feature = "test-support"))]
    pub fn downscale_sig_for_test(&self) -> SignalId {
        self.downscale_sig
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn upscale_sig_for_test(&self) -> SignalId {
        self.upscale_sig
    }

    /// Test-only helper: pre-stage the downscale scalar field's `GetValue()`.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_downscale_value_for_test(
        &self,
        tree: &mut crate::emPanelTree::PanelTree,
        value: f64,
    ) {
        let id = self
            .downscale_id
            .expect("downscale_id set in create_children");
        tree.with_behavior_as::<FactorFieldPanel, _>(id, |p| {
            p.scalar_field.set_value_for_test(value);
        });
    }

    /// Test-only helper: pre-stage the upscale scalar field's `GetValue()`.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_upscale_value_for_test(&self, tree: &mut crate::emPanelTree::PanelTree, value: f64) {
        let id = self.upscale_id.expect("upscale_id set in create_children");
        tree.with_behavior_as::<FactorFieldPanel, _>(id, |p| {
            p.scalar_field.set_value_for_test(value);
        });
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn create_children(&mut self, ctx: &mut PanelCtx) {
        self.create_children_impl(ctx);
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn create_children(&mut self, ctx: &mut PanelCtx) {
        self.create_children_impl(ctx);
    }

    fn create_children_impl(&mut self, ctx: &mut PanelCtx) {
        let (ds_sig, ds_init, us_sig, us_init) = {
            let cfg = self.config.borrow();
            let c = cfg.GetRec();
            (
                c.DownscaleQuality.listened_signal(),
                *c.DownscaleQuality.GetValue() as f64,
                c.UpscaleQuality.listened_signal(),
                *c.UpscaleQuality.GetValue() as f64,
            )
        };

        // MaxMem tunnel
        ctx.create_child_with(
            "maxmem",
            Box::new(MaxMemTunnelPanel::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
            )),
        );

        // CPU group
        ctx.create_child_with(
            "cpu",
            Box::new(CpuGroup::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
            )),
        );

        // DownscaleQuality: range 2-6
        let mut ds_sf = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emScalarField::new(&mut sched, 2.0, 6.0, self.look.clone())
        };
        ds_sf.SetCaption("Downscale quality");
        ds_sf.border_mut().description =
            "Quality of image downscaling (antialiasing filter size)".to_string();
        ds_sf.set_initial_value(ds_init);
        ds_sf.SetScaleMarkIntervals(&[1]);
        ds_sf.SetTextBoxTallness(0.3);
        ds_sf.border_mut().SetBorderScaling(1.5);
        ds_sf.SetTextOfValueFunc(Box::new(downscale_text));
        // B-010 row 773: capture value_signal BEFORE the field moves into
        // the child behavior (SignalId is Copy).
        self.downscale_sig = ds_sf.value_signal;
        let ds_update_config = Rc::clone(&self.config);
        let downscale_id = ctx.create_child_with(
            "downscaleQuality",
            Box::new(FactorFieldPanel {
                scalar_field: ds_sf,
                config_sig: ds_sig,
                get_config_val: Some(Box::new(move || {
                    *ds_update_config
                        .borrow()
                        .GetRec()
                        .DownscaleQuality
                        .GetValue() as f64
                })),
                subscribed_to_config: false,
            }),
        );
        self.downscale_id = Some(downscale_id);

        // UpscaleQuality: range 0-5 (0 = Nearest Pixel)
        let mut us_sf = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emScalarField::new(&mut sched, 0.0, 5.0, self.look.clone())
        };
        us_sf.SetCaption("Upscale quality");
        us_sf.border_mut().description = "Quality of image upscaling (interpolation)".to_string();
        us_sf.set_initial_value(us_init);
        us_sf.SetScaleMarkIntervals(&[1]);
        us_sf.SetTextBoxTallness(0.3);
        us_sf.border_mut().SetBorderScaling(1.5);
        us_sf.SetTextOfValueFunc(Box::new(upscale_text));
        // B-010 row 791: capture value_signal BEFORE child move.
        self.upscale_sig = us_sf.value_signal;
        let us_update_config = Rc::clone(&self.config);
        let upscale_id = ctx.create_child_with(
            "upscaleQuality",
            Box::new(FactorFieldPanel {
                scalar_field: us_sf,
                config_sig: us_sig,
                get_config_val: Some(Box::new(move || {
                    *us_update_config.borrow().GetRec().UpscaleQuality.GetValue() as f64
                })),
                subscribed_to_config: false,
            }),
        );
        self.upscale_id = Some(upscale_id);
    }
}

impl PanelBehavior for PerformanceGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            state.enabled,
            true,
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100),
        );
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        let gen = self.generation.get();
        if gen != self.last_generation && ctx.child_count() > 0 {
            for id in ctx.children() {
                ctx.delete_child(id);
            }
            self.last_generation = gen;
            // B-010: child PanelIds and signals are stale after rebuild;
            // re-subscribe on the next Cycle once children are recreated.
            self.subscribed_init = false;
            self.downscale_id = None;
            self.upscale_id = None;
            self.downscale_sig = SignalId::null();
            self.upscale_sig = SignalId::null();
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let aux_id = crate::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    /// B-010 rows 773/791: D-006 first-Cycle init + IsSignaled subscribe shape.
    ///
    /// Mirrors C++ `emCoreConfigPanel::PerformanceGroup::Cycle`
    /// (emCoreConfigPanel.cpp:687-712): subscribes to DownscaleQualityField's
    /// and UpscaleQualityField's ValueSignal; on `IsSignaled(...)` propagates
    /// the round/clamp transform to `Config->Downscale/UpscaleQuality` + Save,
    /// and (for both fields, per cpp:701/710) calls
    /// `InvalidatePaintingOfAllWindows` if the value actually changed.
    /// Branches in C++ source order: downscale → upscale.
    fn Cycle(&mut self, ectx: &mut crate::emEngineCtx::EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        if !self.subscribed_init && self.downscale_id.is_some() && self.upscale_id.is_some() {
            let eid = ectx.id();
            ectx.connect(self.downscale_sig, eid);
            ectx.connect(self.upscale_sig, eid);
            self.subscribed_init = true;
            ectx.wake_up(eid);
        }

        if !self.downscale_sig.is_null() && ectx.IsSignaled(self.downscale_sig) {
            let val = ctx
                .tree
                .with_behavior_as::<FactorFieldPanel, _>(
                    self.downscale_id
                        .expect("downscale_id set in create_children"),
                    |p| p.scalar_field.GetValue(),
                )
                .unwrap_or(0.0);
            let q = ((val + 0.5) as i64).clamp(2, 6);
            let changed = {
                let mut cm = self.config.borrow_mut();
                let mut sched = ctx.as_sched_ctx().expect("sched");
                let before = *cm.GetRec().DownscaleQuality.GetValue();
                cm.modify(
                    |c, sc| {
                        if *c.DownscaleQuality.GetValue() != q {
                            c.DownscaleQuality.SetValue(q, sc);
                        }
                    },
                    &mut sched,
                );
                let _ = cm.TrySave(false);
                before != q
            };
            if changed {
                InvalidatePaintingOfAllWindows(ectx);
            }
        }

        if !self.upscale_sig.is_null() && ectx.IsSignaled(self.upscale_sig) {
            let val = ctx
                .tree
                .with_behavior_as::<FactorFieldPanel, _>(
                    self.upscale_id.expect("upscale_id set in create_children"),
                    |p| p.scalar_field.GetValue(),
                )
                .unwrap_or(0.0);
            let q = ((val + 0.5) as i64).clamp(0, 5);
            let changed = {
                let mut cm = self.config.borrow_mut();
                let mut sched = ctx.as_sched_ctx().expect("sched");
                let before = *cm.GetRec().UpscaleQuality.GetValue();
                cm.modify(
                    |c, sc| {
                        if *c.UpscaleQuality.GetValue() != q {
                            c.UpscaleQuality.SetValue(q, sc);
                        }
                    },
                    &mut sched,
                );
                let _ = cm.TrySave(false);
                before != q
            };
            if changed {
                InvalidatePaintingOfAllWindows(ectx);
            }
        }

        false
    }
}

/// Mouse control group — 4 factor fields + MouseMiscGroup.
struct MouseGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    stick_possible: bool,
    border: emBorder,
    layout: emRasterLayout,
}

impl MouseGroup {
    fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
        stick_possible: bool,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            stick_possible,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Mouse Control"),
            layout: emRasterLayout::new()
                .with_preferred_tallness(0.2)
                .with_spacing(Spacing {
                    margin_left: 0.05,
                    margin_top: 0.1,
                    margin_right: 0.05,
                    margin_bottom: 0.1,
                    ..Default::default()
                }),
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        let (wz_sig, wz_init, wa_sig, wa_init, zoom_sig, zoom_init, scroll_sig, scroll_init) = {
            let cfg = self.config.borrow();
            let c = cfg.GetRec();
            (
                c.MouseWheelZoomSpeed.listened_signal(),
                *c.MouseWheelZoomSpeed.GetValue(),
                c.MouseWheelZoomAcceleration.listened_signal(),
                *c.MouseWheelZoomAcceleration.GetValue(),
                c.MouseZoomSpeed.listened_signal(),
                *c.MouseZoomSpeed.GetValue(),
                c.MouseScrollSpeed.listened_signal(),
                *c.MouseScrollSpeed.GetValue(),
            )
        };

        // wheelzoom
        let wz_config = Rc::clone(&self.config);
        let mut wz = make_factor_field(
            ctx,
            "Mouse wheel zoom speed",
            "Speed of zooming by mouse wheel",
            self.look.clone(),
            0.25,
            4.0,
            wz_init,
            false,
            wz_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *wz_config.borrow().GetRec().MouseWheelZoomSpeed.GetValue(),
                    0.25,
                    4.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        wz.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.MouseWheelZoomSpeed.SetValue(cfg_val, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("wheelzoom", Box::new(wz));

        // wheelaccel
        let wa_config = Rc::clone(&self.config);
        let mut wa = make_factor_field(
            ctx,
            "Mouse wheel zoom acceleration",
            "Acceleration of zooming by mouse wheel",
            self.look.clone(),
            0.25,
            2.0,
            wa_init,
            true,
            wa_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *wa_config
                        .borrow()
                        .GetRec()
                        .MouseWheelZoomAcceleration
                        .GetValue(),
                    0.25,
                    2.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        wa.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.25, 2.0);
                let mut cm = config.borrow_mut();
                cm.modify(
                    |c, sc| c.MouseWheelZoomAcceleration.SetValue(cfg_val, sc),
                    sched,
                );
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("wheelaccel", Box::new(wa));

        // zoom
        let zoom_config = Rc::clone(&self.config);
        let mut zoom = make_factor_field(
            ctx,
            "Mouse zoom speed",
            "Speed of zooming by mouse",
            self.look.clone(),
            0.25,
            4.0,
            zoom_init,
            false,
            zoom_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *zoom_config.borrow().GetRec().MouseZoomSpeed.GetValue(),
                    0.25,
                    4.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        zoom.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.MouseZoomSpeed.SetValue(cfg_val, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("zoom", Box::new(zoom));

        // scroll
        let scroll_config = Rc::clone(&self.config);
        let mut scroll = make_factor_field(
            ctx,
            "Mouse scroll speed",
            "Speed of scrolling by mouse",
            self.look.clone(),
            0.25,
            4.0,
            scroll_init,
            false,
            scroll_sig,
            Box::new(move || {
                factor_cfg_to_val(
                    *scroll_config.borrow().GetRec().MouseScrollSpeed.GetValue(),
                    0.25,
                    4.0,
                )
            }),
        );
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        scroll.scalar_field.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.MouseScrollSpeed.SetValue(cfg_val, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("scroll", Box::new(scroll));

        // MouseMiscGroup
        ctx.create_child_with(
            "misc",
            Box::new(MouseMiscGroup::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
                self.stick_possible,
            )),
        );
    }
}

impl PanelBehavior for MouseGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            state.enabled,
            true,
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100),
        );
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        let gen = self.generation.get();
        if gen != self.last_generation && ctx.child_count() > 0 {
            for id in ctx.children() {
                ctx.delete_child(id);
            }
            self.last_generation = gen;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let aux_id = crate::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ---------------------------------------------------------------------------
// Top-level panels
// ---------------------------------------------------------------------------

/// Buttons panel — Reset To Defaults button.
///
/// Visibility: `pub(crate)` in production; gated up to `pub` under the
/// `test-support` feature so the row-80 integration test in
/// `tests/rc_shim_b010.rs` can construct and drive it directly. Production
/// callers reach it only through `emCoreConfigPanel::create_children`.
#[cfg(any(test, feature = "test-support"))]
pub struct ButtonsPanel {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    layout: emLinearLayout,
    // B-010 row 80: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    bt_reset_sig: SignalId,
    reset_id: Option<PanelId>,
}

#[cfg(not(any(test, feature = "test-support")))]
pub(crate) struct ButtonsPanel {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    layout: emLinearLayout,
    // B-010 row 80: D-006 first-Cycle init + IsSignaled subscribe state.
    subscribed_init: bool,
    bt_reset_sig: SignalId,
    reset_id: Option<PanelId>,
}

impl ButtonsPanel {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        Self {
            config,
            look,
            generation,
            layout: emLinearLayout::horizontal()
                .with_alignment_h(AlignmentH::Right)
                .with_alignment_v(AlignmentV::Bottom),
            subscribed_init: false,
            bt_reset_sig: SignalId::null(),
            reset_id: None,
        }
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        Self {
            config,
            look,
            generation,
            layout: emLinearLayout::horizontal()
                .with_alignment_h(AlignmentH::Right)
                .with_alignment_v(AlignmentV::Bottom),
            subscribed_init: false,
            bt_reset_sig: SignalId::null(),
            reset_id: None,
        }
    }

    /// Test-only accessor for the captured Reset-button click_signal.
    /// Used by `tests/rc_shim_b010.rs` to fire the click without going through
    /// the `pub(crate)` ButtonPanel adapter.
    #[cfg(any(test, feature = "test-support"))]
    pub fn bt_reset_sig_for_test(&self) -> SignalId {
        self.bt_reset_sig
    }

    /// Visibility: `pub(crate)` in production; gated up to `pub` under the
    /// `test-support` feature so the row-80 integration test can drive child
    /// creation without invoking the full `LayoutChildren` path. Production
    /// callers reach this only through `LayoutChildren`.
    #[cfg(any(test, feature = "test-support"))]
    pub fn create_children(&mut self, ctx: &mut PanelCtx) {
        let btn = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Reset To Defaults", self.look.clone())
        };
        // B-010 row 80: capture the click_signal BEFORE the button moves into
        // the child behavior (SignalId is Copy).
        self.bt_reset_sig = btn.click_signal;
        let id = ctx.create_child_with("reset", Box::new(ButtonPanel { button: btn }));
        self.reset_id = Some(id);
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn create_children(&mut self, ctx: &mut PanelCtx) {
        let btn = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Reset To Defaults", self.look.clone())
        };
        // B-010 row 80: capture the click_signal BEFORE the button moves into
        // the child behavior (SignalId is Copy).
        self.bt_reset_sig = btn.click_signal;
        let id = ctx.create_child_with("reset", Box::new(ButtonPanel { button: btn }));
        self.reset_id = Some(id);
    }
}

impl PanelBehavior for ButtonsPanel {
    fn Paint(
        &mut self,
        _painter: &mut emPainter,
        _canvas_color: emColor,
        _w: f64,
        _h: f64,
        _state: &PanelState,
    ) {
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        self.layout.do_layout_skip(ctx, None, None);
    }

    /// B-010 row 80: D-006 first-Cycle init + IsSignaled subscribe shape.
    ///
    /// Mirrors C++ `emCoreConfigPanel::Cycle` (emCoreConfigPanel.cpp:42),
    /// which subscribes to `ResetButton->GetClickSignal()` and on
    /// `IsSignaled(...)` resets every config field to default + Save.
    fn Cycle(&mut self, ectx: &mut crate::emEngineCtx::EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        if !self.subscribed_init && self.reset_id.is_some() && !self.bt_reset_sig.is_null() {
            let eid = ectx.id();
            ectx.connect(self.bt_reset_sig, eid);
            self.subscribed_init = true;
            ectx.wake_up(eid);
        }
        if !self.bt_reset_sig.is_null() && ectx.IsSignaled(self.bt_reset_sig) {
            // Verbatim port of the deleted `btn.on_click` closure body
            // (D-009 sighting #4 — the generation bump is load-bearing for
            // visible Reset behaviour and must be preserved per design §2.1.1).
            let mut cm = self.config.borrow_mut();
            let mut sched = ctx.as_sched_ctx().expect("sched");
            cm.modify(
                |c, sc| {
                    c.StickMouseWhenNavigating.SetValue(false, sc);
                    c.EmulateMiddleButton.SetValue(false, sc);
                    c.PanFunction.SetValue(false, sc);
                    c.MouseZoomSpeed.SetValue(1.0, sc);
                    c.MouseScrollSpeed.SetValue(1.0, sc);
                    c.MouseWheelZoomSpeed.SetValue(1.0, sc);
                    c.MouseWheelZoomAcceleration.SetValue(1.0, sc);
                    c.KeyboardZoomSpeed.SetValue(1.0, sc);
                    c.KeyboardScrollSpeed.SetValue(1.0, sc);
                    c.KineticZoomingAndScrolling.SetValue(1.0, sc);
                    c.MagnetismRadius.SetValue(1.0, sc);
                    c.MagnetismSpeed.SetValue(1.0, sc);
                    c.VisitSpeed.SetValue(1.0, sc);
                    c.MaxMegabytesPerView.SetValue(2048, sc);
                    c.MaxRenderThreads.SetValue(8, sc);
                    c.AllowSIMD.SetValue(true, sc);
                    c.DownscaleQuality.SetValue(3, sc);
                    c.UpscaleQuality.SetValue(2, sc);
                },
                &mut sched,
            );
            let _ = cm.TrySave(false);
            self.generation.set(self.generation.get() + 1);
        }
        false
    }
}

/// Content panel — 4 groups in a raster layout.
struct ContentPanel {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    stick_possible: bool,
    layout: emRasterLayout,
}

impl ContentPanel {
    fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
        stick_possible: bool,
    ) -> Self {
        Self {
            config,
            look,
            generation,
            stick_possible,
            layout: emRasterLayout::new()
                .with_preferred_tallness(0.5)
                .with_spacing(Spacing {
                    inner_h: 0.1,
                    inner_v: 0.2,
                    ..Default::default()
                }),
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        ctx.create_child_with(
            "mouse",
            Box::new(MouseGroup::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
                self.stick_possible,
            )),
        );

        ctx.create_child_with(
            "keyboard",
            Box::new(KBGroup::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
            )),
        );

        ctx.create_child_with(
            "kinetic",
            Box::new(KineticGroup::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
            )),
        );

        ctx.create_child_with(
            "performance",
            Box::new(PerformanceGroup::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
            )),
        );
    }
}

impl PanelBehavior for ContentPanel {
    fn Paint(
        &mut self,
        _painter: &mut emPainter,
        _canvas_color: emColor,
        _w: f64,
        _h: f64,
        _state: &PanelState,
    ) {
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        self.layout.do_layout_skip(ctx, None, None);
    }
}

// ---------------------------------------------------------------------------
// emCoreConfigPanel — top-level panel
// ---------------------------------------------------------------------------

/// Settings UI panel for emCore configuration.
///
/// Port of C++ `emCoreConfigPanel`. Provides bidirectional binding between
/// UI controls and `emCoreConfig` settings via `emConfigModel`.
pub struct emCoreConfigPanel {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    /// Whether the screen can move the mouse pointer (C++ StickPossible).
    /// Controls whether the "Stick mouse when navigating" checkbox is enabled.
    stick_possible: bool,
    border: emBorder,
    layout: emLinearLayout,
}

impl emCoreConfigPanel {
    pub fn new(config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>, look: Rc<emLook>) -> Self {
        let mut border = emBorder::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("General Preferences");
        border.description = "This panel provides general user settings.".to_string();
        Self {
            config,
            look,
            generation: Rc::new(Cell::new(0)),
            stick_possible: true,
            border,
            layout: emLinearLayout::vertical().with_spacing(Spacing {
                margin_left: 0.01,
                margin_top: 0.1,
                margin_right: 0.01,
                margin_bottom: 0.1,
                inner_h: 0.01,
                ..Default::default()
            }),
        }
    }

    /// Set whether the "Stick mouse when navigating" checkbox is enabled.
    ///
    /// Pass `screen.can_move_mouse_pointer()`. Matches C++ emCoreConfigPanel line 232:
    /// `StickPossible = (screen && screen->CanMoveMousePointer())`.
    /// Must be called before the panel's children are first created (before first layout).
    pub fn SetStickPossible(&mut self, possible: bool) {
        self.stick_possible = possible;
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let content_id = ctx.create_child_with(
            "content",
            Box::new(ContentPanel::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
                self.stick_possible,
            )),
        );
        self.layout.set_child_constraint(
            content_id,
            ChildConstraint {
                weight: 12.0,
                ..Default::default()
            },
        );

        let buttons_id = ctx.create_child_with(
            "buttons",
            Box::new(ButtonsPanel::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
            )),
        );
        self.layout.set_child_constraint(
            buttons_id,
            ChildConstraint {
                weight: 1.0,
                ..Default::default()
            },
        );
    }
}

impl PanelBehavior for emCoreConfigPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            state.enabled,
            true,
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100),
        );
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let aux_id = crate::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factor_roundtrip() {
        for v in [-200.0, -100.0, 0.0, 100.0, 200.0] {
            let cfg = factor_val_to_cfg(v, 0.25, 4.0);
            let back = factor_cfg_to_val(cfg, 0.25, 4.0);
            assert!(
                (back - v).abs() < 1.0,
                "roundtrip failed for v={v}: cfg={cfg}, back={back}"
            );
        }
    }

    #[test]
    fn factor_identity() {
        let val = factor_val_to_cfg(0.0, 0.25, 4.0);
        assert!(
            (val - 1.0).abs() < 1e-10,
            "factor_val_to_cfg(0) should be 1.0, got {val}"
        );
    }

    #[test]
    fn factor_bounds() {
        let min_val = factor_val_to_cfg(-200.0, 0.25, 4.0);
        assert!(
            (min_val - 0.25).abs() < 0.01,
            "val_to_cfg(-200) should be ~0.25, got {min_val}"
        );
        let max_val = factor_val_to_cfg(200.0, 0.25, 4.0);
        assert!(
            (max_val - 4.0).abs() < 0.01,
            "val_to_cfg(200) should be ~4.0, got {max_val}"
        );
    }

    #[test]
    fn factor_text_named() {
        assert_eq!(factor_text_of_value(-200, 100, false, 0.25, 4.0), "Minimal");
        assert_eq!(factor_text_of_value(-100, 100, false, 0.25, 4.0), "Reduced");
        assert_eq!(factor_text_of_value(0, 100, false, 0.25, 4.0), "Default");
        assert_eq!(
            factor_text_of_value(100, 100, false, 0.25, 4.0),
            "Increased"
        );
        assert_eq!(factor_text_of_value(200, 100, false, 0.25, 4.0), "Extreme");
    }

    #[test]
    fn factor_text_disabled() {
        assert_eq!(factor_text_of_value(-200, 100, true, 0.25, 4.0), "Disabled");
        assert_eq!(factor_text_of_value(-200, 100, false, 0.25, 4.0), "Minimal");
    }

    #[test]
    fn factor_text_decimal() {
        let text = factor_text_of_value(0, 10, false, 0.25, 4.0);
        assert_eq!(text, "1.00");
    }

    #[test]
    fn mem_roundtrip() {
        let val = mem_cfg_to_val(2048);
        let back = mem_val_to_cfg(val);
        assert_eq!(back, 2048, "mem roundtrip: val={val}, back={back}");
    }

    #[test]
    fn mem_text() {
        let text = mem_text_of_value(300, 100);
        assert_eq!(text, "8");
    }

    #[test]
    fn downscale_text_check() {
        assert_eq!(downscale_text(3, 1), "3x3");
        assert_eq!(downscale_text(0, 1), "Nearest\nPixel");
    }

    #[test]
    fn upscale_text_check() {
        assert_eq!(upscale_text(2, 1), "Bilinear");
        assert_eq!(upscale_text(0, 1), "Nearest\nPixel");
        assert_eq!(upscale_text(5, 1), "Adaptive");
    }

    #[test]
    fn factor_val_to_cfg_roundtrip() {
        let cfg_min = 0.25;
        let cfg_max = 4.0;

        // value=0 → cfg=1.0 (default)
        let cfg = factor_val_to_cfg(0.0, cfg_min, cfg_max);
        assert!((cfg - 1.0).abs() < 1e-12, "expected cfg=1.0, got {cfg}");
        let back = factor_cfg_to_val(cfg, cfg_min, cfg_max);
        assert!((back - 0.0).abs() <= 1.0, "roundtrip 0: got {back}");

        // value=100 → cfg=2.0
        let cfg = factor_val_to_cfg(100.0, cfg_min, cfg_max);
        assert!((cfg - 2.0).abs() < 1e-12, "expected cfg=2.0, got {cfg}");
        let back = factor_cfg_to_val(cfg, cfg_min, cfg_max);
        assert!((back - 100.0).abs() <= 1.0, "roundtrip 100: got {back}");

        // value=-100 → cfg=0.5
        let cfg = factor_val_to_cfg(-100.0, cfg_min, cfg_max);
        assert!((cfg - 0.5).abs() < 1e-12, "expected cfg=0.5, got {cfg}");
        let back = factor_cfg_to_val(cfg, cfg_min, cfg_max);
        assert!((back - -100.0).abs() <= 1.0, "roundtrip -100: got {back}");

        // value=200 → cfg=4.0
        let cfg = factor_val_to_cfg(200.0, cfg_min, cfg_max);
        assert!((cfg - 4.0).abs() < 1e-12, "expected cfg=4.0, got {cfg}");
        let back = factor_cfg_to_val(cfg, cfg_min, cfg_max);
        assert!((back - 200.0).abs() <= 1.0, "roundtrip 200: got {back}");
    }

    #[test]
    fn factor_text_disabled_label() {
        assert_eq!(factor_text_of_value(-200, 100, true, 0.25, 4.0), "Disabled");
    }

    #[test]
    fn factor_text_default_label() {
        assert_eq!(factor_text_of_value(0, 100, false, 0.25, 4.0), "Default");
    }

    #[test]
    fn memory_field_log2_roundtrip() {
        for &mb in &[8, 1024, 16384] {
            let val = mem_cfg_to_val(mb);
            let back = mem_val_to_cfg(val);
            assert_eq!(
                back, mb,
                "mem roundtrip failed for {mb}: val={val}, back={back}"
            );
        }
    }
}

#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_factor_cfg_to_val() {
        let mut p_d: f64 = kani::any::<f64>();
        kani::assume(p_d.is_finite());
        let mut p_cfg_min: f64 = kani::any::<f64>();
        kani::assume(p_cfg_min.is_finite());
        let mut p_cfg_max: f64 = kani::any::<f64>();
        kani::assume(p_cfg_max.is_finite());
        let _r = factor_cfg_to_val(p_d, p_cfg_min, p_cfg_max);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_factor_val_to_cfg() {
        let mut p_value: f64 = kani::any::<f64>();
        kani::assume(p_value.is_finite());
        let mut p_cfg_min: f64 = kani::any::<f64>();
        kani::assume(p_cfg_min.is_finite());
        let mut p_cfg_max: f64 = kani::any::<f64>();
        kani::assume(p_cfg_max.is_finite());
        let _r = factor_val_to_cfg(p_value, p_cfg_min, p_cfg_max);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_mem_cfg_to_val() {
        let mut p_mb: i32 = kani::any::<i32>();
        let _r = mem_cfg_to_val(p_mb);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_mem_val_to_cfg() {
        let mut p_val: f64 = kani::any::<f64>();
        kani::assume(p_val.is_finite());
        let _r = mem_val_to_cfg(p_val);
    }
}
