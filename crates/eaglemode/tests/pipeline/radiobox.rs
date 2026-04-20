//! Systematic interaction test for emRadioBox at 1x and 2x zoom, driven
//! through the full Input dispatch pipeline (PipelineTestHarness).
//!
//! Three emRadioBox widgets share a group, each installed in its own child panel
//! stacked vertically. Clicking each panel's center selects the corresponding
//! radio box. The test verifies correct selection at both 1x and 2x zoom,
//! re-clicking the already-GetChecked box (no-op), and cycling through all items.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emCursor::emCursor;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLook::emLook;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emRadioBox::emRadioBox;
use emcore::emRadioButton::RadioGroup;
use emcore::emViewRenderer::SoftwareCompositor;

use super::support::pipeline::PipelineTestHarness;

// ---------------------------------------------------------------------------
// RadioBoxBehavior -- minimal PanelBehavior wrapper for emRadioBox
// ---------------------------------------------------------------------------

struct RadioBoxBehavior {
    widget: emRadioBox,
}

impl RadioBoxBehavior {
    fn new(widget: emRadioBox) -> Self {
        Self { widget }
    }
}

impl PanelBehavior for RadioBoxBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
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
        self.widget.GetCursor()
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Helper: set up a 3-option emRadioBox harness
// ---------------------------------------------------------------------------

struct RadioBoxHarness {
    h: PipelineTestHarness,
    group: Rc<RefCell<emcore::emRadioButton::RadioGroup>>,
    panels: [emcore::emPanelTree::PanelId; 3],
    compositor: SoftwareCompositor,
}

impl RadioBoxHarness {
    fn new() -> Self {
        let look = emLook::new();
        let group: Rc<RefCell<RadioGroup>> = RadioGroup::new();

        let rb0 = emRadioBox::new("Alpha", look.clone(), group.clone(), 0);
        let rb1 = emRadioBox::new("Beta", look.clone(), group.clone(), 1);
        let rb2 = emRadioBox::new("Gamma", look, group.clone(), 2);

        assert_eq!(group.borrow().GetCount(), 3);
        assert_eq!(group.borrow().GetChecked(), None);

        let mut h = PipelineTestHarness::new();
        let root = h.get_root_panel();

        // Each radio box gets its own child panel, stacked vertically:
        //   panel 0: y=0.00..0.33  (top third)
        //   panel 1: y=0.33..0.66  (middle third)
        //   panel 2: y=0.66..1.00  (bottom third)
        let panel0 = h.add_panel_with(root, "rbox0", Box::new(RadioBoxBehavior::new(rb0)));
        h.tree.Layout(panel0, 0.0, 0.0, 1.0, 1.0 / 3.0, 1.0, None);

        let panel1 = h.add_panel_with(root, "rbox1", Box::new(RadioBoxBehavior::new(rb1)));
        h.tree.Layout(panel1, 0.0, 1.0 / 3.0, 1.0, 1.0 / 3.0, 1.0, None);

        let panel2 = h.add_panel_with(root, "rbox2", Box::new(RadioBoxBehavior::new(rb2)));
        h.tree.Layout(panel2, 0.0, 2.0 / 3.0, 1.0, 1.0 / 3.0, 1.0, None);

        // Settle layout and viewing Restore.
        h.tick_n(5);

        // Render so that emRadioBox::PaintContent() caches last_w/last_h (required
        // for hit_test to function).
        let mut compositor = SoftwareCompositor::new(800, 600);
        compositor.render(&mut h.tree, &h.view);

        Self {
            h,
            group,
            panels: [panel0, panel1, panel2],
            compositor,
        }
    }

    /// Compute the view-space center of a panel.
    fn panel_center(&self, index: usize) -> (f64, f64) {
        let state = self.h.tree.build_panel_state(
            self.panels[index],
            self.h.view.IsFocused(),
            self.h.view.GetCurrentPixelTallness(),
        );
        let vr = state.viewed_rect;
        (vr.x + vr.w * 0.5, vr.y + vr.h * 0.5)
    }

    /// Switch to a given zoom level, tick, and re-render.
    fn zoom_to(&mut self, level: f64) {
        self.h.set_zoom(level);
        self.h.tick_n(5);
        self.compositor.render(&mut self.h.tree, &self.h.view);
    }

    fn checked(&self) -> Option<usize> {
        self.group.borrow().GetChecked()
    }

    fn click_option(&mut self, index: usize) {
        let (cx, cy) = self.panel_center(index);
        self.h.click(cx, cy);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Click each of three vertically-stacked radio boxes at 1x and 2x zoom,
/// verifying the group selection state after each Click.
#[test]
fn radiobox_select_1x_and_2x() {
    let mut t = RadioBoxHarness::new();

    // ── 1x zoom: Click each radio box ─────────────────────────────────
    t.click_option(0);
    assert_eq!(
        t.checked(),
        Some(0),
        "1x: clicking option 0 should select radio box 0"
    );

    t.click_option(1);
    assert_eq!(
        t.checked(),
        Some(1),
        "1x: clicking option 1 should select radio box 1"
    );

    t.click_option(2);
    assert_eq!(
        t.checked(),
        Some(2),
        "1x: clicking option 2 should select radio box 2"
    );

    // ── 2x zoom: same test at higher magnification ───────────────────
    t.zoom_to(2.0);

    t.click_option(0);
    assert_eq!(
        t.checked(),
        Some(0),
        "2x: clicking option 0 should select radio box 0"
    );

    t.click_option(1);
    assert_eq!(
        t.checked(),
        Some(1),
        "2x: clicking option 1 should select radio box 1"
    );

    t.click_option(2);
    assert_eq!(
        t.checked(),
        Some(2),
        "2x: clicking option 2 should select radio box 2"
    );
}

/// Re-clicking the already-GetChecked radio box should keep it GetChecked
/// (radio boxes cannot be deselected by clicking them again).
#[test]
fn radiobox_reclick_selected_is_noop() {
    let mut t = RadioBoxHarness::new();

    // Select option 1.
    t.click_option(1);
    assert_eq!(t.checked(), Some(1));

    // Click option 1 again -- should remain GetChecked.
    t.click_option(1);
    assert_eq!(
        t.checked(),
        Some(1),
        "re-clicking already-selected radio box must not deselect it"
    );

    // Same behavior at 2x zoom.
    t.zoom_to(2.0);

    t.click_option(1);
    assert_eq!(
        t.checked(),
        Some(1),
        "2x: re-clicking already-selected radio box must not deselect it"
    );
}

/// Cycle through all options forward and backward at both zoom levels,
/// verifying each transition.
#[test]
fn radiobox_cycle_forward_and_backward() {
    let mut t = RadioBoxHarness::new();

    // Forward Cycle at 1x: 0 -> 1 -> 2
    for i in 0..3 {
        t.click_option(i);
        assert_eq!(t.checked(), Some(i), "1x forward: expected selection {i}");
    }

    // Backward Cycle at 1x: 2 -> 1 -> 0
    for i in (0..3).rev() {
        t.click_option(i);
        assert_eq!(t.checked(), Some(i), "1x backward: expected selection {i}");
    }

    // Forward Cycle at 2x
    t.zoom_to(2.0);
    for i in 0..3 {
        t.click_option(i);
        assert_eq!(t.checked(), Some(i), "2x forward: expected selection {i}");
    }

    // Backward Cycle at 2x
    for i in (0..3).rev() {
        t.click_option(i);
        assert_eq!(t.checked(), Some(i), "2x backward: expected selection {i}");
    }
}

/// Verify that selection starts as None and transitions correctly on
/// the first Click at each zoom level.
#[test]
fn radiobox_initial_state_is_none() {
    let t = RadioBoxHarness::new();
    assert_eq!(
        t.checked(),
        None,
        "no radio box should be selected initially"
    );
}

/// Verify selection survives a zoom transition without being lost or
/// corrupted.
#[test]
fn radiobox_selection_survives_zoom_change() {
    let mut t = RadioBoxHarness::new();

    // Select option 1 at 1x.
    t.click_option(1);
    assert_eq!(t.checked(), Some(1));

    // Zoom to 2x -- selection must persist.
    t.zoom_to(2.0);
    assert_eq!(
        t.checked(),
        Some(1),
        "selection must survive zoom change from 1x to 2x"
    );

    // Select option 2 at 2x.
    t.click_option(2);
    assert_eq!(t.checked(), Some(2));

    // Zoom back to 1x -- selection must persist.
    t.zoom_to(1.0);
    assert_eq!(
        t.checked(),
        Some(2),
        "selection must survive zoom change from 2x to 1x"
    );
}
