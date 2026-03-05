use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::layout::{Orientation, ResolvedOrientation};
use crate::panel::PanelCtx;
use crate::render::Painter;

use super::look::Look;
use std::rc::Rc;

const DIVIDER_SIZE: f64 = 5.0;

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
}

impl Splitter {
    pub fn new(orientation: Orientation, look: Rc<Look>) -> Self {
        Self {
            look,
            orientation,
            position: 0.5,
            min_position: 0.05,
            max_position: 0.95,
            dragging: false,
            drag_offset: 0.0,
            last_w: 0.0,
            last_h: 0.0,
        }
    }

    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn set_position(&mut self, pos: f64) {
        self.position = pos.clamp(self.min_position, self.max_position);
    }

    pub fn set_limits(&mut self, min: f64, max: f64) {
        self.min_position = min;
        self.max_position = max;
        self.position = self.position.clamp(min, max);
    }

    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64) {
        self.last_w = w;
        self.last_h = h;

        let resolved = self.orientation.resolve(w, h);
        let color = self.look.border_color;

        match resolved {
            ResolvedOrientation::Horizontal => {
                let x = (w * self.position - DIVIDER_SIZE / 2.0).max(0.0);
                painter.paint_rect(x, 0.0, DIVIDER_SIZE, h, color);
            }
            ResolvedOrientation::Vertical => {
                let y = (h * self.position - DIVIDER_SIZE / 2.0).max(0.0);
                painter.paint_rect(0.0, y, w, DIVIDER_SIZE, color);
            }
        }
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        let w = self.last_w;
        let h = self.last_h;
        let resolved = self.orientation.resolve(w, h);

        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    let (pos, size) = match resolved {
                        ResolvedOrientation::Horizontal => (event.mouse_x, w),
                        ResolvedOrientation::Vertical => (event.mouse_y, h),
                    };
                    let divider_center = size * self.position;
                    if (pos - divider_center).abs() <= DIVIDER_SIZE {
                        self.dragging = true;
                        self.drag_offset = pos - divider_center;
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
                        let (pos, size) = match resolved {
                            ResolvedOrientation::Horizontal => (event.mouse_x, w),
                            ResolvedOrientation::Vertical => (event.mouse_y, h),
                        };
                        if size > 0.0 {
                            let new_pos = (pos - self.drag_offset) / size;
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
        let half_div = DIVIDER_SIZE / 2.0;

        match resolved {
            ResolvedOrientation::Horizontal => {
                let split = w * self.position;
                ctx.layout_child(children[0], 0.0, 0.0, (split - half_div).max(0.0), h);
                ctx.layout_child(
                    children[1],
                    split + half_div,
                    0.0,
                    (w - split - half_div).max(0.0),
                    h,
                );
            }
            ResolvedOrientation::Vertical => {
                let split = h * self.position;
                ctx.layout_child(children[0], 0.0, 0.0, w, (split - half_div).max(0.0));
                ctx.layout_child(
                    children[1],
                    0.0,
                    split + half_div,
                    w,
                    (h - split - half_div).max(0.0),
                );
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
        assert!((sp.position() - 0.05).abs() < 0.001);

        sp.set_position(2.0);
        assert!((sp.position() - 0.95).abs() < 0.001);
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
            is_repeat: false,
            mouse_x: 70.0,
            mouse_y: 10.0,
        };
        sp.input(&drag);
        assert!((sp.position() - 0.7).abs() < 0.01);

        // Release
        let release = InputEvent::release(InputKey::MouseLeft);
        sp.input(&release);
        assert!(!sp.dragging);
    }
}
