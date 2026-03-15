use zuicchini::foundation::Color;
use zuicchini::widget::{ColorField, ListBox, Look};

/// emColorField — auto-expand / auto-shrink panel lifecycle.
#[test]
fn auto_expand_creates_all_panels() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "A".to_string());
    lb.add_item("b".to_string(), "B".to_string());
    lb.add_item("c".to_string(), "C".to_string());

    assert!(lb.get_item_panel(0).is_none());
    lb.auto_expand_items();
    assert!(lb.get_item_panel(0).is_some());
    assert!(lb.get_item_panel(1).is_some());
    assert!(lb.get_item_panel(2).is_some());
}

#[test]
fn auto_shrink_destroys_all_panels() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "A".to_string());
    lb.auto_expand_items();
    assert!(lb.get_item_panel(0).is_some());

    lb.auto_shrink_items();
    assert!(lb.get_item_panel(0).is_none());
}

#[test]
fn auto_expand_idempotent() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "A".to_string());
    lb.auto_expand_items();
    lb.auto_expand_items(); // second call should not create duplicates
    assert!(lb.get_item_panel(0).is_some());
}

#[test]
fn auto_expand_preserves_selection() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "A".to_string());
    lb.select(0, true);
    lb.auto_expand_items();
    assert!(lb.get_item_panel(0).unwrap().is_selected());
}

/// emColorField::Cycle() (virtual)
/// Polls expansion children for value changes and synchronizes color.
#[test]
fn cycle_returns_false_when_not_expanded() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    assert!(!cf.cycle());
}

#[test]
fn cycle_returns_false_when_no_changes() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_expanded(true);
    // No changes since expansion was initialized
    assert!(!cf.cycle());
}

#[test]
fn cycle_detects_rgba_change() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(Color::BLACK);
    cf.set_expanded(true);

    // Modify red channel
    cf.expansion_mut().unwrap().sf_red = 10000; // max = 255
    assert!(cf.cycle());
    assert_eq!(cf.color().r(), 255);
}

#[test]
fn cycle_detects_hsv_change() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_expanded(true);

    // Set to pure green via HSV: hue=120°, sat=100%, val=100%
    let exp = cf.expansion_mut().unwrap();
    exp.sf_hue = 12000; // 120.00°
    exp.sf_sat = 10000; // 100%
    exp.sf_val = 10000; // 100%
    assert!(cf.cycle());

    // Should be green
    assert!(cf.color().g() > 200);
}

#[test]
fn cycle_detects_text_change() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_expanded(true);

    cf.expansion_mut().unwrap().tf_name = "#00FF00".to_string();
    assert!(cf.cycle());
    assert_eq!(cf.color(), Color::rgba(0, 255, 0, 255));
}

#[test]
fn cycle_syncs_sibling_fields_on_rgba_change() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(Color::BLACK);
    cf.set_expanded(true);

    // Set pure red via RGBA
    cf.expansion_mut().unwrap().sf_red = 10000;
    cf.cycle();

    // HSV and name should have been synced
    let exp = cf.expansion().unwrap();
    assert!(exp.sf_val > 0); // value should be non-zero
    assert!(exp.tf_name.contains("FF")); // name should contain red
}

/// emColorField::Expansion struct
/// Verifies the Expansion struct fields and value ranges.
#[test]
fn expansion_exists_when_expanded() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    assert!(cf.expansion().is_none());
    cf.set_expanded(true);
    assert!(cf.expansion().is_some());
    cf.set_expanded(false);
    assert!(cf.expansion().is_none());
}

#[test]
fn expansion_rgba_fields_match_color() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(Color::rgba(100, 150, 200, 128));
    cf.set_expanded(true);

    let exp = cf.expansion().unwrap();
    // C++ formula: (channel * 10000 + 127) / 255
    assert_eq!(exp.sf_red, (100i64 * 10000 + 127) / 255);
    assert_eq!(exp.sf_green, (150i64 * 10000 + 127) / 255);
    assert_eq!(exp.sf_blue, (200i64 * 10000 + 127) / 255);
    assert_eq!(exp.sf_alpha, (128i64 * 10000 + 127) / 255);
}

#[test]
fn expansion_hsv_fields_for_pure_red() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(Color::rgba(255, 0, 0, 255));
    cf.set_expanded(true);

    let exp = cf.expansion().unwrap();
    // Hue should be ~0 (red), sat ~10000 (100%), val ~10000 (100%)
    assert!(exp.sf_hue < 100, "hue {} should be near 0", exp.sf_hue);
    assert!(exp.sf_sat > 9900, "sat {} should be near 10000", exp.sf_sat);
    assert!(exp.sf_val > 9900, "val {} should be near 10000", exp.sf_val);
}

#[test]
fn expansion_name_field_hex_format() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(Color::rgba(0xAB, 0xCD, 0xEF, 0xFF));
    cf.set_expanded(true);

    let exp = cf.expansion().unwrap();
    assert_eq!(exp.tf_name, "#ABCDEF");
}

#[test]
fn expansion_name_field_with_alpha() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(Color::rgba(0x12, 0x34, 0x56, 0x78));
    cf.set_expanded(true);

    let exp = cf.expansion().unwrap();
    assert_eq!(exp.tf_name, "#12345678");
}

#[test]
fn expansion_mut_allows_modification() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_expanded(true);

    let exp = cf.expansion_mut().unwrap();
    exp.sf_red = 5000;
    exp.sf_green = 7500;
    assert_eq!(cf.expansion().unwrap().sf_red, 5000);
    assert_eq!(cf.expansion().unwrap().sf_green, 7500);
}

/// emColorField::UpdateRGBAOutput / UpdateHSVOutput / UpdateNameOutput
#[test]
fn update_rgba_output_syncs_from_color() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(Color::rgba(200, 100, 50, 255));
    cf.set_expanded(true);

    let exp = cf.expansion().unwrap();
    assert_eq!(exp.sf_red, (200i64 * 10000 + 127) / 255);
    assert_eq!(exp.sf_green, (100i64 * 10000 + 127) / 255);
    assert_eq!(exp.sf_blue, (50i64 * 10000 + 127) / 255);
}

#[test]
fn update_hsv_output_preserves_hue_at_zero_value() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    // Start with red
    cf.set_color(Color::rgba(255, 0, 0, 255));
    cf.set_expanded(true);

    let hue_red = cf.expansion().unwrap().sf_hue;

    // Set to black — hue should NOT change (C++ preserves hue when v=0)
    cf.set_color(Color::rgba(0, 0, 0, 255));

    let hue_after = cf.expansion().unwrap().sf_hue;
    assert_eq!(hue_red, hue_after, "hue should be preserved when v=0");
}

#[test]
fn update_hsv_output_preserves_sat_at_zero_value() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    // Start with a saturated color
    cf.set_color(Color::rgba(255, 0, 0, 255));
    cf.set_expanded(true);

    let sat_before = cf.expansion().unwrap().sf_sat;

    // Set to black — sat should NOT change (C++ preserves sat when v=0)
    cf.set_color(Color::rgba(0, 0, 0, 255));

    let sat_after = cf.expansion().unwrap().sf_sat;
    assert_eq!(sat_before, sat_after, "sat should be preserved when v=0");
}

#[test]
fn update_hsv_output_initial_sets_all() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    // Black color — on initial expansion, all HSV values should be set
    cf.set_color(Color::rgba(0, 0, 0, 255));
    cf.set_expanded(true);

    let exp = cf.expansion().unwrap();
    // With initial=true (used in auto_expand), hue/sat/val are all set
    assert_eq!(exp.sf_val, 0); // black has v=0
}

#[test]
fn update_name_output_hex_without_alpha() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(Color::rgba(0xFF, 0x00, 0xFF, 0xFF));
    cf.set_expanded(true);
    assert_eq!(cf.expansion().unwrap().tf_name, "#FF00FF");
}

#[test]
fn update_name_output_hex_with_alpha() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(Color::rgba(0x12, 0x34, 0x56, 0x78));
    cf.set_expanded(true);
    assert_eq!(cf.expansion().unwrap().tf_name, "#12345678");
}

#[test]
fn set_color_syncs_expansion() {
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_expanded(true);
    cf.set_color(Color::rgba(128, 64, 32, 255));

    let exp = cf.expansion().unwrap();
    // RGBA should match
    assert_eq!(exp.sf_red, (128i64 * 10000 + 127) / 255);
    assert_eq!(exp.sf_green, (64i64 * 10000 + 127) / 255);
    // Name should match
    assert!(exp.tf_name.starts_with('#'));
}
