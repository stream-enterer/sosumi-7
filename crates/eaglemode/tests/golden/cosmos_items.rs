use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emImage::emImage;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};

use emMain::emVirtualCosmos::{emVirtualCosmosItemPanel, emVirtualCosmosItemRec};

use super::common::*;
use super::draw_op_dump::{dump_draw_ops_enabled, install_direct_op_logger};

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
        if dump_draw_ops_enabled() {
            install_direct_op_logger(&mut p, "cosmos_item_border");
        }
        p.SetCanvasColor(emColor::TRANSPARENT);
        p.scale(sx, sy);
        panel.Paint(&mut p, emColor::TRANSPARENT, 1.0, panel_h, &state);
    }

    compare_images(
        "cosmos_item_border",
        img.GetMap(),
        &expected,
        ew,
        eh,
        0,
        0.0,
    )
    .expect("cosmos_item_border golden mismatch");
}
