//! Phase 5 W3 popup architecture test: a popup window constructed in
//! `OsSurface::Pending` transitions to `OsSurface::Materialized` after one
//! `App::about_to_wait` drain pass, and its `WindowId` is registered in
//! `App::windows`.
//!
//! This is a DISPLAY-gated test because materialization requires a real
//! `winit::ActiveEventLoop` + GPU surface. The test skips cleanly on
//! headless hosts (no `DISPLAY` and no `WAYLAND_DISPLAY`).
//!
//! The harness wraps `emcore::emGUIFramework::App` in a custom
//! `ApplicationHandler` that drives two `about_to_wait` passes:
//!   1. First pass: delegate to `App::about_to_wait` so the lazy-wire step
//!      installs `pending_framework_actions` on the home view. Then trigger
//!      popup-entry via `RawVisit` with a rect outside the home rect. This
//!      matches the `emView::RawVisitAbs` popup-entry branch which
//!      synchronously constructs `emWindow` via `new_popup_pending` (state
//!      invariant: `PopupWindow.is_some()`, `OsSurface::Pending`) and
//!      enqueues a `DeferredAction` to materialize.
//!   2. Second pass: delegate to `App::about_to_wait`; its `pending_actions`
//!      drain runs the closure which calls `materialize_popup_surface`,
//!      flipping the surface to `OsSurface::Materialized` and inserting the
//!      new `WindowId` into `App::windows`.

use std::cell::RefCell;
use std::rc::Rc;

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

/// Captured state shared between the setup closure and the harness struct.
struct Captured {
    home_window_id: Option<WindowId>,
    popup_trigger_child: Option<emcore::emPanelTree::PanelId>,
    popup_window: Option<Rc<RefCell<emWindow>>>,
    pending_observed: bool,
    materialized_observed: bool,
    popup_in_app_windows: bool,
}

impl Captured {
    fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            home_window_id: None,
            popup_trigger_child: None,
            popup_window: None,
            pending_observed: false,
            materialized_observed: false,
            popup_in_app_windows: false,
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
        // Swallow RedrawRequested during the test to avoid GPU work we don't
        // need; forward everything else to App so close/resize still work.
        if matches!(event, WindowEvent::RedrawRequested) {
            return;
        }
        self.app.window_event(event_loop, window_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.phase {
            0 => {
                // Delegate to App::about_to_wait so the lazy-wire step
                // installs `pending_framework_actions` on every view.
                self.app.about_to_wait(event_loop);

                // Now trigger popup entry: enable POPUP_ZOOM on the home
                // view and RawVisit a rect outside the home rect. The
                // popup-entry branch of RawVisitAbs will construct a
                // `new_popup_pending` window in `emView::PopupWindow` and
                // enqueue a `DeferredAction` via `pending_framework_actions`.
                let home_rc = self
                    .app
                    .windows
                    .values()
                    .next()
                    .expect("setup must create a home window")
                    .clone();
                let home_id = home_rc.borrow().winit_window().id();
                self.captured.borrow_mut().home_window_id = Some(home_id);

                let child = self
                    .captured
                    .borrow()
                    .popup_trigger_child
                    .expect("popup_trigger_child recorded in setup");
                {
                    let mut home = home_rc.borrow_mut();
                    let tree = &mut self.app.tree;
                    let mut view = home.view_mut();
                    // Attach to scheduler so popup-entry can allocate popup
                    // signals via the real scheduler path.
                    view.set_scheduler(Rc::clone(&self.app.scheduler));
                    // Update first — mirrors test_phase4: clears
                    // zoomed_out_before_sg so RawVisit doesn't immediately
                    // zoom out and tear down the popup.
                    view.Update(tree);
                    // Enable popup zoom mode.
                    view.SetViewFlags(emcore::emView::ViewFlags::POPUP_ZOOM, tree);
                    // Visit `child` with very small rel_a — the ancestor
                    // clamp loop ascends to root with vw >> HomeWidth,
                    // triggering outside_home → popup branch.
                    view.RawVisit(tree, child, 0.0, 0.0, 0.1, true);
                }

                // Synchronous W3 invariant: PopupWindow present, Pending.
                {
                    let home = home_rc.borrow();
                    let popup_rc = home
                        .view()
                        .PopupWindow
                        .clone()
                        .expect("PopupWindow must be Some immediately after popup-entry");
                    assert!(
                        !popup_rc.borrow().is_materialized(),
                        "popup must start in OsSurface::Pending"
                    );
                    self.captured.borrow_mut().pending_observed = true;
                    self.captured.borrow_mut().popup_window = Some(popup_rc);
                }

                // Advance to drain phase. Use Poll so winit fires another
                // about_to_wait immediately.
                self.phase = 1;
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            1 => {
                // Delegate again: the drain now runs the enqueued
                // materialize closure.
                self.app.about_to_wait(event_loop);

                let popup_rc = self
                    .captured
                    .borrow()
                    .popup_window
                    .clone()
                    .expect("popup_window captured in phase 0");
                let materialized = popup_rc.borrow().is_materialized();
                let popup_id = if materialized {
                    popup_rc.borrow().winit_window().id()
                } else {
                    // If materialization didn't happen we still want to
                    // exit cleanly; use a dummy id that won't match.
                    WindowId::dummy()
                };
                let in_windows = self.app.windows.contains_key(&popup_id);

                {
                    let mut cap = self.captured.borrow_mut();
                    cap.materialized_observed = materialized;
                    cap.popup_in_app_windows = in_windows;
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
fn popup_surface_materializes_on_about_to_wait() {
    if !display_available() {
        eprintln!(
            "popup_surface_materializes_on_about_to_wait: skipped \
             (no DISPLAY or WAYLAND_DISPLAY; headless environment)"
        );
        return;
    }

    let captured = Captured::new();
    let captured_for_setup = Rc::clone(&captured);

    let setup = Box::new(move |app: &mut App, event_loop: &ActiveEventLoop| {
        // Build a minimal home window: fresh root panel + a deep child so
        // we can trigger popup entry by visiting it with a tiny rel_a
        // (same pattern as emView::tests::test_phase4_popup_zoom_creates_popup_window).
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
        captured_for_setup.borrow_mut().home_window_id = Some(home_id);
    });

    // Unit tests run on non-main threads. Winit requires the `any_thread`
    // escape hatch on Linux to allow non-main-thread event loops. Pick the
    // builder extension matching the active display server.
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

    // Work around a wgpu-on-Linux shutdown segfault: dropping the wgpu
    // Instance/Device after the Surface has been destroyed crashes in the
    // Vulkan driver (see emGUIFramework::App::run; gfx-rs/wgpu#5781).
    // Forget the harness so its App (with gpu/windows/etc.) never Drops.
    std::mem::forget(harness);

    let cap = captured.borrow();
    assert!(
        cap.pending_observed,
        "phase-0 must observe the popup in OsSurface::Pending"
    );
    assert!(
        cap.materialized_observed,
        "phase-1 must observe the popup transitioned to OsSurface::Materialized"
    );
    assert!(
        cap.popup_in_app_windows,
        "after materialization, popup's WindowId must be present in App::windows"
    );
}
