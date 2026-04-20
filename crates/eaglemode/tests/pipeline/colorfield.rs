//! Systematic interaction test for emColorField in expanded state at 1x zoom,
//! driven through the full Input dispatch pipeline (PipelineTestHarness).
//!
//! Verifies that auto-expansion creates the expected child panel structure
//! (emRasterLayout container with emScalarField sliders for R, G, B, A, H, S, V
//! and a emTextField for color name/hex), and that the expansion data is
//! correctly initialized from the widget's color.

use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emColorField::emColorField;
use emcore::emCursor::emCursor;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLook::emLook;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emPanelCtx::PanelCtx;

use super::support::pipeline::PipelineTestHarness;

/// PanelBehavior wrapper for emColorField so it can be installed into the
/// panel tree. Delegates PaintContent/Input/LayoutChildren to the underlying widget.
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

    fn with_color(mut self, color: emColor) -> Self {
        self.color_field.SetColor(color);
        self
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

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        self.color_field.create_expansion_children(ctx);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.color_field.LayoutChildren(ctx, rect.w, rect.h);
    }

    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.color_field.sync_from_children(ctx);
        self.color_field.Cycle()
    }
}

// ---------------------------------------------------------------------------
// Helper: collect child panel names under a given parent_context
// ---------------------------------------------------------------------------
fn child_names(
    h: &PipelineTestHarness,
    parent_context: emcore::emPanelTree::PanelId,
) -> Vec<String> {
    h.tree
        .children(parent_context)
        .filter_map(|id| h.tree.name(id).map(|n| n.to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// Test: expansion structure at 1x zoom
// ---------------------------------------------------------------------------

/// Verify that expanding a emColorField at 16x zoom creates the expected child
/// panel hierarchy:
///
/// ```text
/// color_field
///   emColorField::InnerStuff  (emRasterLayout container)
///     r   (emScalarField - Red)
///     g   (emScalarField - Green)
///     b   (emScalarField - Blue)
///     a   (emScalarField - Alpha)
///     h   (emScalarField - Hue)
///     s   (emScalarField - Saturation)
///     v   (emScalarField - Value/brightness)
///     n   (emTextField   - Name/hex)
/// ```
#[test]
fn colorfield_expanded_has_correct_child_structure() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    // Initial tick for layout.
    h.tick();

    // Trigger auto-expansion at 16x zoom (well above the expansion threshold).
    h.expand_to(16.0);

    // The panel should be auto-expanded.
    assert!(
        h.is_expanded(panel_id),
        "ColorField panel should be auto-expanded at 16x zoom"
    );

    // The emColorField should have exactly 1 direct child: the emRasterLayout container.
    let direct_children = child_names(&h, panel_id);
    assert_eq!(
        direct_children.len(),
        1,
        "Expanded emColorField should have 1 direct child (emRasterLayout container), \
         but found {}: {:?}",
        direct_children.len(),
        direct_children
    );
    assert_eq!(
        direct_children[0], "emColorField::InnerStuff",
        "Direct child should be the RasterLayout container 'emColorField::InnerStuff'"
    );

    // Find the emRasterLayout container and verify its children.
    let layout_id = h
        .tree
        .children(panel_id)
        .next()
        .expect("should have a child");

    let slider_names = child_names(&h, layout_id);
    assert_eq!(
        slider_names,
        vec!["r", "g", "b", "a", "h", "s", "v", "n"],
        "emRasterLayout container should have 8 children: \
         r, g, b, a (RGBA), h, s, v (HSV), n (Name). Got: {:?}",
        slider_names
    );
}

// ---------------------------------------------------------------------------
// Test: expansion data Match the initial color
// ---------------------------------------------------------------------------

/// Verify that the expansion data (RGBA/HSV values) is correctly initialized
/// from the widget's color when auto-expansion creates the child panels.
#[test]
fn colorfield_expanded_data_matches_initial_color() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let color = emColor::rgba(100, 150, 200, 180);
    let behavior = ColorFieldBehavior::new(look).with_color(color);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    assert!(h.is_expanded(panel_id));

    // Take the behavior to inspect the expansion data.
    let behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any()
        .downcast_ref::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    let exp = cfb
        .color_field
        .expansion()
        .expect("expansion data should exist after auto-expand");

    // Verify RGBA channels match the initial color.
    // C++ formula: (channel * 10000 + 127) / 255
    let expected_r = (100i64 * 10000 + 127) / 255;
    let expected_g = (150i64 * 10000 + 127) / 255;
    let expected_b = (200i64 * 10000 + 127) / 255;
    let expected_a = (180i64 * 10000 + 127) / 255;

    assert_eq!(exp.sf_red, expected_r, "Red channel mismatch");
    assert_eq!(exp.sf_green, expected_g, "Green channel mismatch");
    assert_eq!(exp.sf_blue, expected_b, "Blue channel mismatch");
    assert_eq!(exp.sf_alpha, expected_a, "Alpha channel mismatch");

    // Verify HSV values are reasonable for rgb(100, 150, 200).
    // Hue should be in the blue range (~210 degrees = 21000 in C++ units).
    assert!(
        exp.sf_hue > 18000 && exp.sf_hue < 24000,
        "Hue for rgb(100,150,200) should be ~210 degrees (21000), got {}",
        exp.sf_hue
    );
    // Saturation: S = delta/max = 100/200 = 50% → ~5000 in C++ units.
    assert!(
        exp.sf_sat > 4000 && exp.sf_sat < 6000,
        "Saturation for rgb(100,150,200) should be ~5000 (50%), got {}",
        exp.sf_sat
    );
    // Value: V = max/255 = 200/255 → ~7843 in C++ units.
    assert!(
        exp.sf_val > 7000 && exp.sf_val < 8500,
        "Value for rgb(100,150,200) should be ~7843 (200/255), got {}",
        exp.sf_val
    );

    // Put the behavior back.
    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// Test: expansion creates children for different initial colors
// ---------------------------------------------------------------------------

/// Verify that expansion works correctly with different initial colors
/// (black, white, pure red, transparent).
#[test]
fn colorfield_expanded_various_colors() {
    type RgbaAssertion = Box<dyn Fn(i64, i64, i64, i64)>;
    let test_cases: Vec<(&str, emColor, RgbaAssertion)> = vec![
        (
            "black",
            emColor::BLACK,
            Box::new(|r, g, b, _a| {
                assert_eq!(r, 0, "Black: red should be 0");
                assert_eq!(g, 0, "Black: green should be 0");
                assert_eq!(b, 0, "Black: blue should be 0");
            }),
        ),
        (
            "white",
            emColor::WHITE,
            Box::new(|r, g, b, _a| {
                assert_eq!(r, 10000, "White: red should be 10000");
                assert_eq!(g, 10000, "White: green should be 10000");
                assert_eq!(b, 10000, "White: blue should be 10000");
            }),
        ),
        (
            "pure_red",
            emColor::RED,
            Box::new(|r, g, b, _a| {
                assert_eq!(r, 10000, "Red: red should be 10000");
                assert_eq!(g, 0, "Red: green should be 0");
                assert_eq!(b, 0, "Red: blue should be 0");
            }),
        ),
        (
            "transparent",
            emColor::TRANSPARENT,
            Box::new(|_r, _g, _b, a| {
                assert_eq!(a, 0, "Transparent: alpha should be 0");
            }),
        ),
    ];

    for (label, color, check) in &test_cases {
        let mut h = PipelineTestHarness::new();
        let root = h.get_root_panel();

        let look = emLook::new();
        let behavior = ColorFieldBehavior::new(look).with_color(*color);
        let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

        h.tick();
        h.expand_to(16.0);

        assert!(
            h.is_expanded(panel_id),
            "{label}: panel should be auto-expanded"
        );

        // Verify child structure exists.
        let child_count = h.tree.child_count(panel_id);
        assert_eq!(
            child_count, 1,
            "{label}: should have 1 direct child (RasterLayout)"
        );

        let layout_id = h.tree.children(panel_id).next().unwrap();
        let slider_count = h.tree.child_count(layout_id);
        assert_eq!(
            slider_count, 8,
            "{label}: RasterLayout should have 8 children"
        );

        // Inspect expansion data.
        let behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
        let cfb = behavior
            .as_any()
            .downcast_ref::<ColorFieldBehavior>()
            .expect("should be ColorFieldBehavior");

        let exp = cfb
            .color_field
            .expansion()
            .expect("expansion data should exist");

        check(exp.sf_red, exp.sf_green, exp.sf_blue, exp.sf_alpha);

        h.tree.put_behavior(panel_id, behavior);
    }
}

// ---------------------------------------------------------------------------
// Test: child GetCount before vs after expansion
// ---------------------------------------------------------------------------

/// Verify that the emColorField has no children when below the expansion
/// threshold, and gains children once expanded.
///
/// The default auto-expansion threshold is 150 (area). At 1x zoom the panel
/// fills the 800x600 viewport (area=480000), which already exceeds 150. To
/// test the non-expanded state we set a very high threshold so that 1x zoom
/// does NOT trigger expansion, then lower it (or zoom in) to trigger it.
#[test]
fn colorfield_no_children_before_expansion() {
    use emcore::emPanelTree::ViewConditionType;

    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    // Set a very high threshold so the panel is NOT auto-expanded at 1x.
    h.tree
        .SetAutoExpansionThreshold(panel_id, 1e12, ViewConditionType::Area, None);
    h.tick_n(5);

    assert!(
        !h.is_expanded(panel_id),
        "ColorField should NOT be auto-expanded with threshold=1e12"
    );
    assert_eq!(
        h.tree.child_count(panel_id),
        0,
        "Non-expanded ColorField should have 0 children"
    );

    // Now lower the threshold back to default so that expansion is triggered.
    h.tree
        .SetAutoExpansionThreshold(panel_id, 150.0, ViewConditionType::Area, None);
    h.tick_n(10);

    assert!(
        h.is_expanded(panel_id),
        "ColorField should be auto-expanded after lowering threshold"
    );
    assert!(
        h.tree.child_count(panel_id) >= 1,
        "Expanded ColorField should have at least 1 child"
    );
}

// ---------------------------------------------------------------------------
// Test: expansion name field contains hex string
// ---------------------------------------------------------------------------

/// Verify that the Name text field in the expansion is initialized with the
/// hex representation of the current color.
#[test]
fn colorfield_expanded_name_field_initialized() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let color = emColor::rgba(0xAB, 0xCD, 0xEF, 0xFF);
    let behavior = ColorFieldBehavior::new(look).with_color(color);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any()
        .downcast_ref::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    let exp = cfb.color_field.expansion().expect("expansion should exist");

    assert_eq!(
        exp.tf_name, "#ABCDEF",
        "Name field should be initialized with hex color string"
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ===========================================================================
// BP-12: Sub-widget wiring tests
// ===========================================================================
//
// The C++ emColorField::Cycle() polls sub-widget signals each frame:
// - SfRed/SfGreen/SfBlue/SfAlpha emScalarField GetValue changes → update emColor via
//   RGBA, then sync HSV + Name outputs.
// - SfHue/SfSat/SfVal emScalarField GetValue changes → update emColor via HSV, then
//   sync RGBA + Name outputs.
// - TfName emTextField text change → TryParse hex/name, update emColor, then sync
//   RGBA + HSV outputs.
// - If emColor changed: fire ColorSignal (Rust: on_color callback).
//
// The Rust port stores the sub-widget values in Expansion (sf_red, red_out,
// etc.) and Cycle() detects divergence. ColorFieldBehavior::Cycle() calls
// sync_from_children() to read current sub-widget values, then the existing
// Cycle() change-detection logic propagates updates.
//
// The tests below verify end-to-end pipeline dispatch through the harness.

// ---------------------------------------------------------------------------
// Test: Cycle() propagation — red slider GetValue → color update
// ---------------------------------------------------------------------------

/// After expansion, mutate `Expansion::sf_red` and call `Cycle()`.
/// Verify the color's red channel updates and HSV/name fields are synced.
///
/// This tests the Cycle() contract from C++ emColorField.cpp:116-122:
/// if sf_red != red_out, update emColor.SetRed and mark rgbaChanged.
#[test]
fn colorfield_cycle_red_slider_updates_color_and_syncs() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::BLACK);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    // Take behavior, mutate red, Cycle, inspect.
    let mut behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any_mut()
        .downcast_mut::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    // Set red to 50% (5000 out of 10000).
    cfb.color_field.expansion_mut().unwrap().sf_red = 5000;
    let changed = cfb.color_field.Cycle();
    assert!(
        changed,
        "cycle() should return true when red slider changes"
    );

    // Red channel: (5000 * 255 + 5000) / 10000 = 127 or 128
    let r = cfb.color_field.GetColor().GetRed();
    assert!(
        (r as i64 - 127).abs() <= 1,
        "Red channel should be ~127 after setting sf_red=5000, got {}",
        r
    );

    // HSV should have been synced (rgbaChanged → UpdateHSVOutput).
    // V = max(r,g,b)/255 ≈ 127/255 → ~4980 in C++ units.
    let exp = cfb.color_field.expansion().unwrap();
    assert!(
        exp.sf_val > 4000 && exp.sf_val < 6000,
        "HSV value should be ~4980 (127/255) after setting red to 50%, got {}",
        exp.sf_val
    );

    // Name field should have been synced (rgbaChanged → UpdateNameOutput).
    assert!(
        !exp.tf_name.is_empty(),
        "Name field should be updated after red change"
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// Test: Cycle() propagation — green slider GetValue → color update
// ---------------------------------------------------------------------------

/// Mutate `Expansion::sf_green` and call `Cycle()`. Verify green channel updates.
/// C++ ref: emColorField.cpp:124-131.
#[test]
fn colorfield_cycle_green_slider_updates_color() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::BLACK);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let mut behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any_mut()
        .downcast_mut::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    cfb.color_field.expansion_mut().unwrap().sf_green = 7500;
    let changed = cfb.color_field.Cycle();
    assert!(
        changed,
        "cycle() should return true when green slider changes"
    );

    // Green channel: (7500 * 255 + 5000) / 10000 = 191
    let g = cfb.color_field.GetColor().GetGreen();
    assert!(
        (g as i64 - 191).abs() <= 1,
        "Green channel should be ~191 after setting sf_green=7500, got {}",
        g
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// Test: Cycle() propagation — blue slider GetValue → color update
// ---------------------------------------------------------------------------

/// Mutate `Expansion::sf_blue` and call `Cycle()`. Verify blue channel updates.
/// C++ ref: emColorField.cpp:132-139.
#[test]
fn colorfield_cycle_blue_slider_updates_color() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::BLACK);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let mut behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any_mut()
        .downcast_mut::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    cfb.color_field.expansion_mut().unwrap().sf_blue = 2500;
    let changed = cfb.color_field.Cycle();
    assert!(
        changed,
        "cycle() should return true when blue slider changes"
    );

    // Blue channel: (2500 * 255 + 5000) / 10000 = 64
    let b = cfb.color_field.GetColor().GetBlue();
    assert!(
        (b as i64 - 64).abs() <= 1,
        "Blue channel should be ~64 after setting sf_blue=2500, got {}",
        b
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// Test: Cycle() propagation — hex text entry → color update
// ---------------------------------------------------------------------------

/// Mutate `Expansion::tf_name` to a hex string and call `Cycle()`.
/// Verify the color updates to match the hex GetValue and RGBA/HSV fields sync.
/// C++ ref: emColorField.cpp:187-200.
#[test]
fn colorfield_cycle_hex_text_updates_color() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::BLACK);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let mut behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any_mut()
        .downcast_mut::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    cfb.color_field.expansion_mut().unwrap().tf_name = "#00FF80".to_string();
    let changed = cfb.color_field.Cycle();
    assert!(changed, "cycle() should return true when text changes");

    let c = cfb.color_field.GetColor();
    assert_eq!(c.GetRed(), 0x00, "Red should be 0x00 for #00FF80");
    assert_eq!(c.GetGreen(), 0xFF, "Green should be 0xFF for #00FF80");
    assert_eq!(c.GetBlue(), 0x80, "Blue should be 0x80 for #00FF80");

    // RGBA expansion fields should be synced (textChanged → UpdateRGBAOutput).
    let exp = cfb.color_field.expansion().unwrap();
    // #00FF80: red=0x00, green=0xFF. Formula: (channel * 10000 + 127) / 255.
    assert_eq!(exp.sf_red, 0, "sf_red should be 0 for red=0x00 (#00FF80)");
    assert_eq!(
        exp.sf_green,
        (255i64 * 10000 + 127) / 255,
        "sf_green should be synced after hex text change"
    );

    // HSV expansion fields should also be synced (textChanged → UpdateHSVOutput).
    // #00FF80 = rgb(0,255,128): H = 60*(128-0)/255 + 120 ≈ 150° → ~15000 in C++ units.
    assert!(
        exp.sf_hue > 13000 && exp.sf_hue < 17000,
        "Hue should be ~15000 (150°) for #00FF80 (green-cyan), got {}",
        exp.sf_hue
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// Test: Cycle() propagation — HSV change → RGB fields sync
// ---------------------------------------------------------------------------

/// Mutate HSV expansion fields and call `Cycle()`. Verify that RGBA expansion
/// fields are updated to match the new HSV color.
/// C++ ref: emColorField.cpp:148-186 (HSV changes) + line 203 (UpdateRGBAOutput).
#[test]
fn colorfield_cycle_hsv_change_syncs_rgb_fields() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    // Start with black so any HSV change is detectable.
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::BLACK);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let mut behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any_mut()
        .downcast_mut::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    // Set to pure green via HSV: hue=120° (12000), sat=100% (10000), val=100% (10000).
    {
        let exp = cfb.color_field.expansion_mut().unwrap();
        exp.sf_hue = 12000;
        exp.sf_sat = 10000;
        exp.sf_val = 10000;
    }
    let changed = cfb.color_field.Cycle();
    assert!(changed, "cycle() should return true when HSV changes");

    // After HSV change, RGBA fields should be synced to green.
    let exp = cfb.color_field.expansion().unwrap();
    // Red should be near 0.
    assert!(
        exp.sf_red < 500,
        "Red expansion should be ~0 for pure green, got {}",
        exp.sf_red
    );
    // Green should be near 10000.
    assert!(
        exp.sf_green > 9500,
        "Green expansion should be ~10000 for pure green, got {}",
        exp.sf_green
    );
    // Blue should be near 0.
    assert!(
        exp.sf_blue < 500,
        "Blue expansion should be ~0 for pure green, got {}",
        exp.sf_blue
    );

    // Name should also be synced (hsvChanged → UpdateNameOutput).
    assert!(
        !exp.tf_name.is_empty(),
        "Name field should be updated after HSV change"
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// Test: Cycle() propagation — RGB change → HSV fields sync
// ---------------------------------------------------------------------------

/// Mutate RGBA expansion fields and call `Cycle()`. Verify that HSV expansion
/// fields are updated to match the new RGB color.
/// C++ ref: emColorField.cpp:116-147 (RGBA changes) + line 204 (UpdateHSVOutput).
#[test]
fn colorfield_cycle_rgb_change_syncs_hsv_fields() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::BLACK);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let mut behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any_mut()
        .downcast_mut::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    // Set to bright red via RGBA sliders.
    {
        let exp = cfb.color_field.expansion_mut().unwrap();
        exp.sf_red = 10000;
        exp.sf_green = 0;
        exp.sf_blue = 0;
    }
    let changed = cfb.color_field.Cycle();
    assert!(changed, "cycle() should return true when RGBA changes");

    // HSV fields should be synced: hue~0 (red), sat~10000, val~10000.
    let exp = cfb.color_field.expansion().unwrap();
    // Hue should be near 0 or 36000 (red wraps around).
    assert!(
        exp.sf_hue < 500 || exp.sf_hue > 35500,
        "Hue should be ~0 deg (red) after setting pure red via RGBA, got {}",
        exp.sf_hue
    );
    // Saturation should be near 10000 (fully saturated).
    assert!(
        exp.sf_sat > 9500,
        "Saturation should be ~10000 for pure red, got {}",
        exp.sf_sat
    );
    // Value should be near 10000 (full brightness).
    assert!(
        exp.sf_val > 9500,
        "Value should be ~10000 for pure red, got {}",
        exp.sf_val
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// Test: on_color callback fires when Cycle() detects a change
// ---------------------------------------------------------------------------

/// Set the `on_color` callback, mutate expansion data, call `Cycle()`, and
/// verify the callback fires with the new color.
/// C++ ref: emColorField.cpp:207 (Signal(ColorSignal)).
#[test]
fn colorfield_cycle_fires_on_color_callback() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::BLACK);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let mut behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any_mut()
        .downcast_mut::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    // Install a callback that records the received color.
    let received = Rc::new(std::cell::RefCell::new(None::<emColor>));
    let received_clone = received.clone();
    cfb.color_field.on_color = Some(Box::new(move |c| {
        *received_clone.borrow_mut() = Some(c);
    }));

    // Mutate green channel and Cycle.
    cfb.color_field.expansion_mut().unwrap().sf_green = 10000;
    cfb.color_field.Cycle();

    let cb_color = received.borrow();
    let cb_color =
        cb_color.expect("on_color callback should have fired after cycle() detects a change");
    assert_eq!(
        cb_color.GetGreen(),
        255,
        "Callback should receive color with green=255, got {}",
        cb_color.GetGreen()
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// Test: Cycle() with no changes returns false
// ---------------------------------------------------------------------------

/// When no expansion fields have changed, `Cycle()` should return false and
/// the color should remain unchanged.
/// C++ ref: emColorField.cpp:109 (early return when !Exp) and 247 (no-change).
#[test]
fn colorfield_cycle_no_change_returns_false() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let color = emColor::rgba(100, 150, 200, 255);
    let behavior = ColorFieldBehavior::new(look).with_color(color);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let mut behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any_mut()
        .downcast_mut::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    let changed = cfb.color_field.Cycle();
    assert!(
        !changed,
        "cycle() should return false when no expansion fields have changed"
    );
    assert_eq!(
        cfb.color_field.GetColor(),
        color,
        "Color should remain unchanged when cycle() detects no change"
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// Test: invalid hex text does not change color
// ---------------------------------------------------------------------------

/// When an invalid hex string is entered in tf_name, Cycle() should not crash
/// and the color should remain unchanged (C++ catches emException and reverts).
/// C++ ref: emColorField.cpp:191-196 (try/catch).
#[test]
fn colorfield_cycle_invalid_hex_preserves_color() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let original = emColor::rgba(100, 150, 200, 255);
    let behavior = ColorFieldBehavior::new(look).with_color(original);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let mut behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any_mut()
        .downcast_mut::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    // Set an invalid hex string.
    cfb.color_field.expansion_mut().unwrap().tf_name = "not-a-color".to_string();
    cfb.color_field.Cycle();

    // emColor should remain unchanged since parsing failed.
    // The Rust code only updates color if try_parse succeeds, so invalid text
    // leaves the color at whatever it was before.
    let c = cfb.color_field.GetColor();
    assert_eq!(
        c.GetRed(),
        original.GetRed(),
        "Red should be preserved after invalid hex"
    );
    assert_eq!(
        c.GetGreen(),
        original.GetGreen(),
        "Green should be preserved after invalid hex"
    );
    assert_eq!(
        c.GetBlue(),
        original.GetBlue(),
        "Blue should be preserved after invalid hex"
    );

    h.tree.put_behavior(panel_id, behavior);
}

// ---------------------------------------------------------------------------
// BLOCKED: end-to-end pipeline dispatch tests
// ---------------------------------------------------------------------------
// The following tests require the ScalarFieldPanel/TextFieldPanel sub-widgets
// to wire their on_value/on_text callbacks back to the emColorField's Expansion
// data, AND for ColorFieldBehavior to implement PanelBehavior::Cycle() so that
// the scheduler drives change detection. Until that wiring exists, these tests
// cannot exercise the full Input->dispatch->Cycle->color-update pipeline.

/// Click on the Red emScalarField sub-widget at its slider GetPos and verify
/// the emColorField's color red channel changes.
///
/// BLOCKED: needs ScalarFieldPanel.on_value wired to Expansion.sf_red, and
/// ColorFieldBehavior::Cycle() implemented. C++ ref: emColorField.cpp:116-122.
#[test]
fn colorfield_click_red_slider_updates_color_e2e() {
    // BLOCKED: needs sub-widget on_value callback wiring to Expansion.sf_red,
    // and ColorFieldBehavior::Cycle() to propagate changes.
    // C++ ref: emColorField.cpp:116-122 (SfRed signal -> emColor.SetRed).
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::BLACK);
    let _panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));
    h.tick();
    h.expand_to(16.0);
    // Would need: find red slider view-space bounds, Click/drag, tick, verify color.
}

/// Type a hex GetValue into the Name emTextField sub-widget and verify the
/// emColorField's color updates.
///
/// BLOCKED: needs TextFieldPanel.on_text wired to Expansion.tf_name, and
/// ColorFieldBehavior::Cycle() implemented. C++ ref: emColorField.cpp:187-200.
#[test]
fn colorfield_type_hex_in_text_field_updates_color_e2e() {
    // BLOCKED: needs sub-widget on_text callback wiring to Expansion.tf_name,
    // and ColorFieldBehavior::Cycle() to propagate changes.
    // C++ ref: emColorField.cpp:187-200 (TfName signal -> emColor.TryParse).
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::BLACK);
    let _panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));
    h.tick();
    h.expand_to(16.0);
    // Would need: focus text field, type "#FF0000", tick, verify color.
}

/// Drag the Hue emScalarField slider and verify that RGB expansion fields
/// and the color update accordingly.
///
/// BLOCKED: needs ScalarFieldPanel.on_value wired to Expansion.sf_hue, and
/// ColorFieldBehavior::Cycle() implemented. C++ ref: emColorField.cpp:148-159.
#[test]
fn colorfield_drag_hue_slider_updates_rgb_e2e() {
    // BLOCKED: needs sub-widget on_value callback wiring to Expansion.sf_hue,
    // and ColorFieldBehavior::Cycle() to propagate changes.
    // C++ ref: emColorField.cpp:148-159 (SfHue signal -> emColor.SetHSVA).
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::rgba(255, 0, 0, 255));
    let _panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));
    h.tick();
    h.expand_to(16.0);
    // Would need: find hue slider bounds, drag to new GetPos, tick, verify RGBA fields.
}

// ---------------------------------------------------------------------------
// Phase 2C: emColorFieldFieldPanel coverage
// ---------------------------------------------------------------------------

/// After expansion, verify that each scalar-field child panel has a non-zero
/// layout rect (proving the layout was computed and the panel would paint).
#[test]
fn colorfield_scalar_panels_paint() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::RED);
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let layout_id = h.tree.children(panel_id).next().expect("raster layout");
    let scalar_names = ["r", "g", "b", "a", "h", "s", "v"];

    for child_id in h.tree.children(layout_id) {
        let name = h.tree.name(child_id).unwrap_or("?").to_string();
        if !scalar_names.contains(&name.as_str()) {
            continue;
        }
        let r = h
            .tree
            .layout_rect(child_id)
            .expect("layout rect should exist");
        assert!(
            r.w > 0.0 && r.h > 0.0,
            "Scalar panel '{name}' should have non-zero layout rect, got {r:?}"
        );
    }
}

/// After expanding with #FF0000FF, the text field child's expansion data
/// should contain "FF0000" (no alpha suffix for opaque colors).
#[test]
fn colorfield_text_panel_hex() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let behavior = ColorFieldBehavior::new(look).with_color(emColor::rgba(0xFF, 0x00, 0x00, 0xFF));
    let panel_id = h.add_panel_with(root, "color_field", Box::new(behavior));

    h.tick();
    h.expand_to(16.0);

    let behavior = h.tree.take_behavior(panel_id).expect("behavior exists");
    let cfb = behavior
        .as_any()
        .downcast_ref::<ColorFieldBehavior>()
        .expect("should be ColorFieldBehavior");

    let exp = cfb.color_field.expansion().expect("expansion should exist");

    assert!(
        exp.tf_name.contains("FF0000"),
        "Text field should contain 'FF0000' for pure red, got '{}'",
        exp.tf_name
    );

    h.tree.put_behavior(panel_id, behavior);
}
