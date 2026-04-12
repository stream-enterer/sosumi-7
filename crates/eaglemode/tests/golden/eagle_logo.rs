use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emImage::emImage;
use emcore::emPainter::emPainter;
use emcore::emPainterDrawList::RecordedOp;
use emcore::emPanel::{PanelBehavior, PanelState};

use emMain::emMainContentPanel::emMainContentPanel;

use super::common::*;
use super::draw_op_dump::{dump_draw_ops, dump_draw_ops_enabled};

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

#[test]
fn eagle_logo() {
    require_golden!();

    let (ew, eh, expected) = load_painter_golden("eagle_logo");
    assert_eq!(ew, 800);
    assert_eq!(eh, 600);

    // Create the panel and update coordinates for h=0.75.
    let ctx = emcore::emContext::emContext::NewRoot();
    let mut panel = emMainContentPanel::new(Rc::clone(&ctx));
    panel.update_coordinates(0.75);

    // Create 800x600 RGBA image, fill with the canvas color (black).
    // Canvas-blend mode works additively: target += hash(source,a) - hash(canvas,a).
    // The target must start at the canvas color for this formula to produce correct results.
    let mut img = emImage::new(800, 600, 4);
    img.fill(emColor::BLACK);

    {
        // Create painter with uniform scale: map (0,0)-(1.0,0.75) to (0,0)-(800,600).
        // scale_x = 800, scale_y = 800 (uniform, so 0.75 * 800 = 600 pixel rows).
        let mut p = emPainter::new(&mut img);
        p.SetCanvasColor(emColor::TRANSPARENT);
        p.scale(800.0, 800.0);

        // Paint with panel dimensions w=1.0, h=0.75.
        let state = PanelState::default_for_test();
        panel.Paint(&mut p, 1.0, 0.75, &state);
    }

    if dump_draw_ops_enabled() {
        let mut ops: Vec<RecordedOp> = Vec::new();
        {
            let mut rec = emPainter::new_recording(800, 600, &mut ops);
            rec.set_record_subops(true);
            rec.SetCanvasColor(emColor::TRANSPARENT);
            rec.scale(800.0, 800.0);
            let state = PanelState::default_for_test();
            panel.Paint(&mut rec, 1.0, 0.75, &state);
        }
        dump_draw_ops("eagle_logo", &ops);
    }

    // ch_tol=2: Rust paint_linear_gradient uses GetBlended (16-bit fixed-point)
    // while C++ PaintRect+emLinearGradientTexture uses per-scanline integer
    // interpolation, producing ±1–2 LSB differences across the gradient.
    compare_images("eagle_logo", img.GetMap(), &expected, ew, eh, 0, 0.0)
        .expect("eagle_logo golden mismatch");
}
