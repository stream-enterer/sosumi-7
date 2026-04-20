use crate::emCursor::emCursor;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::{emPainter, BORDER_EDGES_ONLY};
use crate::emPanel::PanelState;
use crate::emEngineCtx::PanelCtx;
use crate::emPanelTree::PanelId;
use crate::emTiling::{Orientation, ResolvedOrientation};

use crate::emBorder::{emBorder, with_toolkit_images};
use crate::emLook::emLook;
use std::rc::Rc;

/// C++ emSplitter grip base fraction before borderScaling.
const GRIP_BASE: f64 = 0.015;

/// Resizable two-panel divider widget.
pub struct emSplitter {
    look: Rc<emLook>,
    orientation: Orientation,
    position: f64,
    min_position: f64,
    max_position: f64,
    /// C++ `GetBorderScaling()` — multiplied into grip size.
    border_scaling: f64,
    dragging: bool,
    drag_offset: f64,
    /// C++ `MouseInGrip` — true when mouse is over the grip area.
    mouse_in_grip: bool,
    /// Cached enabled state from last paint (for input gating).
    enabled: bool,
    /// Cached dimensions from the last paint call.
    last_w: f64,
    last_h: f64,
    pub on_position: Option<Box<dyn FnMut(f64)>>,
}

impl emSplitter {
    pub fn new(orientation: Orientation, look: Rc<emLook>) -> Self {
        Self {
            look,
            orientation,
            position: 0.5,
            min_position: 0.0,
            max_position: 1.0,
            border_scaling: 1.0,
            dragging: false,
            drag_offset: 0.0,
            mouse_in_grip: false,
            enabled: true,
            last_w: 0.0,
            last_h: 0.0,
            on_position: None,
        }
    }

    pub fn GetPos(&self) -> f64 {
        self.position
    }

    pub fn GetMinPos(&self) -> f64 {
        self.min_position
    }

    pub fn GetMaxPos(&self) -> f64 {
        self.max_position
    }

    /// Whether the splitter is currently being dragged. C++ `Pressed`.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Whether the mouse is currently over the grip area. C++ `MouseInGrip`.
    pub fn is_mouse_in_grip(&self) -> bool {
        self.mouse_in_grip
    }

    /// Set the border scaling factor. C++ `emSplitter` inherits
    /// `GetBorderScaling()` from `emBorder`; default is 1.0.
    pub fn SetBorderScaling(&mut self, s: f64) {
        self.border_scaling = s.max(1e-10);
    }

    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
    }

    pub fn SetPos(&mut self, pos: f64) {
        let clamped = pos.clamp(self.min_position, self.max_position);
        if (self.position - clamped).abs() > f64::EPSILON {
            self.position = clamped;
            if let Some(cb) = &mut self.on_position {
                cb(self.position);
            }
        }
    }

    /// Set min/max position limits with C++ validation.
    ///
    /// Clamps both to [0,1]. If min > max, averages them.
    /// Matches C++ `emSplitter::SetMinMaxPos`.
    pub fn SetMinMaxPos(&mut self, min: f64, max: f64) {
        let mut min = min.clamp(0.0, 1.0);
        let mut max = max.clamp(0.0, 1.0);
        if min > max {
            let avg = (min + max) * 0.5;
            min = avg;
            max = avg;
        }
        self.min_position = min;
        self.max_position = max;
        self.SetPos(self.position);
    }

    pub fn PaintContent(&mut self, painter: &mut emPainter, w: f64, h: f64, enabled: bool) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;

        let resolved = self.orientation.resolve(w, h);
        let color = self.look.button_bg_color;
        let canvas = painter.GetCanvasColor();

        let (gx, gy, gw, gh) = self.calc_grip_rect(w, h, resolved);
        painter.PaintRect(gx, gy, gw, gh, color, canvas);

        // C++ emSplitter: PaintBorderImage overlay on grip.
        let d = gw.min(gh) * 0.5;
        with_toolkit_images(|img| {
            let image = if self.dragging {
                &img.splitter_pressed
            } else {
                &img.splitter
            };
            painter.PaintBorderImage(
                gx,
                gy,
                gw,
                gh,
                d,
                d,
                d,
                d,
                image,
                150,
                150,
                149,
                149,
                if self.enabled { 255 } else { 64 },
                color,
                BORDER_EDGES_ONLY,
            );
        });
        let _ = canvas;
    }

    /// Compute grip rectangle matching C++ emSplitter::CalcGripRect.
    ///
    /// Origin-based version: returns coordinates relative to (0,0).
    /// Used by PaintContent and Input where the painter / coordinate system
    /// is already translated to the content origin.
    fn calc_grip_rect(
        &self,
        w: f64,
        h: f64,
        resolved: ResolvedOrientation,
    ) -> (f64, f64, f64, f64) {
        self.calc_grip_rect_abs(0.0, 0.0, w, h, resolved)
    }

    /// Compute grip rectangle matching C++ emSplitter::CalcGripRect
    /// (emSplitter.cpp:247-271).
    ///
    /// Takes explicit content rect origin `(cx, cy)` and size `(cw, ch)`.
    /// Used by LayoutChildren which computes child rects relative to the
    /// content rect.
    fn calc_grip_rect_abs(
        &self,
        cx: f64,
        cy: f64,
        cw: f64,
        ch: f64,
        resolved: ResolvedOrientation,
    ) -> (f64, f64, f64, f64) {
        let mut gs = GRIP_BASE * self.border_scaling;
        match resolved {
            ResolvedOrientation::Horizontal => {
                gs *= cw;
                if gs > cw * 0.5 {
                    gs = cw * 0.5;
                }
                (cx + self.position * (cw - gs), cy, gs, ch)
            }
            ResolvedOrientation::Vertical => {
                gs *= ch;
                if gs > ch * 0.5 {
                    gs = ch * 0.5;
                }
                (cx, cy + self.position * (ch - gs), cw, gs)
            }
        }
    }

    pub fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let resolved = self.orientation.resolve(self.last_w, self.last_h);
        let (gx, gy, gw, gh) = self.calc_grip_rect(1.0, tallness, resolved);

        // Track mouse-in-grip for cursor display (C++ MouseInGrip).
        if event.variant == InputVariant::Move {
            self.mouse_in_grip = event.mouse_x >= gx
                && event.mouse_x < gx + gw
                && event.mouse_y >= gy
                && event.mouse_y < gy + gh;
        }

        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    // C++ emSplitter.cpp:144: gates press on IsEnabled().
                    if !self.enabled {
                        return false;
                    }
                    let hit = event.mouse_x >= gx
                        && event.mouse_x < gx + gw
                        && event.mouse_y >= gy
                        && event.mouse_y < gy + gh;
                    if hit {
                        self.dragging = true;
                        let center = match resolved {
                            ResolvedOrientation::Horizontal => gx + gw * 0.5,
                            ResolvedOrientation::Vertical => gy + gh * 0.5,
                        };
                        let pos = match resolved {
                            ResolvedOrientation::Horizontal => event.mouse_x,
                            ResolvedOrientation::Vertical => event.mouse_y,
                        };
                        self.drag_offset = pos - center;
                        return true;
                    }
                    false
                }
                InputVariant::Release => {
                    if self.dragging {
                        self.dragging = false;
                        return true;
                    }
                    false
                }
                InputVariant::Repeat | InputVariant::Move => {
                    if self.dragging {
                        let (pos, size, gs) = match resolved {
                            ResolvedOrientation::Horizontal => (event.mouse_x, 1.0, gw),
                            ResolvedOrientation::Vertical => (event.mouse_y, tallness, gh),
                        };
                        let travel = size - gs;
                        if travel > 0.0 {
                            let new_pos = (pos - self.drag_offset - gs * 0.5) / travel;
                            self.SetPos(new_pos);
                        }
                        return true;
                    }
                    false
                }
            },
            _ => false,
        }
    }

    pub fn GetCursor(&self) -> emCursor {
        // C++ emSplitter.cpp:158: only resize cursor when mouse over grip AND enabled.
        if (!self.mouse_in_grip && !self.dragging) || !self.enabled {
            return emCursor::Normal;
        }
        match self.orientation.resolve(self.last_w, self.last_h) {
            ResolvedOrientation::Horizontal => emCursor::ResizeEW,
            ResolvedOrientation::Vertical => emCursor::ResizeNS,
        }
    }

    /// Layout two child panels around the grip.
    ///
    /// Ported from C++ `emSplitter::LayoutChildren()` (emSplitter.cpp:194-244).
    ///
    /// 1. Position the aux panel (C++ `emBorder::LayoutChildren()` base call).
    /// 2. Iterate children, skipping aux, and layout the first two non-aux
    ///    children: one before the grip, one after.
    /// 3. Propagate canvas color to children.
    ///
    /// The caller provides `border` (for aux panel lookup / content rect)
    /// and `canvas_color` (from `emBorder::content_canvas_color`).
    ///
    /// C++ uses `GetContentRectUnobscured()` for coordinates.  In Rust the
    /// framework already translates to content-rect space, so `(cx,cy)=(0,0)`
    /// and `(cw,ch)=(w,h)`.  No `.max(0.0)` clamps — C++ passes potentially
    /// negative child sizes, matching layout exactly.
    pub fn LayoutChildren(
        &self,
        ctx: &mut PanelCtx,
        w: f64,
        h: f64,
        border: Option<&emBorder>,
        canvas_color: crate::emColor::emColor,
    ) {
        // --- C++ line 200: emBorder::LayoutChildren() base call ---
        let aux_id: Option<PanelId> =
            border.and_then(|b| crate::emTiling::position_aux_panel(ctx, b));

        // --- C++ line 202: p = GetFirstChild(); if (!p) return; ---
        let children = ctx.children();
        if children.is_empty() {
            return;
        }

        // --- C++ lines 204-208: skip aux for first child ---
        let mut iter = children.iter().copied();
        let first = loop {
            match iter.next() {
                None => return,
                Some(id) if aux_id == Some(id) => continue,
                Some(id) => break id,
            }
        };

        // --- C++ line 209: GetContentRectUnobscured(&cx,&cy,&cw,&ch,&canvasColor) ---
        // In Rust, the painter is already translated to content origin, so
        // cx=0, cy=0, cw=w, ch=h.  canvasColor is passed in by the caller.
        let (cx, cy, cw, ch) = (0.0, 0.0, w, h);

        // --- C++ line 210: CalcGripRect(cx,cy,cw,ch,...) ---
        let resolved = self.orientation.resolve(cw, ch);
        let (gx, gy, gw, gh) = self.calc_grip_rect_abs(cx, cy, cw, ch, resolved);

        // --- C++ lines 211-222: first child layout ---
        let (x1, y1, w1, h1) = match resolved {
            ResolvedOrientation::Horizontal => (cx, cy, gx - cx, ch),
            ResolvedOrientation::Vertical => (cx, cy, cw, gy - cy),
        };
        ctx.layout_child_canvas(first, x1, y1, w1, h1, canvas_color);

        // --- C++ lines 225-230: skip aux for second child ---
        let second = loop {
            match iter.next() {
                None => return,
                Some(id) if aux_id == Some(id) => continue,
                Some(id) => break id,
            }
        };

        // --- C++ lines 231-243: second child layout ---
        let (x2, y2, w2, h2) = match resolved {
            ResolvedOrientation::Horizontal => {
                let x = gx + gw;
                (x, cy, cx + cw - x, ch)
            }
            ResolvedOrientation::Vertical => {
                let y = gy + gh;
                (cx, y, cw, cy + ch - y)
            }
        };
        ctx.layout_child_canvas(second, x2, y2, w2, h2, canvas_color);
    }

    /// Convenience wrapper when there is no border / aux panel.
    ///
    /// Used by standalone tests and simple compositions where the splitter
    /// is not embedded in an `emBorder` widget.
    pub fn LayoutChildrenSimple(&self, ctx: &mut PanelCtx, w: f64, h: f64) {
        self.LayoutChildren(ctx, w, h, None, ctx.GetCanvasColor());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanel::Rect;
    use crate::emPanelTree::PanelId;
    use slotmap::Key as _;

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

    #[test]
    fn splitter_position_clamping() {
        let look = emLook::new();
        let mut sp = emSplitter::new(Orientation::Horizontal, look);
        sp.SetPos(0.3);
        assert!((sp.GetPos() - 0.3).abs() < 0.001);

        sp.SetPos(-1.0);
        assert!((sp.GetPos() - 0.0).abs() < 0.001);

        sp.SetPos(2.0);
        assert!((sp.GetPos() - 1.0).abs() < 0.001);
    }

    #[test]
    fn splitter_drag() {
        let look = emLook::new();
        let mut sp = emSplitter::new(Orientation::Horizontal, look);
        sp.SetPos(0.5);
        let ps = default_panel_state();
        let is = default_input_state();

        // Simulate a paint call to cache dimensions (100x50 panel)
        // In real usage, paint() is always called before input().
        sp.last_w = 100.0;
        sp.last_h = 50.0;

        // Press at the divider center in normalized space (tallness = 0.5).
        // Grip center: gx = 0.5 * (1.0 - 0.015) + 0.015/2 ≈ 0.5.
        let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(0.5, 0.1);
        assert!(sp.Input(&press, &ps, &is));
        assert!(sp.dragging);

        // Drag to x = 0.7 in normalized space.
        let drag = emInputEvent {
            key: InputKey::MouseLeft,
            variant: InputVariant::Repeat,
            chars: String::new(),
            repeat: 0,
            source_variant: 0,
            mouse_x: 0.7,
            mouse_y: 0.1,
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
            eaten: false,
        };
        sp.Input(&drag, &ps, &is);
        assert!((sp.GetPos() - 0.7).abs() < 0.01);

        // Release
        let release = emInputEvent::release(InputKey::MouseLeft);
        sp.Input(&release, &ps, &is);
        assert!(!sp.dragging);
    }
}
