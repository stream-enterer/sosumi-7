// SPLIT: Split from emImageFile.h — panel type extracted
use crate::emColor::emColor;
use crate::emFilePanel::emFilePanel;
use crate::emImage::emImage;
use crate::emPainter::emPainter;
use crate::emPanel::{PanelBehavior, PanelState};

/// A panel that displays an image file with aspect-ratio preservation.
///
/// Port of C++ `emImageFilePanel`. Wraps a `emFilePanel` for status display
/// and holds a cached copy of the current image for painting.
pub struct emImageFilePanel {
    file_panel: emFilePanel,
    current_image: Option<emImage>,
}

impl Default for emImageFilePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl emImageFilePanel {
    pub fn new() -> Self {
        Self {
            file_panel: emFilePanel::new(),
            current_image: None,
        }
    }

    pub fn with_model() -> Self {
        Self {
            file_panel: emFilePanel::new(),
            current_image: None,
        }
    }

    pub fn file_panel(&self) -> &emFilePanel {
        &self.file_panel
    }

    pub fn file_panel_mut(&mut self) -> &mut emFilePanel {
        &mut self.file_panel
    }

    /// Update the cached image for painting.
    pub fn set_current_image(&mut self, image: Option<emImage>) {
        self.current_image = image;
    }

    /// Calculate the aspect-ratio-preserving rectangle for the image within
    /// the panel bounds. Returns `(x, y, w, h)` or `None` if no image.
    ///
    /// Port of C++ `emImageFilePanel::GetEssenceRect`. The image is centered
    /// within panel width 1.0 and proportional height.
    pub fn GetEssenceRect(&self, panel_w: f64, panel_h: f64) -> Option<(f64, f64, f64, f64)> {
        let image = self.current_image.as_ref()?;
        let iw = image.GetWidth() as f64;
        let ih = image.GetHeight() as f64;
        if iw <= 0.0 || ih <= 0.0 || panel_w <= 0.0 || panel_h <= 0.0 {
            return None;
        }

        let image_aspect = iw / ih;
        let panel_aspect = panel_w / panel_h;

        if image_aspect > panel_aspect {
            // emImage is wider than panel — fit to width, center vertically
            let w = panel_w;
            let h = panel_w / image_aspect;
            let x = 0.0;
            let y = (panel_h - h) * 0.5;
            Some((x, y, w, h))
        } else {
            // emImage is taller than panel — fit to height, center horizontally
            let h = panel_h;
            let w = panel_h * image_aspect;
            let x = (panel_w - w) * 0.5;
            let y = 0.0;
            Some((x, y, w, h))
        }
    }
}

impl PanelBehavior for emImageFilePanel {
    fn IsOpaque(&self) -> bool {
        if self.file_panel.GetVirFileState().is_good() {
            false
        } else {
            self.file_panel.IsOpaque()
        }
    }

    fn GetCanvasColor(&self) -> emColor {
        self.file_panel.GetCanvasColor()
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        if !self.file_panel.GetVirFileState().is_good() {
            self.file_panel.Paint(painter, canvas_color, w, h, state);
            return;
        }

        if let Some(ref image) = self.current_image {
            if let Some((ix, iy, iw, ih)) = self.GetEssenceRect(w, h) {
                painter.paint_image_full(ix, iy, iw, ih, image, 255, canvas_color);
            }
        }
    }
}
