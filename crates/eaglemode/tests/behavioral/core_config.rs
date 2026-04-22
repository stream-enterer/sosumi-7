use std::cell::RefCell;
use std::rc::Rc;

use emcore::emClipboard::emClipboard;
use emcore::emContext::emContext;
use emcore::emCoreConfig::emCoreConfig;
use emcore::emEngineCtx::{DeferredAction, FrameworkDeferredAction, SchedCtx};
use emcore::emPanelTree::PanelTree;
use emcore::emRec::emRec;
use emcore::emScheduler::EngineScheduler;
use emcore::emView::emView;

fn make_sched_ctx<'a>(
    sched: &'a mut EngineScheduler,
    actions: &'a mut Vec<DeferredAction>,
    ctx_root: &'a Rc<emContext>,
    cb: &'a RefCell<Option<Box<dyn emClipboard>>>,
    pa: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>,
) -> SchedCtx<'a> {
    SchedCtx {
        scheduler: sched,
        framework_actions: actions,
        root_context: ctx_root,
        framework_clipboard: cb,
        current_engine: None,
        pending_actions: pa,
    }
}

#[test]
fn defaults_match_cpp() {
    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let cfg = emCoreConfig::new(&mut sc);
    assert!(!*cfg.StickMouseWhenNavigating.GetValue());
    assert!(!*cfg.EmulateMiddleButton.GetValue());
    assert!(!*cfg.PanFunction.GetValue());
    assert_eq!(*cfg.MouseZoomSpeed.GetValue(), 1.0);
    assert_eq!(*cfg.MouseScrollSpeed.GetValue(), 1.0);
    assert_eq!(*cfg.MouseWheelZoomSpeed.GetValue(), 1.0);
    assert_eq!(*cfg.MouseWheelZoomAcceleration.GetValue(), 1.0);
    assert_eq!(*cfg.KeyboardZoomSpeed.GetValue(), 1.0);
    assert_eq!(*cfg.KeyboardScrollSpeed.GetValue(), 1.0);
    assert_eq!(*cfg.KineticZoomingAndScrolling.GetValue(), 1.0);
    assert_eq!(*cfg.MagnetismRadius.GetValue(), 1.0);
    assert_eq!(*cfg.MagnetismSpeed.GetValue(), 1.0);
    assert_eq!(*cfg.VisitSpeed.GetValue(), 1.0);
    assert_eq!(*cfg.MaxMegabytesPerView.GetValue(), 2048);
    assert_eq!(*cfg.MaxRenderThreads.GetValue(), 8);
    assert!(*cfg.AllowSIMD.GetValue());
    assert_eq!(*cfg.DownscaleQuality.GetValue(), 3); // DQ_3X3
    assert_eq!(*cfg.UpscaleQuality.GetValue(), 2); // UQ_BILINEAR
}

#[test]
fn round_trip_all_fields() {
    use emcore::emRecNodeConfigModel::emRecNodeConfigModel;

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("core_config_rt.rec");

    // Write with non-default values.
    {
        let mut cfg = emCoreConfig::new(&mut sc);
        cfg.StickMouseWhenNavigating.SetValue(true, &mut sc);
        cfg.EmulateMiddleButton.SetValue(true, &mut sc);
        cfg.PanFunction.SetValue(true, &mut sc);
        cfg.MouseZoomSpeed.SetValue(2.5, &mut sc);
        cfg.MouseScrollSpeed.SetValue(3.0, &mut sc);
        cfg.MouseWheelZoomSpeed.SetValue(0.5, &mut sc);
        cfg.MouseWheelZoomAcceleration.SetValue(1.5, &mut sc);
        cfg.KeyboardZoomSpeed.SetValue(3.5, &mut sc);
        cfg.KeyboardScrollSpeed.SetValue(0.25, &mut sc);
        cfg.KineticZoomingAndScrolling.SetValue(0.75, &mut sc);
        cfg.MagnetismRadius.SetValue(2.0, &mut sc);
        cfg.MagnetismSpeed.SetValue(3.0, &mut sc);
        cfg.VisitSpeed.SetValue(5.0, &mut sc);
        cfg.MaxMegabytesPerView.SetValue(4096, &mut sc);
        cfg.MaxRenderThreads.SetValue(16, &mut sc);
        cfg.AllowSIMD.SetValue(false, &mut sc);
        cfg.DownscaleQuality.SetValue(6, &mut sc);
        cfg.UpscaleQuality.SetValue(5, &mut sc);
        // Drain pending signals fired by SetValue.
        sc.scheduler.abort_all_pending();

        let mut model =
            emRecNodeConfigModel::new(cfg, path.clone(), &mut sc).with_format_name("emCoreConfig");
        model.TrySave(true).unwrap();
        model.detach(&mut sc);
    }

    // Read back and verify.
    {
        let cfg2 = emCoreConfig::new(&mut sc);
        let mut model2 =
            emRecNodeConfigModel::new(cfg2, path.clone(), &mut sc).with_format_name("emCoreConfig");
        model2.TryLoad(&mut sc).unwrap();
        // Drain pending signals fired by TryRead's SetValue calls.
        sc.scheduler.abort_all_pending();
        let c = model2.GetRec();
        assert!(*c.StickMouseWhenNavigating.GetValue());
        assert!(*c.EmulateMiddleButton.GetValue());
        assert!(*c.PanFunction.GetValue());
        assert_eq!(*c.MouseZoomSpeed.GetValue(), 2.5);
        assert_eq!(*c.MouseScrollSpeed.GetValue(), 3.0);
        assert_eq!(*c.MouseWheelZoomSpeed.GetValue(), 0.5);
        assert_eq!(*c.MouseWheelZoomAcceleration.GetValue(), 1.5);
        assert_eq!(*c.KeyboardZoomSpeed.GetValue(), 3.5);
        assert_eq!(*c.KeyboardScrollSpeed.GetValue(), 0.25);
        assert_eq!(*c.KineticZoomingAndScrolling.GetValue(), 0.75);
        assert_eq!(*c.MagnetismRadius.GetValue(), 2.0);
        assert_eq!(*c.MagnetismSpeed.GetValue(), 3.0);
        assert_eq!(*c.VisitSpeed.GetValue(), 5.0);
        assert_eq!(*c.MaxMegabytesPerView.GetValue(), 4096);
        assert_eq!(*c.MaxRenderThreads.GetValue(), 16);
        assert!(!*c.AllowSIMD.GetValue());
        assert_eq!(*c.DownscaleQuality.GetValue(), 6);
        assert_eq!(*c.UpscaleQuality.GetValue(), 5);
        model2.detach(&mut sc);
    }
}

/// emDoubleRec::TryRead rejects out-of-range values with an error
/// (matches C++ emRec.cpp:552-560). Verify that files with out-of-range
/// double fields fail to load — the previous `Record::from_rec` silently
/// clamped; the emRec reader rejects instead.
#[test]
fn out_of_range_double_fields_rejected() {
    use emcore::emRecNodeConfigModel::emRecNodeConfigModel;

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oor_double.rec");

    // Write a value exceeding MouseZoomSpeed's max of 4.0.
    std::fs::write(
        &path,
        b"#%rec:emCoreConfig%#\n\n{\n\tMouseZoomSpeed = 100.0\n}\n",
    )
    .unwrap();

    let cfg = emCoreConfig::new(&mut sc);
    let mut model =
        emRecNodeConfigModel::new(cfg, path.clone(), &mut sc).with_format_name("emCoreConfig");
    // emDoubleRec::TryRead returns Err("Number too large.") for values above max.
    assert!(
        model.TryLoad(&mut sc).is_err(),
        "out-of-range double should be rejected"
    );
    model.detach(&mut sc);
}

/// emIntRec::TryRead rejects out-of-range values with an error.
/// Verify that files with out-of-range int fields fail to load.
#[test]
fn out_of_range_int_fields_rejected() {
    use emcore::emRecNodeConfigModel::emRecNodeConfigModel;

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oor_int.rec");

    // Write a value below MaxMegabytesPerView's min of 8.
    std::fs::write(
        &path,
        b"#%rec:emCoreConfig%#\n\n{\n\tMaxMegabytesPerView = 1\n}\n",
    )
    .unwrap();

    let cfg = emCoreConfig::new(&mut sc);
    let mut model =
        emRecNodeConfigModel::new(cfg, path.clone(), &mut sc).with_format_name("emCoreConfig");
    assert!(
        model.TryLoad(&mut sc).is_err(),
        "out-of-range int should be rejected"
    );
    model.detach(&mut sc);
}

#[test]
fn missing_fields_use_defaults() {
    use emcore::emRecNodeConfigModel::emRecNodeConfigModel;

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing_fields.rec");

    // Empty struct body — all fields missing.
    std::fs::write(&path, b"#%rec:emCoreConfig%#\n\n{\n}\n").unwrap();

    let cfg = emCoreConfig::new(&mut sc);
    let mut model =
        emRecNodeConfigModel::new(cfg, path.clone(), &mut sc).with_format_name("emCoreConfig");
    model.TryLoad(&mut sc).unwrap();
    let c = model.GetRec();
    // All fields must remain at their C++ defaults.
    assert!(!*c.StickMouseWhenNavigating.GetValue());
    assert_eq!(*c.VisitSpeed.GetValue(), 1.0);
    assert_eq!(*c.MaxMegabytesPerView.GetValue(), 2048);
    assert_eq!(*c.DownscaleQuality.GetValue(), 3);
    assert_eq!(*c.UpscaleQuality.GetValue(), 2);
    model.detach(&mut sc);
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
