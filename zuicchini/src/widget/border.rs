use crate::foundation::{Color, Image, Rect};
use crate::render::{Painter, Stroke, TextAlignment, VAlign, BORDER_EDGES_ONLY};

use super::look::Look;

/// Minimum font size in pixels — below this the text is too small to read.
const MIN_FONT_SIZE: f64 = 4.0;

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
    _caption_font_size: f64,
    _description_font_size: f64,
}

/// Border chrome helper. Embedded in widgets to draw surrounding decoration.
pub struct Border {
    pub outer: OuterBorderType,
    pub inner: InnerBorderType,
    pub caption: String,
    pub description: String,
    pub border_scaling: f64,
    pub label_alignment: TextAlignment,
    pub caption_alignment: Option<TextAlignment>,
    pub description_alignment: Option<TextAlignment>,
    pub icon: Option<Image>,
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
    pub(crate) aux_panel_name: Option<String>,
    /// Height/width ratio of the auxiliary area (default 1.0 when absent).
    pub(crate) aux_tallness: f64,
}

impl Border {
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
        }
    }

    pub fn with_caption(mut self, caption: &str) -> Self {
        self.caption = caption.to_string();
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    pub fn with_inner(mut self, inner: InnerBorderType) -> Self {
        self.inner = inner;
        self
    }

    pub fn set_outer_border_type(&mut self, obt: OuterBorderType) {
        self.outer = obt;
    }

    pub fn set_inner_border_type(&mut self, ibt: InnerBorderType) {
        self.inner = ibt;
    }

    pub fn set_border_type(&mut self, obt: OuterBorderType, ibt: InnerBorderType) {
        self.outer = obt;
        self.inner = ibt;
    }

    pub fn with_border_scaling(mut self, s: f64) -> Self {
        self.border_scaling = s.max(1e-10);
        self
    }

    pub fn set_border_scaling(&mut self, s: f64) {
        self.border_scaling = s.max(1e-10);
    }

    pub fn with_label_alignment(mut self, a: TextAlignment) -> Self {
        self.label_alignment = a;
        self
    }

    pub fn set_label_alignment(&mut self, a: TextAlignment) {
        self.label_alignment = a;
    }

    pub fn with_caption_alignment(mut self, a: TextAlignment) -> Self {
        self.caption_alignment = Some(a);
        self
    }

    pub fn set_caption_alignment(&mut self, a: Option<TextAlignment>) {
        self.caption_alignment = a;
    }

    pub fn with_description_alignment(mut self, a: TextAlignment) -> Self {
        self.description_alignment = Some(a);
        self
    }

    pub fn set_description_alignment(&mut self, a: Option<TextAlignment>) {
        self.description_alignment = a;
    }

    pub fn with_icon(mut self, icon: Image) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon;
    }

    pub fn set_icon_above_caption(&mut self, above: bool) {
        self.icon_above_caption = above;
    }

    pub fn set_max_icon_area_tallness(&mut self, t: f64) {
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

    /// Set whether the label is rendered inside the border.
    ///
    /// C++ equivalent: `emBorder::SetLabelInBorder`.
    pub fn set_label_in_border(&mut self, in_border: bool) {
        self.label_in_border = in_border;
    }

    /// Create or update the auxiliary panel area.
    ///
    /// If aux data does not exist yet, it is created. If it does exist,
    /// `panel_name` and `tallness` are updated independently only when they
    /// differ from the current values.
    ///
    /// C++ equivalent: `emBorder::HaveAux`.
    pub fn have_aux(&mut self, panel_name: &str, tallness: f64) {
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
    pub fn remove_aux(&mut self) {
        self.aux_panel_name = None;
        self.aux_tallness = 1.0;
    }

    /// Return the auxiliary panel name, or an empty string if no aux data.
    ///
    /// C++ equivalent: `emBorder::GetAuxPanelName`.
    pub fn get_aux_panel_name(&self) -> &str {
        match self.aux_panel_name {
            Some(ref name) => name.as_str(),
            None => "",
        }
    }

    /// Return the auxiliary area tallness, or `1.0` if no aux data.
    ///
    /// C++ equivalent: `emBorder::GetAuxTallness`.
    pub fn get_aux_tallness(&self) -> f64 {
        if self.aux_panel_name.is_some() {
            self.aux_tallness
        } else {
            1.0
        }
    }

    /// Return whether an auxiliary panel is configured.
    ///
    /// In C++ `GetAuxPanel` returned a panel pointer by walking the child tree
    /// and caching the result. Rust `Border` is not a panel, so this method
    /// returns whether aux data exists. The caller can use
    /// [`get_aux_panel_name`](Self::get_aux_panel_name) to resolve the panel by
    /// name in the widget tree.
    ///
    /// C++ equivalent: `emBorder::GetAuxPanel` (structural adaptation).
    pub fn has_aux_panel(&self) -> bool {
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
    pub fn get_aux_rect(&self, w: f64, h: f64) -> Option<Rect> {
        self.aux_panel_name.as_ref()?;

        let (ox, oy, ow, oh) = self.outer_insets(w, h);
        let rnd_x = ox;
        let rnd_y = oy;
        let rnd_w = (w - ow).max(0.0);
        let rnd_h = (h - oh).max(0.0);

        if self.label_in_border && self.has_label() {
            // Label path: aux is placed at the right of the label text area.
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

    pub(crate) fn has_label(&self) -> bool {
        !self.caption.is_empty() || !self.description.is_empty() || self.icon.is_some()
    }

    /// Best (natural) height-to-width ratio of the label.
    ///
    /// C++ equivalent: `emBorder::GetBestLabelTallness()`.
    pub(crate) fn best_label_tallness(&self) -> f64 {
        let has_cap = !self.caption.is_empty();
        let has_desc = !self.description.is_empty();

        let cap_units: f64 = if has_cap { 1.0 } else { 0.0 };
        let desc_units: f64 = if has_desc { 0.15 } else { 0.0 };

        let gap2_units: f64 = if has_cap && has_desc {
            desc_units * 0.05
        } else {
            0.0
        };
        let total_h = cap_units + gap2_units + desc_units;
        if total_h <= 0.0 {
            return 1.0;
        }

        let cap_w = if has_cap {
            Painter::measure_text_width(&self.caption, 1.0)
        } else {
            0.0
        };
        let desc_w = if has_desc {
            Painter::measure_text_width(&self.description, 0.15)
        } else {
            0.0
        };
        let total_w = cap_w.max(desc_w).max(1e-100);
        total_h / total_w
    }

    /// Horizontal offset for positioning a block of width `block_w` within
    /// an available width of `avail_w`, according to [`label_alignment`].
    ///
    /// C++ equivalent: horizontal shift in `DoLabel` after scaling, using
    /// `LabelAlignment` to left-/center-/right-align the label block.
    fn block_offset(&self, avail_w: f64, block_w: f64) -> f64 {
        let slack = (avail_w - block_w).max(0.0);
        match self.label_alignment {
            TextAlignment::Left => 0.0,
            TextAlignment::Center => slack * 0.5,
            TextAlignment::Right => slack,
        }
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
    fn inner_radius(&self, iw: f64, ih: f64) -> f64 {
        let s = iw.min(ih) * self.border_scaling;
        match self.inner {
            InnerBorderType::Group => s * 0.0188,
            InnerBorderType::InputField | InnerBorderType::OutputField => s * 0.094,
            InnerBorderType::CustomRect => s * 0.0125,
            InnerBorderType::None => 0.0,
        }
    }

    /// The label-space factor, which differs by border type.
    /// Eagle Mode: Group uses 0.05, all others use 0.17.
    fn label_space_factor(&self) -> f64 {
        match self.outer {
            OuterBorderType::Group => 0.05,
            _ => 0.17,
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
    /// Dimensions are computed proportionally from the available `area_h`,
    /// matching Eagle Mode's DoLabel algorithm. The font size scales with the
    /// available space rather than being hardcoded.
    fn label_layout(&self, area_x: f64, area_y: f64, area_w: f64, area_h: f64) -> LabelLayout {
        let has_cap = !self.caption.is_empty();
        let has_desc = !self.description.is_empty();
        let icon = self.icon.as_ref().filter(|img| !img.is_empty());

        // Count "rows" to distribute height among: caption=1, description=0.15 relative.
        // Eagle Mode: description height = capH * 0.15.
        let cap_units: f64 = if has_cap { 1.0 } else { 0.0 };
        let desc_units: f64 = if has_desc { 0.15 } else { 0.0 };

        if icon.is_none() {
            // Text-only layout: distribute area_h among caption + gap2 + description.
            // C++ gap2 = descH * 0.05 when both caption and description exist.
            let gap2_units: f64 = if has_cap && has_desc {
                desc_units * 0.05
            } else {
                0.0
            };
            let total_units = cap_units + gap2_units + desc_units;
            if total_units <= 0.0 {
                return LabelLayout {
                    icon_rect: None,
                    caption_rect: None,
                    description_rect: None,
                    total_height: 0.0,
                    _caption_font_size: 0.0,
                    _description_font_size: 0.0,
                };
            }
            let cap_h = area_h * cap_units / total_units;
            let gap2 = area_h * gap2_units / total_units;
            let desc_h = area_h * desc_units / total_units;
            // Font size is ~80% of the row height (leaving padding).
            let cap_font = (cap_h * 0.8).max(MIN_FONT_SIZE);
            let desc_font = (desc_h * 0.8).max(MIN_FONT_SIZE);

            let cap_rect = if has_cap {
                Some(Rect {
                    x: area_x,
                    y: area_y,
                    w: area_w,
                    h: cap_h,
                })
            } else {
                None
            };
            let desc_rect = if has_desc {
                Some(Rect {
                    x: area_x,
                    y: area_y + cap_h + gap2,
                    w: area_w,
                    h: desc_h,
                })
            } else {
                None
            };
            return LabelLayout {
                icon_rect: None,
                caption_rect: cap_rect,
                description_rect: desc_rect,
                total_height: cap_h + gap2 + desc_h,
                _caption_font_size: cap_font,
                _description_font_size: desc_font,
            };
        }

        let img = icon.expect("checked above");
        let img_w = img.width().max(1) as f64;
        let img_h = img.height().max(1) as f64;
        let icon_tallness = (img_h / img_w).min(self.max_icon_area_tallness);

        if self.icon_above_caption {
            // Icon takes 3 "rows" worth, gap is 0.1 rows, caption 1 row, desc 0.15 rows.
            // gap2 = descH * 0.05 between caption/icon and description.
            let gap2_units: f64 = if has_desc { desc_units * 0.05 } else { 0.0 };
            let total_units = 3.0 + 0.1 + cap_units + gap2_units + desc_units;
            let unit = area_h / total_units;
            let icon_h = 3.0 * unit;
            let icon_w = icon_h / icon_tallness;
            let gap = 0.1 * unit;
            let cap_h = cap_units * unit;
            let gap2 = gap2_units * unit;
            let desc_h = desc_units * unit;
            let cap_font = (cap_h * 0.8).max(MIN_FONT_SIZE);
            let desc_font = (desc_h * 0.8).max(MIN_FONT_SIZE);

            // Block width is the widest element. Text spans area_w; icon may
            // be narrower. Compute a block-level horizontal offset using
            // label_alignment so the icon tracks the block position rather
            // than always centering.
            let block_w = area_w.max(icon_w);
            let block_x = area_x + self.block_offset(area_w, block_w);

            let icon_x = match self.label_alignment {
                TextAlignment::Left => block_x,
                TextAlignment::Center => block_x + (block_w - icon_w) / 2.0,
                TextAlignment::Right => block_x + block_w - icon_w,
            };
            let icon_rect = Rect {
                x: icon_x,
                y: area_y,
                w: icon_w,
                h: icon_h,
            };
            let mut y = area_y + icon_h + gap;
            let cap_rect = if has_cap {
                let r = Rect {
                    x: block_x,
                    y,
                    w: block_w,
                    h: cap_h,
                };
                y += cap_h;
                Some(r)
            } else {
                None
            };
            y += gap2;
            let desc_rect = if has_desc {
                Some(Rect {
                    x: block_x,
                    y,
                    w: block_w,
                    h: desc_h,
                })
            } else {
                None
            };
            let total = icon_h + gap + cap_h + gap2 + desc_h;
            LabelLayout {
                icon_rect: Some(icon_rect),
                caption_rect: cap_rect,
                description_rect: desc_rect,
                total_height: total,
                _caption_font_size: cap_font,
                _description_font_size: desc_font,
            }
        } else {
            // Icon beside caption: icon is 1 "row", gap 0.1 rows.
            // gap2 = descH * 0.05 between caption/icon and description.
            let gap2_units: f64 = if has_desc { desc_units * 0.05 } else { 0.0 };
            let text_units = cap_units + gap2_units + desc_units;
            let icon_h = area_h;
            let icon_w = icon_h / icon_tallness;
            let gap = area_h * 0.1 / (1.0 + 0.1);

            // Block width = icon + gap + text region. May be narrower than
            // area_w when the icon is small. Apply block-level alignment.
            let block_w = icon_w + gap + (area_w - icon_w - gap).max(0.0);
            let block_x = area_x + self.block_offset(area_w, block_w);

            let text_x = block_x + icon_w + gap;
            let text_w = (block_w - icon_w - gap).max(0.0);
            let cap_h = if text_units > 0.0 {
                area_h * cap_units / text_units
            } else {
                0.0
            };
            let gap2 = if text_units > 0.0 {
                area_h * gap2_units / text_units
            } else {
                0.0
            };
            let desc_h = if text_units > 0.0 {
                area_h * desc_units / text_units
            } else {
                0.0
            };
            let cap_font = (cap_h * 0.8).max(MIN_FONT_SIZE);
            let desc_font = (desc_h * 0.8).max(MIN_FONT_SIZE);

            let icon_rect = Rect {
                x: block_x,
                y: area_y,
                w: icon_w,
                h: icon_h,
            };
            let cap_rect = if has_cap {
                Some(Rect {
                    x: text_x,
                    y: area_y,
                    w: text_w,
                    h: cap_h,
                })
            } else {
                None
            };
            let desc_rect = if has_desc {
                Some(Rect {
                    x: text_x,
                    y: area_y + cap_h + gap2,
                    w: text_w,
                    h: desc_h,
                })
            } else {
                None
            };
            let total = icon_h.max(cap_h + gap2 + desc_h);
            LabelLayout {
                icon_rect: Some(icon_rect),
                caption_rect: cap_rect,
                description_rect: desc_rect,
                total_height: total,
                _caption_font_size: cap_font,
                _description_font_size: desc_font,
            }
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
  panel.\n\
\n\
* Press the cursor keys to move the focus to a sister panel in the desired\n\
  direction.\n\
\n\
* Press Page-Up or -Down to move the focus to a child or parent panel.\n";

    /// Build the how-to text for this border.
    ///
    /// Returns the preface, optionally appending the disabled and/or focus
    /// sections based on the panel state flags passed in. Callers (widget
    /// behaviors) supply the state because `Border` itself is not a panel.
    ///
    /// C++ equivalent: `emBorder::GetHowTo`.
    pub(crate) fn get_howto(&self, enabled: bool, focusable: bool) -> String {
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
    pub fn is_opaque(&self, look: &Look) -> bool {
        match self.outer {
            OuterBorderType::Filled
            | OuterBorderType::MarginFilled
            | OuterBorderType::PopupRoot => look.bg_color.is_opaque(),
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
    pub fn substance_round_rect(&self, w: f64, h: f64) -> (Rect, f64) {
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
                let d = s * 0.006;
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
                let d = s * 0.006; // half-stroke
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

    /// Returns the content area as a round rectangle with corner radius.
    /// Unlike [`content_rect`](Self::content_rect) which returns the axis-aligned
    /// inscribed rectangle, this returns the round-rect boundary and its radius so
    /// callers can perform round-rect hit-testing or clipping.
    ///
    /// Returns `(rect, corner_radius)`.
    ///
    /// C++ equivalent: `emBorder::GetContentRoundRect`
    /// (via `DoBorder(BORDER_FUNC_CONTENT_ROUND_RECT)`).
    pub fn content_round_rect(&self, w: f64, h: f64, _look: &Look) -> (Rect, f64) {
        let (ox, oy, ow, oh) = self.outer_insets(w, h);
        let label_area_w = (w - ow).max(0.0);
        let rnd_h = (h - oh).max(0.0);
        let label_h = if self.label_in_border && self.has_label() {
            self.label_space(label_area_w, rnd_h)
        } else {
            0.0
        };

        // Round rect after outer insets + label.
        // `rnd_h` (from above) = h - oh, the pre-label outer height.
        let rnd_h_after_label = (rnd_h - label_h).max(0.0);
        // minSpace: C++ computes s = min(rndW, rndH)*BorderScaling BEFORE label
        // subtraction (line 901), so ms uses the pre-label dimensions.
        let ms = label_area_w.min(rnd_h) * self.border_scaling * self.min_space_factor();
        let rnd_x = ox + ms;
        let rnd_w = (label_area_w - 2.0 * ms).max(0.0);
        // C++ lines 983-987 (has-label) vs 1047-1050 (no-label):
        //   has-label: rndY+=labelSpace (no ms on top), rndH-=labelSpace+ms (1 ms, bottom only)
        //   no-label:  rndY+=ms (symmetric top+bottom), rndH-=2*ms
        let (rnd_y, rnd_h_inner) = if label_h > 0.0 {
            (oy + label_h, (rnd_h_after_label - ms).max(0.0))
        } else {
            (oy + ms, (rnd_h_after_label - 2.0 * ms).max(0.0))
        };
        let mut rnd_r = (self.outer_radius(w, h) - ms).max(0.0);

        // Inner border processing: adjust round rect.
        let inner_s = rnd_w.min(rnd_h_inner) * self.border_scaling;
        match self.inner {
            InnerBorderType::None => {}
            InnerBorderType::Group => {
                let r = inner_s * 0.0188;
                if rnd_r < r {
                    rnd_r = r;
                }
            }
            InnerBorderType::InputField | InnerBorderType::OutputField => {
                let r = inner_s * 0.094;
                // C++ line 1093: if (rndR<r) rndR=r;
                let adjusted_r = rnd_r.max(r);
                // C++ line 1094: d=(1-(216.0-16.0)/216.0)*rndR = (16/216)*rndR
                let d = (16.0 / 216.0) * adjusted_r;
                // C++ lines 1095-1099: tx=rndX+d; ty=rndY+d; tw=rndW-2*d; th=rndH-2*d; tr=rndR-d
                return (
                    Rect {
                        x: rnd_x + d,
                        y: rnd_y + d,
                        w: (rnd_w - 2.0 * d).max(0.0),
                        h: (rnd_h_inner - 2.0 * d).max(0.0),
                    },
                    adjusted_r - d,
                );
            }
            InnerBorderType::CustomRect => {
                let r = inner_s * 0.0125;
                if rnd_r < r {
                    rnd_r = r;
                }
            }
        }

        let (ix, iy, iw, ih) = self.inner_insets(rnd_w, rnd_h_inner);
        let ir = self.inner_radius(rnd_w, rnd_h_inner);
        let final_r = if self.inner != InnerBorderType::None {
            ir
        } else {
            rnd_r
        };
        (
            Rect {
                x: rnd_x + ix,
                y: rnd_y + iy,
                w: (rnd_w - iw).max(0.0),
                h: (rnd_h_inner - ih).max(0.0),
            },
            final_r.max(0.0),
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
    pub fn content_rect_unobscured(&self, w: f64, h: f64, look: &Look) -> Rect {
        match self.inner {
            InnerBorderType::InputField | InnerBorderType::OutputField => {
                // IO fields have an overlay border that obscures a strip along
                // each edge. C++ computes d = 220/216 * rndR, then insets by d.
                let cr = self.content_rect(w, h, look);
                let inner_s = cr.w.min(cr.h) * self.border_scaling;
                let rnd_r = inner_s * 0.094;
                let d = rnd_r * 220.0 / 216.0;
                Rect {
                    x: cr.x + d,
                    y: cr.y + d,
                    w: (cr.w - 2.0 * d).max(0.0),
                    h: (cr.h - 2.0 * d).max(0.0),
                }
            }
            _ => self.content_rect(w, h, look),
        }
    }

    /// Compute the content area after border and label insets.
    pub fn content_rect(&self, w: f64, h: f64, _look: &Look) -> Rect {
        let (ox, oy, ow, oh) = self.outer_insets(w, h);
        let label_area_w = (w - ow).max(0.0);
        let rnd_h = (h - oh).max(0.0);
        let label_h = if self.label_in_border && self.has_label() {
            self.label_space(label_area_w, rnd_h)
        } else {
            0.0
        };
        let rnd_w = (w - ow).max(0.0);
        let rnd_h_after_label = (h - oh - label_h).max(0.0);
        // minSpace: padding between outer decoration and content area.
        let ms = rnd_w.min(rnd_h_after_label) * self.border_scaling * self.min_space_factor();
        let iw = (rnd_w - 2.0 * ms).max(0.0);
        let ih = (rnd_h_after_label - 2.0 * ms).max(0.0);
        let (ix, iy, inner_w, inner_h) = self.inner_insets(iw, ih);

        Rect {
            x: ox + ms + ix,
            y: oy + label_h + ms + iy,
            w: (iw - inner_w).max(0.0),
            h: (ih - inner_h).max(0.0),
        }
    }

    /// Preferred size to fit the given content size.
    pub fn preferred_size_for_content(&self, cw: f64, ch: f64) -> (f64, f64) {
        let (_, _, ow, oh) = self.outer_insets(cw, ch);
        let label_area_w = cw;
        let rnd_h = (ch - oh).max(0.0);
        let label_h = if self.label_in_border && self.has_label() {
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
    pub fn paint_label(&self, painter: &mut Painter, area: Rect, look: &Look, enabled: bool) {
        let dim_color = |c: Color| -> Color {
            if enabled {
                c
            } else {
                c.with_alpha((c.a() as u16 * 64 / 255) as u8)
            }
        };
        self.paint_label_impl(painter, area, look, &dim_color);
    }

    /// Paint the label with a custom text color (used by Button for button_fg_color).
    pub fn paint_label_colored(
        &self,
        painter: &mut Painter,
        area: Rect,
        look: &Look,
        color: Color,
        enabled: bool,
    ) {
        let dim_color = move |_c: Color| -> Color {
            if enabled {
                color
            } else {
                color.with_alpha((color.a() as u16 * 64 / 255) as u8)
            }
        };
        self.paint_label_impl(painter, area, look, &dim_color);
    }

    /// Internal helper that paints the label components (icon, caption,
    /// description) into the given area.
    fn paint_label_impl(
        &self,
        painter: &mut Painter,
        area: Rect,
        look: &Look,
        dim_color: &dyn Fn(Color) -> Color,
    ) {
        let label = self.label_layout(area.x, area.y, area.w, area.h);

        let cap_align = self.caption_alignment.unwrap_or(self.label_alignment);
        let desc_align = self.description_alignment.unwrap_or(self.label_alignment);

        // Icon
        if let Some(ref icon_rect) = label.icon_rect {
            if let Some(ref img) = self.icon {
                if !img.is_empty() {
                    if img.channel_count() == 1 {
                        painter.paint_image_colored(
                            icon_rect.x,
                            icon_rect.y,
                            icon_rect.w,
                            icon_rect.h,
                            img,
                            0,
                            0,
                            img.width(),
                            img.height(),
                            Color::TRANSPARENT,
                            dim_color(look.fg_color),
                            Color::TRANSPARENT,
                        );
                    } else {
                        painter.paint_image_scaled(
                            icon_rect.x,
                            icon_rect.y,
                            icon_rect.w,
                            icon_rect.h,
                            img,
                            crate::render::ImageQuality::Bilinear,
                            crate::render::ImageExtension::Clamp,
                        );
                    }
                }
            }
        }

        // Caption — C++ DoLabel proportional scaling: compute a uniform scale
        // factor `f` that fits both width and height, maintaining aspect ratio.
        if let Some(ref cr) = label.caption_rect {
            let (natural_tw, natural_th) = Painter::get_text_size(&self.caption, 1.0, false, 0.0);
            let cap_font = if natural_tw > 0.0 && natural_th > 0.0 {
                let mut f = cr.h / natural_th;
                let w2 = f * natural_tw;
                if w2 > cr.w {
                    let min_ws = 0.5;
                    let min_total_w = natural_tw * min_ws;
                    if f * min_total_w > cr.w {
                        f = cr.w / min_total_w;
                    }
                }
                f.max(0.001)
            } else {
                label._caption_font_size
            };
            painter.paint_text_boxed(
                cr.x,
                cr.y,
                cr.w,
                cr.h,
                &self.caption,
                cap_font,
                dim_color(look.fg_color),
                Color::TRANSPARENT,
                cap_align,
                VAlign::Center,
                cap_align,
                0.5,
                false,
                0.0,
            );
        }

        // Description
        if let Some(ref dr) = label.description_rect {
            painter.paint_text_boxed(
                dr.x,
                dr.y,
                dr.w,
                dr.h,
                &self.description,
                label._description_font_size,
                dim_color(look.fg_color.darken(0.3)),
                Color::TRANSPARENT,
                desc_align,
                VAlign::Center,
                desc_align,
                0.5,
                false,
                0.0,
            );
        }
    }

    /// Paint the border chrome.
    pub fn paint_border(
        &self,
        painter: &mut Painter,
        w: f64,
        h: f64,
        look: &Look,
        _focused: bool,
        enabled: bool,
    ) {
        // Dimming for disabled state: C++ "GetTransparented(75.0)" ~ alpha * 0.25.
        let dim_color = |c: crate::foundation::Color| -> crate::foundation::Color {
            if enabled {
                c
            } else {
                c.with_alpha((c.a() as u16 * 64 / 255) as u8)
            }
        };

        // Outer border
        match self.outer {
            OuterBorderType::None => {}
            OuterBorderType::Filled => {
                painter.paint_rect(0.0, 0.0, w, h, look.bg_color);
            }
            OuterBorderType::Margin => {}
            OuterBorderType::MarginFilled => {
                let (ox, oy, _, _) = self.outer_insets(w, h);
                painter.paint_rect(ox, oy, w - 2.0 * ox, h - 2.0 * oy, look.bg_color);
            }
            OuterBorderType::Rect => {
                // C++ DoBorder: margin d, stroke e, fill at (d,d), outline centered on fill edge.
                let s = self.base_unit(w, h);
                let d = s * 0.023;
                let e = s * 0.02;
                painter.paint_rect(d, d, w - 2.0 * d, h - 2.0 * d, look.bg_color);
                // C++ updates canvasColor to bg_color after fill.
                painter.set_canvas_color(look.bg_color);
                let color = dim_color(look.fg_color);
                let sd = d + e * 0.5;
                painter.paint_rect_outlined(
                    sd,
                    sd,
                    w - 2.0 * sd,
                    h - 2.0 * sd,
                    &Stroke::new(color, e),
                );
            }
            OuterBorderType::RoundRect => {
                // C++ DoBorder: margin d, stroke e, radius f, fill at (d,d), outline centered.
                let s = self.base_unit(w, h);
                let d = s * 0.023;
                let e = s * 0.02;
                let r = s * 0.22;
                painter.paint_round_rect(d, d, w - 2.0 * d, h - 2.0 * d, r, look.bg_color);
                painter.set_canvas_color(look.bg_color);
                let color = dim_color(look.fg_color);
                let sd = d + e * 0.5;
                let sr = r - e * 0.5;
                painter.paint_round_rect_outlined(
                    sd,
                    sd,
                    w - 2.0 * sd,
                    h - 2.0 * sd,
                    sr,
                    &Stroke::new(color, e),
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
                let mut color2 = painter.canvas_color();
                if !color.is_transparent() && (!color2.is_opaque() || color2 != color) {
                    let r = rnd_r * (280.0 / 209.0);
                    let e = r - rnd_r;
                    painter.paint_round_rect(
                        rnd_x - e,
                        rnd_y - e,
                        rnd_w + 2.0 * e,
                        rnd_h + 2.0 * e,
                        r,
                        color,
                    );
                    color2 = Color::TRANSPARENT;
                }
                let r = rnd_r * (286.0 / 209.0);
                let e = r - rnd_r;
                super::toolkit_images::with_toolkit_images(|img| {
                    painter.paint_border_image(
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
                if !color.is_transparent() {
                    painter.set_canvas_color(color);
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
                let mut color2 = painter.canvas_color();
                if !color.is_transparent() && (!color2.is_opaque() || color2 != color) {
                    let r = rnd_r * (280.0 / 209.0);
                    let e = r - rnd_r;
                    painter.paint_round_rect(
                        rnd_x - e,
                        rnd_y - e,
                        rnd_w + 2.0 * e,
                        rnd_h + 2.0 * e,
                        r,
                        color,
                    );
                    color2 = Color::TRANSPARENT;
                }
                let r = rnd_r * (286.0 / 209.0);
                let e = r - rnd_r;
                super::toolkit_images::with_toolkit_images(|img| {
                    painter.paint_border_image(
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
                if !color.is_transparent() {
                    painter.set_canvas_color(color);
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
                let mut color2 = painter.canvas_color();
                if !color.is_transparent() && (!color2.is_opaque() || color2 != color) {
                    let r = rnd_r * (336.0 / 293.4);
                    let e = r - rnd_r;
                    painter.paint_round_rect(
                        rnd_x - e,
                        rnd_y - e,
                        rnd_w + 2.0 * e,
                        rnd_h + 2.0 * e,
                        r,
                        color,
                    );
                    color2 = Color::TRANSPARENT;
                }
                let r = rnd_r * (340.0 / 293.4);
                let e = r - rnd_r;
                super::toolkit_images::with_toolkit_images(|img| {
                    painter.paint_border_image(
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
                if !color.is_transparent() {
                    painter.set_canvas_color(color);
                }
            }
            OuterBorderType::PopupRoot => {
                let s = self.base_unit(w, h);
                let d = s * 0.006;
                let color = look.bg_color;
                let canvas = painter.canvas_color();
                if !color.is_transparent() {
                    painter.paint_rect(0.0, 0.0, w, h, color);
                    painter.set_canvas_color(color);
                }
                let r = d; // C++ ratio 159.0/159.0 = 1.0
                let cc = if !color.is_transparent() {
                    color
                } else {
                    canvas
                };
                super::toolkit_images::with_toolkit_images(|img| {
                    painter.paint_border_image(
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

        // Label area — only painted when label_in_border is true.
        let (ox, oy, ow, oh) = self.outer_insets(w, h);
        let label_area_w = (w - ow).max(0.0);
        let rnd_h = (h - oh).max(0.0);
        let ls = if self.label_in_border {
            self.label_space(label_area_w, rnd_h)
        } else {
            0.0
        };

        // minSpace: C++ emBorder.cpp line 901-902: s=emMin(rndW,rndH)*BorderScaling; minSpace*=s.
        let ms = label_area_w.min(rnd_h) * self.border_scaling * self.min_space_factor();

        if self.label_in_border && self.has_label() {
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
                    ox + e_label,
                    oy + d_label,
                    (label_area_w - 2.0 * e_label).max(0.0),
                    lch,
                ),
                look,
                &dim_color,
            );
        }

        // Inner border — apply minSpace (C++ DoBorder lines 983-987).
        // C++: rndX += minSpace, rndW -= 2*minSpace, rndY += labelSpace,
        //      rndH -= labelSpace + minSpace, rndR -= minSpace.
        let inner_x = ox + ms;
        let inner_y = oy + ls;
        let inner_w = (w - ox * 2.0 - 2.0 * ms).max(0.0);
        let inner_h = (h - oy * 2.0 - ls - ms).max(0.0);
        // C++ rndR = outer_radius - minSpace, then clamped up per inner type.
        let mut inner_r = (self.outer_radius(w, h) - ms).max(0.0);
        let type_r = self.inner_radius(inner_w, inner_h);
        if inner_r < type_r {
            inner_r = type_r;
        }

        match self.inner {
            InnerBorderType::None => {}
            InnerBorderType::Group => {
                let canvas = painter.canvas_color();
                super::toolkit_images::with_toolkit_images(|img| {
                    painter.paint_border_image(
                        inner_x,
                        inner_y,
                        inner_w,
                        inner_h,
                        inner_r,
                        inner_r,
                        inner_r,
                        inner_r,
                        &img.group_inner_border,
                        225,
                        225,
                        225,
                        225,
                        255,
                        canvas,
                        BORDER_EDGES_ONLY,
                    );
                });
            }
            InnerBorderType::InputField => {
                let bg = if enabled {
                    look.input_bg_color
                } else {
                    look.input_bg_color.lerp(look.bg_color, 0.80)
                };
                let canvas = painter.canvas_color();
                // C++ insets the round rect by d = (16/216)*rndR, but paints the
                // border image at the full substance rect (rndX,rndY,rndW,rndH).
                let d = (16.0 / 216.0) * inner_r;
                let tr = inner_r - d;
                painter.paint_round_rect(
                    inner_x + d,
                    inner_y + d,
                    inner_w - 2.0 * d,
                    inner_h - 2.0 * d,
                    tr,
                    bg,
                );
                painter.set_canvas_color(bg);
                super::toolkit_images::with_toolkit_images(|img| {
                    painter.paint_border_image(
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
                        Color::TRANSPARENT,
                        BORDER_EDGES_ONLY,
                    );
                });
                let _ = canvas;
            }
            InnerBorderType::OutputField => {
                let bg = if enabled {
                    look.output_bg_color
                } else {
                    look.output_bg_color.lerp(look.bg_color, 0.80)
                };
                let canvas = painter.canvas_color();
                let d = (16.0 / 216.0) * inner_r;
                let tr = inner_r - d;
                painter.paint_round_rect(
                    inner_x + d,
                    inner_y + d,
                    inner_w - 2.0 * d,
                    inner_h - 2.0 * d,
                    tr,
                    bg,
                );
                painter.set_canvas_color(bg);
                super::toolkit_images::with_toolkit_images(|img| {
                    painter.paint_border_image(
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
                        Color::TRANSPARENT,
                        BORDER_EDGES_ONLY,
                    );
                });
                let _ = canvas;
            }
            InnerBorderType::CustomRect => {
                let canvas = painter.canvas_color();
                super::toolkit_images::with_toolkit_images(|img| {
                    painter.paint_border_image(
                        inner_x,
                        inner_y,
                        inner_w,
                        inner_h,
                        inner_r,
                        inner_r,
                        inner_r,
                        inner_r,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_look() -> Look {
        Look::default()
    }

    #[test]
    fn content_rect_none_border() {
        let border = Border::new(OuterBorderType::None);
        let Rect { x, y, w: cw, h: ch } = border.content_rect(100.0, 50.0, &test_look());
        assert!((x - 0.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);
        assert!((cw - 100.0).abs() < 0.01);
        assert!((ch - 50.0).abs() < 0.01);
    }

    #[test]
    fn content_rect_rect_border() {
        let border = Border::new(OuterBorderType::Rect);
        let Rect { x, y, w: cw, h: ch } = border.content_rect(100.0, 50.0, &test_look());
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
        let border = Border::new(OuterBorderType::Rect).with_caption("Test");
        let Rect { x, y, w: cw, h: ch } = border.content_rect(100.0, 50.0, &test_look());
        let d = 50.0 * 0.043;
        let rnd_w = 100.0 - 2.0 * d;
        let rnd_h = 50.0 - 2.0 * d;
        let label_h = border.label_space(rnd_w, rnd_h);
        let rnd_h_after_label = rnd_h - label_h;
        let ms = rnd_w.min(rnd_h_after_label) * 0.023;
        assert!((x - (d + ms)).abs() < 0.01);
        assert!((y - (d + label_h + ms)).abs() < 0.01);
        assert!((cw - (rnd_w - 2.0 * ms)).abs() < 0.5);
        assert!((ch - (rnd_h_after_label - 2.0 * ms)).abs() < 0.5);
    }

    #[test]
    fn content_rect_with_inner_input_field() {
        let border = Border::new(OuterBorderType::None).with_inner(InnerBorderType::InputField);
        let Rect { x, y, w: cw, h: ch } = border.content_rect(100.0, 50.0, &test_look());
        // OBT_NONE: outer_inset=0, minSpace=0
        // inner s = min(100, 50) * 1.0 = 50, rndR = 50 * 0.094 = 4.7
        // inner inset d = rndR * (16/216)
        let d = 50.0 * 0.094 * (16.0 / 216.0);
        assert!((x - d).abs() < 0.01);
        assert!((y - d).abs() < 0.01);
        assert!((cw - (100.0 - 2.0 * d)).abs() < 0.01);
        assert!((ch - (50.0 - 2.0 * d)).abs() < 0.01);
    }

    #[test]
    fn content_rect_instrument_with_caption_and_inner() {
        let border = Border::new(OuterBorderType::Instrument)
            .with_caption("Cap")
            .with_inner(InnerBorderType::InputField);
        let r = border.content_rect(100.0, 80.0, &test_look());
        // Outer s = min(100,80)*1.0 = 80, d = 80*0.052 = 4.16
        let od = 80.0 * 0.052;
        let rnd_w = 100.0 - 2.0 * od;
        let rnd_h = 80.0 - 2.0 * od;
        let label_h = border.label_space(rnd_w, rnd_h);
        let rnd_h_after_label = rnd_h - label_h;
        // minSpace for Instrument = 0.023
        let ms = rnd_w.min(rnd_h_after_label) * 0.023;
        let iw = rnd_w - 2.0 * ms;
        let ih = rnd_h_after_label - 2.0 * ms;
        // inner inset = rndR * (16/216)
        let id = iw.min(ih) * 0.094 * (16.0 / 216.0);
        assert!((r.x - (od + ms + id)).abs() < 0.5);
        assert!((r.y - (od + label_h + ms + id)).abs() < 0.5);
        assert!((r.w - (iw - 2.0 * id)).abs() < 0.5);
        assert!((r.h - (ih - 2.0 * id)).abs() < 0.5);
    }

    #[test]
    fn preferred_size_round_trips() {
        let border = Border::new(OuterBorderType::RoundRect)
            .with_caption("Title")
            .with_inner(InnerBorderType::Group);
        let (pw, ph) = border.preferred_size_for_content(50.0, 30.0);
        let Rect { w: cw, h: ch, .. } = border.content_rect(pw, ph, &test_look());
        // Approximate round-trip: proportional insets differ when computed from
        // content size vs total size, so we allow broader tolerance.
        assert!((cw - 50.0).abs() < 5.0, "cw={cw}");
        assert!((ch - 30.0).abs() < 5.0, "ch={ch}");
    }

    #[test]
    fn border_scaling_doubles_insets() {
        let border1 = Border::new(OuterBorderType::Rect);
        let border2 = Border::new(OuterBorderType::Rect).with_border_scaling(2.0);
        let (ox1, _, _, _) = border1.outer_insets(100.0, 100.0);
        let (ox2, _, _, _) = border2.outer_insets(100.0, 100.0);
        assert!((ox2 - 2.0 * ox1).abs() < 0.01);
    }

    #[test]
    fn zero_size_clamping() {
        let border = Border::new(OuterBorderType::Instrument)
            .with_caption("Cap")
            .with_inner(InnerBorderType::InputField);
        let r = border.content_rect(1.0, 1.0, &test_look());
        assert!(r.w >= 0.0);
        assert!(r.h >= 0.0);
    }

    #[test]
    fn disabled_dimming_alpha() {
        use crate::foundation::Color;
        let c = Color::rgba(100, 150, 200, 255);
        let dimmed = c.with_alpha((c.a() as u16 * 64 / 255) as u8);
        // 255 * 64 / 255 = 64
        assert_eq!(dimmed.a(), 64);
        assert_eq!(dimmed.r(), 100);
    }

    #[test]
    fn with_alpha_preserves_rgb() {
        use crate::foundation::Color;
        let c = Color::rgb(10, 20, 30);
        let c2 = c.with_alpha(128);
        assert_eq!(c2.r(), 10);
        assert_eq!(c2.g(), 20);
        assert_eq!(c2.b(), 30);
        assert_eq!(c2.a(), 128);
    }

    #[test]
    fn has_label_with_icon_only() {
        let img = Image::new(16, 16, 4);
        let border = Border::new(OuterBorderType::None).with_icon(img);
        assert!(border.has_label());
    }

    #[test]
    fn label_height_icon_above() {
        let img = Image::new(16, 16, 4);
        let mut border = Border::new(OuterBorderType::None)
            .with_caption("Cap")
            .with_icon(img);
        border.set_icon_above_caption(true);
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
        let img = Image::new(16, 16, 4);
        let mut border = Border::new(OuterBorderType::None)
            .with_caption("Cap")
            .with_icon(img);
        border.set_icon_above_caption(true);
        let r = border.content_rect(200.0, 200.0, &test_look());
        // OuterBorderType::None has zero insets, so rnd = full dims.
        // Content rect offset = label_space (includes padding around text).
        let ls = border.label_space(200.0, 200.0);
        assert!((r.y - ls).abs() < 0.01);
    }

    #[test]
    fn image_is_empty() {
        let empty = Image::new(0, 0, 1);
        assert!(empty.is_empty());
        let nonempty = Image::new(1, 1, 1);
        assert!(!nonempty.is_empty());
    }

    // --- is_opaque tests ---

    #[test]
    fn is_opaque_filled_opaque_bg() {
        let look = test_look();
        assert!(
            look.bg_color.is_opaque(),
            "default look bg should be opaque"
        );
        let border = Border::new(OuterBorderType::Filled);
        assert!(border.is_opaque(&look));
    }

    #[test]
    fn is_opaque_margin_filled() {
        let border = Border::new(OuterBorderType::MarginFilled);
        assert!(border.is_opaque(&test_look()));
    }

    #[test]
    fn is_opaque_popup_root() {
        let border = Border::new(OuterBorderType::PopupRoot);
        assert!(border.is_opaque(&test_look()));
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
            let border = Border::new(outer);
            assert!(!border.is_opaque(&look), "expected false for {outer:?}");
        }
    }

    #[test]
    fn is_opaque_transparent_bg() {
        use crate::foundation::Color;
        let mut look = test_look();
        look.bg_color = Color::rgba(100, 100, 100, 128);
        let border = Border::new(OuterBorderType::Filled);
        assert!(!border.is_opaque(&look));
    }

    // --- substance_round_rect tests ---

    #[test]
    fn substance_none_is_full_rect() {
        let border = Border::new(OuterBorderType::None);
        let (rect, r) = border.substance_round_rect(200.0, 100.0);
        assert!(rect.x.abs() < 0.001);
        assert!(rect.y.abs() < 0.001);
        assert!((rect.w - 200.0).abs() < 0.001);
        assert!((rect.h - 100.0).abs() < 0.001);
        assert!(r.abs() < 0.001);
    }

    #[test]
    fn substance_filled_is_full_rect() {
        let border = Border::new(OuterBorderType::Filled);
        let (rect, r) = border.substance_round_rect(200.0, 100.0);
        assert!((rect.w - 200.0).abs() < 0.001);
        assert!((rect.h - 100.0).abs() < 0.001);
        assert!(r.abs() < 0.001);
    }

    #[test]
    fn substance_margin_is_inset() {
        let border = Border::new(OuterBorderType::Margin);
        let (rect, r) = border.substance_round_rect(100.0, 100.0);
        let d = 100.0 * 0.04;
        assert!((rect.x - d).abs() < 0.01);
        assert!((rect.y - d).abs() < 0.01);
        assert!((rect.w - (100.0 - 2.0 * d)).abs() < 0.01);
        assert!((rect.h - (100.0 - 2.0 * d)).abs() < 0.01);
        assert!(r.abs() < 0.001);
    }

    #[test]
    fn substance_round_rect_has_radius() {
        let border = Border::new(OuterBorderType::RoundRect);
        let (rect, r) = border.substance_round_rect(200.0, 100.0);
        assert!(r > 0.0, "round rect substance should have positive radius");
        assert!(rect.w < 200.0, "should be inset from full width");
    }

    #[test]
    fn substance_group_expanded_from_rnd() {
        let border = Border::new(OuterBorderType::Group);
        let (rect, r) = border.substance_round_rect(200.0, 100.0);
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
        let border = Border::new(OuterBorderType::PopupRoot);
        let (rect, r) = border.substance_round_rect(200.0, 100.0);
        assert!(rect.x.abs() < 0.001);
        assert!((rect.w - 200.0).abs() < 0.001);
        assert!(r.abs() < 0.001);
    }

    // --- content_round_rect tests ---

    #[test]
    fn content_round_rect_none_border() {
        let border = Border::new(OuterBorderType::None);
        let look = test_look();
        let (rect, r) = border.content_round_rect(100.0, 50.0, &look);
        assert!(rect.x.abs() < 0.01);
        assert!(rect.y.abs() < 0.01);
        assert!((rect.w - 100.0).abs() < 0.01);
        assert!((rect.h - 50.0).abs() < 0.01);
        assert!(r.abs() < 0.01);
    }

    #[test]
    fn content_round_rect_with_inner_input_field() {
        let border = Border::new(OuterBorderType::None).with_inner(InnerBorderType::InputField);
        let look = test_look();
        let (rect, r) = border.content_round_rect(100.0, 50.0, &look);
        // OBT_NONE: outer_inset=0, minSpace=0
        // inner inset d = s * 0.094 * (16/216), s = min(100,50) * 1.0 = 50
        let d = 50.0 * 0.094 * (16.0 / 216.0);
        assert!((rect.x - d).abs() < 0.5);
        assert!((rect.y - d).abs() < 0.5);
        assert!(r > 0.0, "IO field should have positive radius");
    }

    #[test]
    fn content_round_rect_matches_content_rect_position() {
        // For non-IO inner borders, the rect position should match content_rect.
        let border = Border::new(OuterBorderType::Rect).with_inner(InnerBorderType::Group);
        let look = test_look();
        let (rr, _radius) = border.content_round_rect(100.0, 60.0, &look);
        let cr = border.content_rect(100.0, 60.0, &look);
        assert!((rr.x - cr.x).abs() < 0.5);
        assert!((rr.y - cr.y).abs() < 0.5);
        assert!((rr.w - cr.w).abs() < 0.5);
        assert!((rr.h - cr.h).abs() < 0.5);
    }

    // --- content_rect_unobscured tests ---

    #[test]
    fn content_rect_unobscured_equals_content_rect_for_none() {
        let border = Border::new(OuterBorderType::Rect);
        let look = test_look();
        let cr = border.content_rect(100.0, 50.0, &look);
        let cu = border.content_rect_unobscured(100.0, 50.0, &look);
        assert!((cr.x - cu.x).abs() < 0.001);
        assert!((cr.y - cu.y).abs() < 0.001);
        assert!((cr.w - cu.w).abs() < 0.001);
        assert!((cr.h - cu.h).abs() < 0.001);
    }

    #[test]
    fn content_rect_unobscured_smaller_for_input_field() {
        let border = Border::new(OuterBorderType::None).with_inner(InnerBorderType::InputField);
        let look = test_look();
        let cr = border.content_rect(200.0, 100.0, &look);
        let cu = border.content_rect_unobscured(200.0, 100.0, &look);
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
        let border = Border::new(OuterBorderType::None).with_inner(InnerBorderType::Group);
        let look = test_look();
        let cr = border.content_rect(200.0, 100.0, &look);
        let cu = border.content_rect_unobscured(200.0, 100.0, &look);
        assert!((cr.x - cu.x).abs() < 0.001);
        assert!((cr.w - cu.w).abs() < 0.001);
    }

    // --- aux system tests ---

    #[test]
    fn aux_defaults_absent() {
        let border = Border::new(OuterBorderType::None);
        assert_eq!(border.get_aux_panel_name(), "");
        assert!((border.get_aux_tallness() - 1.0).abs() < f64::EPSILON);
        assert!(!border.has_aux_panel());
        assert!(border.get_aux_rect(200.0, 100.0).is_none());
    }

    #[test]
    fn have_aux_creates_aux() {
        let mut border = Border::new(OuterBorderType::None);
        border.have_aux("my_panel", 2.0);
        assert_eq!(border.get_aux_panel_name(), "my_panel");
        assert!((border.get_aux_tallness() - 2.0).abs() < f64::EPSILON);
        assert!(border.has_aux_panel());
    }

    #[test]
    fn have_aux_updates_name() {
        let mut border = Border::new(OuterBorderType::None);
        border.have_aux("p1", 1.0);
        border.have_aux("p2", 1.0);
        assert_eq!(border.get_aux_panel_name(), "p2");
    }

    #[test]
    fn have_aux_updates_tallness() {
        let mut border = Border::new(OuterBorderType::None);
        border.have_aux("p1", 1.0);
        border.have_aux("p1", 3.5);
        assert!((border.get_aux_tallness() - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn remove_aux_clears() {
        let mut border = Border::new(OuterBorderType::None);
        border.have_aux("p1", 2.0);
        border.remove_aux();
        assert_eq!(border.get_aux_panel_name(), "");
        assert!((border.get_aux_tallness() - 1.0).abs() < f64::EPSILON);
        assert!(!border.has_aux_panel());
        assert!(border.get_aux_rect(200.0, 100.0).is_none());
    }

    #[test]
    fn remove_aux_noop_when_absent() {
        let mut border = Border::new(OuterBorderType::None);
        border.remove_aux(); // should not panic
        assert!(!border.has_aux_panel());
    }

    #[test]
    fn aux_rect_no_label_positive_dimensions() {
        let mut border = Border::new(OuterBorderType::Rect);
        border.have_aux("aux", 1.0);
        let rect = border
            .get_aux_rect(200.0, 100.0)
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
        let mut border = Border::new(OuterBorderType::Rect).with_caption("Caption");
        border.have_aux("aux", 1.5);
        let rect = border
            .get_aux_rect(200.0, 100.0)
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
        let mut b1 = Border::new(OuterBorderType::None);
        b1.have_aux("aux", 1.0);
        let r1 = b1.get_aux_rect(200.0, 200.0).unwrap();

        let mut b2 = Border::new(OuterBorderType::None);
        b2.have_aux("aux", 2.0);
        let r2 = b2.get_aux_rect(200.0, 200.0).unwrap();

        // Higher tallness means taller relative to width.
        assert!(
            (r2.h / r2.w) > (r1.h / r1.w),
            "tallness 2.0 should be taller than 1.0"
        );
    }
}
