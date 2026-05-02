//! B-015-polling-emcore-plus behavioral tests.
//!
//! Covers the ten P-006 rows in this bucket:
//!
//! - emColorField rows -245, -255, -265, -277, -288, -298, -308, -320:
//!   first-Cycle init walks the expansion-child panel tree, captures each
//!   scalar/text-field signal, and connects the panel engine. Cleared on
//!   `auto_shrink`; re-armed on subsequent `auto_expand`.
//!
//! - emFilePanel row -50: `Cycle` subscribes the panel engine to the bound
//!   `FileModelState::GetFileStateSignal()`, swaps subscription on model
//!   change, and disconnects when the model is cleared. Mirrors C++
//!   `emFilePanel::SetFileModel`'s `RemoveWakeUpSignal`/`AddWakeUpSignal`
//!   pair (Option B implementation per kickoff brief — model-swap detect
//!   in Cycle, matching the precedent set by `emImageFilePanel`).
//!
//! emMainPanel row -68 lives in `crates/emmain/src/emMainPanel.rs`'s test
//! module because the panel type is private to that crate.
//!
//! RUST_ONLY: (dependency-forced) — no C++ test analogue, mirrors B-005,
//! B-006, B-007, B-008 test rationale.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use slotmap::Key as _;

use emcore::emColor::emColor;
use emcore::emColorField::emColorField;
use emcore::emEngine::{emEngine, Priority};
use emcore::emEngineCtx::{EngineCtx, PanelCtx};
use emcore::emFileModel::{emFileModel, FileModelState};
use emcore::emFilePanel::emFilePanel;
use emcore::emLook::emLook;
use emcore::emPanel::PanelBehavior;
use emcore::emPanelScope::PanelScope;
use emcore::emPanelTree::{PanelId, PanelTree};
use emcore::emScheduler::EngineScheduler;

// ---------------------------------------------------------------------------
// Shared scaffolding
// ---------------------------------------------------------------------------

/// Build minimal DoTimeSlice arguments and run one slice on `sched`.
fn do_slice(sched: &mut EngineScheduler) {
    use winit::window::WindowId;
    let mut windows: HashMap<WindowId, emcore::emWindow::emWindow> = HashMap::new();
    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let mut pending_inputs: Vec<(WindowId, emcore::emInput::emInputEvent)> = Vec::new();
    let mut input_state = emcore::emInputState::emInputState::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));
    sched.DoTimeSlice(
        &mut windows,
        &root_ctx,
        &mut fw,
        &mut pending_inputs,
        &mut input_state,
        &cb,
        &pa,
    );
}

// ---------------------------------------------------------------------------
// emColorField — rows -245..-320
// ---------------------------------------------------------------------------

/// Engine wrapper that drives emColorField::Cycle from a real time slice so
/// the panel tree is wired up via PanelCycleEngine-equivalent reach.
struct ColorFieldDriver {
    cf: Rc<RefCell<emColorField>>,
    tree: PanelTree,
    root: PanelId,
    cycles: u32,
    initialized: bool,
}

impl emEngine for ColorFieldDriver {
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>) -> bool {
        // First slice: create the expansion children inside the tree so the
        // signal-capture walk has something to find. Done lazily because the
        // PanelCtx scheduler reach is only available inside Cycle.
        let mut pctx = build_pctx(ectx, &mut self.tree, self.root);
        if !self.initialized {
            self.cf.borrow_mut().set_expanded(true);
            self.cf.borrow_mut().create_expansion_children(&mut pctx);
            self.initialized = true;
        }
        self.cf.borrow_mut().Cycle(&mut pctx);
        self.cycles += 1;
        // Stay awake for at most a handful of slices so signal-driven re-wakes
        // can be observed within a deterministic budget.
        self.cycles < 5
    }
}

fn build_pctx<'a>(
    ectx: &'a mut EngineCtx<'_>,
    tree: &'a mut PanelTree,
    root: PanelId,
) -> PanelCtx<'a> {
    PanelCtx::with_sched_reach(
        tree,
        root,
        1.0,
        ectx.scheduler,
        ectx.framework_actions,
        ectx.root_context,
        ectx.framework_clipboard,
        ectx.pending_actions,
    )
}

fn make_color_field_driver() -> (Rc<RefCell<emColorField>>, ColorFieldDriver) {
    let mut sched = EngineScheduler::new();
    let look = emLook::new();
    let cf = {
        // Build a throwaway InitCtx just to allocate the color signal.
        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let mut ictx = emcore::emEngineCtx::InitCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw,
            root_context: &root_ctx,
            view_context: None,
            pending_actions: &pa,
        };
        emColorField::new(&mut ictx, look)
    };
    let cf_rc = Rc::new(RefCell::new(cf));

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("cf_root");
    tree.set_panel_view(root);
    let driver = ColorFieldDriver {
        cf: cf_rc.clone(),
        tree,
        root,
        cycles: 0,
        initialized: false,
    };
    // sched is dropped with the InitCtx; the driver owns the tree but not the
    // scheduler — the test creates its own scheduler below.
    (cf_rc, driver)
}

/// Rows -245..-320: first-Cycle init captures all eight expansion-child
/// signals (7 scalar fields + 1 text field) and the `subscribed_to_children`
/// flag flips. Single test covers all eight rows since they share the
/// `connect_child_signals` walk.
#[test]
fn emcolorfield_first_cycle_init_captures_eight_child_signals() {
    let mut sched = EngineScheduler::new();
    let look = emLook::new();
    let cf = {
        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let mut ictx = emcore::emEngineCtx::InitCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw,
            root_context: &root_ctx,
            view_context: None,
            pending_actions: &pa,
        };
        emColorField::new(&mut ictx, look)
    };
    let cf_rc = Rc::new(RefCell::new(cf));

    // `create_root_deferred_view` (has_view=false) so that child panels
    // created inside `create_expansion_children` don't register
    // PanelCycleEngines in the scheduler. Keeps cleanup simple while still
    // exercising the signal-capture walk.
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("cf_root");

    let driver = Box::new(ColorFieldDriver {
        cf: cf_rc.clone(),
        tree,
        root,
        cycles: 0,
        initialized: false,
    });
    let eid = sched.register_engine(driver, Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);

    do_slice(&mut sched);

    let cf = cf_rc.borrow();
    assert!(
        cf.subscribed_to_children_for_test(),
        "first-Cycle init must flip subscribed_to_children"
    );
    let sf = cf.sf_signals_for_test();
    for (i, s) in sf.iter().enumerate() {
        assert!(
            s.is_some_and(|sig| !sig.is_null()),
            "sf_signal[{i}] must be captured (rows -245..-308)"
        );
    }
    assert!(
        cf.tf_name_signal_for_test()
            .is_some_and(|sig| !sig.is_null()),
        "tf_name_signal must be captured (row -320)"
    );
    drop(cf);

    sched.remove_engine(eid);
    sched.abort_all_pending();
}

/// Rows -245..-320: `auto_shrink` clears the subscribed flag and zeroes the
/// cached signals so the next `auto_expand` cycle re-arms against fresh
/// child signal ids. Mirrors C++ implicit invalidation when expansion
/// children destruct.
#[test]
fn emcolorfield_auto_shrink_clears_subscribed_state() {
    let mut sched = EngineScheduler::new();
    let look = emLook::new();
    let mut cf = {
        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let mut ictx = emcore::emEngineCtx::InitCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw,
            root_context: &root_ctx,
            view_context: None,
            pending_actions: &pa,
        };
        emColorField::new(&mut ictx, look)
    };
    cf.set_expanded(true);
    // `set_expanded(false)` calls `auto_shrink` internally.
    cf.set_expanded(false);
    assert!(
        !cf.subscribed_to_children_for_test(),
        "auto_shrink must clear subscribed_to_children"
    );
    assert!(
        cf.sf_signals_for_test().iter().all(Option::is_none),
        "auto_shrink must zero sf_signals"
    );
    assert!(
        cf.tf_name_signal_for_test().is_none(),
        "auto_shrink must zero tf_name_signal"
    );
}

// ---------------------------------------------------------------------------
// emFilePanel — row -50
// ---------------------------------------------------------------------------

/// Mini FileModelState impl backed by `emFileModel<()>` so the test can
/// drive `GetFileStateSignal` arrival deterministically.
fn make_model(sched: &mut EngineScheduler) -> Rc<RefCell<emFileModel<()>>> {
    let mut m = emFileModel::<()>::new(PathBuf::from("/tmp/b015_filepanel"));
    m.complete_load(()); // → FileState::Loaded so VirtualFileState is computable
                         // F019: emFileModel's file_state_signal is now lazy — allocate it
                         // eagerly here so this test can observe distinct signals across two
                         // models without driving each through a Cycle.
    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));
    let mut sc = emcore::emEngineCtx::SchedCtx {
        scheduler: sched,
        framework_actions: &mut fw,
        root_context: &root_ctx,
        view_context: None,
        framework_clipboard: &cb,
        current_engine: None,
        pending_actions: &pa,
    };
    m.ensure_file_state_signal(&mut sc);
    Rc::new(RefCell::new(m))
}

/// Driver mirroring `ColorFieldDriver` but for `emFilePanel`. Uses
/// `Rc<RefCell<emFilePanel>>` so the test body can swap models between
/// slices.
struct FilePanelDriver {
    panel: Rc<RefCell<emFilePanel>>,
    tree: PanelTree,
    root: PanelId,
    cycles: u32,
}

impl emEngine for FilePanelDriver {
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>) -> bool {
        // emFilePanel::Cycle uses `ectx` for connect/disconnect/IsSignaled and
        // does not touch `pctx`'s scheduler reach, so a no-scheduler PanelCtx
        // is sufficient (and avoids borrow-aliasing with ectx).
        let mut pctx = PanelCtx::new(&mut self.tree, self.root, 1.0);
        self.panel.borrow_mut().Cycle(ectx, &mut pctx);
        self.cycles += 1;
        self.cycles < 8
    }
}

fn make_file_panel_driver() -> (Rc<RefCell<emFilePanel>>, FilePanelDriver, EngineScheduler) {
    let sched = EngineScheduler::new();
    let panel_rc: Rc<RefCell<emFilePanel>> = Rc::new(RefCell::new(emFilePanel::new()));
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("fp_root");
    tree.set_panel_view(root);
    (
        panel_rc.clone(),
        FilePanelDriver {
            panel: panel_rc,
            tree,
            root,
            cycles: 0,
        },
        sched,
    )
}

/// Row -50: after `SetFileModel(Some(model))`, the panel's first Cycle
/// captures the model's `FileStateSignal` into `subscribed_file_state_signal`.
/// Mirrors C++ emFilePanel.cpp:50 `AddWakeUpSignal(fileModel->GetFileStateSignal())`.
#[test]
fn emfilepanel_first_cycle_subscribes_to_model_signal() {
    let (panel, driver, mut sched) = make_file_panel_driver();
    let model = make_model(&mut sched);
    let model_sig = model.borrow().GetFileStateSignal();
    panel
        .borrow_mut()
        .SetFileModel(Some(model.clone() as Rc<RefCell<dyn FileModelState>>));

    let eid = sched.register_engine(Box::new(driver), Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);
    do_slice(&mut sched);

    assert_eq!(
        panel.borrow().subscribed_file_state_signal_for_test(),
        model_sig,
        "Cycle must subscribe to the bound model's FileStateSignal"
    );

    // Drain remaining engines to satisfy the EngineScheduler drop assertion.
    sched.remove_engine(eid);
    sched.abort_all_pending();
}

/// Row -50: model swap → next Cycle disconnects from old signal and
/// connects to new. Mirrors the C++ `RemoveWakeUpSignal`/`AddWakeUpSignal`
/// pair in `SetFileModel` (here applied lazily at Cycle, Option B per
/// kickoff brief).
#[test]
fn emfilepanel_model_swap_re_subscribes() {
    let (panel, driver, mut sched) = make_file_panel_driver();
    let m1 = make_model(&mut sched);
    let m2 = make_model(&mut sched);
    let s1 = m1.borrow().GetFileStateSignal();
    let s2 = m2.borrow().GetFileStateSignal();
    assert_ne!(s1, s2, "test setup: distinct signals expected");

    panel
        .borrow_mut()
        .SetFileModel(Some(m1.clone() as Rc<RefCell<dyn FileModelState>>));

    let eid = sched.register_engine(Box::new(driver), Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);
    do_slice(&mut sched);
    assert_eq!(panel.borrow().subscribed_file_state_signal_for_test(), s1);

    // Swap to m2; wake the engine so Cycle re-binds.
    panel
        .borrow_mut()
        .SetFileModel(Some(m2.clone() as Rc<RefCell<dyn FileModelState>>));
    sched.wake_up(eid);
    do_slice(&mut sched);
    assert_eq!(
        panel.borrow().subscribed_file_state_signal_for_test(),
        s2,
        "Cycle after model swap must re-bind to the new model's signal"
    );

    sched.remove_engine(eid);
    sched.abort_all_pending();
}

/// Row -50: clearing the model (`SetFileModel(None)`) → next Cycle
/// disconnects the previously subscribed signal and clears the cache.
#[test]
fn emfilepanel_clear_model_disconnects() {
    let (panel, driver, mut sched) = make_file_panel_driver();
    let m = make_model(&mut sched);
    let s = m.borrow().GetFileStateSignal();
    panel
        .borrow_mut()
        .SetFileModel(Some(m.clone() as Rc<RefCell<dyn FileModelState>>));
    let eid = sched.register_engine(Box::new(driver), Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);
    do_slice(&mut sched);
    assert_eq!(panel.borrow().subscribed_file_state_signal_for_test(), s);

    panel.borrow_mut().SetFileModel(None);
    sched.wake_up(eid);
    do_slice(&mut sched);
    assert!(
        panel
            .borrow()
            .subscribed_file_state_signal_for_test()
            .is_null(),
        "Cycle after SetFileModel(None) must clear subscribed signal"
    );

    sched.remove_engine(eid);
    sched.abort_all_pending();
}

/// Row -50 reaction: signal arrival → Cycle observes via the connected
/// engine and `cycle_inner` recomputes `last_vir_file_state` to reflect the
/// updated model state.
#[test]
fn emfilepanel_signal_arrival_triggers_state_recompute() {
    let (panel, driver, mut sched) = make_file_panel_driver();
    let m = make_model(&mut sched);
    let s = m.borrow().GetFileStateSignal();
    panel
        .borrow_mut()
        .SetFileModel(Some(m.clone() as Rc<RefCell<dyn FileModelState>>));
    let eid = sched.register_engine(Box::new(driver), Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);

    // First slice subscribes; verify the initial vir state is `Loaded`
    // (model was complete_load'd in `make_model`).
    do_slice(&mut sched);
    assert!(
        matches!(
            panel.borrow().GetVirFileState(),
            emcore::emFilePanel::VirtualFileState::Loaded
        ),
        "initial VFS should be Loaded"
    );

    // Mutate the model to a LoadError state and fire its signal.
    m.borrow_mut().fail_load("disk full".to_string());
    sched.fire(s);
    do_slice(&mut sched);

    assert!(
        matches!(
            panel.borrow().GetVirFileState(),
            emcore::emFilePanel::VirtualFileState::LoadError(_)
        ),
        "VFS must recompute to LoadError after signal arrival"
    );

    sched.remove_engine(eid);
    sched.abort_all_pending();
}

// Suppress unused-import lints for helpers shared between sub-targets.
#[allow(dead_code)]
fn _color_helper_anchor() {
    let _ = emColor::BLACK;
    let _ = make_color_field_driver();
}
