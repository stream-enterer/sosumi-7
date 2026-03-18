use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::layout::{Orientation, ResolvedOrientation};
use crate::panel::PanelCtx;
use crate::render::{Painter, BORDER_EDGES_ONLY};

use super::look::Look;
use super::toolkit_images::with_toolkit_images;
use crate::foundation::Color;
use std::rc::Rc;

/// C++ emSplitter grip fraction: 0.015 * borderScaling (=1.0 for default look).
const GRIP_FRACTION: f64 = 0.015;

/// Resizable two-panel divider widget.
pub struct Splitter {
    look: Rc<Look>,
    orientation: Orientation,
    position: f64,
    min_position: f64,
    max_position: f64,
    dragging: bool,
    drag_offset: f64,
    /// Cached dimensions from the last paint call (like Eagle Mode's
    /// GetContentRect pattern — widgets query their own dimensions during input).
    last_w: f64,
    last_h: f64,
    pub on_position: Option<Box<dyn FnMut(f64)>>,
}

impl Splitter {
    pub fn new(orientation: Orientation, look: Rc<Look>) -> Self {
        Self {
            look,
            orientation,
            position: 0.5,
            min_position: 0.0,
            max_position: 1.0,
            dragging: false,
            drag_offset: 0.0,
            last_w: 0.0,
            last_h: 0.0,
            on_position: None,
        }
    }

    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn min_position(&self) -> f64 {
        self.min_position
    }

    pub fn max_position(&self) -> f64 {
        self.max_position
    }

    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
    }

    pub fn set_position(&mut self, pos: f64) {
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
    pub fn set_limits(&mut self, min: f64, max: f64) {
        let mut min = min.clamp(0.0, 1.0);
        let mut max = max.clamp(0.0, 1.0);
        if min > max {
            let avg = (min + max) * 0.5;
            min = avg;
            max = avg;
        }
        self.min_position = min;
        self.max_position = max;
        self.set_position(self.position);
    }

    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64) {
        self.last_w = w;
        self.last_h = h;

        let resolved = self.orientation.resolve(w, h);
        let color = self.look.button_bg_color;
        let canvas = painter.canvas_color();

        let (gx, gy, gw, gh) = self.calc_grip_rect(w, h, resolved);
        painter.paint_rect(gx, gy, gw, gh, color, Color::TRANSPARENT);

        // C++ emSplitter: PaintBorderImage overlay on grip.
        let d = gw.min(gh) * 0.5;
        with_toolkit_images(|img| {
            let image = if self.dragging {
                &img.splitter_pressed
            } else {
                &img.splitter
            };
            painter.paint_border_image(
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
                255,
                color,
                BORDER_EDGES_ONLY,
            );
        });
        let _ = canvas;
    }

    /// Compute grip rectangle matching C++ emSplitter::CalcGripRect.
    fn calc_grip_rect(
        &self,
        w: f64,
        h: f64,
        resolved: ResolvedOrientation,
    ) -> (f64, f64, f64, f64) {
        match resolved {
            ResolvedOrientation::Horizontal => {
                let mut gs = GRIP_FRACTION * w;
                if gs > w * 0.5 {
                    gs = w * 0.5;
                }
                let gx = self.position * (w - gs);
                (gx, 0.0, gs, h)
            }
            ResolvedOrientation::Vertical => {
                let mut gs = GRIP_FRACTION * h;
                if gs > h * 0.5 {
                    gs = h * 0.5;
                }
                let gy = self.position * (h - gs);
                (0.0, gy, w, gs)
            }
        }
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        let w = self.last_w;
        let h = self.last_h;
        let resolved = self.orientation.resolve(w, h);
        let (gx, gy, gw, gh) = self.calc_grip_rect(w, h, resolved);

        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    let hit = match resolved {
                        ResolvedOrientation::Horizontal => {
                            event.mouse_x >= gx && event.mouse_x <= gx + gw
                        }
                        ResolvedOrientation::Vertical => {
                            event.mouse_y >= gy && event.mouse_y <= gy + gh
                        }
                    };
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
                            ResolvedOrientation::Horizontal => (event.mouse_x, w, gw),
                            ResolvedOrientation::Vertical => (event.mouse_y, h, gh),
                        };
                        let travel = size - gs;
                        if travel > 0.0 {
                            let new_pos = (pos - self.drag_offset - gs * 0.5) / travel;
                            self.set_position(new_pos);
                        }
                        return true;
                    }
                    false
                }
            },
            _ => false,
        }
    }

    pub fn get_cursor(&self) -> Cursor {
        match self.orientation.resolve(self.last_w, self.last_h) {
            ResolvedOrientation::Horizontal => Cursor::ResizeEW,
            ResolvedOrientation::Vertical => Cursor::ResizeNS,
        }
    }

    /// Layout two child panels based on the splitter position.
    pub fn layout_children(&self, ctx: &mut PanelCtx, w: f64, h: f64) {
        let children = ctx.children();
        if children.len() < 2 {
            return;
        }

        let resolved = self.orientation.resolve(w, h);
        let (gx, gy, gw, gh) = self.calc_grip_rect(w, h, resolved);

        match resolved {
            ResolvedOrientation::Horizontal => {
                ctx.layout_child(children[0], 0.0, 0.0, gx.max(0.0), h);
                ctx.layout_child(children[1], gx + gw, 0.0, (w - gx - gw).max(0.0), h);
            }
            ResolvedOrientation::Vertical => {
                ctx.layout_child(children[0], 0.0, 0.0, w, gy.max(0.0));
                ctx.layout_child(children[1], 0.0, gy + gh, w, (h - gy - gh).max(0.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitter_position_clamping() {
        let look = Look::new();
        let mut sp = Splitter::new(Orientation::Horizontal, look);
        sp.set_position(0.3);
        assert!((sp.position() - 0.3).abs() < 0.001);

        sp.set_position(-1.0);
        assert!((sp.position() - 0.0).abs() < 0.001);

        sp.set_position(2.0);
        assert!((sp.position() - 1.0).abs() < 0.001);
    }

    #[test]
    fn splitter_drag() {
        let look = Look::new();
        let mut sp = Splitter::new(Orientation::Horizontal, look);
        sp.set_position(0.5);

        // Simulate a paint call to cache dimensions (100x50 panel)
        // In real usage, paint() is always called before input().
        sp.last_w = 100.0;
        sp.last_h = 50.0;

        // Press at the divider center (x = 50.0 in 100px wide panel)
        let press = InputEvent::press(InputKey::MouseLeft).with_mouse(50.0, 10.0);
        assert!(sp.input(&press));
        assert!(sp.dragging);

        // Drag to x = 70.0
        let drag = InputEvent {
            key: InputKey::MouseLeft,
            variant: InputVariant::Repeat,
            chars: String::new(),
            repeat: 0,
            source_variant: 0,
            mouse_x: 70.0,
            mouse_y: 10.0,
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
            eaten: false,
        };
        sp.input(&drag);
        assert!((sp.position() - 0.7).abs() < 0.01);

        // Release
        let release = InputEvent::release(InputKey::MouseLeft);
        sp.input(&release);
        assert!(!sp.dragging);
    }
}
