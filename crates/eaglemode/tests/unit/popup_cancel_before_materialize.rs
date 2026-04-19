//! Phase 5 W3 popup architecture test: a popup window that enters
//! `OsSurface::Pending` but is torn down by popup-exit *before* the
//! deferred `materialize_popup_surface` closure drains must cancel
//! cleanly — no winit window is created, no panic, and `App::windows`
//! is unchanged.
//!
//! This exercises the `Rc::strong_count(&win_rc) == 1` cancellation
//! branch in `App::materialize_popup_surface`. After popup-exit, the
//! only remaining strong reference to the popup `emWindow` is the one
//! captured by the deferred closure itself; when the drain calls the
//! closure, `strong_count` is 1 and materialization is skipped.
//!
//! The test is DISPLAY-gated like `popup_materialization.rs` because
//! the harness builds an `emWindow` home window which requires a real
//! `winit::ActiveEventLoop` + GPU surface. The cancellation *path* does
//! not need DISPLAY, but the *harness* does.
//!
//! The harness drives three phases via `about_to_wait`:
//!   0. Delegate to `App::about_to_wait` (lazy-wires
//!      `pending_framework_actions`). Trigger popup-entry via
//!      `SetViewFlags(POPUP_ZOOM)` + `RawVisit` outside home. Assert
//!      `PopupWindow.is_some()`, popup is `Pending`, and one
//!      `DeferredAction` is enqueued. Then trigger popup-exit via
//!      `RawVisit(root, 0, 0, 1.0)` (inside home) which takes
//!      `PopupWindow`, dropping the view's strong ref. Assert
//!      `PopupWindow.is_none()`.
//!   1. Delegate to `App::about_to_wait`; the drain runs the
//!      materialize closure, which sees `strong_count == 1` and
//!      returns without creating a winit window. Snapshot
//!      `app.windows.len()` and `Weak::strong_count(&popup_weak)`.
//!   2. Exit event loop.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use emcore::emGUIFramework::App;
use emcore::emWindow::{emWindow, WindowFlags};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
use winit::window::WindowId;

fn display_available() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

struct Captured {
    popup_trigger_child: Option<emcore::emPanelTree::PanelId>,
    /// Weak reference to the popup — strong refs should only be the
    /// view (before exit) and the deferred closure (until drain). We
    /// must NOT hold a strong ref here or the cancellation check fails.
    popup_weak: Option<Weak<RefCell<emWindow>>>,
    /// Phase-0 observations.
    pending_observed: bool,
    queued_action_count: usize,
    popup_window_cleared_after_exit: bool,
    windows_before_exit: usize,
    /// Phase-1 observations.
    windows_after_drain: usize,
    popup_strong_count_after_drain: usize,
}

impl Captured {
    fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            popup_trigger_child: None,
            popup_weak: None,
            pending_observed: false,
            queued_action_count: 0,
            popup_window_cleared_after_exit: false,
            windows_before_exit: 0,
            windows_after_drain: 0,
            popup_strong_count_after_drain: 0,
        }))
    }
}

struct Harness {
    app: App,
    captured: Rc<RefCell<Captured>>,
    phase: u32,
}

impl ApplicationHandler for Harness {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.app.resumed(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::RedrawRequested) {
            return;
        }
        self.app.window_event(event_loop, window_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.phase {
            0 => {
                // Lazy-wire `pending_framework_actions` onto the view.
                self.app.about_to_wait(event_loop);

                let home_rc = self
                    .app
                    .windows
                    .values()
                    .next()
                    .expect("setup must create a home window")
                    .clone();

                let child = self
                    .captured
                    .borrow()
                    .popup_trigger_child
                    .expect("popup_trigger_child recorded in setup");

                // --- Popup-entry ---
                {
                    let mut home = home_rc.borrow_mut();
                    let tree = &mut self.app.tree;
                    let mut view = home.view_mut();
                    view.set_scheduler(Rc::clone(&self.app.scheduler));
                    // Clear zoomed_out_before_sg so RawVisit doesn't
                    // zoom-out-and-tear-down; mirrors popup_materialization.
                    view.Update(tree);
                    view.SetViewFlags(emcore::emView::ViewFlags::POPUP_ZOOM, tree);
                    // Small rel_a → vw >> HomeWidth → outside_home → popup branch.
                    view.RawVisit(tree, child, 0.0, 0.0, 0.1, true);
                }

                // Invariant: popup Pending + one action queued.
                {
                    let home = home_rc.borrow();
                    let popup_rc = home
                        .view()
                        .PopupWindow
                        .clone()
                        .expect("PopupWindow must be Some after popup-entry");
                    assert!(
                        !popup_rc.borrow().is_materialized(),
                        "popup must start in OsSurface::Pending"
                    );
                    let mut cap = self.captured.borrow_mut();
                    cap.pending_observed = true;
                    cap.popup_weak = Some(Rc::downgrade(&popup_rc));
                    cap.queued_action_count = self.app.pending_actions.borrow().len();
                    cap.windows_before_exit = self.app.windows.len();
                    // `popup_rc` drops here — the view still owns one
                    // strong ref, deferred closure owns the other.
                }

                assert!(
                    self.captured.borrow().queued_action_count >= 1,
                    "popup-entry must enqueue a materialize DeferredAction"
                );

                // --- Popup-exit (before materialization drain) ---
                {
                    let mut home = home_rc.borrow_mut();
                    let tree = &mut self.app.tree;
                    let mut view = home.view_mut();
                    // ZoomOut → RawVisit(root, 0, 0, zoom_out_rel_a)
                    // produces a rect fitting inside home. With
                    // POPUP_ZOOM still set, RawVisitAbs takes the
                    // teardown branch (!outside_home && PopupWindow.is_some()),
                    // dropping the view's strong ref on the popup. The
                    // only remaining strong ref is the one captured by
                    // the deferred materialize closure.
                    view.ZoomOut(tree);
                }

                // Invariant: PopupWindow cleared.
                {
                    let home = home_rc.borrow();
                    let cleared = home.view().PopupWindow.is_none();
                    self.captured.borrow_mut().popup_window_cleared_after_exit = cleared;
                }

                // Advance to drain phase.
                self.phase = 1;
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            1 => {
                // Drain runs the materialize closure: strong_count == 1
                // → cancellation returns without creating a window.
                self.app.about_to_wait(event_loop);

                let weak = self
                    .captured
                    .borrow()
                    .popup_weak
                    .clone()
                    .expect("popup_weak recorded in phase 0");
                // After the drain, the closure has run and dropped its
                // captured Rc — strong_count should be 0 (popup fully
                // dropped, no winit window created).
                let strong = weak.strong_count();

                {
                    let mut cap = self.captured.borrow_mut();
                    cap.windows_after_drain = self.app.windows.len();
                    cap.popup_strong_count_after_drain = strong;
                }

                self.phase = 2;
                event_loop.exit();
            }
            _ => {
                event_loop.exit();
            }
        }
    }
}

#[test]
fn popup_cancels_when_dropped_before_materialize() {
    if !display_available() {
        eprintln!(
            "popup_cancels_when_dropped_before_materialize: skipped \
             (no DISPLAY or WAYLAND_DISPLAY; headless environment)"
        );
        return;
    }

    let captured = Captured::new();
    let captured_for_setup = Rc::clone(&captured);

    let setup = Box::new(move |app: &mut App, event_loop: &ActiveEventLoop| {
        let root = app.tree.create_root_deferred_view("test_root");
        app.tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);
        let child = app.tree.create_child(root, "child_a");
        app.tree.Layout(child, 0.0, 0.0, 0.5, 1.0, 1.0);
        captured_for_setup.borrow_mut().popup_trigger_child = Some(child);

        let close = app.scheduler.borrow_mut().create_signal();
        let flags_sig = app.scheduler.borrow_mut().create_signal();
        let focus_sig = app.scheduler.borrow_mut().create_signal();
        let geom_sig = app.scheduler.borrow_mut().create_signal();

        let home = emWindow::create(
            event_loop,
            app.gpu(),
            std::rc::Rc::clone(&app.context),
            root,
            WindowFlags::empty(),
            close,
            flags_sig,
            focus_sig,
            geom_sig,
        );
        let home_id = home.borrow().winit_window().id();
        app.windows.insert(home_id, home);
    });

    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    let mut event_loop = {
        let mut builder = EventLoop::builder();
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            use winit::platform::wayland::EventLoopBuilderExtWayland;
            builder.with_any_thread(true);
        } else {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_any_thread(true);
        }
        builder.build().expect("create event loop")
    };
    #[cfg(not(all(unix, not(target_os = "macos"), not(target_os = "android"))))]
    let mut event_loop = EventLoop::new().expect("create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut harness = Harness {
        app: App::new(setup),
        captured: Rc::clone(&captured),
        phase: 0,
    };

    event_loop
        .run_app_on_demand(&mut harness)
        .expect("run_app_on_demand");

    // wgpu-on-Linux shutdown segfault workaround: keep harness alive.
    std::mem::forget(harness);

    let cap = captured.borrow();
    assert!(
        cap.pending_observed,
        "phase-0 must observe popup in OsSurface::Pending"
    );
    assert!(
        cap.queued_action_count >= 1,
        "popup-entry must enqueue at least one DeferredAction (got {})",
        cap.queued_action_count
    );
    assert!(
        cap.popup_window_cleared_after_exit,
        "popup-exit must clear emView::PopupWindow to None"
    );
    assert_eq!(
        cap.windows_after_drain, cap.windows_before_exit,
        "App::windows must be unchanged after cancellation \
         (before={}, after={})",
        cap.windows_before_exit, cap.windows_after_drain
    );
    assert_eq!(
        cap.popup_strong_count_after_drain, 0,
        "popup Rc must be fully dropped after drain \
         (no winit window created); got strong_count={}",
        cap.popup_strong_count_after_drain
    );
}
