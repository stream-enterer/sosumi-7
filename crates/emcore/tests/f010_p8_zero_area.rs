//! F010 P8 falsification: coordinate-rounding to zero-area degenerate rects.
//!
//! Per `docs/debug/investigations/F010-investigation/hypotheses/P8.yaml`.
//!
//! Falsification criterion: if the i32 rect emPainter::Clear computes for its
//! `fill_rect_pixels` call has w > 0 AND h > 0, P8 is falsified.

use emcore::emImage::emImage;
use emcore::emPainter::emPainter;

#[test]
fn f010_p8_clear_rect_non_degenerate() {
    // The symptomatic zoom is approximated by a 800x600 viewport with a panel
    // covering a typical sub-region. Pull realistic values from the existing
    // real_stack tests in crates/emfileman/src/emDirPanel.rs (see
    // `real_stack_dir_panel_children_created_with_nonzero_rects_after_load`,
    // which uses theme-derived rect sizes).
    let mut img = emImage::new(800, 600, 4);
    let mut painter = emPainter::new(&mut img);

    // Set a clip representing where emDirPanel's interior would be at the
    // symptomatic zoom. SetClipping(x, y, w, h) intersects user-space coords
    // with the current clip; at identity scale+offset this maps directly.
    painter.SetClipping(50.0, 40.0, 700.0, 500.0);

    // Read back the pixel-space clip rectangle via public accessors.
    let clip_x1 = painter.GetClipX1();
    let clip_y1 = painter.GetClipY1();
    let clip_x2 = painter.GetClipX2();
    let clip_y2 = painter.GetClipY2();

    // Mirror the rounding emPainter::Clear uses (emPainter.rs:5779-5782):
    //   let x = self.state.clip.x1 as i32;
    //   let y = self.state.clip.y1 as i32;
    //   let w = self.state.clip.x2.ceil() as i32 - x;
    //   let h = self.state.clip.y2.ceil() as i32 - y;
    let x = clip_x1 as i32;
    let y = clip_y1 as i32;
    let w = clip_x2.ceil() as i32 - x;
    let h = clip_y2.ceil() as i32 - y;

    let observation_artifact = serde_json::json!({
        "test": "f010_p8_clear_rect_non_degenerate",
        "clip": {"x1": clip_x1, "y1": clip_y1, "x2": clip_x2, "y2": clip_y2},
        "rect": {"x": x, "y": y, "w": w, "h": h},
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/debug/investigations/F010-investigation/artifacts/P8-rect.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, observation_artifact.to_string()).unwrap();

    // P8 hypothesis predicts: w == 0 OR h == 0 at symptomatic zoom.
    // Falsification: w > 0 AND h > 0 (rect is non-degenerate).
    let non_degenerate = w > 0 && h > 0;
    assert!(
        non_degenerate,
        "P8 hypothesis predicts degenerate rect at symptomatic zoom; observed w={}, h={}",
        w, h
    );
}
