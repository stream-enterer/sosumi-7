use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emImage::emImage;
use emcore::emPainter::emPainter;
use emcore::emPainterDrawList::RecordedOp;
use emcore::emPanel::{PanelBehavior, PanelState};

use emMain::emVirtualCosmos::{emVirtualCosmosItemPanel, emVirtualCosmosItemRec};

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

fn test_item_rec() -> emVirtualCosmosItemRec {
    let mut rec = emVirtualCosmosItemRec::default();
    rec.Name = "TestItem".to_string();
    rec.Title = "Test Cosmos Item".to_string();
    rec.Width = 1.0;
    rec.ContentTallness = 0.75;
    rec.BorderScaling = 1.0;
    rec.BackgroundColor = emColor::from_packed(0x202040FF);
    rec.BorderColor = emColor::from_packed(0x4060A0FF);
    rec.TitleColor = emColor::from_packed(0xE0E0FFFF);
    rec
}

#[test]
fn cosmos_item_border() {
    require_golden!();

    let (ew, eh, expected) = load_painter_golden("cosmos_item_border");
    assert_eq!(ew, 400);
    assert_eq!(eh, 300);

    // Build the panel and set the test item record.
    let ctx = emcore::emContext::emContext::NewRoot();
    let mut panel = emVirtualCosmosItemPanel::new(Rc::clone(&ctx));
    let rec = test_item_rec();

    // Compute panel height: contentTallness + top_border + bottom_border.
    // b = min(0.75, 1.0) * 1.0 = 0.75
    // top = b * 0.05 = 0.0375, bottom = b * 0.03 = 0.0225
    // panel_h = 0.75 + 0.0375 + 0.0225 = 0.81
    let b_val = rec.ContentTallness.min(1.0) * rec.BorderScaling;
    let bt = b_val * 0.05;
    let bb = b_val * 0.03;
    let panel_h = rec.ContentTallness + bt + bb;

    panel.SetItemRec(rec);

    let sx = 400.0;
    let sy = 300.0 / panel_h;
    let state = PanelState::default_for_test();

    // Create 400x300 RGBA image, fill black (canvas color).
    let mut img = emImage::new(400, 300, 4);
    img.fill(emColor::BLACK);

    {
        // Map panel coords (0,0)-(1.0, panel_h) to pixels (0,0)-(400,300).
        let mut p = emPainter::new(&mut img);
        p.SetCanvasColor(emColor::TRANSPARENT);
        p.scale(sx, sy);
        panel.Paint(&mut p, 1.0, panel_h, &state);
    }

    // Record DrawOps for parameter diff diagnosis when DUMP_DRAW_OPS=1.
    if dump_draw_ops_enabled() {
        let mut ops: Vec<RecordedOp> = Vec::new();
        {
            let mut rec = emPainter::new_recording(400, 300, &mut ops);
            rec.set_record_subops(true);
            rec.SetCanvasColor(emColor::TRANSPARENT);
            rec.scale(sx, sy);
            panel.Paint(&mut rec, 1.0, panel_h, &state);
        }
        dump_draw_ops("cosmos_item_border", &ops);
    }

    // ch_tol=130: Rust PaintRect passes canvas_color=paint_color (opaque fast path)
    // while C++ uses canvasColor=0 (alpha blending), producing different sub-pixel
    // coverage at rect boundaries. Affects 1 row of pixels (0.67% of image).
    compare_images("cosmos_item_border", img.GetMap(), &expected, ew, eh, 0, 0.0)
        .expect("cosmos_item_border golden mismatch");
}
