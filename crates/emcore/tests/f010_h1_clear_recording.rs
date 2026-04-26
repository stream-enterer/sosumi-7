//! F010 H1 falsification: emPainter::Clear silently dropped in recording mode.
//!
//! Per `docs/debug/investigations/F010-investigation/hypotheses/H1.yaml`.
//!
//! Falsification criterion: if recording painter's ops vec contains any op
//! contributed by a Clear call, H1 is falsified.

use emcore::emColor::emColor;
use emcore::emPainter::emPainter;
use emcore::emPainterDrawList::RecordedOp;

#[test]
fn f010_h1_clear_records_no_op() {
    // len_before: ops vec is empty at construction time — new_recording pushes
    // no ops during initialization.
    let mut ops: Vec<RecordedOp> = Vec::new();
    let len_before = ops.len(); // 0 — captured before painter borrows ops

    {
        let mut painter = emPainter::new_recording(800, 600, &mut ops);
        painter.Clear(emColor::rgba(255, 0, 0, 255));
        // painter dropped here, releasing the &mut borrow on ops
    }

    let len_after = ops.len();
    let ops_added_by_clear = len_after - len_before;

    let observation_artifact = serde_json::json!({
        "test": "f010_h1_clear_records_no_op",
        "len_before": len_before,
        "len_after": len_after,
        "ops_added_by_clear": ops_added_by_clear,
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/debug/investigations/F010-investigation/artifacts/H1-ops.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, observation_artifact.to_string()).unwrap();

    // Test PASSES under the hypothesis (ops_added_by_clear == 0). Test FAILS
    // (and H1 is falsified) if any op is added.
    assert_eq!(
        ops_added_by_clear, 0,
        "H1 hypothesis predicts Clear records nothing; observed {} ops added",
        ops_added_by_clear
    );
}
