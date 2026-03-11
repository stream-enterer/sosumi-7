use zuicchini::foundation::Color;
use zuicchini::widget::{ColorField, Look};

/// PORT-0224: emColorField::Expansion struct
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
