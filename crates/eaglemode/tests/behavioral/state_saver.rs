use emcore::emRecParser::RecError;
use emcore::emRecParser::{parse_rec, write_rec, RecStruct};
use emcore::emRecRecord::Record;
use emcore::emWindowStateSaver::WindowGeometry;

#[test]
fn window_geometry_default_values() {
    let geo = WindowGeometry::default();
    assert!((geo.ViewX - 100.0).abs() < f64::EPSILON);
    assert!((geo.ViewY - 100.0).abs() < f64::EPSILON);
    assert!((geo.ViewWidth - 1280.0).abs() < f64::EPSILON);
    assert!((geo.ViewHeight - 720.0).abs() < f64::EPSILON);
    assert!(!geo.Maximized);
    assert!(!geo.Fullscreen);
}

#[test]
fn window_geometry_round_trip_to_rec() {
    let original = WindowGeometry {
        ViewX: 200.0,
        ViewY: 300.0,
        ViewWidth: 1920.0,
        ViewHeight: 1080.0,
        Maximized: true,
        Fullscreen: false,
    };

    let rec = original.to_rec();
    let restored = WindowGeometry::from_rec(&rec).expect("from_rec");
    assert_eq!(restored, original);
}

#[test]
fn window_geometry_round_trip_through_text() {
    let original = WindowGeometry {
        ViewX: -50.0,
        ViewY: 0.0,
        ViewWidth: 800.0,
        ViewHeight: 600.0,
        Maximized: false,
        Fullscreen: true,
    };

    let rec = original.to_rec();
    let text = write_rec(&rec);
    let parsed = parse_rec(&text).expect("parse");
    let restored = WindowGeometry::from_rec(&parsed).expect("from_rec");
    assert_eq!(restored, original);
}

#[test]
fn window_geometry_negative_coordinates() {
    let geo = WindowGeometry {
        ViewX: -100.0,
        ViewY: -200.0,
        ViewWidth: 640.0,
        ViewHeight: 480.0,
        Maximized: false,
        Fullscreen: false,
    };

    let rec = geo.to_rec();
    let restored = WindowGeometry::from_rec(&rec).expect("from_rec");
    assert!((restored.ViewX - (-100.0)).abs() < f64::EPSILON);
    assert!((restored.ViewY - (-200.0)).abs() < f64::EPSILON);
}

#[test]
fn window_geometry_from_rec_missing_view_x_fails() {
    let mut rec = RecStruct::new();
    rec.set_double("ViewY", 100.0);
    rec.set_double("ViewWidth", 800.0);
    rec.set_double("ViewHeight", 600.0);

    let result = WindowGeometry::from_rec(&rec);
    assert!(matches!(result, Err(RecError::MissingField(_))));
}

#[test]
fn window_geometry_from_rec_missing_view_width_fails() {
    let mut rec = RecStruct::new();
    rec.set_double("ViewX", 100.0);
    rec.set_double("ViewY", 100.0);
    rec.set_double("ViewHeight", 600.0);

    let result = WindowGeometry::from_rec(&rec);
    assert!(matches!(result, Err(RecError::MissingField(_))));
}

#[test]
fn window_geometry_bools_default_false_when_absent() {
    let mut rec = RecStruct::new();
    rec.set_double("ViewX", 10.0);
    rec.set_double("ViewY", 20.0);
    rec.set_double("ViewWidth", 800.0);
    rec.set_double("ViewHeight", 600.0);

    let geo = WindowGeometry::from_rec(&rec).expect("from_rec");
    assert!(!geo.Maximized);
    assert!(!geo.Fullscreen);
}

#[test]
fn window_geometry_is_default() {
    let geo = WindowGeometry::default();
    assert!(geo.IsSetToDefault());

    let modified = WindowGeometry {
        ViewX: 999.0,
        ..WindowGeometry::default()
    };
    assert!(!modified.IsSetToDefault());
}

#[test]
fn window_geometry_set_to_default() {
    let mut geo = WindowGeometry {
        ViewX: 999.0,
        ViewY: 888.0,
        ViewWidth: 100.0,
        ViewHeight: 50.0,
        Maximized: true,
        Fullscreen: true,
    };
    geo.SetToDefault();
    assert_eq!(geo, WindowGeometry::default());
}

#[test]
fn window_geometry_all_fields_serialized() {
    let geo = WindowGeometry {
        ViewX: 42.0,
        ViewY: 84.0,
        ViewWidth: 1600.0,
        ViewHeight: 900.0,
        Maximized: true,
        Fullscreen: true,
    };

    let rec = geo.to_rec();
    assert!((rec.get_double("ViewX").unwrap() - 42.0).abs() < f64::EPSILON);
    assert!((rec.get_double("ViewY").unwrap() - 84.0).abs() < f64::EPSILON);
    assert!((rec.get_double("ViewWidth").unwrap() - 1600.0).abs() < f64::EPSILON);
    assert!((rec.get_double("ViewHeight").unwrap() - 900.0).abs() < f64::EPSILON);
    assert_eq!(rec.get_bool("Maximized"), Some(true));
    assert_eq!(rec.get_bool("Fullscreen"), Some(true));
}

#[test]
fn geometry_extreme_values() {
    let geo = WindowGeometry {
        ViewX: f64::MAX,
        ViewY: f64::MAX,
        ViewWidth: f64::MAX,
        ViewHeight: f64::MAX,
        Maximized: false,
        Fullscreen: false,
    };

    let rec = geo.to_rec();
    let restored = WindowGeometry::from_rec(&rec).expect("from_rec");
    assert_eq!(restored.ViewX, f64::MAX);
    assert_eq!(restored.ViewY, f64::MAX);
}

#[test]
fn geometry_zero_size() {
    let geo = WindowGeometry {
        ViewX: 50.0,
        ViewY: 50.0,
        ViewWidth: 0.0,
        ViewHeight: 0.0,
        Maximized: false,
        Fullscreen: false,
    };

    let rec = geo.to_rec();
    let restored = WindowGeometry::from_rec(&rec).expect("from_rec");
    assert!((restored.ViewWidth).abs() < f64::EPSILON);
    assert!((restored.ViewHeight).abs() < f64::EPSILON);
}

#[test]
fn geometry_partial_bools() {
    let mut rec = RecStruct::new();
    rec.set_double("ViewX", 10.0);
    rec.set_double("ViewY", 20.0);
    rec.set_double("ViewWidth", 800.0);
    rec.set_double("ViewHeight", 600.0);
    rec.set_bool("Maximized", true);
    // Do NOT set "Fullscreen" — should default to false.

    let geo = WindowGeometry::from_rec(&rec).expect("from_rec");
    assert!(geo.Maximized);
    assert!(!geo.Fullscreen);
}
