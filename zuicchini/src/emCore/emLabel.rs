use std::rc::Rc;

use crate::emCore::emImage::emImage;
use crate::emCore::emPainter::{emPainter, TextAlignment};

use super::emBorder::{emBorder, OuterBorderType};
use crate::emCore::emLook::emLook;

/// Non-focusable text display widget.
///
/// C++ `emLabel` inherits from `emBorder`. Constructor accepts caption,
/// description, and icon. Paint delegates to `PaintLabel` → `DoLabel`.
///
/// This Rust port now delegates painting to `border.paint_label`, which
/// implements the full DoLabel layout (icon + caption + description with
/// configurable alignment).
pub struct emLabel {
    border: emBorder,
    look: Rc<emLook>,
}

impl emLabel {
    pub fn new(caption: &str, look: Rc<emLook>) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::Margin)
                .with_caption(caption)
                .with_label_in_border(false),
            look,
        }
    }

    /// Construct with caption, description, and icon.
    /// Matches C++ `emLabel::emLabel(parent, name, caption, description, icon)`.
    pub fn with_label(
        caption: &str,
        description: &str,
        icon: Option<emImage>,
        look: Rc<emLook>,
    ) -> Self {
        let mut border = emBorder::new(OuterBorderType::Margin)
            .with_caption(caption)
            .with_label_in_border(false);
        if !description.is_empty() {
            border = border.with_description(description);
        }
        if let Some(img) = icon {
            border = border.with_icon(img);
        }
        Self { border, look }
    }

    pub fn set_caption(&mut self, text: &str) {
        self.border.caption = text.to_string();
    }

    pub fn caption(&self) -> &str {
        &self.border.caption
    }

    pub fn set_description(&mut self, text: &str) {
        self.border.description = text.to_string();
    }

    pub fn description(&self) -> &str {
        &self.border.description
    }

    pub fn set_icon(&mut self, icon: Option<emImage>) {
        self.border.set_icon(icon);
    }

    /// Set horizontal alignment of the label block within content area.
    /// Matches C++ `emBorder::SetLabelAlignment`.
    pub fn set_label_alignment(&mut self, a: TextAlignment) {
        self.border.label_alignment = a;
    }

    /// Set text line alignment for the caption.
    /// Matches C++ `emBorder::SetCaptionAlignment`.
    pub fn set_caption_alignment(&mut self, a: TextAlignment) {
        self.border.set_caption_alignment(Some(a));
    }

    /// Set text line alignment for the description.
    /// Matches C++ `emBorder::SetDescriptionAlignment`.
    pub fn set_description_alignment(&mut self, a: TextAlignment) {
        self.border.set_description_alignment(Some(a));
    }

    /// Paint the label.
    ///
    /// C++ `emLabel::PaintContent` calls `PaintLabel` with the content rect
    /// and fg_color (dimmed when disabled). The border's `paint_label`
    /// implements the full DoLabel layout including icon, caption, and
    /// description with configurable alignment.
    pub fn PaintContent(&self, painter: &mut emPainter, w: f64, h: f64, enabled: bool) {
        self.border
            .paint_border(painter, w, h, &self.look, false, enabled, 1.0);

        let cr = self.border.content_rect(w, h, &self.look);
        if cr.w <= 0.0 || cr.h <= 0.0 {
            return;
        }

        // C++ emLabel::PaintContent delegates to PaintLabel → DoLabel.
        // border.paint_label handles the full layout (icon + caption +
        // description) with alignment and disabled dimming.
        self.border.paint_label(painter, cr, &self.look, enabled);
    }

    // DIVERGED: no C++ equivalent — Rust-only layout helper
    pub fn preferred_size(&self) -> (f64, f64) {
        let ch = 13.0;
        let tw = emPainter::measure_text_width(&self.border.caption, ch);
        let lh = ch + 2.0;
        (tw + 4.0, lh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_caption() {
        let look = emLook::new();
        let mut label = emLabel::new("Hello", look);
        assert_eq!(label.caption(), "Hello");
        label.set_caption("World");
        assert_eq!(label.caption(), "World");
    }

    #[test]
    fn label_preferred_size() {
        let look = emLook::new();
        let label = emLabel::new("Test", look);
        let (w, h) = label.preferred_size();
        assert!(w > 4.0, "Label should have positive width");
        assert!(h > 0.0, "Label should have positive height");
    }

    #[test]
    fn label_with_description() {
        let look = emLook::new();
        let label = emLabel::with_label("Title", "A longer description", None, look);
        assert_eq!(label.caption(), "Title");
        assert_eq!(label.description(), "A longer description");
    }

    #[test]
    fn label_set_description() {
        let look = emLook::new();
        let mut label = emLabel::new("Title", look);
        assert!(label.description().is_empty());
        label.set_description("Desc");
        assert_eq!(label.description(), "Desc");
    }
}
