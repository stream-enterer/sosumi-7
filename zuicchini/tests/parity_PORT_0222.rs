use zuicchini::foundation::Color;
use zuicchini::widget::{ColorField, Look};

/// PORT-0222: emColorField::Cycle() (virtual)
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
