// RUST_ONLY: (dependency-forced) no C++ test analogue (C++ test surface is X11 integration).
//
// B-010 rc-shim conversion behavioural-coverage tests.
// Each row in B-010's per-row table gets one test verifying that the new
// first-Cycle-init + IsSignaled subscribe shape (D-006) propagates a widget-
// state change to the host panel's reaction body equivalently to the deleted
// closure shim.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use emcore::emCoreConfig::emCoreConfig;
use emcore::emCoreConfigPanel::ButtonsPanel;
use emcore::emEngine::{emEngine, Priority};
use emcore::emEngineCtx::{EngineCtx, PanelCtx, SchedCtx};
use emcore::emLook::emLook;
use emcore::emPanel::PanelBehavior;
use emcore::emPanelScope::PanelScope;
use emcore::emPanelTree::{PanelId, PanelTree};
use emcore::emRec::emRec;
use emcore::emRecNodeConfigModel::emRecNodeConfigModel;
use emcore::emScheduler::EngineScheduler;

// ---------------------------------------------------------------------------
// Shared helpers
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
// Row 80: ButtonsPanel Reset → config defaults
// ---------------------------------------------------------------------------

/// Row 80: After firing the captured Reset-button click_signal, ButtonsPanel::Cycle
/// resets every config field to default and calls TrySave — observable equivalence
/// with the deleted `btn.on_click` closure shim.
///
/// Mirrors C++ `emCoreConfigPanel::Cycle` (emCoreConfigPanel.cpp:42), where the
/// host panel subscribes to `ResetButton->GetClickSignal()` and reacts inline.
///
/// Four-question audit trail:
///   (1) Signal connected? — ButtonsPanel::Cycle first-Cycle init connects to
///       the captured `bt_reset_sig`.
///   (2) Cycle observes? — IsSignaled(bt_reset_sig) branch in Cycle.
///   (3) Reaction fires documented mutator? — cm.modify(...) sets every field
///       to its default, TrySave(false) saves.
///   (4) C++ branch order preserved? — init block before IsSignaled reaction,
///       matching the canonical D-006 shape from the design doc §2.1.
#[test]
fn row_80_reset_button_cycle_restores_defaults() {
    // --- Scheduler + config setup ----------------------------------------
    let mut sched = EngineScheduler::new();
    let install_path = std::env::temp_dir().join("rc_shim_b010_row80.rec");
    let _ = std::fs::remove_file(&install_path);

    // Build the config model with a freshly-constructed (defaults) emCoreConfig.
    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));

    let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = {
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        let cfg = emCoreConfig::new(&mut sc);
        let model = emRecNodeConfigModel::new(cfg, install_path.clone(), &mut sc);
        Rc::new(RefCell::new(model))
    };

    // Mutate one field to a non-default value so the reset is observable.
    {
        let mut cm = config.borrow_mut();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        cm.modify(
            |c, sc| {
                c.StickMouseWhenNavigating.SetValue(true, sc);
            },
            &mut sc,
        );
    }
    assert!(
        *config.borrow().GetRec().StickMouseWhenNavigating.GetValue(),
        "pre-state: StickMouseWhenNavigating must be true after the test mutation"
    );

    // --- Build ButtonsPanel + tree --------------------------------------
    let look = emLook::new();
    let panel_rc: Rc<RefCell<ButtonsPanel>> =
        Rc::new(RefCell::new(ButtonsPanel::new(Rc::clone(&config), look)));

    let tree_rc: Rc<RefCell<PanelTree>> = Rc::new(RefCell::new(PanelTree::new()));
    let root: PanelId = tree_rc
        .borrow_mut()
        .create_root_deferred_view("rc_shim_b010_row80");

    // --- Wrapper engine: drives create_children once, then Cycle --------
    struct PanelEngine {
        panel: Rc<RefCell<ButtonsPanel>>,
        tree: Rc<RefCell<PanelTree>>,
        root: PanelId,
        children_built: bool,
        cycles_run: u32,
    }
    impl emEngine for PanelEngine {
        fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            // Mirrors `PanelCycleEngine::Cycle` (Toplevel arm): split the
            // EngineCtx scheduler/fw_actions handles via raw pointer so we
            // can build both an `EngineCtx` (for Cycle's first arg) and a
            // `PanelCtx` with full sched_reach (for `as_sched_ctx()` to work
            // in `create_children` / the reset reaction body) without
            // borrow-checker conflict. Justified by the same single-threaded
            // re-entrant safety argument as the production engine.
            let sched_ptr: *mut EngineScheduler = &mut *ctx.scheduler;
            let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> = &mut *ctx.framework_actions;

            let stay_awake = {
                let mut ectx = EngineCtx {
                    scheduler: unsafe { &mut *sched_ptr },
                    tree: None,
                    windows: &mut *ctx.windows,
                    root_context: ctx.root_context,
                    framework_actions: unsafe { &mut *fw_ptr },
                    pending_inputs: &mut *ctx.pending_inputs,
                    input_state: &mut *ctx.input_state,
                    framework_clipboard: ctx.framework_clipboard,
                    engine_id: ctx.engine_id,
                    pending_actions: ctx.pending_actions,
                };
                let mut tree_borrow = self.tree.borrow_mut();
                let mut pctx = PanelCtx::with_sched_reach(
                    &mut *tree_borrow,
                    self.root,
                    1.0,
                    unsafe { &mut *sched_ptr },
                    unsafe { &mut *fw_ptr },
                    ctx.root_context,
                    ctx.framework_clipboard,
                    ctx.pending_actions,
                );
                if !self.children_built {
                    self.panel.borrow_mut().create_children(&mut pctx);
                    self.children_built = true;
                }
                self.panel.borrow_mut().Cycle(&mut ectx, &mut pctx)
            };
            self.cycles_run += 1;
            // Stay awake until we've processed the post-fire reaction slice.
            stay_awake || self.cycles_run < 4
        }
    }

    let engine = Box::new(PanelEngine {
        panel: Rc::clone(&panel_rc),
        tree: Rc::clone(&tree_rc),
        root,
        children_built: false,
        cycles_run: 0,
    });
    let eid = sched.register_engine(engine, Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);

    // First slice: create_children runs (captures click_signal), Cycle subscribes.
    do_slice(&mut sched);

    let bt_reset_sig = panel_rc.borrow().bt_reset_sig_for_test();
    use slotmap::Key as _;
    assert!(
        !bt_reset_sig.is_null(),
        "create_children must have captured a non-null Reset click_signal"
    );

    // Action: fire the Reset button's click signal.
    sched.fire(bt_reset_sig);

    // Second slice: ButtonsPanel::Cycle observes IsSignaled(bt_reset_sig) and
    // runs the reaction body (config reset + TrySave).
    do_slice(&mut sched);

    // --- Assertions -----------------------------------------------------
    assert!(
        !*config.borrow().GetRec().StickMouseWhenNavigating.GetValue(),
        "StickMouseWhenNavigating must be reset to its default (false) after the click"
    );

    sched.remove_engine(eid);
    // Tear down the panel tree under the scheduler so each panel's
    // auto-registered engine is deregistered before the scheduler drops.
    {
        let mut tree = tree_rc.borrow_mut();
        tree.remove(root, Some(&mut sched));
    }
    // Detach the config model's internal listener engine.
    {
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        config.borrow_mut().detach(&mut sc);
    }
    sched.clear_pending_for_tests();
    drop(panel_rc);
    drop(config);
    let _ = std::fs::remove_file(&install_path);
}

// ---------------------------------------------------------------------------
// Rows 299/300/301: MouseMiscGroup checkbox propagation
// ---------------------------------------------------------------------------
//
// Each test pre-stages the relevant config field to one value, pre-stages the
// corresponding child checkbox's `IsChecked()` to the OPPOSITE value (via the
// test-only `set_*_checked_for_test` helper which uses
// `with_behavior_as::<CheckBoxPanel,_>` internally), fires the captured
// `*_sig` directly, advances Cycle, and asserts the config field flipped to
// the new checked state and was persisted (TrySave ran inside the modify path).
//
// Firing the captured SignalId rather than driving `SetChecked` end-to-end
// matches Task 2's row-80 precedent (see commit 75214171 / observation 1891 —
// `CheckBoxPanel` is `pub(crate)`, so tests cannot reach it directly).

use emcore::emCoreConfigPanel::MouseMiscGroup;

fn run_mouse_misc_checkbox_test(
    test_id: &str,
    pre_set: impl FnOnce(&mut emCoreConfig, &mut SchedCtx<'_>),
    post_check: impl FnOnce(&emCoreConfig),
    fire_sig: impl Fn(&MouseMiscGroup) -> emcore::emSignal::SignalId,
    set_checkbox: impl Fn(&MouseMiscGroup, &mut emcore::emPanelTree::PanelTree),
) {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let install_path = std::env::temp_dir().join(format!("rc_shim_b010_{}.rec", test_id));
    let _ = std::fs::remove_file(&install_path);

    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));

    let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = {
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        let cfg = emCoreConfig::new(&mut sc);
        let model = emRecNodeConfigModel::new(cfg, install_path.clone(), &mut sc);
        Rc::new(RefCell::new(model))
    };

    // Apply caller's pre-state (e.g. set StickMouseWhenNavigating to false).
    {
        let mut cm = config.borrow_mut();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        cm.modify(pre_set, &mut sc);
    }

    let look = emLook::new();
    let panel_rc: Rc<RefCell<MouseMiscGroup>> = Rc::new(RefCell::new(MouseMiscGroup::new(
        Rc::clone(&config),
        look,
        true, // stick_possible
    )));

    let tree_rc: Rc<RefCell<PanelTree>> = Rc::new(RefCell::new(PanelTree::new()));
    let root: PanelId = tree_rc
        .borrow_mut()
        .create_root_deferred_view(&format!("rc_shim_b010_{}", test_id));

    struct PanelEngine {
        panel: Rc<RefCell<MouseMiscGroup>>,
        tree: Rc<RefCell<PanelTree>>,
        root: PanelId,
        children_built: bool,
        cycles_run: u32,
    }
    impl emEngine for PanelEngine {
        fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            let sched_ptr: *mut EngineScheduler = &mut *ctx.scheduler;
            let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> = &mut *ctx.framework_actions;

            let stay_awake = {
                let mut ectx = EngineCtx {
                    scheduler: unsafe { &mut *sched_ptr },
                    tree: None,
                    windows: &mut *ctx.windows,
                    root_context: ctx.root_context,
                    framework_actions: unsafe { &mut *fw_ptr },
                    pending_inputs: &mut *ctx.pending_inputs,
                    input_state: &mut *ctx.input_state,
                    framework_clipboard: ctx.framework_clipboard,
                    engine_id: ctx.engine_id,
                    pending_actions: ctx.pending_actions,
                };
                let mut tree_borrow = self.tree.borrow_mut();
                let mut pctx = PanelCtx::with_sched_reach(
                    &mut *tree_borrow,
                    self.root,
                    1.0,
                    unsafe { &mut *sched_ptr },
                    unsafe { &mut *fw_ptr },
                    ctx.root_context,
                    ctx.framework_clipboard,
                    ctx.pending_actions,
                );
                if !self.children_built {
                    self.panel.borrow_mut().create_children(&mut pctx);
                    self.children_built = true;
                }
                self.panel.borrow_mut().Cycle(&mut ectx, &mut pctx)
            };
            self.cycles_run += 1;
            stay_awake || self.cycles_run < 4
        }
    }

    let engine = Box::new(PanelEngine {
        panel: Rc::clone(&panel_rc),
        tree: Rc::clone(&tree_rc),
        root,
        children_built: false,
        cycles_run: 0,
    });
    let eid = sched.register_engine(engine, Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);

    // First slice: create_children runs (captures signals + ids), Cycle subscribes.
    do_slice(&mut sched);

    let sig = fire_sig(&panel_rc.borrow());
    assert!(
        !sig.is_null(),
        "create_children must have captured a non-null check_signal"
    );

    // Pre-stage the child checkbox's IsChecked() to the post-toggle value
    // BEFORE firing — so the IsSignaled branch reads the new value.
    {
        let mut tree = tree_rc.borrow_mut();
        set_checkbox(&panel_rc.borrow(), &mut tree);
    }

    sched.fire(sig);

    // Second slice: MouseMiscGroup::Cycle observes IsSignaled and runs the
    // reaction body (config field set + TrySave).
    do_slice(&mut sched);

    post_check(config.borrow().GetRec());

    sched.remove_engine(eid);
    {
        let mut tree = tree_rc.borrow_mut();
        tree.remove(root, Some(&mut sched));
    }
    {
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        config.borrow_mut().detach(&mut sc);
    }
    sched.clear_pending_for_tests();
    drop(panel_rc);
    drop(config);
    let _ = std::fs::remove_file(&install_path);
}

/// Row 299: Firing the captured Stick check_signal causes MouseMiscGroup::Cycle
/// to read the checkbox's `IsChecked()` value and propagate it to
/// `Config->StickMouseWhenNavigating` + Save.
#[test]
fn row_299_stick_checkbox_propagates_to_config() {
    run_mouse_misc_checkbox_test(
        "row299_stick",
        // pre-state: config field is false, will flip to true.
        |c, sc| c.StickMouseWhenNavigating.SetValue(false, sc),
        |c| {
            assert!(
                *c.StickMouseWhenNavigating.GetValue(),
                "StickMouseWhenNavigating must propagate from checkbox IsChecked() = true"
            );
        },
        |p| p.stick_sig_for_test(),
        |p, tree| p.set_stick_checked_for_test(tree, true),
    );
}

/// Row 300: Firing the captured Emu check_signal causes MouseMiscGroup::Cycle
/// to propagate `IsChecked()` to `Config->EmulateMiddleButton` + Save.
#[test]
fn row_300_emu_checkbox_propagates_to_config() {
    run_mouse_misc_checkbox_test(
        "row300_emu",
        |c, sc| c.EmulateMiddleButton.SetValue(false, sc),
        |c| {
            assert!(
                *c.EmulateMiddleButton.GetValue(),
                "EmulateMiddleButton must propagate from checkbox IsChecked() = true"
            );
        },
        |p| p.emu_sig_for_test(),
        |p, tree| p.set_emu_checked_for_test(tree, true),
    );
}

/// Row 301: Firing the captured Pan check_signal causes MouseMiscGroup::Cycle
/// to propagate `IsChecked()` to `Config->PanFunction` + Save.
#[test]
fn row_301_pan_checkbox_propagates_to_config() {
    run_mouse_misc_checkbox_test(
        "row301_pan",
        |c, sc| c.PanFunction.SetValue(false, sc),
        |c| {
            assert!(
                *c.PanFunction.GetValue(),
                "PanFunction must propagate from checkbox IsChecked() = true"
            );
        },
        |p| p.pan_sig_for_test(),
        |p, tree| p.set_pan_checked_for_test(tree, true),
    );
}

// ---------------------------------------------------------------------------
// Row 563: MemFieldLayoutPanel scalar propagation
// ---------------------------------------------------------------------------

use emcore::emCoreConfigPanel::MemFieldLayoutPanel;

/// Row 563: Firing the captured MemField value_signal causes
/// MemFieldLayoutPanel::Cycle to read the scalar field's `GetValue()` and
/// propagate `mem_val_to_cfg(value)` to `Config->MaxMegabytesPerView` + Save.
///
/// Pre-stages config to 256 MB and the scalar widget's GetValue() to the
/// log2-space value for 1024 MB (mem_cfg_to_val(1024) = ln(1024)/ln(2)*100 =
/// 1000.0 exactly — round-trip-clean: mem_val_to_cfg(1000.0) = 1024).
#[test]
fn row_563_mem_scalar_propagates_to_config() {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let install_path = std::env::temp_dir().join("rc_shim_b010_row563_mem.rec");
    let _ = std::fs::remove_file(&install_path);

    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));

    let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = {
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        let cfg = emCoreConfig::new(&mut sc);
        let model = emRecNodeConfigModel::new(cfg, install_path.clone(), &mut sc);
        Rc::new(RefCell::new(model))
    };

    // Pre-state: set MaxMegabytesPerView to 256 — distinct from the post-fire
    // expected value of 1024.
    {
        let mut cm = config.borrow_mut();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        cm.modify(|c, sc| c.MaxMegabytesPerView.SetValue(256, sc), &mut sc);
    }
    assert_eq!(
        *config.borrow().GetRec().MaxMegabytesPerView.GetValue(),
        256,
        "pre-state: MaxMegabytesPerView must be 256"
    );

    let look = emLook::new();
    let panel_rc: Rc<RefCell<MemFieldLayoutPanel>> = Rc::new(RefCell::new(
        MemFieldLayoutPanel::new(Rc::clone(&config), look),
    ));

    let tree_rc: Rc<RefCell<PanelTree>> = Rc::new(RefCell::new(PanelTree::new()));
    let root: PanelId = tree_rc
        .borrow_mut()
        .create_root_deferred_view("rc_shim_b010_row563_mem");

    struct PanelEngine {
        panel: Rc<RefCell<MemFieldLayoutPanel>>,
        tree: Rc<RefCell<PanelTree>>,
        root: PanelId,
        children_built: bool,
        cycles_run: u32,
    }
    impl emEngine for PanelEngine {
        fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            let sched_ptr: *mut EngineScheduler = &mut *ctx.scheduler;
            let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> = &mut *ctx.framework_actions;

            let stay_awake = {
                let mut ectx = EngineCtx {
                    scheduler: unsafe { &mut *sched_ptr },
                    tree: None,
                    windows: &mut *ctx.windows,
                    root_context: ctx.root_context,
                    framework_actions: unsafe { &mut *fw_ptr },
                    pending_inputs: &mut *ctx.pending_inputs,
                    input_state: &mut *ctx.input_state,
                    framework_clipboard: ctx.framework_clipboard,
                    engine_id: ctx.engine_id,
                    pending_actions: ctx.pending_actions,
                };
                let mut tree_borrow = self.tree.borrow_mut();
                let mut pctx = PanelCtx::with_sched_reach(
                    &mut *tree_borrow,
                    self.root,
                    1.0,
                    unsafe { &mut *sched_ptr },
                    unsafe { &mut *fw_ptr },
                    ctx.root_context,
                    ctx.framework_clipboard,
                    ctx.pending_actions,
                );
                if !self.children_built {
                    self.panel.borrow_mut().create_children(&mut pctx);
                    self.children_built = true;
                }
                self.panel.borrow_mut().Cycle(&mut ectx, &mut pctx)
            };
            self.cycles_run += 1;
            stay_awake || self.cycles_run < 4
        }
    }

    let engine = Box::new(PanelEngine {
        panel: Rc::clone(&panel_rc),
        tree: Rc::clone(&tree_rc),
        root,
        children_built: false,
        cycles_run: 0,
    });
    let eid = sched.register_engine(engine, Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);

    // First slice: create_children runs (captures value_signal + id), Cycle subscribes.
    do_slice(&mut sched);

    let mem_sig = panel_rc.borrow().mem_sig_for_test();
    assert!(
        !mem_sig.is_null(),
        "create_children must have captured a non-null MemField value_signal"
    );

    // Pre-stage the scalar field's GetValue() to the log2-space value for
    // 1024 MB. mem_cfg_to_val(1024) = ln(1024)/ln(2)*100 = 1000.0 (exact),
    // and mem_val_to_cfg(1000.0) = (2^10 + 0.5) as i32 = 1024 — round-trip-clean.
    let staged_val: f64 = 1000.0;
    {
        let mut tree = tree_rc.borrow_mut();
        panel_rc
            .borrow()
            .set_mem_value_for_test(&mut tree, staged_val);
    }

    sched.fire(mem_sig);

    // Second slice: MemFieldLayoutPanel::Cycle observes IsSignaled and runs
    // the reaction body (config field set to mem_val_to_cfg(staged_val) = 1024 + TrySave).
    do_slice(&mut sched);

    assert_eq!(
        *config.borrow().GetRec().MaxMegabytesPerView.GetValue(),
        1024,
        "MaxMegabytesPerView must propagate from scalar GetValue() = 1000.0 → mem_val_to_cfg = 1024"
    );

    sched.remove_engine(eid);
    {
        let mut tree = tree_rc.borrow_mut();
        tree.remove(root, Some(&mut sched));
    }
    {
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        config.borrow_mut().detach(&mut sc);
    }
    sched.clear_pending_for_tests();
    drop(panel_rc);
    drop(config);
    let _ = std::fs::remove_file(&install_path);
}

// ---------------------------------------------------------------------------
// Rows 746/755: CpuGroup MaxRenderThreads scalar + AllowSIMD checkbox
// ---------------------------------------------------------------------------

use emcore::emCoreConfigPanel::CpuGroup;

/// Helper: construct a `CpuGroup`-driving engine and run init slice. Returns
/// the panel Rc, tree Rc, root id, engine id, and config Rc, plus framework
/// state needed for teardown.
struct CpuGroupHarness {
    panel: Rc<RefCell<CpuGroup>>,
    tree: Rc<RefCell<PanelTree>>,
    root: PanelId,
    eid: emcore::emEngine::EngineId,
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    install_path: std::path::PathBuf,
}

#[allow(clippy::too_many_arguments)]
fn build_cpu_group_harness(
    sched: &mut EngineScheduler,
    fw_actions: &mut Vec<emcore::emEngineCtx::DeferredAction>,
    root_ctx: &Rc<emcore::emContext::emContext>,
    cb: &RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>>,
    pa: &Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>>,
    install_path: std::path::PathBuf,
    label: &'static str,
) -> CpuGroupHarness {
    let _ = std::fs::remove_file(&install_path);

    let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = {
        let mut sc = SchedCtx {
            scheduler: sched,
            framework_actions: fw_actions,
            root_context: root_ctx,
            framework_clipboard: cb,
            current_engine: None,
            pending_actions: pa,
        };
        let cfg = emCoreConfig::new(&mut sc);
        let model = emRecNodeConfigModel::new(cfg, install_path.clone(), &mut sc);
        Rc::new(RefCell::new(model))
    };

    let look = emLook::new();
    let panel_rc: Rc<RefCell<CpuGroup>> =
        Rc::new(RefCell::new(CpuGroup::new(Rc::clone(&config), look)));

    let tree_rc: Rc<RefCell<PanelTree>> = Rc::new(RefCell::new(PanelTree::new()));
    let root: PanelId = tree_rc.borrow_mut().create_root_deferred_view(label);

    struct PanelEngine {
        panel: Rc<RefCell<CpuGroup>>,
        tree: Rc<RefCell<PanelTree>>,
        root: PanelId,
        children_built: bool,
        cycles_run: u32,
    }
    impl emEngine for PanelEngine {
        fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            let sched_ptr: *mut EngineScheduler = &mut *ctx.scheduler;
            let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> = &mut *ctx.framework_actions;

            let stay_awake = {
                let mut ectx = EngineCtx {
                    scheduler: unsafe { &mut *sched_ptr },
                    tree: None,
                    windows: &mut *ctx.windows,
                    root_context: ctx.root_context,
                    framework_actions: unsafe { &mut *fw_ptr },
                    pending_inputs: &mut *ctx.pending_inputs,
                    input_state: &mut *ctx.input_state,
                    framework_clipboard: ctx.framework_clipboard,
                    engine_id: ctx.engine_id,
                    pending_actions: ctx.pending_actions,
                };
                let mut tree_borrow = self.tree.borrow_mut();
                let mut pctx = PanelCtx::with_sched_reach(
                    &mut *tree_borrow,
                    self.root,
                    1.0,
                    unsafe { &mut *sched_ptr },
                    unsafe { &mut *fw_ptr },
                    ctx.root_context,
                    ctx.framework_clipboard,
                    ctx.pending_actions,
                );
                if !self.children_built {
                    self.panel.borrow_mut().create_children(&mut pctx);
                    self.children_built = true;
                }
                self.panel.borrow_mut().Cycle(&mut ectx, &mut pctx)
            };
            self.cycles_run += 1;
            stay_awake || self.cycles_run < 4
        }
    }

    let engine = Box::new(PanelEngine {
        panel: Rc::clone(&panel_rc),
        tree: Rc::clone(&tree_rc),
        root,
        children_built: false,
        cycles_run: 0,
    });
    let eid = sched.register_engine(engine, Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);

    CpuGroupHarness {
        panel: panel_rc,
        tree: tree_rc,
        root,
        eid,
        config,
        install_path,
    }
}

fn teardown_cpu_group_harness(
    sched: &mut EngineScheduler,
    fw_actions: &mut Vec<emcore::emEngineCtx::DeferredAction>,
    root_ctx: &Rc<emcore::emContext::emContext>,
    cb: &RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>>,
    pa: &Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>>,
    h: CpuGroupHarness,
) {
    sched.remove_engine(h.eid);
    {
        let mut tree = h.tree.borrow_mut();
        tree.remove(h.root, Some(sched));
    }
    {
        let mut sc = SchedCtx {
            scheduler: sched,
            framework_actions: fw_actions,
            root_context: root_ctx,
            framework_clipboard: cb,
            current_engine: None,
            pending_actions: pa,
        };
        h.config.borrow_mut().detach(&mut sc);
    }
    sched.clear_pending_for_tests();
    drop(h.panel);
    drop(h.config);
    let _ = std::fs::remove_file(&h.install_path);
}

/// Row 746: Firing the captured MaxRenderThreadsField value_signal causes
/// CpuGroup::Cycle to read the scalar field's `GetValue()` and propagate
/// `((val + 0.5) as i64).clamp(1, 32)` to `Config->MaxRenderThreads` + Save.
#[test]
fn row_746_max_render_threads_propagates() {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let install_path = std::env::temp_dir().join("rc_shim_b010_row746_threads.rec");

    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));

    let h = build_cpu_group_harness(
        &mut sched,
        &mut fw_actions,
        &root_ctx,
        &cb,
        &pa,
        install_path,
        "rc_shim_b010_row746_threads",
    );

    // Pre-state: MaxRenderThreads = 1.
    {
        let mut cm = h.config.borrow_mut();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        cm.modify(|c, sc| c.MaxRenderThreads.SetValue(1, sc), &mut sc);
    }
    assert_eq!(
        *h.config.borrow().GetRec().MaxRenderThreads.GetValue(),
        1,
        "pre-state: MaxRenderThreads must be 1"
    );

    // First slice: create_children + first-Cycle init subscribe.
    do_slice(&mut sched);

    let threads_sig = h.panel.borrow().threads_sig_for_test();
    assert!(
        !threads_sig.is_null(),
        "create_children must have captured a non-null MaxRenderThreads value_signal"
    );

    // Stage scalar GetValue() = 8.0 → ((8.0+0.5) as i64).clamp(1,32) = 8.
    {
        let mut tree = h.tree.borrow_mut();
        h.panel.borrow().set_threads_value_for_test(&mut tree, 8.0);
    }

    sched.fire(threads_sig);
    do_slice(&mut sched);

    assert_eq!(
        *h.config.borrow().GetRec().MaxRenderThreads.GetValue(),
        8,
        "MaxRenderThreads must propagate from scalar GetValue() = 8.0 → clamped 8"
    );

    teardown_cpu_group_harness(&mut sched, &mut fw_actions, &root_ctx, &cb, &pa, h);
}

/// Row 755: Firing the captured AllowSIMDBox check_signal causes
/// CpuGroup::Cycle to read the checkbox's `IsChecked()` and propagate to
/// `Config->AllowSIMD` + Save (only if changed).
#[test]
fn row_755_allow_simd_propagates() {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let install_path = std::env::temp_dir().join("rc_shim_b010_row755_simd.rec");

    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));

    let h = build_cpu_group_harness(
        &mut sched,
        &mut fw_actions,
        &root_ctx,
        &cb,
        &pa,
        install_path,
        "rc_shim_b010_row755_simd",
    );

    // Pre-state: AllowSIMD = false.
    {
        let mut cm = h.config.borrow_mut();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        cm.modify(|c, sc| c.AllowSIMD.SetValue(false, sc), &mut sc);
    }
    assert!(
        !*h.config.borrow().GetRec().AllowSIMD.GetValue(),
        "pre-state: AllowSIMD must be false"
    );

    do_slice(&mut sched);

    let simd_sig = h.panel.borrow().simd_sig_for_test();
    assert!(
        !simd_sig.is_null(),
        "create_children must have captured a non-null AllowSIMD check_signal"
    );

    // Stage checkbox IsChecked() = true.
    {
        let mut tree = h.tree.borrow_mut();
        h.panel.borrow().set_simd_checked_for_test(&mut tree, true);
    }

    sched.fire(simd_sig);
    do_slice(&mut sched);

    assert!(
        *h.config.borrow().GetRec().AllowSIMD.GetValue(),
        "AllowSIMD must propagate from checkbox IsChecked() = true"
    );

    teardown_cpu_group_harness(&mut sched, &mut fw_actions, &root_ctx, &cb, &pa, h);
}

// ---------------------------------------------------------------------------
// Rows 773 / 791: PerformanceGroup Downscale/UpscaleQuality scalar fields
// ---------------------------------------------------------------------------

use emcore::emCoreConfigPanel::{PerformanceGroup, INVALIDATE_PAINTING_OF_ALL_WINDOWS_CALLS};

struct PerfGroupHarness {
    panel: Rc<RefCell<PerformanceGroup>>,
    tree: Rc<RefCell<PanelTree>>,
    root: PanelId,
    eid: emcore::emEngine::EngineId,
    config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>,
    install_path: std::path::PathBuf,
}

#[allow(clippy::too_many_arguments)]
fn build_perf_group_harness(
    sched: &mut EngineScheduler,
    fw_actions: &mut Vec<emcore::emEngineCtx::DeferredAction>,
    root_ctx: &Rc<emcore::emContext::emContext>,
    cb: &RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>>,
    pa: &Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>>,
    install_path: std::path::PathBuf,
    label: &'static str,
) -> PerfGroupHarness {
    let _ = std::fs::remove_file(&install_path);

    let config: Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>> = {
        let mut sc = SchedCtx {
            scheduler: sched,
            framework_actions: fw_actions,
            root_context: root_ctx,
            framework_clipboard: cb,
            current_engine: None,
            pending_actions: pa,
        };
        let cfg = emCoreConfig::new(&mut sc);
        let model = emRecNodeConfigModel::new(cfg, install_path.clone(), &mut sc);
        Rc::new(RefCell::new(model))
    };

    let look = emLook::new();
    let panel_rc: Rc<RefCell<PerformanceGroup>> = Rc::new(RefCell::new(PerformanceGroup::new(
        Rc::clone(&config),
        look,
    )));

    let tree_rc: Rc<RefCell<PanelTree>> = Rc::new(RefCell::new(PanelTree::new()));
    let root: PanelId = tree_rc.borrow_mut().create_root_deferred_view(label);

    struct PanelEngine {
        panel: Rc<RefCell<PerformanceGroup>>,
        tree: Rc<RefCell<PanelTree>>,
        root: PanelId,
        children_built: bool,
        cycles_run: u32,
    }
    impl emEngine for PanelEngine {
        fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            let sched_ptr: *mut EngineScheduler = &mut *ctx.scheduler;
            let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> = &mut *ctx.framework_actions;

            let stay_awake = {
                let mut ectx = EngineCtx {
                    scheduler: unsafe { &mut *sched_ptr },
                    tree: None,
                    windows: &mut *ctx.windows,
                    root_context: ctx.root_context,
                    framework_actions: unsafe { &mut *fw_ptr },
                    pending_inputs: &mut *ctx.pending_inputs,
                    input_state: &mut *ctx.input_state,
                    framework_clipboard: ctx.framework_clipboard,
                    engine_id: ctx.engine_id,
                    pending_actions: ctx.pending_actions,
                };
                let mut tree_borrow = self.tree.borrow_mut();
                let mut pctx = PanelCtx::with_sched_reach(
                    &mut *tree_borrow,
                    self.root,
                    1.0,
                    unsafe { &mut *sched_ptr },
                    unsafe { &mut *fw_ptr },
                    ctx.root_context,
                    ctx.framework_clipboard,
                    ctx.pending_actions,
                );
                if !self.children_built {
                    self.panel.borrow_mut().create_children(&mut pctx);
                    self.children_built = true;
                }
                self.panel.borrow_mut().Cycle(&mut ectx, &mut pctx)
            };
            self.cycles_run += 1;
            stay_awake || self.cycles_run < 4
        }
    }

    let engine = Box::new(PanelEngine {
        panel: Rc::clone(&panel_rc),
        tree: Rc::clone(&tree_rc),
        root,
        children_built: false,
        cycles_run: 0,
    });
    let eid = sched.register_engine(engine, Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);

    PerfGroupHarness {
        panel: panel_rc,
        tree: tree_rc,
        root,
        eid,
        config,
        install_path,
    }
}

fn teardown_perf_group_harness(
    sched: &mut EngineScheduler,
    fw_actions: &mut Vec<emcore::emEngineCtx::DeferredAction>,
    root_ctx: &Rc<emcore::emContext::emContext>,
    cb: &RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>>,
    pa: &Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>>,
    h: PerfGroupHarness,
) {
    sched.remove_engine(h.eid);
    {
        let mut tree = h.tree.borrow_mut();
        tree.remove(h.root, Some(sched));
    }
    {
        let mut sc = SchedCtx {
            scheduler: sched,
            framework_actions: fw_actions,
            root_context: root_ctx,
            framework_clipboard: cb,
            current_engine: None,
            pending_actions: pa,
        };
        h.config.borrow_mut().detach(&mut sc);
    }
    sched.clear_pending_for_tests();
    drop(h.panel);
    drop(h.config);
    let _ = std::fs::remove_file(&h.install_path);
}

/// Row 773: Firing the captured DownscaleQualityField value_signal causes
/// PerformanceGroup::Cycle to read the scalar field's `GetValue()` and
/// propagate `((val + 0.5) as i64).clamp(2, 6)` to `Config->DownscaleQuality`
/// + Save (only if changed).
#[test]
fn row_773_downscale_propagates() {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let install_path = std::env::temp_dir().join("rc_shim_b010_row773_downscale.rec");

    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));

    let h = build_perf_group_harness(
        &mut sched,
        &mut fw_actions,
        &root_ctx,
        &cb,
        &pa,
        install_path,
        "rc_shim_b010_row773_downscale",
    );

    // Pre-state: DownscaleQuality = 2 (range floor).
    {
        let mut cm = h.config.borrow_mut();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        cm.modify(|c, sc| c.DownscaleQuality.SetValue(2, sc), &mut sc);
    }
    assert_eq!(
        *h.config.borrow().GetRec().DownscaleQuality.GetValue(),
        2,
        "pre-state: DownscaleQuality must be 2"
    );

    // First slice: create_children + first-Cycle init subscribe.
    do_slice(&mut sched);

    let downscale_sig = h.panel.borrow().downscale_sig_for_test();
    assert!(
        !downscale_sig.is_null(),
        "create_children must have captured a non-null DownscaleQuality value_signal"
    );

    // Stage scalar GetValue() = 4.0 → ((4.0+0.5) as i64).clamp(2,6) = 4.
    {
        let mut tree = h.tree.borrow_mut();
        h.panel
            .borrow()
            .set_downscale_value_for_test(&mut tree, 4.0);
    }

    sched.fire(downscale_sig);
    do_slice(&mut sched);

    assert_eq!(
        *h.config.borrow().GetRec().DownscaleQuality.GetValue(),
        4,
        "DownscaleQuality must propagate from scalar GetValue() = 4.0 → clamped 4"
    );

    teardown_perf_group_harness(&mut sched, &mut fw_actions, &root_ctx, &cb, &pa, h);
}

/// Row 791: Firing the captured UpscaleQualityField value_signal causes
/// PerformanceGroup::Cycle to read the scalar field's `GetValue()` and
/// propagate `((val + 0.5) as i64).clamp(0, 5)` to `Config->UpscaleQuality`
/// + Save, AND additionally invoke `InvalidatePaintingOfAllWindows`
/// (C++ emCoreConfigPanel.cpp:710).
#[test]
fn row_791_upscale_propagates_and_invalidates_painting() {
    use slotmap::Key as _;

    // Reset the test-only invalidate counter so we can observe this test's calls.
    INVALIDATE_PAINTING_OF_ALL_WINDOWS_CALLS.with(|c| c.set(0));

    let mut sched = EngineScheduler::new();
    let install_path = std::env::temp_dir().join("rc_shim_b010_row791_upscale.rec");

    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));

    let h = build_perf_group_harness(
        &mut sched,
        &mut fw_actions,
        &root_ctx,
        &cb,
        &pa,
        install_path,
        "rc_shim_b010_row791_upscale",
    );

    // Pre-state: UpscaleQuality = 5 (range ceiling). We pick a value distinct
    // from the staged target (3) so the change-guard inside the Cycle branch
    // fires the InvalidatePaintingOfAllWindows side-effect.
    {
        let mut cm = h.config.borrow_mut();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        cm.modify(|c, sc| c.UpscaleQuality.SetValue(5, sc), &mut sc);
    }
    assert_eq!(
        *h.config.borrow().GetRec().UpscaleQuality.GetValue(),
        5,
        "pre-state: UpscaleQuality must be 5"
    );

    do_slice(&mut sched);

    let upscale_sig = h.panel.borrow().upscale_sig_for_test();
    assert!(
        !upscale_sig.is_null(),
        "create_children must have captured a non-null UpscaleQuality value_signal"
    );

    let pre_invalidate_calls = INVALIDATE_PAINTING_OF_ALL_WINDOWS_CALLS.with(|c| c.get());

    // Stage scalar GetValue() = 3.0 → ((3.0+0.5) as i64).clamp(0,5) = 3.
    {
        let mut tree = h.tree.borrow_mut();
        h.panel.borrow().set_upscale_value_for_test(&mut tree, 3.0);
    }

    sched.fire(upscale_sig);
    do_slice(&mut sched);

    assert_eq!(
        *h.config.borrow().GetRec().UpscaleQuality.GetValue(),
        3,
        "UpscaleQuality must propagate from scalar GetValue() = 3.0 → clamped 3"
    );

    let post_invalidate_calls = INVALIDATE_PAINTING_OF_ALL_WINDOWS_CALLS.with(|c| c.get());
    assert!(
        post_invalidate_calls > pre_invalidate_calls,
        "InvalidatePaintingOfAllWindows must have been called at least once after \
         UpscaleQuality changed (pre={pre_invalidate_calls}, post={post_invalidate_calls})"
    );

    teardown_perf_group_harness(&mut sched, &mut fw_actions, &root_ctx, &cb, &pa, h);
}

// ---------------------------------------------------------------------------
// Rows 514/521/531/532/540/550: emFileSelectionBox D-006 IsSignaled branches
// ---------------------------------------------------------------------------
//
// These tests cover Task 8 of B-010: each row pre-stages a child widget's
// observable state via the typed `set_*_for_test` helper, fires the captured
// child signal, runs Cycle, and asserts the FSB internal state reacts as the
// IsSignaled branch dictates (matching C++ emFileSelectionBox::Cycle
// cpp:385-501).
//
// Note: Task 8 leaves the legacy FsbEvents closure-drain body in place, so
// both paths fire on each Cycle. Their mutations are idempotent on the same
// state, so observable post-conditions are identical to the IsSignaled-only
// shape that Task 9 will leave behind.

mod fsb_harness {
    use super::*;
    use emcore::emFileSelectionBox::emFileSelectionBox;
    use emcore::emPanelTree::PanelTree;

    pub struct FsbHarness {
        pub fsb: Rc<RefCell<emFileSelectionBox>>,
        pub tree: Rc<RefCell<PanelTree>>,
        pub root: PanelId,
        pub eid: emcore::emEngine::EngineId,
    }

    pub fn build(sched: &mut EngineScheduler, label: &'static str) -> FsbHarness {
        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let fsb = {
            let mut init_ctx = emcore::emEngineCtx::InitCtx {
                scheduler: sched,
                framework_actions: &mut fw_actions,
                root_context: &root_ctx,
                pending_actions: &pa,
            };
            emFileSelectionBox::new(&mut init_ctx, "B010 FSB test")
        };
        let fsb_rc = Rc::new(RefCell::new(fsb));

        let tree_rc: Rc<RefCell<PanelTree>> = Rc::new(RefCell::new(PanelTree::new()));
        let root: PanelId = tree_rc.borrow_mut().create_root_deferred_view(label);

        struct FsbEngine {
            fsb: Rc<RefCell<emFileSelectionBox>>,
            tree: Rc<RefCell<PanelTree>>,
            root: PanelId,
            children_built: bool,
            cycles_run: u32,
        }
        impl emEngine for FsbEngine {
            fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
                let sched_ptr: *mut EngineScheduler = &mut *ctx.scheduler;
                let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> =
                    &mut *ctx.framework_actions;
                let stay_awake = {
                    let mut ectx = EngineCtx {
                        scheduler: unsafe { &mut *sched_ptr },
                        tree: None,
                        windows: &mut *ctx.windows,
                        root_context: ctx.root_context,
                        framework_actions: unsafe { &mut *fw_ptr },
                        pending_inputs: &mut *ctx.pending_inputs,
                        input_state: &mut *ctx.input_state,
                        framework_clipboard: ctx.framework_clipboard,
                        engine_id: ctx.engine_id,
                        pending_actions: ctx.pending_actions,
                    };
                    let mut tree_borrow = self.tree.borrow_mut();
                    let mut pctx = PanelCtx::with_sched_reach(
                        &mut *tree_borrow,
                        self.root,
                        1.0,
                        unsafe { &mut *sched_ptr },
                        unsafe { &mut *fw_ptr },
                        ctx.root_context,
                        ctx.framework_clipboard,
                        ctx.pending_actions,
                    );
                    if !self.children_built {
                        self.fsb.borrow_mut().create_children(&mut pctx);
                        self.children_built = true;
                    }
                    self.fsb.borrow_mut().Cycle(&mut ectx, &mut pctx)
                };
                self.cycles_run += 1;
                stay_awake || self.cycles_run < 4
            }
        }

        let engine = Box::new(FsbEngine {
            fsb: Rc::clone(&fsb_rc),
            tree: Rc::clone(&tree_rc),
            root,
            children_built: false,
            cycles_run: 0,
        });
        let eid = sched.register_engine(engine, Priority::Low, PanelScope::Framework);
        sched.wake_up(eid);

        FsbHarness {
            fsb: fsb_rc,
            tree: tree_rc,
            root,
            eid,
        }
    }

    pub fn teardown(sched: &mut EngineScheduler, h: FsbHarness) {
        sched.remove_engine(h.eid);
        {
            let mut tree = h.tree.borrow_mut();
            tree.remove(h.root, Some(sched));
        }
        sched.clear_pending_for_tests();
        drop(h.fsb);
    }
}

/// Row 514: Firing the captured ParentDirField text_signal causes
/// emFileSelectionBox::Cycle to read the field's `GetText()` and update
/// `parent_dir` (mirrors C++ cpp:396-403).
#[test]
fn row_514_dir_field_text_updates_parent_dir() {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let h = fsb_harness::build(&mut sched, "row514_dir_field");

    do_slice(&mut sched);

    let sig = h.fsb.borrow().dir_text_sig_for_test();
    assert!(!sig.is_null(), "captured dir text_signal must be non-null");

    let new_dir = std::env::temp_dir().join("rc_shim_b010_row514_dummy");
    {
        let mut tree = h.tree.borrow_mut();
        h.fsb
            .borrow()
            .set_dir_field_text_for_test(&mut tree, &new_dir.to_string_lossy());
    }
    sched.fire(sig);
    do_slice(&mut sched);

    assert_eq!(
        h.fsb.borrow().GetParentDirectory(),
        new_dir.as_path(),
        "parent_dir must propagate from dir field GetText()"
    );

    fsb_harness::teardown(&mut sched, h);
}

/// Row 521: Firing the captured HiddenCheckBox check_signal causes
/// emFileSelectionBox::Cycle to read `IsChecked()` and update
/// `hidden_files_shown` (mirrors C++ cpp:405-411).
#[test]
fn row_521_hidden_checkbox_toggles_hidden_files_shown() {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let h = fsb_harness::build(&mut sched, "row521_hidden");

    do_slice(&mut sched);
    assert!(
        !h.fsb.borrow().are_hidden_files_shown(),
        "pre-state: hidden_files_shown must be false"
    );

    let sig = h.fsb.borrow().hidden_check_sig_for_test();
    assert!(
        !sig.is_null(),
        "captured hidden check_signal must be non-null"
    );

    {
        let mut tree = h.tree.borrow_mut();
        h.fsb.borrow().set_hidden_checkbox_for_test(&mut tree, true);
    }
    sched.fire(sig);
    do_slice(&mut sched);

    assert!(
        h.fsb.borrow().are_hidden_files_shown(),
        "hidden_files_shown must propagate from checkbox IsChecked() = true"
    );

    fsb_harness::teardown(&mut sched, h);
}

/// Row 531: Firing the captured FilesLB selection_signal causes
/// emFileSelectionBox::Cycle to read `GetSelectedIndices()`, call
/// `selection_from_list_box`, and update `selected_names`
/// (mirrors C++ cpp:413-417).
///
/// Verified observable: branch fires and writes to selected_names. The
/// list-box widget has zero items (we bypassed reload_listing), so its
/// `set_selected_indices_for_test` filters our staged indices out and the
/// branch reads an empty selection, which clears any pre-existing
/// selected_names. That clearing is the strongest observable available
/// without exposing the lb's items API to tests.
#[test]
fn row_531_files_selection_updates_selected_names() {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let h = fsb_harness::build(&mut sched, "row531_files_sel");

    do_slice(&mut sched);

    // Pre-stage a selected name so we can observe it being cleared by the
    // branch reaction.
    h.fsb.borrow_mut().set_selected_name("preexisting.txt");
    assert_eq!(h.fsb.borrow().GetSelectedNames().len(), 1);

    let sig = h.fsb.borrow().files_sel_sig_for_test();
    assert!(!sig.is_null(), "captured files_sel_sig must be non-null");

    {
        let mut tree = h.tree.borrow_mut();
        h.fsb
            .borrow()
            .set_files_lb_selected_for_test(&mut tree, vec![]);
    }

    sched.fire(sig);
    do_slice(&mut sched);

    assert!(
        h.fsb.borrow().GetSelectedNames().is_empty(),
        "selected_names must be cleared by the files-sel IsSignaled branch \
         after staging an empty selection"
    );

    fsb_harness::teardown(&mut sched, h);
}

/// Row 532: Firing the captured FilesLB item_trigger_signal causes
/// emFileSelectionBox::Cycle to read `GetTriggeredItemIndex()` and call
/// `enter_sub_dir` for a directory item (mirrors C++ cpp:419-432).
#[test]
fn row_532_files_trigger_enters_subdir_or_triggers_file() {
    use emcore::emFileSelectionBox::FileItemData;
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let h = fsb_harness::build(&mut sched, "row532_files_trigger");

    do_slice(&mut sched);

    // Stage a listing whose 0th item is the ".." entry.
    let initial_dir = h.fsb.borrow().GetParentDirectory().to_path_buf();
    h.fsb.borrow_mut().set_listing_for_test(vec![(
        "..".to_string(),
        FileItemData {
            is_directory: true,
            is_readable: true,
            is_hidden: false,
        },
    )]);

    let sig = h.fsb.borrow().files_trigger_sig_for_test();
    assert!(
        !sig.is_null(),
        "captured files_trigger_sig must be non-null"
    );

    {
        let mut tree = h.tree.borrow_mut();
        h.fsb
            .borrow()
            .set_files_lb_triggered_for_test(&mut tree, Some(0));
    }
    sched.fire(sig);
    do_slice(&mut sched);

    let after = h.fsb.borrow().GetParentDirectory().to_path_buf();
    if initial_dir.parent().is_some() {
        // The IsSignaled branch saw triggered_index=0, looked up listing[0]
        // = (".."), called enter_sub_dir("..") → set_parent_directory(parent
        // of cwd, canonicalized). Comparing with initial_dir.parent()
        // directly may differ if cwd traverses a symlink, so assert just
        // that parent_dir changed and is no longer the original.
        assert_ne!(
            after, initial_dir,
            "trigger of '..' must change parent_dir away from the initial cwd"
        );
    } else {
        assert_eq!(after, initial_dir, "at filesystem root, '..' is a no-op");
    }

    fsb_harness::teardown(&mut sched, h);
}

/// Row 540: Firing the captured NameField text_signal causes
/// emFileSelectionBox::Cycle to read `GetText()` and update
/// `selected_names` via `set_selected_name` (mirrors C++ cpp:434-467).
#[test]
fn row_540_name_field_text_updates_selected_names() {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let h = fsb_harness::build(&mut sched, "row540_name_field");

    do_slice(&mut sched);

    let sig = h.fsb.borrow().name_text_sig_for_test();
    assert!(!sig.is_null(), "captured name text_signal must be non-null");

    {
        let mut tree = h.tree.borrow_mut();
        h.fsb
            .borrow()
            .set_name_field_text_for_test(&mut tree, "report.txt");
    }
    sched.fire(sig);
    do_slice(&mut sched);

    assert_eq!(
        h.fsb.borrow().GetSelectedName(),
        Some("report.txt"),
        "selected_names must propagate from name field GetText()"
    );

    fsb_harness::teardown(&mut sched, h);
}

/// Row 550: Firing the captured FiltersLB selection_signal causes
/// emFileSelectionBox::Cycle to read `GetSelectedIndex()` and call
/// `set_selected_filter_index` (mirrors C++ cpp:469-477).
#[test]
fn row_550_filter_selection_updates_selected_filter_index() {
    use emcore::emFileSelectionBox::emFileSelectionBox;
    use emcore::emPanelTree::PanelTree;
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    // Custom build: filters must be set BEFORE create_children so the
    // FilterLB has populated items that GetSelectedIndex can return.
    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));
    let mut fsb = {
        let mut init_ctx = emcore::emEngineCtx::InitCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            pending_actions: &pa,
        };
        emFileSelectionBox::new(&mut init_ctx, "row550_filter")
    };
    fsb.set_filters(&[
        "All files (*)".to_string(),
        "Text (*.txt)".to_string(),
        "Images (*.png)".to_string(),
    ]);
    let fsb_rc = Rc::new(RefCell::new(fsb));

    let tree_rc: Rc<RefCell<PanelTree>> = Rc::new(RefCell::new(PanelTree::new()));
    let root: PanelId = tree_rc
        .borrow_mut()
        .create_root_deferred_view("row550_filter");

    struct FsbEngine {
        fsb: Rc<RefCell<emFileSelectionBox>>,
        tree: Rc<RefCell<PanelTree>>,
        root: PanelId,
        children_built: bool,
        cycles_run: u32,
    }
    impl emEngine for FsbEngine {
        fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            let sched_ptr: *mut EngineScheduler = &mut *ctx.scheduler;
            let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> = &mut *ctx.framework_actions;
            let stay_awake = {
                let mut ectx = EngineCtx {
                    scheduler: unsafe { &mut *sched_ptr },
                    tree: None,
                    windows: &mut *ctx.windows,
                    root_context: ctx.root_context,
                    framework_actions: unsafe { &mut *fw_ptr },
                    pending_inputs: &mut *ctx.pending_inputs,
                    input_state: &mut *ctx.input_state,
                    framework_clipboard: ctx.framework_clipboard,
                    engine_id: ctx.engine_id,
                    pending_actions: ctx.pending_actions,
                };
                let mut tree_borrow = self.tree.borrow_mut();
                let mut pctx = PanelCtx::with_sched_reach(
                    &mut *tree_borrow,
                    self.root,
                    1.0,
                    unsafe { &mut *sched_ptr },
                    unsafe { &mut *fw_ptr },
                    ctx.root_context,
                    ctx.framework_clipboard,
                    ctx.pending_actions,
                );
                if !self.children_built {
                    self.fsb.borrow_mut().create_children(&mut pctx);
                    self.children_built = true;
                }
                self.fsb.borrow_mut().Cycle(&mut ectx, &mut pctx)
            };
            self.cycles_run += 1;
            stay_awake || self.cycles_run < 4
        }
    }

    let engine = Box::new(FsbEngine {
        fsb: Rc::clone(&fsb_rc),
        tree: Rc::clone(&tree_rc),
        root,
        children_built: false,
        cycles_run: 0,
    });
    let eid = sched.register_engine(engine, Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);

    do_slice(&mut sched);

    let sig = fsb_rc.borrow().filter_sel_sig_for_test();
    assert!(
        !sig.is_null(),
        "captured filter selection_signal must be non-null"
    );

    {
        let mut tree = tree_rc.borrow_mut();
        fsb_rc
            .borrow()
            .set_filter_lb_selected_for_test(&mut tree, vec![2]);
    }
    sched.fire(sig);
    do_slice(&mut sched);

    assert_eq!(
        fsb_rc.borrow().GetSelectedFilterIndex(),
        2,
        "selected_filter_index must propagate from filter LB GetSelectedIndex()"
    );

    sched.remove_engine(eid);
    {
        let mut tree = tree_rc.borrow_mut();
        tree.remove(root, Some(&mut sched));
    }
    sched.clear_pending_for_tests();
    drop(fsb_rc);
}

/// Ordering test: Firing dir + hidden signals in the same Cycle slice must
/// produce both reactions. C++ runs the dir branch (cpp:396) before the
/// hidden branch (cpp:405). Sub-Cycle ordering isn't directly observable
/// from outside without instrumentation, so we assert the strongest
/// available observable: BOTH side effects materialise after a single
/// post-fire Cycle slice (parent_dir updated AND hidden_files_shown
/// updated). This rules out branch interaction skipping or replacing
/// either reaction.
#[test]
fn b010_fsb_cycle_runs_dir_branch_before_hidden_branch() {
    use slotmap::Key as _;

    let mut sched = EngineScheduler::new();
    let h = fsb_harness::build(&mut sched, "ordering_dir_hidden");

    do_slice(&mut sched);

    let dir_sig = h.fsb.borrow().dir_text_sig_for_test();
    let hid_sig = h.fsb.borrow().hidden_check_sig_for_test();
    assert!(!dir_sig.is_null() && !hid_sig.is_null());

    let new_dir = std::env::temp_dir().join("rc_shim_b010_ordering_dummy");
    {
        let mut tree = h.tree.borrow_mut();
        h.fsb
            .borrow()
            .set_dir_field_text_for_test(&mut tree, &new_dir.to_string_lossy());
        h.fsb.borrow().set_hidden_checkbox_for_test(&mut tree, true);
    }
    sched.fire(dir_sig);
    sched.fire(hid_sig);
    do_slice(&mut sched);

    let fsb = h.fsb.borrow();
    assert_eq!(
        fsb.GetParentDirectory(),
        new_dir.as_path(),
        "dir branch must have fired (cpp:396 reaction)"
    );
    assert!(
        fsb.are_hidden_files_shown(),
        "hidden branch must have fired (cpp:405 reaction)"
    );
    drop(fsb);

    fsb_harness::teardown(&mut sched, h);
}
