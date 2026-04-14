use std::cell::OnceCell;

use crate::emColor::emColor;
use crate::emImage::emImage;
use crate::emLook::emLook;
use crate::emPainter::{emPainter, TextAlignment, VAlign, BORDER_EDGES_ONLY};
use crate::emResTga::load_tga;
use crate::emStroke::emStroke;
use crate::emPanel::Rect;

/// Outer border style.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OuterBorderType {
    None,
    Filled,
    Margin,
    MarginFilled,
    Rect,
    RoundRect,
    Group,
    Instrument,
    InstrumentMoreRound,
    PopupRoot,
}

/// Inner border style.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InnerBorderType {
    None,
    Group,
    InputField,
    OutputField,
    CustomRect,
}

/// Layout of icon, caption, and description within the label area.
struct LabelLayout {
    icon_rect: Option<Rect>,
    caption_rect: Option<Rect>,
    description_rect: Option<Rect>,
    total_height: f64,
}

/// emBorder chrome helper. Embedded in widgets to draw surrounding decoration.
pub struct emBorder {
    pub outer: OuterBorderType,
    pub inner: InnerBorderType,
    pub caption: String,
    pub description: String,
    pub border_scaling: f64,
    pub label_alignment: TextAlignment,
    pub caption_alignment: Option<TextAlignment>,
    pub description_alignment: Option<TextAlignment>,
    pub icon: Option<emImage>,
    pub icon_above_caption: bool,
    pub max_icon_area_tallness: f64,
    /// When `true` (default), the label is rendered inside the border and
    /// consumes space from the content area. When `false`, the label area
    /// is *not* subtracted from the content rect — callers paint the label
    /// externally via [`paint_label`](Self::paint_label).
    ///
    /// C++ equivalent: `emBorder::LabelInBorder`.
    pub label_in_border: bool,
    /// Name of the auxiliary child panel, if any.
    ///
    /// DIVERGED: C++ `AuxData` — the C++ `emBorder::AuxData` struct (with
    /// `PanelName`, `Tallness`, `PanelPointerCache`) is flattened into
    /// `emBorder` fields.  `PanelPointerCache` (`emCrossPtr<emPanel>`) is
    /// omitted because Rust `emBorder` is not a panel and has no child tree
    /// to cache lookups into; callers resolve the child by name externally.
    pub(crate) aux_panel_name: Option<String>,
    /// Height/width ratio of the auxiliary area (default 1.0 when absent).
    pub(crate) aux_tallness: f64,
    /// Whether this widget provides HowTo text.
    ///
    /// When `true`, the border reserves space on the left for the HowTo
    /// indicator and shifts the content area rightward.  C++ equivalent:
    /// `emBorder::HasHowTo()` (overridden per widget).
    pub has_how_to: bool,
    /// The HowTo text rendered inside the indicator pill.
    /// C++ equivalent: `emBorder::GetHowTo()` / `emScalarField::GetHowTo()`.
    pub how_to_text: String,
}

impl emBorder {
    pub fn new(outer: OuterBorderType) -> Self {
        Self {
            outer,
            inner: InnerBorderType::None,
            caption: String::new(),
            description: String::new(),
            border_scaling: 1.0,
            label_alignment: TextAlignment::Left,
            caption_alignment: None,
            description_alignment: None,
            icon: None,
            icon_above_caption: false,
            max_icon_area_tallness: 1.0,
            label_in_border: true,
            aux_panel_name: None,
            aux_tallness: 1.0,
            has_how_to: false,
            how_to_text: String::new(),
        }
    }

    pub fn with_caption(mut self, caption: &str) -> Self {
        self.caption = caption.to_string();
        self
    }

    pub fn SetCaption(&mut self, caption: &str) {
        self.caption = caption.to_string();
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    pub fn with_inner(mut self, inner: InnerBorderType) -> Self {
        self.inner = inner;
        self
    }

    pub fn SetOuterBorderType(&mut self, obt: OuterBorderType) {
        self.outer = obt;
    }

    pub fn SetInnerBorderType(&mut self, ibt: InnerBorderType) {
        self.inner = ibt;
    }

    pub fn SetBorderType(&mut self, obt: OuterBorderType, ibt: InnerBorderType) {
        self.outer = obt;
        self.inner = ibt;
    }

    pub fn with_border_scaling(mut self, s: f64) -> Self {
        self.border_scaling = s.max(1e-10);
        self
    }

    pub fn SetBorderScaling(&mut self, s: f64) {
        self.border_scaling = s.max(1e-10);
    }

    pub fn with_label_alignment(mut self, a: TextAlignment) -> Self {
        self.label_alignment = a;
        self
    }

    pub fn SetLabelAlignment(&mut self, a: TextAlignment) {
        self.label_alignment = a;
    }

    pub fn with_caption_alignment(mut self, a: TextAlignment) -> Self {
        self.caption_alignment = Some(a);
        self
    }

    pub fn SetCaptionAlignment(&mut self, a: Option<TextAlignment>) {
        self.caption_alignment = a;
    }

    pub fn with_description_alignment(mut self, a: TextAlignment) -> Self {
        self.description_alignment = Some(a);
        self
    }

    pub fn SetDescriptionAlignment(&mut self, a: Option<TextAlignment>) {
        self.description_alignment = a;
    }

    pub fn with_icon(mut self, icon: emImage) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn SetIcon(&mut self, icon: Option<emImage>) {
        self.icon = icon;
    }

    /// Set caption, description, and icon in a single call.
    ///
    /// Port of C++ `emBorder::SetLabel(caption, description, icon)`.
    pub fn SetLabel(&mut self, caption: &str, description: &str, icon: Option<emImage>) {
        self.caption = caption.to_string();
        self.description = description.to_string();
        self.icon = icon;
    }

    pub fn SetIconAboveCaption(&mut self, above: bool) {
        self.icon_above_caption = above;
    }

    pub fn SetMaxIconAreaTallness(&mut self, t: f64) {
        self.max_icon_area_tallness = t.max(1e-10);
    }

    /// Builder: set whether the label is rendered inside the border.
    ///
    /// When `false`, the label does not consume content area space and must be
    /// painted externally via [`paint_label`](Self::paint_label).
    ///
    /// C++ equivalent: `emBorder::SetLabelInBorder`.
    pub fn with_label_in_border(mut self, in_border: bool) -> Self {
        self.label_in_border = in_border;
        self
    }

    /// Builder: set `has_how_to`.
    pub fn with_how_to(mut self, has: bool) -> Self {
        self.has_how_to = has;
        self
    }

    /// Set the HowTo text rendered inside the indicator pill.
    pub fn set_how_to_text(&mut self, text: String) {
        self.how_to_text = text;
    }

    /// Set whether the label is rendered inside the border.
    ///
    /// C++ equivalent: `emBorder::SetLabelInBorder`.
    pub fn SetLabelInBorder(&mut self, in_border: bool) {
        self.label_in_border = in_border;
    }

    /// Create or update the auxiliary panel area.
    ///
    /// If aux data does not exist yet, it is created. If it does exist,
    /// `panel_name` and `tallness` are updated independently only when they
    /// differ from the current values.
    ///
    /// C++ equivalent: `emBorder::HaveAux`.
    pub fn HaveAux(&mut self, panel_name: &str, tallness: f64) {
        match self.aux_panel_name {
            None => {
                self.aux_panel_name = Some(panel_name.to_string());
                self.aux_tallness = tallness;
            }
            Some(ref mut name) => {
                if name.as_str() != panel_name {
                    *name = panel_name.to_string();
                }
                if (self.aux_tallness - tallness).abs() > f64::EPSILON {
                    self.aux_tallness = tallness;
                }
            }
        }
    }

    /// Remove the auxiliary panel area if present. No-op if already absent.
    ///
    /// C++ equivalent: `emBorder::RemoveAux`.
    pub fn RemoveAux(&mut self) {
        self.aux_panel_name = None;
        self.aux_tallness = 1.0;
    }

    /// Return the auxiliary panel name, or an empty string if no aux data.
    ///
    /// C++ equivalent: `emBorder::GetAuxPanelName`.
    pub fn GetAuxPanelName(&self) -> &str {
        match self.aux_panel_name {
            Some(ref name) => name.as_str(),
            None => "",
        }
    }

    /// Return the auxiliary area tallness, or `1.0` if no aux data.
    ///
    /// C++ equivalent: `emBorder::GetAuxTallness`.
    pub fn GetAuxTallness(&self) -> f64 {
        if self.aux_panel_name.is_some() {
            self.aux_tallness
        } else {
            1.0
        }
    }

    /// Return whether an auxiliary panel is configured.
    ///
    /// DIVERGED: `emBorder::GetAuxPanel` — C++ returned a cached
    /// `emPanel*` via `PanelPointerCache` (an `emCrossPtr` that
    /// auto-nullifies when the child is deleted).  Rust `emBorder` is not a
    /// panel and owns no child tree, so this method returns a bool instead.
    /// Callers use [`GetAuxPanelName`](Self::GetAuxPanelName) to resolve the
    /// panel by name in the widget tree.
    pub fn HasAux(&self) -> bool {
        self.aux_panel_name.is_some()
    }

    /// Compute the auxiliary area rectangle.
    ///
    /// Returns `Some(Rect)` when aux data is present, or `None` otherwise.
    /// When aux data is present and the label is shown, the aux rect is placed
    /// at the right side of the label area. When there is no label, it is placed
    /// at the right side of the content round-rect area, sized at 10% of the
    /// smaller dimension.
    ///
    /// C++ equivalent: `emBorder::GetAuxRect`
    /// (via `DoBorder(BORDER_FUNC_AUX_RECT)`).
    pub fn GetAuxRect(&self, w: f64, h: f64) -> Option<Rect> {
        self.aux_panel_name.as_ref()?;

        let (ox, oy, ow, oh) = self.outer_insets(w, h);
        let rnd_x = ox;
        let rnd_y = oy;
        let rnd_w = (w - ow).max(0.0);
        let rnd_h = (h - oh).max(0.0);

        if self.label_in_border && self.HasLabel() {
            // emLabel path: aux is placed at the right of the label text area.
            let label_area_w = rnd_w;
            let lch = self.label_content_height(label_area_w, rnd_h);
            let layout = self.label_layout(rnd_x, rnd_y, label_area_w, lch);
            let th = layout.total_height;
            if th <= 0.0 {
                return Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 1e-100,
                    h: 1e-100,
                });
            }
            // Aux gap and width per C++ DoBorder logic.
            let gap = th * 0.2;
            let aux_w = th / self.aux_tallness;
            let label_tallness = th / label_area_w.max(1e-100);
            let needed = gap + aux_w + th / label_tallness.max(1e-100) * 0.5;
            let final_aux_w = if label_area_w < needed {
                // Scale down proportionally.
                let scale = label_area_w / needed.max(1e-100);
                aux_w * scale
            } else {
                aux_w
            };
            Some(Rect {
                x: rnd_x + label_area_w - final_aux_w,
                y: rnd_y,
                w: final_aux_w,
                h: th,
            })
        } else {
            // No label path: aux is 10% of the smaller dimension.
            let s = rnd_w.min(rnd_h);
            let mut aux_w = s * 0.1;
            let mut aux_h = aux_w * self.aux_tallness;
            // Space available for aux (vertically).
            let avail_h = rnd_h;
            if aux_h > avail_h {
                aux_h = avail_h.max(1e-100);
                aux_w = aux_h / self.aux_tallness.max(1e-100);
            }
            // Margin from right edge.
            let d = s * 0.015;
            Some(Rect {
                x: rnd_x + rnd_w - aux_w - d,
                y: rnd_y + (rnd_h - aux_h) * 0.5,
                w: aux_w,
                h: aux_h,
            })
        }
    }

    pub(crate) fn HasLabel(&self) -> bool {
        !self.caption.is_empty() || !self.description.is_empty() || self.icon.is_some()
    }

    /// Best (natural) height-to-width ratio of the label.
    ///
    /// C++ equivalent: `emBorder::DoLabel(LABEL_FUNC_GET_BEST_TALLNESS)`.
    pub(crate) fn GetBestLabelTallness(&self) -> f64 {
        let has_cap = !self.caption.is_empty();
        let has_icon = self.icon.is_some();
        let has_desc = !self.description.is_empty();

        // Step 1: caption
        // C++: capW = emPainter::GetTextSize(Caption, 1.0, true, 0.0, &capH)
        // formatted=true so multi-line captions (e.g. "Show\nHidden\nFiles") return capH > 1.0
        let (cap_w, cap_h) = if has_cap {
            emPainter::GetTextSize(&self.caption, 1.0, true, 0.0)
        } else {
            (0.0, 0.0)
        };
        let (mut total_w, mut total_h) = if has_cap {
            (cap_w, cap_h)
        } else {
            (1.0, 1.0) // C++ defaults; overwritten by icon/desc if present
        };

        // Step 2: icon
        let icon_h_for_desc: f64; // iconH after icon processing (needed for desc-only-icon case)
        if let Some(ref img) = self.icon {
            let raw_w = img.GetWidth().max(1) as f64;
            let raw_h = img.GetHeight().max(1) as f64;
            let mut icon_w = raw_w;
            let mut icon_h = raw_h;
            if icon_h > icon_w * self.max_icon_area_tallness {
                icon_h = icon_w * self.max_icon_area_tallness;
            }
            if has_cap {
                if self.icon_above_caption {
                    let f = cap_h * 3.0;
                    icon_w *= f / icon_h;
                    icon_h = f;
                    let gap1 = cap_h * 0.1;
                    total_w = icon_w.max(cap_w);
                    total_h = icon_h + gap1 + cap_h;
                } else {
                    icon_w *= cap_h / icon_h;
                    icon_h = cap_h;
                    let gap1 = cap_h * 0.1;
                    total_w = icon_w + gap1 + cap_w;
                    total_h = cap_h;
                }
            } else {
                total_w = icon_w;
                total_h = icon_h;
            }
            icon_h_for_desc = icon_h;
        } else {
            icon_h_for_desc = 0.0;
        }

        // Step 3: description
        // C++: descW = emPainter::GetTextSize(Description, 1.0, true, 0.0, &descH)
        if has_desc {
            let (desc_w_raw, desc_h_raw) = emPainter::GetTextSize(&self.description, 1.0, true, 0.0);
            if has_icon || has_cap {
                let f = if has_cap {
                    cap_h * 0.15
                } else {
                    icon_h_for_desc * 0.05
                };
                let mut desc_w = f / desc_h_raw;
                let mut desc_h = f;
                if desc_w > total_w {
                    desc_h *= total_w / desc_w;
                    desc_w = total_w;
                }
                let gap2 = desc_h * 0.05;
                total_h += gap2 + desc_h;
                let _ = desc_w; // used in clamping above
            } else {
                // description only
                total_w = desc_w_raw;
                total_h = desc_h_raw;
            }
        }

        // Guard against degenerate (no label at all falls through with defaults 1.0/1.0)
        if !has_cap && !has_icon && !has_desc {
            return 1.0;
        }

        total_h / total_w.max(1e-100)
    }

    /// Unified border geometry computation matching C++ `DoBorder` line-for-line.
    /// All geometry functions delegate to this to eliminate divergence surfaces.
    ///
    /// Returns `(rndX, rndY, rndW, rndH, rndR, recX, recY, recW, recH, canvasColor)`
    /// after processing outer border, howto, label, and inner border.
    /// `w` = 1.0 (normalized width), `h` = panel tallness.
    #[allow(clippy::too_many_arguments)]
    fn do_border_geometry(
        &self,
        w: f64,
        h: f64,
        canvas_color: emColor,
        look: &emLook,
        enabled: bool,
    ) -> (f64, f64, f64, f64, f64, f64, f64, f64, f64, emColor) {
        // C++ emBorder.cpp DoBorder lines 578-899: outer border switch.
        let (mut rnd_x, mut rnd_y, mut rnd_w, mut rnd_h, mut rnd_r,
             min_space, how_to_space, label_space, mut canvas_color) = {
            let s = w.min(h) * self.border_scaling;
            match self.outer {
                OuterBorderType::None | OuterBorderType::Filled => {
                    let mut cc = canvas_color;
                    if self.outer == OuterBorderType::Filled {
                        let color = look.bg_color;
                        if !color.IsTotallyTransparent() { cc = color; }
                    }
                    (0.0, 0.0, w, h, 0.0, 0.0, 0.023, 0.17, cc)
                }
                OuterBorderType::Margin | OuterBorderType::MarginFilled => {
                    let d = s * 0.04;
                    let mut cc = canvas_color;
                    if self.outer == OuterBorderType::MarginFilled {
                        let color = look.bg_color;
                        if !color.IsTotallyTransparent() { cc = color; }
                    }
                    (d, d, w - 2.0*d, h - 2.0*d, 0.0, 0.0, 0.023, 0.17, cc)
                }
                OuterBorderType::Rect => {
                    let d = s * 0.023;
                    let e = s * 0.02;
                    let f = d + e;
                    let color = look.bg_color;
                    let mut cc = canvas_color;
                    if !color.IsTotallyTransparent() { cc = color; }
                    (f, f, w - 2.0*f, h - 2.0*f, 0.0, 0.023, 0.023, 0.17, cc)
                }
                OuterBorderType::RoundRect => {
                    let d = s * 0.023;
                    let e = s * 0.02;
                    let f = s * 0.22;
                    let g = d + e;
                    let color = look.bg_color;
                    let mut cc = canvas_color;
                    if !color.IsTotallyTransparent() { cc = color; }
                    (g, g, w - 2.0*g, h - 2.0*g, f - e, 0.023, 0.023, 0.17, cc)
                }
                OuterBorderType::Group => {
                    let d = s * 0.0104;
                    let color = look.bg_color;
                    let mut cc = canvas_color;
                    if !color.IsTotallyTransparent() { cc = color; }
                    (d, d, w - 2.0*d, h - 2.0*d, s * 0.0188, 0.0046, 0.0046, 0.05, cc)
                }
                OuterBorderType::Instrument => {
                    let d = s * 0.052;
                    let color = look.bg_color;
                    let mut cc = canvas_color;
                    if !color.IsTotallyTransparent() { cc = color; }
                    (d, d, w - 2.0*d, h - 2.0*d, s * 0.094, 0.023, 0.023, 0.17, cc)
                }
                OuterBorderType::InstrumentMoreRound => {
                    let d = s * 0.052;
                    let color = look.bg_color;
                    let mut cc = canvas_color;
                    if !color.IsTotallyTransparent() { cc = color; }
                    (d, d, w - 2.0*d, h - 2.0*d, s * 0.223, 0.023, 0.023, 0.17, cc)
                }
                OuterBorderType::PopupRoot => {
                    let d = s * 0.006;
                    let color = look.bg_color;
                    let mut cc = canvas_color;
                    if !color.IsTotallyTransparent() { cc = color; }
                    (d, d, w - 2.0*d, h - 2.0*d, 0.0, 0.0, 0.023, 0.17, cc)
                }
            }
        };

        // C++ line 901-902: s = min(rndW,rndH)*BorderScaling; minSpace *= s;
        let s = rnd_w.min(rnd_h) * self.border_scaling;
        let min_space = min_space * s;

        // C++ lines 904-933: HowTo space.
        if self.has_how_to {
            let how_to_space = how_to_space * s;
            if how_to_space > min_space {
                rnd_x += how_to_space - min_space;
                rnd_w -= how_to_space - min_space;
            }
        }

        // C++ lines 936-1065: label/aux/no-label paths.
        let label_space = if self.label_in_border && self.HasLabel() {
            label_space * s
        } else {
            0.0
        };

        let (mut rec_x, mut rec_y, mut rec_w, mut rec_h);

        if label_space > 0.0 {
            // C++ lines 983-1002: has-label path.
            rnd_x += min_space;
            rnd_w -= 2.0 * min_space;
            rnd_y += label_space;
            rnd_h -= label_space + min_space;
            rnd_r -= min_space;
            if rnd_r > 0.0 {
                rec_x = rnd_x + rnd_r * 0.5;
                rec_w = rnd_w - rnd_r;
                rec_y = rnd_y;
                rec_h = rnd_h - rnd_r * 0.5;
                let d = min_space + rnd_r * 0.5 - label_space;
                if d > 0.0 { rec_y += d; rec_h -= d; }
            } else {
                rnd_r = 0.0;
                rec_x = rnd_x; rec_w = rnd_w;
                rec_y = rnd_y; rec_h = rnd_h;
            }
        } else {
            // C++ lines 1046-1064: no-label path.
            rnd_x += min_space;
            rnd_y += min_space;
            rnd_w -= 2.0 * min_space;
            rnd_h -= 2.0 * min_space;
            rnd_r -= min_space;
            if rnd_r > 0.0 {
                rec_x = rnd_x + rnd_r * 0.5;
                rec_y = rnd_y + rnd_r * 0.5;
                rec_w = rnd_w - rnd_r;
                rec_h = rnd_h - rnd_r;
            } else {
                rnd_r = 0.0;
                rec_x = rnd_x; rec_w = rnd_w;
                rec_y = rnd_y; rec_h = rnd_h;
            }
        }

        // C++ lines 1067-1168: inner border switch.
        match self.inner {
            InnerBorderType::None => {}
            InnerBorderType::Group => {
                // C++ lines 1068-1089.
                let r = rnd_w.min(rnd_h) * self.border_scaling * 0.0188;
                if rnd_r < r { rnd_r = r; }
                let d = rnd_r * (17.0 / 225.0);
                rnd_x += d; rnd_y += d;
                rnd_w -= 2.0 * d; rnd_h -= 2.0 * d;
                rnd_r -= d;
                rec_x = rnd_x + rnd_r * 0.5;
                rec_y = rnd_y + rnd_r * 0.5;
                rec_w = rnd_w - rnd_r;
                rec_h = rnd_h - rnd_r;
            }
            InnerBorderType::InputField | InnerBorderType::OutputField => {
                // C++ lines 1091-1135.
                let r = rnd_w.min(rnd_h) * self.border_scaling * 0.094;
                if rnd_r < r { rnd_r = r; }
                let d = (16.0 / 216.0) * rnd_r;
                let tx = rnd_x + d;
                let ty = rnd_y + d;
                let tw = rnd_w - 2.0 * d;
                let th = rnd_h - 2.0 * d;
                let tr = rnd_r - d;
                rec_x = tx + tr * 0.5;
                rec_y = ty + tr * 0.5;
                rec_w = tw - tr;
                rec_h = th - tr;
                let color = if self.inner == InnerBorderType::InputField {
                    look.input_bg_color
                } else {
                    look.output_bg_color
                };
                let color = if enabled {
                    color
                } else {
                    color.GetBlended(look.bg_color, 80.0)
                };
                canvas_color = color;
                // Update rnd to tx/ty/tw/th/tr (C++ lines 1130-1134).
                rnd_x = tx; rnd_y = ty;
                rnd_w = tw; rnd_h = th;
                rnd_r = tr;
            }
            InnerBorderType::CustomRect => {
                // C++ lines 1137-1164.
                let d = rnd_r * 0.25;
                rnd_x += d; rnd_y += d;
                rnd_w -= 2.0 * d; rnd_h -= 2.0 * d;
                rnd_r -= d;
                let r = w.min(h) * self.border_scaling * 0.0125;
                if rnd_r < r { rnd_r = r; }
                let d2 = rnd_r;
                rnd_x += d2; rnd_y += d2;
                rnd_w -= 2.0 * d2; rnd_h -= 2.0 * d2;
                rnd_r = 0.0;
                rec_x = rnd_x; rec_y = rnd_y;
                rec_w = rnd_w; rec_h = rnd_h;
            }
        }

        (rnd_x, rnd_y, rnd_w, rnd_h, rnd_r, rec_x, rec_y, rec_w, rec_h, canvas_color)
    }

    /// Base scaling unit for outer geometry.
    fn base_unit(&self, w: f64, h: f64) -> f64 {
        w.min(h) * self.border_scaling
    }

    /// Outer border insets `(x, y, w_total, h_total)` — proportional to dimensions.
    ///
    /// Matches C++ `rndX`/`rndY` for each border type. For Rect and RoundRect,
    /// C++ sets `rndX = d + e` where `d = margin`, `e = stroke_width`.
    fn outer_insets(&self, w: f64, h: f64) -> (f64, f64, f64, f64) {
        let s = self.base_unit(w, h);
        let d = match self.outer {
            OuterBorderType::None | OuterBorderType::Filled => 0.0,
            OuterBorderType::Margin | OuterBorderType::MarginFilled => s * 0.04,
            // C++ OBT_RECT: rndX = d + e = s*0.023 + s*0.02 = s*0.043.
            OuterBorderType::Rect => s * 0.023 + s * 0.02,
            // C++ OBT_ROUND_RECT: rndX = g = d + e = s*0.023 + s*0.02 = s*0.043.
            OuterBorderType::RoundRect => s * 0.023 + s * 0.02,
            OuterBorderType::Group => s * 0.0104,
            OuterBorderType::Instrument => s * 0.052,
            OuterBorderType::InstrumentMoreRound => s * 0.052,
            OuterBorderType::PopupRoot => s * 0.006,
        };
        if d == 0.0 {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            (d, d, 2.0 * d, 2.0 * d)
        }
    }

    /// Inner border insets, computed from the area after outer+label.
    ///
    /// C++ insets by a fraction of the inner radius, not the full radius:
    /// - IBT_GROUP: `rndR * (17/225)`
    /// - IBT_INPUT/OUTPUT: `rndR * (16/216)`
    fn inner_insets(&self, iw: f64, ih: f64) -> (f64, f64, f64, f64) {
        let s = iw.min(ih) * self.border_scaling;
        let d = match self.inner {
            InnerBorderType::None => 0.0,
            InnerBorderType::Group => s * 0.0188 * (17.0 / 225.0),
            InnerBorderType::InputField | InnerBorderType::OutputField => {
                s * 0.094 * (16.0 / 216.0)
            }
            InnerBorderType::CustomRect => s * 0.0125,
        };
        if d == 0.0 {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            (d, d, 2.0 * d, 2.0 * d)
        }
    }

    /// Corner radius for outer border types.
    ///
    /// Returns C++ `rndR` at label-placement time, which for RoundRect is
    /// `f - e = s*0.22 - s*0.02 = s*0.20` (inner edge of the stroke).
    fn outer_radius(&self, w: f64, h: f64) -> f64 {
        let s = self.base_unit(w, h);
        match self.outer {
            // C++ OBT_ROUND_RECT: rndR = f - e = s*0.22 - s*0.02 = s*0.20.
            OuterBorderType::RoundRect => s * 0.20,
            OuterBorderType::Group => s * 0.0188,
            OuterBorderType::Instrument => s * 0.094,
            OuterBorderType::InstrumentMoreRound => s * 0.223,
            _ => 0.0,
        }
    }

    /// Corner radius for inner border types.
    /// The label-space factor, which differs by border type.
    /// Eagle Mode: Group uses 0.05, all others use 0.17.
    fn label_space_factor(&self) -> f64 {
        match self.outer {
            OuterBorderType::Group => 0.05,
            _ => 0.17,
        }
    }

    /// HowTo space factor per outer border type.
    ///
    /// C++ `DoBorder` sets `howToSpace` per outer border type alongside `minSpace`.
    /// After the outer-border switch, `howToSpace *= s` where
    /// `s = min(rndW, rndH) * BorderScaling`. When `has_how_to` and
    /// `howToSpace > minSpace`, the content area shifts rightward.
    fn how_to_space_factor(&self) -> f64 {
        match self.outer {
            OuterBorderType::Group => 0.0046,
            _ => 0.023,
        }
    }

    /// Minimum spacing between decoration and label/content area.
    ///
    /// C++ `DoBorder` sets `minSpace` per outer border type. After the outer
    /// border switch, `minSpace *= s` where `s = min(rndW, rndH) * BorderScaling`.
    /// When no label/aux is present, the rnd rect is inset by minSpace on all sides.
    fn min_space_factor(&self) -> f64 {
        match self.outer {
            OuterBorderType::None
            | OuterBorderType::Filled
            | OuterBorderType::Margin
            | OuterBorderType::MarginFilled
            | OuterBorderType::PopupRoot => 0.0,
            OuterBorderType::Group => 0.0046,
            _ => 0.023,
        }
    }

    /// Full height reserved for the label region (including top/bottom padding).
    ///
    /// Eagle Mode's DoBorder: `labelSpace = s * factor` where
    /// `s = min(rnd_w, rnd_h) * BorderScaling`. This is the space subtracted
    /// from the content area — it includes the text zone plus surrounding padding.
    fn label_space(&self, rnd_w: f64, rnd_h: f64) -> f64 {
        let s = rnd_w.min(rnd_h) * self.border_scaling;
        s * self.label_space_factor()
    }

    /// Usable height within the label space for actual text/icon content.
    ///
    /// Eagle Mode: `d = labelSpace * 0.1; content_h = labelSpace - 2 * d`.
    fn label_content_height(&self, rnd_w: f64, rnd_h: f64) -> f64 {
        self.label_space(rnd_w, rnd_h) * 0.8
    }

    /// Compute label layout within the given area.
    ///
    /// Literal port of C++ `DoLabel` geometry (emBorder.cpp:1194-1352).
    /// Computes natural dimensions via GetTextSize, then applies a uniform
    /// scaling factor `f = area_h / totalH` to all sub-elements.
    fn label_layout(&self, area_x: f64, area_y: f64, area_w: f64, area_h: f64) -> LabelLayout {
        let has_cap = !self.caption.is_empty();
        let has_desc = !self.description.is_empty();
        let icon = self.icon.as_ref().filter(|img| !img.IsEmpty());
        let has_icon = icon.is_some();

        // --- Step 1: natural dimensions at unit scale (C++ lines 1203-1281) ---

        let mut total_w = 1.0_f64;
        let mut total_h = 1.0_f64;

        let (cap_w, mut cap_h) = if has_cap {
            let (w, h) = emPainter::GetTextSize(&self.caption, 1.0, true, 0.0);
            total_w = w;
            total_h = h;
            (w, h)
        } else {
            (0.0, 0.0)
        };

        let (mut icon_w, mut icon_h) = (0.0_f64, 0.0_f64);
        let mut gap1 = 0.0_f64;
        if let Some(img) = icon {
            icon_w = img.GetWidth().max(1) as f64;
            icon_h = img.GetHeight().max(1) as f64;
            if icon_h > icon_w * self.max_icon_area_tallness {
                icon_h = icon_w * self.max_icon_area_tallness;
            }
            if has_cap {
                if self.icon_above_caption {
                    let s = cap_h * 3.0;
                    icon_w *= s / icon_h;
                    icon_h = s;
                    gap1 = cap_h * 0.1;
                    total_w = icon_w.max(cap_w);
                    total_h = icon_h + gap1 + cap_h;
                } else {
                    icon_w *= cap_h / icon_h;
                    icon_h = cap_h;
                    gap1 = cap_h * 0.1;
                    total_w = icon_w + gap1 + cap_w;
                    total_h = cap_h;
                }
            } else {
                total_w = icon_w;
                total_h = icon_h;
            }
        }

        let mut desc_h = 0.0_f64;
        let mut gap2 = 0.0_f64;
        if has_desc {
            let (dw, dh) = emPainter::GetTextSize(&self.description, 1.0, true, 0.0);
            if has_icon || has_cap {
                let target = if has_cap { cap_h * 0.15 } else { icon_h * 0.05 };
                let mut desc_w_nat = target / dh;
                desc_h = target;
                if desc_w_nat > total_w {
                    desc_h *= total_w / desc_w_nat;
                    desc_w_nat = total_w;
                }
                let _ = desc_w_nat;
                gap2 = desc_h * 0.05;
                total_h += gap2 + desc_h;
            } else {
                total_w = dw;
                total_h = dh;
                desc_h = dh;
            }
        }

        if total_h <= 0.0 || total_w <= 0.0 {
            return LabelLayout {
                icon_rect: None,
                caption_rect: None,
                description_rect: None,
                total_height: 0.0,
            };
        }

        // --- Step 2: scaling factor and alignment (C++ lines 1288-1329) ---

        let min_ws = 0.5_f64;
        let mut x = area_x;
        let mut y = area_y;
        let mut w = area_w;

        let mut f = area_h / total_h;
        let w2 = f * total_w;
        if w2 <= w {
            match self.label_alignment {
                TextAlignment::Left => {}
                TextAlignment::Center => { x += (w - w2) * 0.5; }
                TextAlignment::Right => { x += w - w2; }
            }
            w = w2;
        } else {
            let min_total_w = if has_icon {
                if self.icon_above_caption {
                    icon_w
                } else {
                    icon_w + gap1 + cap_w * min_ws
                }
            } else {
                total_w * min_ws
            };
            let w2 = f * min_total_w;
            if w2 > w {
                f = w / min_total_w;
                let h2 = f * total_h;
                // C++ applies vertical alignment (TOP/BOTTOM/CENTER).
                // Rust label_alignment is horizontal only; default to center.
                y += (area_h - h2) * 0.5;
            }
        }

        // --- Step 3: scale and compute final rects (C++ lines 1331-1352) ---

        icon_w *= f;
        icon_h *= f;
        gap1 *= f;
        let icon_y = y;
        cap_h *= f;
        let (icon_x, cap_x, cap_y, cap_w_final);
        if self.icon_above_caption {
            icon_x = x + (w - icon_w) * 0.5;
            cap_x = x;
            cap_y = icon_y + icon_h + gap1;
            cap_w_final = w;
        } else {
            icon_x = x;
            cap_x = icon_x + icon_w + gap1;
            cap_y = y;
            cap_w_final = x + w - cap_x;
        }
        gap2 *= f;
        let desc_x = x;
        let desc_y = (icon_y + icon_h).max(cap_y + cap_h) + gap2;
        let desc_w_final = w;
        desc_h *= f;

        let icon_rect = if has_icon {
            Some(Rect { x: icon_x, y: icon_y, w: icon_w, h: icon_h })
        } else {
            None
        };
        let caption_rect = if has_cap {
            Some(Rect { x: cap_x, y: cap_y, w: cap_w_final, h: cap_h })
        } else {
            None
        };
        let description_rect = if has_desc && desc_h > 0.0 {
            Some(Rect { x: desc_x, y: desc_y, w: desc_w_final, h: desc_h })
        } else {
            None
        };

        LabelLayout {
            icon_rect,
            caption_rect,
            description_rect,
            total_height: f * total_h,
        }
    }

    // ---- HowTo text (C++ emBorder::HowToPreface / HowToDisabled / HowToFocus) ----

    /// Preface text for the how-to section. C++ `emBorder::HowToPreface`.
    pub(crate) const HOWTO_PREFACE: &'static str = "\
How to use this panel\n\
#####################\n\
\n\
Here is some text describing the usage of this panel. The text consists of\n\
multiple sections which may come from different parts of the program based on\n\
each other. If something is contradictory, the later section should count.\n";

    /// Disabled-state how-to section. C++ `emBorder::HowToDisabled`.
    pub(crate) const HOWTO_DISABLED: &'static str = "\
\n\
\n\
DISABLED\n\
\n\
This panel is currently disabled, because the panel is probably irrelevant for\n\
the current state of the program or data. Any try to modify data or to trigger a\n\
function may silently be ignored.\n";

    /// Focus how-to section. C++ `emBorder::HowToFocus`.
    pub(crate) const HOWTO_FOCUS: &'static str = "\
\n\
\n\
FOCUS\n\
\n\
This panel is focusable. Only one panel can be focused at a time. The focus is\n\
indicated by small arrows pointing to the focused panel. If a panel is focused,\n\
it gets the keyboard input. If the focused panel does not know what to do with a\n\
certain input key, it may even forward the input to its ancestor panels.\n\
\n\
How to move or set the focus:\n\
\n\
* Just zoom and scroll around - the focus is moved automatically by that.\n\
\n\
* Click with the left or right mouse button on a panel to give it the focus.\n\
\n\
* Press Tab or Shift+Tab to move the focus to the next or previous sister\n\
\x20\x20panel.\n\
\n\
* Press the cursor keys to move the focus to a sister panel in the desired\n\
\x20\x20direction.\n\
\n\
* Press Page-Up or -Down to move the focus to a child or parent panel.\n";

    /// Build the how-to text for this border.
    ///
    /// Returns the preface, optionally appending the disabled and/or focus
    /// sections based on the panel state flags passed in. Callers (widget
    /// behaviors) supply the state because `emBorder` itself is not a panel.
    ///
    /// C++ equivalent: `emBorder::GetHowTo`.
    pub(crate) fn GetHowTo(&self, enabled: bool, focusable: bool) -> String {
        let mut text = String::from(Self::HOWTO_PREFACE);
        if !enabled {
            text.push_str(Self::HOWTO_DISABLED);
        }
        if focusable {
            text.push_str(Self::HOWTO_FOCUS);
        }
        text
    }

    /// Returns `true` if this border type fully fills its rect so nothing behind
    /// it is visible. Only the border types that paint a solid background over the
    /// entire panel area qualify, and only when the background color is opaque.
    ///
    /// C++ equivalent: `emBorder::IsOpaque`.
    pub fn IsOpaque(&self, look: &emLook) -> bool {
        match self.outer {
            OuterBorderType::Filled
            | OuterBorderType::MarginFilled
            | OuterBorderType::PopupRoot => look.bg_color.IsOpaque(),
            _ => false,
        }
    }

    /// Returns the *substance* round rectangle -- the outermost region where this
    /// border actually paints pixels. For simple rect-based types the radius is 0.
    /// For round types the radius matches the outer corner radius. For
    /// group/instrument types the rect is slightly expanded (per C++ TGA ratios)
    /// to cover the border-image area even though the Rust port paints simple
    /// fills.
    ///
    /// Returns `(rect, corner_radius)`.
    ///
    /// C++ equivalent: `emBorder::GetSubstanceRect`
    /// (via `DoBorder(BORDER_FUNC_SUBSTANCE_ROUND_RECT)`).
    pub fn GetSubstanceRect(&self, w: f64, h: f64) -> (Rect, f64) {
        let s = self.base_unit(w, h);
        match self.outer {
            OuterBorderType::None | OuterBorderType::Filled => (
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                },
                0.0,
            ),
            OuterBorderType::Margin | OuterBorderType::MarginFilled => {
                let d = s * 0.04;
                (
                    Rect {
                        x: d,
                        y: d,
                        w: (w - 2.0 * d).max(0.0),
                        h: (h - 2.0 * d).max(0.0),
                    },
                    0.0,
                )
            }
            OuterBorderType::Rect => {
                // Substance rect at the stroke center line.
                let d = s * 0.023;
                (
                    Rect {
                        x: d,
                        y: d,
                        w: (w - 2.0 * d).max(0.0),
                        h: (h - 2.0 * d).max(0.0),
                    },
                    0.0,
                )
            }
            OuterBorderType::RoundRect => {
                let d = s * 0.023; // substance rect inset
                let f = s * 0.22; // outer radius
                (
                    Rect {
                        x: d,
                        y: d,
                        w: (w - 2.0 * d).max(0.0),
                        h: (h - 2.0 * d).max(0.0),
                    },
                    (f - d).max(0.0),
                )
            }
            OuterBorderType::Group => {
                let d = s * 0.0104;
                let rnd_r = s * 0.0188;
                let r = rnd_r * 280.0 / 209.0;
                let e = r - rnd_r;
                (
                    Rect {
                        x: (d - e).max(0.0),
                        y: (d - e).max(0.0),
                        w: (w - 2.0 * d + 2.0 * e).max(0.0),
                        h: (h - 2.0 * d + 2.0 * e).max(0.0),
                    },
                    r,
                )
            }
            OuterBorderType::Instrument => {
                let d = s * 0.052;
                let rnd_r = s * 0.094;
                let r = rnd_r * 280.0 / 209.0;
                let e = r - rnd_r;
                (
                    Rect {
                        x: (d - e).max(0.0),
                        y: (d - e).max(0.0),
                        w: (w - 2.0 * d + 2.0 * e).max(0.0),
                        h: (h - 2.0 * d + 2.0 * e).max(0.0),
                    },
                    r,
                )
            }
            OuterBorderType::InstrumentMoreRound => {
                let d = s * 0.052;
                let rnd_r = s * 0.223;
                let r = rnd_r * 336.0 / 293.4;
                let e = r - rnd_r;
                (
                    Rect {
                        x: (d - e).max(0.0),
                        y: (d - e).max(0.0),
                        w: (w - 2.0 * d + 2.0 * e).max(0.0),
                        h: (h - 2.0 * d + 2.0 * e).max(0.0),
                    },
                    r,
                )
            }
            OuterBorderType::PopupRoot => (
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                },
                0.0,
            ),
        }
    }

    /// Compute the inner substance rectangle (no corner radius) from a panel rect.
    ///
    /// This is a convenience wrapper around
    /// [`substance_round_rect`](Self::substance_round_rect) that discards the
    /// corner radius and returns only the axis-aligned rectangle.
    ///
    /// Port of C++ `emBorder::GetSubstanceRect` (scalar rect variant).
    pub fn substance_rect(&self, panel_rect: Rect) -> Rect {
        let (r, _radius) = self.GetSubstanceRect(panel_rect.w, panel_rect.h);
        Rect {
            x: panel_rect.x + r.x,
            y: panel_rect.y + r.y,
            w: r.w,
            h: r.h,
        }
    }

    /// Returns the content area as a round rectangle with corner radius.
    /// Unlike [`content_rect`](Self::content_rect) which returns the axis-aligned
    /// inscribed rectangle, this returns the round-rect boundary and its radius so
    /// callers can perform round-rect hit-testing or clipping.
    ///
    /// Returns `(rect, corner_radius)`.
    ///
    /// C++ equivalent: `emBorder::GetContentRoundRect`
    /// (via `DoBorder(BORDER_FUNC_CONTENT_ROUND_RECT)`).
    pub fn GetContentRoundRect(&self, w: f64, h: f64, look: &emLook) -> (Rect, f64) {
        let (rnd_x, rnd_y, rnd_w, rnd_h, rnd_r, _, _, _, _, _) =
            self.do_border_geometry(w, h, emColor::TRANSPARENT, look, true);
        (
            Rect { x: rnd_x, y: rnd_y, w: rnd_w.max(0.0), h: rnd_h.max(0.0) },
            rnd_r.max(0.0),
        )
    }

    /// Returns the content rect with areas obscured by inner-border overlays
    /// removed. For input/output field inner borders, this is slightly smaller
    /// than [`content_rect`](Self::content_rect) because the field shadow/border
    /// images paint over the edges of the content area. For all other inner
    /// border types the result equals `content_rect`.
    ///
    /// C++ equivalent: `emBorder::GetContentRectUnobscured`
    /// (via `DoBorder(BORDER_FUNC_CONTENT_RECT_UNOBSCURED)`).
    pub fn GetContentRectUnobscured(&self, w: f64, h: f64, look: &emLook) -> Rect {
        match self.inner {
            InnerBorderType::InputField | InnerBorderType::OutputField => {
                // C++ emBorder.cpp lines 1121-1128: compute from the round-rect
                // boundary (after outer+label+minSpace, BEFORE inner border
                // inset) using the bumped inner radius.
                let (ox, oy, ow, oh) = self.outer_insets(w, h);
                let mut rnd_x = ox;
                let mut rnd_y = oy;
                let mut rnd_w = (w - ow).max(0.0);
                let mut rnd_h = (h - oh).max(0.0);
                let s = rnd_w.min(rnd_h) * self.border_scaling;
                let ms = s * self.min_space_factor();
                let mut rnd_r = self.outer_radius(w, h);

                if self.has_how_to {
                    let hts = s * self.how_to_space_factor();
                    if hts > ms {
                        rnd_x += hts - ms;
                        rnd_w -= hts - ms;
                    }
                }

                let label_h = if self.label_in_border && self.HasLabel() {
                    s * self.label_space_factor()
                } else {
                    0.0
                };

                if label_h > 0.0 {
                    rnd_x += ms;
                    rnd_w -= 2.0 * ms;
                    rnd_y += label_h;
                    rnd_h -= label_h + ms;
                    rnd_r -= ms;
                } else {
                    rnd_x += ms;
                    rnd_y += ms;
                    rnd_w -= 2.0 * ms;
                    rnd_h -= 2.0 * ms;
                    rnd_r -= ms;
                }
                if rnd_r < 0.0 {
                    rnd_r = 0.0;
                }

                // Bump rndR for IO field, then apply d = 220/216 * rndR.
                let r = rnd_w.min(rnd_h) * self.border_scaling * 0.094;
                if rnd_r < r {
                    rnd_r = r;
                }
                let d = (220.0 / 216.0) * rnd_r;

                Rect {
                    x: rnd_x + d,
                    y: rnd_y + d,
                    w: (rnd_w - 2.0 * d).max(0.0),
                    h: (rnd_h - 2.0 * d).max(0.0),
                }
            }
            _ => self.GetContentRect(w, h, look),
        }
    }

    /// Compute the content area after border and label insets.
    pub fn GetContentRect(&self, w: f64, h: f64, look: &emLook) -> Rect {
        let (_, _, _, _, _, rec_x, rec_y, rec_w, rec_h, _) =
            self.do_border_geometry(w, h, emColor::TRANSPARENT, look, true);
        Rect { x: rec_x, y: rec_y, w: rec_w.max(0.0), h: rec_h.max(0.0) }
    }

    /// Compute the canvas color at the content area, matching C++ DoBorder's
    /// canvasColor tracking.
    ///
    /// In C++, `DoBorder()` tracks `canvasColor` through outer and inner border
    /// painting. After the outer border paints its fill (using `emLook.GetBgColor()`),
    /// `canvasColor` becomes `bg_color`. After the inner border paints its fill
    /// (e.g., `InputField` uses `emLook.GetInputBgColor()`), `canvasColor` is
    /// updated again. The final value is what child panels receive via `Layout()`.
    ///
    /// This method replicates that logic without needing a painter.
    pub fn content_canvas_color(&self, parent_canvas: emColor, look: &emLook, enabled: bool) -> emColor {
        let (_, _, _, _, _, _, _, _, _, cc) =
            self.do_border_geometry(1.0, 1.0, parent_canvas, look, enabled);
        cc
    }

    /// Preferred size to fit the given content size.
    pub fn preferred_size_for_content(&self, cw: f64, ch: f64) -> (f64, f64) {
        let (_, _, ow, oh) = self.outer_insets(cw, ch);
        let label_area_w = cw;
        let rnd_h = (ch - oh).max(0.0);
        let label_h = if self.label_in_border && self.HasLabel() {
            self.label_space(label_area_w, rnd_h)
        } else {
            0.0
        };
        let (_, _, iw, ih) = self.inner_insets(cw, ch);
        (cw + ow + iw, ch + oh + label_h + ih)
    }

    /// Minimum size to fit any content.
    pub fn min_size_for_content(&self, min_cw: f64, min_ch: f64) -> (f64, f64) {
        self.preferred_size_for_content(min_cw, min_ch)
    }

    /// Paint the label externally at the given position.
    ///
    /// Use this when [`label_in_border`](Self::label_in_border) is `false` to
    /// render the label above or beside the border. The caller provides the
    /// position and dimensions for the label area.
    ///
    /// C++ equivalent: `emBorder::PaintLabel`.
    pub fn paint_label(&self, painter: &mut emPainter, area: Rect, look: &emLook, enabled: bool) {
        let dim_color = |c: emColor| -> emColor {
            if enabled {
                c
            } else {
                c.SetAlpha((c.GetAlpha() as f64 * 0.25 + 0.5) as u8)
            }
        };
        self.paint_label_impl(painter, area, look, &dim_color);
    }

    /// Paint the label with a custom text color (used by emButton for button_fg_color).
    pub fn paint_label_colored(
        &self,
        painter: &mut emPainter,
        area: Rect,
        look: &emLook,
        color: emColor,
        enabled: bool,
    ) {
        let dim_color = move |_c: emColor| -> emColor {
            if enabled {
                color
            } else {
                color.SetAlpha((color.GetAlpha() as f64 * 0.25 + 0.5) as u8)
            }
        };
        self.paint_label_impl(painter, area, look, &dim_color);
    }

    /// Internal helper that paints the label components (icon, caption,
    /// description) into the given area.
    fn paint_label_impl(
        &self,
        painter: &mut emPainter,
        area: Rect,
        look: &emLook,
        dim_color: &dyn Fn(emColor) -> emColor,
    ) {
        let label = self.label_layout(area.x, area.y, area.w, area.h);

        let cap_align = self.caption_alignment.unwrap_or(self.label_alignment);
        let desc_align = self.description_alignment.unwrap_or(self.label_alignment);

        // Icon — C++ re-centers to image's true aspect ratio (emBorder.cpp:1354-1357)
        if let Some(ref icon_rect) = label.icon_rect {
            if let Some(ref img) = self.icon {
                if !img.IsEmpty() {
                    let true_w =
                        icon_rect.h * img.GetWidth() as f64 / img.GetHeight() as f64;
                    let icon_x = icon_rect.x + (icon_rect.w - true_w) * 0.5;
                    let icon_w = true_w;
                    if img.GetChannelCount() == 1 {
                        painter.PaintImageColored(
                            icon_x,
                            icon_rect.y,
                            icon_w,
                            icon_rect.h,
                            img,
                            0,
                            0,
                            img.GetWidth(),
                            img.GetHeight(),
                            emColor::TRANSPARENT,
                            dim_color(look.fg_color),
                            emColor::TRANSPARENT,
                            crate::emTexture::ImageExtension::EdgeOrZero,
                        );
                    } else {
                        painter.paint_image_scaled(
                            icon_x,
                            icon_rect.y,
                            icon_w,
                            icon_rect.h,
                            img,
                            crate::emTexture::ImageQuality::Bilinear,
                            crate::emTexture::ImageExtension::Clamp,
                        );
                    }
                }
            }
        }

        // Caption — C++ DoLabel passes capH (rect height) as maxCharHeight
        // (emBorder.cpp:1384-1396).
        if let Some(ref cr) = label.caption_rect {
            let label_canvas = painter.GetCanvasColor();
            painter.PaintTextBoxed(
                cr.x,
                cr.y,
                cr.w,
                cr.h,
                &self.caption,
                cr.h,
                dim_color(look.fg_color),
                label_canvas,
                TextAlignment::Center,
                VAlign::Center,
                cap_align,
                0.5,
                true,
                0.0,
            );
        }

        // Description — C++ DoLabel passes descH (rect height) as maxCharHeight
        // (emBorder.cpp:1398-1412).
        if let Some(ref dr) = label.description_rect {
            let label_canvas = painter.GetCanvasColor();
            painter.PaintTextBoxed(
                dr.x,
                dr.y,
                dr.w,
                dr.h,
                &self.description,
                dr.h,
                dim_color(look.fg_color),
                label_canvas,
                TextAlignment::Center,
                VAlign::Center,
                desc_align,
                0.5,
                true,
                0.0,
            );
        }
    }

    /// Paint the border chrome.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_border(
        &self,
        painter: &mut emPainter,
        w: f64,
        h: f64,
        look: &emLook,
        _focused: bool,
        enabled: bool,
        pixel_scale: f64,
    ) {
        // Dimming for disabled state: C++ "GetTransparented(75.0)" = alpha * 0.25 + 0.5, truncate.
        let dim_color = |c: crate::emColor::emColor| -> crate::emColor::emColor {
            if enabled {
                c
            } else {
                c.SetAlpha((c.GetAlpha() as f64 * 0.25 + 0.5) as u8)
            }
        };

        // Outer border
        match self.outer {
            OuterBorderType::None => {}
            OuterBorderType::Filled => {
                painter.PaintRect(0.0, 0.0, w, h, look.bg_color, painter.GetCanvasColor());
                // C++ DoBorder: canvasColor=color after fill.
                if !look.bg_color.IsTotallyTransparent() {
                    painter.SetCanvasColor(look.bg_color);
                }
            }
            OuterBorderType::Margin => {}
            OuterBorderType::MarginFilled => {
                // C++ DoBorder: Clear fills the ENTIRE panel, not the inset rect.
                if !look.bg_color.IsTotallyTransparent() {
                    painter.PaintRect(0.0, 0.0, w, h, look.bg_color, painter.GetCanvasColor());
                    painter.SetCanvasColor(look.bg_color);
                }
            }
            OuterBorderType::Rect => {
                // C++ DoBorder: margin d, stroke e, fill at (d,d), outline centered on fill edge.
                let s = self.base_unit(w, h);
                let d = s * 0.023;
                let e = s * 0.02;
                if !look.bg_color.IsTotallyTransparent() {
                    painter.PaintRect(
                        d,
                        d,
                        w - 2.0 * d,
                        h - 2.0 * d,
                        look.bg_color,
                        painter.GetCanvasColor(),
                    );
                    // C++ updates canvasColor to bg_color after fill.
                    painter.SetCanvasColor(look.bg_color);
                }
                let color = dim_color(look.fg_color);
                let sd = d + e * 0.5;
                painter.PaintRectOutline(
                    sd,
                    sd,
                    w - 2.0 * sd,
                    h - 2.0 * sd,
                    &emStroke::new(color, e),
                    painter.GetCanvasColor(),
                );
            }
            OuterBorderType::RoundRect => {
                // C++ DoBorder: margin d, stroke e, radius f, fill at (d,d), outline centered.
                let s = self.base_unit(w, h);
                let d = s * 0.023;
                let e = s * 0.02;
                let r = s * 0.22;
                if !look.bg_color.IsTotallyTransparent() {
                    painter.PaintRoundRect(d, d, w - 2.0 * d, h - 2.0 * d, r, r, look.bg_color, painter.GetCanvasColor());
                    painter.SetCanvasColor(look.bg_color);
                }
                let color = dim_color(look.fg_color);
                let sd = d + e * 0.5;
                let sr = r - e * 0.5;
                painter.PaintRoundRectOutline(
                    sd,
                    sd,
                    w - 2.0 * sd,
                    h - 2.0 * sd,
                    sr,
                    sr,
                    &emStroke::new(color, e),
                );
            }
            OuterBorderType::Group => {
                let s = self.base_unit(w, h);
                let d = s * 0.0104;
                let rnd_r = s * 0.0188;
                let rnd_x = d;
                let rnd_y = d;
                let rnd_w = w - 2.0 * d;
                let rnd_h = h - 2.0 * d;
                let color = look.bg_color;
                let mut color2 = painter.GetCanvasColor();
                if !color.IsTotallyTransparent() && (!color2.IsOpaque() || color2 != color) {
                    let r = rnd_r * (280.0 / 209.0);
                    let e = r - rnd_r;
                    painter.PaintRoundRect(
                        rnd_x - e,
                        rnd_y - e,
                        rnd_w + 2.0 * e,
                        rnd_h + 2.0 * e,
                        r,
                        r,
                        color,
                        painter.GetCanvasColor(),
                    );
                    color2 = emColor::TRANSPARENT;
                }
                let r = rnd_r * (286.0 / 209.0);
                let e = r - rnd_r;
                with_toolkit_images(|img| {
                    painter.PaintBorderImage(
                        rnd_x - e,
                        rnd_y - e,
                        rnd_w + 2.0 * e,
                        rnd_h + 2.0 * e,
                        r,
                        r,
                        r,
                        r,
                        &img.group_border,
                        286,
                        286,
                        286,
                        286,
                        255,
                        color2,
                        BORDER_EDGES_ONLY,
                    );
                });
                if !color.IsTotallyTransparent() {
                    painter.SetCanvasColor(color);
                }
            }
            OuterBorderType::Instrument => {
                let s = self.base_unit(w, h);
                let d = s * 0.052;
                let rnd_r = s * 0.094;
                let rnd_x = d;
                let rnd_y = d;
                let rnd_w = w - 2.0 * d;
                let rnd_h = h - 2.0 * d;
                let color = look.bg_color;
                let mut color2 = painter.GetCanvasColor();
                if !color.IsTotallyTransparent() && (!color2.IsOpaque() || color2 != color) {
                    let r = rnd_r * (280.0 / 209.0);
                    let e = r - rnd_r;
                    painter.PaintRoundRect(
                        rnd_x - e,
                        rnd_y - e,
                        rnd_w + 2.0 * e,
                        rnd_h + 2.0 * e,
                        r,
                        r,
                        color,
                        painter.GetCanvasColor(),
                    );
                    color2 = emColor::TRANSPARENT;
                }
                let r = rnd_r * (286.0 / 209.0);
                let e = r - rnd_r;
                with_toolkit_images(|img| {
                    painter.PaintBorderImage(
                        rnd_x - e,
                        rnd_y - e,
                        rnd_w + 2.0 * e,
                        rnd_h + 2.0 * e,
                        r,
                        r,
                        r,
                        r,
                        &img.group_border,
                        286,
                        286,
                        286,
                        286,
                        255,
                        color2,
                        BORDER_EDGES_ONLY,
                    );
                });
                if !color.IsTotallyTransparent() {
                    painter.SetCanvasColor(color);
                }
            }
            OuterBorderType::InstrumentMoreRound => {
                let s = self.base_unit(w, h);
                let d = s * 0.052;
                let rnd_r = s * 0.223;
                let rnd_x = d;
                let rnd_y = d;
                let rnd_w = w - 2.0 * d;
                let rnd_h = h - 2.0 * d;
                let color = look.bg_color;
                let mut color2 = painter.GetCanvasColor();
                if !color.IsTotallyTransparent() && (!color2.IsOpaque() || color2 != color) {
                    let r = rnd_r * (336.0 / 293.4);
                    let e = r - rnd_r;
                    painter.PaintRoundRect(
                        rnd_x - e,
                        rnd_y - e,
                        rnd_w + 2.0 * e,
                        rnd_h + 2.0 * e,
                        r,
                        r,
                        color,
                        painter.GetCanvasColor(),
                    );
                    color2 = emColor::TRANSPARENT;
                }
                let r = rnd_r * (340.0 / 293.4);
                let e = r - rnd_r;
                with_toolkit_images(|img| {
                    painter.PaintBorderImage(
                        rnd_x - e,
                        rnd_y - e,
                        rnd_w + 2.0 * e,
                        rnd_h + 2.0 * e,
                        r,
                        r,
                        r,
                        r,
                        &img.button_border,
                        340,
                        340,
                        340,
                        340,
                        255,
                        color2,
                        BORDER_EDGES_ONLY,
                    );
                });
                if !color.IsTotallyTransparent() {
                    painter.SetCanvasColor(color);
                }
            }
            OuterBorderType::PopupRoot => {
                let s = self.base_unit(w, h);
                let d = s * 0.006;
                let color = look.bg_color;
                let canvas = painter.GetCanvasColor();
                if !color.IsTotallyTransparent() {
                    painter.PaintRect(0.0, 0.0, w, h, color, painter.GetCanvasColor());
                    painter.SetCanvasColor(color);
                }
                let r = d; // C++ ratio 159.0/159.0 = 1.0
                let cc = if !color.IsTotallyTransparent() {
                    color
                } else {
                    canvas
                };
                with_toolkit_images(|img| {
                    painter.PaintBorderImage(
                        0.0,
                        0.0,
                        w,
                        h,
                        r,
                        r,
                        r,
                        r,
                        &img.popup_border,
                        159,
                        159,
                        159,
                        159,
                        255,
                        cc,
                        BORDER_EDGES_ONLY,
                    );
                });
            }
        }

        // emLabel area — only painted when label_in_border is true.
        let (ox, oy, ow, oh) = self.outer_insets(w, h);
        let mut rnd_x = ox;
        let mut rnd_w = (w - ow).max(0.0);
        let rnd_h = (h - oh).max(0.0);

        // minSpace/howToSpace: C++ emBorder.cpp lines 901-933.
        let s = rnd_w.min(rnd_h) * self.border_scaling;
        let ms = s * self.min_space_factor();

        // HowTo space: shift content rightward if howToSpace > minSpace.
        if self.has_how_to {
            let hts = s * self.how_to_space_factor();

            // Paint HowTo indicator (C++ emBorder.cpp lines 906-928).
            let tw = hts * 0.9;
            let th = tw * 2.0;
            let tx = rnd_x + (hts - tw) * 0.5;
            let ty = oy + (rnd_h - th) * 0.5;
            // C++ GetTransparented(90) = alpha * 0.10 + 0.5
            painter.PaintRoundRect(
                tx,
                ty,
                tw,
                th,
                tw * 0.01,
                tw * 0.01,
                look.fg_color.SetAlpha((255.0 * 0.10 + 0.5) as u8),
                painter.GetCanvasColor(),
            );

            // C++ emBorder.cpp:916-927: paint text inside the pill when large enough.
            if tw * th * pixel_scale > 100.0 && !self.how_to_text.is_empty() {
                let d = tw * 0.01;
                // C++ GetTransparented(35) = alpha * 0.65 + 0.5
                let text_alpha = (look.fg_color.GetAlpha() as f64 * 0.65 + 0.5) as u8;
                painter.PaintTextBoxed(
                    tx + d,
                    ty + d,
                    tw - d * 2.0,
                    th - d * 2.0,
                    &self.how_to_text,
                    th,
                    look.fg_color.SetAlpha(text_alpha),
                    painter.GetCanvasColor(),
                    TextAlignment::Left,
                    VAlign::Top,
                    TextAlignment::Left,
                    0.9,
                    true,
                    0.0,
                );
            }

            if hts > ms {
                rnd_x += hts - ms;
                rnd_w -= hts - ms;
            }
        }

        let label_area_w = rnd_w;
        let ls = if self.label_in_border && self.HasLabel() {
            self.label_space(label_area_w, rnd_h)
        } else {
            0.0
        };

        if ls > 0.0 {
            let lch = self.label_content_height(label_area_w, rnd_h);
            // C++ emBorder.cpp lines 939-951:
            //   d = labelSpace*0.1; ty = rndY+d; th = labelSpace-2*d;
            //   e = emMax(d, minSpace); [corner-clearance]; tx = rndX+e; tw = rndW-2*e
            let d_label = ls * 0.1;
            let mut e_label = d_label.max(ms);
            // Corner-clearance: for rounded borders, e must clear the rounded corner
            // so the label text doesn't overlap the corner arc. C++ lines 943-948.
            let rnd_r = self.outer_radius(w, h);
            if e_label < rnd_r {
                let f = d_label * 0.77;
                let r = rnd_r - f;
                let g = r - d_label + f;
                let f2 = rnd_r - (r * r - g * g).sqrt();
                if e_label < f2 {
                    e_label = f2;
                }
            }
            self.paint_label_impl(
                painter,
                Rect::new(
                    rnd_x + e_label,
                    oy + d_label,
                    (label_area_w - 2.0 * e_label).max(0.0),
                    lch,
                ),
                look,
                &dim_color,
            );
        }

        // Inner border — use do_border_geometry's pre-inner-border computation.
        // This replicates C++ DoBorder lines 983-1065: minSpace+label adjustments.
        let (inner_x, inner_y, inner_w, inner_h, mut inner_r) = {
            let (ox2, oy2, ow2, oh2) = self.outer_insets(w, h);
            let mut rx = ox2;
            let mut rw = (w - ow2).max(0.0);
            let rh = (h - oh2).max(0.0);
            let s2 = rw.min(rh) * self.border_scaling;
            let ms2 = s2 * self.min_space_factor();
            if self.has_how_to {
                let hts2 = s2 * self.how_to_space_factor();
                if hts2 > ms2 { rx += hts2 - ms2; rw -= hts2 - ms2; }
            }
            let ls2 = if self.label_in_border && self.HasLabel() {
                s2 * self.label_space_factor()
            } else { 0.0 };
            if ls2 > 0.0 {
                rx += ms2; rw -= 2.0 * ms2;
                let ry = oy2 + ls2;
                let rh2 = rh - ls2 - ms2;
                let rr = self.outer_radius(w, h) - ms2;
                (rx, ry, rw, rh2, rr)
            } else {
                rx += ms2;
                let ry = oy2 + ms2;
                rw -= 2.0 * ms2;
                let rh2 = rh - 2.0 * ms2;
                let rr = self.outer_radius(w, h) - ms2;
                (rx, ry, rw, rh2, rr)
            }
        };

        match self.inner {
            InnerBorderType::None => {}
            InnerBorderType::Group => {
                // C++ line 1069-1070: r = min(rndW,rndH)*BorderScaling*0.0188; if(rndR<r) rndR=r;
                let r = inner_w.min(inner_h) * self.border_scaling * 0.0188;
                if inner_r < r { inner_r = r; }
                let canvas = painter.GetCanvasColor();
                with_toolkit_images(|img| {
                    painter.PaintBorderImage(
                        inner_x, inner_y, inner_w, inner_h,
                        inner_r, inner_r, inner_r, inner_r,
                        &img.group_inner_border,
                        225, 225, 225, 225,
                        255, canvas, BORDER_EDGES_ONLY,
                    );
                });
            }
            InnerBorderType::InputField => {
                // C++ line 1093-1094: r = min(rndW,rndH)*BorderScaling*0.094; if(rndR<r) rndR=r;
                let r = inner_w.min(inner_h) * self.border_scaling * 0.094;
                if inner_r < r { inner_r = r; }
                let bg = if enabled {
                    look.input_bg_color
                } else {
                    look.input_bg_color.GetBlended(look.bg_color, 80.0)
                };
                let d = (16.0 / 216.0) * inner_r;
                let tr = inner_r - d;
                painter.PaintRoundRect(
                    inner_x + d, inner_y + d,
                    inner_w - 2.0 * d, inner_h - 2.0 * d,
                    tr, tr, bg, painter.GetCanvasColor(),
                );
                painter.SetCanvasColor(bg);
            }
            InnerBorderType::OutputField => {
                let r = inner_w.min(inner_h) * self.border_scaling * 0.094;
                if inner_r < r { inner_r = r; }
                let bg = if enabled {
                    look.output_bg_color
                } else {
                    look.output_bg_color.GetBlended(look.bg_color, 80.0)
                };
                let d = (16.0 / 216.0) * inner_r;
                let tr = inner_r - d;
                painter.PaintRoundRect(
                    inner_x + d, inner_y + d,
                    inner_w - 2.0 * d, inner_h - 2.0 * d,
                    tr, tr, bg, painter.GetCanvasColor(),
                );
                painter.SetCanvasColor(bg);
            }
            InnerBorderType::CustomRect => {
                // C++ lines 1137-1153: first inset by 25% of corner radius,
                // then bump radius, then paint border image.
                // The generic inner_r bump at lines 1830-1833 uses the wrong
                // formula for CustomRect; recompute from the raw radius.
                let raw_r = (self.outer_radius(w, h) - ms).max(0.0);
                let d = raw_r * 0.25;
                let cr_x = inner_x + d;
                let cr_y = inner_y + d;
                let cr_w = (inner_w - 2.0 * d).max(0.0);
                let cr_h = (inner_h - 2.0 * d).max(0.0);
                let mut cr_r = raw_r - d;
                // C++ uses emMin(1.0, h) where 1.0 = normalized width.
                // In pixel space: w.min(h) with original panel dimensions.
                let r = w.min(h) * self.border_scaling * 0.0125;
                if cr_r < r {
                    cr_r = r;
                }
                let canvas = painter.GetCanvasColor();
                with_toolkit_images(|img| {
                    painter.PaintBorderImage(
                        cr_x,
                        cr_y,
                        cr_w,
                        cr_h,
                        cr_r,
                        cr_r,
                        cr_r,
                        cr_r,
                        &img.custom_rect_border,
                        200,
                        200,
                        200,
                        200,
                        255,
                        canvas,
                        BORDER_EDGES_ONLY,
                    );
                });
            }
        }
    }
    /// Paint the IO field border image overlay on top of content.
    ///
    /// C++ `emBorder::DoBorder` paints `PaintContent` (widget content) first,
    /// then overlays the IO field border image. Widgets using `InputField` or
    /// `OutputField` inner border types must call this AFTER painting their
    /// content to match this paint order.
    ///
    /// For other inner border types this is a no-op.
    pub fn paint_inner_overlay(&self, painter: &mut emPainter, w: f64, h: f64, _look: &emLook) {
        if self.inner != InnerBorderType::InputField && self.inner != InnerBorderType::OutputField {
            return;
        }

        // Use do_border_geometry pre-inner values matching C++ DoBorder lines 1091-1118.
        // We need rndX/rndY/rndW/rndH (before inner inset) and rndR (after clamping).
        // do_border_geometry returns post-inner values, so recompute pre-inner here
        // matching C++ exactly.
        let (ox2, oy2, ow2, oh2) = self.outer_insets(w, h);
        let mut rx = ox2;
        let mut rw = (w - ow2).max(0.0);
        let rh = (h - oh2).max(0.0);
        let s = rw.min(rh) * self.border_scaling;
        let ms = s * self.min_space_factor();
        if self.has_how_to {
            let hts = s * self.how_to_space_factor();
            if hts > ms { rx += hts - ms; rw -= hts - ms; }
        }
        let ls = if self.label_in_border && self.HasLabel() {
            s * self.label_space_factor()
        } else { 0.0 };
        let (inner_x, inner_y, inner_w, inner_h) = if ls > 0.0 {
            (rx + ms, oy2 + ls, (rw - 2.0*ms).max(0.0), rh - ls - ms)
        } else {
            (rx + ms, oy2 + ms, (rw - 2.0*ms).max(0.0), rh - 2.0*ms)
        };
        let mut inner_r = (self.outer_radius(w, h) - ms).max(0.0);
        let r = inner_w.min(inner_h) * self.border_scaling * 0.094;
        if inner_r < r { inner_r = r; }

        with_toolkit_images(|img| {
            painter.PaintBorderImage(
                inner_x,
                inner_y,
                inner_w,
                inner_h,
                300.0 / 216.0 * inner_r,
                346.0 / 216.0 * inner_r,
                inner_r,
                inner_r,
                &img.io_field,
                300,
                346,
                216,
                216,
                255,
                emColor::TRANSPARENT,
                BORDER_EDGES_ONLY,
            );
        });
    }
}

// RUST_ONLY: toolkit_images.rs -- compile-time TGA atlas extracted from C++ emBorder::TkResources
// (emBorder.h:321-341). C++ loads images at runtime via emGetResImage(); Rust embeds them via
// include_bytes!().

pub(crate) struct ToolkitImages {
    pub group_border: emImage,
    pub button_border: emImage,
    pub popup_border: emImage,
    pub group_inner_border: emImage,
    pub io_field: emImage,
    pub custom_rect_border: emImage,
    pub button: emImage,
    pub button_pressed: emImage,
    pub button_checked: emImage,
    pub splitter: emImage,
    pub splitter_pressed: emImage,
    pub check_box: emImage,
    pub check_box_pressed: emImage,
    pub radio_box: emImage,
    pub radio_box_pressed: emImage,
    pub tunnel: emImage,
    pub dir: emImage,
    pub dir_up: emImage,
}

fn decode_toolkit_image(data: &[u8], name: &str, expected_w: u32, expected_h: u32) -> emImage {
    let img = load_tga(data).unwrap_or_else(|e| panic!("failed to decode {name}: {e}"));
    assert_eq!(
        (img.GetWidth(), img.GetHeight()),
        (expected_w, expected_h),
        "{name} dimensions mismatch: got {}x{}, expected {expected_w}x{expected_h}",
        img.GetWidth(),
        img.GetHeight(),
    );
    img
}

impl ToolkitImages {
    fn TryLoad() -> Self {
        Self {
            group_border: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/GroupBorder.tga"),
                "GroupBorder",
                592,
                592,
            ),
            button_border: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/ButtonBorder.tga"),
                "ButtonBorder",
                704,
                704,
            ),
            popup_border: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/PopupBorder.tga"),
                "PopupBorder",
                320,
                320,
            ),
            group_inner_border: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/GroupInnerBorder.tga"),
                "GroupInnerBorder",
                470,
                470,
            ),
            io_field: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/IOField.tga"),
                "IOField",
                572,
                572,
            ),
            custom_rect_border: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/CustomRectBorder.tga"),
                "CustomRectBorder",
                450,
                450,
            ),
            button: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/Button.tga"),
                "Button",
                658,
                658,
            ),
            button_pressed: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/ButtonPressed.tga"),
                "ButtonPressed",
                648,
                648,
            ),
            button_checked: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/ButtonChecked.tga"),
                "ButtonChecked",
                648,
                648,
            ),
            splitter: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/Splitter.tga"),
                "Splitter",
                300,
                300,
            ),
            splitter_pressed: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/SplitterPressed.tga"),
                "SplitterPressed",
                300,
                300,
            ),
            check_box: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/CheckBox.tga"),
                "CheckBox",
                380,
                380,
            ),
            check_box_pressed: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/CheckBoxPressed.tga"),
                "CheckBoxPressed",
                380,
                380,
            ),
            radio_box: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/RadioBox.tga"),
                "RadioBox",
                380,
                380,
            ),
            radio_box_pressed: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/RadioBoxPressed.tga"),
                "RadioBoxPressed",
                380,
                380,
            ),
            tunnel: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/Tunnel.tga"),
                "Tunnel",
                200,
                200,
            ),
            dir: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/Dir.tga"),
                "Dir",
                320,
                260,
            ),
            dir_up: decode_toolkit_image(
                include_bytes!("../../../res/toolkit/DirUp.tga"),
                "DirUp",
                310,
                216,
            ),
        }
    }
}

thread_local! {
    static TOOLKIT: OnceCell<ToolkitImages> = const { OnceCell::new() };
}

pub(crate) fn with_toolkit_images<R>(f: impl FnOnce(&ToolkitImages) -> R) -> R {
    TOOLKIT.with(|cell| f(cell.get_or_init(ToolkitImages::TryLoad)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_look() -> emLook {
        emLook::default()
    }

    #[test]
    fn content_rect_none_border() {
        let border = emBorder::new(OuterBorderType::None);
        let Rect { x, y, w: cw, h: ch } = border.GetContentRect(100.0, 50.0, &test_look());
        assert!((x - 0.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);
        assert!((cw - 100.0).abs() < 0.01);
        assert!((ch - 50.0).abs() < 0.01);
    }

    #[test]
    fn content_rect_rect_border() {
        let border = emBorder::new(OuterBorderType::Rect);
        let Rect { x, y, w: cw, h: ch } = border.GetContentRect(100.0, 50.0, &test_look());
        // s = 50 * 1.0 = 50
        // outer_inset d = s * (0.023 + 0.02) = s * 0.043 = 2.15
        // After outer: rnd_w = 100 - 2*2.15 = 95.7, rnd_h = 50 - 2*2.15 = 45.7
        // minSpace ms = min(95.7, 45.7) * 1.0 * 0.023 = 45.7 * 0.023 = 1.0511
        let d: f64 = 50.0 * 0.043;
        let rnd_w = 100.0 - 2.0 * d;
        let rnd_h = 50.0 - 2.0 * d;
        let ms = rnd_w.min(rnd_h) * 0.023;
        assert!((x - (d + ms)).abs() < 0.01);
        assert!((y - (d + ms)).abs() < 0.01);
        assert!((cw - (rnd_w - 2.0 * ms)).abs() < 0.01);
        assert!((ch - (rnd_h - 2.0 * ms)).abs() < 0.01);
    }

    #[test]
    fn content_rect_with_caption() {
        let border = emBorder::new(OuterBorderType::Rect).with_caption("Test");
        let Rect { x, y, w: cw, h: ch } = border.GetContentRect(100.0, 50.0, &test_look());
        let d = 50.0 * 0.043;
        let rnd_w = 100.0 - 2.0 * d;
        let rnd_h = 50.0 - 2.0 * d;
        let label_h = border.label_space(rnd_w, rnd_h);
        // C++ uses pre-label rndH: s = min(rndW, rndH) * BorderScaling
        let ms = rnd_w.min(rnd_h) * 0.023;
        // Has-label path: rndR=0 for Rect, so no inscribed-rect conversion.
        // y = d + labelSpace (no ms at top), h = rndH - labelSpace - ms (1*ms bottom).
        assert!((x - (d + ms)).abs() < 0.01);
        assert!((y - (d + label_h)).abs() < 0.01);
        assert!((cw - (rnd_w - 2.0 * ms)).abs() < 0.5);
        assert!((ch - (rnd_h - label_h - ms)).abs() < 0.5);
    }

    #[test]
    fn content_rect_with_inner_input_field() {
        let border = emBorder::new(OuterBorderType::None).with_inner(InnerBorderType::InputField);
        let Rect { x, y, w: cw, h: ch } = border.GetContentRect(100.0, 50.0, &test_look());
        // OBT_NONE: outer_inset=0, minSpace=0, rndR=0
        // No-label: all ms=0, rndR=0-0=0, so rec = rndX/rndW (no inscribed rect)
        // Inner IO: rndR = max(0, min(100,50)*1.0*0.094) = 4.7
        // d = (16/216)*4.7, tr = 4.7-d, inscribed rect: x=d+tr/2, w=100-2*d-tr
        let rnd_r = 50.0 * 0.094;
        let d = (16.0 / 216.0) * rnd_r;
        let tr = rnd_r - d;
        assert!((x - (d + tr * 0.5)).abs() < 0.01);
        assert!((y - (d + tr * 0.5)).abs() < 0.01);
        assert!((cw - (100.0 - 2.0 * d - tr)).abs() < 0.01);
        assert!((ch - (50.0 - 2.0 * d - tr)).abs() < 0.01);
    }

    #[test]
    fn content_rect_instrument_with_caption_and_inner() {
        let border = emBorder::new(OuterBorderType::Instrument)
            .with_caption("Cap")
            .with_inner(InnerBorderType::InputField);
        let r = border.GetContentRect(100.0, 80.0, &test_look());
        // Just verify it produces a sane rect inside the panel.
        // The exact values depend on the inscribed-rect chain
        // (outer inscribed + inner IO inscribed) which matches C++ DoBorder.
        assert!(r.x > 0.0 && r.x < 50.0, "r.x={}", r.x);
        assert!(r.y > 0.0 && r.y < 40.0, "r.y={}", r.y);
        assert!(r.w > 30.0 && r.w < 100.0, "r.w={}", r.w);
        assert!(r.h > 20.0 && r.h < 80.0, "r.h={}", r.h);
    }

    #[test]
    fn preferred_size_round_trips() {
        let border = emBorder::new(OuterBorderType::RoundRect)
            .with_caption("Title")
            .with_inner(InnerBorderType::Group);
        let (pw, ph) = border.preferred_size_for_content(50.0, 30.0);
        let Rect { w: cw, h: ch, .. } = border.GetContentRect(pw, ph, &test_look());
        // Approximate round-trip: preferred_size_for_content uses simple additive
        // insets while content_rect uses the full inscribed-rect conversion, so
        // the round-trip is lossy. Allow broad tolerance.
        assert!((cw - 50.0).abs() < 15.0, "cw={cw}");
        assert!((ch - 30.0).abs() < 15.0, "ch={ch}");
    }

    #[test]
    fn border_scaling_doubles_insets() {
        let border1 = emBorder::new(OuterBorderType::Rect);
        let border2 = emBorder::new(OuterBorderType::Rect).with_border_scaling(2.0);
        let (ox1, _, _, _) = border1.outer_insets(100.0, 100.0);
        let (ox2, _, _, _) = border2.outer_insets(100.0, 100.0);
        assert!((ox2 - 2.0 * ox1).abs() < 0.01);
    }

    #[test]
    fn zero_size_clamping() {
        let border = emBorder::new(OuterBorderType::Instrument)
            .with_caption("Cap")
            .with_inner(InnerBorderType::InputField);
        let r = border.GetContentRect(1.0, 1.0, &test_look());
        assert!(r.w >= 0.0);
        assert!(r.h >= 0.0);
    }

    #[test]
    fn disabled_dimming_alpha() {
        use crate::emColor::emColor;
        let c = emColor::rgba(100, 150, 200, 255);
        // C++ GetTransparented(75.0): alpha * 0.25 + 0.5, truncate
        let dimmed = c.SetAlpha((c.GetAlpha() as f64 * 0.25 + 0.5) as u8);
        // 255 * 0.25 + 0.5 = 64.25, truncated = 64
        assert_eq!(dimmed.GetAlpha(), 64);
        assert_eq!(dimmed.GetRed(), 100);
    }

    #[test]
    fn with_alpha_preserves_rgb() {
        use crate::emColor::emColor;
        let c = emColor::rgb(10, 20, 30);
        let c2 = c.SetAlpha(128);
        assert_eq!(c2.GetRed(), 10);
        assert_eq!(c2.GetGreen(), 20);
        assert_eq!(c2.GetBlue(), 30);
        assert_eq!(c2.GetAlpha(), 128);
    }

    #[test]
    fn has_label_with_icon_only() {
        let img = emImage::new(16, 16, 4);
        let border = emBorder::new(OuterBorderType::None).with_icon(img);
        assert!(border.HasLabel());
    }

    #[test]
    fn label_height_icon_above() {
        let img = emImage::new(16, 16, 4);
        let mut border = emBorder::new(OuterBorderType::None)
            .with_caption("Cap")
            .with_icon(img);
        border.SetIconAboveCaption(true);
        let area_h = 100.0;
        let layout = border.label_layout(0.0, 0.0, 200.0, area_h);
        // icon_above: total_units = 3.0 + 0.1 + 1.0 = 4.1, total = area_h
        // icon_h = 3/4.1*100, gap = 0.1/4.1*100, cap_h = 1/4.1*100
        let total_units = 3.0 + 0.1 + 1.0;
        let expected = (3.0 / total_units + 0.1 / total_units + 1.0 / total_units) * area_h;
        assert!((layout.total_height - expected).abs() < 0.01);
    }

    #[test]
    fn content_rect_accounts_for_icon_height() {
        let img = emImage::new(16, 16, 4);
        let mut border = emBorder::new(OuterBorderType::None)
            .with_caption("Cap")
            .with_icon(img);
        border.SetIconAboveCaption(true);
        let r = border.GetContentRect(200.0, 200.0, &test_look());
        // OuterBorderType::None has zero insets, so rnd = full dims.
        // Content rect offset = label_space (includes padding around text).
        let ls = border.label_space(200.0, 200.0);
        assert!((r.y - ls).abs() < 0.01);
    }

    #[test]
    fn image_is_empty() {
        let empty = emImage::new(0, 0, 1);
        assert!(empty.IsEmpty());
        let nonempty = emImage::new(1, 1, 1);
        assert!(!nonempty.IsEmpty());
    }

    // --- is_opaque tests ---

    #[test]
    fn is_opaque_filled_opaque_bg() {
        let look = test_look();
        assert!(
            look.bg_color.IsOpaque(),
            "default look bg should be opaque"
        );
        let border = emBorder::new(OuterBorderType::Filled);
        assert!(border.IsOpaque(&look));
    }

    #[test]
    fn is_opaque_margin_filled() {
        let border = emBorder::new(OuterBorderType::MarginFilled);
        assert!(border.IsOpaque(&test_look()));
    }

    #[test]
    fn is_opaque_popup_root() {
        let border = emBorder::new(OuterBorderType::PopupRoot);
        assert!(border.IsOpaque(&test_look()));
    }

    #[test]
    fn is_opaque_false_for_non_filled() {
        let look = test_look();
        for outer in [
            OuterBorderType::None,
            OuterBorderType::Margin,
            OuterBorderType::Rect,
            OuterBorderType::RoundRect,
            OuterBorderType::Group,
            OuterBorderType::Instrument,
            OuterBorderType::InstrumentMoreRound,
        ] {
            let border = emBorder::new(outer);
            assert!(!border.IsOpaque(&look), "expected false for {outer:?}");
        }
    }

    #[test]
    fn is_opaque_transparent_bg() {
        use crate::emColor::emColor;
        let mut look = test_look();
        look.bg_color = emColor::rgba(100, 100, 100, 128);
        let border = emBorder::new(OuterBorderType::Filled);
        assert!(!border.IsOpaque(&look));
    }

    // --- substance_round_rect tests ---

    #[test]
    fn substance_none_is_full_rect() {
        let border = emBorder::new(OuterBorderType::None);
        let (rect, r) = border.GetSubstanceRect(200.0, 100.0);
        assert!(rect.x.abs() < 0.001);
        assert!(rect.y.abs() < 0.001);
        assert!((rect.w - 200.0).abs() < 0.001);
        assert!((rect.h - 100.0).abs() < 0.001);
        assert!(r.abs() < 0.001);
    }

    #[test]
    fn substance_filled_is_full_rect() {
        let border = emBorder::new(OuterBorderType::Filled);
        let (rect, r) = border.GetSubstanceRect(200.0, 100.0);
        assert!((rect.w - 200.0).abs() < 0.001);
        assert!((rect.h - 100.0).abs() < 0.001);
        assert!(r.abs() < 0.001);
    }

    #[test]
    fn substance_margin_is_inset() {
        let border = emBorder::new(OuterBorderType::Margin);
        let (rect, r) = border.GetSubstanceRect(100.0, 100.0);
        let d = 100.0 * 0.04;
        assert!((rect.x - d).abs() < 0.01);
        assert!((rect.y - d).abs() < 0.01);
        assert!((rect.w - (100.0 - 2.0 * d)).abs() < 0.01);
        assert!((rect.h - (100.0 - 2.0 * d)).abs() < 0.01);
        assert!(r.abs() < 0.001);
    }

    #[test]
    fn substance_round_rect_has_radius() {
        let border = emBorder::new(OuterBorderType::RoundRect);
        let (rect, r) = border.GetSubstanceRect(200.0, 100.0);
        assert!(r > 0.0, "round rect substance should have positive radius");
        assert!(rect.w < 200.0, "should be inset from full width");
    }

    #[test]
    fn substance_group_expanded_from_rnd() {
        let border = emBorder::new(OuterBorderType::Group);
        let (rect, r) = border.GetSubstanceRect(200.0, 100.0);
        let s = 100.0; // min(200,100) * 1.0
        let d = s * 0.0104; // outer inset
        let rnd_r = s * 0.0188;
        let expanded_r = rnd_r * 280.0 / 209.0;
        let e = expanded_r - rnd_r;
        assert!((r - expanded_r).abs() < 0.01);
        assert!((rect.x - (d - e)).abs() < 0.01);
    }

    #[test]
    fn substance_popup_root_is_full_rect() {
        let border = emBorder::new(OuterBorderType::PopupRoot);
        let (rect, r) = border.GetSubstanceRect(200.0, 100.0);
        assert!(rect.x.abs() < 0.001);
        assert!((rect.w - 200.0).abs() < 0.001);
        assert!(r.abs() < 0.001);
    }

    // --- content_round_rect tests ---

    #[test]
    fn content_round_rect_none_border() {
        let border = emBorder::new(OuterBorderType::None);
        let look = test_look();
        let (rect, r) = border.GetContentRoundRect(100.0, 50.0, &look);
        assert!(rect.x.abs() < 0.01);
        assert!(rect.y.abs() < 0.01);
        assert!((rect.w - 100.0).abs() < 0.01);
        assert!((rect.h - 50.0).abs() < 0.01);
        assert!(r.abs() < 0.01);
    }

    #[test]
    fn content_round_rect_with_inner_input_field() {
        let border = emBorder::new(OuterBorderType::None).with_inner(InnerBorderType::InputField);
        let look = test_look();
        let (rect, r) = border.GetContentRoundRect(100.0, 50.0, &look);
        // OBT_NONE: outer_inset=0, minSpace=0
        // inner inset d = s * 0.094 * (16/216), s = min(100,50) * 1.0 = 50
        let d = 50.0 * 0.094 * (16.0 / 216.0);
        assert!((rect.x - d).abs() < 0.5);
        assert!((rect.y - d).abs() < 0.5);
        assert!(r > 0.0, "IO field should have positive radius");
    }

    #[test]
    fn content_round_rect_matches_content_rect_position() {
        // content_rect is the inscribed axis-aligned rect inside
        // the round rect returned by content_round_rect.
        // So content_rect should be inset by ~radius*0.5 from the round rect.
        let border = emBorder::new(OuterBorderType::Rect).with_inner(InnerBorderType::Group);
        let look = test_look();
        let (rr, radius) = border.GetContentRoundRect(100.0, 60.0, &look);
        let cr = border.GetContentRect(100.0, 60.0, &look);
        if radius > 0.0 {
            assert!(cr.x >= rr.x, "cr.x={} < rr.x={}", cr.x, rr.x);
            assert!(cr.w <= rr.w, "cr.w={} > rr.w={}", cr.w, rr.w);
        }
        // Both should be inside the panel.
        assert!(cr.w > 0.0 && cr.h > 0.0);
    }

    // --- content_rect_unobscured tests ---

    #[test]
    fn content_rect_unobscured_equals_content_rect_for_none() {
        let border = emBorder::new(OuterBorderType::Rect);
        let look = test_look();
        let cr = border.GetContentRect(100.0, 50.0, &look);
        let cu = border.GetContentRectUnobscured(100.0, 50.0, &look);
        assert!((cr.x - cu.x).abs() < 0.001);
        assert!((cr.y - cu.y).abs() < 0.001);
        assert!((cr.w - cu.w).abs() < 0.001);
        assert!((cr.h - cu.h).abs() < 0.001);
    }

    #[test]
    fn content_rect_unobscured_smaller_for_input_field() {
        let border = emBorder::new(OuterBorderType::None).with_inner(InnerBorderType::InputField);
        let look = test_look();
        let cr = border.GetContentRect(200.0, 100.0, &look);
        let cu = border.GetContentRectUnobscured(200.0, 100.0, &look);
        // Unobscured should be strictly inset from content rect.
        assert!(
            cu.x > cr.x,
            "unobscured x ({}) > content x ({})",
            cu.x,
            cr.x
        );
        assert!(
            cu.y > cr.y,
            "unobscured y ({}) > content y ({})",
            cu.y,
            cr.y
        );
        assert!(
            cu.w < cr.w,
            "unobscured w ({}) < content w ({})",
            cu.w,
            cr.w
        );
        assert!(
            cu.h < cr.h,
            "unobscured h ({}) < content h ({})",
            cu.h,
            cr.h
        );
    }

    #[test]
    fn content_rect_unobscured_equals_content_rect_for_group_inner() {
        let border = emBorder::new(OuterBorderType::None).with_inner(InnerBorderType::Group);
        let look = test_look();
        let cr = border.GetContentRect(200.0, 100.0, &look);
        let cu = border.GetContentRectUnobscured(200.0, 100.0, &look);
        assert!((cr.x - cu.x).abs() < 0.001);
        assert!((cr.w - cu.w).abs() < 0.001);
    }

    // --- aux system tests ---

    #[test]
    fn aux_defaults_absent() {
        let border = emBorder::new(OuterBorderType::None);
        assert_eq!(border.GetAuxPanelName(), "");
        assert!((border.GetAuxTallness() - 1.0).abs() < f64::EPSILON);
        assert!(!border.HasAux());
        assert!(border.GetAuxRect(200.0, 100.0).is_none());
    }

    #[test]
    fn have_aux_creates_aux() {
        let mut border = emBorder::new(OuterBorderType::None);
        border.HaveAux("my_panel", 2.0);
        assert_eq!(border.GetAuxPanelName(), "my_panel");
        assert!((border.GetAuxTallness() - 2.0).abs() < f64::EPSILON);
        assert!(border.HasAux());
    }

    #[test]
    fn have_aux_updates_name() {
        let mut border = emBorder::new(OuterBorderType::None);
        border.HaveAux("p1", 1.0);
        border.HaveAux("p2", 1.0);
        assert_eq!(border.GetAuxPanelName(), "p2");
    }

    #[test]
    fn have_aux_updates_tallness() {
        let mut border = emBorder::new(OuterBorderType::None);
        border.HaveAux("p1", 1.0);
        border.HaveAux("p1", 3.5);
        assert!((border.GetAuxTallness() - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn remove_aux_clears() {
        let mut border = emBorder::new(OuterBorderType::None);
        border.HaveAux("p1", 2.0);
        border.RemoveAux();
        assert_eq!(border.GetAuxPanelName(), "");
        assert!((border.GetAuxTallness() - 1.0).abs() < f64::EPSILON);
        assert!(!border.HasAux());
        assert!(border.GetAuxRect(200.0, 100.0).is_none());
    }

    #[test]
    fn remove_aux_noop_when_absent() {
        let mut border = emBorder::new(OuterBorderType::None);
        border.RemoveAux(); // should not panic
        assert!(!border.HasAux());
    }

    #[test]
    fn aux_rect_no_label_positive_dimensions() {
        let mut border = emBorder::new(OuterBorderType::Rect);
        border.HaveAux("aux", 1.0);
        let rect = border
            .GetAuxRect(200.0, 100.0)
            .expect("aux should be present");
        assert!(rect.w > 0.0, "aux width should be positive");
        assert!(rect.h > 0.0, "aux height should be positive");
        assert!(rect.x >= 0.0, "aux x should be non-negative");
        assert!(rect.y >= 0.0, "aux y should be non-negative");
        // Should be on the right side.
        assert!(
            rect.x + rect.w <= 200.0 + 0.01,
            "aux should fit within widget width"
        );
    }

    #[test]
    fn aux_rect_with_label_positive_dimensions() {
        let mut border = emBorder::new(OuterBorderType::Rect).with_caption("Caption");
        border.HaveAux("aux", 1.5);
        let rect = border
            .GetAuxRect(200.0, 100.0)
            .expect("aux should be present");
        assert!(rect.w > 0.0, "aux width should be positive");
        assert!(rect.h > 0.0, "aux height should be positive");
        // Should be at right side of label area.
        assert!(
            rect.x + rect.w <= 200.0 + 0.01,
            "aux should fit within widget width"
        );
    }

    #[test]
    fn aux_rect_tallness_affects_shape() {
        let mut b1 = emBorder::new(OuterBorderType::None);
        b1.HaveAux("aux", 1.0);
        let r1 = b1.GetAuxRect(200.0, 200.0).unwrap();

        let mut b2 = emBorder::new(OuterBorderType::None);
        b2.HaveAux("aux", 2.0);
        let r2 = b2.GetAuxRect(200.0, 200.0).unwrap();

        // Higher tallness means taller relative to width.
        assert!(
            (r2.h / r2.w) > (r1.h / r1.w),
            "tallness 2.0 should be taller than 1.0"
        );
    }

    /// Verify that `substance_round_rect` uses the correct C++ coefficient
    /// `d = s * 0.023` for `OuterBorderType::Rect` and `RoundRect` (not the
    /// old buggy value of 0.006). With `border_scaling = 1.0` and
    /// `w = 1000.0, h = 100.0`, `s = min(w, h) * 1.0 = 100.0`, so
    /// `d = 100.0 * 0.023 = 2.3`. The substance rect should be inset by
    /// `d` on each side.
    #[test]
    fn substance_round_rect_rect_uses_correct_coefficient() {
        let border = emBorder::new(OuterBorderType::Rect);
        let w = 1000.0_f64;
        let h = 100.0_f64;
        let (rect, radius) = border.GetSubstanceRect(w, h);

        // s = min(1000, 100) * 1.0 = 100.0
        // d = 100.0 * 0.023 = 2.3
        let s = w.min(h) * border.border_scaling;
        let expected_d = s * 0.023;

        let eps = 1e-6;
        assert!(
            (rect.x - expected_d).abs() < eps,
            "rect.x = {}, expected {}",
            rect.x,
            expected_d,
        );
        assert!(
            (rect.y - expected_d).abs() < eps,
            "rect.y = {}, expected {}",
            rect.y,
            expected_d,
        );
        assert!(
            (rect.w - (w - 2.0 * expected_d)).abs() < eps,
            "rect.w = {}, expected {}",
            rect.w,
            w - 2.0 * expected_d,
        );
        assert!(
            (rect.h - (h - 2.0 * expected_d)).abs() < eps,
            "rect.h = {}, expected {}",
            rect.h,
            h - 2.0 * expected_d,
        );
        assert!(radius.abs() < eps, "Rect border should have radius 0");

        // Sanity: the buggy coefficient 0.006 would give d = 0.6, which is
        // far from 2.3.  Make sure we are NOT near the old value.
        let buggy_d = s * 0.006;
        assert!(
            (rect.x - buggy_d).abs() > 1.0,
            "rect.x {} is too close to the buggy inset {}",
            rect.x,
            buggy_d,
        );
    }

    #[test]
    fn substance_round_rect_roundrect_uses_correct_coefficient() {
        let border = emBorder::new(OuterBorderType::RoundRect);
        let w = 1000.0_f64;
        let h = 100.0_f64;
        let (rect, radius) = border.GetSubstanceRect(w, h);

        let s = w.min(h) * border.border_scaling;
        let expected_d = s * 0.023;
        let expected_f = s * 0.22;
        let expected_radius = (expected_f - expected_d).max(0.0);

        let eps = 1e-6;
        assert!(
            (rect.x - expected_d).abs() < eps,
            "rect.x = {}, expected {}",
            rect.x,
            expected_d,
        );
        assert!(
            (rect.y - expected_d).abs() < eps,
            "rect.y = {}, expected {}",
            rect.y,
            expected_d,
        );
        assert!(
            (rect.w - (w - 2.0 * expected_d)).abs() < eps,
            "rect.w = {}, expected {}",
            rect.w,
            w - 2.0 * expected_d,
        );
        assert!(
            (rect.h - (h - 2.0 * expected_d)).abs() < eps,
            "rect.h = {}, expected {}",
            rect.h,
            h - 2.0 * expected_d,
        );
        assert!(
            (radius - expected_radius).abs() < eps,
            "radius = {}, expected {}",
            radius,
            expected_radius,
        );

        // Sanity: must not match the old buggy coefficient.
        let buggy_d = s * 0.006;
        assert!(
            (rect.x - buggy_d).abs() > 1.0,
            "rect.x {} is too close to the buggy inset {}",
            rect.x,
            buggy_d,
        );
    }

    // --- best_label_tallness icon contribution tests ---

    #[test]
    fn best_label_tallness_no_icon() {
        let border = emBorder::new(OuterBorderType::None)
            .with_caption("Hi")
            .with_description("A short description");
        let t = border.GetBestLabelTallness();
        assert!(t > 0.0, "tallness should be positive, got {t}");
    }

    #[test]
    fn best_label_tallness_with_icon_above_greater_than_without() {
        // With icon_above_caption=true the icon stacks vertically above the
        // caption, adding 3*cap_h + gap to total_h. This must produce a
        // strictly greater tallness than the same label without an icon.
        let no_icon = emBorder::new(OuterBorderType::None)
            .with_caption("Hi")
            .with_description("A short description");
        let t_no_icon = no_icon.GetBestLabelTallness();

        let icon = emImage::new(16, 16, 4);
        let mut with_icon = emBorder::new(OuterBorderType::None)
            .with_caption("Hi")
            .with_description("A short description")
            .with_icon(icon);
        with_icon.SetIconAboveCaption(true);
        let t_with_icon = with_icon.GetBestLabelTallness();

        assert!(
            t_with_icon > t_no_icon,
            "icon-above should increase tallness: with_icon={t_with_icon}, no_icon={t_no_icon}"
        );
    }

    #[test]
    fn best_label_tallness_with_icon_beside_changes_tallness() {
        // With icon_above_caption=false (default), the icon is placed beside
        // the caption, widening total_w. The tallness must differ from no-icon.
        let no_icon = emBorder::new(OuterBorderType::None)
            .with_caption("Hi")
            .with_description("A short description");
        let t_no_icon = no_icon.GetBestLabelTallness();

        let icon = emImage::new(32, 32, 4);
        let with_icon = emBorder::new(OuterBorderType::None)
            .with_caption("Hi")
            .with_description("A short description")
            .with_icon(icon);
        let t_with_icon = with_icon.GetBestLabelTallness();

        assert!(
            (t_with_icon - t_no_icon).abs() > 1e-6,
            "icon-beside should change tallness: with_icon={t_with_icon}, no_icon={t_no_icon}"
        );
        // Beside layout widens total_w without changing total_h, so tallness decreases.
        assert!(
            t_with_icon < t_no_icon,
            "icon-beside should decrease tallness (wider): with_icon={t_with_icon}, no_icon={t_no_icon}"
        );
    }

    #[test]
    fn best_label_tallness_icon_above_vs_beside() {
        let icon = emImage::new(16, 64, 4);
        let mut above = emBorder::new(OuterBorderType::None)
            .with_caption("Hi")
            .with_description("A short description")
            .with_icon(icon);
        above.SetIconAboveCaption(true);
        let t_above = above.GetBestLabelTallness();

        let icon2 = emImage::new(16, 64, 4);
        let mut beside = emBorder::new(OuterBorderType::None)
            .with_caption("Hi")
            .with_description("A short description")
            .with_icon(icon2);
        beside.SetIconAboveCaption(false);
        let t_beside = beside.GetBestLabelTallness();

        assert!(
            (t_above - t_beside).abs() > 1e-6,
            "icon_above_caption should change tallness: above={t_above}, beside={t_beside}"
        );
        // icon_above stacks vertically → taller total → higher tallness ratio
        assert!(
            t_above > t_beside,
            "icon above should yield greater tallness: above={t_above}, beside={t_beside}"
        );
    }

    #[test]
    fn best_label_tallness_icon_only_no_caption() {
        // Icon without caption: tallness should reflect the icon's own aspect ratio
        // (clamped by max_icon_area_tallness).
        let icon = emImage::new(16, 64, 4);
        let with_icon = emBorder::new(OuterBorderType::None).with_icon(icon);
        let t = with_icon.GetBestLabelTallness();

        let no_label = emBorder::new(OuterBorderType::None);
        let t_empty = no_label.GetBestLabelTallness();

        // The icon (16x64 → raw tallness 4.0, clamped to max_icon_area_tallness=1.0)
        // should still produce tallness = 1.0, same as the no-label default.
        // But with a wider icon the tallness should differ from default.
        let wide_icon = emImage::new(64, 16, 4);
        let with_wide = emBorder::new(OuterBorderType::None).with_icon(wide_icon);
        let t_wide = with_wide.GetBestLabelTallness();

        // wide icon (64x16) → tallness = 16/64 = 0.25, less than default 1.0
        assert!(
            t_wide < t_empty,
            "wide icon should reduce tallness below default: wide={t_wide}, default={t_empty}"
        );
        assert!(t > 0.0, "icon-only tallness should be positive, got {t}");
    }

    // --- label_space uses pre-HowTo s (Fix 21) ---

    /// Helper: compute the expected label_h for a given panel size using the
    /// pre-HowTo `s = min(rnd_w, rnd_h) * border_scaling`, which is the correct
    /// value per C++ emBorder.cpp line 901/937.
    fn expected_label_h_pre_howto(border: &emBorder, w: f64, h: f64) -> f64 {
        let (_, _, ow, oh) = border.outer_insets(w, h);
        let rnd_w = (w - ow).max(0.0);
        let rnd_h = (h - oh).max(0.0);
        // s is computed BEFORE HowTo shift — this is the whole point of Fix 21.
        let s = rnd_w.min(rnd_h) * border.border_scaling;
        s * border.label_space_factor()
    }

    /// If the bug were present (label_space called with post-HowTo width),
    /// this is the wrong value that would be computed.
    fn buggy_label_h_post_howto(border: &emBorder, w: f64, h: f64) -> f64 {
        let (_, _, ow, oh) = border.outer_insets(w, h);
        let rnd_w = (w - ow).max(0.0);
        let rnd_h = (h - oh).max(0.0);
        let s = rnd_w.min(rnd_h) * border.border_scaling;
        let ms = s * border.min_space_factor();
        // Apply HowTo shift to rnd_w (the bug path).
        let mut post_w = rnd_w;
        if border.has_how_to {
            let hts = s * border.how_to_space_factor();
            if hts > ms {
                post_w -= hts - ms;
            }
        }
        // Buggy: recompute s from the narrower post-HowTo width.
        let s_post = post_w.min(rnd_h) * border.border_scaling;
        s_post * border.label_space_factor()
    }

    #[test]
    fn label_space_uses_pre_howto_s_in_content_rect() {
        // Tall panel where width is the constraining dimension for
        // s = min(rnd_w, rnd_h). The HowTo shift reduces rnd_w, so if
        // label_space were (incorrectly) called with post-HowTo width,
        // it would produce a smaller label_h.
        //
        // OuterBorderType::None has zero outer insets AND min_space_factor=0,
        // so how_to_space_factor (0.023) > min_space_factor (0.0) — the HowTo
        // shift is the full howToSpace amount.
        let border_howto = emBorder::new(OuterBorderType::None)
            .with_caption("Test")
            .with_how_to(true);

        let w = 100.0;
        let h = 200.0;

        let correct_lh = expected_label_h_pre_howto(&border_howto, w, h);
        let buggy_lh = buggy_label_h_post_howto(&border_howto, w, h);

        // Sanity: the bug would produce a noticeably different value.
        assert!(
            (correct_lh - buggy_lh).abs() > 0.1,
            "test inputs must produce a measurable difference: \
             correct={correct_lh}, buggy={buggy_lh}"
        );

        // content_rect: the label_h used should match the pre-HowTo value.
        let look = test_look();
        let r_howto = border_howto.GetContentRect(w, h, &look);

        // Without HowTo, same caption — label_h should be identical because
        // both use s = min(rnd_w, rnd_h) before any HowTo shift.
        let border_no_howto = emBorder::new(OuterBorderType::None)
            .with_caption("Test");
        let r_no_howto = border_no_howto.GetContentRect(w, h, &look);

        // With the fix, the label contribution to `y` is the same in both
        // cases (label_h = s * factor, s computed pre-HowTo).
        // The howto border shifts x rightward but does NOT change label_h.
        // So r_howto.y == r_no_howto.y.
        assert!(
            (r_howto.y - r_no_howto.y).abs() < 1e-10,
            "content_rect label_h must use pre-HowTo s: \
             with_howto.y={}, without_howto.y={}, diff={}",
            r_howto.y, r_no_howto.y, (r_howto.y - r_no_howto.y).abs()
        );

        // Also verify the y offset matches our expected pre-HowTo label_h.
        assert!(
            (r_howto.y - correct_lh).abs() < 1e-10,
            "content_rect y should equal pre-HowTo label_h: \
             y={}, expected={correct_lh}",
            r_howto.y
        );
    }

    #[test]
    fn label_space_uses_pre_howto_s_in_content_round_rect() {
        // Same setup as above but exercising content_round_rect.
        let border_howto = emBorder::new(OuterBorderType::None)
            .with_caption("Test")
            .with_how_to(true);
        let border_no_howto = emBorder::new(OuterBorderType::None)
            .with_caption("Test");

        let w = 100.0;
        let h = 200.0;
        let look = test_look();

        let (rr_howto, _) = border_howto.GetContentRoundRect(w, h, &look);
        let (rr_no_howto, _) = border_no_howto.GetContentRoundRect(w, h, &look);

        // label_h determines the y offset — it must be the same whether or not
        // HowTo is enabled, because s is computed pre-HowTo.
        assert!(
            (rr_howto.y - rr_no_howto.y).abs() < 1e-10,
            "content_round_rect label_h must use pre-HowTo s: \
             with_howto.y={}, without_howto.y={}, diff={}",
            rr_howto.y, rr_no_howto.y, (rr_howto.y - rr_no_howto.y).abs()
        );
    }

    #[test]
    fn label_space_uses_pre_howto_s_in_content_rect_unobscured() {
        // content_rect_unobscured has its own IO-field path (InputField).
        // The final y = rnd_y + d, where d depends on post-HowTo rnd_w
        // (legitimately). To isolate the label_h contribution, compare a
        // captioned vs non-captioned border, both with HowTo + InputField.
        // The y difference should equal the pre-HowTo label_h = s * 0.17.
        let border_cap = emBorder::new(OuterBorderType::None)
            .with_caption("Test")
            .with_how_to(true)
            .with_inner(InnerBorderType::InputField);
        let border_nocap = emBorder::new(OuterBorderType::None)
            .with_how_to(true)
            .with_inner(InnerBorderType::InputField);

        let w = 100.0;
        let h = 200.0;
        let look = test_look();

        let cu_cap = border_cap.GetContentRectUnobscured(w, h, &look);
        let cu_nocap = border_nocap.GetContentRectUnobscured(w, h, &look);

        // The y difference is label_h (plus downstream effects through rnd_h
        // on the IO radius bump). For OuterBorderType::None with ms=0:
        //   rnd_y_cap = label_h, rnd_y_nocap = 0
        //   rnd_h_cap = h - label_h, rnd_h_nocap = h
        // The IO bump d depends on min(rnd_w, rnd_h), and since rnd_h >> rnd_w
        // for our tall panel, rnd_w dominates and d is the same in both cases.
        // So y_diff ≈ label_h exactly.
        let expected_label_h = expected_label_h_pre_howto(&border_cap, w, h);
        let y_diff = cu_cap.y - cu_nocap.y;

        // With the buggy code, label_h would be smaller (using post-HowTo s).
        // With the fix, it must match the pre-HowTo value.
        assert!(
            (y_diff - expected_label_h).abs() < 0.01,
            "content_rect_unobscured label_h must use pre-HowTo s: \
             y_diff={y_diff}, expected_label_h={expected_label_h}, diff={}",
            (y_diff - expected_label_h).abs()
        );

        // Verify the buggy value would be measurably different.
        let buggy_label_h = buggy_label_h_post_howto(&border_cap, w, h);
        assert!(
            (expected_label_h - buggy_label_h).abs() > 0.1,
            "test inputs must produce detectable difference: \
             correct={expected_label_h}, buggy={buggy_label_h}"
        );
    }

    #[test]
    fn label_space_factor_is_accessible() {
        // Verify label_space_factor returns the expected values per border type.
        let group = emBorder::new(OuterBorderType::Group);
        assert!((group.label_space_factor() - 0.05).abs() < 1e-10);

        let rect = emBorder::new(OuterBorderType::Rect);
        assert!((rect.label_space_factor() - 0.17).abs() < 1e-10);
    }

    /// Regression test for MarginFilled painting the full panel area.
    ///
    /// The bug: MarginFilled used `paint_rect(ox, oy, w-2*ox, h-2*oy)` which
    /// left the margin corners showing canvas color instead of bg_color.
    /// The fix: `paint_rect(0, 0, w, h)` fills the entire panel, matching
    /// C++ `emBorder.cpp:628` which calls `painter->Clear(color, canvasColor)`.
    #[test]
    fn margin_filled_paints_full_panel_including_corners() {
        let bg = emColor::rgba(200, 100, 50, 255); // distinctive color
        let canvas = emColor::rgba(0, 0, 0, 255); // black canvas

        let mut look = test_look();
        look.bg_color = bg;

        let border = emBorder::new(OuterBorderType::MarginFilled);

        // For a 100x100 image, the margin inset d = s * 0.04 = 100 * 0.04 = 4.
        // Old buggy code: paint_rect(4, 4, 92, 92) — corners at (0,0) untouched.
        // Fixed code: paint_rect(0, 0, 100, 100) — entire panel filled.
        let mut img = emImage::new(100, 100, 4);
        // Fill the image with canvas color so unfilled pixels are distinguishable.
        img.fill(canvas);
        let mut painter = emPainter::new(&mut img);

        border.paint_border(&mut painter, 100.0, 100.0, &look, false, true, 1.0);
        drop(painter);

        // Corner pixels must be bg_color, not canvas color.
        let tl = img.GetPixel(0, 0);
        assert_eq!(
            [tl[0], tl[1], tl[2], tl[3]],
            [bg.GetRed(), bg.GetGreen(), bg.GetBlue(), bg.GetAlpha()],
            "top-left corner (0,0) should be bg_color, not canvas"
        );

        let br = img.GetPixel(99, 99);
        assert_eq!(
            [br[0], br[1], br[2], br[3]],
            [bg.GetRed(), bg.GetGreen(), bg.GetBlue(), bg.GetAlpha()],
            "bottom-right corner (99,99) should be bg_color, not canvas"
        );

        // Also check a pixel inside the old inset area to confirm it's still filled.
        let mid = img.GetPixel(50, 50);
        assert_eq!(
            [mid[0], mid[1], mid[2], mid[3]],
            [bg.GetRed(), bg.GetGreen(), bg.GetBlue(), bg.GetAlpha()],
            "center pixel should be bg_color"
        );
    }

    // --- description-only label_layout total_w tests (Fix 26) ---

    #[test]
    fn desc_only_label_width_not_hardcoded() {
        // A border with description only (no caption, no icon) should compute
        // total_w from the actual description text width, not fall back to 1.0.
        // With a wide area and short text, the label rect should be narrower
        // than the full area width — proving the text measurement is used.
        let border = emBorder::new(OuterBorderType::None).with_description("X");
        let area_w = 1000.0;
        let area_h = 100.0;
        let layout = border.label_layout(0.0, 0.0, area_w, area_h);
        let desc_rect = layout
            .description_rect
            .expect("description-only border must produce a description_rect");
        // If total_w were hardcoded to 1.0, then f * total_w = (area_h / 0.15) * 1.0
        // = 666.67, which is less than area_w=1000 so label_w = 666.67.
        // With the fix, total_w = get_text_size("X", 1.0).0 which is
        // 1 / CHAR_BOX_TALLNESS (~0.55), giving f * total_w = 666.67 * 0.55 = ~366.
        // The key: the actual value must differ from the buggy 1.0 path.
        let buggy_total_w = 1.0;
        let total_units = 0.15; // desc_units only
        let f = area_h / total_units;
        let buggy_label_w = (f * buggy_total_w).min(area_w);
        assert!(
            (desc_rect.w - buggy_label_w).abs() > 1.0,
            "desc_rect.w ({}) must differ from buggy hardcoded width ({}); \
             total_w should reflect actual text measurement",
            desc_rect.w,
            buggy_label_w
        );
        // Also: the actual width must be positive and less than area_w.
        assert!(desc_rect.w > 0.0, "desc_rect.w must be positive");
        assert!(
            desc_rect.w < area_w,
            "desc_rect.w ({}) should be less than area_w ({})",
            desc_rect.w,
            area_w
        );
    }

    #[test]
    fn desc_only_longer_text_wider_layout() {
        // Longer description text should produce a wider label rect
        // than shorter text, proving total_w depends on text width.
        let short = emBorder::new(OuterBorderType::None).with_description("Hi");
        let long = emBorder::new(OuterBorderType::None)
            .with_description("This is a much longer description text");
        let area_w = 5000.0; // very wide so labels don't clamp to area_w
        let area_h = 100.0;
        let short_layout = short.label_layout(0.0, 0.0, area_w, area_h);
        let long_layout = long.label_layout(0.0, 0.0, area_w, area_h);
        let short_w = short_layout
            .description_rect
            .expect("short desc must produce rect")
            .w;
        let long_w = long_layout
            .description_rect
            .expect("long desc must produce rect")
            .w;
        assert!(
            long_w > short_w,
            "longer description ({long_w}) should produce wider label than short ({short_w})"
        );
    }

    /// Regression test: HowTo pill text visibility must account for pixel_scale.
    ///
    /// The fix changed the check from `tw * th > 100.0` to
    /// `tw * th * pixel_scale > 100.0`.  We pick dimensions where tw*th ≈ 77
    /// (below 100), so without pixel_scale the pill text is always hidden.
    /// With a large pixel_scale (100.0) it should be visible; with a tiny one
    /// (0.01) it should remain hidden.  If pixel_scale is ignored (the old bug),
    /// both renders are identical and the test fails.
    #[test]
    fn howto_pill_text_respects_pixel_scale() {
        // w=h=300 → s=300, hts=6.9, tw=6.21, th=12.42, tw*th ≈ 77.1
        // pixel_scale=100 → 77.1*100 = 7710 > 100 → text painted
        // pixel_scale=0.01 → 77.1*0.01 = 0.77 < 100 → text hidden
        let w = 300.0;
        let h = 300.0;
        let img_w = 300_u32;
        let img_h = 300_u32;

        let mut border = emBorder::new(OuterBorderType::None).with_how_to(true);
        border.set_how_to_text("Zoom".to_string());

        let mut look = test_look();
        // Ensure fg_color is fully opaque so the text actually writes pixels.
        look.fg_color = emColor::rgba(255, 255, 255, 255);

        // Render with large pixel_scale (HowTo text should appear).
        let mut img_large = emImage::new(img_w, img_h, 4);
        img_large.fill(emColor::rgba(0, 0, 0, 0));
        {
            let mut painter = emPainter::new(&mut img_large);
            border.paint_border(&mut painter, w, h, &look, false, true, 100.0);
        }

        // Render with tiny pixel_scale (HowTo text should be hidden).
        let mut img_small = emImage::new(img_w, img_h, 4);
        img_small.fill(emColor::rgba(0, 0, 0, 0));
        {
            let mut painter = emPainter::new(&mut img_small);
            border.paint_border(&mut painter, w, h, &look, false, true, 0.01);
        }

        // The two buffers must differ — the large-scale render includes text
        // that the small-scale render omits.
        assert_ne!(
            img_large.GetMap(),
            img_small.GetMap(),
            "HowTo pill text should be visible at pixel_scale=100.0 but hidden \
             at pixel_scale=0.01; buffers must differ"
        );
    }
}
