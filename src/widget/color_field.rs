use std::rc::Rc;

use crate::foundation::Color;
use crate::input::{InputEvent, InputKey, InputVariant};
use crate::panel::PanelCtx;
use crate::render::Painter;

use super::border::{Border, OuterBorderType};
use super::look::Look;

/// RGBA color editor widget.
pub struct ColorField {
    border: Border,
    look: Rc<Look>,
    color: Color,
    expanded: bool,
    pub on_color: Option<Box<dyn FnMut(Color)>>,
}

const SWATCH_SIZE: f64 = 20.0;

impl ColorField {
    pub fn new(look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::Rect),
            look,
            color: Color::BLACK,
            expanded: false,
            on_color: None,
        }
    }

    pub fn color(&self) -> Color {
        self.color
    }

    pub fn set_color(&mut self, color: Color) {
        if self.color != color {
            self.color = color;
            if let Some(cb) = &mut self.on_color {
                cb(color);
            }
        }
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border.paint_border(painter, w, h, &self.look, false);

        let (cx, cy, cw, ch) = self.border.content_rect(w, h, &self.look);

        // Color swatch
        let sw = if self.expanded {
            SWATCH_SIZE.min(cw)
        } else {
            cw
        };
        painter.paint_rect(cx, cy, sw, ch.min(SWATCH_SIZE), self.color);
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        match event.key {
            InputKey::MouseLeft if event.variant == InputVariant::Release => {
                self.expanded = !self.expanded;
                true
            }
            _ => false,
        }
    }

    /// Layout child scalar fields for R, G, B, A editing when expanded.
    pub fn layout_children(&self, ctx: &mut PanelCtx, w: f64, h: f64) {
        let children = ctx.children();
        if !self.expanded {
            // Hide all children
            for &child in &children {
                ctx.layout_child(child, 0.0, 0.0, 0.0, 0.0);
            }
            return;
        }

        let (cx, cy, cw, _ch) = self.border.content_rect(w, h, &self.look);
        let field_h = 16.0;
        let start_y = cy + SWATCH_SIZE + 2.0;

        // Expect 4 children (R, G, B, A scalar fields)
        for (i, &child) in children.iter().take(4).enumerate() {
            ctx.layout_child(child, cx, start_y + i as f64 * (field_h + 2.0), cw, field_h);
        }
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
}
