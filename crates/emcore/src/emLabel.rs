use std::rc::Rc;

use crate::emColor::emColor;
use crate::emImage::emImage;
use crate::emPainter::{emPainter, TextAlignment};

use super::emBorder::{emBorder, OuterBorderType};
use crate::emLook::emLook;

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

    pub fn SetCaption(&mut self, text: &str) {
        self.border.caption = text.to_string();
    }

    pub fn caption(&self) -> &str {
        &self.border.caption
    }

    pub fn SetDescription(&mut self, text: &str) {
        self.border.description = text.to_string();
    }

    pub fn description(&self) -> &str {
        &self.border.description
    }

    pub fn SetIcon(&mut self, icon: Option<emImage>) {
        self.border.SetIcon(icon);
    }

    /// Set horizontal alignment of the label block within content area.
    /// Matches C++ `emBorder::SetLabelAlignment`.
    pub fn SetLabelAlignment(&mut self, a: TextAlignment) {
        self.border.label_alignment = a;
    }

    /// Set text line alignment for the caption.
    /// Matches C++ `emBorder::SetCaptionAlignment`.
    pub fn SetCaptionAlignment(&mut self, a: TextAlignment) {
        self.border.SetCaptionAlignment(Some(a));
    }

    /// Set text line alignment for the description.
    /// Matches C++ `emBorder::SetDescriptionAlignment`.
    pub fn SetDescriptionAlignment(&mut self, a: TextAlignment) {
        self.border.SetDescriptionAlignment(Some(a));
    }

    /// Paint the label.
    ///
    /// C++ `emLabel::PaintContent` calls `PaintLabel` with the content rect
    /// and fg_color (dimmed when disabled). The border's `paint_label`
    /// implements the full DoLabel layout including icon, caption, and
    /// description with configurable alignment.
    pub fn PaintContent(
        &self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        enabled: bool,
        pixel_scale: f64,
    ) {
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

        let cr = self.border.GetContentRect(w, h, &self.look);
        if cr.w <= 0.0 || cr.h <= 0.0 {
            return;
        }

        // C++ emLabel::PaintContent delegates to PaintLabel → DoLabel.
        // border.paint_label handles the full layout (icon + caption +
        // description) with alignment and disabled dimming.
        self.border
            .paint_label(painter, canvas_color, cr, &self.look, enabled);
    }

    // RUST_ONLY: (language-forced-utility) — no C++ analogue; Rust-only layout helper
    // for callers that need a sizing hint outside emCore's panel layout pipeline.
    pub fn preferred_size(&self) -> (f64, f64) {
        let ch = 13.0;
        let tw = emPainter::measure_text_width(&self.border.caption, ch);
        let lh = ch + 2.0;
        (tw + 4.0, lh)
    }
}

/// `PanelBehavior` wrapper for `emLabel`. Paints the label filling the panel.
/// Used by `emDialog::ShowMessage` as the content panel behavior.
///
/// RUST_ONLY: (language-forced-utility) — C++ `emDialog::ShowMessage` (emDialog.cpp:162-180)
/// creates an `emLabel` child panel via the normal `emPanel` hierarchy; Rust's
/// panel tree stores behaviors via `PanelBehavior`, so a thin wrapper is needed
/// to adapt `emLabel` to the `PanelBehavior` trait.
pub(crate) struct LabelBehavior {
    pub(crate) label: emLabel,
}

impl crate::emPanel::PanelBehavior for LabelBehavior {
    fn IsOpaque(&self) -> bool {
        false
    }

    fn Paint(
        &mut self,
        p: &mut crate::emPainter::emPainter,
        canvas_color: crate::emColor::emColor,
        w: f64,
        h: f64,
        _state: &crate::emPanel::PanelState,
    ) {
        self.label.PaintContent(p, canvas_color, w, h, true, 1.0);
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
        label.SetCaption("World");
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
        label.SetDescription("Desc");
        assert_eq!(label.description(), "Desc");
    }
}
