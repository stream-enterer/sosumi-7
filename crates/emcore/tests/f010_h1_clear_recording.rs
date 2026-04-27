//! F010 H1 inversion test: post-fix, emPainter::Clear records exactly one
//! DrawOp::Clear in recording mode.
//!
//! Pre-fix this test would FAIL (Clear records nothing). Post-fix it PASSES.
//! Inversion is the regression marker for the H1 dispatch-hole fix.
//!
//! Per `docs/debug/investigations/F010-investigation/hypotheses/H1.yaml` and
//! the fix spec at `docs/superpowers/specs/2026-04-26-F010-h1-fix-design.md`.

use emcore::emColor::emColor;
use emcore::emPainter::emPainter;
use emcore::emPainterDrawList::{DrawOp, RecordedOp};

#[test]
fn f010_h1_clear_records_one_op() {
    let mut ops: Vec<RecordedOp> = Vec::new();
    let target_color = emColor::rgba(255, 0, 0, 255);
    {
        let mut painter = emPainter::new_recording(800, 600, &mut ops);
        painter.Clear(target_color);
    }
    let len_after = ops.len();

    let observation_artifact = serde_json::json!({
        "test": "f010_h1_clear_records_one_op",
        "len_after": len_after,
        "ops_added_by_clear": len_after,
        "fix_landed": true,
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/debug/investigations/F010-investigation/artifacts/H1-ops.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, observation_artifact.to_string()).unwrap();

    assert_eq!(
        len_after, 1,
        "post-fix: Clear must record exactly one op, got {}",
        len_after
    );
    match &ops[0].op {
        DrawOp::Clear { color } => {
            assert_eq!(*color, target_color, "recorded color mismatches caller's");
        }
        other => panic!("expected DrawOp::Clear, got {:?}", other),
    }
}
