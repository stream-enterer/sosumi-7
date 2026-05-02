use emcore::emColor::emColor;
use emcore::emEngineCtx::SignalCtx;
use emcore::emPainter::emPainter;

use super::emStocksPricesFetcher::emStocksPricesFetcher;

/// Port of C++ emStocksFetchPricesDialog::ProgressBarPanel.
pub struct ProgressBarPanel {
    pub(crate) progress_in_percent: f64,
}

/// Background color for the progress bar: dark blue-grey.
const PROGRESS_BG_COLOR: emColor = emColor::rgb(43, 49, 70);

/// Fill color for the progress bar: blue.
const PROGRESS_FG_COLOR: emColor = emColor::rgb(109, 158, 204);

impl Default for ProgressBarPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressBarPanel {
    pub fn new() -> Self {
        Self {
            progress_in_percent: 0.0,
        }
    }

    pub fn SetProgressInPercent(&mut self, progress: f64) {
        self.progress_in_percent = progress;
    }

    /// Port of C++ ProgressBarPanel::PaintContent.
    /// Draws the progress bar fill rect within the given bounds, inset by 10% of
    /// the minimum dimension.
    pub fn PaintContent(
        &self,
        painter: &mut emPainter,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        canvas_color: emColor,
    ) {
        let d = w.min(h) * 0.1;
        let x = x + d;
        let y = y + d;
        let w = w - 2.0 * d;
        let h = h - 2.0 * d;
        // Background
        painter.PaintRect(x, y, w, h, PROGRESS_BG_COLOR, canvas_color);
        // Fill
        let fill_w = w * self.progress_in_percent / 100.0;
        if fill_w > 0.0 {
            painter.PaintRect(x, y, fill_w, h, PROGRESS_FG_COLOR, PROGRESS_BG_COLOR);
        }
    }
}

/// Port of C++ emStocksFetchPricesDialog.
pub struct emStocksFetchPricesDialog {
    pub(crate) fetcher: emStocksPricesFetcher,
    pub(crate) label_text: String,
    pub(crate) progress_bar: ProgressBarPanel,
    /// Whether the dialog has finished (set by Cycle when fetcher completes).
    pub(crate) finished: bool,
    /// Error message from the fetcher, if any, after finishing.
    pub(crate) finish_error: String,
}

impl emStocksFetchPricesDialog {
    pub fn new(api_script: &str, api_script_interpreter: &str, api_key: &str) -> Self {
        Self {
            fetcher: emStocksPricesFetcher::new(api_script, api_script_interpreter, api_key),
            label_text: String::new(),
            progress_bar: ProgressBarPanel::new(),
            finished: false,
            finish_error: String::new(),
        }
    }

    /// Port of C++ AddStockIds. Threads `ectx` per D-007 to forward the
    /// `Signal(ChangeSignal)` fire from the fetcher.
    pub fn AddStockIds(&mut self, ectx: &mut impl SignalCtx, stock_ids: &[String]) {
        self.fetcher.AddStockIds(ectx, stock_ids);
    }

    /// Port of C++ Cycle.
    /// Polls the fetcher and updates the dialog controls. Returns `true` if the
    /// dialog is still active and needs further cycling, `false` if finished.
    /// Threads `ectx` per D-007: B-001 G3 cascade — `fetcher.Signal(ChangeSignal)`
    /// fires into ectx from `StartProcess`/`PollProcess`/`SetFailed`. The ectx
    /// is currently unused at this layer (Rust Dialog::Cycle does not yet drive
    /// fetcher.Cycle — that wires up in B-017 row 1, the consumer subscribe);
    /// the parameter is added now to lock in the cascade signature.
    pub fn Cycle(&mut self, _ectx: &mut impl SignalCtx) -> bool {
        self.UpdateControls();
        if self.fetcher.HasFinished() {
            let error = self.fetcher.GetError();
            if !error.is_empty() {
                self.finish_error = error.to_string();
            }
            self.finished = true;
            return false;
        }
        true
    }

    /// Port of C++ UpdateControls.
    /// Matches C++ check order: error first, then finished, then in-progress.
    fn UpdateControls(&mut self) {
        let error = self.fetcher.GetError();
        if !error.is_empty() {
            self.label_text = format!("Error: {}", error);
            self.progress_bar
                .SetProgressInPercent(self.fetcher.GetProgressInPercent());
        } else if self.fetcher.HasFinished() {
            self.label_text = "Done".to_string();
            self.progress_bar.SetProgressInPercent(100.0);
        } else {
            if let Some(stock_id) = self.fetcher.GetCurrentStockId() {
                self.label_text = stock_id.to_string();
            } else {
                self.label_text.clear();
            }
            self.progress_bar
                .SetProgressInPercent(self.fetcher.GetProgressInPercent());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emEngineCtx::DropOnlySignalCtx;
    use emcore::emImage::emImage;

    // ── ProgressBarPanel tests ──

    #[test]
    fn progress_bar_default() {
        let pb = ProgressBarPanel::new();
        assert_eq!(pb.progress_in_percent, 0.0);
    }

    #[test]
    fn progress_bar_set() {
        let mut pb = ProgressBarPanel::new();
        pb.SetProgressInPercent(50.0);
        assert_eq!(pb.progress_in_percent, 50.0);
    }

    #[test]
    fn progress_bar_paint_content_does_not_panic() {
        let mut pb = ProgressBarPanel::new();
        pb.SetProgressInPercent(75.0);
        let mut img = emImage::new(200, 50, 4);
        let mut painter = emPainter::new(&mut img);
        pb.PaintContent(&mut painter, 0.0, 0.0, 200.0, 50.0, emColor::TRANSPARENT);
    }

    #[test]
    fn progress_bar_paint_content_zero_progress() {
        let pb = ProgressBarPanel::new();
        let mut img = emImage::new(200, 50, 4);
        let mut painter = emPainter::new(&mut img);
        // Should not panic even at 0%
        pb.PaintContent(&mut painter, 10.0, 5.0, 180.0, 40.0, emColor::TRANSPARENT);
    }

    #[test]
    fn progress_bar_paint_content_full_progress() {
        let mut pb = ProgressBarPanel::new();
        pb.SetProgressInPercent(100.0);
        let mut img = emImage::new(200, 50, 4);
        let mut painter = emPainter::new(&mut img);
        pb.PaintContent(&mut painter, 0.0, 0.0, 200.0, 50.0, emColor::TRANSPARENT);
    }

    #[test]
    fn progress_bar_colors() {
        // Verify the colors match the C++ look values
        assert_eq!(PROGRESS_BG_COLOR, emColor::rgb(43, 49, 70));
        assert_eq!(PROGRESS_FG_COLOR, emColor::rgb(109, 158, 204));
    }

    #[test]
    fn progress_bar_paint_fills_pixels() {
        let mut pb = ProgressBarPanel::new();
        pb.SetProgressInPercent(50.0);
        let mut img = emImage::new(200, 50, 4);
        img.fill(emColor::rgb(0, 0, 0));
        let mut painter = emPainter::new(&mut img);
        pb.PaintContent(&mut painter, 0.0, 0.0, 200.0, 50.0, emColor::rgb(0, 0, 0));
        // After painting, the center of the filled area should have the fg color.
        // The inset is min(200,50)*0.1 = 5, so content starts at x=5, y=5, w=190, h=40.
        // Fill width = 190 * 50/100 = 95 pixels.
        // Check a pixel in the filled zone (x=50, y=25).
        let px = img.GetPixel(50, 25);
        // The fill color is (109, 158, 204), check it was painted
        assert_eq!(px[0], 109);
        assert_eq!(px[1], 158);
        assert_eq!(px[2], 204);
    }

    // ── Dialog tests ──

    #[test]
    fn dialog_new() {
        let dialog = emStocksFetchPricesDialog::new("script.pl", "perl", "key");
        assert!(dialog.fetcher.HasFinished());
        assert_eq!(dialog.progress_bar.progress_in_percent, 0.0);
        assert!(!dialog.finished);
        assert!(dialog.finish_error.is_empty());
    }

    #[test]
    fn dialog_add_stock_ids() {
        let mut dialog = emStocksFetchPricesDialog::new("script.pl", "perl", "key");
        dialog.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string(), "2".to_string()]);
        assert!(!dialog.fetcher.HasFinished());
    }

    #[test]
    fn dialog_update_controls_finished_no_error() {
        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        // Fetcher with no stock IDs is immediately finished with no error.
        dialog.UpdateControls();
        assert_eq!(dialog.label_text, "Done");
        assert_eq!(dialog.progress_bar.progress_in_percent, 100.0);
    }

    #[test]
    fn dialog_update_controls_error_first() {
        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        dialog
            .fetcher
            .SetFailed(&mut DropOnlySignalCtx, "network error");
        dialog.UpdateControls();
        assert_eq!(dialog.label_text, "Error: network error");
    }

    #[test]
    fn dialog_update_controls_in_progress() {
        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        dialog.AddStockIds(
            &mut DropOnlySignalCtx,
            &["AAPL".to_string(), "GOOG".to_string()],
        );
        dialog.UpdateControls();
        // Current stock ID should be the label
        assert_eq!(dialog.label_text, "AAPL");
        // Progress at index 0 of 2: (0 + 0.5) * 100 / 2 = 25.0
        assert_eq!(dialog.progress_bar.progress_in_percent, 25.0);
    }

    #[test]
    fn dialog_cycle_finishes_immediately_when_no_stocks() {
        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        let active = dialog.Cycle(&mut DropOnlySignalCtx);
        assert!(!active);
        assert!(dialog.finished);
        assert!(dialog.finish_error.is_empty());
        assert_eq!(dialog.label_text, "Done");
    }

    #[test]
    fn dialog_cycle_returns_true_when_in_progress() {
        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        dialog.AddStockIds(&mut DropOnlySignalCtx, &["AAPL".to_string()]);
        let active = dialog.Cycle(&mut DropOnlySignalCtx);
        assert!(active);
        assert!(!dialog.finished);
    }

    #[test]
    fn dialog_cycle_captures_error_on_finish() {
        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        dialog.fetcher.SetFailed(&mut DropOnlySignalCtx, "timeout");
        let active = dialog.Cycle(&mut DropOnlySignalCtx);
        assert!(!active);
        assert!(dialog.finished);
        assert_eq!(dialog.finish_error, "timeout");
    }
}
