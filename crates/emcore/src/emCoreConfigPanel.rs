use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::emColor::emColor;
use crate::emCoreConfig::emCoreConfig;
use crate::emEngineCtx::PanelCtx;
use crate::emLinearLayout::emLinearLayout;
use crate::emPainter::emPainter;
use crate::emPanel::{PanelBehavior, PanelState};
use crate::emRasterLayout::emRasterLayout;
use crate::emRec::emRec;
use crate::emRecNodeConfigModel::emRecNodeConfigModel;
use crate::emTiling::{AlignmentH, AlignmentV, ChildConstraint, Spacing};

use super::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use super::emColorFieldFieldPanel::{ButtonPanel, CheckBoxPanel, LabelPanel, ScalarFieldPanel};
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
// Factory helper: build a ScalarFieldPanel with factor-field config
// ---------------------------------------------------------------------------

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
) -> ScalarFieldPanel {
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
    ScalarFieldPanel { scalar_field: sf }
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
        let cfg = self.config.borrow();
        let c = cfg.GetRec();

        let mut zoom = make_factor_field(
            ctx,
            "Keyboard zoom speed",
            "Speed of zooming by keyboard",
            self.look.clone(),
            0.25,
            4.0,
            *c.KeyboardZoomSpeed.GetValue(),
            false,
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

        let mut scroll = make_factor_field(
            ctx,
            "Keyboard scroll speed",
            "Speed of scrolling by keyboard",
            self.look.clone(),
            0.25,
            4.0,
            *c.KeyboardScrollSpeed.GetValue(),
            false,
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
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
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

/// Miscellaneous mouse settings group — 3 checkboxes.
struct MouseMiscGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    stick_possible: bool,
    border: emBorder,
    layout: emRasterLayout,
}

impl MouseMiscGroup {
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
                .with_caption("Miscellaneous mouse settings"),
            layout: emRasterLayout::new().with_preferred_tallness(0.04),
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        let cfg = self.config.borrow();
        let c = cfg.GetRec();

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
        stick.SetChecked(*c.StickMouseWhenNavigating.GetValue(), ctx);
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        stick.on_check = Some(Box::new(
            move |checked, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let mut cm = config.borrow_mut();
                cm.modify(
                    |c, sc| c.StickMouseWhenNavigating.SetValue(checked, sc),
                    sched,
                );
                let _ = cm.TrySave(false);
            },
        ));
        let stick_id = ctx.create_child_with("stick", Box::new(CheckBoxPanel { check_box: stick }));
        if !self.stick_possible {
            ctx.tree
                .SetEnableSwitch(stick_id, false, ctx.scheduler.as_deref_mut());
        }

        let mut emu = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(&mut sched, "Emulate\nmiddle button", self.look.clone())
        };
        emu.SetChecked(*c.EmulateMiddleButton.GetValue(), ctx);
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        emu.on_check = Some(Box::new(
            move |checked, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.EmulateMiddleButton.SetValue(checked, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("emu", Box::new(CheckBoxPanel { check_box: emu }));

        let mut pan = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emCheckBox::new(&mut sched, "Pan\nfunction", self.look.clone())
        };
        pan.SetChecked(*c.PanFunction.GetValue(), ctx);
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        pan.on_check = Some(Box::new(
            move |checked, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.PanFunction.SetValue(checked, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("pan", Box::new(CheckBoxPanel { check_box: pan }));
    }
}

impl PanelBehavior for MouseMiscGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
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
        let cfg = self.config.borrow();
        let c = cfg.GetRec();

        // KineticZoomingAndScrolling
        let mut kinetic = make_factor_field(
            ctx,
            "Kinetic zooming and scrolling",
            "Whether and how much to have kinetic effects on zooming and scrolling",
            self.look.clone(),
            0.25,
            2.0,
            *c.KineticZoomingAndScrolling.GetValue(),
            true,
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
        let mut mag_radius = make_factor_field(
            ctx,
            "Magnetism radius",
            "Maximum radius for magnetism to snap the focus to nearby panels",
            self.look.clone(),
            0.25,
            4.0,
            *c.MagnetismRadius.GetValue(),
            true,
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
        let mut mag_speed = make_factor_field(
            ctx,
            "Magnetism speed",
            "Speed of the magnetism movement",
            self.look.clone(),
            0.25,
            4.0,
            *c.MagnetismSpeed.GetValue(),
            false,
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
        let mut visit = make_factor_field(
            ctx,
            "Visit speed",
            "Speed of the visit animation",
            self.look.clone(),
            0.1,
            10.0,
            *c.VisitSpeed.GetValue(),
            false,
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
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
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
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
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
struct MemFieldLayoutPanel {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    layout: emLinearLayout,
}

impl MemFieldLayoutPanel {
    fn new(config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>, look: Rc<emLook>) -> Self {
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
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        let cfg = self.config.borrow();
        let c = cfg.GetRec();

        // Memory field: log2 space, range 8..16384 → ~300..1400 in val space
        let min_val = mem_cfg_to_val(8);
        let max_val = mem_cfg_to_val(16384);
        let mut sf = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emScalarField::new(&mut sched, min_val, max_val, self.look.clone())
        };
        sf.SetCaption("Max megabytes per view");
        sf.set_initial_value(mem_cfg_to_val(*c.MaxMegabytesPerView.GetValue() as i32));
        sf.SetScaleMarkIntervals(&[100, 10]);
        sf.SetTextBoxTallness(0.3);
        sf.border_mut().SetBorderScaling(1.5);
        sf.SetTextOfValueFunc(Box::new(mem_text_of_value));
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        sf.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let mb = mem_val_to_cfg(val).clamp(8, 16384) as i64;
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.MaxMegabytesPerView.SetValue(mb, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("mem", Box::new(ScalarFieldPanel { scalar_field: sf }));
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
struct CpuGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: emBorder,
    layout: emLinearLayout,
}

impl CpuGroup {
    fn new(
        config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
        look: Rc<emLook>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        let mut border = emBorder::new(OuterBorderType::Instrument)
            .with_inner(InnerBorderType::Group)
            .with_caption("CPU");
        border.SetBorderScaling(1.5);
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border,
            layout: emLinearLayout::vertical().with_spacing(Spacing {
                inner_v: 0.1,
                ..Default::default()
            }),
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let cfg = self.config.borrow();
        let c = cfg.GetRec();

        // MaxRenderThreads: range 1-32
        let mut sf = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emScalarField::new(&mut sched, 1.0, 32.0, self.look.clone())
        };
        sf.SetCaption("Max render threads");
        sf.set_initial_value(*c.MaxRenderThreads.GetValue() as f64);
        sf.SetScaleMarkIntervals(&[1]);
        sf.border_mut().outer = OuterBorderType::None;
        sf.border_mut().inner = InnerBorderType::InputField;
        sf.border_mut().SetBorderScaling(1.5);
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        sf.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let threads = ((val + 0.5) as i64).clamp(1, 32);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.MaxRenderThreads.SetValue(threads, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        let threads_id = ctx.create_child_with(
            "MaxRenderThreads",
            Box::new(ScalarFieldPanel { scalar_field: sf }),
        );
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
        cb.SetChecked(*c.AllowSIMD.GetValue(), ctx);
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        cb.on_check = Some(Box::new(
            move |checked, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.AllowSIMD.SetValue(checked, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with("allowSIMD", Box::new(CheckBoxPanel { check_box: cb }));
    }
}

impl PanelBehavior for CpuGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
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

/// Performance group — tunnel, CPU group, 2 quality scalar fields.
struct PerformanceGroup {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: emBorder,
    layout: emRasterLayout,
}

impl PerformanceGroup {
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
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        let cfg = self.config.borrow();
        let c = cfg.GetRec();

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
        ds_sf.set_initial_value(*c.DownscaleQuality.GetValue() as f64);
        ds_sf.SetScaleMarkIntervals(&[1]);
        ds_sf.SetTextBoxTallness(0.3);
        ds_sf.border_mut().SetBorderScaling(1.5);
        ds_sf.SetTextOfValueFunc(Box::new(downscale_text));
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        ds_sf.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let q = ((val + 0.5) as i64).clamp(2, 6);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.DownscaleQuality.SetValue(q, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with(
            "downscaleQuality",
            Box::new(ScalarFieldPanel {
                scalar_field: ds_sf,
            }),
        );

        // UpscaleQuality: range 0-5 (0 = Nearest Pixel)
        let mut us_sf = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emScalarField::new(&mut sched, 0.0, 5.0, self.look.clone())
        };
        us_sf.SetCaption("Upscale quality");
        us_sf.border_mut().description = "Quality of image upscaling (interpolation)".to_string();
        us_sf.set_initial_value(*c.UpscaleQuality.GetValue() as f64);
        us_sf.SetScaleMarkIntervals(&[1]);
        us_sf.SetTextBoxTallness(0.3);
        us_sf.border_mut().SetBorderScaling(1.5);
        us_sf.SetTextOfValueFunc(Box::new(upscale_text));
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        us_sf.on_value = Some(Box::new(
            move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let q = ((val + 0.5) as i64).clamp(0, 5);
                let mut cm = config.borrow_mut();
                cm.modify(|c, sc| c.UpscaleQuality.SetValue(q, sc), sched);
                let _ = cm.TrySave(false);
            },
        ));
        ctx.create_child_with(
            "upscaleQuality",
            Box::new(ScalarFieldPanel {
                scalar_field: us_sf,
            }),
        );
    }
}

impl PanelBehavior for PerformanceGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
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
        let cfg = self.config.borrow();
        let c = cfg.GetRec();

        // wheelzoom
        let mut wz = make_factor_field(
            ctx,
            "Mouse wheel zoom speed",
            "Speed of zooming by mouse wheel",
            self.look.clone(),
            0.25,
            4.0,
            *c.MouseWheelZoomSpeed.GetValue(),
            false,
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
        let mut wa = make_factor_field(
            ctx,
            "Mouse wheel zoom acceleration",
            "Acceleration of zooming by mouse wheel",
            self.look.clone(),
            0.25,
            2.0,
            *c.MouseWheelZoomAcceleration.GetValue(),
            true,
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
        let mut zoom = make_factor_field(
            ctx,
            "Mouse zoom speed",
            "Speed of zooming by mouse",
            self.look.clone(),
            0.25,
            4.0,
            *c.MouseZoomSpeed.GetValue(),
            false,
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
        let mut scroll = make_factor_field(
            ctx,
            "Mouse scroll speed",
            "Speed of scrolling by mouse",
            self.look.clone(),
            0.25,
            4.0,
            *c.MouseScrollSpeed.GetValue(),
            false,
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
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
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
struct ButtonsPanel {
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    look: Rc<emLook>,
    generation: Rc<Cell<u64>>,
    layout: emLinearLayout,
}

impl ButtonsPanel {
    fn new(
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
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        let mut btn = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Reset To Defaults", self.look.clone())
        };
        let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = Rc::clone(&self.config);
        let generation = Rc::clone(&self.generation);
        btn.on_click = Some(Box::new(
            move |(), sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                let mut cm = config.borrow_mut();
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
                    sched,
                );
                let _ = cm.TrySave(false);
                generation.set(generation.get() + 1);
            },
        ));
        ctx.create_child_with("reset", Box::new(ButtonPanel { button: btn }));
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
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.border.paint_border(
            painter,
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
