use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emEngineCtx::{EngineCtx, SignalCtx};
use emcore::emPainter::emPainter;
use emcore::emSignal::SignalId;

use super::emStocksFileModel::emStocksFileModel;
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
    /// Cached SignalId from `emStocksPricesFetcher::GetChangeSignal`, captured
    /// at first-Cycle init time. `None` until `subscribed_init` flips true.
    /// Mirrors C++ ctor `AddWakeUpSignal(Fetcher.GetChangeSignal())` at
    /// `emStocksFetchPricesDialog.cpp:62`, deferred to first-Cycle per D-006.
    fetcher_change_sig: Option<SignalId>,
    /// D-006 first-Cycle init latch for the fetcher subscribe.
    subscribed_init: bool,
}

impl emStocksFetchPricesDialog {
    /// Construct the dialog without an attached `emStocksFileModel`. Useful
    /// for tests that exercise the UI surface without driving the fetcher's
    /// engine-mirror `cycle()`. Production callers must use
    /// [`new_with_model`](Self::new_with_model) so the fetcher can subscribe
    /// to `FileModel.GetChangeSignal()` + `GetFileStateSignal()` via the
    /// dialog's proxy `Cycle` (B-001-followup Phase E).
    pub fn new(api_script: &str, api_script_interpreter: &str, api_key: &str) -> Self {
        Self {
            fetcher: emStocksPricesFetcher::new(api_script, api_script_interpreter, api_key),
            label_text: String::new(),
            progress_bar: ProgressBarPanel::new(),
            finished: false,
            finish_error: String::new(),
            fetcher_change_sig: None,
            subscribed_init: false,
        }
    }

    /// Construct the dialog with an attached `emStocksFileModel` so the
    /// fetcher's proxy-engine `cycle()` can subscribe to FileModel signals
    /// (B-001-followup Phase E.1). Mirrors C++ ctor at
    /// `emStocksFetchPricesDialog.cpp:35-...` which receives the
    /// `emStocksFileModel &` and threads it into the fetcher's
    /// `emStocksPricesFetcher` ctor (`emStocksPricesFetcher.cpp:24`).
    pub fn new_with_model(
        api_script: &str,
        api_script_interpreter: &str,
        api_key: &str,
        file_model: Rc<RefCell<emStocksFileModel>>,
    ) -> Self {
        Self {
            fetcher: emStocksPricesFetcher::new(api_script, api_script_interpreter, api_key)
                .with_file_model(file_model),
            label_text: String::new(),
            progress_bar: ProgressBarPanel::new(),
            finished: false,
            finish_error: String::new(),
            fetcher_change_sig: None,
            subscribed_init: false,
        }
    }

    /// Port of C++ AddStockIds. Threads `ectx` per D-007 to forward the
    /// `Signal(ChangeSignal)` fire from the fetcher.
    pub fn AddStockIds(&mut self, ectx: &mut impl SignalCtx, stock_ids: &[String]) {
        self.fetcher.AddStockIds(ectx, stock_ids);
    }

    /// Port of C++ inline `emStocksFetchPricesDialog::AddListBox`
    /// (`emStocksFetchPricesDialog.h:78-81`). Delegates to the fetcher.
    pub fn AddListBox(&mut self, list_box: &Rc<RefCell<crate::emStocksListBox::emStocksListBox>>) {
        self.fetcher.AddListBox(list_box);
    }

    /// Port of C++ Cycle (`emStocksFetchPricesDialog.cpp:71-87`). Returns
    /// `true` while the dialog is still active, `false` once finished.
    ///
    /// Wiring: B-017 row 1 consumer-side subscribe. C++ ctor at
    /// `emStocksFetchPricesDialog.cpp:62` calls
    /// `AddWakeUpSignal(Fetcher.GetChangeSignal())`; the Rust port defers
    /// this to first-Cycle init per D-006 (the ctor has no `ectx`). The
    /// reaction body is gated on `IsSignaled(fetcher_change_sig)` to match
    /// C++ exactly — eliminating the pre-fix per-frame `UpdateControls` poll
    /// that drifted from C++ semantics.
    ///
    /// B-001-followup Phase E (this block): drive the fetcher's
    /// engine-mirror `cycle(ectx, eid)` after the consumer-side IsSignaled
    /// gate. The dialog acts as proxy engine for the fetcher per the panel-
    /// as-proxy-engine pattern (B-017 SaveTimer precedent at
    /// `emStocksFilePanel.cpp:454-...`); first call performs the deferred
    /// upstream subscribes (cpp:38-39), every call evaluates the C++ Cycle
    /// switch on `FileModel->GetFileState()` and runs the PollProcess /
    /// StartProcess body when state permits.
    pub fn Cycle(&mut self, ectx: &mut EngineCtx<'_>) -> bool {
        // First-Cycle init: subscribe to Fetcher.GetChangeSignal (D-006).
        // C++ analogue: ctor `AddWakeUpSignal(Fetcher.GetChangeSignal())` at
        // `emStocksFetchPricesDialog.cpp:62`.
        if !self.subscribed_init {
            let sig = self.fetcher.GetChangeSignal(ectx);
            ectx.connect(sig, ectx.id());
            self.fetcher_change_sig = Some(sig);
            self.subscribed_init = true;
        }

        // IsSignaled-gated reaction. Mirrors C++ `cpp:73-86`:
        //   if (IsSignaled(Fetcher.GetChangeSignal())) {
        //       UpdateControls();
        //       if (Fetcher.HasFinished()) { ...; Finish(0); }
        //   }
        let fetcher_fired = self.fetcher_change_sig.is_some_and(|s| ectx.IsSignaled(s));

        if fetcher_fired {
            self.UpdateControls();
            if self.fetcher.HasFinished() {
                let error = self.fetcher.GetError();
                if !error.is_empty() {
                    self.finish_error = error.to_string();
                }
                self.finished = true;
                return false;
            }
        }

        // B-001-followup Phase E.2: drive the fetcher's proxy engine via the
        // dialog's engine. Mirrors C++ `emStocksPricesFetcher` inheriting
        // `emEngine` (cpp:38-39 upstream subscribes); Rust language-forced
        // proxy because the fetcher is owned by the dialog and `emEngine`
        // identity is panel/dialog-bound in this codebase. No-op when no
        // FileModel was attached at construction (legacy `new()` callers).
        let eid = ectx.id();
        let _fetcher_active = self.fetcher.cycle(ectx, eid);

        // Mirror C++ `return emDialog::Cycle();` — the Rust dialog struct does
        // not embed an emDialog base today, so preserve the prior "active
        // while not finished" return value. The fetcher's own `cycle()`
        // returns its CurrentProcessActive flag, but the dialog's
        // active/finished signal is the established contract here.
        !self.finished
    }

    /// Test/internal accessor for the fetcher's upstream-subscribe latch.
    /// Used by `tests/fetcher_engine_b001_followup_phase_e.rs`.
    #[doc(hidden)]
    pub fn fetcher_subscribed_init_for_test(&self) -> bool {
        self.fetcher.subscribed_init_for_test()
    }

    /// Test/internal accessor for the fetcher's cached upstream-change SignalId.
    #[doc(hidden)]
    pub fn fetcher_file_model_change_sig_for_test(&self) -> Option<SignalId> {
        self.fetcher.file_model_change_sig_for_test()
    }

    /// Test/internal accessor for the fetcher's cached upstream-state SignalId.
    #[doc(hidden)]
    pub fn fetcher_file_model_state_sig_for_test(&self) -> Option<SignalId> {
        self.fetcher.file_model_state_sig_for_test()
    }

    /// Test/internal accessor for the fetcher's `current_process_active`.
    #[doc(hidden)]
    pub fn fetcher_current_process_active_for_test(&self) -> bool {
        self.fetcher.current_process_active
    }

    /// Test/internal accessor for the fetcher's GetError.
    #[doc(hidden)]
    pub fn fetcher_get_error_for_test(&self) -> &str {
        self.fetcher.GetError()
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

    // ── Cycle tests under a real EngineCtx (post-B-017 row 1) ──
    //
    // The Cycle signature now takes `&mut EngineCtx<'_>` so the dialog can
    // subscribe to `fetcher.GetChangeSignal()` on the first slice (D-006)
    // and gate its reaction on `IsSignaled` (matches C++ cpp:73-86).
    //
    // Each test below provisions a `TestViewHarness`, registers a no-op
    // engine, and pre-fires the fetcher's change-signal where needed so the
    // dialog observes the signal on the slice it subscribes to. (Same-slice
    // semantics: the signal's clock is bumped by `fire` and read by
    // `IsSignaled` on the same `EngineCtx`; the engine's clock starts at 0,
    // so `sig_clock > eng_clock` holds.)

    use emcore::emEngine::Priority;
    use emcore::emPanelScope::PanelScope;
    use emcore::test_view_harness::TestViewHarness;

    struct NoopEngine;
    impl emcore::emEngine::emEngine for NoopEngine {
        fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
            false
        }
    }

    /// Helper: flush pending signals so `IsSignaled` returns true for what
    /// has been fired this slice. Mirrors the `process_pending_signals`
    /// step at the top of `DoTimeSlice`'s inner loop.
    fn flush(h: &mut TestViewHarness) {
        h.scheduler.flush_signals_for_test();
    }

    /// Helper: tear down a registered test engine so the EngineScheduler's
    /// Drop-time invariant (no engines left) holds.
    fn cleanup(h: &mut TestViewHarness, eid: emcore::emEngine::EngineId) {
        h.scheduler.remove_engine(eid);
    }

    #[test]
    fn dialog_cycle_finishes_immediately_when_no_stocks() {
        let mut h = TestViewHarness::new();
        let eid = h.scheduler.register_engine(
            Box::new(NoopEngine),
            Priority::Medium,
            PanelScope::Framework,
        );

        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        // Pre-allocate the fetcher's change_signal and fire it so the dialog's
        // first-Cycle subscribe observes a fired signal on the same slice.
        // (No-stocks fetcher reports `HasFinished()=true` immediately; without
        // a fired signal the IsSignaled gate would skip the reaction.)
        {
            let mut ectx = h.engine_ctx(eid);
            let sig = dialog.fetcher.GetChangeSignal(&mut ectx);
            ectx.fire(sig);
        }
        flush(&mut h);

        let active = {
            let mut ectx = h.engine_ctx(eid);
            dialog.Cycle(&mut ectx)
        };
        assert!(!active);
        assert!(dialog.finished);
        assert!(dialog.finish_error.is_empty());
        assert_eq!(dialog.label_text, "Done");

        cleanup(&mut h, eid);
    }

    #[test]
    fn dialog_cycle_returns_true_when_in_progress() {
        let mut h = TestViewHarness::new();
        let eid = h.scheduler.register_engine(
            Box::new(NoopEngine),
            Priority::Medium,
            PanelScope::Framework,
        );

        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        {
            let mut ectx = h.engine_ctx(eid);
            // Allocate the change-signal so AddStockIds' Signal(ChangeSignal)
            // actually fires (no-op when the signal slot is null).
            let _ = dialog.fetcher.GetChangeSignal(&mut ectx);
            dialog.AddStockIds(&mut ectx, &["AAPL".to_string()]);
        }
        flush(&mut h);

        let active = {
            let mut ectx = h.engine_ctx(eid);
            dialog.Cycle(&mut ectx)
        };
        assert!(active);
        assert!(!dialog.finished);

        cleanup(&mut h, eid);
    }

    #[test]
    fn dialog_cycle_captures_error_on_finish() {
        let mut h = TestViewHarness::new();
        let eid = h.scheduler.register_engine(
            Box::new(NoopEngine),
            Priority::Medium,
            PanelScope::Framework,
        );

        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        {
            let mut ectx = h.engine_ctx(eid);
            let _ = dialog.fetcher.GetChangeSignal(&mut ectx);
            dialog.fetcher.SetFailed(&mut ectx, "timeout");
        }
        flush(&mut h);

        let active = {
            let mut ectx = h.engine_ctx(eid);
            dialog.Cycle(&mut ectx)
        };
        assert!(!active);
        assert!(dialog.finished);
        assert_eq!(dialog.finish_error, "timeout");

        cleanup(&mut h, eid);
    }

    #[test]
    fn dialog_cycle_skips_reaction_when_no_signal_fired() {
        // Post-B-017-row-1 invariant: when the fetcher's change-signal has
        // not fired, Cycle MUST NOT touch UpdateControls. This is the
        // observable improvement over the pre-fix per-frame poll and
        // matches C++ cpp:73-86.
        let mut h = TestViewHarness::new();
        let eid = h.scheduler.register_engine(
            Box::new(NoopEngine),
            Priority::Medium,
            PanelScope::Framework,
        );

        let mut dialog = emStocksFetchPricesDialog::new("", "", "");
        let active = {
            let mut ectx = h.engine_ctx(eid);
            // No fire → IsSignaled is false → reaction skipped.
            dialog.Cycle(&mut ectx)
        };
        // No-stocks fetcher HasFinished=true, but reaction is gated on the
        // signal so `finished` stays false this slice. Dialog reports active.
        assert!(active);
        assert!(!dialog.finished);
        // UpdateControls was not invoked — label_text still its initial value.
        assert!(dialog.label_text.is_empty());
        // Subscribe latch must have flipped on this slice.
        assert!(dialog.subscribed_init);
        assert!(dialog.fetcher_change_sig.is_some());

        cleanup(&mut h, eid);
    }

    #[test]
    fn dialog_add_list_box_delegates_to_fetcher() {
        let mut d = emStocksFetchPricesDialog::new("script", "", "key");
        let lb = Rc::new(RefCell::new(crate::emStocksListBox::emStocksListBox::new()));
        d.AddListBox(&lb);
        assert_eq!(
            d.fetcher
                .list_boxes
                .iter()
                .filter(|w| w.upgrade().is_some())
                .count(),
            1,
        );
    }
}
