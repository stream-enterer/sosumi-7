use std::rc::Rc;

use crate::render::Painter;

use super::border::{Border, OuterBorderType};
use super::look::Look;

/// Non-focusable text display widget.
pub struct Label {
    border: Border,
    look: Rc<Look>,
}

impl Label {
    pub fn new(caption: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::None).with_caption(caption),
            look,
        }
    }

    pub fn set_caption(&mut self, text: &str) {
        self.border.caption = text.to_string();
    }

    pub fn caption(&self) -> &str {
        &self.border.caption
    }

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true);
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let tw = self.border.caption.len() as f64 * 7.0; // TODO(font): measure_text stub
        let lh = 15.0; // TODO(font): line_height stub
        (tw + 4.0, lh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_caption() {
        let look = Look::new();
        let mut label = Label::new("Hello", look);
        assert_eq!(label.caption(), "Hello");
        label.set_caption("World");
        assert_eq!(label.caption(), "World");
    }

    #[test]
    fn label_preferred_size() {
        let look = Look::new();
        let label = Label::new("Test", look);
        let (w, h) = label.preferred_size();
        // Width = measured text width + 4px padding
        assert!(w > 4.0, "Label should have positive width");
        assert!(h > 0.0, "Label should have positive height");
    }
}
