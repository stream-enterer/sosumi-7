//! Behavioral interaction tests for widgets driven through the full Input
//! pipeline (PipelineTestHarness). These tests verify that mouse drag and
//! Click interactions produce expected state changes when dispatched through
//! the coordinate-transform pipeline.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emButton::emButton;
use emcore::emColorField::emColorField;
use emcore::emCursor::emCursor;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emListBox::{emListBox, SelectionMode};
use emcore::emLook::emLook;
use emcore::emPainter::emPainter;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelCtx::PanelCtx;
use emcore::emScalarField::emScalarField;
use emcore::emViewRenderer::SoftwareCompositor;

use super::support::pipeline::PipelineTestHarness;

/// PanelBehavior wrapper for emScalarField so it can be installed into the
/// panel tree. Delegates PaintContent/Input to the underlying widget.
struct ScalarFieldBehavior {
    sf: emScalarField,
    /// Shared handle so the test can read the value after interaction.
    value: Rc<RefCell<f64>>,
}

impl ScalarFieldBehavior {
    fn new(sf: emScalarField, value: Rc<RefCell<f64>>) -> Self {
        Self { sf, value }
    }
}

impl PanelBehavior for ScalarFieldBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.sf.Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        let consumed = self.sf.Input(event, state, input_state);
        // Sync the shared value so the test can observe it.
        *self.value.borrow_mut() = self.sf.GetValue();
        consumed
    }

    fn GetCursor(&self) -> emCursor {
        self.sf.GetCursor()
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

/// Dragging on a emScalarField should change its value. This test fails because
/// of a known bug where `CheckMouse` passes the wrong height to
/// `GetContentRoundRect`, causing mouse hit-testing to reject all drag
/// positions and leaving the value unchanged.
#[test]
fn scalarfield_drag_changes_value() {
    let look = emLook::new();
    let mut sf = emScalarField::new(0.0, 100.0, look);
    sf.SetValue(50.0);
    sf.SetEditable(true);

    let value = Rc::new(RefCell::new(50.0));
    let value_read = value.clone();

    let behavior = ScalarFieldBehavior::new(sf, value);

    // Set up the pipeline harness (800x600 viewport).
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // Add the emScalarField as a child panel filling the entire root.
    let _panel_id = h.add_panel_with(root, "scalar_field", Box::new(behavior));
    // Default layout from add_panel_with is (0,0,1,1) which fills root.

    // Tick to settle layout and viewing Restore.
    h.tick_n(5);

    // Render once via SoftwareCompositor so that PaintContent() is called on the
    // emScalarField, populating its cached last_w / last_h dimensions which
    // are required for mouse hit-testing in CheckMouse().
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    // At 1x zoom, the panel fills the 800x600 viewport. The emScalarField's
    // scale area is within the content rect (after border insets). We drag
    // from the center (50% value) to a point 80% across horizontally.
    //
    // emView-space coordinates: the panel maps to the full viewport.
    let center_x = 400.0; // 50% of 800
    let center_y = 300.0; // 50% of 600
    let target_x = 640.0; // 80% of 800
    let target_y = 300.0; // same vertical GetPos

    // Perform the drag through the full pipeline.
    h.drag(center_x, center_y, target_x, target_y);

    let final_value = *value_read.borrow();
    assert!(
        (final_value - 50.0).abs() > 1.0,
        "emScalarField value should have changed from 50.0 after drag, but it is still {final_value:.1}. \
         This is a known bug: CheckMouse passes height=0.0 to GetContentRoundRect, \
         causing all mouse positions to fall outside the scale area."
    );
}

// ---------------------------------------------------------------------------
// ColorFieldBehavior -- minimal PanelBehavior wrapper for emColorField
//
// In the production code there is NO PanelBehavior impl wrapping emColorField.
// This test-only wrapper reproduces the pattern used by every other widget
// panel (ScalarFieldPanel, TextFieldPanel, etc.) so that we can exercise
// the auto-expansion -> LayoutChildren path through the real panel tree.
//
// The wrapper delegates LayoutChildren to emColorField::LayoutChildren,
// which only POSITIONS existing children. The bug: create_expansion_children
// is never called during auto-expansion, so no child panels are created.
// ---------------------------------------------------------------------------
struct ColorFieldBehavior {
    color_field: emColorField,
}

impl ColorFieldBehavior {
    fn new(look: Rc<emLook>) -> Self {
        let mut cf = emColorField::new(look);
        cf.SetEditable(true);
        cf.SetAlphaEnabled(true);
        Self { color_field: cf }
    }
}

impl PanelBehavior for ColorFieldBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.color_field.Paint(painter, w, h, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        self.color_field.Input(event, state, input_state)
    }

    fn GetCursor(&self) -> emCursor {
        emCursor::Normal
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // This is what the production code SHOULD do but doesn't, because
        // no ColorFieldPanel exists. The LayoutChildren method only positions
        // children -- it does NOT create them.
        let rect = ctx.layout_rect();
        self.color_field.LayoutChildren(ctx, rect.w, rect.h);
    }
}

/// **Calibration test for known bug:**
/// emColorField expanded state is missing RGB sliders -- auto-expansion doesn't
/// create the expected child emScalarField panels.
///
/// When the panel tree's auto-expansion mechanism fires for a emColorField
/// panel, it calls `behavior.LayoutChildren()`. The emColorField's
/// `LayoutChildren` method positions existing children but never calls
/// `create_expansion_children` to actually CREATE the emScalarField/emTextField
/// child panels. As a GetResult, the expanded emColorField has zero children
/// instead of the expected 8 (R, G, B, A, H, S, V, Name -- inside a
/// emRasterLayout container).
///
/// Expected: after expansion, the emColorField panel should have at least 1
/// child (the emRasterLayout container "emColorField::InnerStuff"), which
/// itself should contain >= 3 children (at minimum R, G, B sliders).
///
/// Actual: the panel has 0 children because create_expansion_children is
/// never called.
#[test]
fn colorfield_expansion_creates_child_sliders() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // Create a emColorField panel with behavior.
    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    // Tick for initial layout.
    h.tick();

    // Trigger auto-expansion. expand_to sets the zoom and runs 10 ticks,
    // which triggers update_auto_expansion -> behavior.LayoutChildren().
    // At zoom 16x, the panel's viewed area is enormous (>> 150 threshold).
    h.expand_to(16.0);

    // The panel should be auto-expanded at this zoom level.
    assert!(
        h.is_expanded(panel_id),
        "ColorField panel should be auto-expanded at 16x zoom"
    );

    // BUG: after auto-expansion, LayoutChildren was called but
    // create_expansion_children was NOT called, so no child panels exist.
    //
    // Count children: the expanded emColorField should have at least 1 child
    // (the emRasterLayout "emColorField::InnerStuff" container), which itself
    // should contain the R/G/B/A/H/S/V emScalarField children + Name emTextField.
    let child_count = h.tree.child_count(panel_id);

    assert!(
        child_count >= 1,
        "Expanded emColorField should have child panels (emRasterLayout container \
         with emScalarField sliders), but found {child_count} children. \
         Bug: create_expansion_children() is never called during auto-expansion."
    );
}

/// **Calibration test for known bug:**
/// emButton Click has no effect after zoom -- `CheckMouse` coordinate space
/// divergence from C++.
///
/// ## Root cause
///
/// C++ `emButton::CheckMouse` receives panel-local coordinates (0..1 for X,
/// 0..tallness for Y) and calls `GetContentRoundRect` which also returns
/// panel-local Restore. Both sides are in the same normalized space, so the
/// hit test works at every zoom level.
///
/// Rust `emButton::CheckMouse` calls `GetContentRoundRect(self.last_w,
/// self.last_h)` which returns PIXEL-space Restore. It then tests the
/// caller's coordinates against pixel-space face Restore. This means
/// `CheckMouse` only works when passed pixel coordinates that match the
/// current `last_w`/`last_h` from the most recent PaintContent.
///
/// The internal `hit_test` method (used by `emButton::Input()`) correctly
/// normalizes to (1.0, tallness) space and is NOT affected. Through the
/// Input pipeline, button clicks work at all zoom levels.
///
/// The bug manifests when any code (including future port work) calls the
/// public `CheckMouse` API with panel-local coordinates, as C++ callers do.
/// At 1x zoom the pixel-space face is small and the point lands inside.
/// At 2x zoom the face Restore doubles, moving the boundary past the same
/// point.
///
/// ## What this test verifies
///
/// 1. Paint the button at 1x (last_w=600). A panel-local point (0.15, 0.15)
///    converted to pixel coords (90, 90) passes `CheckMouse`.
/// 2. Paint at 2x (last_w=1200). The same panel-local point (0.15, 0.15)
///    converted to 1x pixel coords (90, 90) is REJECTED by `CheckMouse`
///    because the face inset doubled.
/// 3. Show that the same panel-local point WOULD pass at 2x if
///    `CheckMouse` normalized like `hit_test` does.
///
/// Expected: CheckMouse(90, 90) passes at both 1x and 2x PaintContent sizes
///           (same panel-local GetPos).
/// Actual:   passes at 1x (600x600), fails at 2x (1200x1200).
#[test]
fn button_click_works_after_zoom() {
    use emcore::emImage::emImage;
    use std::cell::Cell;

    let look = emLook::new();
    let mut btn = emButton::new("Zoom Test", look.clone());

    // ── Step 1: Paint at 1x dimensions (last_w = last_h = 600) ──────
    {
        let mut img = emImage::new(600, 600, 4);
        let mut p = emPainter::new(&mut img);
        btn.Paint(&mut p, 600.0, 600.0, true, 1.0);
    }

    // Calibration: pixel center (300, 300) passes at 1x.
    assert!(
        btn.CheckMouse(300.0, 300.0),
        "Calibration: pixel center should hit at 1x"
    );

    // Find the face boundary at 1x: scan from the origin to find the first
    // pixel coordinate where CheckMouse returns true.
    let mut boundary_1x = 0.0_f64;
    for i in 0..300 {
        let v = i as f64;
        if btn.CheckMouse(v, v) {
            boundary_1x = v;
            break;
        }
    }
    assert!(
        boundary_1x > 0.0,
        "Should find a face boundary at 1x (got {boundary_1x})"
    );

    // Pick a test point just inside the face boundary at 1x.
    let test_px = boundary_1x + 6.0;
    assert!(
        btn.CheckMouse(test_px, test_px),
        "Calibration: ({test_px},{test_px}) should be inside face at 1x \
         (boundary ~{boundary_1x} px)"
    );

    // Convert to panel-local (normalized) coordinates for reference.
    let panel_local_coord = test_px / 600.0;

    // ── Step 2: Paint at 2x dimensions (last_w = last_h = 1200) ─────
    {
        let mut img = emImage::new(1200, 1200, 4);
        let mut p = emPainter::new(&mut img);
        btn.Paint(&mut p, 1200.0, 1200.0, true, 1.0);
    }

    // Calibration: pixel center at 2x (600, 600) passes.
    assert!(
        btn.CheckMouse(600.0, 600.0),
        "Calibration: pixel center should hit at 2x"
    );

    // After the fix, CheckMouse normalizes to (1.0, tallness) space.
    // The same PANEL-LOCAL coordinate should work at any PaintContent size.
    // At 2x, the pixel equivalent of panel_local_coord is:
    let test_px_2x = panel_local_coord * 1200.0;
    assert!(
        btn.CheckMouse(test_px_2x, test_px_2x),
        "CheckMouse should accept the same panel-local coordinate ({panel_local_coord:.4}) \
         at 2x PaintContent size. Pixel equivalent at 2x: ({test_px_2x},{test_px_2x}). \
         If this fails, CheckMouse is still operating in raw pixel space."
    );

    // ── Step 3: Verify the pipeline is NOT affected ──────────────────
    // button.Input() uses hit_test() which normalizes correctly.
    // Build a full pipeline and confirm a Click works at 2x zoom.
    let clicked = Rc::new(Cell::new(false));
    let clicked_clone = clicked.clone();

    let mut btn2 = emButton::new("Pipeline Test", look);
    btn2.on_click = Some(Box::new(move || {
        clicked_clone.set(true);
    }));

    struct BtnPanel {
        widget: emButton,
    }
    impl PanelBehavior for BtnPanel {
        fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
            let pixel_scale =
                state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
            self.widget.Paint(painter, w, h, state.enabled, pixel_scale);
        }
        fn Input(
            &mut self,
            event: &emInputEvent,
            state: &PanelState,
            input_state: &emInputState,
        ) -> bool {
            self.widget.Input(event, state, input_state)
        }
        fn GetCursor(&self) -> emCursor {
            emCursor::Normal
        }
        fn IsOpaque(&self) -> bool {
            true
        }
    }

    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let _panel_id = h.add_panel_with(root, "button", Box::new(BtnPanel { widget: btn2 }));
    h.tick_n(5);

    // Render at 1x so emButton::PaintContent() caches last_w/last_h.
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    // Click at viewport center at 1x -- should work (calibration).
    h.click(400.0, 300.0);
    assert!(
        clicked.get(),
        "Pipeline calibration: button should fire at 1x zoom"
    );
    clicked.set(false);

    // Zoom to 2x and re-render so last_w/last_h update.
    h.set_zoom(2.0);
    h.tick_n(5);
    compositor.render(&mut h.tree, &h.view);

    // Click at viewport center at 2x -- this WORKS because Input()
    // uses hit_test() which normalizes. The CheckMouse bug does not
    // affect the pipeline. If this assertion starts failing, the bug
    // has spread beyond CheckMouse into the pipeline.
    h.click(400.0, 300.0);
    assert!(
        clicked.get(),
        "Pipeline: button should fire at 2x zoom (hit_test normalizes correctly). \
         If this fails, the bug has spread beyond CheckMouse."
    );
}

// ---------------------------------------------------------------------------
// SharedListBoxPanel -- minimal PanelBehavior wrapper for emListBox
// ---------------------------------------------------------------------------

/// PanelBehavior wrapper for emListBox, allowing shared access via Rc<RefCell>.
///
/// The emListBox is stored behind Rc<RefCell> so the test can inspect widget
/// state (GetSelectedIndex, etc.) after Input dispatch.
struct SharedListBoxPanel {
    inner: Rc<RefCell<emListBox>>,
}

impl PanelBehavior for SharedListBoxPanel {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.inner.borrow_mut().Paint(painter, w, h, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        self.inner.borrow_mut().Input(event, state, input_state)
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.inner
                .borrow_mut()
                .on_focus_changed(state.in_active_path);
        }
        if flags.intersects(NoticeFlags::ENABLE_CHANGED) {
            self.inner.borrow_mut().on_enable_changed(state.enabled);
        }
    }

    fn IsOpaque(&self) -> bool {
        true
    }

    fn GetCursor(&self) -> emCursor {
        emCursor::Normal
    }
}

/// Calibration test for known bug: emListBox mouse Click always selects item 0
/// regardless of where the user clicks.
///
/// Root cause: `emListBox::Input()` computes GetContentRect from the pixel-space
/// dimensions (last_w, last_h set during PaintContent), but receives mouse
/// coordinates in normalized panel-local space (0..1 x 0..tallness) from the
/// view-to-panel transform. The GetContentRect's `cy` (in pixels, e.g. ~80) is
/// always much larger than mouse_y (normalized, ~0..1), making `rel_y`
/// negative and `(rel_y / row_height) as usize` saturate to 0.
#[test]
fn listbox_click_selects_correct_item() {
    // ── 1. Build pipeline harness (800x600 viewport) ─────────────
    let mut harness = super::support::pipeline::PipelineTestHarness::new();

    // ── 2. Create a emListBox with 5 items ─────────────────────────
    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetSelectionType(SelectionMode::Single);
    lb.AddItem("item0".to_string(), "Alpha".to_string());
    lb.AddItem("item1".to_string(), "Beta".to_string());
    lb.AddItem("item2".to_string(), "Gamma".to_string());
    lb.AddItem("item3".to_string(), "Delta".to_string());
    lb.AddItem("item4".to_string(), "Epsilon".to_string());

    let lb_ref = Rc::new(RefCell::new(lb));

    // ── 3. Add it as a panel with behavior ───────────────────────
    let root = harness.get_root_panel();
    let panel_id = harness.add_panel_with(
        root,
        "listbox",
        Box::new(SharedListBoxPanel {
            inner: lb_ref.clone(),
        }),
    );

    // ── 4. Settle layout ─────────────────────────────────────────
    harness.tick_n(5);

    // ── 5. Set zoom to 1x (panel fills viewport) ─────────────────
    harness.set_zoom(1.0);
    harness.tick_n(5);

    // ── 6. Render to trigger PaintContent() on the emListBox ──────────────
    // The emListBox needs PaintContent() to have been called so that last_w,
    // last_h, and visible_height are set (hit_test requires last_w > 0).
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut harness.tree, &harness.view);

    // ── 7. Compute view-space coordinates for item 2 ─────────────
    // Use build_panel_state to GetRec the viewed rect in viewport pixels.
    let state = harness.tree.build_panel_state(
        panel_id,
        harness.view.IsFocused(),
        harness.view.GetCurrentPixelTallness(),
    );
    let vr = state.viewed_rect;

    // Item 2 (0-indexed) is in the middle of the 5-item list.
    // Each item occupies 1/5 of the panel's vertical extent.
    // Item 2's vertical center in view space:
    //   vr.y + (2.5 / 5.0) * vr.h  (vertical center of item 2)
    // Horizontal center of the panel:
    //   vr.x + 0.5 * vr.w
    let click_x = vr.x + vr.w * 0.5;
    let click_y = vr.y + vr.h * (2.5 / 5.0);

    // ── 8. Click at the computed coordinates ─────────────────────
    harness.click(click_x, click_y);

    // ── 9. Assert that item 2 was GetChecked ───────────────────────
    // BUG: The Click always selects item 0 due to coordinate space
    // mismatch between normalized panel-local coords and pixel-space
    // GetContentRect computation.
    let checked = lb_ref.borrow().GetSelectedIndex();
    assert_eq!(
        checked,
        Some(2),
        "Expected clicking on item 2 to select it, but got {:?}. \
         This is the known bug: emListBox Click always selects the first \
         item because GetContentRect is computed in pixel space while mouse \
         coordinates arrive in normalized panel-local space.",
        checked
    );
}
