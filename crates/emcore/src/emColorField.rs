use std::rc::Rc;

use crate::emColor::emColor;
use crate::emEngineCtx::{ConstructCtx, PanelCtx, WidgetCallback};
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::{emPainter, TextAlignment, VAlign};
use crate::emPanel::PanelState;
use crate::emRasterLayout::emRasterLayout;
use crate::emSignal::SignalId;
use crate::emTiling::{AlignmentH, AlignmentV, Spacing};

use super::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use super::emColorFieldFieldPanel::{ScalarFieldPanel, TextFieldPanel};
use crate::emLook::emLook;

/// Expansion child panels for color editing.
///
/// Port of C++ `emColorField::Expansion` struct. Contains scalar fields
/// for RGBA and HSV channels, plus a text field for color name/hex input.
/// Values use the C++ convention: RGBA channels are 0–10000 (mapping to
/// 0–255), hue is 0–36000 (mapping to 0–360°), sat/val are 0–10000
/// (mapping to 0.0–1.0).
pub struct Expansion {
    /// Red channel (0–10000 maps to 0–255).
    pub sf_red: i64,
    /// Green channel (0–10000 maps to 0–255).
    pub sf_green: i64,
    /// Blue channel (0–10000 maps to 0–255).
    pub sf_blue: i64,
    /// Alpha channel (0–10000 maps to 0–255).
    pub sf_alpha: i64,
    /// Hue (0–36000 maps to 0–360°).
    pub sf_hue: i64,
    /// Saturation (0–10000 maps to 0.0–1.0).
    pub sf_sat: i64,
    /// Value/brightness (0–10000 maps to 0.0–1.0).
    pub sf_val: i64,
    /// emColor name or hex string.
    pub tf_name: String,

    // Cached output values for change detection (C++ RedOut, GreenOut, etc.).
    red_out: i64,
    green_out: i64,
    blue_out: i64,
    alpha_out: i64,
    hue_out: i64,
    sat_out: i64,
    val_out: i64,
    name_out: String,
}

impl Expansion {
    fn new() -> Self {
        Self {
            sf_red: 0,
            sf_green: 0,
            sf_blue: 0,
            sf_alpha: 10000,
            sf_hue: 0,
            sf_sat: 0,
            sf_val: 0,
            tf_name: String::new(),
            red_out: 0,
            green_out: 0,
            blue_out: 0,
            alpha_out: 10000,
            hue_out: 0,
            sat_out: 0,
            val_out: 0,
            name_out: String::new(),
        }
    }
}

/// RGBA color editor widget.
pub struct emColorField {
    border: emBorder,
    look: Rc<emLook>,
    last_w: f64,
    last_h: f64,
    color: emColor,
    editable: bool,
    alpha_enabled: bool,
    expanded: bool,
    /// Expansion child data, created during auto-expand.
    /// Port of C++ `emOwnPtr<Expansion> Exp`.
    expansion: Option<Box<Expansion>>,
    pub on_color: Option<WidgetCallback<emColor>>,
    /// Allocated per C++ `emColorField::GetColorSignal()`. B3.4b: alloc only.
    pub color_signal: SignalId,
}

const SWATCH_SIZE: f64 = 20.0;

impl emColorField {
    pub fn new<C: ConstructCtx>(ctx: &mut C, look: Rc<emLook>) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::Instrument)
                .with_inner(InnerBorderType::OutputField)
                .with_how_to(true),
            look,
            last_w: 0.0,
            last_h: 0.0,
            color: emColor::BLACK,
            editable: false,
            alpha_enabled: false,
            expanded: false,
            expansion: None,
            on_color: None,
            color_signal: ctx.create_signal(),
        }
    }

    pub fn SetCaption(&mut self, caption: &str) {
        self.border.caption = caption.to_string();
    }

    pub fn SetDescription(&mut self, desc: &str) {
        self.border.description = desc.to_string();
    }

    pub fn GetColor(&self) -> emColor {
        self.color
    }

    /// Construction-time color assignment — no signal, no callback. Used by
    /// test harnesses and constructor-like setup paths where no scheduler
    /// reach exists. C++ parity: ctor sets color directly.
    pub fn set_initial_color(&mut self, color: emColor) {
        if self.color != color {
            self.color = color;
            if self.expansion.is_some() {
                self.UpdateRGBAOutput();
                self.UpdateHSVOutput(false);
                self.UpdateNameOutput();
            }
        }
    }

    /// Mirrors C++ `emColorField::SetColor` (emColorField.cpp): updates color,
    /// syncs sub-widget displays, fires ColorSignal + on_color callback.
    pub fn SetColor(&mut self, color: emColor, ctx: &mut PanelCtx<'_>) {
        if self.color != color {
            self.color = color;
            // Sync expansion if present.
            if self.expansion.is_some() {
                self.UpdateRGBAOutput();
                self.UpdateHSVOutput(false);
                self.UpdateNameOutput();
            }
            if let Some(mut sched) = ctx.as_sched_ctx() {
                sched.fire(self.color_signal);
                if let Some(cb) = self.on_color.as_mut() {
                    cb(self.color, &mut sched);
                }
            }
        }
    }

    pub fn IsEditable(&self) -> bool {
        self.editable
    }

    pub fn SetEditable(&mut self, editable: bool) {
        if self.editable != editable {
            self.editable = editable;
            if editable {
                if self.border.inner == InnerBorderType::OutputField {
                    self.border.inner = InnerBorderType::InputField;
                }
            } else if self.border.inner == InnerBorderType::InputField {
                self.border.inner = InnerBorderType::OutputField;
            }
        }
    }

    pub fn IsAlphaEnabled(&self) -> bool {
        self.alpha_enabled
    }

    /// Construction-time variant — toggles alpha without firing signals.
    pub fn set_initial_alpha_enabled(&mut self, alpha_enabled: bool) {
        if self.alpha_enabled != alpha_enabled {
            self.alpha_enabled = alpha_enabled;
            if !alpha_enabled && self.color.GetAlpha() != 255 {
                self.color = self.color.SetAlpha(255);
            }
        }
    }

    pub fn SetAlphaEnabled(&mut self, alpha_enabled: bool, ctx: &mut PanelCtx<'_>) {
        if self.alpha_enabled != alpha_enabled {
            self.alpha_enabled = alpha_enabled;
            if !alpha_enabled && self.color.GetAlpha() != 255 {
                self.color = self.color.SetAlpha(255);
                if let Some(mut sched) = ctx.as_sched_ctx() {
                    sched.fire(self.color_signal);
                    if let Some(cb) = self.on_color.as_mut() {
                        cb(self.color, &mut sched);
                    }
                }
            }
        }
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        if expanded && !self.expanded {
            self.auto_expand();
        } else if !expanded && self.expanded {
            self.auto_shrink();
        }
        self.expanded = expanded;
    }

    /// Get the expansion data, if currently expanded.
    pub fn expansion(&self) -> Option<&Expansion> {
        self.expansion.as_deref()
    }

    /// Get mutable expansion data, if currently expanded.
    pub fn expansion_mut(&mut self) -> Option<&mut Expansion> {
        self.expansion.as_deref_mut()
    }

    /// Create expansion child data.
    /// Port of C++ `emColorField::AutoExpand()`.
    fn auto_expand(&mut self) {
        let mut exp = Box::new(Expansion::new());

        // Initialize from current color.
        let c = self.color;
        exp.red_out = (c.GetRed() as i64 * 10000 + 127) / 255;
        exp.sf_red = exp.red_out;
        exp.green_out = (c.GetGreen() as i64 * 10000 + 127) / 255;
        exp.sf_green = exp.green_out;
        exp.blue_out = (c.GetBlue() as i64 * 10000 + 127) / 255;
        exp.sf_blue = exp.blue_out;
        exp.alpha_out = (c.GetAlpha() as i64 * 10000 + 127) / 255;
        exp.sf_alpha = exp.alpha_out;

        let (h, s, v) = c.GetHSV();
        exp.hue_out = (h * 100.0 + 0.5) as i64;
        exp.sf_hue = exp.hue_out;
        // GetHSV now returns s/v in [0,100] matching C++. Scale by 100 for [0,10000] range.
        exp.sat_out = (s * 100.0 + 0.5) as i64;
        exp.sf_sat = exp.sat_out;
        exp.val_out = (v * 100.0 + 0.5) as i64;
        exp.sf_val = exp.val_out;

        exp.name_out = c.to_string();
        exp.tf_name = exp.name_out.clone();

        self.expansion = Some(exp);
    }

    /// Destroy expansion child data.
    /// Port of C++ `emColorField::AutoShrink()`.
    fn auto_shrink(&mut self) {
        self.expansion = None;
    }

    /// Poll expansion children for value changes and synchronize.
    /// Port of C++ `emColorField::Cycle()`.
    ///
    /// Returns `true` if the color changed.
    pub fn Cycle(&mut self, ctx: &mut PanelCtx<'_>) -> bool {
        let exp = match &mut self.expansion {
            Some(exp) => exp,
            None => return false,
        };

        let rgba_changed = exp.sf_red != exp.red_out
            || exp.sf_green != exp.green_out
            || exp.sf_blue != exp.blue_out
            || exp.sf_alpha != exp.alpha_out;

        let hsv_changed =
            exp.sf_hue != exp.hue_out || exp.sf_sat != exp.sat_out || exp.sf_val != exp.val_out;

        let text_changed = exp.tf_name != exp.name_out;

        if !rgba_changed && !hsv_changed && !text_changed {
            return false;
        }

        // C++ Cycle() checks each signal independently (not if/else-if).
        // Each change is applied; last one wins if multiple change simultaneously.
        if rgba_changed {
            let r = ((exp.sf_red * 255 + 5000) / 10000).clamp(0, 255) as u8;
            let g = ((exp.sf_green * 255 + 5000) / 10000).clamp(0, 255) as u8;
            let b = ((exp.sf_blue * 255 + 5000) / 10000).clamp(0, 255) as u8;
            let a = ((exp.sf_alpha * 255 + 5000) / 10000).clamp(0, 255) as u8;
            self.color = emColor::rgba(r, g, b, a);
        }
        if hsv_changed {
            let h = exp.sf_hue as f32 / 100.0;
            let s = (exp.sf_sat as f32 / 100.0).clamp(0.0, 100.0);
            let v = (exp.sf_val as f32 / 100.0).clamp(0.0, 100.0);
            self.color = emColor::SetHSVA(h, s, v).SetAlpha(self.color.GetAlpha());
        }
        if text_changed {
            if let Some(parsed) = emColor::TryParse(&exp.tf_name) {
                self.color = parsed;
            }
        }

        // Synchronize sibling fields.
        if hsv_changed || text_changed {
            self.UpdateRGBAOutput();
        }
        if rgba_changed || text_changed {
            self.UpdateHSVOutput(false);
        }
        if rgba_changed || hsv_changed {
            self.UpdateNameOutput();
        }

        // C++ emColorField::Cycle (emColorField.cpp:205-207):
        // on any recomputed color change → Signal(ColorSignal) → ColorChanged.
        if let Some(mut sched) = ctx.as_sched_ctx() {
            sched.fire(self.color_signal);
            if let Some(cb) = self.on_color.as_mut() {
                cb(self.color, &mut sched);
            }
        }

        true
    }

    /// Read current values from child emScalarField/emTextField panels in the tree
    /// and update the Expansion's `sf_*` / `tf_name` fields.
    ///
    /// This mirrors the C++ `emColorField::Cycle()` pattern where each child
    /// widget's current value is polled via `GetValue()` / `GetText()`. Call
    /// this before `cycle()` so that changes made by user interaction on the
    /// child panels are visible to the change-detection logic.
    pub fn sync_from_children(&mut self, ctx: &mut PanelCtx) {
        if self.expansion.is_none() {
            return;
        }

        // The tree structure is: self -> emRasterLayout -> {r, g, b, a, h, s, v, n}.
        let children: Vec<_> = ctx.tree.children(ctx.id).collect();
        if children.is_empty() {
            return;
        }
        let layout_id = children[0];
        let grandchildren: Vec<_> = ctx.tree.children(layout_id).collect();

        // Map child names to their emScalarField values or emTextField text.
        for &child_id in &grandchildren {
            let name = ctx.tree.name(child_id).unwrap_or("").to_string();
            if let Some(behavior) = ctx.tree.take_behavior(child_id) {
                if let Some(sfp) = behavior.as_any().downcast_ref::<ScalarFieldPanel>() {
                    let val = sfp.scalar_field.GetValue() as i64;
                    let exp = self.expansion.as_mut().unwrap();
                    match name.as_str() {
                        "r" => exp.sf_red = val,
                        "g" => exp.sf_green = val,
                        "b" => exp.sf_blue = val,
                        "a" => exp.sf_alpha = val,
                        "h" => exp.sf_hue = val,
                        "s" => exp.sf_sat = val,
                        "v" => exp.sf_val = val,
                        _ => {}
                    }
                } else if let Some(tfp) = behavior.as_any().downcast_ref::<TextFieldPanel>() {
                    let exp = self.expansion.as_mut().unwrap();
                    if name == "n" {
                        exp.tf_name = tfp.text_field.GetText().to_string();
                    }
                }
                ctx.tree.put_behavior(child_id, behavior);
            }
        }
    }

    /// Sync RGBA scalar fields from current color.
    /// Port of C++ `emColorField::UpdateRGBAOutput()`.
    pub fn UpdateRGBAOutput(&mut self) {
        let exp = match &mut self.expansion {
            Some(exp) => exp,
            None => return,
        };
        let c = self.color;
        exp.red_out = (c.GetRed() as i64 * 10000 + 127) / 255;
        exp.sf_red = exp.red_out;
        exp.green_out = (c.GetGreen() as i64 * 10000 + 127) / 255;
        exp.sf_green = exp.green_out;
        exp.blue_out = (c.GetBlue() as i64 * 10000 + 127) / 255;
        exp.sf_blue = exp.blue_out;
        exp.alpha_out = (c.GetAlpha() as i64 * 10000 + 127) / 255;
        exp.sf_alpha = exp.alpha_out;
    }

    /// Sync HSV scalar fields from current color.
    /// Port of C++ `emColorField::UpdateHSVOutput(bool initial)`.
    ///
    /// When `initial` is false, hue is only updated if saturation > 0 and
    /// value > 0, and saturation is only updated if value > 0. This prevents
    /// hue/sat from jumping to 0 when the color is black.
    pub fn UpdateHSVOutput(&mut self, initial: bool) {
        let exp = match &mut self.expansion {
            Some(exp) => exp,
            None => return,
        };
        let (h, s, v) = self.color.GetHSV();
        if v > 0.0 || initial {
            if s > 0.0 || initial {
                exp.hue_out = (h * 100.0 + 0.5) as i64;
                exp.sf_hue = exp.hue_out;
            }
            exp.sat_out = (s * 100.0 + 0.5) as i64;
            exp.sf_sat = exp.sat_out;
        }
        exp.val_out = (v * 100.0 + 0.5) as i64;
        exp.sf_val = exp.val_out;
    }

    /// Sync name/hex text field from current color.
    /// Port of C++ `emColorField::UpdateNameOutput()`.
    pub fn UpdateNameOutput(&mut self) {
        let exp = match &mut self.expansion {
            Some(exp) => exp,
            None => return,
        };
        exp.name_out = self.color.to_string();
        exp.tf_name = exp.name_out.clone();
    }

    /// Paint using C++ emColorField::PaintContent (emColorField.cpp:371-404).
    ///
    /// Gets content round rect, insets by d=min(w,h)*0.1, paints color rect + outline.
    pub fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, pixel_scale: f64) {
        self.last_w = w;
        self.last_h = h;
        self.border.how_to_text = self.GetHowTo(true, true);
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
        let mut canvas_color = painter.GetCanvasColor();

        // C++ PaintContent — emColorField.cpp:371-404
        let (cr, _r) = self.border.GetContentRoundRect(w, h, &self.look);
        let x = cr.x;
        let y = cr.y;
        let cw = cr.w;
        let ch = cr.h;
        let d = cw.min(ch) * 0.1;

        if !self.color.IsOpaque() {
            painter.PaintTextBoxed(
                x + d,
                y + d,
                cw - 2.0 * d,
                ch - 2.0 * d,
                "transparent",
                ch,
                if self.editable {
                    self.look.input_fg_color
                } else {
                    self.look.output_fg_color
                },
                canvas_color,
                TextAlignment::Center,
                VAlign::Center,
                TextAlignment::Center,
                0.5,
                true,
                0.0,
            );
            canvas_color = emColor::TRANSPARENT;
        }
        painter.PaintRect(
            x + d,
            y + d,
            cw - 2.0 * d,
            ch - 2.0 * d,
            self.color,
            canvas_color,
        );
        {
            let stroke = crate::emStroke::emStroke::new(self.look.input_fg_color, d * 0.08);
            painter.PaintRectOutline(
                x + d,
                y + d,
                cw - 2.0 * d,
                ch - 2.0 * d,
                &stroke,
                emColor::TRANSPARENT,
            );
        }

        self.border.paint_inner_overlay(painter, w, h, &self.look);
    }

    fn hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let (rect, r) = self.border.GetContentRoundRect(1.0, tallness, &self.look);
        // RUST_ONLY: (language-forced-utility) widget_utils.rs -- C++ inlines this formula per widget
        let dx = ((rect.x - mx).max(mx - rect.x - rect.w) + r).max(0.0);
        let dy = ((rect.y - my).max(my - rect.y - rect.h) + r).max(0.0);
        dx * dx + dy * dy <= r * r
    }

    pub fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        match event.key {
            InputKey::MouseLeft if event.variant == InputVariant::Release => {
                if !self.hit_test(event.mouse_x, event.mouse_y) {
                    return false;
                }
                self.set_expanded(!self.expanded);
                true
            }
            _ => false,
        }
    }

    /// Create expansion child panels matching C++ `emColorField::AutoExpand()`.
    ///
    /// Creates a `emRasterLayout` child ("emColorField::InnerStuff") with
    /// `fixed_columns=2`, `preferred_child_tallness=0.2`, `alignment=End`,
    /// and `spacing=(0.08, 0.2, 0.04, 0.1)`. Under it, 7 emScalarField panels
    /// (r, g, b, a, h, s, v) and 1 emTextField panel (n).
    pub fn create_expansion_children(&mut self, ctx: &mut PanelCtx) {
        if !self.expanded {
            self.auto_expand();
            self.expanded = true;
        }
        let exp = self.expansion.as_ref().expect("expansion must exist");

        // Create the emRasterLayout child panel.
        let mut layout = emRasterLayout::new();
        layout.fixed_columns = Some(2);
        layout.preferred_child_tallness = 0.2;
        // C++ SetChildTallness(0.2) sets PrefCT=MinCT=MaxCT=0.2, locking cell
        // aspect ratio.  Without clamping, cells fill the available height
        // instead of being compact with centered vertical surplus.
        layout.min_child_tallness = 0.2;
        layout.max_child_tallness = 0.2;
        // C++ EM_ALIGN_RIGHT = right horizontally, center vertically
        layout.alignment_h = AlignmentH::Right;
        layout.alignment_v = AlignmentV::Center;
        // C++ SetSpace(0.08, 0.2, 0.04, 0.1) = SetSpace(lr, tb, h, v)
        // = (l=0.08, t=0.2, h=0.04, v=0.1, r=0.08, b=0.2)
        layout.spacing = Spacing {
            margin_left: 0.08,
            margin_top: 0.2,
            margin_right: 0.08,
            margin_bottom: 0.2,
            inner_h: 0.04,
            inner_v: 0.1,
        };
        let layout_id = ctx.create_child_with("emColorField::InnerStuff", Box::new(layout));

        // C++ UpdateExpAppearance: create a modified look where bg_color and
        // fg_color are swapped to input/output variants based on editability,
        // then propagate to all children via SetLook(look, true).
        let child_look = {
            let mut l = (*self.look).clone();
            if self.editable {
                l.bg_color = l.input_bg_color;
                l.fg_color = l.input_fg_color;
            } else {
                l.bg_color = l.output_bg_color;
                l.fg_color = l.output_fg_color;
            }
            Rc::new(l)
        };
        let editable = self.editable;

        // C++ emColorField sets scale mark intervals and percent-value
        // formatters on each emScalarField child (emColorField.cpp:234-309).
        let pct_intervals: &[u64] = &[2500, 500, 100];
        let hue_intervals: &[u64] = &[6000, 1500, 500, 100];

        // Helper: create a percent-valued emScalarField child.
        let create_pct_sf = |ctx: &mut PanelCtx<'_>,
                             parent: crate::emPanelTree::PanelId,
                             name: &str,
                             caption: &str,
                             value: i64| {
            let child = ctx
                .tree
                .create_child(parent, name, ctx.scheduler.as_deref_mut());
            let mut panel = ScalarFieldPanel::new(
                ctx,
                caption,
                0.0,
                10000.0,
                value as f64,
                child_look.clone(),
                editable,
            );
            panel.scalar_field.SetScaleMarkIntervals(pct_intervals);
            panel
                .scalar_field
                .SetTextOfValueFunc(Box::new(|val, _iv| format!("{}%", val as f64 / 100.0)));
            ctx.tree.set_behavior(child, Box::new(panel));
            child
        };

        create_pct_sf(ctx, layout_id, "r", "Red", exp.sf_red);
        create_pct_sf(ctx, layout_id, "g", "Green", exp.sf_green);
        create_pct_sf(ctx, layout_id, "b", "Blue", exp.sf_blue);
        // Alpha field: C++ has description "The lower the more transparent."
        let alpha_id = {
            let child = ctx
                .tree
                .create_child(layout_id, "a", ctx.scheduler.as_deref_mut());
            let mut panel = ScalarFieldPanel::new(
                ctx,
                "Alpha",
                0.0,
                10000.0,
                exp.sf_alpha as f64,
                child_look.clone(),
                editable,
            );
            panel.scalar_field.SetScaleMarkIntervals(pct_intervals);
            panel
                .scalar_field
                .SetTextOfValueFunc(Box::new(|val, _iv| format!("{}%", val as f64 / 100.0)));
            panel.scalar_field.border_mut().description =
                "The lower the more transparent.".to_string();
            ctx.tree.set_behavior(child, Box::new(panel));
            child
        };

        // C++ UpdateExpAppearance: SfAlpha->SetEnableSwitch(AlphaEnabled)
        if !self.alpha_enabled {
            ctx.tree
                .SetEnableSwitch(alpha_id, false, ctx.scheduler.as_deref_mut());
        }

        // Hue field: different intervals, text formatter, and tallness.
        {
            let child = ctx
                .tree
                .create_child(layout_id, "h", ctx.scheduler.as_deref_mut());
            let mut panel = ScalarFieldPanel::new(
                ctx,
                "Hue",
                0.0,
                36000.0,
                exp.sf_hue as f64,
                child_look.clone(),
                editable,
            );
            panel.scalar_field.SetScaleMarkIntervals(hue_intervals);
            panel.scalar_field.SetTextBoxTallness(0.35);
            panel.scalar_field.SetTextOfValueFunc(Box::new(|val, iv| {
                if iv >= 6000 {
                    // C++ TextOfHueValue: major marks show color names
                    match (val / 6000) % 6 {
                        0 => "Red".to_string(),
                        1 => "Yellow".to_string(),
                        2 => "Green".to_string(),
                        3 => "Cyan".to_string(),
                        4 => "Blue".to_string(),
                        5 => "Magenta".to_string(),
                        _ => format!("{}", val as f64 / 100.0),
                    }
                } else {
                    format!("{}", val as f64 / 100.0)
                }
            }));
            ctx.tree.set_behavior(child, Box::new(panel));
        }

        create_pct_sf(ctx, layout_id, "s", "Saturation", exp.sf_sat);
        create_pct_sf(ctx, layout_id, "v", "Value (brightness)", exp.sf_val);

        // emTextField child for color name/hex.
        // C++ description: "Here you can enter a color name like 'powder blue',\n
        //                    or a hexadecimal RGB value like '#c88' or '#73c81D'."
        let tf_child = ctx
            .tree
            .create_child(layout_id, "n", ctx.scheduler.as_deref_mut());
        let mut tf_panel = TextFieldPanel::new(ctx, "Name", &exp.tf_name, child_look, editable);
        tf_panel.text_field.border_mut().description =
            "Here you can enter a color name like 'powder blue',\n\
             or a hexadecimal RGB value like '#c88' or '#73c81D'."
                .to_string();
        ctx.tree.set_behavior(tf_child, Box::new(tf_panel));
    }

    /// Layout children matching C++ `emColorField::LayoutChildren()`.
    ///
    /// Positions the emRasterLayout child in the right half of the content rect,
    /// inset by `d = min(w,h) * 0.05`.
    ///
    /// Children are created by `create_expansion_children` which must be called
    /// from `AutoExpand()` on the PanelBehavior wrapper, not here.
    pub fn LayoutChildren(&mut self, ctx: &mut PanelCtx, w: f64, h: f64) {
        let children = ctx.children();
        if children.is_empty() || !self.expanded {
            for &child in &children {
                ctx.layout_child(child, 0.0, 0.0, 0.0, 0.0);
            }
            return;
        }

        // C++ GetContentRectUnobscured then inset by d
        let cr = self.border.GetContentRectUnobscured(w, h, &self.look);
        let d = cr.w.min(cr.h) * 0.05;
        let x = cr.x + d;
        let y = cr.y + d;
        let cw = (cr.w - 2.0 * d).max(0.0);
        let ch = (cr.h - 2.0 * d).max(0.0);

        // Position the emRasterLayout child in the right half.
        // C++ Exp->Layout->Layout(x+w*0.5,y,w*0.5,h) — no canvasColor arg,
        // so child gets default canvasColor=0 (transparent).
        // C++ emBorder::LayoutChildren() only touches the Aux panel (none here),
        // so no child gets canvas_color propagated.
        let layout_id = children[0];
        ctx.layout_child(layout_id, x + cw * 0.5, y, cw * 0.5, ch);
    }

    /// Whether this color field provides how-to help text.
    /// Matches C++ `emColorField::HasHowTo` (always true).
    pub fn HasHowTo(&self) -> bool {
        true
    }

    /// Help text describing how to use this color field.
    ///
    /// Chains the border's base how-to with color-field-specific sections.
    /// Matches C++ `emColorField::GetHowTo`.
    pub fn GetHowTo(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.GetHowTo(enabled, focusable);
        text.push_str(HOWTO_COLOR_FIELD);
        if !self.editable {
            text.push_str(HOWTO_READ_ONLY);
        }
        text
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        if self.expanded {
            self.border
                .preferred_size_for_content(SWATCH_SIZE, SWATCH_SIZE + 4.0 * 18.0)
        } else {
            self.border
                .preferred_size_for_content(SWATCH_SIZE, SWATCH_SIZE)
        }
    }
}

/// C++ `emColorField::HowToColorField`.
const HOWTO_COLOR_FIELD: &str = "\n\n\
    COLOR FIELD\n\n\
    This panel is for viewing and editing a color. For editing, refer to the inner\n\
    fields.\n";

/// C++ `emColorField::HowToReadOnly`.
const HOWTO_READ_ONLY: &str = "\n\n\
    READ-ONLY\n\n\
    This color field is read-only. You cannot edit the color.\n";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngineCtx::{DeferredAction, InitCtx};
    use crate::emPanelTree::{PanelId, PanelTree};
    use crate::emScheduler::EngineScheduler;

    fn scratch_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let id = tree.create_root("t", false);
        (tree, id)
    }

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

    #[test]
    fn toggle_expanded() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        assert!(!cf.is_expanded());

        // Use programmatic toggle since mouse needs paint for hit test
        cf.set_expanded(true);
        assert!(cf.is_expanded());

        cf.set_expanded(false);
        assert!(!cf.is_expanded());
    }

    #[test]
    fn set_and_get_color() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        cf.set_initial_color(emColor::RED);
        assert_eq!(cf.GetColor(), emColor::RED);
    }

    #[test]
    fn expansion_created_on_expand() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        assert!(cf.expansion().is_none());
        cf.set_expanded(true);
        assert!(cf.expansion().is_some());
    }

    #[test]
    fn expansion_destroyed_on_shrink() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        cf.set_expanded(true);
        cf.set_expanded(false);
        assert!(cf.expansion().is_none());
    }

    #[test]
    fn expansion_rgba_values_match_color() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        cf.set_initial_color(emColor::rgba(100, 150, 200, 255));
        cf.set_expanded(true);
        let exp = cf.expansion().expect("expanded");
        // r=100 → (100 * 10000 + 127) / 255 = 3922
        assert_eq!(exp.sf_red, (100i64 * 10000 + 127) / 255);
        assert_eq!(exp.sf_green, (150i64 * 10000 + 127) / 255);
        assert_eq!(exp.sf_blue, (200i64 * 10000 + 127) / 255);
        assert_eq!(exp.sf_alpha, (255i64 * 10000 + 127) / 255);
    }

    #[test]
    fn cycle_rgba_change() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        cf.set_initial_color(emColor::BLACK);
        cf.set_expanded(true);
        // Modify red via expansion
        cf.expansion_mut().unwrap().sf_red = 5000; // ~50% = 127
        let (mut tree, tid) = scratch_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        assert!(cf.Cycle(&mut ctx));
        // Fire path is exercised by `color_field_fires_color_signal_on_cycle`
        // below; scratch ctx here has no scheduler reach so the signal is not
        // actually fired. The boolean return value is the observable under test.
        // emColor should have updated red channel
        let r = cf.GetColor().GetRed();
        assert!((r as i64 - 127).abs() <= 1, "expected ~127, got {}", r);
    }

    #[test]
    fn cycle_hsv_change() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        cf.set_initial_color(emColor::BLACK);
        cf.set_expanded(true);
        // Set via HSV: hue=0 (red), sat=100%, val=100%
        let exp = cf.expansion_mut().unwrap();
        exp.sf_hue = 0;
        exp.sf_sat = 10000;
        exp.sf_val = 10000;
        let (mut tree, tid) = scratch_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        assert!(cf.Cycle(&mut ctx));
        // Should be red
        assert_eq!(cf.GetColor().GetRed(), 255);
        assert!(cf.GetColor().GetGreen() < 5);
        assert!(cf.GetColor().GetBlue() < 5);
    }

    #[test]
    fn cycle_text_change() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        cf.set_expanded(true);
        cf.expansion_mut().unwrap().tf_name = "#FF0000".to_string();
        let (mut tree, tid) = scratch_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        assert!(cf.Cycle(&mut ctx));
        assert_eq!(cf.GetColor(), emColor::rgba(255, 0, 0, 255));
    }

    #[test]
    fn update_name_output_hex_format() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        cf.set_initial_color(emColor::rgba(0xAB, 0xCD, 0xEF, 0xFF));
        cf.set_expanded(true);
        let exp = cf.expansion().unwrap();
        assert_eq!(exp.tf_name, "#ABCDEF");
    }

    #[test]
    fn color_field_fires_color_signal_on_cycle() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        cf.set_initial_color(emColor::BLACK);
        cf.set_expanded(true);
        cf.expansion_mut().unwrap().sf_red = 5000;
        let sig = cf.color_signal;
        let (mut tree, tid) = scratch_tree();
        let fw_cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
                &__init.pa,
            );
            assert!(cf.Cycle(&mut ctx));
        }
        assert!(__init.sched.is_pending(sig));
    }

    #[test]
    fn update_hsv_preserves_hue_at_black() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut cf = emColorField::new(&mut __init.ctx(), look);
        cf.set_initial_color(emColor::rgba(255, 0, 0, 255)); // Red
        cf.set_expanded(true);
        let hue_before = cf.expansion().unwrap().sf_hue;
        // Now set to black via RGBA
        cf.set_initial_color(emColor::BLACK);
        // Hue should be preserved (not reset to 0) because v=0
        let hue_after = cf.expansion().unwrap().sf_hue;
        assert_eq!(hue_before, hue_after);
    }
}
