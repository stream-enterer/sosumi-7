// Port of C++ emViewPort (emView.h:719-794). View↔OS connection class.
//
// DIVERGED: not a class hierarchy but a concrete struct with optional backend
// hooks. Rust has no dummy-base-class pattern; the "default implementation
// connects to nothing" model becomes an Option<Box<dyn BackendPort>> in the
// future; for Phase 4 the struct is self-contained with no backend.
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
// For Phase 4, the methods called by emView viewing/geometry paths are fully
// implemented:
//   GetViewX/Y/Width/Height   — delegate to stored home geometry
//   SetViewGeometry           — update stored home geometry
//   SetViewFocused            — update focused flag
//   RequestFocus              — set_focused(true) (dummy base class behaviour)
//   is_focused / set_focused  — Rust accessors (DIVERGED: no C++ equivalent
//                               with these exact names; C++ uses Focused field)
//   SetViewPosSize            — update home geometry (used by SwapViewPorts
//                               and popup window placement)
//   InvalidatePainting        — PHASE-5-TODO: backend dirty-rect dispatch
//   InvalidateCursor          — PHASE-5-TODO: backend cursor dirty flag
//
// The remaining virtual methods are stubs:
//   PaintView             — PHASE-5-TODO: backend compositing hook
//   GetViewCursor         — PHASE-5-TODO: backend cursor query
//   IsSoftKeyboardShown   — PHASE-5-TODO: touch platform hook
//   ShowSoftKeyboard      — PHASE-5-TODO: touch platform hook
//   GetInputClockMS       — PHASE-5-TODO: returns 0 as placeholder
//   InputToView           — PHASE-5-TODO: backend input dispatch

/// Port of C++ `emViewPort` (emView.h:719-794).
///
/// Connects an `emView` to its OS/hardware backend. The default ("dummy")
/// instance connects to nothing. A real instance would be created by a
/// backend (e.g. `emWindowPort` in C++).
#[allow(non_snake_case)]
pub struct emViewPort {
    // === C++ private fields (emView.h:789-793) ===
    // HomeView pointer — in C++ a raw *emView.
    // DIVERGED: Phase 4 stores the home geometry as plain f64 fields rather
    // than holding a back-reference to the emView. The C++ design used raw
    // pointer access; Rust Rc<RefCell> cycles require an alternative approach.
    // emView reads/writes these fields directly via SwapViewPorts.
    pub home_x: f64,
    pub home_y: f64,
    pub home_width: f64,
    pub home_height: f64,

    // Focus state for this port. C++ stores it on the view (Focused field);
    // here it lives on the port so SwapViewPorts can transfer it between home
    // and popup without touching the view's window_focused directly.
    //
    // DIVERGED: no direct C++ equivalent field on emViewPort; focus is on
    // the emView. Introduced here to support SwapViewPorts stub.
    focused: bool,
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
            focused: false,
        }
    }

    /// Creates a port with known initial geometry (used by popup window stubs).
    ///
    /// Port of C++ `emViewPort::emViewPort(emView & homeView)`
    /// (emView.cpp:2623-2633): registers the port on the view.
    ///
    /// DIVERGED: registration side-effect moved to call site; geometry
    /// passed explicitly instead of read from homeView on construction.
    pub fn new_with_geometry(home_x: f64, home_y: f64, home_width: f64, home_height: f64) -> Self {
        Self {
            home_x,
            home_y,
            home_width,
            home_height,
            focused: false,
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
    ///
    /// PHASE-5-TODO: backend cursor query.
    pub fn GetViewCursor(&self) -> crate::emCursor::emCursor {
        crate::emCursor::emCursor::Normal
    }

    /// Port of C++ `emViewPort::PaintView`.
    ///
    /// PHASE-5-TODO: backend compositing hook.
    pub fn PaintView(&self) {
        // PHASE-5-TODO: dispatch to backend compositor
    }

    // === Protected methods (emView.h:763-778) ===

    /// Port of C++ `emViewPort::SetViewGeometry(x, y, w, h, pixelTallness)`.
    /// Updates the stored home geometry. Called by `SetViewPosSize` and the
    /// backend geometry callback.
    pub fn SetViewGeometry(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.home_x = x;
        self.home_y = y;
        self.home_width = w;
        self.home_height = h;
    }

    /// Port of C++ `emViewPort::SetViewFocused(bool focused)`.
    /// Updates the focus state on this port.
    pub fn SetViewFocused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Port of C++ `emViewPort::RequestFocus`.
    ///
    /// Default implementation (emView.cpp:2661): calls `SetViewFocused(true)`.
    pub fn RequestFocus(&mut self) {
        // emView.cpp:2661-2664: default base-class implementation
        self.SetViewFocused(true);
    }

    /// Port of C++ `emViewPort::IsSoftKeyboardShown`.
    ///
    /// PHASE-5-TODO: touch platform hook.
    pub fn IsSoftKeyboardShown(&self) -> bool {
        // emView.cpp:2667-2671: default returns false
        false
    }

    /// Port of C++ `emViewPort::ShowSoftKeyboard`.
    ///
    /// PHASE-5-TODO: touch platform hook.
    pub fn ShowSoftKeyboard(&mut self, _show: bool) {
        // emView.cpp:2673-2676: default is a no-op
    }

    /// Port of C++ `emViewPort::GetInputClockMS`.
    ///
    /// PHASE-5-TODO: returns 0 as a placeholder; backend should return a
    /// high-resolution monotonic clock value in milliseconds.
    pub fn GetInputClockMS(&self) -> u64 {
        // emView.cpp:2678-2681: default calls emGetClockMS()
        // PHASE-5-TODO: call the scheduler's clock when available.
        0
    }

    /// Port of C++ `emViewPort::InputToView`.
    ///
    /// PHASE-5-TODO: backend input dispatch to view.
    pub fn InputToView(&self) {
        // emView.cpp:2684-2691: routes event to FirstVIF or View::Input
        // PHASE-5-TODO: wire up when input path is in scope.
    }

    /// Port of C++ `emViewPort::InvalidateCursor`.
    ///
    /// PHASE-5-TODO: backend cursor dirty flag.
    pub fn InvalidateCursor(&mut self) {
        // emView.cpp:2693-2695: default is a no-op
        // PHASE-5-TODO: notify backend to update cursor
    }

    /// Port of C++ `emViewPort::InvalidatePainting(x, y, w, h)`.
    ///
    /// PHASE-5-TODO: backend dirty-rect dispatch.
    pub fn InvalidatePainting(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {
        // emView.cpp:2698-2700: default is a no-op
        // PHASE-5-TODO: accumulate dirty rect and notify backend compositor
    }

    // === Rust-only accessors for SwapViewPorts / focus transfer ===

    /// Returns whether this port currently holds focus.
    ///
    /// DIVERGED: no C++ equivalent method on emViewPort; focus state was
    /// stored directly on emView::Focused. Introduced to support
    /// SwapViewPorts focus-transfer without holding a back-reference.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Sets the focused flag on this port.
    ///
    /// DIVERGED: see `is_focused`.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
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
    pub fn SetViewPosSize(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.home_x = x;
        self.home_y = y;
        self.home_width = w;
        self.home_height = h;
        // pixel_tallness unchanged — popup inherits home pixel tallness
    }
}
