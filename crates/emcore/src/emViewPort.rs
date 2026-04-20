// Port of C++ emViewPort (emView.h:719-794). View↔OS connection class.
//
// DIVERGED: not a class hierarchy but a concrete struct with optional backend
// hooks. Rust has no dummy-base-class pattern; the "default implementation
// connects to nothing" model becomes an `Option<WindowId>` back-reference
// (resolved through `EngineCtx::windows` / `App::windows`) which is `None`
// for dummy instances. Earlier versions used `Weak<RefCell<emWindow>>`;
// Phase-2 port-ownership-rewrite narrowed windows to plain `emWindow` so
// Weak is no longer expressible.
//
// C++ emViewPort has two constructors:
//   emViewPort(emView & homeView) — real port, registers itself on the view
//   emViewPort()                  — dummy port (private, used for DummyViewPort)
//
// DIVERGED: Rust uses two named constructors: `new_dummy()` (connects to
// nothing) and `new_for_view(home_x, home_y, home_w, home_h, home_pt)`.
// The C++ constructor-registers-on-view side-effect is handled by the call
// site (emView::new and emWindow::new_popup) rather than inside emViewPort.
//
// Phase 5 wires the seven backend dispatch methods:
//   PaintView             — request_redraw on owning emWindow
//   GetViewCursor         — return cached cursor
//   SetViewCursor         — update cached cursor, set dirty flag (Rust-only helper)
//   IsSoftKeyboardShown   — UPSTREAM-GAP: no C++ backend overrides this
//   ShowSoftKeyboard      — UPSTREAM-GAP: no C++ backend overrides this
//   GetInputClockMS       — return cached scheduler clock
//   InputToView           — forward to emView::Input
//   InvalidateCursor      — set cursor-dirty flag (consumed by emWindow on next frame)
//   InvalidatePainting    — delegate to emWindow::invalidate_rect (tile cache)

use winit::window::WindowId;

/// Port of C++ `emViewPort` (emView.h:719-794).
///
/// Connects an `emView` to its OS/hardware backend. The default ("dummy")
/// instance connects to nothing (back-reference is `None`). A real instance
/// has a `WindowId` back-reference set by `emWindow::create`; callers resolve
/// it through `EngineCtx::windows` / `App::windows`.
#[allow(non_snake_case)]
pub struct emViewPort {
    // === C++ private fields (emView.h:789-793) ===
    // HomeView pointer — in C++ a raw *emView.
    // DIVERGED: stores the home geometry on the port (plain f64 fields)
    // rather than following a back-reference to the emView. C++ reads these
    // through a raw `HomeView*` pointer (emViewPort reaches the view's
    // HomeX/Y/Width/Height/HomePixelTallness). A Rust `Weak<RefCell<emView>>`
    // back-reference would not be a cycle-hazard (the `window` field below
    // uses exactly that shape), but `upgrade().borrow()` cannot run while
    // emView is already borrowed_mut — which is the common case inside
    // `SetGeometry` and `SwapViewPorts`. Storing the geometry on the port
    // avoids that re-entrancy and lets `SwapViewPorts` move it atomically
    // when the Home and Current ports exchange identities.
    pub home_x: f64,
    pub home_y: f64,
    pub home_width: f64,
    pub home_height: f64,
    /// C++ `HomeView->HomePixelTallness` — pixel shape ratio of the home view
    /// owning this port. Used by `SwapViewPorts` to update `CurrentPixelTallness`
    /// from the correct view after the port exchange, matching C++ emView.cpp:1990.
    pub home_pixel_tallness: f64,

    /// Back-reference to the owning emWindow by WindowId. Used by
    /// `PaintView` / `InvalidatePainting` to dispatch to backend machinery.
    /// Resolved through `EngineCtx::windows` / `App::windows` at call time,
    /// so no `Weak` back-reference is needed. `None` for dummy ports and
    /// for popup ports while the OS surface is still `Pending`.
    pub(crate) window_id: Option<WindowId>,

    /// Cursor reported by the view; `emWindow` consumes it on each frame.
    /// (C++ stores the cursor on `emView`, not `emViewPort`; the Rust
    /// design caches it on the port so the window can read it without
    /// a back-ref to the view.)
    pub(crate) cursor: crate::emCursor::emCursor,

    /// Set by `InvalidateCursor`; `emWindow` consumes the flag on next frame.
    pub(crate) cursor_dirty: bool,

    /// Monotonic-millisecond clock value, set by `emWindow` on each input
    /// dispatch from the scheduler. Read by `GetInputClockMS`.
    pub(crate) input_clock_ms: u64,

    /// Test instrumentation: counts `InputToView` dispatches.
    pub input_event_count: u64,
}

#[allow(non_snake_case)]
impl emViewPort {
    /// Creates a dummy port not connected to any backend.
    ///
    /// Port of C++ `emViewPort::emViewPort()` (private no-arg ctor,
    /// emView.cpp:2715-2719): `HomeView=NULL; CurrentView=NULL;`
    pub fn new_dummy() -> Self {
        Self {
            home_x: 0.0,
            home_y: 0.0,
            home_width: 0.0,
            home_height: 0.0,
            home_pixel_tallness: 1.0,
            window_id: None,
            cursor: crate::emCursor::emCursor::Normal,
            cursor_dirty: false,
            input_clock_ms: 0,
            input_event_count: 0,
        }
    }

    /// Creates a port with known initial geometry (used by popup window stubs).
    ///
    /// Port of C++ `emViewPort::emViewPort(emView & homeView)`
    /// (emView.cpp:2623-2633): registers the port on the view.
    ///
    /// DIVERGED: registration side-effect moved to call site; geometry
    /// passed explicitly instead of read from homeView on construction.
    pub fn new_with_geometry(
        home_x: f64,
        home_y: f64,
        home_width: f64,
        home_height: f64,
        home_pixel_tallness: f64,
    ) -> Self {
        Self {
            home_x,
            home_y,
            home_width,
            home_height,
            home_pixel_tallness,
            window_id: None,
            cursor: crate::emCursor::emCursor::Normal,
            cursor_dirty: false,
            input_clock_ms: 0,
            input_event_count: 0,
        }
    }

    // === GetView* accessors (emView.h:752-755) ===

    /// Port of C++ `emViewPort::GetViewX`.
    pub fn GetViewX(&self) -> f64 {
        self.home_x
    }

    /// Port of C++ `emViewPort::GetViewY`.
    pub fn GetViewY(&self) -> f64 {
        self.home_y
    }

    /// Port of C++ `emViewPort::GetViewWidth`.
    pub fn GetViewWidth(&self) -> f64 {
        self.home_width
    }

    /// Port of C++ `emViewPort::GetViewHeight`.
    pub fn GetViewHeight(&self) -> f64 {
        self.home_height
    }

    /// Port of C++ `emViewPort::GetViewCursor`.
    pub fn GetViewCursor(&self) -> crate::emCursor::emCursor {
        self.cursor
    }

    /// Cache the cursor reported by the view.
    ///
    /// Marks the dirty flag only if the cursor actually changed.
    /// (C++ stores this on emViewPort identically per the plan comment.)
    pub fn SetViewCursor(&mut self, cursor: crate::emCursor::emCursor) {
        if self.cursor != cursor {
            self.cursor = cursor;
            self.cursor_dirty = true;
        }
    }

    /// Port of C++ `emViewPort::PaintView`.
    ///
    /// Requests a redraw on the owning `emWindow`, resolved via
    /// `windows[self.window_id]`. No-op for dummy ports (`window_id` is
    /// `None`) and for popup ports whose OS surface is still `Pending`
    /// (WindowId not yet in `windows`).
    pub fn PaintView(
        &self,
        windows: &std::collections::HashMap<WindowId, crate::emWindow::emWindow>,
    ) {
        debug_assert!(
            self.window_id.is_some() || cfg!(test),
            "emViewPort::PaintView called on viewport without back-reference"
        );
        if let Some(wid) = self.window_id {
            if let Some(win) = windows.get(&wid) {
                win.request_redraw();
            }
        }
    }

    // === Protected methods (emView.h:763-778) ===

    /// Port of C++ `emViewPort::SetViewGeometry(x, y, w, h, pixelTallness)`
    /// (emView.h:1025-1030).
    ///
    /// Updates the stored home geometry including pixel tallness. Called by
    /// `SetViewPosSize` and the backend geometry callback.
    pub fn SetViewGeometry(&mut self, x: f64, y: f64, w: f64, h: f64, pixel_tallness: f64) {
        self.home_x = x;
        self.home_y = y;
        self.home_width = w;
        self.home_height = h;
        self.home_pixel_tallness = pixel_tallness;
    }

    /// Port of C++ `emViewPort::SetViewFocused(bool focused)`.
    ///
    /// C++ (emView.h:1032-1035): calls `CurrentView->SetFocused(focused)`.
    /// Rust: focus is canonical on `emView::window_focused`; callers that have
    /// an `&mut emView` in scope must call `emView::SetFocused` directly.
    /// This stub exists to satisfy the C++ name correspondence requirement;
    /// it is a no-op because the port no longer stores a focused field.
    ///
    /// DIVERGED: C++ mutates the view through a raw `CurrentView*` pointer;
    /// Rust drops the forwarding because the port has no back-reference to
    /// `emView`. All known call sites have been migrated to `emView::SetFocused`.
    pub fn SetViewFocused(&mut self, _focused: bool) {}

    /// Port of C++ `emViewPort::RequestFocus`.
    ///
    /// C++ default implementation (emView.cpp:2661): calls `SetViewFocused(true)`.
    /// Rust: see `SetViewFocused` — callers with `&mut emView` in scope call
    /// `emView::SetFocused(tree, true)` directly.
    pub fn RequestFocus(&mut self) {}

    /// Port of C++ `emViewPort::IsSoftKeyboardShown`.
    ///
    /// UPSTREAM-GAP: emCore ships this as a no-op; no platform backend
    /// (emX11, emWnds) overrides it. Soft-keyboard support is absent in
    /// upstream Eagle Mode.
    pub fn IsSoftKeyboardShown(&self) -> bool {
        false
    }

    /// Port of C++ `emViewPort::ShowSoftKeyboard`.
    ///
    /// UPSTREAM-GAP: emCore ships this as a no-op; no platform backend
    /// (emX11, emWnds) overrides it. Soft-keyboard support is absent in
    /// upstream Eagle Mode.
    pub fn ShowSoftKeyboard(&mut self, _show: bool) {}

    /// Port of C++ `emViewPort::GetInputClockMS`.
    ///
    /// Returns the monotonic-millisecond clock cached at input-dispatch
    /// time by `emWindow`. C++ calls `emGetClockMS()` directly; Rust caches
    /// the value so the view-port doesn't need a back-reference to the
    /// scheduler.
    pub fn GetInputClockMS(&self) -> u64 {
        self.input_clock_ms
    }

    /// Port of C++ `emViewPort::InputToView`.
    ///
    /// Routes an input event to the home view. C++ dispatches via
    /// `CurrentView->Input(event, state)`; Rust takes `view` and `tree` as
    /// parameters because the back-reference is `Option<WindowId>` (resolved
    /// through `EngineCtx::windows`) and `emView` lives inside `emWindow`.
    /// The dispatch site in `emWindow::dispatch_input` already holds borrows
    /// to both.
    pub fn InputToView(
        &mut self,
        view: &mut crate::emView::emView,
        tree: &mut crate::emPanelTree::PanelTree,
        event: &crate::emInput::emInputEvent,
        state: &crate::emInputState::emInputState,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) {
        self.input_event_count += 1;
        view.Input(tree, event, state, ctx);
    }

    /// Port of C++ `emViewPort::InvalidateCursor`.
    ///
    /// Marks the cursor dirty so the owning `emWindow` will apply the new
    /// cursor on the next frame.
    pub fn InvalidateCursor(&mut self) {
        self.cursor_dirty = true;
    }

    /// Port of C++ `emViewPort::InvalidatePainting(x, y, w, h)`.
    ///
    /// Delegates to the owning `emWindow`'s tile cache, resolved via
    /// `windows[self.window_id]`. No-op for dummy ports and for popup
    /// ports whose OS surface is still `Pending`.
    pub fn InvalidatePainting(
        &mut self,
        windows: &mut std::collections::HashMap<WindowId, crate::emWindow::emWindow>,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) {
        debug_assert!(
            self.window_id.is_some() || cfg!(test),
            "emViewPort::InvalidatePainting called on viewport without back-reference"
        );
        if let Some(wid) = self.window_id {
            if let Some(win) = windows.get_mut(&wid) {
                win.invalidate_rect(x, y, w, h);
            }
        }
    }

    /// Updates the home geometry fields and pixel tallness.
    ///
    /// Called by `emWindow::SetViewPosSize` when the popup window is
    /// repositioned, and by `SwapViewPorts` geometry updates.
    ///
    /// DIVERGED: no direct C++ emViewPort method with this name;
    /// the equivalent in C++ is the `emWindowPort` override of
    /// `SetViewGeometry` triggered by the windowing system. Phase 4
    /// implements this as a direct setter on the stub.
    pub fn SetViewPosSize(&mut self, x: f64, y: f64, w: f64, h: f64, pixel_tallness: f64) {
        self.home_x = x;
        self.home_y = y;
        self.home_width = w;
        self.home_height = h;
        self.home_pixel_tallness = pixel_tallness;
    }
}
