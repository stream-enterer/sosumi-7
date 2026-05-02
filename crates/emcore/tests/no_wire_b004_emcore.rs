//! B-004 emcore-slice behavioral tests — P-001-no-subscribe-no-accessor.
//!
//! Row `emFilePanel-accessor-vir-file-state`:
//!   Verifies that `emFilePanel` fires `VirFileStateSignal` when
//!   `VirtualFileState` changes (via `cycle_inner` state detection and
//!   via the deferred-pending drain for out-of-Cycle mutations).
//!
//! Row `emImageFile-117`:
//!   Verifies that `emImageFilePanel` subscribes to `VirFileStateSignal`
//!   on first Cycle and populates `current_image` when the signal fires.
//!
//! RUST_ONLY: (dependency-forced) — no C++ test analogue; mirrors
//! B-005, B-006, B-007, B-008, B-015 test rationale.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use slotmap::Key as _;

use emcore::emEngine::{emEngine, EngineId, Priority};
use emcore::emEngineCtx::{EngineCtx, PanelCtx};
use emcore::emFileModel::{emFileModel, FileModelState};
use emcore::emFilePanel::emFilePanel;
use emcore::emImage::emImage;
use emcore::emImageFile::{emImageFileModel, ImageFileData};
use emcore::emImageFileImageFilePanel::emImageFilePanel;
use emcore::emPanel::PanelBehavior;
use emcore::emPanelScope::PanelScope;
use emcore::emPanelTree::{PanelId, PanelTree};
use emcore::emScheduler::EngineScheduler;
use emcore::emSignal::SignalId;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

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
// Test 1: emFilePanel fires VirFileStateSignal on SetFileModel + cycle_inner
// ---------------------------------------------------------------------------

/// Drive `emFilePanel::Cycle` through the scheduler; also serves as the
/// observer of `VirFileStateSignal` after the signal is allocated.
struct FilePanelB004Driver {
    panel: Rc<RefCell<emFilePanel>>,
    tree: PanelTree,
    root: PanelId,
    /// Id of the observer engine that will subscribe to VirFileStateSignal.
    observer_eid: EngineId,
    cycles: u32,
    /// Tracks whether we have already connected the observer to the signal.
    observer_connected: bool,
}

impl emEngine for FilePanelB004Driver {
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>) -> bool {
        let mut pctx = PanelCtx::new(&mut self.tree, self.root, 1.0);
        self.panel.borrow_mut().Cycle(ectx, &mut pctx);
        // After the first Cycle, VirFileStateSignal is allocated. Connect the
        // observer to it so subsequent fires are delivered to it.
        if !self.observer_connected {
            let sig = self.panel.borrow().GetVirFileStateSignal();
            if !sig.is_null() {
                ectx.connect(sig, self.observer_eid);
                self.observer_connected = true;
            }
        }
        self.cycles += 1;
        self.cycles < 8
    }
}

/// Observer engine: subscribes to VirFileStateSignal and records first fire.
struct VirStateObserver {
    signal: Rc<Cell<SignalId>>,
    fired: Rc<Cell<bool>>,
}

impl emEngine for VirStateObserver {
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>) -> bool {
        let sig = self.signal.get();
        if !sig.is_null() && ectx.IsSignaled(sig) {
            self.fired.set(true);
        }
        !self.fired.get()
    }
}

/// B-004 row `emFilePanel-accessor-vir-file-state`:
///
/// `emFilePanel::Cycle` fires `VirFileStateSignal` when:
/// (a) `SetFileModel` sets `pending_vir_state_fire` (drained next Cycle), and
/// (b) `cycle_inner` detects a `VirtualFileState` transition.
///
/// Mirrors C++ `emFilePanel::SetFileModel` Signal(VirFileStateSignal) at
/// emFilePanel.cpp:51 and `Cycle` Signal(VirFileStateSignal) at :158/:179.
#[test]
fn vir_file_state_signal_fires_on_set_file_model_and_cycle_change() {
    let mut sched = EngineScheduler::new();

    let model_rc = Rc::new(RefCell::new(emFileModel::<String>::new(PathBuf::from(
        "/tmp/b004_vfs_signal",
    ))));

    let panel_rc: Rc<RefCell<emFilePanel>> = Rc::new(RefCell::new(emFilePanel::new()));

    let fired = Rc::new(Cell::new(false));
    let vir_sig: Rc<Cell<SignalId>> = Rc::new(Cell::new(SignalId::null()));

    // Register observer first so its EngineId is available to the driver.
    let observer = Box::new(VirStateObserver {
        signal: vir_sig.clone(),
        fired: fired.clone(),
    });
    let obs_eid = sched.register_engine(observer, Priority::Low, PanelScope::Framework);
    sched.wake_up(obs_eid);

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("b004_fp");
    let driver = Box::new(FilePanelB004Driver {
        panel: panel_rc.clone(),
        tree,
        root,
        observer_eid: obs_eid,
        cycles: 0,
        observer_connected: false,
    });
    let driver_eid = sched.register_engine(driver, Priority::Low, PanelScope::Framework);
    sched.wake_up(driver_eid);

    // Slice 1: driver Cycle 0 — allocates VirFileStateSignal; observer runs
    // (signal still null, nothing to observe).
    do_slice(&mut sched);

    // Propagate signal id to observer.
    vir_sig.set(panel_rc.borrow().GetVirFileStateSignal());
    assert!(
        !vir_sig.get().is_null(),
        "VirFileStateSignal must be allocated after first Cycle"
    );

    // Bind model: triggers pending_vir_state_fire = true.
    panel_rc
        .borrow_mut()
        .SetFileModel(Some(model_rc.clone() as Rc<RefCell<dyn FileModelState>>));
    sched.wake_up(driver_eid);
    sched.wake_up(obs_eid);

    // Slice 2: driver Cycle 1 — drains pending, fires VirFileStateSignal.
    // Observer is already connected (from end of Cycle 0) and will be woken.
    do_slice(&mut sched);

    // Give the observer one more slice to observe the signal if it woke after driver.
    do_slice(&mut sched);

    assert!(
        fired.get(),
        "VirFileStateSignal must fire when SetFileModel triggers pending_vir_state_fire"
    );

    sched.remove_engine(driver_eid);
    sched.remove_engine(obs_eid);
    sched.abort_all_pending();
}

// ---------------------------------------------------------------------------
// Test 2: emImageFilePanel refreshes current_image on VirFileStateSignal
// ---------------------------------------------------------------------------

struct ImagePanelB004Engine {
    panel: Rc<RefCell<emImageFilePanel>>,
    tree: PanelTree,
    root: PanelId,
    cycles: u32,
}

impl emEngine for ImagePanelB004Engine {
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>) -> bool {
        let mut pctx = PanelCtx::new(&mut self.tree, self.root, 1.0);
        self.panel.borrow_mut().Cycle(ectx, &mut pctx);
        self.cycles += 1;
        self.cycles < 6
    }
}

/// B-004 row `emImageFile-117`:
///
/// After the underlying `emImageFileModel` completes loading,
/// `emImageFilePanel::Cycle` detects the `VirtualFileState` transition
/// via `VirFileStateSignal`, calls `refresh_current_image_from_model()`,
/// and populates `current_image`.
///
/// Mirrors C++ `emImageFilePanel::Cycle` reaction at emImageFile.cpp:202-205
/// where `IsSignaled(GetVirFileStateSignal())` triggers state-dependent
/// behavior (here: image cache population instead of control-panel invalidation).
#[test]
fn image_panel_refreshes_current_image_after_load() {
    let mut sched = EngineScheduler::new();

    let data_change_sig = sched.create_signal();
    let mut model_inner =
        emImageFileModel::new(PathBuf::from("/tmp/b004_image_panel.tga"), data_change_sig);

    // Complete the load with a non-empty image so GetImage() returns Some.
    let image_data = ImageFileData {
        image: emImage::new(4, 4, 4),
        ..ImageFileData::default()
    };
    model_inner.file_model_mut().complete_load(image_data);

    let model_rc = Rc::new(RefCell::new(model_inner));

    let panel_rc: Rc<RefCell<emImageFilePanel>> = Rc::new(RefCell::new(emImageFilePanel::new()));
    panel_rc
        .borrow_mut()
        .SetImageFileModel(Some(model_rc.clone()));

    assert!(
        panel_rc.borrow().current_image_for_test().is_none(),
        "current_image must be None before any Cycle"
    );

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("b004_img_panel");
    let engine = Box::new(ImagePanelB004Engine {
        panel: panel_rc.clone(),
        tree,
        root,
        cycles: 0,
    });
    let eid = sched.register_engine(engine, Priority::Low, PanelScope::Framework);
    sched.wake_up(eid);

    // Run enough slices for:
    // Slice 1: Cycle 0 — allocates VirFileStateSignal, drains pending fire
    //           from SetImageFileModel, subscribes to ChangeSignal +
    //           VirFileStateSignal, detects Waiting→Loaded transition via
    //           cycle_inner, fires VirFileStateSignal → wakes engine.
    // Slice 2: Cycle 1 — IsSignaled(VirFileStateSignal) → refresh_current_image.
    // Extra slices for safety.
    for _ in 0..4 {
        do_slice(&mut sched);
    }

    assert!(
        panel_rc.borrow().current_image_for_test().is_some(),
        "B-004 row emImageFile-117: current_image must be Some after load + VirFileStateSignal"
    );

    sched.remove_engine(eid);
    sched.abort_all_pending();
}
