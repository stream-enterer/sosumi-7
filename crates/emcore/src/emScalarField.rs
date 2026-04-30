use std::rc::Rc;

use crate::emColor::emColor;
use crate::emCursor::emCursor;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::{emPainter, TextAlignment, VAlign};
use crate::emPanel::PanelState;

use super::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use crate::emEngineCtx::{ConstructCtx, PanelCtx, WidgetCallback};
use crate::emLook::emLook;
use crate::emSignal::SignalId;

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
pub struct emScalarField {
    border: emBorder,
    look: Rc<emLook>,
    value: f64,
    min: f64,
    max: f64,
    precision: usize,
    editable: bool,
    /// Cached enabled state from last paint (for input gating).
    enabled: bool,
    dragging: bool,
    /// Cached dimensions from the last paint call.
    last_w: f64,
    last_h: f64,

    // --- Scale mark configuration ---
    scale_mark_intervals: Vec<u64>,
    marks_never_hidden: bool,
    text_of_value_fn: Box<dyn Fn(i64, u64) -> String>,
    text_box_tallness: f64,
    kb_interval: u64,

    pub on_value: Option<WidgetCallback<f64>>,
    /// Allocated per C++ `emScalarField::GetValueSignal()`. B3.4b: alloc only.
    pub value_signal: SignalId,
}

impl emScalarField {
    pub fn new<C: ConstructCtx>(ctx: &mut C, min: f64, max: f64, look: Rc<emLook>) -> Self {
        let clamped_max = if max < min { min } else { max };
        let value = min;
        Self {
            border: emBorder::new(OuterBorderType::Instrument)
                .with_inner(InnerBorderType::OutputField)
                .with_how_to(true),
            look,
            value,
            min,
            max: clamped_max,
            precision: 2,
            editable: false,
            enabled: true,
            dragging: false,
            last_w: 0.0,
            last_h: 0.0,
            scale_mark_intervals: vec![1],
            marks_never_hidden: false,
            text_of_value_fn: Box::new(default_text_of_value),
            text_box_tallness: 0.5,
            kb_interval: 0,
            on_value: None,
            value_signal: ctx.create_signal(),
        }
    }

    pub fn SetCaption(&mut self, caption: &str) {
        self.border.caption = caption.to_string();
    }

    /// Set the border description text. Matches C++ `emScalarField::SetDescription`.
    pub fn SetDescription(&mut self, desc: &str) {
        self.border.description = desc.to_string();
    }

    pub(crate) fn border_mut(&mut self) -> &mut emBorder {
        &mut self.border
    }

    // --- Editable ---

    pub fn IsEditable(&self) -> bool {
        self.editable
    }

    pub fn SetEditable(&mut self, editable: bool) {
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

    pub fn GetValue(&self) -> f64 {
        self.value
    }

    /// Construction-time value assignment — no signal fires, no callback.
    /// Used from widget constructors where no scheduler reach exists.
    /// C++ parity: `emScalarField` constructor sets `Value` directly without
    /// calling `SetValue` (emScalarField.cpp:44).
    pub fn set_initial_value(&mut self, val: f64) {
        self.value = val.clamp(self.min, self.max);
    }

    /// Test-only setter that bypasses signal firing. Used by B-010 row 563
    /// integration test in `tests/rc_shim_b010.rs` to pre-stage `GetValue()`
    /// before firing the captured `value_signal` directly. Production code
    /// must use `SetValue` (which atomically updates state + fires the signal).
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn set_value_for_test(&mut self, val: f64) {
        self.value = val.clamp(self.min, self.max);
    }

    /// Set value without firing `value_signal` or calling `on_value`. Used by
    /// `FactorFieldPanel::update_output` to sync display from config without
    /// triggering the feedback loop that `SetValue` would cause.
    pub fn set_value_silent(&mut self, val: f64) {
        let clamped = val.clamp(self.min, self.max);
        if (clamped - self.value).abs() > f64::EPSILON {
            self.value = clamped;
        }
    }

    /// Update the maximum value without firing signals or clamping via `SetValue`.
    /// Used by `ScalarFieldWithDynamicMax` to sync the max before painting when
    /// no `PanelCtx` is available. Does not trigger `on_value` or `value_signal`.
    pub fn set_max_silent(&mut self, max: f64) {
        if (self.max - max).abs() < f64::EPSILON {
            return;
        }
        self.max = max;
        if self.min > self.max {
            self.min = self.max;
        }
        // Clamp value silently without firing signals.
        if self.value > self.max {
            self.value = self.max;
        }
    }

    /// Mirrors C++ `emScalarField::SetValue` (emScalarField.cpp:102-111):
    /// InvalidatePainting → Signal(ValueSignal) → ValueChanged.
    pub fn SetValue(&mut self, val: f64, ctx: &mut PanelCtx<'_>) {
        let clamped = val.clamp(self.min, self.max);
        if (clamped - self.value).abs() > f64::EPSILON {
            self.value = clamped;
            if let Some(mut sched) = ctx.as_sched_ctx() {
                sched.fire(self.value_signal);
                if let Some(cb) = self.on_value.as_mut() {
                    cb(self.value, &mut sched);
                }
            }
        }
    }

    pub fn set_precision(&mut self, precision: usize) {
        self.precision = precision;
    }

    // --- Min/Max ---

    pub fn GetMinValue(&self) -> f64 {
        self.min
    }

    pub fn GetMaxValue(&self) -> f64 {
        self.max
    }

    pub fn SetMinValue(&mut self, min: f64, ctx: &mut PanelCtx<'_>) {
        if (self.min - min).abs() < f64::EPSILON {
            return;
        }
        self.min = min;
        if self.max < self.min {
            self.max = self.min;
        }
        if self.value < self.min {
            self.SetValue(self.min, ctx);
        }
    }

    pub fn SetMaxValue(&mut self, max: f64, ctx: &mut PanelCtx<'_>) {
        if (self.max - max).abs() < f64::EPSILON {
            return;
        }
        self.max = max;
        if self.min > self.max {
            self.min = self.max;
        }
        if self.value > self.max {
            self.SetValue(self.max, ctx);
        }
    }

    pub fn SetMinMaxValues(&mut self, min: f64, max: f64, ctx: &mut PanelCtx<'_>) {
        self.SetMinValue(min, ctx);
        self.SetMaxValue(max, ctx);
    }

    // --- Scale mark configuration ---

    /// Returns the current scale mark intervals (descending order, each > 0).
    pub fn GetScaleMarkIntervals(&self) -> &[u64] {
        &self.scale_mark_intervals
    }

    /// Sets scale mark intervals. Each element must be > 0 and the sequence
    /// must be in strictly descending order. Panics on invalid input (matching
    /// the C++ `emFatalError` behaviour).
    pub fn SetScaleMarkIntervals(&mut self, intervals: &[u64]) {
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

    pub fn IsNeverHidingMarks(&self) -> bool {
        self.marks_never_hidden
    }

    pub fn SetNeverHideMarks(&mut self, never_hide: bool) {
        self.marks_never_hidden = never_hide;
    }

    // --- Text formatting ---

    /// Sets a custom value-to-text formatter. The function receives the value
    /// as `i64` and the current mark interval as `u64`, returning the display
    /// string.
    pub fn SetTextOfValueFunc(&mut self, f: Box<dyn Fn(i64, u64) -> String>) {
        self.text_of_value_fn = f;
    }

    pub fn GetTextBoxTallness(&self) -> f64 {
        self.text_box_tallness
    }

    pub fn SetTextBoxTallness(&mut self, tallness: f64) {
        self.text_box_tallness = tallness;
    }

    // --- Keyboard interval ---

    pub fn GetKeyboardInterval(&self) -> u64 {
        self.kb_interval
    }

    pub fn SetKeyboardInterval(&mut self, interval: u64) {
        self.kb_interval = interval;
    }

    // --- Paint ---

    pub fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        enabled: bool,
        pixel_scale: f64,
    ) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;
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
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            false,
            enabled,
            pixel_scale,
        );
        let mut canvas_color = self
            .border
            .content_canvas_color(canvas_color, &self.look, enabled);

        // C++ DoScalarField(SCALAR_FIELD_FUNC_PAINT) — line-by-line from emScalarField.cpp:318-473
        let (content, r) = self.border.GetContentRoundRect(w, h, &self.look);
        let x = content.x;
        let y = content.y;
        let cw = content.w;
        let ch = content.h;

        let v_range: f64 = self.max - self.min;
        let all_ivals = &self.scale_mark_intervals;
        let mut ival_off = 0;
        let mut ival_cnt = all_ivals.len();
        if !self.marks_never_hidden {
            while ival_cnt > 1 && all_ivals[ival_off] as f64 > v_range {
                ival_off += 1;
                ival_cnt -= 1;
            }
        }
        let ivals = &all_ivals[ival_off..ival_off + ival_cnt];
        let mut ival_sum: u64 = 0;
        for iv in &ivals[..ival_cnt] {
            ival_sum += iv;
        }

        // C++ lines 345-352
        let mtw0 = 1.0_f64;
        let mth0 = self.text_box_tallness;
        let mah0 = mtw0.min(mth0) * 0.5;
        let d_inv = 1.0 / (mth0 + mah0);
        let mtw = mtw0 * d_inv;
        let mth = mth0 * d_inv;
        let mah = mah0 * d_inv;
        let mw = mtw * 1.5;

        // C++ lines 353-356
        let rx = x + r * 0.5;
        let ry = y + r * 0.5;
        let rw = cw - r;
        let rh = ch - r;

        // C++ lines 358-363
        let s = rh.min(rw);
        let d_base = s * 0.04;
        let mut ax = rx + d_base;
        let mut ay = ry + d_base;
        let mut aw = rw - 2.0 * d_base;
        let mut ah = rh - 2.0 * d_base;

        // C++ lines 365-381
        let mut e = s * 0.3 * 0.5;
        let mut d = e - d_base;
        if d < 0.0 {
            d = 0.0;
        }
        if ival_cnt > 0 && v_range > 0.0 {
            let mut th = ah;
            let f = th * ivals[0] as f64 / ival_sum as f64;
            let tw = f * mw * v_range / ivals[0] as f64;
            let mut f2 = f * mtw;
            if tw + f2 > aw {
                f2 *= aw / (tw + f2);
            }
            f2 *= 0.5;
            if d < f2 {
                d = f2;
            }
            let f_lim = aw * 0.2;
            if d > f_lim {
                d = f_lim;
            }
            if tw > aw - 2.0 * d {
                th *= (aw - 2.0 * d) / tw;
            }
            ay += ah - th;
            ah = th;
        }
        ax += d;
        aw -= 2.0 * d;

        // C++ lines 400-416: color selection by InnerBorderType
        let (mut bg_col, mut fg_col) = match self.border.inner {
            InnerBorderType::InputField => (self.look.input_bg_color, self.look.input_fg_color),
            InnerBorderType::OutputField => (self.look.output_bg_color, self.look.output_fg_color),
            _ => (self.look.bg_color, self.look.fg_color),
        };
        if !enabled {
            bg_col = bg_col.GetBlended(self.look.bg_color, 80.0);
            fg_col = fg_col.GetBlended(self.look.bg_color, 80.0);
        }

        // C++ lines 418-421: side bars
        let col = bg_col.GetBlended(fg_col, 25.0);
        painter.PaintRect(rx, ry, ax - rx, rh, col, canvas_color);
        painter.PaintRect(ax + aw, ry, rx + rw - ax - aw, rh, col, canvas_color);
        canvas_color = emColor::TRANSPARENT;

        // C++ lines 423-436: value arrow (5-point polygon)
        let tx = if v_range > 0.0 {
            ax + aw * (self.value - self.min) / v_range
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
        painter.PaintPolygon(&arrow, fg_col, canvas_color);
        canvas_color = emColor::TRANSPARENT;

        // C++ lines 438-473: scale marks
        if ival_cnt > 0 && v_range > 0.0 {
            let f = aw / v_range;
            let col = bg_col.GetBlended(fg_col, 66.0);
            let mut ty = ay;
            for &ival in ivals.iter() {
                let th = ah / ival_sum as f64 * ival as f64;
                let tw = mtw * th;
                if tw * painter.GetScaleX() > 1.0 {
                    let h4 = mth * th;
                    let h5 = mah * th;
                    let mut x3 = painter.GetUserClipX1() - tw * 0.5;
                    let mut w3 = painter.GetUserClipX2() + tw * 0.5 - x3;
                    if x3 < ax {
                        x3 = ax;
                    }
                    if w3 > ax + aw - x3 {
                        w3 = ax + aw - x3;
                    }
                    let k1 = (((x3 - ax) / f + self.min - 0.01) / ival as f64).ceil() as i64;
                    let k2 = (((x3 + w3 - ax) / f + self.min + 0.01) / ival as f64).floor() as i64;
                    let mut k = k1;
                    while k <= k2 {
                        let v = k as f64 * ival as f64;
                        let mark_tx = (v - self.min) * f + ax;
                        let label = (self.text_of_value_fn)(v as i64, ival);
                        painter.PaintTextBoxed(
                            mark_tx - tw * 0.5,
                            ty,
                            tw,
                            h4,
                            &label,
                            h4,
                            col,
                            canvas_color,
                            TextAlignment::Center,
                            VAlign::Center,
                            TextAlignment::Center,
                            0.5,
                            true,
                            0.0,
                        );
                        let tri = [
                            (mark_tx - h5 * 0.5, ty + h4),
                            (mark_tx + h5 * 0.5, ty + h4),
                            (mark_tx, ty + h4 + h5),
                        ];
                        painter.PaintPolygon(&tri, col, canvas_color);
                        k += 1;
                    }
                }
                ty += th;
            }
        }

        self.border.paint_inner_overlay(painter, w, h, &self.look);
    }

    // --- Input ---

    pub fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        _input_state: &emInputState,
        ctx: &mut crate::emEngineCtx::PanelCtx,
    ) -> bool {
        // C++ emScalarField.cpp:246-268: gates on IsEditable() && IsEnabled().
        if !self.editable || !self.enabled {
            return false;
        }
        // C++ emScalarField: GetViewCondition(VCT_MIN_EXT) >= 10.0
        let min_ext = state.viewed_rect.w.min(state.viewed_rect.h);
        if min_ext < 10.0 {
            return false;
        }

        // C++ emScalarField.cpp:239: compute mouse value on every event.
        // CheckMouse returns (hit, value) — value is always computed even
        // when the mouse is outside the content round-rect.
        let (in_area, mv) = self.CheckMouse(event.mouse_x, event.mouse_y);

        // C++ absolute drag model: pressed state continuously sets value
        // to wherever the mouse points on the scale.
        if self.dragging && (mv - self.value).abs() > f64::EPSILON {
            self.SetValue(mv, ctx);
        }

        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    // C++ emScalarField.cpp:250-258: inArea && LeftButton → Pressed.
                    if !in_area {
                        return false;
                    }
                    self.dragging = true;
                    // Immediately position needle at click location.
                    if (mv - self.value).abs() > f64::EPSILON {
                        self.SetValue(mv, ctx);
                    }
                    true
                }
                InputVariant::Release => {
                    // C++ emScalarField.cpp:241-245: clear Pressed on release.
                    if !self.dragging {
                        return false;
                    }
                    self.dragging = false;
                    true
                }
                InputVariant::Repeat | InputVariant::Move => {
                    // Drag is handled above (continuous absolute positioning).
                    self.dragging
                }
            },
            // C++ emScalarField.cpp:261-272: only '+' and '-' character keys.
            // Arrow keys are NOT in C++ (would conflict with focus navigation).
            InputKey::Key('+') if event.variant == InputVariant::Press => {
                self.StepByKeyboard(1, ctx);
                true
            }
            InputKey::Key('-') if event.variant == InputVariant::Press => {
                self.StepByKeyboard(-1, ctx);
                true
            }
            _ => false,
        }
    }

    pub fn GetCursor(&self) -> emCursor {
        // C++ emScalarField doesn't override GetCursor — uses default panel cursor.
        emCursor::Normal
    }

    /// Whether this scalar field provides how-to help text.
    /// Matches C++ `emScalarField::HasHowTo` (always true).
    pub fn HasHowTo(&self) -> bool {
        true
    }

    /// Help text describing how to use this scalar field.
    ///
    /// Chains the border's base how-to with scalar-field-specific sections.
    /// Matches C++ `emScalarField::GetHowTo`.
    pub fn GetHowTo(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.GetHowTo(enabled, focusable);
        text.push_str(HOWTO_SCALAR_FIELD);
        if !self.editable {
            text.push_str(HOWTO_READ_ONLY);
        }
        text
    }

    /// Check whether (`mx`, `my`) is within the scale area and compute
    /// the corresponding value.
    ///
    /// Returns `(hit, value)` where `hit` is true when the point is inside the
    /// content round-rect and `value` is the clamped scale value corresponding
    /// to `mx` (always computed, even when outside). Matches the two-output
    /// semantics of C++ `emScalarField::CheckMouse`.
    pub fn CheckMouse(&self, mx: f64, my: f64) -> (bool, f64) {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return (false, self.min);
        }
        let tallness = self.last_h / self.last_w;
        let (content, r) = self.border.GetContentRoundRect(1.0, tallness, &self.look);
        let x = content.x;
        let y = content.y;
        let cw = content.w;
        let ch = content.h;

        let v_range: f64 = self.max - self.min;
        let all_ivals = &self.scale_mark_intervals;
        let mut ival_off = 0;
        let mut ival_cnt = all_ivals.len();
        if !self.marks_never_hidden {
            while ival_cnt > 1 && all_ivals[ival_off] as f64 > v_range {
                ival_off += 1;
                ival_cnt -= 1;
            }
        }
        let ivals = &all_ivals[ival_off..ival_off + ival_cnt];
        let mut ival_sum: u64 = 0;
        for iv in &ivals[..ival_cnt] {
            ival_sum += iv;
        }

        let mtw0 = 1.0_f64;
        let mth0 = self.text_box_tallness;
        let mah0 = mtw0.min(mth0) * 0.5;
        let d_inv = 1.0 / (mth0 + mah0);
        let mtw = mtw0 * d_inv;
        let mw = mtw * 1.5;

        let rx = x + r * 0.5;
        let rw = cw - r;
        let rh = ch - r;

        let s = rh.min(rw);
        let d_base = s * 0.04;
        let mut ax = rx + d_base;
        let mut aw = rw - 2.0 * d_base;
        let ah = rh - 2.0 * d_base;

        let mut d = s * 0.3 * 0.5 - d_base;
        if d < 0.0 {
            d = 0.0;
        }
        if ival_cnt > 0 && v_range > 0.0 {
            let th = ah;
            let f = th * ivals[0] as f64 / ival_sum as f64;
            let tw = f * mw * v_range / ivals[0] as f64;
            let mut f2 = f * mtw;
            if tw + f2 > aw {
                f2 *= aw / (tw + f2);
            }
            f2 *= 0.5;
            if d < f2 {
                d = f2;
            }
            let f_lim = aw * 0.2;
            if d > f_lim {
                d = f_lim;
            }
        }
        ax += d;
        aw -= 2.0 * d;

        // C++ hit test (lines 386-388)
        let dx = ((x - mx).max(mx - x - cw) + r).max(0.0);
        let dy = ((y - my).max(my - y - ch) + r).max(0.0);
        let hit = dx * dx + dy * dy <= r * r;

        // C++ value computation (lines 389-396)
        let mut val = (mx - ax) / aw;
        val = val * v_range + self.min;
        if val < self.min {
            val = self.min;
        }
        if val > self.max {
            val = self.max;
        }
        val = (val + 0.5).floor();
        if val < self.min {
            val = self.min;
        }
        if val > self.max {
            val = self.max;
        }
        (hit, val)
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        // C++ emScalarField inherits emBorder::GetBestTallness which uses
        // the label tallness. Compute content size from tallness-based ratio.
        let tallness = self.border.GetBestLabelTallness().max(0.1);
        let cw = 100.0;
        let ch = cw * tallness;
        self.border.preferred_size_for_content(cw, ch)
    }

    // --- Keyboard stepping (C++ StepByKeyboard parity) ---

    /// Steps the value by a keyboard increment in the given direction.
    ///
    /// Matches the C++ `StepByKeyboard` logic: if `kb_interval > 0`, uses that
    /// as step. Otherwise computes `range/129` (min 1) and finds the best
    /// matching scale mark interval. Snaps to grid with direction-dependent
    /// rounding using integer division.
    fn StepByKeyboard(&mut self, dir: i32, ctx: &mut PanelCtx<'_>) {
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

        self.SetValue(v as f64, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngineCtx::{DeferredAction, InitCtx, PanelCtx};
    use crate::emPanel::Rect;
    use crate::emPanelTree::{PanelId, PanelTree};
    use crate::emScheduler::EngineScheduler;
    use slotmap::Key as _;
    use std::cell::RefCell;

    fn test_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let id = tree.create_root("t", false);
        (tree, id)
    }

    fn default_panel_state() -> PanelState {
        PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        }
    }

    fn default_input_state() -> emInputState {
        emInputState::new()
    }

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<crate::emContext::emContext>,
        pa: Rc<RefCell<Vec<crate::emEngineCtx::FrameworkDeferredAction>>>,
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
                pa: Rc::new(RefCell::new(Vec::new())),
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
    fn value_clamping() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);

        sf.set_initial_value(50.0);
        assert!((sf.GetValue() - 50.0).abs() < 0.001);

        sf.set_initial_value(-10.0);
        assert!((sf.GetValue() - 0.0).abs() < 0.001);

        sf.set_initial_value(200.0);
        assert!((sf.GetValue() - 100.0).abs() < 0.001);
    }

    #[test]
    fn arrow_key_stepping() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        sf.SetEditable(true);
        sf.set_initial_value(50.0);

        // Cache dimensions (paint would do this in real usage)
        sf.last_w = 200.0;
        sf.last_h = 40.0;

        // emScalarField uses '+' and '-' keys (not arrow keys).
        sf.Input(
            &emInputEvent::press(InputKey::Key('+')),
            &default_panel_state(),
            &default_input_state(),
            &mut ctx,
        );
        assert!(sf.GetValue() > 50.0);

        sf.Input(
            &emInputEvent::press(InputKey::Key('-')),
            &default_panel_state(),
            &default_input_state(),
            &mut ctx,
        );
        // Should be roughly back to 50
        assert!((sf.GetValue() - 50.0).abs() < 2.0);
    }

    #[test]
    fn callback_on_change() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let look = emLook::new();
        let values = Rc::new(RefCell::new(Vec::new()));
        let val_clone = values.clone();

        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 10.0, look);
        sf.SetEditable(true);
        sf.set_initial_value(5.0);
        sf.last_w = 200.0;
        sf.last_h = 40.0;
        sf.on_value = Some(Box::new(
            move |v, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                val_clone.borrow_mut().push(v);
            },
        ));

        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
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
            sf.Input(
                &emInputEvent::press(InputKey::Key('+')),
                &default_panel_state(),
                &default_input_state(),
                &mut ctx,
            );
        }
        assert_eq!(values.borrow().len(), 1);
        assert!(values.borrow()[0] > 5.0);
    }

    #[test]
    fn scalar_field_fires_value_signal_on_input_step() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 10.0, look);
        sf.SetEditable(true);
        sf.set_initial_value(5.0);
        sf.last_w = 200.0;
        sf.last_h = 40.0;
        let sig = sf.value_signal;
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
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
            sf.Input(
                &emInputEvent::press(InputKey::Key('+')),
                &default_panel_state(),
                &default_input_state(),
                &mut ctx,
            );
        }
        assert!(__init.sched.is_pending(sig));
    }

    #[test]
    fn editable_toggle() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);

        assert!(!sf.IsEditable());
        assert_eq!(sf.border.inner, InnerBorderType::OutputField);

        sf.SetEditable(true);
        assert!(sf.IsEditable());
        assert_eq!(sf.border.inner, InnerBorderType::InputField);

        sf.SetEditable(false);
        assert!(!sf.IsEditable());
        assert_eq!(sf.border.inner, InnerBorderType::OutputField);

        // Input should be disabled when not editable
        sf.set_initial_value(50.0);
        sf.last_w = 200.0;
        sf.last_h = 40.0;
        let handled = sf.Input(
            &emInputEvent::press(InputKey::Key('+')),
            &default_panel_state(),
            &default_input_state(),
            &mut ctx,
        );
        assert!(!handled);
        assert!((sf.GetValue() - 50.0).abs() < 0.001);

        sf.SetEditable(true);
        assert!(sf.IsEditable());
        assert_eq!(sf.border.inner, InnerBorderType::InputField);
    }

    #[test]
    fn min_max_getters_setters() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);

        assert!((sf.GetMinValue() - 0.0).abs() < f64::EPSILON);
        assert!((sf.GetMaxValue() - 100.0).abs() < f64::EPSILON);

        // Setting min above max clamps max up
        sf.SetMinValue(200.0, &mut ctx);
        assert!((sf.GetMinValue() - 200.0).abs() < f64::EPSILON);
        assert!((sf.GetMaxValue() - 200.0).abs() < f64::EPSILON);
        assert!((sf.GetValue() - 200.0).abs() < f64::EPSILON);

        // Setting max below min clamps min down
        sf.SetMaxValue(50.0, &mut ctx);
        assert!((sf.GetMaxValue() - 50.0).abs() < f64::EPSILON);
        assert!((sf.GetMinValue() - 50.0).abs() < f64::EPSILON);

        // set_min_max_values
        sf.SetMinMaxValues(10.0, 90.0, &mut ctx);
        assert!((sf.GetMinValue() - 10.0).abs() < f64::EPSILON);
        assert!((sf.GetMaxValue() - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn constructor_clamps_max() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let sf = emScalarField::new(&mut __init.ctx(), 50.0, 10.0, look);
        // max < min => max clamped to min
        assert!((sf.GetMaxValue() - 50.0).abs() < f64::EPSILON);
        assert!((sf.GetMinValue() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn GetScaleMarkIntervals() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);

        // Default is [1]
        assert_eq!(sf.GetScaleMarkIntervals(), &[1]);

        sf.SetScaleMarkIntervals(&[100, 50, 10, 5, 1]);
        assert_eq!(sf.GetScaleMarkIntervals(), &[100, 50, 10, 5, 1]);
    }

    #[test]
    #[should_panic(expected = "strictly descending")]
    fn scale_mark_intervals_rejects_non_descending() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        sf.SetScaleMarkIntervals(&[10, 50]); // ascending — invalid
    }

    #[test]
    #[should_panic(expected = "must be > 0")]
    fn scale_mark_intervals_rejects_zero() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        sf.SetScaleMarkIntervals(&[0]);
    }

    #[test]
    fn scale_mark_intervals_empty_is_ok() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        sf.SetScaleMarkIntervals(&[]);
        assert_eq!(sf.GetScaleMarkIntervals(), &[] as &[u64]);
    }

    #[test]
    fn never_hide_marks() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        assert!(!sf.IsNeverHidingMarks());
        sf.SetNeverHideMarks(true);
        assert!(sf.IsNeverHidingMarks());
    }

    #[test]
    fn GetTextBoxTallness() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        assert!((sf.GetTextBoxTallness() - 0.5).abs() < f64::EPSILON);
        sf.SetTextBoxTallness(0.75);
        assert!((sf.GetTextBoxTallness() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn GetKeyboardInterval() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        assert_eq!(sf.GetKeyboardInterval(), 0);
        sf.SetKeyboardInterval(5);
        assert_eq!(sf.GetKeyboardInterval(), 5);
    }

    #[test]
    fn step_by_keyboard_with_explicit_interval() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        sf.SetEditable(true);
        sf.SetKeyboardInterval(10);
        sf.set_initial_value(50.0);
        sf.last_w = 200.0;
        sf.last_h = 40.0;

        sf.Input(
            &emInputEvent::press(InputKey::Key('+')),
            &default_panel_state(),
            &default_input_state(),
            &mut ctx,
        );
        assert!((sf.GetValue() - 60.0).abs() < 1.0);

        sf.Input(
            &emInputEvent::press(InputKey::Key('-')),
            &default_panel_state(),
            &default_input_state(),
            &mut ctx,
        );
        assert!((sf.GetValue() - 50.0).abs() < 1.0);
    }

    #[test]
    fn custom_text_of_value() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        sf.SetTextOfValueFunc(Box::new(|val, _iv| format!("{}%", val)));
        // The function is stored and usable
        let text = (sf.text_of_value_fn)(50, 1);
        assert_eq!(text, "50%");
    }

    #[test]
    fn plus_minus_keys_work() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        sf.SetEditable(true);
        sf.set_initial_value(50.0);
        sf.last_w = 200.0;
        sf.last_h = 40.0;

        let handled = sf.Input(
            &emInputEvent::press(InputKey::Key('+')),
            &default_panel_state(),
            &default_input_state(),
            &mut ctx,
        );
        assert!(handled);
        assert!(sf.GetValue() > 50.0);
    }

    #[test]
    fn cursor_is_always_normal() {
        let mut __init = TestInit::new();
        // C++ doesn't override GetCursor — always default panel cursor.
        let look = emLook::new();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        assert_eq!(sf.GetCursor(), emCursor::Normal);
        sf.SetEditable(false);
        assert_eq!(sf.GetCursor(), emCursor::Normal);
    }

    #[test]
    fn set_value_fires_callback() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let look = emLook::new();
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();
        let mut sf = emScalarField::new(&mut __init.ctx(), 0.0, 100.0, look);
        sf.on_value = Some(Box::new(
            move |_v, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                *count_clone.borrow_mut() += 1;
            },
        ));

        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
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
        sf.SetValue(50.0, &mut ctx);
        assert_eq!(*count.borrow(), 1);

        // Setting same value should not fire
        sf.SetValue(50.0, &mut ctx);
        assert_eq!(*count.borrow(), 1);
    }
}
