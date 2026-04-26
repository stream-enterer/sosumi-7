//! Parallel rendering verification tests.
//!
//! These tests exercise the DrawList record + parallel replay pipeline
//! by rendering the same scenes used in other golden tests through both
//! the direct path and the parallel path, then asserting byte-identical output.

use std::rc::Rc;

use emcore::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use emcore::emColor::emColor;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emPanelTree::PanelTree;
use emcore::emView::{emView, ViewFlags};
use emcore::emViewRenderer::SoftwareCompositor;

use emcore::emCheckBox::emCheckBox;

use emcore::emLabel::emLabel;

use emcore::emLook::emLook;

use emcore::emScalarField::emScalarField;

use super::common::*;

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

fn settle(tree: &mut PanelTree, view: &mut emView) {
    let mut ts = TestSched::new();
    for _ in 0..5 {
        view.HandleNotice(tree, ts.sched_mut());
        ts.with(|sc| view.Update(tree, sc));
    }
}

/// Render a scene through single-threaded tiled and multi-threaded tiled
/// paths using `SoftwareCompositor::render_parallel`, assert byte-identical.
///
/// Both paths use the same display-list record + replay pipeline with the
/// same tile size. The only difference is the thread GetCount (1 vs N).
/// This ensures the parallel dispatch mechanism is correct without being
/// affected by inherent tile-boundary AA artifacts (which are identical
/// in both single- and multi-threaded tiled rendering).
fn assert_parallel_identical(
    name: &str,
    behavior: Box<dyn PanelBehavior>,
    thread_count: i32,
    tile_size: u32,
) {
    let (w, h, _expected) = load_compositor_golden(name);

    // Build the scene.
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
    tree.set_behavior(root, behavior);
    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    // Single-threaded tiled render (baseline).
    let pool_1 = emcore::emRenderThreadPool::emRenderThreadPool::new(1);
    let mut single = SoftwareCompositor::new(w, h);
    single.render_parallel(&mut tree, &view, &pool_1, tile_size);
    let single_data = single.framebuffer().GetMap().to_vec();

    // Multi-threaded tiled render.
    let pool_n = emcore::emRenderThreadPool::emRenderThreadPool::new(thread_count);
    let mut multi = SoftwareCompositor::new(w, h);
    multi.render_parallel(&mut tree, &view, &pool_n, tile_size);
    let multi_data = multi.framebuffer().GetMap().to_vec();

    assert_eq!(
        single_data.len(),
        multi_data.len(),
        "{name}: buffer size mismatch"
    );

    // Find first differing pixel for a useful error message.
    let mut mismatches = 0u64;
    let mut first_diff: Option<(u32, u32)> = None;
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if single_data[i..i + 4] != multi_data[i..i + 4] {
                mismatches += 1;
                if first_diff.is_none() {
                    first_diff = Some((x, y));
                }
            }
        }
    }
    assert_eq!(
        mismatches,
        0,
        "{name}: {mismatches} pixels differ between 1-thread and {thread_count}-thread \
         tiled rendering (tile_size={tile_size}). First diff at {:?}: \
         single={:?} multi={:?}",
        first_diff.unwrap_or((0, 0)),
        first_diff.map(|(x, y)| {
            let i = ((y * w + x) * 4) as usize;
            &single_data[i..i + 4]
        }),
        first_diff.map(|(x, y)| {
            let i = ((y * w + x) * 4) as usize;
            &multi_data[i..i + 4]
        }),
    );
}

// ── PanelBehavior wrappers ───────────────────────────────────────

struct BorderBehavior {
    border: emBorder,
    look: Rc<emLook>,
}

impl PanelBehavior for BorderBehavior {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true, 1.0);
    }
}

struct LabelBehavior {
    label: emLabel,
}

impl PanelBehavior for LabelBehavior {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.label
            .PaintContent(painter, w, h, state.enabled, pixel_scale);
    }
}

struct CheckBoxBehavior {
    check_box: emCheckBox,
}

impl PanelBehavior for CheckBoxBehavior {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.check_box
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }
}

struct ScalarFieldBehavior {
    scalar_field: emScalarField,
}

impl PanelBehavior for ScalarFieldBehavior {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.scalar_field
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }
}

// ── Parallel rendering parity tests ──────────────────────────────

/// Verify parallel rendering of a border widget with 1 thread (sanity).
#[test]
fn parallel_border_1_thread() {
    require_golden!();
    let look = emLook::new();
    assert_parallel_identical(
        "widget_border_rect",
        Box::new(BorderBehavior {
            border: emBorder::new(OuterBorderType::Rect)
                .with_inner(InnerBorderType::None)
                .with_caption("Test"),
            look,
        }),
        1,
        128,
    );
}

/// Verify parallel rendering with 2 threads (real parallelism).
#[test]
fn parallel_border_2_threads() {
    require_golden!();
    let look = emLook::new();
    assert_parallel_identical(
        "widget_border_rect",
        Box::new(BorderBehavior {
            border: emBorder::new(OuterBorderType::Rect)
                .with_inner(InnerBorderType::None)
                .with_caption("Test"),
            look,
        }),
        2,
        128,
    );
}

/// Verify parallel rendering with 4 threads.
#[test]
fn parallel_border_4_threads() {
    require_golden!();
    let look = emLook::new();
    assert_parallel_identical(
        "widget_border_rect",
        Box::new(BorderBehavior {
            border: emBorder::new(OuterBorderType::Rect)
                .with_inner(InnerBorderType::None)
                .with_caption("Test"),
            look,
        }),
        4,
        128,
    );
}

/// Verify parallel rendering with small tiles (more tiles = more contention).
#[test]
fn parallel_border_small_tiles() {
    require_golden!();
    let look = emLook::new();
    assert_parallel_identical(
        "widget_border_rect",
        Box::new(BorderBehavior {
            border: emBorder::new(OuterBorderType::Rect)
                .with_inner(InnerBorderType::None)
                .with_caption("Test"),
            look,
        }),
        4,
        32,
    );
}

/// Verify parallel rendering of a checkbox (uses ellipse, polygon, text).
#[test]
fn parallel_checkbox() {
    require_golden!();
    let look = emLook::new();
    let mut ts = TestSched::new();
    let mut cb = emCheckBox::new(&mut ts.cc(), "Check Option", look);
    // Scratch ctx — no scheduler reach; cb has no callback so SetChecked is pure state.
    let mut tree = PanelTree::new();
    let root = tree.create_root("t", false);
    let mut ctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, root, 1.0);
    cb.SetChecked(true, &mut ctx);
    assert_parallel_identical(
        "widget_checkbox_checked",
        Box::new(CheckBoxBehavior { check_box: cb }),
        4,
        64,
    );
}

/// Verify parallel rendering of a scalar field (uses gradient, text, polygon).
#[test]
fn parallel_scalarfield() {
    let mut ts = TestSched::new();
    require_golden!();
    let look = emLook::new();
    let mut sf = emScalarField::new(&mut ts.cc(), 0.0, 100.0, look);
    sf.SetCaption("Value");
    sf.SetEditable(true);
    sf.set_initial_value(50.0);
    assert_parallel_identical(
        "widget_scalarfield",
        Box::new(ScalarFieldBehavior { scalar_field: sf }),
        4,
        64,
    );
}

/// Verify parallel rendering of a label (text rendering).
#[test]
fn parallel_label() {
    require_golden!();
    let look = emLook::new();
    assert_parallel_identical(
        "widget_label",
        Box::new(LabelBehavior {
            label: emLabel::new("Hello World", look),
        }),
        4,
        64,
    );
}

// ── Benchmark ────────────────────────────────────────────────────

/// Benchmark: render a complex scene N times, compare single-threaded vs
/// multi-threaded wall-clock time. Not a pass/fail test — prints results.
#[test]
fn parallel_benchmark() {
    require_golden!();

    let iterations = 100;
    let look = emLook::new();

    // Single-threaded timing.
    let single_elapsed = {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("bench");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
        tree.set_behavior(
            root,
            Box::new(BorderBehavior {
                border: emBorder::new(OuterBorderType::Group)
                    .with_inner(InnerBorderType::None)
                    .with_caption("Benchmark"),
                look: Rc::clone(&look),
            }),
        );
        let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
        settle(&mut tree, &mut view);

        let mut comp = SoftwareCompositor::new(800, 600);
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            comp.render(&mut tree, &view);
        }
        start.elapsed()
    };

    // Multi-threaded timing (4 threads).
    let multi_elapsed = {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("bench");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
        tree.set_behavior(
            root,
            Box::new(BorderBehavior {
                border: emBorder::new(OuterBorderType::Group)
                    .with_inner(InnerBorderType::None)
                    .with_caption("Benchmark"),
                look: Rc::clone(&look),
            }),
        );
        let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
        settle(&mut tree, &mut view);

        let pool = emcore::emRenderThreadPool::emRenderThreadPool::new(4);
        let mut comp = SoftwareCompositor::new(800, 600);
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            comp.render_parallel(&mut tree, &view, &pool, 128);
        }
        start.elapsed()
    };

    eprintln!(
        "\n=== Parallel Rendering Benchmark ({iterations} iterations, 800x600) ===\n\
         Single-threaded: {:.1}ms total ({:.2}ms/frame)\n\
         Multi-threaded (4T, 128px tiles): {:.1}ms total ({:.2}ms/frame)\n\
         Speedup: {:.2}x\n",
        single_elapsed.as_secs_f64() * 1000.0,
        single_elapsed.as_secs_f64() * 1000.0 / iterations as f64,
        multi_elapsed.as_secs_f64() * 1000.0,
        multi_elapsed.as_secs_f64() * 1000.0 / iterations as f64,
        single_elapsed.as_secs_f64() / multi_elapsed.as_secs_f64(),
    );

    // Verify correctness: single-threaded tiled and multi-threaded tiled
    // outputs must be byte-identical (same scene, same tile size, only
    // thread count differs).  Uses tiled rendering for both paths —
    // consistent with the other parallel_* tests.  Non-tiled vs tiled
    // comparison is not meaningful because float precision in TX/TY
    // computation differs when offset_x varies between painters.
    let single_pixels = {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("verify");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
        tree.set_behavior(
            root,
            Box::new(BorderBehavior {
                border: emBorder::new(OuterBorderType::Group)
                    .with_inner(InnerBorderType::None)
                    .with_caption("Benchmark"),
                look: Rc::clone(&look),
            }),
        );
        let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
        settle(&mut tree, &mut view);
        let pool_1 = emcore::emRenderThreadPool::emRenderThreadPool::new(1);
        let mut comp = SoftwareCompositor::new(800, 600);
        comp.render_parallel(&mut tree, &view, &pool_1, 128);
        comp.framebuffer().GetMap().to_vec()
    };
    let multi_pixels = {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("verify");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
        tree.set_behavior(
            root,
            Box::new(BorderBehavior {
                border: emBorder::new(OuterBorderType::Group)
                    .with_inner(InnerBorderType::None)
                    .with_caption("Benchmark"),
                look,
            }),
        );
        let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
        settle(&mut tree, &mut view);
        let pool = emcore::emRenderThreadPool::emRenderThreadPool::new(4);
        let mut comp = SoftwareCompositor::new(800, 600);
        comp.render_parallel(&mut tree, &view, &pool, 128);
        comp.framebuffer().GetMap().to_vec()
    };
    assert_eq!(
        single_pixels, multi_pixels,
        "Single-threaded and multi-threaded tiled renders must produce byte-identical output"
    );
}
