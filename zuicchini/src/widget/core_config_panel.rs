use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::layout::linear::LinearLayout;
use crate::layout::raster::RasterLayout;
use crate::layout::{AlignmentH, AlignmentV, ChildConstraint, Spacing};
use crate::model::{ConfigModel, CoreConfig};
use crate::panel::{PanelBehavior, PanelCtx, PanelState};
use crate::render::Painter;

use super::border::{Border, InnerBorderType, OuterBorderType};
use super::button::Button;
use super::check_box::CheckBox;
use super::field_panel::{ButtonPanel, CheckBoxPanel, LabelPanel, ScalarFieldPanel};
use super::label::Label;
use super::look::Look;
use super::scalar_field::ScalarField;
use super::tunnel::Tunnel;

// ---------------------------------------------------------------------------
// Pure conversion functions (C++ emCoreConfigPanel.cpp factor field logic)
// ---------------------------------------------------------------------------

/// ScalarField value (-200..+200) to config domain value.
fn factor_val_to_cfg(value: f64, cfg_min: f64, cfg_max: f64) -> f64 {
    let m = if value >= 0.0 { cfg_max } else { 1.0 / cfg_min };
    m.sqrt().powf(value / 100.0)
}

/// Config domain value to ScalarField value (-200..+200), rounded.
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

/// Memory MB to ScalarField value (log2 space).
fn mem_cfg_to_val(mb: i32) -> f64 {
    (mb as f64).ln() / 2.0_f64.ln() * 100.0
}

/// ScalarField value to memory MB (log2 space).
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

fn make_factor_field(
    caption: &str,
    description: &str,
    look: Rc<Look>,
    cfg_min: f64,
    cfg_max: f64,
    cfg_value: f64,
    minimum_means_disabled: bool,
) -> ScalarFieldPanel {
    let mut sf = ScalarField::new(-200.0, 200.0, look);
    sf.set_caption(caption);
    sf.border_mut().description = description.to_string();
    sf.set_value(factor_cfg_to_val(cfg_value, cfg_min, cfg_max));
    sf.set_scale_mark_intervals(&[100, 10]);
    sf.set_text_box_tallness(0.3);
    sf.border_mut().set_border_scaling(1.5);
    let (min, max, dis) = (cfg_min, cfg_max, minimum_means_disabled);
    sf.set_text_of_value_fn(Box::new(move |v, mi| {
        factor_text_of_value(v, mi, dis, min, max)
    }));
    ScalarFieldPanel { scalar_field: sf }
}

// ---------------------------------------------------------------------------
// Leaf Groups
// ---------------------------------------------------------------------------

/// Keyboard control group — 2 factor fields.
struct KBGroup {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: Border,
    layout: RasterLayout,
}

impl KBGroup {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: Border::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Keyboard Control"),
            layout: RasterLayout::new()
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
        let c = cfg.get();

        let mut zoom = make_factor_field(
            "Keyboard zoom speed",
            "Speed of zooming by keyboard",
            self.look.clone(),
            0.25,
            4.0,
            c.keyboard_zoom_speed,
            false,
        );
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        zoom.scalar_field.on_value = Some(Box::new(move |val| {
            let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.keyboard_zoom_speed = cfg_val);
            let _ = cm.save();
        }));
        ctx.create_child_with("zoom", Box::new(zoom));

        let mut scroll = make_factor_field(
            "Keyboard scroll speed",
            "Speed of scrolling by keyboard",
            self.look.clone(),
            0.25,
            4.0,
            c.keyboard_scroll_speed,
            false,
        );
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        scroll.scalar_field.on_value = Some(Box::new(move |val| {
            let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.keyboard_scroll_speed = cfg_val);
            let _ = cm.save();
        }));
        ctx.create_child_with("scroll", Box::new(scroll));
    }
}

impl PanelBehavior for KBGroup {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.enabled, true);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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

        let aux_id = crate::layout::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.content_rect_unobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

/// Miscellaneous mouse settings group — 3 checkboxes.
struct MouseMiscGroup {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: Border,
    layout: RasterLayout,
}

impl MouseMiscGroup {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: Border::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Miscellaneous mouse settings"),
            layout: RasterLayout::new().with_preferred_tallness(0.04),
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        let cfg = self.config.borrow();
        let c = cfg.get();

        let mut stick = CheckBox::new("Stick mouse\nwhen navigating", self.look.clone());
        stick.set_checked(c.stick_mouse_when_navigating);
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        stick.on_check = Some(Box::new(move |checked| {
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.stick_mouse_when_navigating = checked);
            let _ = cm.save();
        }));
        ctx.create_child_with("stick", Box::new(CheckBoxPanel { check_box: stick }));

        let mut emu = CheckBox::new("Emulate\nmiddle button", self.look.clone());
        emu.set_checked(c.emulate_middle_button);
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        emu.on_check = Some(Box::new(move |checked| {
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.emulate_middle_button = checked);
            let _ = cm.save();
        }));
        ctx.create_child_with("emu", Box::new(CheckBoxPanel { check_box: emu }));

        let mut pan = CheckBox::new("Pan\nfunction", self.look.clone());
        pan.set_checked(c.pan_function);
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        pan.on_check = Some(Box::new(move |checked| {
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.pan_function = checked);
            let _ = cm.save();
        }));
        ctx.create_child_with("pan", Box::new(CheckBoxPanel { check_box: pan }));
    }
}

impl PanelBehavior for MouseMiscGroup {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.enabled, true);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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

        let aux_id = crate::layout::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.content_rect_unobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

/// Kinetic effects group — 4 factor fields.
struct KineticGroup {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: Border,
    layout: RasterLayout,
}

impl KineticGroup {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: Border::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Kinetic Effects"),
            layout: RasterLayout::new()
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
        let c = cfg.get();

        // KineticZoomingAndScrolling — disabled
        let kinetic = make_factor_field(
            "Kinetic zooming and scrolling",
            "Whether and how much to have kinetic effects on zooming and scrolling",
            self.look.clone(),
            0.25,
            2.0,
            c.kinetic_zooming_and_scrolling,
            true,
        );
        ctx.create_child_with("KineticZoomingAndScrolling", Box::new(kinetic));

        // MagnetismRadius — disabled
        let mag_radius = make_factor_field(
            "Magnetism radius",
            "Maximum radius for magnetism to snap the focus to nearby panels",
            self.look.clone(),
            0.25,
            4.0,
            c.magnetism_radius,
            true,
        );
        ctx.create_child_with("MagnetismRadius", Box::new(mag_radius));

        // MagnetismSpeed
        let mut mag_speed = make_factor_field(
            "Magnetism speed",
            "Speed of the magnetism movement",
            self.look.clone(),
            0.25,
            4.0,
            c.magnetism_speed,
            false,
        );
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        mag_speed.scalar_field.on_value = Some(Box::new(move |val| {
            let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.magnetism_speed = cfg_val);
            let _ = cm.save();
        }));
        ctx.create_child_with("MagnetismSpeed", Box::new(mag_speed));

        // VisitSpeed
        let mut visit = make_factor_field(
            "Visit speed",
            "Speed of the visit animation",
            self.look.clone(),
            0.1,
            10.0,
            c.visit_speed,
            false,
        );
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        visit.scalar_field.on_value = Some(Box::new(move |val| {
            let cfg_val = factor_val_to_cfg(val, 0.1, 10.0);
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.visit_speed = cfg_val);
            let _ = cm.save();
        }));
        ctx.create_child_with("VisitSpeed", Box::new(visit));
    }
}

impl PanelBehavior for KineticGroup {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.enabled, true);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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

        let aux_id = crate::layout::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.content_rect_unobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

/// Max megabytes per view group — label + scalar field.
struct MaxMemGroup {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: Border,
    layout: LinearLayout,
}

impl MaxMemGroup {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: Border::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Max Megabytes Per View"),
            layout: LinearLayout::vertical(),
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let label_text = "WARNING: If this is set too high, the system may\n\
            get extremely slow, or the program could even abort.\n\
            The value is the maximum number of megabytes for the\n\
            pixel engines of a single view. Please note that the\n\
            total consumption can be much more, depending on what\n\
            file types are shown, and how many views are open.";
        let label = Label::new(label_text, self.look.clone());
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
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.enabled, true);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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

        let aux_id = crate::layout::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.content_rect_unobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

/// Bare LinearLayout wrapping the memory ScalarField.
struct MemFieldLayoutPanel {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    layout: LinearLayout,
}

impl MemFieldLayoutPanel {
    fn new(config: Rc<RefCell<ConfigModel<CoreConfig>>>, look: Rc<Look>) -> Self {
        Self {
            config,
            look,
            layout: LinearLayout::horizontal().with_spacing(Spacing {
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
        let c = cfg.get();

        // Memory field: log2 space, range 8..16384 → ~300..1400 in val space
        let min_val = mem_cfg_to_val(8);
        let max_val = mem_cfg_to_val(16384);
        let mut sf = ScalarField::new(min_val, max_val, self.look.clone());
        sf.set_caption("Max megabytes per view");
        sf.set_value(mem_cfg_to_val(c.max_megabytes_per_view));
        sf.set_scale_mark_intervals(&[100, 10]);
        sf.set_text_box_tallness(0.3);
        sf.border_mut().set_border_scaling(1.5);
        sf.set_text_of_value_fn(Box::new(mem_text_of_value));
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        sf.on_value = Some(Box::new(move |val| {
            let mb = mem_val_to_cfg(val);
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.max_megabytes_per_view = mb.clamp(8, 16384));
            let _ = cm.save();
        }));
        ctx.create_child_with("mem", Box::new(ScalarFieldPanel { scalar_field: sf }));
    }
}

impl PanelBehavior for MemFieldLayoutPanel {
    fn paint(&mut self, _painter: &mut Painter, _w: f64, _h: f64, _state: &PanelState) {}

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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
    tunnel: Tunnel,
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
}

impl MaxMemInnerTunnelPanel {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let mut tunnel = Tunnel::new(look.clone())
            .with_caption("Please read all text\nbefore changing this setting!");
        tunnel.set_child_tallness(0.7);
        Self {
            tunnel,
            config,
            look,
            generation,
        }
    }
}

impl PanelBehavior for MaxMemInnerTunnelPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.tunnel.paint_tunnel(painter, w, h);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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
        let cr = self.tunnel.child_rect(rect.w, rect.h, ctx.canvas_color());
        if let Some(&child) = ctx.children().first() {
            ctx.layout_child(child, cr.x, cr.y, cr.w, cr.h);
            ctx.tree.set_canvas_color(child, cr.canvas_color);
        }
    }
}

/// Outer tunnel wrapping MaxMemInnerTunnelPanel (child_tallness=0.3, border_scaling=1.5).
struct MaxMemTunnelPanel {
    tunnel: Tunnel,
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
}

impl MaxMemTunnelPanel {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let mut tunnel = Tunnel::new(look.clone());
        tunnel.set_child_tallness(0.3);
        tunnel.border_mut().set_border_scaling(1.5);
        Self {
            tunnel,
            config,
            look,
            generation,
        }
    }
}

impl PanelBehavior for MaxMemTunnelPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.tunnel.paint_tunnel(painter, w, h);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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
        let cr = self.tunnel.child_rect(rect.w, rect.h, ctx.canvas_color());
        if let Some(&child) = ctx.children().first() {
            ctx.layout_child(child, cr.x, cr.y, cr.w, cr.h);
            ctx.tree.set_canvas_color(child, cr.canvas_color);
        }
    }
}

/// CPU group — MaxRenderThreads scalar field + AllowSIMD checkbox.
struct CpuGroup {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: Border,
    layout: LinearLayout,
}

impl CpuGroup {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        let mut border = Border::new(OuterBorderType::Instrument)
            .with_inner(InnerBorderType::Group)
            .with_caption("CPU");
        border.set_border_scaling(1.5);
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border,
            layout: LinearLayout::vertical().with_spacing(Spacing {
                inner_v: 0.1,
                ..Default::default()
            }),
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let cfg = self.config.borrow();
        let c = cfg.get();

        // MaxRenderThreads: range 1-32
        let mut sf = ScalarField::new(1.0, 32.0, self.look.clone());
        sf.set_caption("Max render threads");
        sf.set_value(c.max_render_threads as f64);
        sf.set_scale_mark_intervals(&[1]);
        sf.border_mut().outer = OuterBorderType::None;
        sf.border_mut().inner = InnerBorderType::InputField;
        sf.border_mut().set_border_scaling(1.5);
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        sf.on_value = Some(Box::new(move |val| {
            let threads = (val + 0.5) as i32;
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.max_render_threads = threads.clamp(1, 32));
            let _ = cm.save();
        }));
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
        let mut cb = CheckBox::new("Allow SIMD", self.look.clone());
        cb.set_checked(c.allow_simd);
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        cb.on_check = Some(Box::new(move |checked| {
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.allow_simd = checked);
            let _ = cm.save();
        }));
        ctx.create_child_with("allowSIMD", Box::new(CheckBoxPanel { check_box: cb }));
    }
}

impl PanelBehavior for CpuGroup {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.enabled, true);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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

        let aux_id = crate::layout::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.content_rect_unobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

/// Performance group — tunnel, CPU group, 2 quality scalar fields.
struct PerformanceGroup {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: Border,
    layout: RasterLayout,
}

impl PerformanceGroup {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: Border::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Graphics Performance vs. Quality"),
            layout: RasterLayout::new()
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
        let c = cfg.get();

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
        let mut ds_sf = ScalarField::new(2.0, 6.0, self.look.clone());
        ds_sf.set_caption("Downscale quality");
        ds_sf.border_mut().description =
            "Quality of image downscaling (antialiasing filter size)".to_string();
        ds_sf.set_value(c.downscale_quality as f64);
        ds_sf.set_scale_mark_intervals(&[1]);
        ds_sf.set_text_box_tallness(0.3);
        ds_sf.border_mut().set_border_scaling(1.5);
        ds_sf.set_text_of_value_fn(Box::new(downscale_text));
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        ds_sf.on_value = Some(Box::new(move |val| {
            let q = (val + 0.5) as i32;
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.downscale_quality = q.clamp(2, 6));
            let _ = cm.save();
        }));
        ctx.create_child_with(
            "downscaleQuality",
            Box::new(ScalarFieldPanel {
                scalar_field: ds_sf,
            }),
        );

        // UpscaleQuality: range 1-5
        let mut us_sf = ScalarField::new(1.0, 5.0, self.look.clone());
        us_sf.set_caption("Upscale quality");
        us_sf.border_mut().description = "Quality of image upscaling (interpolation)".to_string();
        us_sf.set_value(c.upscale_quality as f64);
        us_sf.set_scale_mark_intervals(&[1]);
        us_sf.set_text_box_tallness(0.3);
        us_sf.border_mut().set_border_scaling(1.5);
        us_sf.set_text_of_value_fn(Box::new(upscale_text));
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        us_sf.on_value = Some(Box::new(move |val| {
            let q = (val + 0.5) as i32;
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.upscale_quality = q.clamp(1, 5));
            let _ = cm.save();
        }));
        ctx.create_child_with(
            "upscaleQuality",
            Box::new(ScalarFieldPanel {
                scalar_field: us_sf,
            }),
        );
    }
}

impl PanelBehavior for PerformanceGroup {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.enabled, true);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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

        let aux_id = crate::layout::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.content_rect_unobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

/// Mouse control group — 4 factor fields + MouseMiscGroup.
struct MouseGroup {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    last_generation: u64,
    border: Border,
    layout: RasterLayout,
}

impl MouseGroup {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        let gen = generation.get();
        Self {
            config,
            look,
            generation,
            last_generation: gen,
            border: Border::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Mouse Control"),
            layout: RasterLayout::new()
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
        let c = cfg.get();

        // wheelzoom
        let mut wz = make_factor_field(
            "Mouse wheel zoom speed",
            "Speed of zooming by mouse wheel",
            self.look.clone(),
            0.25,
            4.0,
            c.mouse_wheel_zoom_speed,
            false,
        );
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        wz.scalar_field.on_value = Some(Box::new(move |val| {
            let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.mouse_wheel_zoom_speed = cfg_val);
            let _ = cm.save();
        }));
        ctx.create_child_with("wheelzoom", Box::new(wz));

        // wheelaccel — disabled (no callback)
        let wa = make_factor_field(
            "Mouse wheel zoom acceleration",
            "Acceleration of zooming by mouse wheel",
            self.look.clone(),
            0.25,
            2.0,
            c.mouse_wheel_zoom_acceleration,
            true,
        );
        ctx.create_child_with("wheelaccel", Box::new(wa));

        // zoom
        let mut zoom = make_factor_field(
            "Mouse zoom speed",
            "Speed of zooming by mouse",
            self.look.clone(),
            0.25,
            4.0,
            c.mouse_zoom_speed,
            false,
        );
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        zoom.scalar_field.on_value = Some(Box::new(move |val| {
            let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.mouse_zoom_speed = cfg_val);
            let _ = cm.save();
        }));
        ctx.create_child_with("zoom", Box::new(zoom));

        // scroll
        let mut scroll = make_factor_field(
            "Mouse scroll speed",
            "Speed of scrolling by mouse",
            self.look.clone(),
            0.25,
            4.0,
            c.mouse_scroll_speed,
            false,
        );
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        scroll.scalar_field.on_value = Some(Box::new(move |val| {
            let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
            let mut cm = config.borrow_mut();
            cm.modify(|c| c.mouse_scroll_speed = cfg_val);
            let _ = cm.save();
        }));
        ctx.create_child_with("scroll", Box::new(scroll));

        // MouseMiscGroup
        ctx.create_child_with(
            "misc",
            Box::new(MouseMiscGroup::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
            )),
        );
    }
}

impl PanelBehavior for MouseGroup {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.enabled, true);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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

        let aux_id = crate::layout::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.content_rect_unobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ---------------------------------------------------------------------------
// Top-level panels
// ---------------------------------------------------------------------------

/// Buttons panel — Reset To Defaults button.
struct ButtonsPanel {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    layout: LinearLayout,
}

impl ButtonsPanel {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        Self {
            config,
            look,
            generation,
            layout: LinearLayout::horizontal()
                .with_alignment_h(AlignmentH::Right)
                .with_alignment_v(AlignmentV::Bottom),
        }
    }

    fn create_children(&self, ctx: &mut PanelCtx) {
        let mut btn = Button::new("Reset To Defaults", self.look.clone());
        let config: Rc<RefCell<ConfigModel<CoreConfig>>> = Rc::clone(&self.config);
        let generation = Rc::clone(&self.generation);
        btn.on_click = Some(Box::new(move || {
            let mut cm = config.borrow_mut();
            cm.reset_to_default();
            let _ = cm.save();
            generation.set(generation.get() + 1);
        }));
        ctx.create_child_with("reset", Box::new(ButtonPanel { button: btn }));
    }
}

impl PanelBehavior for ButtonsPanel {
    fn paint(&mut self, _painter: &mut Painter, _w: f64, _h: f64, _state: &PanelState) {}

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
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
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    layout: RasterLayout,
}

impl ContentPanel {
    fn new(
        config: Rc<RefCell<ConfigModel<CoreConfig>>>,
        look: Rc<Look>,
        generation: Rc<Cell<u64>>,
    ) -> Self {
        Self {
            config,
            look,
            generation,
            layout: RasterLayout::new()
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
    fn paint(&mut self, _painter: &mut Painter, _w: f64, _h: f64, _state: &PanelState) {}

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
            return;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        self.layout.do_layout_skip(ctx, None, None);
    }
}

// ---------------------------------------------------------------------------
// CoreConfigPanel — top-level panel
// ---------------------------------------------------------------------------

/// Settings UI panel for emCore configuration.
///
/// Port of C++ `emCoreConfigPanel`. Provides bidirectional binding between
/// UI controls and `CoreConfig` settings via `ConfigModel`.
pub struct CoreConfigPanel {
    config: Rc<RefCell<ConfigModel<CoreConfig>>>,
    look: Rc<Look>,
    generation: Rc<Cell<u64>>,
    border: Border,
    layout: LinearLayout,
}

impl CoreConfigPanel {
    pub fn new(config: Rc<RefCell<ConfigModel<CoreConfig>>>, look: Rc<Look>) -> Self {
        let mut border = Border::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("General Preferences");
        border.description = "This panel provides general user settings.".to_string();
        Self {
            config,
            look,
            generation: Rc::new(Cell::new(0)),
            border,
            layout: LinearLayout::vertical().with_spacing(Spacing {
                margin_left: 0.01,
                margin_top: 0.1,
                margin_right: 0.01,
                margin_bottom: 0.1,
                inner_h: 0.01,
                ..Default::default()
            }),
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let content_id = ctx.create_child_with(
            "content",
            Box::new(ContentPanel::new(
                Rc::clone(&self.config),
                self.look.clone(),
                Rc::clone(&self.generation),
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

impl PanelBehavior for CoreConfigPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.enabled, true);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
            return;
        }

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let aux_id = crate::layout::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.content_rect_unobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
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
}
