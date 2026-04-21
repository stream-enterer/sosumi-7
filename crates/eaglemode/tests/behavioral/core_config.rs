use std::rc::Rc;

use emcore::emContext::emContext;
use emcore::emCoreConfig::emCoreConfig;
use emcore::emPanelTree::PanelTree;
use emcore::emRecParser::RecStruct;
use emcore::emRecRecord::Record;
use emcore::emView::emView;

#[test]
fn defaults_match_cpp() {
    let cfg = emCoreConfig::default();
    assert!(!cfg.stick_mouse_when_navigating);
    assert!(!cfg.emulate_middle_button);
    assert!(!cfg.pan_function);
    assert_eq!(cfg.mouse_zoom_speed, 1.0);
    assert_eq!(cfg.mouse_scroll_speed, 1.0);
    assert_eq!(cfg.mouse_wheel_zoom_speed, 1.0);
    assert_eq!(cfg.mouse_wheel_zoom_acceleration, 1.0);
    assert_eq!(cfg.keyboard_zoom_speed, 1.0);
    assert_eq!(cfg.keyboard_scroll_speed, 1.0);
    assert_eq!(cfg.kinetic_zooming_and_scrolling, 1.0);
    assert_eq!(cfg.magnetism_radius, 1.0);
    assert_eq!(cfg.magnetism_speed, 1.0);
    assert_eq!(cfg.visit_speed, 1.0);
    assert_eq!(cfg.max_megabytes_per_view, 2048);
    assert_eq!(cfg.max_render_threads, 8);
    assert!(cfg.allow_simd);
    assert_eq!(cfg.downscale_quality, 3); // DQ_3X3
    assert_eq!(cfg.upscale_quality, 2); // UQ_BILINEAR
}

#[test]
fn round_trip_all_fields() {
    let cfg = emCoreConfig {
        stick_mouse_when_navigating: true,
        emulate_middle_button: true,
        pan_function: true,
        mouse_zoom_speed: 2.5,
        mouse_scroll_speed: 3.0,
        mouse_wheel_zoom_speed: 0.5,
        mouse_wheel_zoom_acceleration: 1.5,
        keyboard_zoom_speed: 3.5,
        keyboard_scroll_speed: 0.25,
        kinetic_zooming_and_scrolling: 0.75,
        magnetism_radius: 2.0,
        magnetism_speed: 3.0,
        visit_speed: 5.0,
        max_megabytes_per_view: 4096,
        max_render_threads: 16,
        allow_simd: false,
        downscale_quality: 6,
        upscale_quality: 5,
    };

    let rec = cfg.to_rec();
    let restored = emCoreConfig::from_rec(&rec).unwrap();
    assert_eq!(cfg, restored);
}

#[test]
fn clamping_double_fields() {
    let mut rec = RecStruct::new();
    // Out-of-range values
    rec.set_double("MouseZoomSpeed", 100.0); // max 4.0
    rec.set_double("MouseScrollSpeed", 0.01); // min 0.25
    rec.set_double("MouseWheelZoomAcceleration", 5.0); // max 2.0
    rec.set_double("VisitSpeed", 0.001); // min 0.1
    rec.set_double("KineticZoomingAndScrolling", 99.0); // max 2.0

    let cfg = emCoreConfig::from_rec(&rec).unwrap();
    assert_eq!(cfg.mouse_zoom_speed, 4.0);
    assert_eq!(cfg.mouse_scroll_speed, 0.25);
    assert_eq!(cfg.mouse_wheel_zoom_acceleration, 2.0);
    assert_eq!(cfg.visit_speed, 0.1);
    assert_eq!(cfg.kinetic_zooming_and_scrolling, 2.0);
}

#[test]
fn clamping_int_fields() {
    let mut rec = RecStruct::new();
    rec.set_int("MaxMegabytesPerView", 1); // min 8
    rec.set_int("MaxRenderThreads", 100); // max 32
    rec.set_int("DownscaleQuality", 0); // min 2 (DQ_2X2)
    rec.set_int("UpscaleQuality", 99); // max 5 (UQ_ADAPTIVE)

    let cfg = emCoreConfig::from_rec(&rec).unwrap();
    assert_eq!(cfg.max_megabytes_per_view, 8);
    assert_eq!(cfg.max_render_threads, 32);
    assert_eq!(cfg.downscale_quality, 2);
    assert_eq!(cfg.upscale_quality, 5);
}

#[test]
fn missing_fields_use_defaults() {
    let rec = RecStruct::new();
    let cfg = emCoreConfig::from_rec(&rec).unwrap();
    assert_eq!(cfg, emCoreConfig::default());
}

#[test]
fn acquire_returns_singleton() {
    let ctx = emContext::NewRoot();
    let m1 = emCoreConfig::Acquire(&ctx);
    let m2 = emCoreConfig::Acquire(&ctx);
    assert!(std::rc::Rc::ptr_eq(&m1, &m2));
}

#[test]
fn core_config_is_singleton_across_sibling_contexts() {
    let root = emContext::NewRoot();
    let child_a = emContext::NewChild(&root);
    let child_b = emContext::NewChild(&root);

    let m_a = emCoreConfig::Acquire(&child_a);
    let m_b = emCoreConfig::Acquire(&child_b);
    let m_root = emCoreConfig::Acquire(&root);

    assert!(std::rc::Rc::ptr_eq(&m_a, &m_b));
    assert!(std::rc::Rc::ptr_eq(&m_a, &m_root));
}

#[test]
fn sp7_sibling_views_share_core_config_singleton() {
    // Two views built under the same parent emContext see the same
    // emCoreConfig singleton — per C++ Acquire semantics (emView.cpp:35).
    let root = emContext::NewRoot();

    let mut tree1 = PanelTree::new();
    let p1 = tree1.create_root_deferred_view("");
    let mut tree2 = PanelTree::new();
    let p2 = tree2.create_root_deferred_view("");

    let v1 = emView::new(Rc::clone(&root), p1, 100.0, 100.0);
    let v2 = emView::new(Rc::clone(&root), p2, 100.0, 100.0);

    assert!(Rc::ptr_eq(&v1.CoreConfig, &v2.CoreConfig));
    let _ = (tree1, tree2);
}

#[test]
fn set_to_default_restores_defaults() {
    let mut cfg = emCoreConfig {
        mouse_zoom_speed: 3.5,
        max_render_threads: 1,
        allow_simd: false,
        ..emCoreConfig::default()
    };
    assert!(!cfg.IsSetToDefault());
    cfg.SetToDefault();
    assert!(cfg.IsSetToDefault());
    assert_eq!(cfg, emCoreConfig::default());
}
