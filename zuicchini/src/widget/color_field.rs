use std::rc::Rc;

use crate::foundation::Color;
use crate::input::{InputEvent, InputKey, InputVariant};
use crate::layout::raster::RasterLayout;
use crate::layout::{AlignmentH, AlignmentV, Spacing};
use crate::panel::PanelCtx;
use crate::render::Painter;

use super::border::{Border, InnerBorderType, OuterBorderType};
use super::field_panel::{ScalarFieldPanel, TextFieldPanel};
use super::look::Look;

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
    /// Color name or hex string.
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
pub struct ColorField {
    border: Border,
    look: Rc<Look>,
    color: Color,
    editable: bool,
    alpha_enabled: bool,
    expanded: bool,
    /// Expansion child data, created during auto-expand.
    /// Port of C++ `emOwnPtr<Expansion> Exp`.
    expansion: Option<Box<Expansion>>,
    pub on_color: Option<Box<dyn FnMut(Color)>>,
}

const SWATCH_SIZE: f64 = 20.0;

impl ColorField {
    pub fn new(look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::Instrument)
                .with_inner(InnerBorderType::OutputField)
                .with_how_to(true),
            look,
            color: Color::BLACK,
            editable: false,
            alpha_enabled: false,
            expanded: false,
            expansion: None,
            on_color: None,
        }
    }

    pub fn set_caption(&mut self, caption: &str) {
        self.border.caption = caption.to_string();
    }

    pub fn color(&self) -> Color {
        self.color
    }

    pub fn set_color(&mut self, color: Color) {
        if self.color != color {
            self.color = color;
            // Sync expansion if present.
            if self.expansion.is_some() {
                self.update_rgba_output();
                self.update_hsv_output(false);
                self.update_name_output();
            }
            if let Some(cb) = &mut self.on_color {
                cb(color);
            }
        }
    }

    pub fn is_editable(&self) -> bool {
        self.editable
    }

    pub fn set_editable(&mut self, editable: bool) {
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

    pub fn is_alpha_enabled(&self) -> bool {
        self.alpha_enabled
    }

    pub fn set_alpha_enabled(&mut self, alpha_enabled: bool) {
        if self.alpha_enabled != alpha_enabled {
            self.alpha_enabled = alpha_enabled;
            if !alpha_enabled && self.color.a() != 255 {
                self.color = self.color.with_alpha(255);
                if let Some(cb) = &mut self.on_color {
                    cb(self.color);
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
        exp.red_out = (c.r() as i64 * 10000 + 127) / 255;
        exp.sf_red = exp.red_out;
        exp.green_out = (c.g() as i64 * 10000 + 127) / 255;
        exp.sf_green = exp.green_out;
        exp.blue_out = (c.b() as i64 * 10000 + 127) / 255;
        exp.sf_blue = exp.blue_out;
        exp.alpha_out = (c.a() as i64 * 10000 + 127) / 255;
        exp.sf_alpha = exp.alpha_out;

        let (h, s, v) = c.to_hsv();
        exp.hue_out = (h * 100.0 + 0.5) as i64;
        exp.sf_hue = exp.hue_out;
        // C++ GetSat/GetVal return [0,100]; Rust to_hsv returns [0,1].
        // Scale by 10000 to match C++ range [0,10000].
        exp.sat_out = (s * 10000.0 + 0.5) as i64;
        exp.sf_sat = exp.sat_out;
        exp.val_out = (v * 10000.0 + 0.5) as i64;
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
    pub fn cycle(&mut self) -> bool {
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

        // Apply changes to color.
        if rgba_changed {
            let r = ((exp.sf_red * 255 + 5000) / 10000).clamp(0, 255) as u8;
            let g = ((exp.sf_green * 255 + 5000) / 10000).clamp(0, 255) as u8;
            let b = ((exp.sf_blue * 255 + 5000) / 10000).clamp(0, 255) as u8;
            let a = ((exp.sf_alpha * 255 + 5000) / 10000).clamp(0, 255) as u8;
            self.color = Color::rgba(r, g, b, a);
        } else if hsv_changed {
            let h = exp.sf_hue as f32 / 100.0; // [0, 36000] → [0, 360)
            let s = (exp.sf_sat as f32 / 10000.0).clamp(0.0, 1.0); // [0, 10000] → [0, 1]
            let v = (exp.sf_val as f32 / 10000.0).clamp(0.0, 1.0); // [0, 10000] → [0, 1]
            self.color = Color::from_hsv(h, s, v).with_alpha(self.color.a());
        } else if text_changed {
            if let Ok(parsed) = exp.tf_name.parse::<Color>() {
                self.color = parsed;
            }
        }

        // Synchronize sibling fields.
        if hsv_changed || text_changed {
            self.update_rgba_output();
        }
        if rgba_changed || text_changed {
            self.update_hsv_output(false);
        }
        if rgba_changed || hsv_changed {
            self.update_name_output();
        }

        if let Some(cb) = &mut self.on_color {
            cb(self.color);
        }

        true
    }

    /// Sync RGBA scalar fields from current color.
    /// Port of C++ `emColorField::UpdateRGBAOutput()`.
    pub fn update_rgba_output(&mut self) {
        let exp = match &mut self.expansion {
            Some(exp) => exp,
            None => return,
        };
        let c = self.color;
        exp.red_out = (c.r() as i64 * 10000 + 127) / 255;
        exp.sf_red = exp.red_out;
        exp.green_out = (c.g() as i64 * 10000 + 127) / 255;
        exp.sf_green = exp.green_out;
        exp.blue_out = (c.b() as i64 * 10000 + 127) / 255;
        exp.sf_blue = exp.blue_out;
        exp.alpha_out = (c.a() as i64 * 10000 + 127) / 255;
        exp.sf_alpha = exp.alpha_out;
    }

    /// Sync HSV scalar fields from current color.
    /// Port of C++ `emColorField::UpdateHSVOutput(bool initial)`.
    ///
    /// When `initial` is false, hue is only updated if saturation > 0 and
    /// value > 0, and saturation is only updated if value > 0. This prevents
    /// hue/sat from jumping to 0 when the color is black.
    pub fn update_hsv_output(&mut self, initial: bool) {
        let exp = match &mut self.expansion {
            Some(exp) => exp,
            None => return,
        };
        let (h, s, v) = self.color.to_hsv();
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
    pub fn update_name_output(&mut self) {
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
    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true);

        // C++ PaintContent: GetContentRoundRect, then inset by d.
        let (cr, _r) = self.border.content_round_rect(w, h, &self.look);
        let d = cr.w.min(cr.h) * 0.1;

        let rx = cr.x + d;
        let ry = cr.y + d;
        let rw = (cr.w - 2.0 * d).max(0.0);
        let rh = (cr.h - 2.0 * d).max(0.0);

        // Paint color rect.
        painter.paint_rect(rx, ry, rw, rh, self.color);

        // Paint rect outline (C++ PaintRectOutline with d*0.08 thickness).
        // C++ uses a 10-vertex polygon (outer + inner rect) centered on the rect edges.
        let thickness = d * 0.08;
        if thickness > 0.0 {
            let outline_stroke = crate::render::Stroke::new(self.look.input_fg_color, thickness);
            painter.paint_rect_outlined(rx, ry, rw, rh, &outline_stroke);
        }

        // C++ paints content, THEN overlays the IO field border image.
        self.border.paint_inner_overlay(painter, w, h, &self.look);
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        match event.key {
            InputKey::MouseLeft if event.variant == InputVariant::Release => {
                self.set_expanded(!self.expanded);
                true
            }
            _ => false,
        }
    }

    /// Create expansion child panels matching C++ `emColorField::AutoExpand()`.
    ///
    /// Creates a `RasterLayout` child ("emColorField::InnerStuff") with
    /// `fixed_columns=2`, `preferred_child_tallness=0.2`, `alignment=End`,
    /// and `spacing=(0.08, 0.2, 0.04, 0.1)`. Under it, 7 ScalarField panels
    /// (r, g, b, a, h, s, v) and 1 TextField panel (n).
    pub fn create_expansion_children(&mut self, ctx: &mut PanelCtx) {
        if !self.expanded {
            self.auto_expand();
            self.expanded = true;
        }
        let exp = self.expansion.as_ref().expect("expansion must exist");

        // Create the RasterLayout child panel.
        let mut layout = RasterLayout::new();
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

        let look = self.look.clone();

        // C++ emColorField sets scale mark intervals and percent-value
        // formatters on each ScalarField child (emColorField.cpp:234-309).
        let pct_intervals: &[u64] = &[2500, 500, 100];
        let hue_intervals: &[u64] = &[6000, 1500, 500, 100];

        // Helper: create a percent-valued ScalarField child.
        let create_pct_sf = |tree: &mut crate::panel::PanelTree,
                             parent: crate::panel::PanelId,
                             name: &str,
                             caption: &str,
                             value: i64| {
            let child = tree.create_child(parent, name);
            let mut panel =
                ScalarFieldPanel::new(caption, 0.0, 10000.0, value as f64, look.clone());
            panel.scalar_field.set_scale_mark_intervals(pct_intervals);
            panel
                .scalar_field
                .set_text_of_value_fn(Box::new(|val, _iv| format!("{}%", val as f64 / 100.0)));
            tree.set_behavior(child, Box::new(panel));
            child
        };

        create_pct_sf(ctx.tree, layout_id, "r", "Red", exp.sf_red);
        create_pct_sf(ctx.tree, layout_id, "g", "Green", exp.sf_green);
        create_pct_sf(ctx.tree, layout_id, "b", "Blue", exp.sf_blue);
        create_pct_sf(ctx.tree, layout_id, "a", "Alpha", exp.sf_alpha);

        // Hue field: different intervals, text formatter, and tallness.
        {
            let child = ctx.tree.create_child(layout_id, "h");
            let mut panel =
                ScalarFieldPanel::new("Hue", 0.0, 36000.0, exp.sf_hue as f64, look.clone());
            panel.scalar_field.set_scale_mark_intervals(hue_intervals);
            panel.scalar_field.set_text_box_tallness(0.35);
            panel.scalar_field.set_text_of_value_fn(Box::new(|val, iv| {
                if iv >= 6000 {
                    // C++ TextOfHueValue: major marks show color names
                    match (val / 6000) % 6 {
                        0 => "Red".to_string(),
                        1 => "Yellow".to_string(),
                        2 => "Green".to_string(),
                        3 => "Cyan".to_string(),
                        4 => "Blue".to_string(),
                        5 => "Magenta".to_string(),
                        _ => format!("{}\u{00B0}", val as f64 / 100.0),
                    }
                } else {
                    format!("{}\u{00B0}", val as f64 / 100.0)
                }
            }));
            ctx.tree.set_behavior(child, Box::new(panel));
        }

        create_pct_sf(ctx.tree, layout_id, "s", "Saturation", exp.sf_sat);
        create_pct_sf(ctx.tree, layout_id, "v", "Value", exp.sf_val);

        // TextField child for color name/hex.
        let tf_child = ctx.tree.create_child(layout_id, "n");
        ctx.tree.set_behavior(
            tf_child,
            Box::new(TextFieldPanel::new("Name", &exp.tf_name, look)),
        );
    }

    /// Layout children matching C++ `emColorField::LayoutChildren()`.
    ///
    /// Positions the RasterLayout child in the right half of the content rect,
    /// inset by `d = min(w,h) * 0.05`.
    pub fn layout_children(&self, ctx: &mut PanelCtx, w: f64, h: f64) {
        let children = ctx.children();
        if children.is_empty() || !self.expanded {
            for &child in &children {
                ctx.layout_child(child, 0.0, 0.0, 0.0, 0.0);
            }
            return;
        }

        // C++ GetContentRectUnobscured then inset by d
        let cr = self.border.content_rect_unobscured(w, h, &self.look);
        let d = cr.w.min(cr.h) * 0.05;
        let x = cr.x + d;
        let y = cr.y + d;
        let cw = (cr.w - 2.0 * d).max(0.0);
        let ch = (cr.h - 2.0 * d).max(0.0);

        // Position the RasterLayout child in the right half.
        let layout_id = children[0];
        ctx.layout_child(layout_id, x + cw * 0.5, y, cw * 0.5, ch);
    }

    /// Whether this color field provides how-to help text.
    /// Matches C++ `emColorField::HasHowTo` (always true).
    pub fn has_how_to(&self) -> bool {
        true
    }

    /// Help text describing how to use this color field.
    ///
    /// Chains the border's base how-to with color-field-specific sections.
    /// Matches C++ `emColorField::GetHowTo`.
    pub fn get_how_to(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.get_howto(enabled, focusable);
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

    #[test]
    fn toggle_expanded() {
        let look = Look::new();
        let mut cf = ColorField::new(look);
        assert!(!cf.is_expanded());

        cf.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(cf.is_expanded());

        cf.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(!cf.is_expanded());
    }

    #[test]
    fn set_and_get_color() {
        let look = Look::new();
        let mut cf = ColorField::new(look);
        cf.set_color(Color::RED);
        assert_eq!(cf.color(), Color::RED);
    }

    #[test]
    fn expansion_created_on_expand() {
        let look = Look::new();
        let mut cf = ColorField::new(look);
        assert!(cf.expansion().is_none());
        cf.set_expanded(true);
        assert!(cf.expansion().is_some());
    }

    #[test]
    fn expansion_destroyed_on_shrink() {
        let look = Look::new();
        let mut cf = ColorField::new(look);
        cf.set_expanded(true);
        cf.set_expanded(false);
        assert!(cf.expansion().is_none());
    }

    #[test]
    fn expansion_rgba_values_match_color() {
        let look = Look::new();
        let mut cf = ColorField::new(look);
        cf.set_color(Color::rgba(100, 150, 200, 255));
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
        let look = Look::new();
        let mut cf = ColorField::new(look);
        cf.set_color(Color::BLACK);
        cf.set_expanded(true);
        // Modify red via expansion
        cf.expansion_mut().unwrap().sf_red = 5000; // ~50% = 127
        assert!(cf.cycle());
        // Color should have updated red channel
        let r = cf.color().r();
        assert!((r as i64 - 127).abs() <= 1, "expected ~127, got {}", r);
    }

    #[test]
    fn cycle_hsv_change() {
        let look = Look::new();
        let mut cf = ColorField::new(look);
        cf.set_color(Color::BLACK);
        cf.set_expanded(true);
        // Set via HSV: hue=0 (red), sat=100%, val=100%
        let exp = cf.expansion_mut().unwrap();
        exp.sf_hue = 0;
        exp.sf_sat = 10000;
        exp.sf_val = 10000;
        assert!(cf.cycle());
        // Should be red
        assert_eq!(cf.color().r(), 255);
        assert!(cf.color().g() < 5);
        assert!(cf.color().b() < 5);
    }

    #[test]
    fn cycle_text_change() {
        let look = Look::new();
        let mut cf = ColorField::new(look);
        cf.set_expanded(true);
        cf.expansion_mut().unwrap().tf_name = "#FF0000".to_string();
        assert!(cf.cycle());
        assert_eq!(cf.color(), Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn update_name_output_hex_format() {
        let look = Look::new();
        let mut cf = ColorField::new(look);
        cf.set_color(Color::rgba(0xAB, 0xCD, 0xEF, 0xFF));
        cf.set_expanded(true);
        let exp = cf.expansion().unwrap();
        assert_eq!(exp.tf_name, "#ABCDEF");
    }

    #[test]
    fn update_hsv_preserves_hue_at_black() {
        let look = Look::new();
        let mut cf = ColorField::new(look);
        cf.set_color(Color::rgba(255, 0, 0, 255)); // Red
        cf.set_expanded(true);
        let hue_before = cf.expansion().unwrap().sf_hue;
        // Now set to black via RGBA
        cf.set_color(Color::BLACK);
        // Hue should be preserved (not reset to 0) because v=0
        let hue_after = cf.expansion().unwrap().sf_hue;
        assert_eq!(hue_before, hue_after);
    }
}
