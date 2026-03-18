use std::rc::Rc;

use crate::foundation::{Color, Rect};
use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::{Painter, TextAlignment, VAlign};

use super::border::{Border, InnerBorderType, OuterBorderType};
use super::look::Look;

/// Default text formatter: decimal representation of the value.
/// The `mark_interval` parameter is ignored by the default.
fn default_text_of_value(value: i64, _mark_interval: u64) -> String {
    value.to_string()
}

// C++ HowTo text constants (emBorder.cpp:1416-1460, emScalarField.cpp:507-527).
const HOWTO_PREFACE: &str = concat!(
    "How to use this panel\n",
    "#####################\n",
    "\n",
    "Here is some text describing the usage of this panel. The text consists of\n",
    "multiple sections which may come from different parts of the program based on\n",
    "each other. If something is contradictory, the later section should count.\n",
);

const HOWTO_DISABLED: &str = concat!(
    "\n",
    "\n",
    "DISABLED\n",
    "\n",
    "This panel is currently disabled, because the panel is probably irrelevant for\n",
    "the current state of the program or data. Any try to modify data or to trigger a\n",
    "function may silently be ignored.\n",
);

const HOWTO_FOCUS: &str = concat!(
    "\n",
    "\n",
    "FOCUS\n",
    "\n",
    "This panel is focusable. Only one panel can be focused at a time. The focus is\n",
    "indicated by small arrows pointing to the focused panel. If a panel is focused,\n",
    "it gets the keyboard input. If the focused panel does not know what to do with a\n",
    "certain input key, it may even forward the input to its ancestor panels.\n",
    "\n",
    "How to move or set the focus:\n",
    "\n",
    "* Just zoom and scroll around - the focus is moved automatically by that.\n",
    "\n",
    "* Click with the left or right mouse button on a panel to give it the focus.\n",
    "\n",
    "* Press Tab or Shift+Tab to move the focus to the next or previous sister\n",
    "  panel.\n",
    "\n",
    "* Press the cursor keys to move the focus to a sister panel in the desired\n",
    "  direction.\n",
    "\n",
    "* Press Page-Up or -Down to move the focus to a child or parent panel.\n",
);

const HOWTO_SCALAR_FIELD: &str = concat!(
    "\n",
    "\n",
    "SCALAR FIELD\n",
    "\n",
    "This is a scalar field. In such a field, a scalar value can be viewed and\n",
    "edited. Usually it is a number, but it can even be a choice of a series of\n",
    "possibilities.\n",
    "\n",
    "To move the needle to a desired value, click or drag with the left mouse button.\n",
    "Alternatively, you can move the needle by pressing the + and - keys.\n",
);

const HOWTO_READ_ONLY: &str = concat!(
    "\n",
    "\n",
    "READ-ONLY\n",
    "\n",
    "This scalar field is read-only. You cannot move the needle.\n",
);

/// Numeric input with scale bar.
///
/// Values are stored as `f64` but keyboard stepping logic uses integer
/// arithmetic internally to match the C++ emScalarField behaviour.
pub struct ScalarField {
    border: Border,
    look: Rc<Look>,
    value: f64,
    min: f64,
    max: f64,
    precision: usize,
    editable: bool,
    dragging: bool,
    drag_start_x: f64,
    drag_start_value: f64,
    /// Cached dimensions from the last paint call.
    last_w: f64,
    last_h: f64,

    // --- Scale mark configuration ---
    scale_mark_intervals: Vec<u64>,
    marks_never_hidden: bool,
    text_of_value_fn: Box<dyn Fn(i64, u64) -> String>,
    text_box_tallness: f64,
    kb_interval: u64,

    pub on_value: Option<Box<dyn FnMut(f64)>>,
}

impl ScalarField {
    pub fn new(min: f64, max: f64, look: Rc<Look>) -> Self {
        let clamped_max = if max < min { min } else { max };
        let value = min;
        Self {
            border: Border::new(OuterBorderType::Instrument)
                .with_inner(InnerBorderType::InputField)
                .with_how_to(true),
            look,
            value,
            min,
            max: clamped_max,
            precision: 2,
            editable: true,
            dragging: false,
            drag_start_x: 0.0,
            drag_start_value: 0.0,
            last_w: 0.0,
            last_h: 0.0,
            scale_mark_intervals: vec![1],
            marks_never_hidden: false,
            text_of_value_fn: Box::new(default_text_of_value),
            text_box_tallness: 0.5,
            kb_interval: 0,
            on_value: None,
        }
    }

    pub fn set_caption(&mut self, caption: &str) {
        self.border.caption = caption.to_string();
    }

    pub(crate) fn border_mut(&mut self) -> &mut Border {
        &mut self.border
    }

    // --- Editable ---

    pub fn is_editable(&self) -> bool {
        self.editable
    }

    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        // Switch inner border type to match editability, but only if it was
        // the "other" standard type (matching C++ SetEditable behaviour).
        if editable && self.border.inner == InnerBorderType::OutputField {
            self.border.inner = InnerBorderType::InputField;
        } else if !editable && self.border.inner == InnerBorderType::InputField {
            self.border.inner = InnerBorderType::OutputField;
        }
    }

    // --- Value ---

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn set_value(&mut self, val: f64) {
        let clamped = val.clamp(self.min, self.max);
        if (clamped - self.value).abs() > f64::EPSILON {
            self.value = clamped;
            self.fire_change();
        }
    }

    pub fn set_precision(&mut self, precision: usize) {
        self.precision = precision;
    }

    // --- Min/Max ---

    pub fn min_value(&self) -> f64 {
        self.min
    }

    pub fn max_value(&self) -> f64 {
        self.max
    }

    pub fn set_min_value(&mut self, min: f64) {
        if (self.min - min).abs() < f64::EPSILON {
            return;
        }
        self.min = min;
        if self.max < self.min {
            self.max = self.min;
        }
        if self.value < self.min {
            self.set_value(self.min);
        }
    }

    pub fn set_max_value(&mut self, max: f64) {
        if (self.max - max).abs() < f64::EPSILON {
            return;
        }
        self.max = max;
        if self.min > self.max {
            self.min = self.max;
        }
        if self.value > self.max {
            self.set_value(self.max);
        }
    }

    pub fn set_min_max_values(&mut self, min: f64, max: f64) {
        self.set_min_value(min);
        self.set_max_value(max);
    }

    // --- Scale mark configuration ---

    /// Returns the current scale mark intervals (descending order, each > 0).
    pub fn scale_mark_intervals(&self) -> &[u64] {
        &self.scale_mark_intervals
    }

    /// Sets scale mark intervals. Each element must be > 0 and the sequence
    /// must be in strictly descending order. Panics on invalid input (matching
    /// the C++ `emFatalError` behaviour).
    pub fn set_scale_mark_intervals(&mut self, intervals: &[u64]) {
        for (i, &iv) in intervals.iter().enumerate() {
            assert!(iv > 0, "scale mark interval must be > 0 (index {i})");
            if i > 0 {
                assert!(
                    iv < intervals[i - 1],
                    "scale mark intervals must be strictly descending \
                     (index {i}: {} >= {})",
                    iv,
                    intervals[i - 1]
                );
            }
        }
        if self.scale_mark_intervals == intervals {
            return;
        }
        self.scale_mark_intervals = intervals.to_vec();
    }

    pub fn is_never_hiding_marks(&self) -> bool {
        self.marks_never_hidden
    }

    pub fn set_never_hide_marks(&mut self, never_hide: bool) {
        self.marks_never_hidden = never_hide;
    }

    // --- Text formatting ---

    /// Sets a custom value-to-text formatter. The function receives the value
    /// as `i64` and the current mark interval as `u64`, returning the display
    /// string.
    pub fn set_text_of_value_fn(&mut self, f: Box<dyn Fn(i64, u64) -> String>) {
        self.text_of_value_fn = f;
    }

    pub fn text_box_tallness(&self) -> f64 {
        self.text_box_tallness
    }

    pub fn set_text_box_tallness(&mut self, tallness: f64) {
        self.text_box_tallness = tallness;
    }

    // --- Keyboard interval ---

    pub fn keyboard_interval(&self) -> u64 {
        self.kb_interval
    }

    pub fn set_keyboard_interval(&mut self, interval: u64) {
        self.kb_interval = interval;
    }

    // --- Paint ---

    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, enabled: bool) {
        self.last_w = w;
        self.last_h = h;
        // C++ emScalarField::GetHowTo() builds the text dynamically.
        {
            let mut text = String::from(HOWTO_PREFACE);
            if !enabled {
                text.push_str(HOWTO_DISABLED);
            }
            text.push_str(HOWTO_FOCUS);
            text.push_str(HOWTO_SCALAR_FIELD);
            if !self.editable {
                text.push_str(HOWTO_READ_ONLY);
            }
            self.border.how_to_text = text;
        }
        self.border
            .paint_border(painter, w, h, &self.look, false, enabled);
        let canvas_color = painter.canvas_color();

        let (content, radius) = self.border.content_round_rect(w, h, &self.look);
        let Rect { x, y, w: cw, h: ch } = content;
        let r = radius;
        let v_range = self.max - self.min;

        // C++ DoScalarField selects colors by InnerBorderType, not editable flag.
        // IBT_INPUT_FIELD → input colors, IBT_OUTPUT_FIELD → output colors,
        // else (IBT_CUSTOM_RECT etc.) → look bg/fg colors.
        let (mut bg_col, mut fg_col) = match self.border.inner {
            InnerBorderType::InputField => (self.look.input_bg_color, self.look.input_fg_color),
            InnerBorderType::OutputField => (self.look.output_bg_color, self.look.output_fg_color),
            _ => (self.look.bg_color, self.look.fg_color),
        };

        // C++ emScalarField.cpp:413-416: dim colors when disabled.
        if !enabled {
            bg_col = bg_col.lerp(self.look.bg_color, 0.80);
            fg_col = fg_col.lerp(self.look.bg_color, 0.80);
        }

        // C++ DoScalarField layout matching emScalarField.cpp
        let rx = x + r * 0.5;
        let ry = y + r * 0.5;
        let rw = cw - r;
        let rh = ch - r;

        let s = rh.min(rw);
        let d_base = s * 0.04;
        let mut ax = rx + d_base;
        let mut ay = ry + d_base;
        let mut aw = rw - 2.0 * d_base;
        let mut ah = rh - 2.0 * d_base;

        let mut e = s * 0.3 * 0.5;

        // Scale mark layout calculations.
        // C++ interval culling: skip leading intervals > vRange when !marks_never_hidden.
        let ivals = &self.scale_mark_intervals;
        let mut ival_start = 0;
        let mut ival_cnt = ivals.len();
        if !self.marks_never_hidden {
            while ival_cnt > 1 && ivals[ival_start] as f64 > v_range {
                ival_start += 1;
                ival_cnt -= 1;
            }
        }
        let ivals = &self.scale_mark_intervals[ival_start..ival_start + ival_cnt];
        let ival_sum: u64 = ivals.iter().sum();

        let mtw0 = 1.0_f64;
        let mth0 = self.text_box_tallness;
        let mah0 = mtw0.min(mth0) * 0.5;
        let norm = 1.0 / (mth0 + mah0);
        let mtw = mtw0 * norm;
        let mth = mth0 * norm;
        let mah = mtw0.min(mth0) * 0.5 * norm;
        let mw = mtw * 1.5;

        let mut d = e - d_base;
        if d < 0.0 {
            d = 0.0;
        }
        if ival_cnt > 0 && v_range > 0.0 {
            let mut th_mark = ah;
            let f_mark = th_mark * ivals[0] as f64 / ival_sum as f64;
            let tw_mark = f_mark * mw * v_range / ivals[0] as f64;
            let mut f2 = f_mark * mtw;
            if tw_mark + f2 > aw {
                f2 *= aw / (tw_mark + f2);
            }
            f2 *= 0.5;
            if d < f2 {
                d = f2;
            }
            let f_max = aw * 0.2;
            if d > f_max {
                d = f_max;
            }
            if tw_mark > aw - 2.0 * d {
                th_mark *= (aw - 2.0 * d) / tw_mark;
            }
            ay += ah - th_mark;
            ah = th_mark;
        }
        ax += d;
        aw -= 2.0 * d;

        // Side bars — C++: col = bgCol.GetBlended(fgCol, 25)
        let side_col = bg_col.lerp(fg_col, 0.25);
        if ax > rx {
            painter.paint_rect(rx, ry, ax - rx, rh, side_col, canvas_color);
        }
        if ax + aw < rx + rw {
            painter.paint_rect(ax + aw, ry, rx + rw - ax - aw, rh, side_col, canvas_color);
        }

        // Value arrow polygon (5-point downward arrow)
        let tx = if v_range > 0.0 {
            ax + aw * ((self.value - self.min) / v_range)
        } else {
            ax + aw * 0.5
        };
        if e > ay + ah - ry {
            e = ay + ah - ry;
        }
        let arrow = [
            (tx - e, ry),
            (tx + e, ry),
            (tx + e, ay + ah - e),
            (tx, ay + ah),
            (tx - e, ay + ah - e),
        ];
        painter.paint_polygon(&arrow, fg_col, Color::TRANSPARENT);

        // Scale marks with text labels and small arrows.
        // C++ emScalarField.cpp lines 438-473.
        if ival_cnt > 0 && v_range > 0.0 {
            let f = aw / v_range;
            let mark_col = bg_col.lerp(fg_col, 0.66);
            let (scale_x, _) = painter.scaling();
            let mut mark_ty = ay;
            for &ival in ivals.iter() {
                let th = ah / ival_sum as f64 * ival as f64;
                let tw = mtw * th;

                // C++ visibility gate: skip tier if mark text < 1px wide on screen.
                if tw * scale_x <= 1.0 {
                    mark_ty += th;
                    continue;
                }

                let h4 = mth * th;
                let h5 = mah * th;

                // C++ clip-region culling: only iterate marks within visible area.
                let interval = ival as f64;
                let mut x3 = painter.get_user_clip_x1() - tw * 0.5;
                let mut w3 = painter.get_user_clip_x2() + tw * 0.5 - x3;
                if x3 < ax {
                    x3 = ax;
                }
                if w3 > ax + aw - x3 {
                    w3 = ax + aw - x3;
                }
                let k1 = ((x3 - ax) / f + self.min - 0.01) / interval;
                let k2 = ((x3 + w3 - ax) / f + self.min + 0.01) / interval;
                let mut k = k1.ceil() as i64;
                let k_end = k2.floor() as i64;
                while k <= k_end {
                    let v = k as f64 * interval;
                    let mark_tx = (v - self.min) * f + ax;

                    // Text label
                    let label = (self.text_of_value_fn)(v as i64, ival);
                    // C++ PaintTextBoxed defaults: minWidthScale=0.5, formatted=true.
                    painter.paint_text_boxed(
                        mark_tx - tw * 0.5,
                        mark_ty,
                        tw,
                        h4,
                        &label,
                        h4,
                        mark_col,
                        Color::TRANSPARENT,
                        TextAlignment::Center,
                        VAlign::Center,
                        TextAlignment::Center,
                        0.5,
                        true,
                        0.0,
                    );

                    // Small downward arrow below label
                    let mini_arrow = [
                        (mark_tx - h5 * 0.5, mark_ty + h4),
                        (mark_tx + h5 * 0.5, mark_ty + h4),
                        (mark_tx, mark_ty + h4 + h5),
                    ];
                    painter.paint_polygon(&mini_arrow, mark_col, Color::TRANSPARENT);

                    k += 1;
                }
                mark_ty += th;
            }
        }

        // C++ paints content, THEN overlays the IO field border image.
        self.border.paint_inner_overlay(painter, w, h, &self.look);
    }

    // --- Input ---

    /// Rounded-rect hit test matching C++ `emScalarField::CheckMouse`.
    fn hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let (rect, r) = self.border.content_round_rect(1.0, tallness, &self.look);
        super::check_mouse_round_rect(mx, my, &rect, r)
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        if !self.editable {
            return false;
        }

        let Rect { w: cw, .. } = self.border.content_rect(self.last_w, 0.0, &self.look);
        let range = self.max - self.min;

        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    if !self.hit_test(event.mouse_x, event.mouse_y) {
                        return false;
                    }
                    self.dragging = true;
                    self.drag_start_x = event.mouse_x;
                    self.drag_start_value = self.value;
                    true
                }
                InputVariant::Release => {
                    if !self.dragging {
                        return false;
                    }
                    self.dragging = false;
                    true
                }
                InputVariant::Repeat | InputVariant::Move => {
                    if self.dragging && cw > 0.0 {
                        let dx = event.mouse_x - self.drag_start_x;
                        let dv = dx / cw * range;
                        let new_val = (self.drag_start_value + dv).clamp(self.min, self.max);
                        if (new_val - self.value).abs() > f64::EPSILON {
                            self.value = new_val;
                            self.fire_change();
                        }
                    }
                    true
                }
            },
            // C++ emScalarField.cpp:261-272: only '+' and '-' character keys.
            // Arrow keys are NOT in C++ (would conflict with focus navigation).
            InputKey::Key('+') if event.variant == InputVariant::Press => {
                self.step_by_keyboard(1);
                true
            }
            InputKey::Key('-') if event.variant == InputVariant::Press => {
                self.step_by_keyboard(-1);
                true
            }
            _ => false,
        }
    }

    pub fn get_cursor(&self) -> Cursor {
        if self.editable {
            Cursor::ResizeEW
        } else {
            Cursor::Normal
        }
    }

    /// Whether this scalar field provides how-to help text.
    /// Matches C++ `emScalarField::HasHowTo` (always true).
    pub fn has_how_to(&self) -> bool {
        true
    }

    /// Help text describing how to use this scalar field.
    ///
    /// Chains the border's base how-to with scalar-field-specific sections.
    /// Matches C++ `emScalarField::GetHowTo`.
    pub fn get_how_to(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.get_howto(enabled, focusable);
        text.push_str(HOWTO_SCALAR_FIELD);
        if !self.editable {
            text.push_str(HOWTO_READ_ONLY);
        }
        text
    }

    /// Check whether (`mx`, `my`) is within the scale area and compute
    /// the corresponding value.
    ///
    /// Returns `Some(value)` if the point is inside the scale area, `None`
    /// otherwise. Matches C++ `emScalarField::CheckMouse`.
    pub fn check_mouse(&self, mx: f64, my: f64) -> Option<f64> {
        let w = self.last_w;
        if w <= 0.0 {
            return None;
        }
        // Replicate the layout math from paint to find (ax, aw).
        let (content, radius) = self.border.content_round_rect(w, 0.0, &self.look);
        let Rect { x, y, w: cw, h: ch } = content;
        let r = radius;
        let v_range = self.max - self.min;

        let rx = x + r * 0.5;
        let ry = y + r * 0.5;
        let rw = cw - r;
        let rh = ch - r;

        let s = rh.min(rw);
        let d_base = s * 0.04;
        let mut ax = rx + d_base;
        let ay = ry + d_base;
        let mut aw = rw - 2.0 * d_base;
        let ah = rh - 2.0 * d_base;

        let mut e = s * 0.3 * 0.5;

        let ivals = &self.scale_mark_intervals;
        let ival_cnt = ivals.len();
        let ival_sum: u64 = ivals.iter().sum();

        let mtw0 = 1.0_f64;
        let mth0 = self.text_box_tallness;
        let norm = 1.0 / (mth0 + mtw0.min(mth0) * 0.5);
        let mtw = mtw0 * norm;
        let mw = mtw * 1.5;

        let mut d = e - d_base;
        if d < 0.0 {
            d = 0.0;
        }
        if ival_cnt > 0 && v_range > 0.0 {
            let th_mark = ah;
            let f_mark = th_mark * ivals[0] as f64 / ival_sum as f64;
            let tw_mark = f_mark * mw * v_range / ivals[0] as f64;
            let mut f2 = f_mark * mtw;
            if tw_mark + f2 > aw {
                f2 *= aw / (tw_mark + f2);
            }
            f2 *= 0.5;
            if d < f2 {
                d = f2;
            }
            let f_max = aw * 0.2;
            if d > f_max {
                d = f_max;
            }
        }
        ax += d;
        aw -= 2.0 * d;

        // Check bounds: the active area is the arrow zone.
        if e > ay + ah - ry {
            e = ay + ah - ry;
        }
        if mx < ax - e || mx > ax + aw + e || my < ry || my > ay + ah {
            return None;
        }

        // Convert x position to value.
        if v_range <= 0.0 || aw <= 0.0 {
            return Some(self.min);
        }
        let frac = ((mx - ax) / aw).clamp(0.0, 1.0);
        let val = self.min + frac * v_range;
        Some(val.clamp(self.min, self.max))
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let cw = 100.0;
        let ch = 13.0 + 4.0;
        self.border.preferred_size_for_content(cw, ch)
    }

    // --- Keyboard stepping (C++ StepByKeyboard parity) ---

    /// Steps the value by a keyboard increment in the given direction.
    ///
    /// Matches the C++ `StepByKeyboard` logic: if `kb_interval > 0`, uses that
    /// as step. Otherwise computes `range/129` (min 1) and finds the best
    /// matching scale mark interval. Snaps to grid with direction-dependent
    /// rounding using integer division.
    fn step_by_keyboard(&mut self, dir: i32) {
        let range_f = self.max - self.min;
        let range = range_f as i64;

        let dv: i64 = if self.kb_interval > 0 {
            self.kb_interval as i64
        } else {
            // Auto mode: range/129, at least 1
            let mindv = (range / 129).max(1);
            let mut dv = mindv;
            for (i, &iv) in self.scale_mark_intervals.iter().enumerate() {
                let iv = iv as i64;
                if iv >= mindv || i == 0 {
                    dv = iv;
                }
            }
            dv
        };

        if dv <= 0 {
            return;
        }

        let cur = self.value as i64;
        let v = if dir < 0 {
            let v = cur - dv;
            // Snap to grid: direction-dependent rounding
            if v < 0 {
                -((-v) / dv) * dv
            } else {
                (v + dv - 1) / dv * dv
            }
        } else {
            let v = cur + dv;
            if v < 0 {
                -((-v + dv - 1) / dv) * dv
            } else {
                (v / dv) * dv
            }
        };

        self.set_value(v as f64);
    }

    fn fire_change(&mut self) {
        if let Some(cb) = &mut self.on_value {
            cb(self.value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn value_clamping() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);

        sf.set_value(50.0);
        assert!((sf.value() - 50.0).abs() < 0.001);

        sf.set_value(-10.0);
        assert!((sf.value() - 0.0).abs() < 0.001);

        sf.set_value(200.0);
        assert!((sf.value() - 100.0).abs() < 0.001);
    }

    #[test]
    fn arrow_key_stepping() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        sf.set_value(50.0);

        // Cache dimensions (paint would do this in real usage)
        sf.last_w = 200.0;
        sf.last_h = 40.0;

        // ScalarField uses '+' and '-' keys (not arrow keys).
        sf.input(&InputEvent::press(InputKey::Key('+')));
        assert!(sf.value() > 50.0);

        sf.input(&InputEvent::press(InputKey::Key('-')));
        // Should be roughly back to 50
        assert!((sf.value() - 50.0).abs() < 2.0);
    }

    #[test]
    fn callback_on_change() {
        let look = Look::new();
        let values = Rc::new(RefCell::new(Vec::new()));
        let val_clone = values.clone();

        let mut sf = ScalarField::new(0.0, 10.0, look);
        sf.set_value(5.0);
        sf.last_w = 200.0;
        sf.last_h = 40.0;
        sf.on_value = Some(Box::new(move |v| {
            val_clone.borrow_mut().push(v);
        }));

        sf.input(&InputEvent::press(InputKey::Key('+')));
        assert_eq!(values.borrow().len(), 1);
        assert!(values.borrow()[0] > 5.0);
    }

    #[test]
    fn editable_toggle() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);

        assert!(sf.is_editable());
        assert_eq!(sf.border.inner, InnerBorderType::InputField);

        sf.set_editable(false);
        assert!(!sf.is_editable());
        assert_eq!(sf.border.inner, InnerBorderType::OutputField);

        // Input should be disabled when not editable
        sf.set_value(50.0);
        sf.last_w = 200.0;
        sf.last_h = 40.0;
        let handled = sf.input(&InputEvent::press(InputKey::Key('+')));
        assert!(!handled);
        assert!((sf.value() - 50.0).abs() < 0.001);

        sf.set_editable(true);
        assert!(sf.is_editable());
        assert_eq!(sf.border.inner, InnerBorderType::InputField);
    }

    #[test]
    fn min_max_getters_setters() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);

        assert!((sf.min_value() - 0.0).abs() < f64::EPSILON);
        assert!((sf.max_value() - 100.0).abs() < f64::EPSILON);

        // Setting min above max clamps max up
        sf.set_min_value(200.0);
        assert!((sf.min_value() - 200.0).abs() < f64::EPSILON);
        assert!((sf.max_value() - 200.0).abs() < f64::EPSILON);
        assert!((sf.value() - 200.0).abs() < f64::EPSILON);

        // Setting max below min clamps min down
        sf.set_max_value(50.0);
        assert!((sf.max_value() - 50.0).abs() < f64::EPSILON);
        assert!((sf.min_value() - 50.0).abs() < f64::EPSILON);

        // set_min_max_values
        sf.set_min_max_values(10.0, 90.0);
        assert!((sf.min_value() - 10.0).abs() < f64::EPSILON);
        assert!((sf.max_value() - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn constructor_clamps_max() {
        let look = Look::new();
        let sf = ScalarField::new(50.0, 10.0, look);
        // max < min => max clamped to min
        assert!((sf.max_value() - 50.0).abs() < f64::EPSILON);
        assert!((sf.min_value() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scale_mark_intervals() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);

        // Default is [1]
        assert_eq!(sf.scale_mark_intervals(), &[1]);

        sf.set_scale_mark_intervals(&[100, 50, 10, 5, 1]);
        assert_eq!(sf.scale_mark_intervals(), &[100, 50, 10, 5, 1]);
    }

    #[test]
    #[should_panic(expected = "strictly descending")]
    fn scale_mark_intervals_rejects_non_descending() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        sf.set_scale_mark_intervals(&[10, 50]); // ascending — invalid
    }

    #[test]
    #[should_panic(expected = "must be > 0")]
    fn scale_mark_intervals_rejects_zero() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        sf.set_scale_mark_intervals(&[0]);
    }

    #[test]
    fn scale_mark_intervals_empty_is_ok() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        sf.set_scale_mark_intervals(&[]);
        assert_eq!(sf.scale_mark_intervals(), &[] as &[u64]);
    }

    #[test]
    fn never_hide_marks() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        assert!(!sf.is_never_hiding_marks());
        sf.set_never_hide_marks(true);
        assert!(sf.is_never_hiding_marks());
    }

    #[test]
    fn text_box_tallness() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        assert!((sf.text_box_tallness() - 0.5).abs() < f64::EPSILON);
        sf.set_text_box_tallness(0.75);
        assert!((sf.text_box_tallness() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn keyboard_interval() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        assert_eq!(sf.keyboard_interval(), 0);
        sf.set_keyboard_interval(5);
        assert_eq!(sf.keyboard_interval(), 5);
    }

    #[test]
    fn step_by_keyboard_with_explicit_interval() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        sf.set_keyboard_interval(10);
        sf.set_value(50.0);
        sf.last_w = 200.0;
        sf.last_h = 40.0;

        sf.input(&InputEvent::press(InputKey::Key('+')));
        assert!((sf.value() - 60.0).abs() < 1.0);

        sf.input(&InputEvent::press(InputKey::Key('-')));
        assert!((sf.value() - 50.0).abs() < 1.0);
    }

    #[test]
    fn custom_text_of_value() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        sf.set_text_of_value_fn(Box::new(|val, _iv| format!("{}%", val)));
        // The function is stored and usable
        let text = (sf.text_of_value_fn)(50, 1);
        assert_eq!(text, "50%");
    }

    #[test]
    fn plus_minus_keys_work() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        sf.set_value(50.0);
        sf.last_w = 200.0;
        sf.last_h = 40.0;

        let handled = sf.input(&InputEvent::press(InputKey::Key('+')));
        assert!(handled);
        assert!(sf.value() > 50.0);
    }

    #[test]
    fn non_editable_cursor_is_default() {
        let look = Look::new();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        assert_eq!(sf.get_cursor(), Cursor::ResizeEW);
        sf.set_editable(false);
        assert_eq!(sf.get_cursor(), Cursor::Normal);
    }

    #[test]
    fn set_value_fires_callback() {
        let look = Look::new();
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();
        let mut sf = ScalarField::new(0.0, 100.0, look);
        sf.on_value = Some(Box::new(move |_v| {
            *count_clone.borrow_mut() += 1;
        }));

        sf.set_value(50.0);
        assert_eq!(*count.borrow(), 1);

        // Setting same value should not fire
        sf.set_value(50.0);
        assert_eq!(*count.borrow(), 1);
    }
}
