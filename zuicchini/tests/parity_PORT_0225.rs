use zuicchini::foundation::Color;
use zuicchini::widget::{ColorField, Look};

/// PORT-0225: emColorField::UpdateRGBAOutput / UpdateHSVOutput / UpdateNameOutput
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
