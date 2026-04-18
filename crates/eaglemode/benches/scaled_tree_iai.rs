#[allow(dead_code)]
mod common;

use emcore::emColor::emColor;
use gungraun::{library_benchmark, library_benchmark_group, main};

use emcore::emImage::emImage;
use emcore::emPanelTree::PanelTree;

use emcore::emPainter::emPainter;
use emcore::emView::emView;

use common::scaled::{build_scaled_tree, run_one_scaled_frame};
use common::{DEFAULT_VH, DEFAULT_VW};

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

type FrameState = (PanelTree, emView, emImage);

fn setup_pan_zoom(count: usize) -> FrameState {
    let (mut tree, mut view, _) = build_scaled_tree(count);
    let mut buf = emImage::new(DEFAULT_VW, DEFAULT_VH, 4);
    // Warmup
    run_one_scaled_frame(&mut tree, &mut view, &mut buf, 3.0, 0.0, 0.015);
    (tree, view, buf)
}

fn setup_paint(count: usize) -> FrameState {
    let (tree, view, _) = build_scaled_tree(count);
    let buf = emImage::new(DEFAULT_VW, DEFAULT_VH, 4);
    (tree, view, buf)
}

fn setup_update(count: usize) -> (PanelTree, emView) {
    let (tree, view, _) = build_scaled_tree(count);
    (tree, view)
}

// ---------------------------------------------------------------------------
// Pan+Zoom benchmarks
// ---------------------------------------------------------------------------

#[library_benchmark]
#[bench::run(args = (10), setup = setup_pan_zoom)]
fn pan_zoom_10(state: FrameState) {
    let (mut tree, mut view, mut buf) = state;
    run_one_scaled_frame(&mut tree, &mut view, &mut buf, 3.0, 0.0, 0.015);
}

#[library_benchmark]
#[bench::run(args = (50), setup = setup_pan_zoom)]
fn pan_zoom_50(state: FrameState) {
    let (mut tree, mut view, mut buf) = state;
    run_one_scaled_frame(&mut tree, &mut view, &mut buf, 3.0, 0.0, 0.015);
}

#[library_benchmark]
#[bench::run(args = (200), setup = setup_pan_zoom)]
fn pan_zoom_200(state: FrameState) {
    let (mut tree, mut view, mut buf) = state;
    run_one_scaled_frame(&mut tree, &mut view, &mut buf, 3.0, 0.0, 0.015);
}

// ---------------------------------------------------------------------------
// Paint benchmarks
// ---------------------------------------------------------------------------

#[library_benchmark]
#[bench::run(args = (10), setup = setup_paint)]
fn paint_10(state: FrameState) {
    let (mut tree, view, mut buf) = state;
    buf.fill(emColor::BLACK);
    let mut painter = emPainter::new(&mut buf);
    view.Paint(&mut tree, &mut painter, emColor::TRANSPARENT);
}

#[library_benchmark]
#[bench::run(args = (50), setup = setup_paint)]
fn paint_50(state: FrameState) {
    let (mut tree, view, mut buf) = state;
    buf.fill(emColor::BLACK);
    let mut painter = emPainter::new(&mut buf);
    view.Paint(&mut tree, &mut painter, emColor::TRANSPARENT);
}

#[library_benchmark]
#[bench::run(args = (200), setup = setup_paint)]
fn paint_200(state: FrameState) {
    let (mut tree, view, mut buf) = state;
    buf.fill(emColor::BLACK);
    let mut painter = emPainter::new(&mut buf);
    view.Paint(&mut tree, &mut painter, emColor::TRANSPARENT);
}

// ---------------------------------------------------------------------------
// Update benchmarks
// ---------------------------------------------------------------------------

#[library_benchmark]
#[bench::run(args = (10), setup = setup_update)]
fn update_10(state: (PanelTree, emView)) {
    let (mut tree, mut view) = state;
    tree.HandleNotice(true, 1.0);
    view.Update(&mut tree);
}

#[library_benchmark]
#[bench::run(args = (50), setup = setup_update)]
fn update_50(state: (PanelTree, emView)) {
    let (mut tree, mut view) = state;
    tree.HandleNotice(true, 1.0);
    view.Update(&mut tree);
}

#[library_benchmark]
#[bench::run(args = (200), setup = setup_update)]
fn update_200(state: (PanelTree, emView)) {
    let (mut tree, mut view) = state;
    tree.HandleNotice(true, 1.0);
    view.Update(&mut tree);
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------

library_benchmark_group!(
    name = scaled_pan_zoom,
    benchmarks = [pan_zoom_10, pan_zoom_50, pan_zoom_200]
);

library_benchmark_group!(
    name = scaled_paint,
    benchmarks = [paint_10, paint_50, paint_200]
);

library_benchmark_group!(
    name = scaled_update,
    benchmarks = [update_10, update_50, update_200]
);

main!(library_benchmark_groups = [scaled_pan_zoom, scaled_paint, scaled_update]);
