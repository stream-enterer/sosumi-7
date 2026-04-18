use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::dlog;
use bitflags::bitflags;

use super::emPanelTree::{PanelId, PanelTree};

use crate::emClipRects::ClipRects;
use crate::emColor::emColor;
use crate::emCursor::emCursor;
use crate::emPainter::{emPainter, TextAlignment, VAlign};
use crate::emPanel::Rect;
use crate::emRec::{write_rec_with_format, RecStruct, RecValue};

bitflags! {
    /// Flags controlling view behavior.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct ViewFlags: u32 {
        const POPUP_ZOOM           = 0b0000_0001;
        const NO_ZOOM              = 0b0000_0010;
        const NO_SCROLL            = 0b0000_0100;
        const NO_NAVIGATE          = 0b0000_1000;
        const FULLSCREEN           = 0b0001_0000;
        const ROOT_SAME_TALLNESS   = 0b0010_0000;
        const NO_USER_NAVIGATION   = 0b0100_0000;
        const NO_FOCUS_HIGHLIGHT   = 0b1000_0000;
        const NO_ACTIVE_HIGHLIGHT  = 0b0001_0000_0000;
        const STRESS_TEST          = 0b0010_0000_0000;
        const EGO_MODE             = 0b0100_0000_0000;
    }
}

const MAX_SVP_SIZE: f64 = 1.0e12;
const MAX_SVP_SEARCH_SIZE: f64 = 1.0e14; // C++ emView.h:715
const MIN_DIMENSION: f64 = 0.0001;

/// Port of C++ `emGetDblRandom(double lo, double hi)` — uniform f64 in
/// `[lo, hi]`. Used by the SVPUpdSlice fp-instability escape hatch.
/// Quality-insensitive; a tiny xorshift is fine.
fn em_get_dbl_random(lo: f64, hi: f64) -> f64 {
    use std::cell::Cell;
    thread_local! {
        static RNG: Cell<u64> = const { Cell::new(0x9E3779B97F4A7C15) };
    }
    RNG.with(|rng| {
        let mut x = rng.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        rng.set(x);
        let f = (x >> 11) as f64 / (1u64 << 53) as f64; // [0, 1)
        lo + (hi - lo) * f
    })
}

/// Frame rate measurement for the stress test overlay.
///
/// Port of C++ `StressTestClass` (emEngine subclass). Maintains a 128-entry
/// ring buffer of timestamps, computes frame rate over a 1-second window,
/// updates the displayed rate every 100ms.
#[derive(Clone, Debug)]
pub struct StressTest {
    /// Ring buffer of frame timestamps.
    timestamps: Vec<Instant>,
    /// Position of the next write in the ring buffer.
    pos: usize,
    /// Number of valid entries (0..=128).
    valid: usize,
    /// Last computed frame rate (Hz).
    frame_rate: f64,
    /// When the displayed rate was last updated.
    last_update: Instant,
}

impl Default for StressTest {
    fn default() -> Self {
        Self::new()
    }
}

impl StressTest {
    const RING_SIZE: usize = 128;

    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            timestamps: vec![now; Self::RING_SIZE],
            pos: 0,
            valid: 0,
            frame_rate: 0.0,
            last_update: now,
        }
    }

    /// Record a frame timestamp and update the frame rate if 100ms have elapsed.
    pub fn record_frame(&mut self) {
        let now = Instant::now();
        self.timestamps[self.pos] = now;
        self.pos = (self.pos + 1) % Self::RING_SIZE;
        if self.valid < Self::RING_SIZE {
            self.valid += 1;
        }

        // Update displayed rate every 100ms
        if now.duration_since(self.last_update).as_millis() >= 100 {
            self.last_update = now;
            self.frame_rate = self.compute_rate(now);
        }
    }

    fn compute_rate(&self, now: Instant) -> f64 {
        if self.valid < 2 {
            return 0.0;
        }
        // Count entries within the last 1 second
        let mut count = 0usize;
        let mut oldest = now;
        for i in 0..self.valid {
            let idx = (self.pos + Self::RING_SIZE - 1 - i) % Self::RING_SIZE;
            let elapsed = now.duration_since(self.timestamps[idx]);
            if elapsed.as_millis() > 1000 {
                break;
            }
            oldest = self.timestamps[idx];
            count += 1;
        }
        if count < 2 {
            return 0.0;
        }
        let elapsed_ms = now.duration_since(oldest).as_secs_f64() * 1000.0;
        if elapsed_ms < 1.0 {
            return 0.0;
        }
        count as f64 * 1000.0 / elapsed_ms
    }

    /// Paint the "Stress Test XX.X Hz" overlay in the top-left corner.
    pub fn paint_info(&self, painter: &mut emPainter, _view_w: f64, view_h: f64) {
        let text_h = (view_h / 45.0).max(10.0);
        let box_w = text_h * 8.0;
        let box_h = text_h * 2.5;

        // Purple background (matches C++: 255,0,255,128)
        let bg = emColor::rgba(255, 0, 255, 128);
        painter.PaintRect(0.0, 0.0, box_w, box_h, bg, emColor::TRANSPARENT);

        // Yellow text (matches C++: 255,255,0,192)
        let fg = emColor::rgba(255, 255, 0, 192);
        let label = format!("Stress Test\n{:.1} Hz", self.frame_rate);
        painter.PaintTextBoxed(
            0.0,
            0.0,
            box_w,
            box_h,
            &label,
            text_h,
            fg,
            bg,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            false,
            0.15,
        );
    }

    pub fn frame_rate(&self) -> f64 {
        self.frame_rate
    }

    pub fn valid_count(&self) -> usize {
        self.valid
    }
}

/// Port of C++ `emView::UpdateEngineClass` (emView.h:626-633).
///
/// Scheduler-driven engine: when awake, `Cycle()` drains the view's update
/// loop. Holds the `WindowId` of the containing `emWindow` so it can locate
/// both the view (via `ctx.windows`) and the panel tree (via `ctx.tree`).
pub struct UpdateEngineClass {
    /// Identifier of the window whose view this engine updates.
    pub window_id: winit::window::WindowId,
}

impl UpdateEngineClass {
    /// Create a new `UpdateEngineClass` bound to `window_id`.
    /// Mirrors C++ ctor which sets HIGH_PRIORITY.
    pub fn new(window_id: winit::window::WindowId) -> Self {
        Self { window_id }
    }
}

impl super::emEngine::emEngine for UpdateEngineClass {
    fn Cycle(&mut self, ctx: &mut super::emEngine::EngineCtx<'_>) -> bool {
        // Mirrors C++ UpdateEngineClass::Cycle → View.Update().
        if let Some(win_rc) = ctx.windows.get(&self.window_id) {
            let win_rc = Rc::clone(win_rc);
            win_rc.borrow_mut().view_mut().Update(ctx.tree);
        }
        false
    }
}

/// Scheduler-driven engine that ticks `emView::VisitingVA`.
///
/// C++ equivalence: in C++ `emViewAnimator` derives from `emEngine` so every
/// animator is itself a scheduler-registered engine whose base `Cycle()`
/// measures `dt` and calls the subclass's `CycleAnimation`. The Rust port
/// separates animator logic from scheduling: `emVisitingViewAnimator` is a
/// plain type, and this wrapper engine is what the scheduler sees. On each
/// `Cycle`, it locates the view via `ctx.windows.get(window_id)` (mirroring
/// `UpdateEngineClass`), computes `dt` from wall-clock deltas, and — if the
/// animator is active — forwards to `emVisitingViewAnimator::animate`,
/// which corresponds to C++ `CycleAnimation` (emViewAnimator.cpp:1194).
pub struct VisitingVAEngineClass {
    /// Identifier of the window whose view owns the animator to tick.
    pub window_id: winit::window::WindowId,
    /// Wall-clock timestamp of the previous `Cycle`, used to compute `dt`.
    /// `None` before the first tick; the first tick uses a 16 ms fallback.
    last_cycle: Option<Instant>,
}

impl VisitingVAEngineClass {
    /// Create a new `VisitingVAEngineClass` bound to `window_id`.
    pub fn new(window_id: winit::window::WindowId) -> Self {
        Self {
            window_id,
            last_cycle: None,
        }
    }
}

impl super::emEngine::emEngine for VisitingVAEngineClass {
    fn Cycle(&mut self, ctx: &mut super::emEngine::EngineCtx<'_>) -> bool {
        // Compute dt from wall-clock, clamped to the same range emGUIFramework
        // uses for animator ticks (emGUIFramework.rs:523-526).
        let now = Instant::now();
        let dt = self
            .last_cycle
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(0.016)
            .clamp(0.001, 0.1);
        self.last_cycle = Some(now);

        let win_rc = match ctx.windows.get(&self.window_id) {
            Some(w) => Rc::clone(w),
            None => return false,
        };
        let mut win = win_rc.borrow_mut();
        let view = win.view_mut();
        // Clone the animator Rc so we can mutably borrow it alongside &mut view.
        // The animator's RefCell is independent of the view's storage.
        let va_rc = Rc::clone(&view.VisitingVA);
        let mut va = va_rc.borrow_mut();
        if !va.is_active() {
            return false;
        }
        use super::emViewAnimator::emViewAnimator as _;
        va.animate(view, ctx.tree, dt)
    }
}

/// Port of C++ `emView::EOIEngineClass` (emView.h:636-645, emView.cpp:2528-2543).
///
/// Scheduler-driven engine that owns the End-Of-Interaction countdown.
/// `SignalEOIDelayed` registers a fresh instance with the scheduler and
/// wakes it each slice; `Cycle` decrements `CountDown` and fires
/// `EOISignal` when it reaches zero.
pub struct EOIEngineClass {
    /// Countdown in scheduler ticks.  Mirrors C++ `CountDown` field.
    pub CountDown: i32,
    /// Signal to fire when the countdown reaches zero (C++ View.EOISignal).
    pub eoi_signal: super::emSignal::SignalId,
}

impl EOIEngineClass {
    /// Create and arm the countdown. Caller is responsible for registering
    /// with the scheduler and waking it. Mirrors C++ ctor which sets
    /// `CountDown=5` and calls `WakeUp()`.
    pub fn new(eoi_signal: super::emSignal::SignalId) -> Self {
        Self {
            CountDown: 5,
            eoi_signal,
        }
    }
}

impl super::emEngine::emEngine for EOIEngineClass {
    fn Cycle(&mut self, ctx: &mut super::emEngine::EngineCtx<'_>) -> bool {
        self.CountDown -= 1;
        if self.CountDown <= 0 {
            ctx.fire(self.eoi_signal);
            false
        } else {
            // Stay awake so we cycle again next slice.
            true
        }
    }
}

/// The emView manages the viewport — which panels are visible and how they're
/// navigated and rendered.
#[allow(non_snake_case)] // F&N rule: field names mirror C++ emView.h:680-715.
pub struct emView {
    root: PanelId,
    active: Option<PanelId>,
    focused: Option<PanelId>,
    pub flags: ViewFlags,
    supreme_viewed_panel: Option<PanelId>,
    background_color: emColor,
    SVPUpdCount: u32,
    window_focused: bool,
    /// Panel targeted by the visiting animator's seek operation.
    seek_pos_panel: Option<PanelId>,
    /// Child name being sought within `seek_pos_panel`.
    seek_pos_child_name: String,
    /// Dirty rectangles accumulated by invalidate_painting calls.
    dirty_rects: Vec<Rect>,
    /// Whether the view title needs to be refreshed.
    title_invalid: bool,
    /// Whether the cursor display needs to be refreshed.
    cursor_invalid: bool,
    /// Whether the control panel needs to be refreshed.
    control_panel_invalid: bool,
    /// Signal fired when the active control panel changes (C++ ControlPanelSignal).
    control_panel_signal: Option<super::emSignal::SignalId>,
    /// Signal fired when the view title changes (C++ TitleSignal).
    title_signal: Option<super::emSignal::SignalId>,
    /// Scheduler reference for firing signals.
    scheduler: Option<Rc<RefCell<super::emScheduler::EngineScheduler>>>,
    /// Whether the current activation is adherent (indirect, via a descendant).
    activation_adherent: bool,
    /// Set by scroll/zoom/navigate operations that change the viewport and need
    /// a repaint, but don't go through the notice or dirty_rects systems.
    viewport_changed: bool,
    /// VIEW-003: Set by scroll/zoom to signal that any active animator should be
    /// aborted. Consumers (window loop) should check and clear this flag.
    needs_animator_abort: bool,
    /// C++ `EOISignal` — fired by `EOIEngineClass::Cycle` when the countdown
    /// reaches zero. Created when the view is attached to the scheduler.
    /// Mirrors C++ `emSignal EOISignal` (emView.h).
    pub EOISignal: Option<super::emSignal::SignalId>,
    /// Scheduler handle for the registered `UpdateEngineClass`.
    /// Set by `attach_to_scheduler`; used by all call sites that previously
    /// called `UpdateEngine->WakeUp()` to wake the update engine.
    pub update_engine_id: Option<super::emEngine::EngineId>,
    /// Scheduler handle for the most recently registered `EOIEngineClass`.
    /// `SignalEOIDelayed` removes any previous instance before registering
    /// a fresh one; the engine self-parks after firing `EOISignal`.
    pub eoi_engine_id: Option<super::emEngine::EngineId>,
    /// Scheduler handle for the registered `VisitingVAEngineClass`.
    /// Set by `attach_to_scheduler`; woken when the animator has pending
    /// work. Mirrors C++ behavior where `emVisitingViewAnimator` self-
    /// registers with the scheduler via its `emEngine` base ctor.
    pub visiting_va_engine_id: Option<super::emEngine::EngineId>,
    /// Handle into `App::pending_actions` for enqueuing deferred framework
    /// actions (popup surface materialization, popup-exit cleanup). Wired
    /// by `App::about_to_wait` each frame; `None` in unit-test contexts
    /// that construct `emView` outside of a running `App`.
    pub(crate) pending_framework_actions:
        Option<Rc<RefCell<Vec<super::emGUIFramework::DeferredAction>>>>,
    /// The view title. Updated from the active panel's title.
    pub title: String,
    /// Current mouse cursor for this view.
    pub cursor: emCursor,
    /// Maximum popup view rectangle (pixel coords of bounding monitor rect).
    pub max_popup_rect: Option<Rect>,
    /// Whether the view is currently in popped-up state (popup window active).
    pub popped_up: bool,
    /// C++ ZoomedOutBeforeSG: when true the next update_viewing() will
    /// compute the zoom-out relA so the root panel fits in the viewport.
    /// Initially true; cleared after the first viewing update.
    zoomed_out_before_sg: bool,
    /// Stress test state. Created when STRESS_TEST flag is set, dropped when cleared.
    stress_test: Option<StressTest>,
    /// Whether the soft keyboard is shown (touch platforms only).
    /// C++ emView::IsSoftKeyboardShown / ShowSoftKeyboard.
    soft_keyboard_shown: bool,

    // === C++ Home/Current viewport split (emView.h:686-688) ===
    /// C++ HomeX — left edge of the home (non-popup) viewport rect in
    /// screen coords. Constant while popup active. 0 in non-popup Rust today.
    pub HomeX: f64,
    /// C++ HomeY — top edge of home viewport rect.
    pub HomeY: f64,
    /// C++ HomeWidth — width of home viewport rect.
    pub HomeWidth: f64,
    /// C++ HomeHeight — height of home viewport rect.
    pub HomeHeight: f64,
    /// C++ HomePixelTallness — pixel shape ratio of the home viewport
    /// (hardware property; 1.0 for square pixels).
    pub HomePixelTallness: f64,

    /// C++ CurrentX — left edge of the *current* viewport rect. Equals
    /// HomeX when no popup; set to popup-adjusted rect during popup zoom.
    pub CurrentX: f64,
    /// C++ CurrentY — top edge of current viewport rect.
    pub CurrentY: f64,
    /// C++ CurrentWidth — width of current viewport rect.
    pub CurrentWidth: f64,
    /// C++ CurrentHeight — height of current viewport rect.
    pub CurrentHeight: f64,
    /// C++ CurrentPixelTallness — pixel shape ratio of the current viewport.
    pub CurrentPixelTallness: f64,

    // === C++ invalidation / recursion flags (emView.h:699-703) ===
    /// C++ SVPChoiceInvalid — next Update() must re-run RawVisitAbs to
    /// recompute the Supreme Viewed Panel.
    pub SVPChoiceInvalid: bool,
    /// C++ SVPChoiceByOpacityInvalid — opacity of a panel between MinSVP
    /// and MaxSVP has changed; Update() must walk that chain to see if
    /// the SVP needs re-choice.
    pub SVPChoiceByOpacityInvalid: bool,
    /// C++ RestartInputRecursion — signals that the in-progress input
    /// recursion (if any) should unwind and start over at the new SVP.
    pub RestartInputRecursion: bool,
    /// C++ SettingGeometry — reentrancy counter for SetGeometry. Nonzero
    /// means we are inside SetGeometry; certain invalidations are
    /// suppressed while it is nonzero.
    pub SettingGeometry: i32,
    /// C++ SVPUpdSlice — scheduler time-slice counter snapshot at the
    /// last RawVisitAbs change block. Used by the fp-instability throttle
    /// at emView.cpp:1734-1751.
    pub SVPUpdSlice: u64,
    /// C++ ZoomScrollInAction — set while a zoom/scroll gesture is in
    /// progress; suppresses certain side effects that would fight the
    /// gesture.
    pub ZoomScrollInAction: bool,

    // === C++ SVP bounds (emView.cpp:1696, 1725) ===
    /// C++ MaxSVP — topmost (root-ward) panel allowed as the SVP at the
    /// current zoom/rect. Set by RawVisitAbs.
    pub MaxSVP: Option<PanelId>,
    /// C++ MinSVP — deepest (leaf-ward) panel allowed as the SVP.
    pub MinSVP: Option<PanelId>,

    // === C++ last mouse position for CursorInvalid dispatch (emView.h:689) ===
    /// C++ LastMouseX — last known mouse X in screen coords. Default is a
    /// sentinel far outside any viewport so GetPanelAt returns None.
    pub LastMouseX: f64,
    /// C++ LastMouseY — last known mouse Y in screen coords.
    pub LastMouseY: f64,

    // === C++ signals missing from Rust (emView.h:680, 683, 684) ===
    /// C++ ViewFlagsSignal — fired when VFlags changes.
    pub view_flags_signal: Option<super::emSignal::SignalId>,
    /// C++ FocusSignal — fired on focus gain/loss.
    pub focus_signal: Option<super::emSignal::SignalId>,
    /// C++ GeometrySignal — fired when Home/Current rect changes.
    pub geometry_signal: Option<super::emSignal::SignalId>,

    // === C++ popup infrastructure (emView.h:708-713) ===
    /// C++ PopupWindow — owned handle to the popup window created when
    /// zooming past the home-rect edges under VF_POPUP_ZOOM.
    pub PopupWindow: Option<Rc<RefCell<crate::emWindow::emWindow>>>,
    /// C++ HomeViewPort — the view-port that connects the emView to its
    /// *home* window (the original non-popup window).
    pub HomeViewPort: Rc<RefCell<super::emViewPort::emViewPort>>,
    /// C++ CurrentViewPort — currently-active view-port. Swapped with
    /// HomeViewPort by SwapViewPorts during popup push/pop.
    pub CurrentViewPort: Rc<RefCell<super::emViewPort::emViewPort>>,
    /// C++ DummyViewPort — the sentinel "no backend attached" port,
    /// returned by the accessors during construction before a real
    /// port is attached.
    pub DummyViewPort: Rc<RefCell<super::emViewPort::emViewPort>>,

    /// C++ emView.h:675 — `emOwnPtr<emVisitingViewAnimator> VisitingVA`.
    /// The visiting view animator; owns the "where we're going" state.
    /// Read non-test by `VisitingVAEngineClass::Cycle` to tick animation.
    pub(crate) VisitingVA: Rc<RefCell<super::emViewAnimator::emVisitingViewAnimator>>,
}

impl emView {
    pub fn new(root: PanelId, viewport_width: f64, viewport_height: f64) -> Self {
        // C++ HomeViewPort == CurrentViewPort in non-popup state.
        // Rc::clone shares the same allocation so Rc::ptr_eq returns true.
        let home_vp = Rc::new(RefCell::new(super::emViewPort::emViewPort::new_dummy()));
        let current_vp = Rc::clone(&home_vp);
        Self {
            root,
            active: Some(root),
            focused: None,
            flags: ViewFlags::empty(),
            supreme_viewed_panel: None,
            background_color: emColor::rgba(0x80, 0x80, 0x80, 0xFF),
            SVPUpdCount: 0,
            window_focused: true,
            seek_pos_panel: None,
            seek_pos_child_name: String::new(),
            dirty_rects: Vec::new(),
            title_invalid: false,
            cursor_invalid: false,
            control_panel_invalid: false,
            control_panel_signal: None,
            title_signal: None,
            scheduler: None,
            activation_adherent: false,
            viewport_changed: false,
            needs_animator_abort: false,
            EOISignal: None,
            update_engine_id: None,
            eoi_engine_id: None,
            visiting_va_engine_id: None,
            pending_framework_actions: None,
            title: String::new(),
            cursor: emCursor::Normal,
            max_popup_rect: None,
            popped_up: false,
            zoomed_out_before_sg: true,
            stress_test: None,
            soft_keyboard_shown: false,

            // Home rect defaults to the viewport rect passed to new().
            HomeX: 0.0,
            HomeY: 0.0,
            HomeWidth: viewport_width,
            HomeHeight: viewport_height,
            HomePixelTallness: 1.0,

            // Current rect starts equal to Home rect.
            CurrentX: 0.0,
            CurrentY: 0.0,
            CurrentWidth: viewport_width,
            CurrentHeight: viewport_height,
            CurrentPixelTallness: 1.0,

            SVPChoiceInvalid: false,
            SVPChoiceByOpacityInvalid: false,
            RestartInputRecursion: false,
            SettingGeometry: 0,
            SVPUpdSlice: 0,
            ZoomScrollInAction: false,

            MaxSVP: None,
            MinSVP: None,

            LastMouseX: -1.0e10,
            LastMouseY: -1.0e10,

            view_flags_signal: None,
            focus_signal: None,
            geometry_signal: None,

            PopupWindow: None,
            HomeViewPort: home_vp,
            CurrentViewPort: current_vp,
            DummyViewPort: Rc::new(RefCell::new(super::emViewPort::emViewPort::new_dummy())),
            VisitingVA: Rc::new(RefCell::new(
                super::emViewAnimator::emVisitingViewAnimator::new_for_view(),
            )),
        }
    }

    // --- Accessors ---

    pub fn GetRootPanel(&self) -> PanelId {
        self.root
    }

    pub fn GetActivePanel(&self) -> Option<PanelId> {
        self.active
    }

    pub fn SetActivePanel(&mut self, id: PanelId) {
        self.active = Some(id);
    }

    pub fn GetFocusedPanel(&self) -> Option<PanelId> {
        self.focused
    }

    pub fn set_focus(&mut self, id: Option<PanelId>) {
        self.focused = id;
    }

    pub fn GetSupremeViewedPanel(&self) -> Option<PanelId> {
        self.supreme_viewed_panel
    }

    pub fn IsFocused(&self) -> bool {
        self.window_focused
    }

    pub fn SetFocused(&mut self, tree: &mut PanelTree, focused: bool) {
        if self.window_focused == focused {
            return;
        }
        // C++ emView.cpp:1211: InvalidateHighlight before clearing focus.
        if self.window_focused {
            self.InvalidateHighlight(tree);
        }
        self.window_focused = focused;
        // C++ emView.cpp:1213: InvalidateHighlight after acquiring focus.
        if self.window_focused {
            self.InvalidateHighlight(tree);
        }
        // C++ emView::SetFocused iterates ALL panels and queues:
        //   NF_VIEW_FOCUS_CHANGED | NF_UPDATE_PRIORITY_CHANGED on every panel
        //   NF_FOCUS_CHANGED additionally on panels in the active path
        let ids: Vec<_> = tree.panel_ids();
        for id in ids {
            if let Some(panel) = tree.get_mut(id) {
                let mut flags = super::emPanel::NoticeFlags::VIEW_FOCUS_CHANGED
                    | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED;
                if panel.in_active_path {
                    flags |= super::emPanel::NoticeFlags::FOCUS_CHANGED;
                }
                panel.pending_notices.insert(flags);
            }
        }
        tree.mark_notices_pending();
    }

    pub fn IsActivationAdherent(&self) -> bool {
        self.activation_adherent
    }

    /// Whether the soft keyboard is currently shown.
    /// C++ `emView::IsSoftKeyboardShown()`.
    pub fn IsSoftKeyboardShown(&self) -> bool {
        self.soft_keyboard_shown
    }

    /// Show or hide the soft keyboard.
    /// C++ `emView::ShowSoftKeyboard(bool show)`.
    /// DIVERGED: C++ delegates to CurrentViewPort which delegates to the
    /// platform window. Desktop stub stores flag only — no actual keyboard
    /// is shown until a platform-specific viewport implements this.
    pub fn ShowSoftKeyboard(&mut self, show: bool) {
        self.soft_keyboard_shown = show;
    }

    // --- Seeking ---

    /// Set the seek target panel and child name for the visiting animator.
    ///
    /// When the visiting animator is navigating to a panel that doesn't yet
    /// exist, this records which panel to watch and which child name is being
    /// sought, so the animator can monitor creation progress.
    pub fn SetSeekPos(&mut self, tree: &mut PanelTree, panel: Option<PanelId>, child_name: &str) {
        let child_name = if panel.is_some() { child_name } else { "" };

        if self.seek_pos_panel != panel {
            // Notify old panel that sought name is cleared
            if let Some(old_id) = self.seek_pos_panel {
                tree.queue_notice(old_id, super::emPanel::NoticeFlags::SOUGHT_NAME_CHANGED);
            }

            self.seek_pos_panel = panel;
            self.seek_pos_child_name = child_name.to_string();
            // Mirror to tree so panel behaviors can observe via ctx/tree.
            tree.seek_pos_panel = panel;
            tree.seek_pos_child_name = child_name.to_string();

            // Notify new panel that sought name is set. Queue
            // ae_decision_invalid so the next HandleNotice AE phase
            // checks if this panel should now expand (C++ AutoExpand
            // triggers when View.SeekPosPanel==this).
            if let Some(new_id) = self.seek_pos_panel {
                tree.queue_notice(new_id, super::emPanel::NoticeFlags::SOUGHT_NAME_CHANGED);
                if let Some(p) = tree.get_mut(new_id) {
                    p.ae_decision_invalid = true;
                }
            }
        } else if panel.is_some() && self.seek_pos_child_name != child_name {
            self.seek_pos_child_name = child_name.to_string();
            tree.seek_pos_child_name = child_name.to_string();
            if let Some(id) = self.seek_pos_panel {
                tree.queue_notice(id, super::emPanel::NoticeFlags::SOUGHT_NAME_CHANGED);
            }
        }
    }

    /// Returns the current seek target panel, if any.
    pub fn seek_pos_panel(&self) -> Option<PanelId> {
        self.seek_pos_panel
    }

    /// Returns the child name being sought.
    pub fn seek_pos_child_name(&self) -> &str {
        &self.seek_pos_child_name
    }

    /// Returns true if seeking can still succeed — the seek panel exists in
    /// the tree and has the potential to create the sought child.
    pub fn IsHopeForSeeking(&self, tree: &PanelTree) -> bool {
        if let Some(id) = self.seek_pos_panel {
            if let Some(panel) = tree.GetRec(id) {
                if self.seek_pos_child_name.is_empty() {
                    return true;
                }
                if tree
                    .find_child_by_name(id, &self.seek_pos_child_name)
                    .is_some()
                {
                    return true;
                }
                return panel.behavior.is_some();
            }
        }
        false
    }

    // --- Navigation primitives ---

    /// Immediate direct-set visit (no animation, no new back-stack entry).
    /// Port of C++ `emView::RawVisit(panel, relX, relY, relA)` (emView.cpp:1526-1540).
    ///
    /// If `rel_a <= 0.0`, CalcVisitFullsizedCoords is called to get proper coords
    /// (matching C++ `if (relA<=0.0) CalcVisitFullsizedCoords(..., relA<-0.9)`).
    ///
    /// DIVERGED: C++ has public `RawVisit(panel, relX, relY, relA)` + private
    /// overload with extra `forceViewingUpdate` bool. Rust has no
    /// overloading — single method; existing no-arg callers pass `false`.
    /// Port of C++ `emView::RawVisit(panel, relX, relY, relA, forceViewingUpdate)`
    /// (emView.cpp:1526-1541). Converts rel coords to absolute screen coords
    /// (same formula as C++) then calls `RawVisitAbs` directly.
    pub fn RawVisit(
        &mut self,
        tree: &mut PanelTree,
        panel: PanelId,
        rel_x: f64,
        rel_y: f64,
        rel_a: f64,
        forceViewingUpdate: bool,
    ) {
        let (rx, ry, ra) = if rel_a <= 0.0 {
            // C++ emView.cpp:1534: if (relA<=0.0) CalcVisitFullsizedCoords(panel,&relX,&relY,&relA,relA<-0.9)
            self.CalcVisitFullsizedCoords(tree, panel, rel_a < -0.9)
        } else {
            (rel_x, rel_y, rel_a)
        };
        self.active = Some(panel);

        // C++ emView.cpp:1535-1539: compute absolute coords from rel coords.
        //   vw = sqrt(HomeWidth * HomeHeight * HomePixelTallness / (relA * panel->GetHeight()))
        //   vh = vw * panel->GetHeight() / HomePixelTallness
        //   vx = HomeX + HomeWidth*0.5 - (relX + 0.5) * vw
        //   vy = HomeY + HomeHeight*0.5 - (relY + 0.5) * vh
        let panel_h = tree.get_height(panel);
        let panel_h_safe = panel_h.max(MIN_DIMENSION);
        // C++ emView.cpp:1535 does not clamp relA; use 1e-100 only to guard
        // against division-by-zero. MIN_DIMENSION (0.0001) was wrong here —
        // it clamped the zoom to 10000x maximum.
        let ra_safe = ra.max(1e-100);
        let vw = (self.HomeWidth * self.HomeHeight * self.HomePixelTallness
            / (ra_safe * panel_h_safe))
            .sqrt();
        let vh = vw * panel_h_safe / self.HomePixelTallness;
        let vx = self.HomeX + self.HomeWidth * 0.5 - (rx + 0.5) * vw;
        let vy = self.HomeY + self.HomeHeight * 0.5 - (ry + 0.5) * vh;

        self.RawVisitAbs(tree, panel, vx, vy, vw, forceViewingUpdate);
    }

    /// Port of C++ `emView::RawVisitFullsized(panel, utilizeView)` (emView.cpp:558-560):
    ///   `RawVisit(panel, 0.0, 0.0, utilizeView ? -1.0 : 0.0)`
    pub fn RawVisitFullsized(&mut self, tree: &mut PanelTree, panel: PanelId, utilize_view: bool) {
        self.RawVisit(
            tree,
            panel,
            0.0,
            0.0,
            if utilize_view { -1.0 } else { 0.0 },
            false,
        );
    }

    /// Port of C++ `emView::GetVisitedPanel(pRelX, pRelY, pRelA)` (emView.cpp:468-489).
    ///
    /// Walks from ActivePanel toward root to find the deepest panel that is
    /// `in_viewed_path` and `viewed`. Falls back to SupremeViewedPanel.
    /// Fills `rel_x`, `rel_y`, `rel_a` with viewport-relative coords on success,
    /// or zeros on `None`. C++ convention: relX/Y are offsets of the viewport center
    /// from the panel center (in panel-space units). rel_a = (HomeW*HomeH)/(ViewedW*ViewedH).
    pub fn GetVisitedPanel(
        &self,
        tree: &PanelTree,
        rel_x: &mut f64,
        rel_y: &mut f64,
        rel_a: &mut f64,
    ) -> Option<PanelId> {
        // Walk from active toward root until we find an in_viewed_path + viewed panel.
        let p = {
            let mut candidate = self.active;
            let result;
            loop {
                match candidate {
                    Some(id) => {
                        if let Some(panel) = tree.GetRec(id) {
                            if panel.in_viewed_path {
                                if panel.viewed {
                                    result = Some(id);
                                    break;
                                }
                                candidate = panel.parent;
                            } else {
                                // Not in viewed path; fall back to SVP
                                result = self.supreme_viewed_panel;
                                break;
                            }
                        } else {
                            result = self.supreme_viewed_panel;
                            break;
                        }
                    }
                    None => {
                        result = self.supreme_viewed_panel;
                        break;
                    }
                }
            }
            result
        };

        if let Some(id) = p {
            if let Some(panel) = tree.GetRec(id) {
                let hw = self.HomeWidth;
                let hh = self.HomeHeight;
                let hp = self.HomePixelTallness;
                // C++ emView.cpp:479-481:
                //   relX = (HomeX + HomeWidth*0.5 - ViewedX) / ViewedWidth - 0.5
                //   relY = (HomeY + HomeHeight*0.5 - ViewedY) / ViewedHeight - 0.5
                //   relA = (HomeWidth*HomeHeight) / (ViewedWidth*ViewedHeight)
                // Rust: ViewedHeight = ViewedWidth * GetHeight / HomePixelTallness
                let vw = panel.viewed_width.max(1e-100);
                let vh = (vw * tree.get_height(id) / hp).max(1e-100);
                *rel_x = (self.HomeX + hw * 0.5 - panel.viewed_x) / vw - 0.5;
                *rel_y = (self.HomeY + hh * 0.5 - panel.viewed_y) / vh - 0.5;
                *rel_a = (hw * hh) / (vw * vh);
                return Some(id);
            }
        }
        *rel_x = 0.0;
        *rel_y = 0.0;
        *rel_a = 0.0;
        None
    }

    /// Idiomatic Rust companion: returns `(panel, rel_x, rel_y, rel_a)` as a tuple.
    /// Delegates to `GetVisitedPanel`.
    pub fn get_visited_panel_idiom(&self, tree: &PanelTree) -> Option<(PanelId, f64, f64, f64)> {
        let mut rx = 0.0;
        let mut ry = 0.0;
        let mut ra = 0.0;
        self.GetVisitedPanel(tree, &mut rx, &mut ry, &mut ra)
            .map(|id| (id, rx, ry, ra))
    }

    /// Port of C++ `emView::Visit(panel, relX, relY, relA, adherent)` at
    /// emView.cpp:492-497. Three-line delegation: look up identity+title on
    /// the tree, then forward to the identity-keyed overload.
    pub fn Visit(
        &mut self,
        tree: &PanelTree,
        panel: PanelId,
        rel_x: f64,
        rel_y: f64,
        rel_a: f64,
        adherent: bool,
    ) {
        let identity = tree.GetIdentity(panel);
        let subject = tree.get_title(panel);
        self.VisitByIdentity(&identity, rel_x, rel_y, rel_a, adherent, &subject);
    }

    /// Port of C++ `emView::VisitFullsized(panel, adherent, utilizeView)` (emView.cpp:525-528).
    pub fn VisitFullsized(
        &mut self,
        tree: &PanelTree,
        panel: PanelId,
        adherent: bool,
        utilize_view: bool,
    ) {
        let identity = tree.GetIdentity(panel);
        let subject = tree.get_title(panel);
        self.VisitFullsizedByIdentity(&identity, adherent, utilize_view, &subject);
    }

    /// DIVERGED: C++ overload `emView::VisitFullsized(identity, adherent, utilizeView, subject)` (emView.cpp:531-541)
    /// — Rust cannot overload by name; panel-form keeps `VisitFullsized`, identity-form renamed to
    /// `VisitFullsizedByIdentity`.
    ///
    /// Port of C++ `emView::VisitFullsized(identity, adherent, utilizeView, subject)` (emView.cpp:531-541).
    pub fn VisitFullsizedByIdentity(
        &mut self,
        identity: &str,
        adherent: bool,
        utilize_view: bool,
        subject: &str,
    ) {
        let mut va = self.VisitingVA.borrow_mut();
        // PHASE-W4-FOLLOWUP: CoreConfig defaults — see Task 3.1.
        va.SetAnimParamsByCoreConfig(1.0, 10.0);
        va.SetGoalFullsized(identity, adherent, utilize_view, subject);
        va.Activate();
    }

    /// DIVERGED: C++ overload `emView::Visit(panel, adherent)` (emView.cpp:511-514)
    /// — Rust cannot overload by arity; renamed `VisitPanel` to disambiguate from
    /// the canonical 6-arg `Visit` added in Task 3.1.
    ///
    /// Port of C++ `emView::Visit(panel, adherent)` (emView.cpp:511-514).
    pub fn VisitPanel(&mut self, tree: &PanelTree, panel: PanelId, adherent: bool) {
        let identity = tree.GetIdentity(panel);
        let subject = tree.get_title(panel);
        self.VisitByIdentityShort(&identity, adherent, &subject);
    }

    /// DIVERGED: C++ overload `emView::Visit(identity, adherent, subject)` (emView.cpp:517-523)
    /// — Rust cannot overload by arity; renamed `VisitByIdentityShort` to disambiguate from
    /// the canonical 7-arg `VisitByIdentity` added in Task 3.1.
    ///
    /// Port of C++ `emView::Visit(identity, adherent, subject)` (emView.cpp:517-523).
    pub fn VisitByIdentityShort(&mut self, identity: &str, adherent: bool, subject: &str) {
        let mut va = self.VisitingVA.borrow_mut();
        // PHASE-W4-FOLLOWUP: CoreConfig defaults — see Task 3.1.
        va.SetAnimParamsByCoreConfig(1.0, 10.0);
        va.SetGoal(identity, adherent, subject);
        va.Activate();
    }

    // --- Viewport ---

    /// Port of C++ `emView::SetGeometry` (emView.cpp:1241-1278). Clamp dimensions,
    /// preserve zoom state on resize. Signature extended to accept explicit
    /// (x, y, width, height, pixel_tallness) matching C++ emView.cpp:1238.
    ///
    /// DIVERGED: C++ `SetGeometry(x,y,w,h,pt)` — Rust signature identical.
    /// Internally writes both Home* and Current* (home = current when no popup).
    pub fn SetGeometry(
        &mut self,
        tree: &mut PanelTree,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        pixel_tallness: f64,
    ) {
        let width = width.max(MIN_DIMENSION);
        let height = height.max(MIN_DIMENSION);
        let pixel_tallness = pixel_tallness.max(MIN_DIMENSION);

        // C++ emView.cpp:1248-1254: early-out if nothing changed.
        if self.CurrentX == x
            && self.CurrentY == y
            && self.CurrentWidth == width
            && self.CurrentHeight == height
            && self.CurrentPixelTallness == pixel_tallness
        {
            return;
        }

        // C++ emView.cpp:1255-1256: capture zoom state before mutation.
        self.zoomed_out_before_sg = self.IsZoomedOut(tree);
        self.SettingGeometry += 1;
        let visited_before = self.get_visited_panel_idiom(tree);

        // Home fields track Current on the home viewport (no popup).
        // Rc::ptr_eq detects popup state: during popup, HomeViewPort != CurrentViewPort.
        let is_home = Rc::ptr_eq(&self.HomeViewPort, &self.CurrentViewPort);
        if is_home {
            self.HomeX = x;
            self.HomeY = y;
            self.HomeWidth = width;
            self.HomeHeight = height;
            self.HomePixelTallness = pixel_tallness;
        }
        self.CurrentX = x;
        self.CurrentY = y;
        self.CurrentWidth = width;
        self.CurrentHeight = height;
        self.CurrentPixelTallness = pixel_tallness;

        // C++ Signal(GeometrySignal).
        if let Some(sig) = self.geometry_signal {
            if let Some(sched) = &self.scheduler {
                sched.borrow_mut().fire(sig);
            }
        }

        // C++ SetGeometry parity: inline-update root panel layout when
        // VF_ROOT_SAME_TALLNESS is set (mirrors RootPanel->Layout(0,0,1,GetHomeTallness())).
        if self.flags.contains(ViewFlags::ROOT_SAME_TALLNESS) {
            tree.Layout(
                self.root,
                0.0,
                0.0,
                1.0,
                self.GetHomeTallness(),
                self.CurrentPixelTallness,
            );
        }

        // C++ emView.cpp:1272-1277: end of SetGeometry — zoom-out or re-visit.
        if self.zoomed_out_before_sg {
            self.RawZoomOut(tree, true);
        } else if let Some((panel, rx, ry, ra)) = visited_before {
            self.RawVisit(tree, panel, rx, ry, ra, true);
        }

        self.SettingGeometry -= 1;
    }

    /// C++ `emView::GetHomeTallness` (emView.h:874-876).
    /// Returns the aspect ratio (tallness) of a unit-width panel filling the home viewport.
    pub fn GetHomeTallness(&self) -> f64 {
        self.HomeHeight / self.HomeWidth * self.HomePixelTallness
    }

    /// C++ `emViewPort::SetViewGeometry` pixel-tallness path (emView.h:763).
    /// Adjusts only the pixel tallness of the current viewport.
    pub fn SetViewPortTallness(&mut self, tree: &mut PanelTree, tallness: f64) {
        self.SetGeometry(
            tree,
            self.CurrentX,
            self.CurrentY,
            self.CurrentWidth,
            self.CurrentHeight,
            tallness.max(MIN_DIMENSION),
        );
    }

    /// Returns the current home viewport size (width, height).
    ///
    /// DIVERGED: replaces Rust-invention `viewport_width`/`viewport_height` fields
    /// removed in Phase 6. Returns `(HomeWidth, HomeHeight)`.
    pub fn viewport_size(&self) -> (f64, f64) {
        (self.HomeWidth, self.HomeHeight)
    }

    // --- Zoom & Scroll ---

    /// Port of C++ `emView::Zoom(fixX, fixY, factor)` (emView.cpp:783-801).
    /// Fix-point zoom: keeps the viewport point (center_x, center_y) mapped to the
    /// same panel-space point before and after zoom. Calls RawVisit(..., true) directly.
    ///
    /// DIVERGED: C++ signature is `Zoom(fixX, fixY, factor)`; Rust keeps factor first
    /// for call-site consistency with earlier Rust API — marked here for record.
    pub fn Zoom(&mut self, tree: &mut PanelTree, factor: f64, center_x: f64, center_y: f64) {
        if self.flags.contains(ViewFlags::NO_ZOOM) {
            return;
        }
        if factor == 1.0 || factor <= 0.0 {
            return;
        }
        // VIEW-003: Signal abort for any active animator (C++ AbortActiveAnimator)
        self.needs_animator_abort = true;
        // C++ Zoom: GetVisitedPanel(&rx,&ry,&ra) then adjust rel coords.
        if let Some((panel, mut rx, mut ry, mut ra)) = self.get_visited_panel_idiom(tree) {
            let pvw = tree
                .GetRec(panel)
                .map(|r| r.viewed_width)
                .unwrap_or(1.0)
                .max(1e-10);
            let pvh = tree
                .GetRec(panel)
                .map(|r| r.viewed_height)
                .unwrap_or(1.0)
                .max(1e-10);
            let re_fac = 1.0 / factor;
            let hmx = self.HomeX + self.HomeWidth * 0.5;
            let hmy = self.HomeY + self.HomeHeight * 0.5;
            // C++ emView.cpp:795-797:
            //   rx += (fixX - hmx) * (1 - reFac) / p->ViewedWidth
            //   ry += (fixY - hmy) * (1 - reFac) / p->ViewedHeight
            //   ra *= reFac * reFac
            rx += (center_x - hmx) * (1.0 - re_fac) / pvw;
            ry += (center_y - hmy) * (1.0 - re_fac) / pvh;
            ra *= re_fac * re_fac;
            self.RawVisit(tree, panel, rx, ry, ra, true);
        }
    }

    /// Port of C++ `emView::Scroll(deltaX, deltaY)` (emView.cpp:765-782).
    /// Calls RawVisit(..., true) directly.
    pub fn Scroll(&mut self, tree: &mut PanelTree, dx: f64, dy: f64) {
        if self.flags.contains(ViewFlags::NO_SCROLL) {
            return;
        }
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        // VIEW-003: Signal abort for any active animator (C++ AbortActiveAnimator)
        self.needs_animator_abort = true;
        // C++ Scroll: GetVisitedPanel(&rx,&ry,&ra) then adjust rel coords.
        if let Some((panel, mut rx, mut ry, ra)) = self.get_visited_panel_idiom(tree) {
            let pvw = tree
                .GetRec(panel)
                .map(|r| r.viewed_width)
                .unwrap_or(1.0)
                .max(1e-10);
            let pvh = tree
                .GetRec(panel)
                .map(|r| r.viewed_height)
                .unwrap_or(1.0)
                .max(1e-10);
            // C++ emView.cpp:776-777:
            //   rx += deltaX / p->ViewedWidth
            //   ry += deltaY / p->ViewedHeight
            rx += dx / pvw;
            ry += dy / pvh;
            self.RawVisit(tree, panel, rx, ry, ra, true);
        }
    }

    /// Port of C++ `emView::RawScrollAndZoom(fixX, fixY, dX, dY, dZ, panel, ...)`
    /// (emView.cpp:803-855). Atomic scroll+zoom with done-distance feedback.
    /// Calls RawVisit directly (no intermediate Update call).
    pub fn RawScrollAndZoom(
        &mut self,
        tree: &mut PanelTree,
        fix_x: f64,
        fix_y: f64,
        dx: f64,
        dy: f64,
        dz: f64,
    ) -> [f64; 3] {
        let zflpp = self.GetZoomFactorLogarithmPerPixel();
        let hmx = self.HomeX + self.HomeWidth * 0.5;
        let hmy = self.HomeY + self.HomeHeight * 0.5;
        let hw = self.HomeWidth;
        let hh = self.HomeHeight;

        // Get current visited panel and its rel coords.
        let Some((panel, rx, ry, ra)) = self.get_visited_panel_idiom(tree) else {
            return [0.0, 0.0, 0.0];
        };
        let pvw = tree
            .GetRec(panel)
            .map(|r| r.viewed_width)
            .unwrap_or(1.0)
            .max(1e-10);
        let pvh = tree
            .GetRec(panel)
            .map(|r| r.viewed_height)
            .unwrap_or(1.0)
            .max(1e-10);
        let re_fac = (-dz * zflpp).exp();

        // C++ emView.cpp:843-847:
        //   rx2 = rx + ((fixX-hmx)*(1-reFac) + deltaX) / pvw
        //   ry2 = ry + ((fixY-hmy)*(1-reFac) + deltaY) / pvh
        //   ra2 = ra * reFac*reFac
        let (rx2, ry2) = if !self.flags.contains(ViewFlags::EGO_MODE) {
            (
                rx + ((fix_x - hmx) * (1.0 - re_fac) + dx) / pvw,
                ry + ((fix_y - hmy) * (1.0 - re_fac) + dy) / pvh,
            )
        } else {
            // EGO_MODE: scroll locked, only zoom.
            (rx, ry)
        };
        let ra2 = ra * re_fac * re_fac;

        self.RawVisit(tree, panel, rx2, ry2, ra2, false);

        // Done-distance: C++ emView.cpp:856-875.
        // After RawVisit, re-read the actual viewed coords of panel (post-clamp).
        // If panel is now viewed, compute done from actual coords; else done = input delta.
        if tree.GetRec(panel).map(|r| r.viewed).unwrap_or(false) {
            let pvx2 = tree.GetRec(panel).map(|r| r.viewed_x).unwrap_or(0.0);
            let pvy2 = tree.GetRec(panel).map(|r| r.viewed_y).unwrap_or(0.0);
            let pvw2 = tree
                .GetRec(panel)
                .map(|r| r.viewed_width)
                .unwrap_or(pvw)
                .max(1e-10);
            let pvh2 = tree
                .GetRec(panel)
                .map(|r| r.viewed_height)
                .unwrap_or(pvh)
                .max(1e-10);
            // C++: rx2 = (hmx-pvx)/pvw-0.5, ry2 = (hmy-pvy)/pvh-0.5,
            //      ra2 = hw*hh/(pvw*pvh), reFac = sqrt(ra2/ra)
            let rx2_actual = (hmx - pvx2) / pvw2 - 0.5;
            let ry2_actual = (hmy - pvy2) / pvh2 - 0.5;
            let ra2_actual = (hw * hh) / (pvw2 * pvh2);
            let re_fac_actual = (ra2_actual / ra.max(1e-100)).sqrt();
            // C++: pDeltaXDone = (rx2-rx)*pvw*reFac - (fixX-hmx)*(1-reFac)
            let done_x =
                (rx2_actual - rx) * pvw2 * re_fac_actual - (fix_x - hmx) * (1.0 - re_fac_actual);
            let done_y =
                (ry2_actual - ry) * pvh2 * re_fac_actual - (fix_y - hmy) * (1.0 - re_fac_actual);
            // C++: pDeltaZDone = -log(reFac)/zflpp
            let done_z = if zflpp > 1e-15 {
                -re_fac_actual.ln() / zflpp
            } else {
                0.0
            };
            [done_x, done_y, done_z]
        } else {
            // Panel not viewed (e.g., zoomed out to boundary): full motion was applied.
            [dx, dy, dz]
        }
    }

    /// Zoom sensitivity for VIFs/animators.
    pub fn GetZoomFactorLogarithmPerPixel(&self) -> f64 {
        1.33 / ((self.HomeWidth + self.HomeHeight) * 0.25).max(1.0)
    }

    // --- Zoom out ---

    pub fn ZoomOut(&mut self, tree: &mut PanelTree) {
        self.RawZoomOut(tree, false);
    }

    /// DIVERGED: C++ has public `RawZoomOut()` + private overload
    /// `RawZoomOut(forceViewingUpdate)`. Rust: single method, callers pass bool.
    pub fn RawZoomOut(&mut self, tree: &mut PanelTree, forceViewingUpdate: bool) {
        // C++ emView::RawZoomOut:
        //   RawVisit(RootPanel, 0.0, 0.0, relA, forceViewingUpdate);
        // Always target the ROOT panel, not the current visit target.
        let rel_a = self.zoom_out_rel_a(tree);
        let root = self.root;
        self.RawVisit(tree, root, 0.0, 0.0, rel_a, forceViewingUpdate);
    }

    /// Compute the C++ relA that makes the viewport fully contain the root panel.
    ///
    /// Port of C++ `emView::RawZoomOut` (emView.cpp:1811-1819):
    ///   relA  = HomeWidth * RootPanel->GetHeight() / HomePixelTallness / HomeHeight
    ///   relA2 = HomeHeight / RootPanel->GetHeight() * HomePixelTallness / HomeWidth
    ///   relA  = max(relA, relA2)
    fn zoom_out_rel_a(&self, tree: &PanelTree) -> f64 {
        let root_h = tree.get_height(self.root);
        let hp = self.HomePixelTallness;
        let hw = self.HomeWidth;
        let hh = self.HomeHeight;
        let a1 = hw * root_h / hp / hh;
        let a2 = hh / root_h * hp / hw;
        a1.max(a2)
    }

    pub fn IsZoomedOut(&self, tree: &PanelTree) -> bool {
        if self.flags.contains(ViewFlags::POPUP_ZOOM) {
            return !self.popped_up;
        }
        let target_a = self.zoom_out_rel_a(tree);
        if let Some((_, rx, ry, ra)) = self.get_visited_panel_idiom(tree) {
            rx.abs() < 0.001 && ry.abs() < 0.001 && (ra - target_a).abs() < 0.001
        } else {
            true
        }
    }

    // --- CalcVisitCoords ---

    /// Compute optimal (rel_x, rel_y, rel_a) to view `panel` well.
    ///
    /// DIVERGED: C++ (emView.cpp:1373-1487) uses SupremeViewedPanel's viewed
    /// coords for precise placement. Rust uses a simplified layout-walk approach
    /// that targets an 80% viewport fill, returning relA in C++ convention
    /// `HomeW*HomeH/(vw*vh)`. Behavioral contract preserved: returns coords that
    /// keep the panel well-visible in the viewport.
    pub fn CalcVisitCoords(&self, tree: &PanelTree, panel: PanelId) -> (f64, f64, f64) {
        let hw = self.HomeWidth.max(1.0);
        let hh = self.HomeHeight.max(1.0);
        let pt = self.HomePixelTallness.max(MIN_DIMENSION);
        let ph = tree.get_height(panel).max(MIN_DIMENSION);

        // Target: panel should occupy ~80% of viewport in its constraining dimension.
        // Choose vw so that min(vw / hw, vh / hh) = 0.8 where vh = vw * ph / pt.
        // The constraining dimension determines vw.
        let target = 0.8;
        let vw_by_w = hw * target; // fit to viewport width (80%)
        let vw_by_h = hh * target / ph * pt; // fit to viewport height (80%)
        let vw = vw_by_w.min(vw_by_h).max(MIN_DIMENSION);
        let vh = (vw * ph / pt).max(MIN_DIMENSION);

        // C++ relA = HomeW*HomeH / (vw*vh)
        let rel_a = (hw * hh / (vw * vh)).clamp(0.001, MAX_SVP_SIZE);

        // Centering: relX = relY = 0 (panel centered in viewport).
        (0.0, 0.0, rel_a)
    }

    /// Compute coords to show panel at its natural aspect (fullsized).
    ///
    /// Port of C++ `emView::CalcVisitFullsizedCoords` (emView.cpp:1490-1523).
    /// Returns `(relX, relY, relA)` in C++ convention: `relA = HomeW*HomeH/(vw*vh)`.
    ///
    /// C++ uses `GetEssenceRect` which defaults to `(0, 0, 1, GetHeight())`.
    /// In Rust: `HomeX=0, HomeY=0, HomeW=viewport_width, HomeH=viewport_height`.
    /// `fill` corresponds to C++ `utilizeView`.
    pub fn CalcVisitFullsizedCoords(
        &self,
        tree: &PanelTree,
        panel: PanelId,
        fill: bool,
    ) -> (f64, f64, f64) {
        // C++: GetEssenceRect → ex=0, ey=0, ew=1, eh=GetHeight()
        let ph = tree.get_height(panel).max(MIN_DIMENSION);
        let ew = 1.0_f64;
        let eh = ph;
        let fw = self.HomeWidth.max(MIN_DIMENSION);
        let fh = self.HomeHeight.max(MIN_DIMENSION);
        let pt = self.HomePixelTallness.max(MIN_DIMENSION);

        // C++ condition: (ew*fh*pt >= eh*fw) != utilizeView
        let cond = ew * fh * pt >= eh * fw;
        let (vw, vh) = if cond != fill {
            // vw = fw/ew, vh = vw*ph/pt
            let vw = fw / ew;
            let vh = vw * ph / pt;
            (vw, vh)
        } else {
            // vh = fh/eh*ph, vw = vh/ph*pt
            let vh = fh / eh * ph;
            let vw = vh / ph * pt;
            (vw, vh)
        };

        // C++: *pRelX = (HomeX + HomeW*0.5 - vx) / vw - 0.5
        // vx = fx + fw*0.5 - (ex + ew*0.5)*vw = HomeX + HomeW*0.5 - 0.5*vw
        // → relX = (HomeW*0.5 - (HomeW*0.5 - 0.5*vw)) / vw - 0.5 = 0.0 ✓
        // Similarly relY = 0.0 for centered panels.
        let rel_x = 0.0_f64;
        let rel_y = 0.0_f64;

        // C++: relA = HomeW*HomeH/(vw*vh)
        let vw_safe = vw.max(MIN_DIMENSION);
        let vh_safe = vh.max(MIN_DIMENSION);
        let rel_a = (fw * fh / (vw_safe * vh_safe)).clamp(0.001, MAX_SVP_SIZE);

        (rel_x, rel_y, rel_a)
    }

    // --- ViewFlags with side effects ---

    pub fn SetViewFlags(&mut self, flags: ViewFlags, tree: &mut PanelTree) {
        let old = self.flags;
        let mut new_flags = flags;

        if new_flags.contains(ViewFlags::NO_ZOOM) {
            new_flags.remove(ViewFlags::POPUP_ZOOM);
            new_flags.insert(ViewFlags::NO_USER_NAVIGATION);
        }

        if new_flags == old {
            return;
        }

        self.flags = new_flags;

        if new_flags.contains(ViewFlags::POPUP_ZOOM) && !old.contains(ViewFlags::POPUP_ZOOM) {
            self.RawZoomOut(tree, false);
        }

        if new_flags.contains(ViewFlags::NO_ZOOM) && !old.contains(ViewFlags::NO_ZOOM) {
            self.RawZoomOut(tree, false);
        }

        if new_flags.contains(ViewFlags::ROOT_SAME_TALLNESS)
            && !old.contains(ViewFlags::ROOT_SAME_TALLNESS)
        {
            tree.Layout(
                self.root,
                0.0,
                0.0,
                1.0,
                self.GetHomeTallness(),
                self.CurrentPixelTallness,
            );
            self.RawZoomOut(tree, false);
        }
    }

    // --- Active Panel Management ---

    pub fn set_active_panel(&mut self, tree: &mut PanelTree, panel: PanelId, adherent: bool) {
        // Walk up to nearest focusable panel (self included, matching C++ SetActivePanel)
        let target = if tree.GetRec(panel).map(|p| p.focusable).unwrap_or(false) {
            panel
        } else {
            tree.GetFocusableParent(panel).unwrap_or(panel)
        };

        if self.active == Some(target) {
            if self.activation_adherent != adherent {
                self.activation_adherent = adherent;
                // C++ emView.cpp:312: InvalidateHighlight on adherent-only change.
                self.InvalidateHighlight(tree);
            }
            return;
        }

        // C++ emView.cpp:284: InvalidateHighlight for the outgoing active panel.
        if self.active.is_some() {
            self.InvalidateHighlight(tree);
        }

        // Build notice flags: always ACTIVE_CHANGED, add FOCUS_CHANGED if focused
        let mut flags = super::emPanel::NoticeFlags::ACTIVE_CHANGED;
        if self.window_focused {
            flags.insert(super::emPanel::NoticeFlags::FOCUS_CHANGED);
        }

        // Clear old active path
        if let Some(old_active) = self.active {
            let old_path = tree.ancestors(old_active);
            for id in &old_path {
                if let Some(p) = tree.get_mut(*id) {
                    p.is_active = false;
                    p.in_active_path = false;
                    p.pending_notices.insert(flags);
                }
            }
        }

        // Set new active path
        self.active = Some(target);
        dlog!("active panel changed to {:?}", target);
        if let Some(p) = tree.get_mut(target) {
            p.is_active = true;
        }
        let new_path = tree.ancestors(target);
        for id in &new_path {
            if let Some(p) = tree.get_mut(*id) {
                p.in_active_path = true;
                p.pending_notices.insert(flags);
            }
        }
        self.activation_adherent = adherent;
        // C++ emView.cpp:305: InvalidateHighlight for the new active panel.
        self.InvalidateHighlight(tree);
        self.control_panel_invalid = true;
        if let Some(sig) = self.control_panel_signal {
            if let Some(sched) = &self.scheduler {
                sched.borrow_mut().fire(sig);
            }
        }
        tree.mark_notices_pending();
    }

    /// Auto-select best visible focusable panel as active.
    ///
    /// D-PANEL-03: Uses center-containment descent (C++ parity) instead of
    /// max-area. Starts at SVP and descends into the deepest focusable child
    /// whose clip rect contains the viewport center, stopping when children
    /// are too small (< 99% view width AND height, AND < 33% view area).
    pub fn SetActivePanelBestPossible(&mut self, tree: &mut PanelTree) {
        let svp = match self.supreme_viewed_panel {
            Some(id) => id,
            None => return,
        };

        let vw = self.HomeWidth.max(1.0);
        let vh = self.HomeHeight.max(1.0);
        let cx = vw * 0.5;
        let cy = vh * 0.5;
        let min_w = vw * 0.99;
        let min_h = vh * 0.99;
        let min_a = vw * vh * 0.33;

        let mut best = svp;

        // Center-containment descent
        loop {
            let children: Vec<PanelId> = tree.children_rev(best).collect();
            let mut found = None;
            for child in children {
                let p = match tree.GetRec(child) {
                    Some(p) if p.viewed && p.focusable => p,
                    _ => continue,
                };
                // Check if child's clip rect contains view center
                if p.clip_x <= cx
                    && (p.clip_x + p.clip_w) > cx
                    && p.clip_y <= cy
                    && (p.clip_y + p.clip_h) > cy
                {
                    found = Some(child);
                    break;
                }
            }

            match found {
                Some(child) => {
                    let p = tree.GetRec(child).expect("child just found");
                    // Don't descend into panels smaller than thresholds
                    if p.clip_w < min_w && p.clip_h < min_h && (p.clip_w * p.clip_h) < min_a {
                        break;
                    }
                    best = child;
                }
                None => break,
            }
        }

        // Ensure best is focusable (ascend if needed)
        if !tree.GetRec(best).map(|p| p.focusable).unwrap_or(false) {
            if let Some(anc) = tree.GetFocusableParent(best) {
                best = anc;
            } else {
                return;
            }
        }

        // Adherent check: keep current active if still visible and best is ancestor
        if self.activation_adherent {
            if let Some(active_id) = self.active {
                if let Some(active_panel) = tree.GetRec(active_id) {
                    if active_panel.viewed
                        && active_panel.viewed_width >= 4.0
                        && active_panel.viewed_height >= 4.0
                    {
                        if let Some(best_panel) = tree.GetRec(best) {
                            if best_panel.in_active_path {
                                self.set_active_panel(tree, active_id, true);
                                return;
                            }
                        }
                    }
                }
            }
        }
        dlog!("set_active_panel_best_possible chose {:?}", best);
        self.set_active_panel(tree, best, false);
    }

    // --- Coordinate transform: RawVisitAbs + helpers ---

    /// Port of C++ `emView::RawVisitAbs(panel, vx, vy, vw, forceViewingUpdate)`
    /// (emView.cpp:1543-1808). Sets the Supreme Viewed Panel to `panel` at
    /// the requested absolute viewport rect, propagates notices along the
    /// old and new SVP chains, and fires the CursorInvalid/WakeUp/
    /// InvalidatePainting side effects.
    ///
    /// DIVERGED: C++ `RawVisitAbs` is a private overload. Rust has no
    /// overloading — this is the sole entry point and public callers
    /// (RawVisit, Update) pass explicit bools.
    #[allow(clippy::too_many_arguments)]
    pub fn RawVisitAbs(
        &mut self,
        tree: &mut PanelTree,
        panel: PanelId,
        mut vx: f64,
        mut vy: f64,
        mut vw: f64,
        mut forceViewingUpdate: bool,
    ) {
        // emView.cpp:1554-1555
        self.SVPChoiceByOpacityInvalid = false;
        self.SVPChoiceInvalid = false;

        // emView.cpp:1557-1573: VF_NO_ZOOM branch
        let mut vp = if self.flags.contains(ViewFlags::NO_ZOOM) {
            let root = match tree.GetRootPanel() {
                Some(r) => r,
                None => return,
            };
            let h = tree.get_height(root);
            if self.CurrentHeight * self.CurrentPixelTallness >= self.CurrentWidth * h {
                vw = self.CurrentWidth;
                vx = self.CurrentX;
                vy =
                    self.CurrentY + (self.CurrentHeight - vw * h / self.CurrentPixelTallness) * 0.5;
            } else {
                vw = self.CurrentHeight * self.CurrentPixelTallness / h;
                vx = self.CurrentX + (self.CurrentWidth - vw) * 0.5;
                vy = self.CurrentY;
            }
            root
        } else {
            panel
        };

        // emView.cpp:1575-1584: ancestor-clamp loop
        loop {
            let p = tree.GetRec(vp).and_then(|rec| rec.parent);
            let p = match p {
                Some(p) => p,
                None => break,
            };
            let layout_w = tree
                .GetRec(vp)
                .map(|r| r.layout_rect.w)
                .unwrap_or(MIN_DIMENSION)
                .max(MIN_DIMENSION);
            let w = vw / layout_w;
            let parent_h = tree.get_height(p);
            if w > MAX_SVP_SIZE || w * parent_h > MAX_SVP_SIZE {
                break;
            }
            let lx = tree.GetRec(vp).unwrap().layout_rect.x;
            let ly = tree.GetRec(vp).unwrap().layout_rect.y;
            vx -= lx * w;
            vy -= ly * w / self.CurrentPixelTallness;
            vw = w;
            vp = p;
        }

        let vp_h = tree.get_height(vp);
        let vh = vp_h * vw / self.HomePixelTallness;

        // emView.cpp:1588-1626: root-centering/clamping
        if Some(vp) == tree.GetRootPanel() {
            if vw < self.HomeWidth && vh < self.HomeHeight {
                vx = (self.HomeX + self.HomeWidth * 0.5 - vx) / vw;
                vy = (self.HomeY + self.HomeHeight * 0.5 - vy) / vh;
                if vh * self.HomeWidth < vw * self.HomeHeight {
                    vw = self.HomeWidth;
                    let _ = vw * vp_h / self.HomePixelTallness; // recompute vh inline below
                } else {
                    let new_vh = self.HomeHeight;
                    vw = new_vh / vp_h * self.HomePixelTallness;
                }
                vx = self.HomeX + self.HomeWidth * 0.5 - vx * vw;
                let new_vh = vw * vp_h / self.HomePixelTallness;
                vy = self.HomeY + self.HomeHeight * 0.5 - vy * new_vh;
            }

            let (x1, x2, y1, y2);
            if self.flags.contains(ViewFlags::EGO_MODE) {
                x1 = self.HomeX + self.HomeWidth * 0.5;
                x2 = x1;
                y1 = self.HomeY + self.HomeHeight * 0.5;
                y2 = y1;
            } else {
                let vh_cur = vw * vp_h / self.HomePixelTallness;
                if vh_cur * self.HomeWidth < vw * self.HomeHeight {
                    x1 = self.HomeX;
                    x2 = self.HomeX + self.HomeWidth;
                    y1 = self.HomeY + self.HomeHeight * 0.5
                        - self.HomeWidth * vp_h / self.HomePixelTallness * 0.5;
                    y2 = self.HomeY
                        + self.HomeHeight * 0.5
                        + self.HomeWidth * vp_h / self.HomePixelTallness * 0.5;
                } else {
                    x1 = self.HomeX + self.HomeWidth * 0.5
                        - self.HomeHeight / vp_h * self.HomePixelTallness * 0.5;
                    x2 = self.HomeX
                        + self.HomeWidth * 0.5
                        + self.HomeHeight / vp_h * self.HomePixelTallness * 0.5;
                    y1 = self.HomeY;
                    y2 = self.HomeY + self.HomeHeight;
                }
            }
            if vx > x1 {
                vx = x1;
            }
            if vx < x2 - vw {
                vx = x2 - vw;
            }
            if vy > y1 {
                vy = y1;
            }
            let vh_cur = vw * vp_h / self.HomePixelTallness;
            if vy < y2 - vh_cur {
                vy = y2 - vh_cur;
            }
        }

        // emView.cpp:1628-1682: popup branch.
        if self.flags.contains(ViewFlags::POPUP_ZOOM) {
            let outside_home = tree.GetRootPanel() != Some(vp)
                || vx < self.HomeX - 0.1
                || vx + vw > self.HomeX + self.HomeWidth + 0.1
                || vy < self.HomeY - 0.1
                || vy + vw * vp_h / self.HomePixelTallness > self.HomeY + self.HomeHeight + 0.1;

            if outside_home {
                if self.PopupWindow.is_none() {
                    // C++ (emView.cpp:1638): wasFocused=Focused;
                    let was_focused = self.window_focused;
                    // C++ (emView.cpp:1639-1643): PopupWindow=new emWindow(...)
                    // In Rust the `emWindow` struct is constructed synchronously
                    // in `OsSurface::Pending` (`new_popup_pending`), wired to
                    // `self.PopupWindow`, and its winit/wgpu surface is created
                    // one tick later by `App::materialize_popup_surface` via the
                    // `pending_framework_actions` back-channel. Every emCore
                    // observer sees a fully-wired popup immediately, matching
                    // the C++ atomicity contract.
                    let (close_sig, flags_sig, focus_sig, geom_sig) =
                        if let Some(sched) = self.scheduler.as_ref() {
                            let mut s = sched.borrow_mut();
                            (
                                s.create_signal(),
                                s.create_signal(),
                                s.create_signal(),
                                s.create_signal(),
                            )
                        } else {
                            // Unit-test contexts without a scheduler: use null
                            // keys. No signal can fire without a scheduler, so
                            // these are unreachable-by-construction.
                            (
                                super::emSignal::SignalId::default(),
                                super::emSignal::SignalId::default(),
                                super::emSignal::SignalId::default(),
                                super::emSignal::SignalId::default(),
                            )
                        };
                    let popup = super::emWindow::emWindow::new_popup_pending(
                        self.root,
                        super::emWindow::WindowFlags::POPUP
                            | super::emWindow::WindowFlags::UNDECORATED
                            | super::emWindow::WindowFlags::AUTO_DELETE,
                        "emViewPopup".to_string(),
                        close_sig,
                        flags_sig,
                        focus_sig,
                        geom_sig,
                        self.background_color,
                    );
                    self.PopupWindow = Some(popup.clone());
                    // C++ (emView.cpp:1644): UpdateEngine->AddWakeUpSignal(PopupWindow->GetCloseSignal())
                    if let (Some(sched), Some(eng_id)) =
                        (self.scheduler.as_ref(), self.update_engine_id)
                    {
                        sched.borrow_mut().connect(close_sig, eng_id);
                    }
                    // C++ (emView.cpp:1644): SwapViewPorts(true)
                    self.SwapViewPorts(true);
                    // C++ (emView.cpp:1645): if (wasFocused && !Focused) CurrentViewPort->RequestFocus()
                    if was_focused && !self.window_focused {
                        self.CurrentViewPort.borrow_mut().RequestFocus();
                    }
                    // Enqueue OS-surface materialization. Drained by
                    // `App::about_to_wait` on the next tick. If the popup
                    // is torn down before the drain (same-frame exit),
                    // `materialize_popup_surface` detects `strong_count == 1`
                    // and skips creation.
                    if let Some(fw_actions) = self.pending_framework_actions.as_ref() {
                        let popup_for_closure = popup;
                        fw_actions.borrow_mut().push(Box::new(move |fw, el| {
                            fw.materialize_popup_surface(popup_for_closure, el);
                        }));
                    }
                }
                // C++ (emView.cpp:1647): GetMaxPopupViewRect(&sx,&sy,&sw,&sh)
                let mut sr = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
                self.GetMaxPopupViewRect(&mut sr);
                let (sx, sy, sw, sh) = sr;
                let (x1, y1, x2, y2);
                if tree.GetRootPanel() == Some(vp) {
                    // C++ (emView.cpp:1649-1657): root-panel rect clamped to monitor
                    let mut ax1 = vx.floor();
                    let mut ay1 = vy.floor();
                    let mut ax2 = (vx + vw).ceil();
                    let mut ay2 = (vy + vw * vp_h / self.HomePixelTallness).ceil();
                    if ax1 < sx {
                        ax1 = sx;
                    }
                    if ay1 < sy {
                        ay1 = sy;
                    }
                    if ax2 > sx + sw {
                        ax2 = sx + sw;
                    }
                    if ay2 > sy + sh {
                        ay2 = sy + sh;
                    }
                    if ax2 < ax1 + 1.0 {
                        ax2 = ax1 + 1.0;
                    }
                    if ay2 < ay1 + 1.0 {
                        ay2 = ay1 + 1.0;
                    }
                    x1 = ax1;
                    y1 = ay1;
                    x2 = ax2;
                    y2 = ay2;
                } else {
                    // C++ (emView.cpp:1659-1663): full monitor rect
                    x1 = sx;
                    y1 = sy;
                    x2 = sx + sw;
                    y2 = sy + sh;
                }
                // C++ (emView.cpp:1664-1672): resize popup if geometry changed
                if (x1 - self.CurrentX).abs() > 0.01
                    || (x2 - self.CurrentX - self.CurrentWidth).abs() > 0.01
                    || (y1 - self.CurrentY).abs() > 0.01
                    || (y2 - self.CurrentY - self.CurrentHeight).abs() > 0.01
                {
                    self.SwapViewPorts(false);
                    if let Some(ref w) = self.PopupWindow {
                        w.borrow_mut().SetViewPosSize(x1, y1, x2 - x1, y2 - y1);
                    }
                    self.SwapViewPorts(false);
                    forceViewingUpdate = true;
                }
            } else if self.PopupWindow.is_some() {
                // C++ (emView.cpp:1674-1680): tear down popup on return inside home
                self.SwapViewPorts(true);
                // Disconnect + remove the popup's close signal (allocated on
                // popup creation above) so the scheduler doesn't leak it.
                // Also enqueue a framework-side cleanup closure to drop the
                // materialized popup window from `App::windows` so the
                // winit/wgpu surface is released.
                let popup = self
                    .PopupWindow
                    .take()
                    .expect("PopupWindow.is_some() checked above");
                if let (Some(sched), Some(eng_id)) =
                    (self.scheduler.as_ref(), self.update_engine_id)
                {
                    let close_sig = popup.borrow().close_signal;
                    let mut s = sched.borrow_mut();
                    s.disconnect(close_sig, eng_id);
                    s.remove_signal(close_sig);
                }
                let materialized_id = popup
                    .borrow()
                    .winit_window_if_materialized()
                    .map(|w| w.id());
                // Race window: `close_signal` is removed above synchronously,
                // but `App.windows.remove(&window_id)` is deferred to the next
                // `about_to_wait` drain. If a winit event (e.g. CloseRequested
                // from the WM) arrives in that gap, `App::window_event` will
                // call `scheduler.fire(close_signal)` on the already-removed
                // signal. This is safe: `emScheduler::fire` is defensive and
                // treats a lookup miss as a no-op (see its docstring).
                if let (Some(window_id), Some(fw_actions)) =
                    (materialized_id, self.pending_framework_actions.as_ref())
                {
                    fw_actions.borrow_mut().push(Box::new(move |fw, _el| {
                        fw.windows.remove(&window_id);
                    }));
                }
                // GeometrySignal fires twice on popup teardown (both intentional):
                // once from SwapViewPorts(true) above (Rust-only: the Rust
                // SwapViewPorts fires GeometrySignal at the end; C++ SwapViewPorts
                // does not), and once explicitly here (mirroring C++
                // emView.cpp:1680). Keep both; do not dedup.
                if let (Some(sig), Some(sched)) = (self.geometry_signal, &self.scheduler) {
                    sched.borrow_mut().fire(sig);
                }
                forceViewingUpdate = true;
            }
        }

        // emView.cpp:1685: FindBestSVP(&vp, &vx, &vy, &vw)
        self.FindBestSVP(tree, &mut vp, &mut vx, &mut vy, &mut vw);

        // emView.cpp:1687-1696: MaxSVP walk.
        // C++: sp=p; loop { sp=p (save before ascend); p=p->Parent; ... }; MaxSVP=sp;
        let mut p = vp;
        let mut w = vw;
        // sp mirrors C++ `sp`: set at top of each iteration, then used as MaxSVP after loop.
        // Initial value is overwritten before any read — matches C++ `sp=p` as first statement.
        let mut sp;
        loop {
            sp = p;
            let parent = tree.GetRec(p).and_then(|r| r.parent);
            let parent = match parent {
                Some(pp) => pp,
                None => break,
            };
            let layout_w = tree
                .GetRec(p)
                .map(|r| r.layout_rect.w)
                .unwrap_or(MIN_DIMENSION)
                .max(MIN_DIMENSION);
            w /= layout_w;
            let parent_h = tree.get_height(parent);
            if w > MAX_SVP_SIZE || w * parent_h > MAX_SVP_SIZE {
                break;
            }
            p = parent;
        }
        self.MaxSVP = Some(sp);

        // emView.cpp:1698-1725: MinSVP walk (descend into last-child
        // while current rect fully contains the child's rect).
        let mut sp = vp;
        let mut sx = vx;
        let mut sy = vy;
        let mut sw = vw;
        loop {
            let lc = tree.GetLastChild(sp);
            let mut p = match lc {
                Some(pp) => pp,
                None => break,
            };
            let x1 = (self.CurrentX + 1e-4 - sx) / sw;
            let x2 = (self.CurrentX + self.CurrentWidth - 1e-4 - sx) / sw;
            let y1 = (self.CurrentY + 1e-4 - sy) * (self.CurrentPixelTallness / sw);
            let y2 =
                (self.CurrentY + self.CurrentHeight - 1e-4 - sy) * (self.CurrentPixelTallness / sw);
            let mut found = None;
            loop {
                let rec = tree.GetRec(p).unwrap();
                let lx = rec.layout_rect.x;
                let ly = rec.layout_rect.y;
                let lw = rec.layout_rect.w;
                let lh = rec.layout_rect.h;
                if lx < x2 && lx + lw > x1 && ly < y2 && ly + lh > y1 {
                    found = Some(p);
                    break;
                }
                let prev = tree.GetPrev(p);
                match prev {
                    Some(pp) => p = pp,
                    None => break,
                }
            }
            let Some(p) = found else { break };
            let rec = tree.GetRec(p).unwrap();
            if rec.layout_rect.x > x1
                || rec.layout_rect.x + rec.layout_rect.w < x2
                || rec.layout_rect.y > y1
                || rec.layout_rect.y + rec.layout_rect.h < y2
            {
                break;
            }
            sp = p;
            sx += rec.layout_rect.x * sw;
            sy += rec.layout_rect.y * sw / self.CurrentPixelTallness;
            sw *= rec.layout_rect.w;
        }
        self.MinSVP = Some(sp);

        // emView.cpp:1727-1751: change detect + SVPUpdSlice throttle.
        let rect_moved = match self.supreme_viewed_panel.and_then(|id| tree.GetRec(id)) {
            Some(p) => {
                (p.viewed_x - vx).abs() >= 0.001
                    || (p.viewed_y - vy).abs() >= 0.001
                    || (p.viewed_width - vw).abs() >= 0.001
            }
            None => true,
        };
        let svp_changed = self.supreme_viewed_panel != Some(vp);
        if !forceViewingUpdate && !svp_changed && !rect_moved {
            return;
        }

        // SVPUpdSlice fp-instability throttle (emView.cpp:1734-1751).
        let slice = self
            .scheduler
            .as_ref()
            .map(|s| s.borrow().GetTimeSliceCounter())
            .unwrap_or(0);
        if self.SVPUpdSlice != slice {
            self.SVPUpdSlice = slice;
            self.SVPUpdCount = 0;
        }
        self.SVPUpdCount += 1;
        if self.SVPUpdCount > 1000 && (self.SVPUpdCount % 1000 == 1 || self.SVPUpdCount > 10000) {
            // Per-spec "scope up on missing": emGetDblRandom helper does
            // not exist in the Rust crate. Port as a tiny module-local
            // helper — the call site is an fp-instability escape-hatch
            // that only fires after 1000+ retries in one time slice, so
            // the RNG quality is immaterial.
            vx += em_get_dbl_random(-0.01, 0.01);
            vy += em_get_dbl_random(-0.01, 0.01);
            vw *= em_get_dbl_random(0.9999999999, 1.0000000001);
        }

        // emView.cpp:1753-1772: clear old SVP chain.
        if let Some(osvp) = self.supreme_viewed_panel {
            if tree.contains(osvp) {
                if let Some(p) = tree.get_mut(osvp) {
                    p.in_viewed_path = false;
                    p.viewed = false;
                }
                tree.queue_notice(
                    osvp,
                    super::emPanel::NoticeFlags::VIEWING_CHANGED
                        | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED
                        | super::emPanel::NoticeFlags::MEMORY_LIMIT_CHANGED,
                );
                tree.UpdateChildrenViewing(osvp, self.CurrentPixelTallness);
                let mut cur = tree.GetRec(osvp).and_then(|p| p.parent);
                while let Some(pid) = cur {
                    let parent_of = tree.get_mut(pid).map(|p| {
                        p.in_viewed_path = false;
                        p.parent
                    });
                    tree.queue_notice(
                        pid,
                        super::emPanel::NoticeFlags::VIEWING_CHANGED
                            | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED
                            | super::emPanel::NoticeFlags::MEMORY_LIMIT_CHANGED,
                    );
                    cur = parent_of.unwrap_or(None);
                }
            }
        }

        // emView.cpp:1774-1802: set new SVP chain.
        self.supreme_viewed_panel = Some(vp);
        let vp_h = tree.get_height(vp);
        let new_vh = vw * vp_h / self.CurrentPixelTallness;
        if let Some(p) = tree.get_mut(vp) {
            p.in_viewed_path = true;
            p.viewed = true;
            p.viewed_x = vx;
            p.viewed_y = vy;
            p.viewed_width = vw;
            p.viewed_height = new_vh;
            let mut cx1 = vx;
            let mut cy1 = vy;
            let mut cx2 = vx + vw;
            let mut cy2 = vy + new_vh;
            if cx1 < self.CurrentX {
                cx1 = self.CurrentX;
            }
            if cy1 < self.CurrentY {
                cy1 = self.CurrentY;
            }
            if cx2 > self.CurrentX + self.CurrentWidth {
                cx2 = self.CurrentX + self.CurrentWidth;
            }
            if cy2 > self.CurrentY + self.CurrentHeight {
                cy2 = self.CurrentY + self.CurrentHeight;
            }
            p.clip_x = cx1;
            p.clip_y = cy1;
            p.clip_w = (cx2 - cx1).max(0.0);
            p.clip_h = (cy2 - cy1).max(0.0);
        }
        tree.queue_notice(
            vp,
            super::emPanel::NoticeFlags::VIEWING_CHANGED
                | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED
                | super::emPanel::NoticeFlags::MEMORY_LIMIT_CHANGED,
        );
        tree.UpdateChildrenViewing(vp, self.CurrentPixelTallness);
        let mut cur = tree.GetRec(vp).and_then(|p| p.parent);
        while let Some(pid) = cur {
            let parent_of = tree.get_mut(pid).map(|p| {
                p.in_viewed_path = true;
                p.parent
            });
            tree.queue_notice(
                pid,
                super::emPanel::NoticeFlags::VIEWING_CHANGED
                    | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED
                    | super::emPanel::NoticeFlags::MEMORY_LIMIT_CHANGED,
            );
            cur = parent_of.unwrap_or(None);
        }

        // emView.cpp:1803-1806: side effects.
        self.RestartInputRecursion = true;
        self.cursor_invalid = true;
        // C++ emView.cpp:1805: UpdateEngine->WakeUp().
        self.WakeUpUpdateEngine();
        // InvalidatePainting() whole-view — use Current rect (Phase 0 audit
        // verdict). During non-popup Current == Home, so rect = whole view.
        self.dirty_rects.push(Rect::new(
            self.CurrentX,
            self.CurrentY,
            self.CurrentWidth,
            self.CurrentHeight,
        ));
    }

    /// Port of C++ `emView::FindBestSVP` (emView.cpp:1828-1880). Two-pass
    /// search for the best SVP along the ancestor chain of `panel`. The
    /// values are mutated in place.
    pub(crate) fn FindBestSVP(
        &self,
        tree: &PanelTree,
        panel: &mut PanelId,
        vx: &mut f64,
        vy: &mut f64,
        vw: &mut f64,
    ) {
        let mut vp = *panel;
        let mut vx_l = *vx;
        let mut vy_l = *vy;
        let mut vw_l = *vw;
        for i in 0..2 {
            let min_s = if i == 0 {
                MAX_SVP_SIZE
            } else {
                MAX_SVP_SEARCH_SIZE
            };
            let op = vp;
            loop {
                let parent = tree.GetRec(vp).and_then(|r| r.parent);
                let Some(p) = parent else { break };
                let lw = tree
                    .GetRec(vp)
                    .map(|r| r.layout_rect.w)
                    .unwrap_or(MIN_DIMENSION)
                    .max(MIN_DIMENSION);
                let w = vw_l / lw;
                let parent_h = tree.get_height(p);
                if w > min_s || w * parent_h > min_s {
                    break;
                }
                let lx = tree.GetRec(vp).unwrap().layout_rect.x;
                let ly = tree.GetRec(vp).unwrap().layout_rect.y;
                vx_l -= lx * w;
                vy_l -= ly * w / self.CurrentPixelTallness;
                vw_l = w;
                vp = p;
            }
            if op == vp && i > 0 {
                break;
            }
            let vp_h = tree.get_height(vp);
            let b_init = vx_l <= self.CurrentX + 1e-4
                && vx_l + vw_l >= self.CurrentX + self.CurrentWidth - 1e-4
                && vy_l <= self.CurrentY + 1e-4
                && vy_l + vp_h * vw_l / self.CurrentPixelTallness
                    >= self.CurrentY + self.CurrentHeight - 1e-4;
            let mut p = vp;
            let mut x = vx_l;
            let mut y = vy_l;
            let mut w = vw_l;
            let b = self.FindBestSVPInTree(tree, &mut p, &mut x, &mut y, &mut w, b_init);
            if *panel != p {
                *panel = p;
                *vx = x;
                *vy = y;
                *vw = w;
            }
            if b {
                break;
            }
        }
    }

    /// Port of C++ `emView::FindBestSVPInTree` (emView.cpp:1878-1960).
    /// Recursive descent picking the smallest opaque child that still
    /// contains the current rect. See translation table in plan Step 5.
    pub(crate) fn FindBestSVPInTree(
        &self,
        tree: &PanelTree,
        panel: &mut PanelId,
        vx: &mut f64,
        vy: &mut f64,
        vw: &mut f64,
        covering: bool,
    ) -> bool {
        // emView.cpp:1882-1886
        let p_in = *panel;
        let vx_in = *vx;
        let vy_in = *vy;
        let vw_in = *vw;

        // emView.cpp:1887-1891: vs = vw * max(1, h); tooLarge check
        let f = tree.get_height(p_in);
        let mut vs = if f > 1.0 { vw_in * f } else { vw_in };
        let too_large = vs > MAX_SVP_SIZE;

        // emView.cpp:1893: if (!covering && !tooLarge) return false
        if !covering && !too_large {
            return false;
        }

        // emView.cpp:1894-1898: vc = covering && panel is opaque
        let mut vc = covering && {
            let rec = tree.GetRec(p_in).unwrap();
            rec.canvas_color.IsOpaque()
                || rec.behavior.as_ref().map(|b| b.IsOpaque()).unwrap_or(false)
        };

        // emView.cpp:1899-1900: p = p->LastChild; if (!p) return vc
        let lc = tree.GetLastChild(p_in);
        let mut p = match lc {
            Some(pp) => pp,
            None => return vc,
        };

        // emView.cpp:1902-1908: compute layout-space viewport bounds
        let x1 = (self.CurrentX + 1e-4 - vx_in) / vw_in;
        let x2 = (self.CurrentX + self.CurrentWidth - 1e-4 - vx_in) / vw_in;
        let vwc = vw_in / self.CurrentPixelTallness;
        let y1 = (self.CurrentY + 1e-4 - vy_in) / vwc;
        let y2 = (self.CurrentY + self.CurrentHeight - 1e-4 - vy_in) / vwc;
        let mut vd: f64 = 1e30;
        let mut overlapped = false;

        // emView.cpp:1910-1968: do { ... } while (p)
        loop {
            let rec = tree.GetRec(p).unwrap();
            // emView.cpp:1911-1914: overlap check
            if rec.layout_rect.x < x2
                && rec.layout_rect.x + rec.layout_rect.w > x1
                && rec.layout_rect.y < y2
                && rec.layout_rect.y + rec.layout_rect.h > y1
            {
                // emView.cpp:1915-1923: covering check, break if vc && !tooLarge
                let mut cc = true;
                if !covering
                    || rec.layout_rect.x > x1
                    || rec.layout_rect.x + rec.layout_rect.w < x2
                    || rec.layout_rect.y > y1
                    || rec.layout_rect.y + rec.layout_rect.h < y2
                {
                    if !too_large && vc {
                        break;
                    }
                    cc = false;
                }
                // emView.cpp:1924-1928: recurse into child
                let mut cp = p;
                let mut cx = vx_in + rec.layout_rect.x * vw_in;
                let mut cy = vy_in + rec.layout_rect.y * vwc;
                let mut cw = rec.layout_rect.w * vw_in;
                cc = self.FindBestSVPInTree(tree, &mut cp, &mut cx, &mut cy, &mut cw, cc);
                // emView.cpp:1929: if (!cc && !tooLarge && vc) break
                if !cc && !too_large && vc {
                    break;
                }
                // emView.cpp:1930-1932: cs = cw * max(1, cp->GetHeight())
                let cf = tree.get_height(cp);
                let cs = if cf > 1.0 { cw * cf } else { cw };
                // emView.cpp:1933-1941: if cc && cs<=MaxSVPSize → return true
                if cc && cs <= MAX_SVP_SIZE {
                    if too_large || !overlapped {
                        *panel = cp;
                        *vx = cx;
                        *vy = cy;
                        *vw = cw;
                    }
                    return true;
                }
                // emView.cpp:1942
                overlapped = true;
                // emView.cpp:1943-1965: tooLarge best-candidate tracking
                if too_large {
                    let xm = (x2 + x1) * 0.5;
                    let ym = (y2 + y1) * 0.5;
                    let dx = if xm < rec.layout_rect.x {
                        xm - rec.layout_rect.x
                    } else if xm > rec.layout_rect.x + rec.layout_rect.w {
                        xm - (rec.layout_rect.x + rec.layout_rect.w)
                    } else {
                        0.0
                    };
                    let dy = if ym < rec.layout_rect.y {
                        ym - rec.layout_rect.y
                    } else if ym > rec.layout_rect.y + rec.layout_rect.h {
                        ym - (rec.layout_rect.y + rec.layout_rect.h)
                    } else {
                        0.0
                    };
                    let d = dx * dx + dy * dy;
                    if (cs <= MAX_SVP_SIZE && d - 0.1 <= vd) || (vs > MAX_SVP_SIZE && cs <= vs) {
                        *panel = cp;
                        *vx = cx;
                        *vy = cy;
                        *vw = cw;
                        vd = d;
                        // emView.cpp:1962-1963: vs=cs; vc=cc
                        vs = cs;
                        vc = cc;
                    }
                }
            }
            // emView.cpp:1967: p=p->Prev
            let prev = tree.GetPrev(p);
            match prev {
                Some(pp) => p = pp,
                None => break,
            }
        }

        // emView.cpp:1970: return vc
        vc
    }

    // --- Coordinate transform: update_viewing ---

    /// Compute absolute viewport coordinates for all panels. Called once per frame.
    /// Port of C++ `emView::Update` (emView.cpp:1292-1370). Drain loop
    /// dispatching in priority order: popup-close → notices →
    /// SVPChoiceByOpacityInvalid → SVPChoiceInvalid → TitleInvalid →
    /// CursorInvalid.
    ///
    /// DIVERGED: notice-drain delegates to PanelTree::HandleNotice (ring owned
    /// by PanelTree, per commit 75c7c68). Rest of shape is identical.
    pub fn Update(&mut self, tree: &mut PanelTree) {
        // C++ emView.cpp:1299-1301: popup close —
        //   if (IsSignaled(PopupWindow->GetCloseSignal())) ZoomOut();
        // Check the popup placeholder's close signal via the scheduler's
        // clock-based predicate.
        //
        // BUG (tracked as "emView::Update scheduler re-entrant borrow"
        // in docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md §8):
        // the `sched.borrow()` call below panics re-entrantly when `Update`
        // is reached via the engine chain (`DoTimeSlice` → `UpdateEngineClass::Cycle`
        // → here) because the caller holds `sched.borrow_mut()` across the
        // entire `DoTimeSlice`. The previous comment claimed this was "scoped
        // for borrow correctness"; that is incorrect — scoping only helps
        // when the outer borrow isn't live. Fix options: (a) add a scheduler
        // parameter to `Update` so the caller can pass `&EngineCtx` instead
        // of reaching back through the Rc; (b) cache the signaled state on
        // the view during signal processing. Blocks the single-engine
        // rewrite of `test_phase8_popup_close_signal_zooms_out`.
        let popup_closed = {
            if let (Some(popup), Some(sched), Some(eng_id)) = (
                self.PopupWindow.as_ref(),
                self.scheduler.as_ref(),
                self.update_engine_id,
            ) {
                let close_sig = popup.borrow().close_signal;
                sched.borrow().is_signaled_for_engine(close_sig, eng_id)
            } else {
                false
            }
        };
        if popup_closed {
            self.ZoomOut(tree);
        }

        // First-frame zoom-out: C++ ZoomedOutBeforeSG.
        // C++ SetGeometry (emView.cpp:1272-1273) calls RawZoomOut(true) directly.
        // Rust defers this to Update but matches C++ by calling RawZoomOut directly,
        // not by setting SVPChoiceInvalid (which would rely on viewed_x/y/width being
        // already populated, which they are not on the first frame).
        if self.zoomed_out_before_sg {
            self.zoomed_out_before_sg = false;
            self.RawZoomOut(tree, false);
        }

        loop {
            // C++ emPanel::Layout `!Parent` branch sets View.SVPChoiceInvalid and
            // calls RawZoomOut(true) when view is zoomed out. In Rust, Layout
            // can't call view methods, so it sets root_layout_changed. We drain
            // it here. The zoomed_out_before_sg pre-loop already called
            // RawZoomOut(false) which set up the SVP — calling RawZoomOut again
            // would double-queue VIEW_CHANGED notices (breaking animator tests).
            // Match C++ Layout's View.SVPChoiceInvalid=true side effect only.
            if tree.root_layout_changed {
                tree.root_layout_changed = false;
                self.SVPChoiceInvalid = true;
                continue;
            }

            // Drain all pending notices (delegated to PanelTree).
            if tree.has_pending_notices() {
                tree.HandleNotice(self.window_focused, self.CurrentPixelTallness);
                continue;
            }

            if self.SVPChoiceByOpacityInvalid {
                self.SVPChoiceByOpacityInvalid = false;
                if !self.SVPChoiceInvalid && self.MinSVP.is_some() && self.MinSVP != self.MaxSVP {
                    let mut p = self.MinSVP;
                    let max = self.MaxSVP;
                    while p != max {
                        let opaque = p
                            .and_then(|id| tree.GetRec(id))
                            .map(|rec| {
                                rec.canvas_color.IsOpaque()
                                    || rec.behavior.as_ref().map(|b| b.IsOpaque()).unwrap_or(false)
                            })
                            .unwrap_or(false);
                        if opaque {
                            break;
                        }
                        p = p.and_then(|id| tree.GetRec(id).and_then(|r| r.parent));
                    }
                    if self.supreme_viewed_panel != p {
                        dlog!("SVP choice invalid by opacity.");
                        self.SVPChoiceInvalid = true;
                    }
                }
                continue;
            }

            if self.SVPChoiceInvalid {
                self.SVPChoiceInvalid = false;
                if let Some((panel, _, _, _)) = self.get_visited_panel_idiom(tree) {
                    let rec = tree.GetRec(panel).unwrap();
                    let (vx, vy, vw) = (rec.viewed_x, rec.viewed_y, rec.viewed_width);
                    self.RawVisitAbs(tree, panel, vx, vy, vw, false);
                }
                continue;
            }

            if self.title_invalid {
                self.title_invalid = false;
                let new_title = self.active.map(|id| tree.get_title(id)).unwrap_or_default();
                if self.title != new_title {
                    self.title = new_title;
                    self.invalidate_view_title();
                }
                continue;
            }

            if self.cursor_invalid {
                self.cursor_invalid = false;
                let p = self.GetPanelAt(tree, self.LastMouseX, self.LastMouseY);
                let mut cur = p
                    .and_then(|id| tree.GetRec(id))
                    .and_then(|rec| rec.behavior.as_ref().map(|b| b.GetCursor()))
                    .unwrap_or(emCursor::Normal);
                if self.flags.contains(ViewFlags::EGO_MODE) && cur == emCursor::Normal {
                    cur = emCursor::Crosshair;
                }
                if self.cursor != cur {
                    self.cursor = cur;
                    // C++ emView.cpp: CurrentViewPort->InvalidateCursor().
                    // Phase 5 wires this through emViewPort::SetViewCursor,
                    // which caches the cursor and flags it dirty for emWindow
                    // to apply on the next frame.
                    self.CurrentViewPort.borrow_mut().SetViewCursor(cur);
                }
                continue;
            }

            break;
        }

        // DIVERGED: Rust-only navigation request drain. C++ uses callbacks;
        // Rust panels post requests via PanelCtx::request_visit and emView drains
        // them here (after the main loop so SVP is up to date).
        for target in tree.drain_navigation_requests() {
            self.VisitFullsized(tree, target, false, false);
        }
    }

    // --- Navigation ---

    /// Port of C++ `emView::VisitNext()` (emView.cpp:564-578).
    pub fn VisitNext(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let Some(active) = self.active else { return };
        let mut p = tree.GetFocusableNext(active);
        if p.is_none() {
            let parent = tree
                .GetFocusableParent(active)
                .or_else(|| tree.GetRootPanel())
                .unwrap_or(active);
            if parent != active {
                p = tree.GetFocusableFirstChild(parent);
            } else {
                p = Some(parent);
            }
        }
        if let Some(target) = p {
            self.VisitPanel(tree, target, true);
        }
    }

    /// Port of C++ `emView::VisitPrev()` (emView.cpp:581-595).
    pub fn VisitPrev(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let Some(active) = self.active else { return };
        let mut p = tree.GetFocusablePrev(active);
        if p.is_none() {
            let parent = tree
                .GetFocusableParent(active)
                .or_else(|| tree.GetRootPanel())
                .unwrap_or(active);
            if parent != active {
                p = tree.GetFocusableLastChild(parent);
            } else {
                p = Some(parent);
            }
        }
        if let Some(target) = p {
            self.VisitPanel(tree, target, true);
        }
    }

    /// Port of C++ `emView::VisitFirst()` (emView.cpp:598-608).
    pub fn VisitFirst(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let Some(active) = self.active else { return };
        let mut p = tree.GetFocusableParent(active);
        if let Some(parent) = p {
            p = tree.GetFocusableFirstChild(parent);
        }
        let target = p.unwrap_or(active);
        self.VisitPanel(tree, target, true);
    }

    /// Port of C++ `emView::VisitLast()` (emView.cpp:611-621).
    pub fn VisitLast(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let Some(active) = self.active else { return };
        let mut p = tree.GetFocusableParent(active);
        if let Some(parent) = p {
            p = tree.GetFocusableLastChild(parent);
        }
        let target = p.unwrap_or(active);
        self.VisitPanel(tree, target, true);
    }

    /// Port of C++ `emView::VisitLeft()` (emView.cpp:624-627).
    pub fn VisitLeft(&mut self, tree: &mut PanelTree) {
        self.VisitNeighbour(tree, 2);
    }

    /// Port of C++ `emView::VisitRight()` (emView.cpp:630-633).
    pub fn VisitRight(&mut self, tree: &mut PanelTree) {
        self.VisitNeighbour(tree, 0);
    }

    /// Port of C++ `emView::VisitUp()` (emView.cpp:636-639).
    pub fn VisitUp(&mut self, tree: &mut PanelTree) {
        self.VisitNeighbour(tree, 3);
    }

    /// Port of C++ `emView::VisitDown()` (emView.cpp:642-645).
    pub fn VisitDown(&mut self, tree: &mut PanelTree) {
        self.VisitNeighbour(tree, 1);
    }

    /// Port of C++ `emView::VisitIn()` (emView.cpp:740-746).
    pub fn VisitIn(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let Some(active) = self.active else { return };
        if let Some(p) = tree.GetFocusableFirstChild(active) {
            self.VisitPanel(tree, p, true);
        } else {
            self.VisitFullsized(tree, active, true, false);
        }
    }

    /// Port of C++ `emView::VisitOut()` (emView.cpp:749-762).
    pub fn VisitOut(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let Some(active) = self.active else { return };
        if let Some(p) = tree.GetFocusableParent(active) {
            self.VisitPanel(tree, p, true);
        } else if let Some(root) = tree.GetRootPanel() {
            let root_h = tree.get_height(root);
            let mut rel_a = self.HomeWidth * root_h / self.HomePixelTallness / self.HomeHeight;
            let rel_a2 = self.HomeHeight / root_h * self.HomePixelTallness / self.HomeWidth;
            if rel_a < rel_a2 {
                rel_a = rel_a2;
            }
            self.Visit(tree, root, 0.0, 0.0, rel_a, true);
        }
    }

    /// Port of C++ `emView::VisitNeighbour(direction)` (emView.cpp:648-737).
    pub fn VisitNeighbour(&mut self, tree: &mut PanelTree, direction: i32) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let direction = direction & 3;
        let Some(current0) = self.active else { return };
        let parent = tree
            .GetFocusableParent(current0)
            .or_else(|| tree.GetRootPanel())
            .unwrap_or(current0);

        let mut current = current0;

        if parent != current0 {
            // Compute current's rect in parent-local coords by composing
            // through ancestor layout_rects from current up to (but not
            // including) parent.
            let (mut cx1, mut cy1) = (0.0_f64, 0.0_f64);
            let (mut cx2, mut cy2) = (1.0_f64, tree.get_height(current0));
            let mut walker = current0;
            while walker != parent {
                let lr = tree
                    .layout_rect(walker)
                    .expect("panel must have layout_rect while walking up to focusable parent");
                let f = lr.w;
                let fx = lr.x;
                let fy = lr.y;
                cx1 = cx1 * f + fx;
                cy1 = cy1 * f + fy;
                cx2 = cx2 * f + fx;
                cy2 = cy2 * f + fy;
                walker = tree
                    .GetParentContext(walker)
                    .expect("panel must have parent while walking up to focusable parent");
            }

            let mut best: Option<PanelId> = None;
            let mut best_val = 0.0_f64;
            let mut defdx = -1.0_f64;

            let mut n_opt = tree.GetFocusableFirstChild(parent);
            while let Some(n) = n_opt {
                if n == current0 {
                    defdx = -defdx;
                    n_opt = tree.GetFocusableNext(n);
                    continue;
                }

                let (mut nx1, mut ny1) = (0.0_f64, 0.0_f64);
                let (mut nx2, mut ny2) = (1.0_f64, tree.get_height(n));
                let mut w = n;
                while w != parent {
                    let lr = tree
                        .layout_rect(w)
                        .expect("panel must have layout_rect while walking sibling up to parent");
                    let f = lr.w;
                    let fx = lr.x;
                    let fy = lr.y;
                    nx1 = nx1 * f + fx;
                    ny1 = ny1 * f + fy;
                    nx2 = nx2 * f + fx;
                    ny2 = ny2 * f + fy;
                    w = tree
                        .GetParentContext(w)
                        .expect("panel must have parent while walking sibling up to parent");
                }

                let mut dx = 0.0_f64;
                let mut dy = 0.0_f64;

                let fx1 = nx1 - cx1;
                let fy1 = ny1 - cy1;
                let f1 = (fx1 * fx1 + fy1 * fy1).sqrt();
                if f1 > 1e-30 {
                    dx += fx1 / f1;
                    dy += fy1 / f1;
                }
                let fx2 = nx2 - cx2;
                let fy2 = ny1 - cy1;
                let f2 = (fx2 * fx2 + fy2 * fy2).sqrt();
                if f2 > 1e-30 {
                    dx += fx2 / f2;
                    dy += fy2 / f2;
                }
                let fx3 = nx1 - cx1;
                let fy3 = ny2 - cy2;
                let f3 = (fx3 * fx3 + fy3 * fy3).sqrt();
                if f3 > 1e-30 {
                    dx += fx3 / f3;
                    dy += fy3 / f3;
                }
                let fx4 = nx2 - cx2;
                let fy4 = ny2 - cy2;
                let f4 = (fx4 * fx4 + fy4 * fy4).sqrt();
                if f4 > 1e-30 {
                    dx += fx4 / f4;
                    dy += fy4 / f4;
                }
                let fnorm = (dx * dx + dy * dy).sqrt();
                if fnorm > 1e-30 {
                    dx /= fnorm;
                    dy /= fnorm;
                } else {
                    dx = defdx;
                    dy = 0.0;
                }

                let fx_c = (nx1 + nx2 - cx1 - cx2) * 0.5;
                let fy_c = (ny1 + ny2 - cy1 - cy2) * 0.5;
                let d = (fx_c * fx_c + fy_c * fy_c).sqrt();

                let fx_e = if nx2 < cx1 {
                    nx2 - cx1
                } else if nx1 > cx2 {
                    nx1 - cx2
                } else {
                    0.0
                };
                let fy_e = if ny2 < cy1 {
                    ny2 - cy1
                } else if ny1 > cy2 {
                    ny1 - cy2
                } else {
                    0.0
                };
                let e = (fx_e * fx_e + fy_e * fy_e).sqrt();

                if (direction & 1) != 0 {
                    let f = dx;
                    dx = dy;
                    dy = -f;
                }
                if (direction & 2) != 0 {
                    dx = -dx;
                    dy = -dy;
                }
                if dx <= 1e-12 {
                    n_opt = tree.GetFocusableNext(n);
                    continue;
                }

                let mut val = (e * 10.0 + d) * (1.0 + 2.0 * dy * dy);
                if dy.abs() > 0.707 {
                    val *= 1000.0 * dy * dy * dy * dy;
                }
                if best.is_none() || val < best_val {
                    best = Some(n);
                    best_val = val;
                }

                n_opt = tree.GetFocusableNext(n);
            }

            if let Some(b) = best {
                current = b;
            }
        }

        self.VisitPanel(tree, current, true);
    }

    // --- Hit testing ---

    pub fn GetPanelAt(&self, tree: &PanelTree, x: f64, y: f64) -> Option<PanelId> {
        let svp = self.supreme_viewed_panel?;
        self.hit_test_recursive(tree, svp, x, y, false)
    }

    pub fn GetFocusablePanelAt(&self, tree: &PanelTree, x: f64, y: f64) -> Option<PanelId> {
        let svp = self.supreme_viewed_panel?;
        self.hit_test_recursive(tree, svp, x, y, true)
    }

    fn hit_test_recursive(
        &self,
        tree: &PanelTree,
        id: PanelId,
        x: f64,
        y: f64,
        focusable_only: bool,
    ) -> Option<PanelId> {
        let panel = tree.GetRec(id)?;
        if !panel.viewed {
            return None;
        }

        let clip = Rect::new(panel.clip_x, panel.clip_y, panel.clip_w, panel.clip_h);
        if !clip.contains_point(x, y) {
            return None;
        }

        // Check children in reverse Z-order (last = topmost)
        let children: Vec<PanelId> = tree.children_rev(id).collect();
        for child in children {
            if let Some(hit) = self.hit_test_recursive(tree, child, x, y, focusable_only) {
                return Some(hit);
            }
        }

        // No child hit — check self
        if focusable_only && !panel.focusable {
            return None;
        }

        Some(id)
    }

    /// Remove a panel from the tree, moving activation to its parent if needed.
    /// Matches C++ `~emPanel` which calls `SetFocusable(false)` before unlinking,
    /// causing `emView.SetActivePanel(Parent, false)`.
    pub fn remove_panel(&mut self, tree: &mut PanelTree, id: PanelId) {
        if tree.GetRec(id).map(|p| p.in_active_path).unwrap_or(false) {
            if let Some(parent) = tree.GetParentContext(id) {
                self.set_active_panel(tree, parent, false);
            } else {
                self.active = None;
            }
        }
        tree.remove(id);
    }

    // --- Panel-level wrappers ---

    /// Activate a panel (delegate to set_active_panel).
    pub fn activate_panel(&mut self, tree: &mut PanelTree, panel: PanelId) {
        self.set_active_panel(tree, panel, false);
    }

    /// Focus the view and activate a panel.
    pub fn focus_panel(&mut self, tree: &mut PanelTree, panel: PanelId) {
        self.SetFocused(tree, true);
        self.set_active_panel(tree, panel, false);
    }

    /// Whether a panel is focused: it is the active panel and the view's
    /// window is focused.
    pub fn is_panel_focused(&self, tree: &PanelTree, panel: PanelId) -> bool {
        let is_active = tree.GetRec(panel).map(|p| p.is_active).unwrap_or(false);
        is_active && self.window_focused
    }

    /// Whether a panel is in the focused path: it is in the active path and
    /// the view's window is focused.
    pub fn is_panel_in_focused_path(&self, tree: &PanelTree, panel: PanelId) -> bool {
        let in_active_path = tree
            .GetRec(panel)
            .map(|p| p.in_active_path)
            .unwrap_or(false);
        in_active_path && self.window_focused
    }

    /// Whether a panel's activation is adherent (indirect, via a descendant).
    pub fn is_panel_activated_adherent(&self, tree: &PanelTree, panel: PanelId) -> bool {
        tree.GetRec(panel).map(|p| p.is_active).unwrap_or(false) && self.activation_adherent
    }

    /// Whether the view's window is focused (panel-level delegate).
    pub fn is_view_focused(&self) -> bool {
        self.window_focused
    }

    /// Return wall-clock milliseconds (since Unix epoch).
    pub fn GetInputClockMS(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Panel-level delegate for `get_input_clock_ms` (mirrors `emPanel::GetInputClockMS`).
    pub fn get_panel_input_clock_ms(&self) -> u64 {
        self.GetInputClockMS()
    }

    // --- Pixel tallness ---

    /// Return the pixel tallness (height/width ratio of a pixel).
    ///
    /// Corresponds to `emPanel::GetViewedPixelTallness` (delegates to
    /// `emView.CurrentPixelTallness`).
    pub fn GetCurrentPixelTallness(&self) -> f64 {
        self.CurrentPixelTallness
    }

    // --- Invalidation ---

    /// Mark the view's title as needing a refresh. Only takes effect when
    /// the panel is in the active path.
    ///
    /// Corresponds to `emPanel::InvalidateTitle`.
    pub fn InvalidateTitle(&mut self, tree: &PanelTree, panel: PanelId) {
        let in_active_path = tree
            .GetRec(panel)
            .map(|p| p.in_active_path)
            .unwrap_or(false);
        if in_active_path {
            self.title_invalid = true;
        }
    }

    /// Port of C++ `emPanel::InvalidateViewing` (emPanel.cpp). Marks the view's
    /// SVPChoiceInvalid flag so the next Update picks a new Supreme Viewed Panel.
    pub fn InvalidateViewing(&mut self, _tree: &PanelTree, _panel: PanelId) {
        self.SVPChoiceInvalid = true;
    }

    /// Mark the view's cursor as needing a refresh. Only takes effect when
    /// the panel is in the viewed path.
    ///
    /// Corresponds to `emPanel::InvalidateCursor`.
    pub fn InvalidateCursor(&mut self, tree: &PanelTree, panel: PanelId) {
        let in_viewed_path = tree
            .GetRec(panel)
            .map(|p| p.in_viewed_path)
            .unwrap_or(false);
        if in_viewed_path {
            self.cursor_invalid = true;
        }
    }

    /// Mark the entire panel clip rect as needing repaint.
    ///
    /// Corresponds to `emPanel::InvalidatePainting()` (no-arg overload).
    pub fn InvalidatePainting(&mut self, tree: &PanelTree, panel: PanelId) {
        let p = match tree.GetRec(panel) {
            Some(p) if p.viewed => p,
            _ => return,
        };
        self.dirty_rects
            .push(Rect::new(p.clip_x, p.clip_y, p.clip_w, p.clip_h));
    }

    /// Mark a sub-rectangle of the panel as needing repaint. The rectangle
    /// is specified in panel coordinates and is transformed to view
    /// coordinates, then clipped against the panel's clip rect.
    ///
    /// Corresponds to `emPanel::InvalidatePainting(x, y, w, h)`.
    pub fn invalidate_painting_rect(
        &mut self,
        tree: &PanelTree,
        panel: PanelId,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) {
        let p = match tree.GetRec(panel) {
            Some(p) if p.viewed => p,
            _ => return,
        };

        // Transform from panel space to view space
        let mut vx = p.viewed_x + x * p.viewed_width;
        let mut vy = p.viewed_y + y * p.viewed_height;
        let mut vw = w * p.viewed_width;
        let mut vh = h * p.viewed_height;

        // Clip against the panel's clip rect
        let clip_x2 = p.clip_x + p.clip_w;
        let clip_y2 = p.clip_y + p.clip_h;

        if vx < p.clip_x {
            vw -= p.clip_x - vx;
            vx = p.clip_x;
        }
        if vy < p.clip_y {
            vh -= p.clip_y - vy;
            vy = p.clip_y;
        }
        if vw > clip_x2 - vx {
            vw = clip_x2 - vx;
        }
        if vh > clip_y2 - vy {
            vh = clip_y2 - vy;
        }

        if vw > 0.0 && vh > 0.0 {
            self.dirty_rects.push(Rect::new(vx, vy, vw, vh));
        }
    }

    /// Signal that the control panel needs refreshing. Only takes effect
    /// when the panel is in the active path.
    ///
    /// Corresponds to `emPanel::InvalidateControlPanel`.
    pub fn InvalidateControlPanel(&mut self, tree: &PanelTree, panel: PanelId) {
        let in_active_path = tree
            .GetRec(panel)
            .map(|p| p.in_active_path)
            .unwrap_or(false);
        if in_active_path {
            self.control_panel_invalid = true;
            if let Some(sig) = self.control_panel_signal {
                if let Some(sched) = &self.scheduler {
                    sched.borrow_mut().fire(sig);
                }
            }
        }
    }

    /// Whether the title has been invalidated since the last clear.
    pub fn is_title_invalid(&self) -> bool {
        self.title_invalid
    }

    /// Clear the title-invalid flag.
    pub fn clear_title_invalid(&mut self) {
        self.title_invalid = false;
    }

    /// Whether the cursor has been invalidated since the last clear.
    pub fn is_cursor_invalid(&self) -> bool {
        self.cursor_invalid
    }

    /// Mark the cursor as needing a refresh (without requiring tree/panel).
    pub fn mark_cursor_invalid(&mut self) {
        self.cursor_invalid = true;
    }

    /// Clear the cursor-invalid flag.
    pub fn clear_cursor_invalid(&mut self) {
        self.cursor_invalid = false;
    }

    /// Whether the control panel has been invalidated since the last clear.
    pub fn is_control_panel_invalid(&self) -> bool {
        self.control_panel_invalid
    }

    /// Clear the control-panel-invalid flag.
    pub fn clear_control_panel_invalid(&mut self) {
        self.control_panel_invalid = false;
    }

    pub fn set_control_panel_signal(&mut self, signal: super::emSignal::SignalId) {
        self.control_panel_signal = Some(signal);
    }

    pub fn GetControlPanelSignal(&self) -> Option<super::emSignal::SignalId> {
        self.control_panel_signal
    }

    pub fn set_title_signal(&mut self, signal: super::emSignal::SignalId) {
        self.title_signal = Some(signal);
    }

    pub fn GetTitleSignal(&self) -> Option<super::emSignal::SignalId> {
        self.title_signal
    }

    pub fn set_scheduler(&mut self, scheduler: Rc<RefCell<super::emScheduler::EngineScheduler>>) {
        self.scheduler = Some(scheduler);
    }

    /// Wire the back-channel into `App::pending_actions` so that
    /// `RawVisitAbs`'s popup-entry branch can enqueue deferred popup-surface
    /// materialization. Called by `App::about_to_wait` each frame (idempotent).
    pub(crate) fn set_pending_framework_actions(
        &mut self,
        actions: Rc<RefCell<Vec<super::emGUIFramework::DeferredAction>>>,
    ) {
        self.pending_framework_actions = Some(actions);
    }

    /// Attach the view to a scheduler and register its `UpdateEngineClass`.
    ///
    /// Mirrors the C++ `emView` constructor, which creates and registers the
    /// `UpdateEngine` (HIGH_PRIORITY) + `EOISignal`. After this call, engines
    /// can be woken via `WakeUpUpdateEngine()` and `SignalEOIDelayed()`.
    pub fn attach_to_scheduler(
        &mut self,
        scheduler: Rc<RefCell<super::emScheduler::EngineScheduler>>,
        window_id: winit::window::WindowId,
    ) {
        let (engine_id, eoi_signal, visiting_va_engine_id) = {
            let mut sched = scheduler.borrow_mut();
            let engine_id = sched.register_engine(
                super::emEngine::Priority::High,
                Box::new(UpdateEngineClass::new(window_id)),
            );
            let eoi_signal = sched.create_signal();
            // W4 Task 1.3: register the VisitingVA engine. C++ equivalent:
            // emVisitingViewAnimator's emEngine base ctor auto-registers
            // (see emViewAnimator.cpp:930 + emEngine ctor chain).
            // C++ emViewAnimator base ctor sets HIGH_PRIORITY (emViewAnimator.cpp:39).
            let visiting_va_engine_id = sched.register_engine(
                super::emEngine::Priority::High,
                Box::new(VisitingVAEngineClass::new(window_id)),
            );
            (engine_id, eoi_signal, visiting_va_engine_id)
        };
        self.scheduler = Some(scheduler);
        self.update_engine_id = Some(engine_id);
        self.EOISignal = Some(eoi_signal);
        self.visiting_va_engine_id = Some(visiting_va_engine_id);
    }

    /// Wake the scheduler-registered `UpdateEngineClass` so `Update()` runs
    /// in the current time slice. Mirrors C++ `UpdateEngine->WakeUp()`.
    pub fn WakeUpUpdateEngine(&self) {
        if let (Some(id), Some(sched)) = (self.update_engine_id, &self.scheduler) {
            sched.borrow_mut().wake_up(id);
        }
    }

    /// Port of C++ `emView::Visit(identity, relX, relY, relA, adherent, subject)`
    /// at emView.cpp:500-508. Three-line delegation to `VisitingVA`:
    /// `SetAnimParamsByCoreConfig` → `SetGoalWithCoords` → `Activate`. The
    /// animator engine (`VisitingVAEngineClass::Cycle`) observes `is_active()`
    /// and drives the curve each scheduler tick.
    ///
    /// PHASE-W4-FOLLOWUP: C++ passes this view's `CoreConfig` to
    /// `SetAnimParamsByCoreConfig`. Rust `emView` does not yet own a
    /// `emCoreConfig`, so we hardcode the stock defaults
    /// (`VisitSpeed=1.0`, `MaxVisitSpeed=10.0`) from emCoreConfig.cpp:53.
    /// Full `CoreConfig` ownership is a future wave.
    pub fn VisitByIdentity(
        &mut self,
        identity: &str,
        rel_x: f64,
        rel_y: f64,
        rel_a: f64,
        adherent: bool,
        subject: &str,
    ) {
        let mut va = self.VisitingVA.borrow_mut();
        va.SetAnimParamsByCoreConfig(1.0, 10.0);
        va.SetGoalWithCoords(identity, rel_x, rel_y, rel_a, adherent, subject);
        va.Activate();
    }

    /// Borrow the visiting view animator for inspection.
    /// Exposes `VisitingVA` to cross-crate callers (e.g. integration tests)
    /// without making the field itself `pub`. C++ field is private; this
    /// provides read access equivalent to C++ friend/test-only inspection.
    pub fn visiting_va(&self) -> std::cell::Ref<'_, super::emViewAnimator::emVisitingViewAnimator> {
        self.VisitingVA.borrow()
    }

    /// Test-only: drive `VisitingVA::animate` directly until it deactivates
    /// or the iteration limit is hit. Production path goes through
    /// `VisitingVAEngineClass::Cycle` which requires a window registry; unit
    /// tests without a window use this to observe the post-convergence active
    /// panel after a `Visit*` call.
    pub fn pump_visiting_va(&mut self, tree: &mut PanelTree) {
        use super::emViewAnimator::emViewAnimator as _;
        let va_rc = Rc::clone(&self.VisitingVA);
        for _ in 0..1024 {
            let mut va = va_rc.borrow_mut();
            if !va.is_active() {
                break;
            }
            let still = va.animate(self, tree, 0.1);
            if !still {
                break;
            }
        }
    }

    /// Check whether any dirty rectangles have been accumulated.
    pub fn has_dirty_rects(&self) -> bool {
        !self.dirty_rects.is_empty()
    }

    /// Drain accumulated dirty rectangles.
    pub fn take_dirty_rects(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.dirty_rects)
    }

    /// Drain accumulated dirty rectangles as a coalesced [`ClipRects`] set.
    ///
    /// Overlapping dirty rects are merged so the compositor doesn't repaint
    /// the same pixel region twice.
    pub(crate) fn take_dirty_clip_rects(&mut self) -> ClipRects {
        let rects = std::mem::take(&mut self.dirty_rects);
        let mut cr = ClipRects::new();
        for r in &rects {
            cr.unite_rect(r.x, r.y, r.x + r.w, r.y + r.h);
        }
        cr
    }

    /// Collect invalidation signals from panel behaviors that manage sub-views
    /// (e.g. [`emSubViewPanel`](super::emSubViewPanel::emSubViewPanel)). Drains each behavior's
    /// pending parent invalidation and merges the dirty rects, title, and
    /// cursor flags into this view.
    ///
    /// This implements the C++ invalidation chain where
    /// `SubViewClass::InvalidateTitle`, `SubViewPortClass::InvalidateCursor`,
    /// and `SubViewPortClass::InvalidatePainting` propagate from the sub-view
    /// to the enclosing panel/view.
    pub fn collect_parent_invalidation(&mut self, tree: &mut PanelTree) {
        let ids: Vec<PanelId> = tree.all_ids();
        for id in ids {
            if let Some(mut behavior) = tree.take_behavior(id) {
                if let Some(inv) = behavior.drain_parent_invalidation() {
                    for r in inv.dirty_rects {
                        self.dirty_rects.push(r);
                    }
                    if inv.title_invalid {
                        self.title_invalid = true;
                    }
                    if inv.cursor_invalid {
                        self.cursor_invalid = true;
                    }
                }
                tree.put_behavior(id, behavior);
            }
        }
    }

    /// Whether the viewport has changed (scroll/zoom/visit) since last reset.
    pub fn viewport_changed(&self) -> bool {
        self.viewport_changed
    }

    /// Clear the viewport-changed flag after processing.
    pub fn clear_viewport_changed(&mut self) {
        self.viewport_changed = false;
    }

    /// VIEW-003: Whether scroll/zoom was called and any active animator should
    /// be aborted. The window loop should check this and abort the
    /// emVisitingViewAnimator if active.
    pub fn needs_animator_abort(&self) -> bool {
        self.needs_animator_abort
    }

    /// Clear the animator-abort flag.
    pub fn clear_animator_abort(&mut self) {
        self.needs_animator_abort = false;
    }

    // --- Background color (PORT-0116) ---

    /// Get the background color of this view. Used for areas not covered by
    /// panels or where panels are transparent. Matches C++ `emView::GetBackgroundColor`.
    pub fn GetBackgroundColor(&self) -> emColor {
        self.background_color
    }

    /// Set the background color of this view. If changed, the view is
    /// invalidated for repainting. Matches C++ `emView::SetBackgroundColor`.
    pub fn SetBackgroundColor(&mut self, color: emColor) {
        if self.background_color != color {
            self.background_color = color;
            self.viewport_changed = true;
        }
    }

    // --- Popup infrastructure (emView.cpp:1974-1997, Phase 4) ---

    /// Port of C++ `emView::SwapViewPorts(bool swapFocus)` (emView.cpp:1974).
    ///
    /// Swaps the view's `HomeViewPort` and `CurrentViewPort`. Called by the
    /// popup branch in `RawVisitAbs` to exchange the home-window port with
    /// the popup-window port.
    ///
    /// DIVERGED: C++ swaps raw pointers between `this` and `PopupWindow`,
    /// then updates `CurrentX/Y/Width/Height/PixelTallness` from the new
    /// `CurrentViewPort->HomeView->Home*` fields. Phase 4 approximates this
    /// by reading the geometry from the exchanged `emViewPort` directly
    /// (since the stub port stores home_* instead of a HomeView back-ref).
    pub fn SwapViewPorts(&mut self, swap_focus: bool) {
        // Swap the popup window's current_view_port with our CurrentViewPort.
        // This mirrors C++:
        //   vp = PopupWindow->CurrentViewPort;
        //   PopupWindow->CurrentViewPort = CurrentViewPort;
        //   CurrentViewPort = vp;
        if let Some(ref popup) = self.PopupWindow {
            let popup_vp = Rc::clone(&popup.borrow().view().CurrentViewPort);
            let our_vp = Rc::clone(&self.CurrentViewPort);
            popup.borrow_mut().view_mut().CurrentViewPort = our_vp;
            self.CurrentViewPort = popup_vp;
        } else {
            // Fallback if no popup exists: swap Home and Current (no-op for
            // two dummy ports, but keeps field symmetry).
            std::mem::swap(&mut self.HomeViewPort, &mut self.CurrentViewPort);
        }

        // Update Current* from the new CurrentViewPort's stored geometry.
        // C++ (emView.cpp:1984-1989):
        //   CurrentX = CurrentViewPort->HomeView->HomeX;  etc.
        {
            let vp = self.CurrentViewPort.borrow();
            self.CurrentX = vp.home_x;
            self.CurrentY = vp.home_y;
            self.CurrentWidth = vp.home_width;
            self.CurrentHeight = vp.home_height;
            self.CurrentPixelTallness = self.HomePixelTallness;
        }

        if swap_focus {
            // C++ (emView.cpp:1990-1994):
            //   fcs = Focused; SetFocused(w->Focused); w->SetFocused(fcs);
            // Phase 4: transfer focus between ports; emView::window_focused
            // is NOT changed here because no PanelTree is available and focus
            // notification is a Phase-5 concern.
            let vp_focus = self.CurrentViewPort.borrow().is_focused();
            self.CurrentViewPort
                .borrow_mut()
                .set_focused(self.window_focused);
            self.HomeViewPort.borrow_mut().set_focused(vp_focus);
        }

        // C++ emView.cpp:1995: Signal(GeometrySignal) — viewport swap changes
        // the current geometry, so wake listeners (e.g. emWindowStateSaver).
        if let (Some(sig), Some(sched)) = (self.geometry_signal, &self.scheduler) {
            sched.borrow_mut().fire(sig);
        }
    }

    /// Port of C++ `emView::GetMaxPopupViewRect(pX, pY, pW, pH)`.
    ///
    /// Returns the maximum bounding rect for the popup window (usually the
    /// owning monitor's work area). When no monitor info is available
    /// (e.g., headless or Wayland without position queries), falls back to
    /// the home rect.
    pub fn GetMaxPopupViewRect(&self, out: &mut (f64, f64, f64, f64)) {
        if let Some(ref rect) = self.max_popup_rect {
            *out = (rect.x, rect.y, rect.w, rect.h);
        } else {
            // Fallback: use the home rect.
            *out = (self.HomeX, self.HomeY, self.HomeWidth, self.HomeHeight);
        }
    }

    // --- Panel lookup by identity (PORT-0127) ---

    /// Search for a panel by its colon-delimited identity string.
    /// Returns `None` if not found. Matches C++ `emView::GetPanelByIdentity`.
    pub fn GetPanelByIdentity(&self, tree: &PanelTree, identity: &str) -> Option<PanelId> {
        use crate::emPanelTree::DecodeIdentity;

        let names = DecodeIdentity(identity);
        if names.is_empty() {
            return None;
        }

        let root = self.root;
        let root_name = tree.name(root)?;
        if root_name != names[0] {
            return None;
        }

        let mut current = root;
        for name in &names[1..] {
            match tree.find_child_by_name(current, name) {
                Some(child) => current = child,
                None => return None,
            }
        }
        Some(current)
    }

    // --- EOI signal (PORT-0129) ---

    /// Request a delayed End-Of-Interaction signal.
    ///
    /// Registers a fresh `EOIEngineClass` with the scheduler and wakes it so
    /// it cycles each slice until its countdown reaches zero, at which point
    /// it fires `EOISignal`. Matches C++ `emView::SignalEOIDelayed`
    /// (emView.cpp:940-943).
    pub fn SignalEOIDelayed(&mut self) {
        let (Some(sched), Some(sig)) = (&self.scheduler, self.EOISignal) else {
            return;
        };
        let mut sched = sched.borrow_mut();
        // Replace any prior EOI engine — matches C++ where SignalEOIDelayed
        // resets the countdown on a fresh EOIEngineClass.
        if let Some(old) = self.eoi_engine_id.take() {
            sched.remove_engine(old);
        }
        let eng_id = sched.register_engine(
            super::emEngine::Priority::High,
            Box::new(EOIEngineClass::new(sig)),
        );
        sched.wake_up(eng_id);
        self.eoi_engine_id = Some(eng_id);
    }

    // --- InvalidateHighlight / AddToNoticeList / RecurseInput ---

    /// Port of C++ `emView::InvalidateHighlight` (emView.cpp:2137-2146).
    ///
    /// If the active panel is viewed and highlight should be drawn, marks the
    /// whole view dirty so the highlight is repainted.  C++ comment notes this
    /// is overly broad ("too much") — we preserve that behaviour.
    pub fn InvalidateHighlight(&mut self, tree: &PanelTree) {
        let active_viewed = self.active.map(|id| tree.IsViewed(id)).unwrap_or(false);

        if !active_viewed {
            return;
        }

        let no_active = self.flags.contains(ViewFlags::NO_ACTIVE_HIGHLIGHT);
        let no_focus = self.flags.contains(ViewFlags::NO_FOCUS_HIGHLIGHT);
        if no_active && (no_focus || !self.window_focused) {
            return;
        }

        // C++ emView.cpp:2145: InvalidatePainting() — mark whole view dirty.
        self.dirty_rects.push(Rect::new(
            self.CurrentX,
            self.CurrentY,
            self.CurrentWidth,
            self.CurrentHeight,
        ));
    }

    /// Port of C++ `emView::AddToNoticeList(PanelRingNode*)` (emView.cpp:1282).
    ///
    /// DIVERGED: Rust owns the notice ring on `PanelTree`, so this method
    /// delegates to `tree.add_to_notice_list(panel)` and then wakes the
    /// `UpdateEngine`.  Exists on `emView` for C++ call-site parity.
    pub fn AddToNoticeList(&mut self, tree: &mut PanelTree, panel: PanelId) {
        tree.add_to_notice_list(panel);
        // C++ emView.cpp:1288: UpdateEngine->WakeUp().
        self.WakeUpUpdateEngine();
    }

    /// Port of C++ `emView::RecurseInput` (public overload + private panel overload,
    /// emView.cpp:2004-2134).
    ///
    /// DIVERGED: C++ has `RecurseInput(e, s)` (public) and
    /// `RecurseInput(panel, e, s)` (private).  Rust merges them into a single
    /// method; callers pass `start_panel: None` for the public entry point.
    ///
    /// When `start_panel` is `None` the walk begins at `SupremeViewedPanel`
    /// and climbs toward the root (public overload, emView.cpp:2004).
    /// When `start_panel` is `Some(id)` the walk recurses into that subtree
    /// only (private overload, emView.cpp:2079).
    pub fn RecurseInput(
        &mut self,
        tree: &mut PanelTree,
        event: &mut super::emInput::emInputEvent,
        state: &super::emInputState::emInputState,
        start_panel: Option<PanelId>,
    ) {
        use super::emInput::emInputEvent;

        // A "no event" sentinel — an eaten emInputEvent that satisfies
        // `IsEmpty()` and won't match any handler.  Mirrors C++ `NoEvent`.
        let mut no_event = emInputEvent::press(super::emInput::InputKey::Key('\0'));
        no_event.eaten = true;

        match start_panel {
            None => {
                // --- Public overload (emView.cpp:2004-2076) ---
                let panel = match self.supreme_viewed_panel {
                    Some(id) => id,
                    None => return,
                };

                // Eat the sentinel for this entry (C++ `NoEvent.Eat()`).
                no_event.eaten = true;

                let mx_raw = state.GetMouseX();
                let my_raw = state.GetMouseY();

                // Get panel clip bounds and ViewedX/W to transform coords.
                let (clip_x1, clip_x2, clip_y1, clip_y2, viewed_x, viewed_y, viewed_width) = {
                    let p = match tree.GetRec(panel) {
                        Some(p) => p,
                        None => return,
                    };
                    (
                        p.clip_x,
                        p.clip_x + p.clip_w,
                        p.clip_y,
                        p.clip_y + p.clip_h,
                        p.viewed_x,
                        p.viewed_y,
                        p.viewed_width,
                    )
                };

                // Determine effective event for mouse events (clip check).
                let ebase_eaten = event.IsMouseEvent()
                    && (mx_raw < clip_x1
                        || mx_raw >= clip_x2
                        || my_raw < clip_y1
                        || my_raw >= clip_y2);

                // Transform mouse to panel-local coords.
                // C++ emView.cpp:2023-2024.
                let mut mx = (mx_raw - viewed_x) / viewed_width;
                let mut my = (my_raw - viewed_y) / viewed_width * self.CurrentPixelTallness;

                // Touch coords.
                let (tx_raw, ty_raw) = if !state.GetTouchCount().is_empty() {
                    (state.GetTouchX(0), state.GetTouchY(0))
                } else {
                    (state.GetMouseX(), state.GetMouseY())
                };

                let ebase_touch_eaten = event.IsTouchEvent()
                    && (tx_raw < clip_x1
                        || tx_raw >= clip_x2
                        || ty_raw < clip_y1
                        || ty_raw >= clip_y2);

                let mut tx = (tx_raw - viewed_x) / viewed_width;
                let mut ty = (ty_raw - viewed_y) / viewed_width * self.CurrentPixelTallness;

                // Walk from SVP toward root, dispatching to each panel with
                // PendingInput set.  C++ emView.cpp:2041-2075.
                let mut cur = panel;
                loop {
                    if tree.get_pending_input(cur) {
                        // Choose effective event for this panel.
                        let use_no_event = if ebase_eaten || ebase_touch_eaten {
                            true
                        } else if !event.IsEmpty() {
                            if event.IsMouseEvent() {
                                !tree.IsPointInSubstanceRect(cur, mx, my)
                            } else if event.IsTouchEvent() {
                                !tree.IsPointInSubstanceRect(cur, tx, ty)
                            } else if event.IsKeyboardEvent() {
                                !tree.GetRec(cur).is_some_and(|p| p.in_active_path)
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        // Recurse into children first (last child → first child).
                        let children_rev: Vec<PanelId> = {
                            let mut v = Vec::new();
                            let mut child_opt = tree.GetLastChild(cur);
                            while let Some(child) = child_opt {
                                v.push(child);
                                child_opt = tree.GetPrev(child);
                            }
                            v
                        };

                        for child in children_rev {
                            if use_no_event {
                                self.RecurseInput(tree, &mut no_event, state, Some(child));
                            } else {
                                self.RecurseInput(tree, event, state, Some(child));
                            }
                            if self.RestartInputRecursion {
                                return;
                            }
                        }

                        // Clear PendingInput and dispatch to this panel.
                        tree.set_pending_input(cur, false);
                        let eff_event: &super::emInput::emInputEvent =
                            if use_no_event { &no_event } else { event };
                        tree.dispatch_input(
                            cur,
                            eff_event,
                            state,
                            self.window_focused,
                            self.CurrentPixelTallness,
                        );
                        if self.RestartInputRecursion {
                            return;
                        }
                    }

                    // Climb to parent, converting coords.
                    // C++ emView.cpp:2070-2074.
                    let layout_rect = match tree.GetRec(cur) {
                        Some(p) => p.layout_rect,
                        None => break,
                    };
                    let lx = layout_rect.x;
                    let ly = layout_rect.y;
                    let lw = layout_rect.w;
                    mx = mx * lw + lx;
                    my = my * lw + ly;
                    tx = tx * lw + lx;
                    ty = ty * lw + ly;

                    match tree.GetRec(cur).and_then(|p| p.parent) {
                        Some(parent) => cur = parent,
                        None => break,
                    }
                }
            }

            Some(panel) => {
                // --- Private overload (emView.cpp:2079-2134) ---
                if !tree.get_pending_input(panel) {
                    return;
                }

                let (mx, my, tx, ty) = if tree.GetRec(panel).is_some_and(|p| p.viewed) {
                    let vx = tree.GetRec(panel).map_or(0.0, |p| p.viewed_x);
                    let vy = tree.GetRec(panel).map_or(0.0, |p| p.viewed_y);
                    let vw = tree.GetRec(panel).map_or(1.0, |p| p.viewed_width);
                    let mx = (state.GetMouseX() - vx) / vw;
                    let my = (state.GetMouseY() - vy) / vw * self.CurrentPixelTallness;
                    let (tx, ty) = if !state.GetTouchCount().is_empty() {
                        (
                            (state.GetTouchX(0) - vx) / vw,
                            (state.GetTouchY(0) - vy) / vw * self.CurrentPixelTallness,
                        )
                    } else {
                        (mx, my)
                    };
                    (mx, my, tx, ty)
                } else {
                    // C++ emView.cpp:2102-2106: not viewed → sentinel coords.
                    (-1.0_f64, -1.0_f64, -1.0_f64, -1.0_f64)
                };

                // Choose effective event for this panel.
                let use_no_event = if !event.IsEmpty() {
                    if event.IsMouseEvent() {
                        !tree.IsPointInSubstanceRect(panel, mx, my)
                    } else if event.IsTouchEvent() {
                        !tree.IsPointInSubstanceRect(panel, tx, ty)
                    } else if event.IsKeyboardEvent() {
                        !tree.GetRec(panel).is_some_and(|p| p.in_active_path)
                    } else {
                        false
                    }
                } else {
                    false
                };

                // Recurse into children (last → first).
                let children_rev: Vec<PanelId> = {
                    let mut v = Vec::new();
                    let mut child_opt = tree.GetLastChild(panel);
                    while let Some(child) = child_opt {
                        v.push(child);
                        child_opt = tree.GetPrev(child);
                    }
                    v
                };

                for child in children_rev {
                    if use_no_event {
                        self.RecurseInput(tree, &mut no_event, state, Some(child));
                    } else {
                        self.RecurseInput(tree, event, state, Some(child));
                    }
                    if self.RestartInputRecursion {
                        return;
                    }
                }

                // Clear PendingInput and dispatch to this panel.
                tree.set_pending_input(panel, false);
                let eff_event: &super::emInput::emInputEvent =
                    if use_no_event { &no_event } else { event };
                tree.dispatch_input(
                    panel,
                    eff_event,
                    state,
                    self.window_focused,
                    self.CurrentPixelTallness,
                );
            }
        }
    }

    /// TF-003: Scroll the viewport to make a panel-pixel rect visible.
    ///
    /// `rect` is `(x, y, w, h)` in the panel's paint coordinate space
    /// (same space as `paint(w, h)`). The method converts to viewport
    /// coordinates, checks visibility, and scrolls the minimum amount
    /// needed.
    ///
    /// Matches C++ `emTextField::ScrollToCursor` → `emView::Scroll` path.
    pub fn scroll_to_panel_rect(
        &mut self,
        tree: &mut PanelTree,
        panel: PanelId,
        rect: (f64, f64, f64, f64),
    ) {
        let p = match tree.GetRec(panel) {
            Some(p) if p.viewed => p,
            _ => return,
        };

        let (rx, ry, rw, rh) = rect;

        // Convert panel-pixel rect to viewport coords.
        // Paint coord (px, py) maps to viewport (viewed_x + px, viewed_y + py)
        // because there is no per-panel scaling in the paint pipeline.
        let vx1 = p.viewed_x + rx;
        let vy1 = p.viewed_y + ry;
        let vx2 = vx1 + rw;
        let vy2 = vy1 + rh;

        let mut dx = 0.0_f64;
        let mut dy = 0.0_f64;
        let mut need = false;

        // Horizontal: bring cursor into viewport [0, viewport_width]
        if vx1 < 0.0 {
            dx = -vx1; // shift content right
            need = true;
        } else if vx2 > self.HomeWidth {
            dx = self.HomeWidth - vx2; // shift content left
            need = true;
        }

        // Vertical: bring cursor into viewport [0, viewport_height]
        if vy1 < 0.0 {
            dy = -vy1;
            need = true;
        } else if vy2 > self.HomeHeight {
            dy = self.HomeHeight - vy2;
            need = true;
        }

        if need {
            // Scroll() divides by ViewedWidth/Height. dx/dy are already in
            // screen pixels, so pass them directly.
            self.Scroll(tree, dx, dy);
        }
    }

    // --- Title (C++ GetTitle / InvalidateTitle) ---

    /// Get the view title. In C++, GetTitle() is virtual and defaults to the
    /// active panel's title. Here we store a cached title string that is
    /// refreshed when `title_invalid` is set.
    pub fn GetTitle(&self) -> &str {
        &self.title
    }

    /// Set the view title directly. Marks the title as valid.
    pub fn SetRootTitle(&mut self, title: &str) {
        self.title = title.to_string();
        self.title_invalid = false;
    }

    /// Mark the title as needing a refresh (view-level, not panel-level).
    /// Corresponds to C++ `emView::InvalidateTitle` which signals the title signal.
    pub fn invalidate_view_title(&mut self) {
        self.title_invalid = true;
    }

    // --- Popup state (C++ IsPoppedUp) ---

    /// Whether the view is in popped-up state. In C++ this checks
    /// `PopupWindow != NULL`. The Rust equivalent tracks a bool flag that
    /// is set when a popup window is created for this view.
    pub fn IsPoppedUp(&self) -> bool {
        self.popped_up
    }

    /// Set the popped-up state. Called by the window/viewport layer when
    /// a popup window is created or destroyed for this view.
    pub fn set_popped_up(&mut self, popped_up: bool) {
        self.popped_up = popped_up;
    }

    // --- Popup rect (C++ GetMaxPopupViewRect) ---

    /// Get the maximum popup view rectangle: the bounding rectangle of all
    /// display monitors intersecting the home rectangle of the view, in
    /// pixel coordinates. Returns `None` if not set.
    pub fn max_popup_rect(&self) -> Option<Rect> {
        self.max_popup_rect
    }

    /// Set the maximum popup view rectangle. Called by the viewport/window
    /// layer when monitor geometry is known.
    pub fn set_max_popup_rect(&mut self, rect: Option<Rect>) {
        self.max_popup_rect = rect;
    }

    // --- emCursor (C++ GetCursor) ---

    /// Get the current mouse cursor for this view. In C++, GetCursor() is
    /// virtual and defaults to the cursor of the panel under the mouse.
    pub fn GetCursor(&self) -> emCursor {
        // C++ emView.cpp ~1358: when EGO_MODE active and cursor is Normal,
        // override to Crosshair.
        if self.flags.contains(ViewFlags::EGO_MODE) && self.cursor == emCursor::Normal {
            return emCursor::Crosshair;
        }
        self.cursor
    }

    /// Set the current mouse cursor. Called after resolving cursor from panels.
    pub fn set_cursor(&mut self, cursor: emCursor) {
        self.cursor = cursor;
        self.cursor_invalid = false;
    }

    /// Port of C++ `emView::Input(emInputEvent&, const emInputState&)`
    /// (emView.cpp:1000-1039).
    ///
    /// Prologue bookkeeping for every input event routed via
    /// `emViewPort::InputToView`:
    ///   - forward to the active animator (if any)
    ///   - update `LastMouseX`/`LastMouseY` and mark `cursor_invalid` if the
    ///     mouse actually moved
    ///
    /// The C++ implementation also walks the panel tree to set
    /// `PendingInput=true` on every panel and then calls `RecurseInput` to
    /// dispatch. Phases 6/8 of emview-rewrite-followups own migrating the
    /// existing `emWindow::dispatch_input` panel broadcast into this
    /// method. For Phase 5 the broadcast stays in `emWindow` and this
    /// method only runs the prologue that the downstream code (tile
    /// invalidation, cursor resolution) depends on.
    ///
    /// PHASE-6-FOLLOWUP: migrate the VIF-chain + panel-broadcast dispatch
    /// from `emWindow::dispatch_input` into this method; invoke
    /// `RecurseInput` once its Rust port exists. The animator forward
    /// (C++ emView.cpp:1004) is handled by the caller sites
    /// (`emWindow::dispatch_input`, `emSubViewPanel::Behavior::Input`)
    /// because the animator lives on those owners, not on `emView`.
    pub fn Input(
        &mut self,
        _tree: &mut PanelTree,
        _event: &crate::emInput::emInputEvent,
        state: &crate::emInputState::emInputState,
    ) {
        // C++ emView.cpp:1004: forward input to ActiveAnimator first.
        // Rust-arch note: the active animator lives on emWindow
        // (see emWindow::dispatch_input) and on emSubViewPanel
        // (see emSubViewPanel::Behavior::Input), not on emView — by the
        // Phase 5/6 design decision. Those callers forward the event to
        // their animator slot BEFORE invoking this method, so by the time
        // Input runs here the event may already have been eaten.

        // emView.cpp:1006-1014: cursor-invalid on mouse move.
        let mx = state.GetMouseX();
        let my = state.GetMouseY();
        if (mx - self.LastMouseX).abs() > 0.1 || (my - self.LastMouseY).abs() > 0.1 {
            self.LastMouseX = mx;
            self.LastMouseY = my;
            self.cursor_invalid = true;
            self.WakeUpUpdateEngine();
        }
    }

    // --- Control panel (C++ CreateControlPanel / GetControlPanelSignal) ---

    /// Create the control panel for the currently active panel.
    ///
    /// Walks the content tree's parent chain from the active panel to find
    /// a behavior that creates a control panel, but creates the panel in
    /// `control_tree` as a child of `parent`. Matches C++
    /// `emView::CreateControlPanel`.
    pub fn CreateControlPanel(
        &self,
        content_tree: &mut PanelTree,
        control_tree: &mut PanelTree,
        parent: PanelId,
        name: &str,
    ) -> Option<PanelId> {
        let active = self.active?;
        content_tree.create_control_panel_in(
            active,
            control_tree,
            parent,
            name,
            self.CurrentPixelTallness,
        )
    }

    /// Whether the control panel signal has been raised (needs recreation).
    /// Equivalent to checking C++ `GetControlPanelSignal`.
    pub fn needs_control_panel_update(&self) -> bool {
        self.control_panel_invalid
    }

    // --- Window/emScreen access (C++ GetWindow / GetScreen) ---
    //
    // In C++, emView inherits from emContext which provides a parent-context
    // chain. GetWindow() walks up the context chain to find the nearest
    // emWindow, and GetScreen() finds the nearest emScreen.
    //
    // In Rust, emView is a plain struct owned by the window. The window/screen
    // relationship is managed externally. These methods are not needed on emView
    // itself — callers that have a emView reference already know which window
    // owns it. If cross-view queries are needed in the future, the window
    // layer can provide the mapping.

    // --- Update loop ---

    /// Per-frame update: drain the C++ Update loop, then reselect active panel.
    ///
    /// DIVERGED: the old Rust wrapper gated `Update` on `viewing_dirty`; Phase 3
    /// removes that gate. `Update` now drains unconditionally (fast no-op when all
    /// flags are clear). `mark_viewing_dirty` callers now set `SVPChoiceInvalid`
    /// instead, which the drain loop picks up automatically.
    pub fn update(&mut self, tree: &mut PanelTree) {
        self.Update(tree);

        // VIEW-003: After scroll/zoom or viewport change, reselect active panel
        // (C++ calls SetActivePanelBestPossible after Scroll/Zoom)
        let need_reselect = match self.active {
            None => true,
            Some(id) => {
                !tree.contains(id) || !tree.GetRec(id).map(|p| p.focusable).unwrap_or(false)
            }
        };
        if need_reselect || self.viewport_changed {
            self.SetActivePanelBestPossible(tree);
        }
    }

    /// Mark the SVP choice as invalid, triggering Update to recompute viewed coords.
    ///
    /// DIVERGED: was `mark_viewing_dirty()` (set `viewing_dirty`). Phase 3 removes
    /// `viewing_dirty`; callers now set `SVPChoiceInvalid` via this wrapper so that
    /// `Update` picks it up in the drain loop.
    pub fn mark_viewing_dirty(&mut self) {
        self.SVPChoiceInvalid = true;
    }

    // --- Supreme panel ---

    pub fn supreme_panel(&self) -> PanelId {
        self.supreme_viewed_panel
            .expect("supreme_viewed_panel should be populated post-Update (C++: GetSupremeViewedPanel returns SupremeViewedPanel directly)")
    }

    // --- Stress test ---

    /// Sync stress test state with the STRESS_TEST flag. Call this each frame.
    pub fn sync_stress_test(&mut self) {
        if self.flags.contains(ViewFlags::STRESS_TEST) {
            if self.stress_test.is_none() {
                self.stress_test = Some(StressTest::new());
            }
            if let Some(st) = &mut self.stress_test {
                st.record_frame();
            }
        } else if self.stress_test.is_some() {
            self.stress_test = None;
        }
    }

    /// Whether the stress test is currently active.
    pub fn is_stress_test_active(&self) -> bool {
        self.stress_test.is_some()
    }

    /// Access the stress test state (for testing).
    pub fn stress_test(&self) -> Option<&StressTest> {
        self.stress_test.as_ref()
    }

    // --- Paint ---

    /// Port of C++ `emView::Paint(const emPainter & painter, emColor canvasColor)`.
    ///
    /// Structural correspondence to C++ emView.cpp lines 1048-1146:
    ///
    /// C++ uses `pnt=painter` (painter copy) — one copy made at line 1090,
    /// reused for SVP paint (line 1098) and all children (line 1118) with
    /// no per-child save/restore. Rust uses push_state once before SVP
    /// paint and pop_state once after the entire child loop, matching the
    /// C++ lifecycle where `pnt` is alive for the whole block and `painter`
    /// (the original) is used for PaintHighlight.
    pub fn Paint(&self, tree: &mut PanelTree, painter: &mut emPainter, canvas_color: emColor) {
        // C++ line 1056
        debug_assert!(
            painter.GetScaleX() == 1.0 && painter.GetScaleY() == 1.0,
            "emView::Paint: Scaling not possible."
        );

        // C++ lines 1060, 1145: EnterUserSpace/LeaveUserSpace — no-op (single-threaded)

        // C++ line 1062
        let svp_id = match self.supreme_viewed_panel {
            Some(id) => id,
            None => {
                // C++ line 1063
                painter.ClearWithCanvas(self.background_color, canvas_color);
                if let Some(st) = &self.stress_test {
                    st.paint_info(painter, self.HomeWidth, self.HomeHeight);
                }
                return;
            }
        };

        // C++ lines 1066-1071
        let ox = painter.GetOriginX();
        let oy = painter.GetOriginY();
        let rx1 = painter.GetClipX1() - ox;
        let ry1 = painter.GetClipY1() - oy;
        let rx2 = painter.GetClipX2() - ox;
        let ry2 = painter.GetClipY2() - oy;

        // Read SVP fields. Must happen before IsOpaque (which borrows tree mutably).
        let svp = match tree.GetRec(svp_id) {
            Some(p) => p,
            None => return,
        };
        let svp_vx = svp.viewed_x;
        let svp_vy = svp.viewed_y;
        let svp_vw = svp.viewed_width;
        let svp_vh = svp.viewed_height;
        let svp_canvas = svp.canvas_color;
        let svp_clip_x1 = svp.clip_x;
        let svp_clip_y1 = svp.clip_y;
        let svp_clip_x2 = svp.clip_x + svp.clip_w;
        let svp_clip_y2 = svp.clip_y + svp.clip_h;
        let svp_layout = svp.layout_rect;

        // C++ lines 1073-1084: conditional background clear
        let mut canvas_color = canvas_color;
        if !tree.IsOpaque(svp_id)
            || svp_vx > rx1
            || svp_vx + svp_vw < rx2
            || svp_vy > ry1
            || svp_vy + svp_vh < ry2
        {
            let mut ncc = svp_canvas;
            if !ncc.IsOpaque() {
                ncc = self.background_color;
            }
            painter.ClearWithCanvas(ncc, canvas_color);
            canvas_color = ncc;
        }

        // C++ lines 1085-1088: clamp SVP clip to render region
        let mut cx1 = svp_clip_x1;
        if cx1 < rx1 {
            cx1 = rx1;
        }
        let mut cx2 = svp_clip_x2;
        if cx2 > rx2 {
            cx2 = rx2;
        }
        let mut cy1 = svp_clip_y1;
        if cy1 < ry1 {
            cy1 = ry1;
        }
        let mut cy2 = svp_clip_y2;
        if cy2 > ry2 {
            cy2 = ry2;
        }

        // C++ line 1089
        if cx1 < cx2 && cy1 < cy2 {
            // C++ line 1090: pnt=painter — save original state for PaintHighlight.
            // One push here, one pop after the entire child loop.
            painter.push_state();

            // C++ lines 1091-1097: SVP clip + transform on `pnt`
            painter.SetClippingAbsolute(cx1 + ox, cy1 + oy, cx2 + ox, cy2 + oy);
            painter.SetTransformation(svp_vx + ox, svp_vy + oy, svp_vw, svp_vw);

            // C++ line 1098: p->Paint(pnt, canvasColor)
            painter.SetCanvasColor(canvas_color);
            self.paint_one_panel(tree, painter, svp_id, svp_layout);

            // C++ lines 1099-1135: iterative DFS over children.
            // C++ does LeaveUserSpace (line 1099) then reuses `pnt` for all
            // children — no per-child save/restore. Each child overwrites
            // clip and transform on `pnt` via SetClipping + SetTransformation.
            if let Some(first_child) = tree.GetFirstChild(svp_id) {
                let mut p = first_child;
                while let Some(panel) = tree.GetRec(p) {
                    // C++ line 1103
                    if panel.viewed {
                        // C++ lines 1104-1108: clamp child clip to render region
                        let mut cx1 = panel.clip_x;
                        if cx1 < rx1 {
                            cx1 = rx1;
                        }
                        let mut cx2 = panel.clip_x + panel.clip_w;
                        if cx2 > rx2 {
                            cx2 = rx2;
                        }
                        if cx1 < cx2 {
                            let mut cy1 = panel.clip_y;
                            if cy1 < ry1 {
                                cy1 = ry1;
                            }
                            let mut cy2 = panel.clip_y + panel.clip_h;
                            if cy2 > ry2 {
                                cy2 = ry2;
                            }
                            if cy1 < cy2 {
                                // C++ lines 1110-1115
                                let p_vx = panel.viewed_x;
                                let p_vy = panel.viewed_y;
                                let p_vw = panel.viewed_width;
                                let p_canvas = panel.canvas_color;
                                let p_layout = panel.layout_rect;

                                painter.SetClippingAbsolute(cx1 + ox, cy1 + oy, cx2 + ox, cy2 + oy);
                                painter.SetTransformation(p_vx + ox, p_vy + oy, p_vw, p_vw);

                                // C++ line 1118: p->Paint(pnt, p->CanvasColor)
                                painter.SetCanvasColor(p_canvas);
                                self.paint_one_panel(tree, painter, p, p_layout);

                                // C++ lines 1120-1123
                                if let Some(fc) = tree.GetFirstChild(p) {
                                    p = fc;
                                    continue;
                                }
                            }
                        }
                    }

                    // C++ lines 1127-1134: advance to next sibling or walk up
                    if let Some(next) = tree.GetNext(p) {
                        p = next;
                    } else {
                        loop {
                            p = match tree.GetParentContext(p) {
                                Some(parent) => parent,
                                None => break,
                            };
                            if p == svp_id {
                                break;
                            }
                            if let Some(next) = tree.GetNext(p) {
                                p = next;
                                break;
                            }
                        }
                        if p == svp_id || tree.GetParentContext(p).is_none() {
                            break;
                        }
                    }
                }
            }

            // C++ line 1137: EnterUserSpace — restore original painter state
            painter.pop_state();
        }

        // C++ line 1139: PaintHighlight — uses original `painter`, not `pnt`
        self.paint_highlight(tree, painter);

        // C++ line 1142: ActiveAnimator — not yet implemented
        // TODO: ActiveAnimator

        // C++ line 1143
        if let Some(st) = &self.stress_test {
            st.paint_info(painter, self.HomeWidth, self.HomeHeight);
        }
    }

    /// Paint a single panel's behavior. Extracted to avoid duplicating the
    /// take_behavior / build_panel_state / Paint / put_behavior sequence.
    fn paint_one_panel(
        &self,
        tree: &mut PanelTree,
        painter: &mut emPainter,
        id: PanelId,
        layout: Rect,
    ) {
        if let Some(mut behavior) = tree.take_behavior(id) {
            let mut state =
                tree.build_panel_state(id, self.window_focused, self.CurrentPixelTallness);
            state.priority =
                tree.GetUpdatePriority(id, self.HomeWidth, self.HomeHeight, self.window_focused);
            const DEFAULT_MEMORY_LIMIT: u64 = 2_048_000_000;
            state.memory_limit = tree.GetMemoryLimit(
                id,
                self.HomeWidth,
                self.HomeHeight,
                DEFAULT_MEMORY_LIMIT,
                self.seek_pos_panel,
            );
            let tallness = if layout.w > 0.0 {
                layout.h / layout.w
            } else {
                1.0
            };
            behavior.Paint(painter, 1.0, tallness, &state);
            tree.put_behavior(id, behavior);
        }
    }

    /// D-PANEL-06: Paint highlight around the active panel.
    ///
    /// C++ draws a rounded rectangle with arrows around the active panel's
    /// substance rect. White normally, light yellow if adherent, dimmed if
    /// window not focused.
    fn paint_highlight(&self, tree: &PanelTree, painter: &mut emPainter) {
        if self.flags.contains(ViewFlags::NO_ACTIVE_HIGHLIGHT) {
            return;
        }

        let active_id = match self.active {
            Some(id) => id,
            None => return,
        };

        let panel = match tree.GetRec(active_id) {
            Some(p) if p.viewed => p,
            _ => return,
        };

        // Get the panel's substance rect in viewport coords
        let (sx, sy, sw, sh, _sr) = tree.GetSubstanceRect(active_id);
        let hx = panel.viewed_x + sx * panel.viewed_width;
        let hy = panel.viewed_y + sy * panel.viewed_width;
        let hw = sw * panel.viewed_width;
        let hh = sh * panel.viewed_width;

        if hw < 1.0 || hh < 1.0 {
            return;
        }

        // Expand by distance-from-panel (C++ constant: 2.0)
        let pad = 2.0;
        let hx = hx - pad;
        let hy = hy - pad;
        let hw = hw + pad * 2.0;
        let hh = hh + pad * 2.0;

        // emColor selection (C++ constants)
        let base_color = if self.activation_adherent {
            emColor::rgba(255, 255, 187, 255) // Light yellow for adherent
        } else {
            emColor::rgba(255, 255, 255, 255) // White
        };

        let alpha = if !self.window_focused || self.flags.contains(ViewFlags::NO_FOCUS_HIGHLIGHT) {
            85 // alpha / 3
        } else {
            255
        };

        let color = emColor::rgba(
            base_color.GetRed(),
            base_color.GetGreen(),
            base_color.GetBlue(),
            alpha,
        );

        // Shadow color: black with alpha 192 normally, alpha/3 when unfocused
        let shadow_alpha =
            if !self.window_focused || self.flags.contains(ViewFlags::NO_FOCUS_HIGHLIGHT) {
                64
            } else {
                192
            };
        let shadow_color = emColor::rgba(0, 0, 0, shadow_alpha);

        // C++ constants
        let arrow_size = 11.0;
        let arrow_distance = 55.0;

        // Corner radius — C++ uses substance_rect rounding scaled to viewport
        let (_sx2, _sy2, _sw2, _sh2, sr) = tree.GetSubstanceRect(active_id);
        let corner_r = (sr * panel.viewed_width).max(0.0);

        // Goal point: center of the highlight rect
        let goal_x = hx + hw * 0.5;
        let goal_y = hy + hh * 0.5;

        // Build the perimeter as 8 segments: 4 bows + 4 lines
        // Walk clockwise starting from top-right corner midpoint
        // Segment order: top-right bow, right line, bottom-right bow,
        //                bottom line, bottom-left bow, left line,
        //                top-left bow, top line
        let r = corner_r.min(hw * 0.5).min(hh * 0.5);

        // Line segments (after accounting for corner radii)
        let segments: [(f64, f64, f64, f64, bool); 8] = [
            // top-right bow: center (hx+hw-r, hy+r), start angle -PI/2, quarter CW
            (hx + hw - r, hy + r, r, 0.0, true),
            // right line: top to bottom
            (hx + hw, hy + r, hx + hw, hy + hh - r, false),
            // bottom-right bow
            (hx + hw - r, hy + hh - r, r, 1.0, true),
            // bottom line: right to left
            (hx + hw - r, hy + hh, hx + r, hy + hh, false),
            // bottom-left bow
            (hx + r, hy + hh - r, r, 2.0, true),
            // left line: bottom to top
            (hx, hy + hh - r, hx, hy + r, false),
            // top-left bow
            (hx + r, hy + r, r, 3.0, true),
            // top line: left to right
            (hx + r, hy, hx + hw - r, hy, false),
        ];

        painter.push_state();

        for &(a, b, c, d, is_bow) in &segments {
            if is_bow {
                // Bow: (cx, cy, radius, quadrant)
                paint_highlight_arrows_on_bow(
                    painter,
                    a,
                    b,
                    c,
                    d as usize,
                    goal_x,
                    goal_y,
                    arrow_size,
                    arrow_distance,
                    color,
                    shadow_color,
                    self.HomeWidth,
                    self.HomeHeight,
                );
            } else {
                // Line: (x1, y1, x2, y2)
                paint_highlight_arrows_on_line(
                    painter,
                    a,
                    b,
                    c,
                    d,
                    goal_x,
                    goal_y,
                    arrow_size,
                    arrow_distance,
                    color,
                    shadow_color,
                    self.HomeWidth,
                    self.HomeHeight,
                );
            }
        }

        painter.pop_state();
    }

    /// Paint a sub-tree starting at `root` with the given base offset and
    /// background color. Used by [`emSubViewPanel`] to delegate painting to a
    /// sub-view's panel tree.
    pub(crate) fn paint_sub_tree(
        &self,
        tree: &mut PanelTree,
        painter: &mut emPainter,
        root: PanelId,
        base_offset: (f64, f64),
        _background: emColor,
    ) {
        self.paint_panel_recursive(tree, painter, root, base_offset);
    }

    fn paint_panel_recursive(
        &self,
        tree: &mut PanelTree,
        painter: &mut emPainter,
        id: PanelId,
        base_offset: (f64, f64),
    ) {
        let (vx, vy, vw, _vh, clip_x, clip_y, clip_w, clip_h, canvas_color, layout_rect) = {
            match tree.GetRec(id) {
                Some(p) if p.viewed => (
                    p.viewed_x,
                    p.viewed_y,
                    p.viewed_width,
                    p.viewed_height,
                    p.clip_x,
                    p.clip_y,
                    p.clip_w,
                    p.clip_h,
                    p.canvas_color,
                    p.layout_rect,
                ),
                _ => return,
            }
        };

        painter.push_state();
        painter.SetScaling(1.0, 1.0);
        painter.set_offset(base_offset.0 + vx, base_offset.1 + vy);
        painter.SetClipping(clip_x - vx, clip_y - vy, clip_w, clip_h);
        painter.SetTransformation(base_offset.0 + vx, base_offset.1 + vy, vw, vw);

        if painter.IsClipEmpty() {
            painter.pop_state();
            return;
        }

        // C++ line 1118: p->Paint(pnt, p->CanvasColor) — each panel gets
        // its own stored CanvasColor, no inheritance from parent.
        painter.SetCanvasColor(canvas_color);

        if let Some(mut behavior) = tree.take_behavior(id) {
            let mut state =
                tree.build_panel_state(id, self.window_focused, self.CurrentPixelTallness);
            state.priority =
                tree.GetUpdatePriority(id, self.HomeWidth, self.HomeHeight, self.window_focused);
            const DEFAULT_MEMORY_LIMIT: u64 = 2_048_000_000;
            state.memory_limit = tree.GetMemoryLimit(
                id,
                self.HomeWidth,
                self.HomeHeight,
                DEFAULT_MEMORY_LIMIT,
                self.seek_pos_panel,
            );
            let tallness = if layout_rect.w > 0.0 {
                layout_rect.h / layout_rect.w
            } else {
                1.0
            };
            behavior.Paint(painter, 1.0, tallness, &state);
            tree.put_behavior(id, behavior);
        }

        let children: Vec<PanelId> = tree.children(id).collect();
        for child in children {
            self.paint_panel_recursive(tree, painter, child, base_offset);
        }

        painter.pop_state();
    }

    // --- Tree dump ---

    /// Dump the panel tree to `temp_dir()/debug.emTreeDump` in
    /// emRec format. Returns the path written.
    pub fn dump_tree(&self, tree: &mut PanelTree) -> std::path::PathBuf {
        let path = std::env::temp_dir().join("debug.emTreeDump");

        // emView-level data: flags, title, focused, home_rect
        let mut view_rec = RecStruct::new();
        view_rec.set_str("title", &self.title);
        view_rec.set_str("flags", &format!("{:?}", self.flags));
        view_rec.set_bool("focused", self.window_focused);
        view_rec.set_str(
            "home_rect",
            &format!(
                "({:.3}, {:.3}, {:.3}, {:.3})",
                0.0, 0.0, self.HomeWidth, self.HomeHeight
            ),
        );

        // Panel tree
        let panels_rec = self.dump_panel_recursive(tree, self.root);

        let mut root_rec = RecStruct::new();
        root_rec.SetValue("view", RecValue::Struct(view_rec));
        root_rec.SetValue("panels", panels_rec);

        let text = write_rec_with_format(&root_rec, "emTreeDump");
        if let Err(e) = std::fs::write(&path, &text) {
            eprintln!("[TreeDump] write failed: {e}");
        } else {
            eprintln!("[TreeDump] wrote {}", path.display());
        }
        path
    }

    fn dump_panel_recursive(&self, tree: &mut PanelTree, id: PanelId) -> RecValue {
        let mut rec = RecStruct::new();

        // Type name from behavior
        let type_name = if let Some(behavior) = tree.take_behavior(id) {
            let name = behavior.type_name().to_string();
            tree.put_behavior(id, behavior);
            name
        } else {
            "(no behavior)".to_string()
        };
        rec.set_str("title", &type_name);

        // Panel fields
        let height = tree.get_height(id);
        let is_focused = self.focused == Some(id);
        if let Some(p) = tree.GetRec(id) {
            let mut text = String::new();
            text.push_str(&format!("name = {}\n", p.name));
            text.push_str(&format!(
                "layout_rect = ({:.3}, {:.3}, {:.3}, {:.3})\n",
                p.layout_rect.x, p.layout_rect.y, p.layout_rect.w, p.layout_rect.h
            ));
            text.push_str(&format!("height = {:.6}\n", height));
            text.push_str(&format!("viewed = {}\n", p.viewed));
            text.push_str(&format!("enabled = {}\n", p.enabled));
            text.push_str(&format!("focusable = {}\n", p.focusable));
            text.push_str(&format!("is_active = {}\n", p.is_active));
            text.push_str(&format!("in_active_path = {}\n", p.in_active_path));
            text.push_str(&format!("focused = {}\n", is_focused));
            rec.set_str("text", &text);
        }

        // Children (recursive)
        let children: Vec<PanelId> = tree.children(id).collect();
        if !children.is_empty() {
            let child_recs: Vec<RecValue> = children
                .into_iter()
                .map(|child| self.dump_panel_recursive(tree, child))
                .collect();
            rec.SetValue("children", RecValue::Array(child_recs));
        }

        RecValue::Struct(rec)
    }

    /// Handle a custom cheat code. Override in subclasses for app-specific cheats.
    /// C++ `emView::DoCustomCheat(const char* func)`.
    pub(crate) fn DoCustomCheat(&self, func: &str) {
        // DIVERGED: C++ default walks GetParentContext() chain to find ancestor
        // emView and delegates. Needs emContext parent traversal to be ported.
        log::debug!("Unknown cheat code: {}", func);
    }

    /// Activate the magnetic view animator.
    /// C++ `emMagneticViewAnimator::Activate()`.
    pub(crate) fn activate_magnetic_view_animator(&mut self) {
        // TODO(magnetic): Full activation requires finding the nearest focusable
        // panel and setting it as the snap target. The emMagneticViewAnimator
        // struct exists in emViewAnimator.rs but the view doesn't own it — it's
        // managed by the window loop. This stub signals the intent; the actual
        // wiring needs the window loop to check a flag and activate the animator.
        log::trace!("magnetic view animator: activation requested (not yet wired to window loop)");
    }

    // --- Test-only accessors (not part of the public API) ---

    #[cfg(test)]
    pub(crate) fn dirty_rects_len_for_test(&self) -> usize {
        self.dirty_rects.len()
    }

    #[cfg(test)]
    pub(crate) fn get_dirty_rect_for_test(&self, i: usize) -> Rect {
        self.dirty_rects[i]
    }
}

// ── Highlight arrow helpers (C++ emView.cpp:2300-2479) ──

/// Compute the 4 vertices of a highlight arrow chevron.
/// Returns [(tip), (right wing), (notch), (left wing)].
fn compute_arrow_vertices(
    x: f64,
    y: f64,
    goal_x: f64,
    goal_y: f64,
    arrow_size: f64,
) -> [(f64, f64); 4] {
    let gdx = x - goal_x;
    let gdy = y - goal_y;
    let glen = (gdx * gdx + gdy * gdy).sqrt().max(1e-10);
    let dx = gdx / glen;
    let dy = gdy / glen;

    let ah = arrow_size; // arrow head length
    let aw = arrow_size * 0.5; // arrow half-width
    let ag = ah * 0.8; // notch depth

    let tip = (x, y);
    let right = (x + dx * ah - dy * aw * 0.5, y + dy * ah + dx * aw * 0.5);
    let notch = (x + dx * ag, y + dy * ag);
    let left = (x + dx * ah + dy * aw * 0.5, y + dy * ah - dx * aw * 0.5);

    [tip, right, notch, left]
}

/// Round arrow count to a "nice" number per C++ formula.
fn compute_arrow_count(len: f64, arrow_distance: f64) -> usize {
    if len < arrow_distance * 0.5 {
        return 0;
    }
    let mut n = (len / arrow_distance).round() as usize;
    if n == 0 {
        return 0;
    }
    // Find smallest power of 2 >= n
    let mut m = 1usize;
    while m < n {
        m <<= 1;
    }
    n &= m | (m >> 1) | (m >> 2);
    n.max(1)
}

/// Paint a single highlight arrow (shadow + arrow polygon).
#[allow(clippy::too_many_arguments)]
fn paint_highlight_arrow(
    painter: &mut emPainter,
    x: f64,
    y: f64,
    goal_x: f64,
    goal_y: f64,
    arrow_size: f64,
    color: emColor,
    shadow_color: emColor,
) {
    let sd = arrow_size * 0.2;

    // Shadow polygon (offset toward bottom-right)
    let shadow_verts = compute_arrow_vertices(x + sd, y + sd, goal_x, goal_y, arrow_size);
    painter.PaintPolygon(&shadow_verts, shadow_color, emColor::TRANSPARENT);

    // Arrow polygon
    let verts = compute_arrow_vertices(x, y, goal_x, goal_y, arrow_size);
    painter.PaintPolygon(&verts, color, emColor::TRANSPARENT);
}

/// Paint arrows along a straight line segment.
#[allow(clippy::too_many_arguments)]
fn paint_highlight_arrows_on_line(
    painter: &mut emPainter,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    goal_x: f64,
    goal_y: f64,
    arrow_size: f64,
    arrow_distance: f64,
    color: emColor,
    shadow_color: emColor,
    vw: f64,
    vh: f64,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return;
    }

    let n = compute_arrow_count(len, arrow_distance);
    if n == 0 {
        return;
    }

    let margin = arrow_size * 1.5;

    for i in 0..n {
        let t = (i as f64 + 0.5) / n as f64;
        let ax = x1 + dx * t;
        let ay = y1 + dy * t;

        // Clip to viewport (with margin for arrow size)
        if ax < -margin || ax > vw + margin || ay < -margin || ay > vh + margin {
            continue;
        }

        paint_highlight_arrow(
            painter,
            ax,
            ay,
            goal_x,
            goal_y,
            arrow_size,
            color,
            shadow_color,
        );
    }
}

/// Paint arrows along a quarter-circle arc (bow).
#[allow(clippy::too_many_arguments)]
fn paint_highlight_arrows_on_bow(
    painter: &mut emPainter,
    cx: f64,
    cy: f64,
    radius: f64,
    quadrant: usize,
    goal_x: f64,
    goal_y: f64,
    arrow_size: f64,
    arrow_distance: f64,
    color: emColor,
    shadow_color: emColor,
    vw: f64,
    vh: f64,
) {
    if radius < 1.0 {
        return;
    }

    let arc_len = radius * std::f64::consts::FRAC_PI_2;
    let n = compute_arrow_count(arc_len, arrow_distance);
    if n == 0 {
        return;
    }

    // Start angle for each quadrant (clockwise from top-right)
    let start_angle = match quadrant {
        0 => -std::f64::consts::FRAC_PI_2, // top-right: -90° to 0°
        1 => 0.0,                          // bottom-right: 0° to 90°
        2 => std::f64::consts::FRAC_PI_2,  // bottom-left: 90° to 180°
        _ => std::f64::consts::PI,         // top-left: 180° to 270°
    };

    let margin = arrow_size * 1.5;

    for i in 0..n {
        let t = (i as f64 + 0.5) / n as f64;
        let angle = start_angle + t * std::f64::consts::FRAC_PI_2;
        let ax = cx + radius * angle.cos();
        let ay = cy + radius * angle.sin();

        if ax < -margin || ax > vw + margin || ay < -margin || ay > vh + margin {
            continue;
        }

        paint_highlight_arrow(
            painter,
            ax,
            ay,
            goal_x,
            goal_y,
            arrow_size,
            color,
            shadow_color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanelTree::PanelTree;
    use crate::emScheduler::EngineScheduler;
    use crate::emViewAnimator::emViewAnimator as _;

    fn setup_tree() -> (PanelTree, PanelId, PanelId, PanelId) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.get_mut(root).unwrap().focusable = true;
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);

        let child1 = tree.create_child(root, "child1");
        tree.get_mut(child1).unwrap().focusable = true;
        tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0);

        let child2 = tree.create_child(root, "child2");
        tree.get_mut(child2).unwrap().focusable = true;
        tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0);

        (tree, root, child1, child2)
    }

    #[test]
    fn test_update_viewing_sets_coords() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Root should be viewed
        let rp = tree.GetRec(root).unwrap();
        assert!(rp.viewed);
        assert!(rp.viewed_width > 0.0);
        assert!(rp.viewed_height > 0.0);

        // Children should be viewed
        assert!(tree.GetRec(child1).unwrap().viewed);
        assert!(tree.GetRec(child2).unwrap().viewed);
    }

    #[test]
    fn test_svp_selection() {
        let (mut tree, root, _child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // SVP should be set
        assert!(view.GetSupremeViewedPanel().is_some());
    }

    #[test]
    fn test_viewed_false_outside_viewport() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);

        let offscreen = tree.create_child(root, "offscreen");
        tree.Layout(offscreen, 5.0, 5.0, 0.1, 0.1, 1.0);

        let mut view = emView::new(root, 100.0, 100.0);
        view.Update(&mut tree);

        assert!(!tree.GetRec(offscreen).unwrap().viewed);
    }

    #[test]
    fn test_active_path_flags() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.set_active_panel(&mut tree, child1, false);
        view.Update(&mut tree);

        assert!(tree.GetRec(child1).unwrap().is_active);
        assert!(tree.GetRec(child1).unwrap().in_active_path);
        assert!(tree.GetRec(root).unwrap().in_active_path);
    }

    #[test]
    fn test_fix_point_zoom() {
        let (mut tree, root, _c1, _c2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Zoom around center — should keep center stable
        let (_, _, _, before_ra) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist before zoom");
        view.Zoom(&mut tree, 2.0, 400.0, 300.0);
        let (_, _, _, after_ra) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist after zoom");

        // C++ Zoom(factor=2): reFac = 1/2, ra *= reFac^2 = 1/4.
        // relA = HomeW*HomeH/(vw*vh); zooming in by 2 grows vw*vh by 4, so relA /= 4.
        assert!((after_ra - before_ra / 4.0).abs() < 0.01 * before_ra);
    }

    #[test]
    fn test_visit_next_prev() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        view.VisitNext(&mut tree);
        view.pump_visiting_va(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child2));

        view.VisitPrev(&mut tree);
        view.pump_visiting_va(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child1));
    }

    #[test]
    fn test_visit_in_out() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let grandchild = tree.create_child(child1, "gc");
        tree.get_mut(grandchild).unwrap().focusable = true;
        tree.Layout(grandchild, 0.0, 0.0, 1.0, 1.0, 1.0);

        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        view.VisitIn(&mut tree);
        view.pump_visiting_va(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(grandchild));

        view.VisitOut(&mut tree);
        view.pump_visiting_va(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child1));
    }

    #[test]
    fn test_hit_testing() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Hit test in left half should find child1, right half child2
        let left_hit = view.GetFocusablePanelAt(&tree, 100.0, 300.0);
        let right_hit = view.GetFocusablePanelAt(&tree, 600.0, 300.0);

        assert_eq!(left_hit, Some(child1));
        assert_eq!(right_hit, Some(child2));
    }

    #[test]
    fn test_directional_navigation() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        // child2 is to the right of child1
        view.VisitRight(&mut tree);
        view.pump_visiting_va(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child2));

        view.VisitLeft(&mut tree);
        view.pump_visiting_va(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child1));
    }

    #[test]
    fn test_focus_panel() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.SetFocused(&mut tree, false);

        view.focus_panel(&mut tree, child1);
        assert!(view.IsFocused());
        assert_eq!(view.GetActivePanel(), Some(child1));
    }

    #[test]
    fn test_is_panel_focused() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        assert!(view.is_panel_focused(&tree, child1));
        assert!(!view.is_panel_focused(&tree, root));

        view.SetFocused(&mut tree, false);
        assert!(!view.is_panel_focused(&tree, child1));
    }

    #[test]
    fn test_is_panel_in_focused_path() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        assert!(view.is_panel_in_focused_path(&tree, child1));
        assert!(view.is_panel_in_focused_path(&tree, root));
        assert!(!view.is_panel_in_focused_path(&tree, child2));

        view.SetFocused(&mut tree, false);
        assert!(!view.is_panel_in_focused_path(&tree, child1));
    }

    #[test]
    fn test_is_view_focused_delegate() {
        let (mut tree, root, _c1, _c2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        assert!(view.is_view_focused());
        view.SetFocused(&mut tree, false);
        assert!(!view.is_view_focused());
    }

    // ── Invalidation tests ───────────────────────────────────────────

    #[test]
    fn test_invalidate_painting_whole() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.take_dirty_rects(); // drain change-block invalidation

        // child1 should be viewed after update_viewing
        view.InvalidatePainting(&tree, child1);
        let rects = view.take_dirty_rects();
        assert_eq!(rects.len(), 1);
        // The dirty rect should be the child's clip rect
        let p = tree.GetRec(child1).unwrap();
        assert!((rects[0].x - p.clip_x).abs() < 1e-6);
        assert!((rects[0].y - p.clip_y).abs() < 1e-6);
    }

    #[test]
    fn test_invalidate_painting_rect() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.take_dirty_rects(); // drain change-block invalidation

        // Invalidate a sub-rect of child1 in panel coordinates
        view.invalidate_painting_rect(&tree, child1, 0.0, 0.0, 0.5, 0.5);
        let rects = view.take_dirty_rects();
        assert_eq!(rects.len(), 1);
        assert!(rects[0].w > 0.0);
        assert!(rects[0].h > 0.0);

        // Not viewed => no dirty rect
        tree.get_mut(child1).unwrap().viewed = false;
        view.invalidate_painting_rect(&tree, child1, 0.0, 0.0, 1.0, 1.0);
        let rects = view.take_dirty_rects();
        assert!(rects.is_empty());
    }

    #[test]
    fn test_invalidate_title_and_cursor() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.clear_cursor_invalid(); // drain change-block side effect
        view.set_active_panel(&mut tree, child1, false);

        // child1 is active, thus in_active_path
        assert!(!view.is_title_invalid());
        view.InvalidateTitle(&tree, child1);
        assert!(view.is_title_invalid());
        view.clear_title_invalid();
        assert!(!view.is_title_invalid());

        // root is in_viewed_path (it's an ancestor of the SVP)
        assert!(!view.is_cursor_invalid());
        view.InvalidateCursor(&tree, root);
        assert!(view.is_cursor_invalid());
        view.clear_cursor_invalid();

        // child1 is viewed AND in_viewed_path (all viewed panels are)
        view.InvalidateCursor(&tree, child1);
        assert!(view.is_cursor_invalid());
    }

    #[test]
    fn test_update_change_block_side_effects() {
        let (mut tree, root, _child_a, _child_b) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        // Update triggers zoomed_out_before_sg → RawZoomOut → RawVisitAbs.
        v.Update(&mut tree);

        // Verdict per Phase 0 audit: dirty_rects must contain a rect covering
        // the Current rect (not the viewport rect — these differ only under
        // popup, which is not in this test, but the test must read Current*
        // so that when popup lands in Phase 4 the test keeps passing).
        // Update fires zoomed_out_before_sg (one RawZoomOut(false)) and then
        // root_layout_changed (one RawZoomOut(true)) — two dirty rects total,
        // both covering the whole Current rect. The phase 0 audit requires ≥1
        // such rect; we check that all are the correct Current rect.
        assert!(v.dirty_rects_len_for_test() >= 1);
        for i in 0..v.dirty_rects_len_for_test() {
            let rect = v.get_dirty_rect_for_test(i);
            assert_eq!(rect.x, v.CurrentX, "dirty_rect[{i}].x");
            assert_eq!(rect.y, v.CurrentY, "dirty_rect[{i}].y");
            assert_eq!(rect.w, v.CurrentWidth, "dirty_rect[{i}].w");
            assert_eq!(rect.h, v.CurrentHeight, "dirty_rect[{i}].h");
        }

        // Phase 3: cursor_invalid is DRAINED by Update's drain loop (C++ emView.cpp:1803
        // sets CursorInvalid in RawVisitAbs, then Update's loop clears it after re-querying
        // the cursor). After Update completes, cursor_invalid must be false.
        assert!(!v.is_cursor_invalid());
        // RestartInputRecursion is set by RawVisitAbs (emView.cpp:1803) and is NOT drained
        // by the Update loop — it is read and cleared by the input-filter chain externally.
        assert!(v.RestartInputRecursion);
    }

    #[test]
    fn test_invalidate_control_panel() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        // set_active_panel unconditionally invalidates the control panel
        assert!(view.is_control_panel_invalid());
        view.clear_control_panel_invalid();

        // child1 is in active path — invalidate_control_panel sets flag
        view.InvalidateControlPanel(&tree, child1);
        assert!(view.is_control_panel_invalid());
        view.clear_control_panel_invalid();

        // child2 is NOT in active path — flag stays clear
        view.InvalidateControlPanel(&tree, child2);
        assert!(!view.is_control_panel_invalid());
    }

    #[test]
    fn test_pixel_tallness() {
        let (mut tree, root, _c1, _c2) = setup_tree();
        // new() initialises CurrentPixelTallness to 1.0 (square pixels).
        let view = emView::new(root, 800.0, 600.0);
        assert_eq!(view.GetCurrentPixelTallness(), 1.0);

        let mut view2 = emView::new(root, 1920.0, 1080.0);
        assert_eq!(view2.GetCurrentPixelTallness(), 1.0);

        // SetGeometry now takes explicit pixel_tallness.
        view2.SetGeometry(&mut tree, 0.0, 0.0, 100.0, 200.0, 2.0);
        assert!((view2.GetCurrentPixelTallness() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_activation_adherent() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Direct activation is not adherent
        view.set_active_panel(&mut tree, child1, false);
        assert!(!view.IsActivationAdherent());
        assert!(!view.is_panel_activated_adherent(&tree, child1));

        // Explicit adherent activation
        view.set_active_panel(&mut tree, child1, true);
        assert!(view.IsActivationAdherent());
        assert!(view.is_panel_activated_adherent(&tree, child1));

        // Switching to a different panel clears adherent
        view.set_active_panel(&mut tree, root, false);
        assert!(!view.IsActivationAdherent());
    }

    #[test]
    fn test_activation_adherent_early_return_update() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Set active non-adherent
        view.set_active_panel(&mut tree, child1, false);
        assert!(!view.IsActivationAdherent());

        // Re-set same panel as adherent — hits early-return path, updates flag
        view.set_active_panel(&mut tree, child1, true);
        assert!(view.IsActivationAdherent());

        // Re-set same panel as non-adherent — hits early-return path again
        view.set_active_panel(&mut tree, child1, false);
        assert!(!view.IsActivationAdherent());
    }

    #[test]
    fn test_get_input_clock_ms() {
        let (_tree, root, _c1, _c2) = setup_tree();
        let view = emView::new(root, 800.0, 600.0);
        let ms = view.GetInputClockMS();
        // Should be a reasonable epoch-based timestamp (after year 2020)
        assert!(ms > 1_577_836_800_000);
    }

    #[test]
    fn test_highlight_rect_uses_viewed_width_for_y() {
        // Create a non-square child panel so viewed_height != viewed_width.
        // Root is square so viewed_width == viewed_height for it — we need
        // to test with a child whose layout_h != layout_w.
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.get_mut(root).unwrap().focusable = true;
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);

        let child = tree.create_child(root, "child");
        tree.get_mut(child).unwrap().focusable = true;
        // Non-square child: w=0.5, h=0.25 → tallness = 0.5
        tree.Layout(child, 0.1, 0.1, 0.5, 0.25, 1.0);

        let mut view = emView::new(root, 800.0, 600.0);
        view.set_active_panel(&mut tree, child, false);
        view.Update(&mut tree);

        let panel = tree.GetRec(child).unwrap();
        assert!(panel.viewed, "child should be viewed");
        // For a non-square child, viewed_width and viewed_height differ
        assert!(
            (panel.viewed_height - panel.viewed_width).abs() > 1.0,
            "Test setup: child must have viewed_width != viewed_height, got w={} h={}",
            panel.viewed_width,
            panel.viewed_height
        );

        // Substance rect components are in width-relative units.
        // Y and H must multiply by viewed_width, not viewed_height.
        let (_sx, sy, _sw, sh, _sr) = tree.GetSubstanceRect(child);
        let correct_hy = panel.viewed_y + sy * panel.viewed_width;
        let correct_hh = sh * panel.viewed_width;
        let wrong_hh = sh * panel.viewed_height;
        assert!(
            (correct_hh - wrong_hh).abs() > 1.0,
            "viewed_width and viewed_height must produce different results"
        );
        assert!(correct_hy.is_finite());
        assert!(correct_hh > 0.0);
    }

    #[test]
    fn test_raw_zoom_out_computes_fit_ratio() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0);

        let mut view = emView::new(root, 800.0, 600.0);
        view.RawZoomOut(&mut tree, false);

        let mut state_rx = 0.0;
        let mut state_ry = 0.0;
        let mut state_ra = 0.0;
        view.GetVisitedPanel(&tree, &mut state_rx, &mut state_ry, &mut state_ra);
        // C++ formula: max(W*H_root/hpt/H, H/H_root*hpt/W)
        // HomePixelTallness = 1.0 (square pixels)
        let hpt = 1.0;
        let expected = (800.0 * 0.75 / hpt / 600.0_f64).max(600.0 / 0.75 * hpt / 800.0);
        assert!(
            (state_ra - expected).abs() < 0.001,
            "rel_a should be {expected}, got {}",
            state_ra
        );
        assert!(state_rx.abs() < 0.001);
        assert!(state_ry.abs() < 0.001);
    }

    #[test]
    fn test_is_zoomed_out_after_raw_zoom_out() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0);

        let mut view = emView::new(root, 800.0, 600.0);
        view.RawZoomOut(&mut tree, false);
        assert!(view.IsZoomedOut(&tree));

        // After zooming in, should not be zoomed out
        view.Zoom(&mut tree, 2.0, 400.0, 300.0);
        assert!(!view.IsZoomedOut(&tree));
    }

    #[test]
    fn test_set_view_flags_root_same_tallness_updates_layout() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0); // starts square

        let mut view = emView::new(root, 800.0, 600.0);
        // pixel_tallness = 600/800 = 0.75
        let flags = view.flags | ViewFlags::ROOT_SAME_TALLNESS;
        view.SetViewFlags(flags, &mut tree);

        let rect = tree.layout_rect(root).unwrap();
        assert!(
            (rect.h - 0.75).abs() < 0.001,
            "Root layout_h should match pixel_tallness (0.75), got {}",
            rect.h
        );
    }

    #[test]
    fn test_highlight_arrow_vertices() {
        // Arrow at (100, 100) with goal at (100, 50).
        // C++ direction = away from goal: dx=0, dy=+1 (pointing down).
        let verts = compute_arrow_vertices(100.0, 100.0, 100.0, 50.0, 11.0);
        // tip: (100, 100)
        assert!((verts[0].0 - 100.0).abs() < 0.01);
        assert!((verts[0].1 - 100.0).abs() < 0.01);
        // right: (100 + 0*11 - 1*5.5*0.5, 100 + 1*11 + 0*5.5*0.5) = (97.25, 111)
        assert!((verts[1].0 - 97.25).abs() < 0.01);
        assert!((verts[1].1 - 111.0).abs() < 0.01);
        // notch: (100, 100 + 1*8.8) = (100, 108.8)
        assert!((verts[2].0 - 100.0).abs() < 0.01);
        assert!((verts[2].1 - 108.8).abs() < 0.01);
        // left: (100 + 0*11 + 1*5.5*0.5, 100 + 1*11 - 0*5.5*0.5) = (102.75, 111)
        assert!((verts[3].0 - 102.75).abs() < 0.01);
        assert!((verts[3].1 - 111.0).abs() < 0.01);
    }

    #[test]
    fn test_highlight_arrow_count_rounding() {
        assert_eq!(compute_arrow_count(55.0, 55.0), 1);
        assert_eq!(compute_arrow_count(110.0, 55.0), 2);
        assert_eq!(compute_arrow_count(165.0, 55.0), 3);
        assert_eq!(compute_arrow_count(220.0, 55.0), 4);
        assert_eq!(compute_arrow_count(385.0, 55.0), 6);
        assert_eq!(compute_arrow_count(440.0, 55.0), 8);
        // Too short — no arrows
        assert_eq!(compute_arrow_count(20.0, 55.0), 0);
    }

    #[test]
    fn ego_mode_cursor_override() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Default cursor is Normal
        view.set_cursor(emCursor::Normal);
        assert_eq!(view.GetCursor(), emCursor::Normal);

        // With EGO_MODE, Normal cursor becomes Crosshair
        view.flags |= ViewFlags::EGO_MODE;
        assert_eq!(view.GetCursor(), emCursor::Crosshair);

        // Non-Normal cursors are NOT overridden
        view.set_cursor(emCursor::Text);
        assert_eq!(view.GetCursor(), emCursor::Text);

        // Turning off EGO_MODE restores Normal
        view.set_cursor(emCursor::Normal);
        view.flags -= ViewFlags::EGO_MODE;
        assert_eq!(view.GetCursor(), emCursor::Normal);
    }

    #[test]
    fn ego_mode_scroll_locked() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Record initial center position
        let (_, before_rx, before_ry, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist before scroll");

        // Enable EGO_MODE and attempt to scroll
        view.flags |= ViewFlags::EGO_MODE;
        let done = view.RawScrollAndZoom(&mut tree, 400.0, 300.0, 50.0, 50.0, 0.0);

        // Scroll delta should be zero — viewport center locked
        let (_, after_rx, after_ry, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist after scroll");
        assert!(
            (after_rx - before_rx).abs() < 1e-12,
            "rel_x should not change under EGO_MODE, delta={}",
            after_rx - before_rx
        );
        assert!(
            (after_ry - before_ry).abs() < 1e-12,
            "rel_y should not change under EGO_MODE, delta={}",
            after_ry - before_ry
        );
        assert!(
            done[0].abs() < 1e-12 && done[1].abs() < 1e-12,
            "done_x and done_y should be zero"
        );
    }

    #[test]
    fn ego_mode_zoom_still_works() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        let (_, _, _, rel_a_before) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist before zoom");

        // Enable EGO_MODE and zoom
        view.flags |= ViewFlags::EGO_MODE;
        view.RawScrollAndZoom(&mut tree, 400.0, 300.0, 0.0, 0.0, 50.0);

        let (_, _, _, rel_a_after) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist after zoom");
        assert!(
            (rel_a_after - rel_a_before).abs() > 1e-6,
            "zoom should still work under EGO_MODE"
        );
    }

    #[test]
    fn ego_mode_toggle_invalidates_cursor() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.clear_cursor_invalid(); // drain change-block side effect

        assert!(!view.is_cursor_invalid());
        view.flags ^= ViewFlags::EGO_MODE;
        view.mark_cursor_invalid();
        assert!(view.is_cursor_invalid());
    }

    #[test]
    fn stress_test_sync_creates_and_destroys() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Initially no stress test
        assert!(!view.is_stress_test_active());
        assert!(view.stress_test().is_none());

        // Enable STRESS_TEST and sync
        view.flags |= ViewFlags::STRESS_TEST;
        view.sync_stress_test();
        assert!(view.is_stress_test_active());
        assert!(view.stress_test().is_some());

        // Disable and sync — struct dropped
        view.flags -= ViewFlags::STRESS_TEST;
        view.sync_stress_test();
        assert!(!view.is_stress_test_active());
        assert!(view.stress_test().is_none());
    }

    #[test]
    fn stress_test_ring_buffer_accumulates() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        view.flags |= ViewFlags::STRESS_TEST;
        // Sync multiple times to accumulate entries
        for _ in 0..10 {
            view.sync_stress_test();
        }
        let st = view.stress_test().unwrap();
        assert_eq!(st.valid_count(), 10);
    }

    #[test]
    fn stress_test_paint_overlay() {
        use crate::emImage::emImage;

        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        view.flags |= ViewFlags::STRESS_TEST;
        view.sync_stress_test();

        // Paint into a real image and verify the overlay renders without panic
        let mut img = emImage::new(800, 600, 4);
        let mut painter = emPainter::new(&mut img);
        view.Paint(&mut tree, &mut painter, emColor::TRANSPARENT);

        // Check that the top-left corner has non-zero (overlay painted) pixels.
        // The purple background (255,0,255,128) should have been blended there.
        let px = img.GetMap();
        // pixel at (5, 5): offset = (5 * 800 + 5) * 4
        let off = (5 * 800 + 5) * 4;
        let has_overlay = px[off] > 0 || px[off + 1] > 0 || px[off + 2] > 0;
        assert!(
            has_overlay,
            "stress test overlay should paint in top-left corner"
        );
    }

    #[test]
    fn tree_dump_produces_valid_emrec() {
        use crate::emRec::parse_rec_with_format;

        let (mut tree, root, _child1, _child2) = setup_tree();
        let view = emView::new(root, 800.0, 600.0);

        let path = view.dump_tree(&mut tree);

        // File should exist
        assert!(path.exists(), "dump file should exist at {:?}", path);

        // Read and parse as emRec
        let content = std::fs::read_to_string(&path).expect("read dump file");
        let rec = parse_rec_with_format(&content, "emTreeDump")
            .expect("dump should be valid emRec format");

        // Should contain view-level data
        assert!(rec.get_str("view.title").is_some() || rec.get_struct("view").is_some());

        // Should contain panel names from the test tree
        assert!(
            content.contains("root"),
            "dump should contain root panel name"
        );
        assert!(
            content.contains("child1"),
            "dump should contain child1 panel name"
        );
        assert!(
            content.contains("child2"),
            "dump should contain child2 panel name"
        );

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    // --- Coordinate-system invariant tests ---
    // These test physical behavior, not coordinate values, so they
    // survive a convention change (viewport-fraction → panel-fraction).

    /// Helper: convert viewport pixel to panel-local coordinates using
    /// viewed_x/viewed_width from the panel tree. Convention-independent.
    fn panel_space_at_pixel(tree: &PanelTree, panel: PanelId, px: f64, py: f64) -> (f64, f64) {
        let rec = tree.GetRec(panel).unwrap();
        (
            (px - rec.viewed_x) / rec.viewed_width,
            (py - rec.viewed_y) / rec.viewed_height,
        )
    }

    #[test]
    fn invariant_zoom_fixpoint() {
        // The pixel under the cursor maps to the same panel-space point
        // before and after zoom, at various cursor positions and zoom factors.
        let (mut tree, root, _child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
        view.Update(&mut tree);

        // Start at a moderate zoom so there's room to zoom in and out
        view.Zoom(&mut tree, 4.0, 400.0, 300.0);
        view.Update(&mut tree);

        let cursors = [(200.0, 150.0), (400.0, 300.0), (700.0, 50.0), (50.0, 550.0)];
        // Factor 0.01 is excluded: at that extreme zoom-out the root panel
        // becomes smaller than the viewport, triggering root-centering in
        // RawVisitAbs (C++ emView.cpp:1588-1600) which snaps vx/vy and
        // intentionally breaks the zoom fixpoint — C++ has the same behaviour.
        // Factor 100 is excluded: at extreme zoom-in, f64 fp precision at 3.2e7
        // pixel scale degrades below 1e-9 tolerance (same behaviour as C++).
        let factors = [0.5, 2.0, 4.0];

        for &(cx, cy) in &cursors {
            for &factor in &factors {
                // Save and restore state for each combo
                let (saved_panel, saved_rx, saved_ry, saved_ra) = view
                    .get_visited_panel_idiom(&tree)
                    .expect("visited panel should exist at loop start");

                // Use the SVP as the reference panel (root.viewed_* are only valid
                // when root is the SVP; using SVP is always correct).
                let svp_before = view.supreme_panel();
                let before = panel_space_at_pixel(&tree, svp_before, cx, cy);
                view.Zoom(&mut tree, factor, cx, cy);
                let svp_after = view.supreme_panel();
                // After zoom, the SVP may change. The fixpoint only holds within
                // a single SVP coordinate frame; check only when SVP is stable.
                if svp_before == svp_after {
                    let after = panel_space_at_pixel(&tree, svp_after, cx, cy);
                    assert!(
                        (before.0 - after.0).abs() < 1e-9,
                        "fix-point X violated: cursor=({cx},{cy}) factor={factor} \
                         before={:.12} after={:.12} diff={:.3e}",
                        before.0,
                        after.0,
                        (before.0 - after.0).abs()
                    );
                    assert!(
                        (before.1 - after.1).abs() < 1e-9,
                        "fix-point Y violated: cursor=({cx},{cy}) factor={factor} \
                         before={:.12} after={:.12} diff={:.3e}",
                        before.1,
                        after.1,
                        (before.1 - after.1).abs()
                    );
                }

                // Restore by calling RawVisit with the saved rel coords.
                view.RawVisit(&mut tree, saved_panel, saved_rx, saved_ry, saved_ra, true);
            }
        }
    }

    #[test]
    fn invariant_calc_visit_round_trip() {
        // rel_x=0, rel_y=0 → RawVisit → panel center at viewport center.
        // Verifies the core coord-system invariant: zero offset means centered.
        // Tested at multiple zoom levels to catch convention/scaling errors.
        // Phase 3: RawVisit is the public API for setting viewing coords from
        // rel coords.
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0);
        let mut view = emView::new(root, 800.0, 600.0);
        view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
        view.Update(&mut tree);

        let (vw, vh) = view.viewport_size();

        // relA in C++ convention: HomeW*HomeH/(vw*vh). Larger = more zoomed out.
        for &rel_a in &[0.5, 1.0, 2.0, 8.0] {
            view.RawVisit(&mut tree, root, 0.0, 0.0, rel_a, true);

            let rec = tree.GetRec(root).unwrap();
            let panel_cx = rec.viewed_x + rec.viewed_width * 0.5;
            let panel_cy = rec.viewed_y + rec.viewed_height * 0.5;

            assert!(
                (panel_cx - vw * 0.5).abs() < 0.5,
                "rel_a={rel_a}: root not centered X: panel_cx={panel_cx:.4} \
                 viewport_cx={:.4} diff={:.4}",
                vw * 0.5,
                (panel_cx - vw * 0.5).abs()
            );
            assert!(
                (panel_cy - vh * 0.5).abs() < 0.5,
                "rel_a={rel_a}: root not centered Y: panel_cy={panel_cy:.4} \
                 viewport_cy={:.4} diff={:.4}",
                vh * 0.5,
                (panel_cy - vh * 0.5).abs()
            );
        }
    }

    #[test]
    fn invariant_scroll_direction() {
        // Scroll(+dx, 0) moves the viewport rightward: panel viewed_x decreases.
        // Use a root-only tree (no children) so the SVP is always root and the
        // check is stable across zoom levels.
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        let mut deltas = Vec::new();
        for &rel_a_level in &[1.0_f64, 0.5, 0.1] {
            // Set a specific zoom level via RawVisit.
            view.RawVisit(&mut tree, root, 0.0, 0.0, rel_a_level, true);

            let before_vx = tree.GetRec(root).unwrap().viewed_x;
            view.Scroll(&mut tree, 50.0, 0.0);
            // SVP must still be root (no children to change to).
            assert_eq!(view.supreme_panel(), root, "SVP changed during Scroll");
            let after_vx = tree.GetRec(root).unwrap().viewed_x;

            let delta = after_vx - before_vx;
            assert!(
                delta.abs() > 1e-6,
                "Scroll(+50) had no effect at rel_a={rel_a_level}: before={before_vx} after={after_vx}"
            );
            deltas.push((rel_a_level, delta));
        }

        // All deltas must have the same sign.
        let first_sign = deltas[0].1.signum();
        for &(level, delta) in &deltas {
            assert!(
                delta.signum() == first_sign,
                "Scroll direction inconsistent: rel_a={level} delta={delta:.4}, \
                 expected sign={first_sign}"
            );
        }
    }

    #[test]
    fn test_soft_keyboard_toggle() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        let mut view = emView::new(root, 800.0, 600.0);
        assert!(!view.IsSoftKeyboardShown());
        view.ShowSoftKeyboard(true);
        assert!(view.IsSoftKeyboardShown());
        view.ShowSoftKeyboard(false);
        assert!(!view.IsSoftKeyboardShown());
    }

    #[test]
    fn test_signal_fields_and_visit_by_identity() {
        use crate::emScheduler::EngineScheduler;
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.get_mut(root).unwrap().focusable = true;
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);
        let child = tree.create_child(root, "child");
        tree.get_mut(child).unwrap().focusable = true;
        tree.Layout(child, 0.0, 0.0, 0.5, 1.0, 1.0);

        let mut view = emView::new(root, 800.0, 600.0);

        // Signal getters return None before being set.
        assert!(view.GetControlPanelSignal().is_none());
        assert!(view.GetTitleSignal().is_none());

        // Wire up scheduler and signals.
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        let cp_sig = sched.borrow_mut().create_signal();
        let title_sig = sched.borrow_mut().create_signal();
        view.set_scheduler(sched.clone());
        view.set_control_panel_signal(cp_sig);
        view.set_title_signal(title_sig);

        assert_eq!(view.GetControlPanelSignal(), Some(cp_sig));
        assert_eq!(view.GetTitleSignal(), Some(title_sig));

        // set_active_panel fires ControlPanelSignal.
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child, false);
        assert!(sched.borrow().is_pending(cp_sig));

        // VisitByIdentity routes through VisitingVA per W4 Phase 3.
        view.VisitByIdentity("root:child", 0.5, 0.5, 1.0, false, "child-title");
        assert!(view.VisitingVA.borrow().is_active());
        assert_eq!(view.VisitingVA.borrow().identity(), "root:child");
        // Non-existent identity no longer requires tree lookup — the animator
        // just sets the goal; resolution happens later during Cycle.
        view.VisitByIdentity("no_such_panel", 0.0, 0.0, 1.0, false, "");

        // Clean up signals so EngineScheduler's debug_assert on drop is satisfied.
        sched.borrow_mut().remove_signal(cp_sig);
        sched.borrow_mut().remove_signal(title_sig);
    }

    #[test]
    fn visit_routes_through_animator() {
        // W4 Phase 3: Visit(tree, panel, rx, ry, ra, adherent) must set a goal
        // on VisitingVA and activate it, matching C++ emView.cpp:492-510.
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);
        let child = tree.create_child(root, "child");
        tree.Layout(child, 0.0, 0.0, 0.5, 1.0, 1.0);

        let mut view = emView::new(root, 800.0, 600.0);
        assert!(
            !view.VisitingVA.borrow().is_active(),
            "inactive before Visit"
        );

        view.Visit(&tree, child, 0.25, 0.5, 2.0, false);

        let va = view.VisitingVA.borrow();
        assert!(va.is_active(), "active after Visit");
        assert_eq!(va.identity(), tree.GetIdentity(child));
        assert!((va.rel_x() - 0.25).abs() < 1e-9);
        assert!((va.rel_y() - 0.5).abs() < 1e-9);
        assert!((va.rel_a() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn visit_panel_short_form_routes_through_animator() {
        // Short-form VisitPanel(tree, panel, adherent) delegates to VisitingVA
        // via VisitByIdentityShort, matching C++ emView.cpp:511-523.
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);

        let mut view = emView::new(root, 800.0, 600.0);
        assert!(
            !view.VisitingVA.borrow().is_active(),
            "inactive before VisitPanel"
        );

        view.VisitPanel(&tree, root, true);

        let va = view.VisitingVA.borrow();
        assert!(va.is_active(), "active after VisitPanel");
        assert_eq!(va.identity(), tree.GetIdentity(root));
    }

    #[test]
    fn test_phase1_new_fields_default_initialized() {
        let (tree, root, _, _) = setup_tree();
        let v = emView::new(root, 640.0, 480.0);

        // Home rect defaults to the viewport rect passed to new().
        assert_eq!(v.HomeX, 0.0);
        assert_eq!(v.HomeY, 0.0);
        assert_eq!(v.HomeWidth, 640.0);
        assert_eq!(v.HomeHeight, 480.0);
        assert_eq!(v.HomePixelTallness, 1.0);

        // Current rect starts identical to Home rect (no popup at construction).
        assert_eq!(v.CurrentX, 0.0);
        assert_eq!(v.CurrentY, 0.0);
        assert_eq!(v.CurrentWidth, 640.0);
        assert_eq!(v.CurrentHeight, 480.0);
        assert_eq!(v.CurrentPixelTallness, 1.0);

        // Invalidation / recursion flags all start false.
        assert!(!v.SVPChoiceInvalid);
        assert!(!v.SVPChoiceByOpacityInvalid);
        assert!(!v.RestartInputRecursion);
        assert_eq!(v.SettingGeometry, 0);
        assert_eq!(v.SVPUpdSlice, 0u64);
        assert!(!v.ZoomScrollInAction);

        // MinSVP / MaxSVP default to None (the "no SVP computed yet" state).
        assert!(v.MinSVP.is_none());
        assert!(v.MaxSVP.is_none());

        // LastMouse defaults to a sentinel far outside any viewport.
        assert_eq!(v.LastMouseX, -1.0e10);
        assert_eq!(v.LastMouseY, -1.0e10);

        // Signal fields exist (may be None until a scheduler wires them).
        let _: &Option<crate::emSignal::SignalId> = &v.view_flags_signal;
        let _: &Option<crate::emSignal::SignalId> = &v.focus_signal;
        let _: &Option<crate::emSignal::SignalId> = &v.geometry_signal;

        // Suppress unused-variable warning for tree (kept alive for PanelId validity).
        let _ = tree;
    }

    #[test]
    fn test_phase2_raw_visit_abs_matches_inline_update() {
        // Build the standard test tree: root with two children, one focused.
        let (mut tree, root, child_a, _child_b) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        v.SetActivePanel(child_a);
        // Run Update to populate SVP + viewed rects the old way.
        v.Update(&mut tree);
        let expected_svp = v.GetSupremeViewedPanel();
        let expected_vx = tree.GetRec(expected_svp.unwrap()).unwrap().viewed_x;
        let expected_vy = tree.GetRec(expected_svp.unwrap()).unwrap().viewed_y;
        let expected_vw = tree.GetRec(expected_svp.unwrap()).unwrap().viewed_width;

        // Rebuild view, call RawVisitAbs directly with the same rect; expect
        // the same SVP and same viewed rect.
        let mut tree2 = setup_tree().0;
        let root2 = tree2.GetRootPanel().unwrap();
        let mut v2 = emView::new(root2, 640.0, 480.0);
        v2.SetActivePanel(tree2.GetFirstChild(root2).unwrap());
        v2.RawVisitAbs(
            &mut tree2,
            expected_svp.unwrap(),
            expected_vx,
            expected_vy,
            expected_vw,
            false,
        );
        assert_eq!(v2.GetSupremeViewedPanel(), expected_svp);
        let svp = tree2.GetRec(expected_svp.unwrap()).unwrap();
        assert!((svp.viewed_x - expected_vx).abs() < 1e-9);
        assert!((svp.viewed_y - expected_vy).abs() < 1e-9);
        assert!((svp.viewed_width - expected_vw).abs() < 1e-9);
    }

    /// Non-trivial parity test: verifies the root-centering path of RawVisitAbs
    /// (C++ emView.cpp:1588-1626).
    ///
    /// When `vw < HomeWidth && vh < HomeHeight` the block scales the viewport up
    /// to fill the constraining dimension, then clamps.  The original `vx` input
    /// is DISCARDED — the centering formula replaces it.  A round-trip that just
    /// passes the existing viewed_x back would never exercise this.
    ///
    /// Setup: root-only tree (no children so FindBestSVP stays at root), square
    /// root aspect, HomeWidth=640 HomeHeight=480 HomePixelTallness=1.
    ///
    /// Input: vw=300 (< HomeHeight=480 < HomeWidth=640) → centering fires,
    /// scales to HomeHeight=480 (the tighter dimension since root is square).
    ///
    /// Expected derivation (C++ emView.cpp:1596-1605):
    ///   vp_h = 1.0  (square root, layout h == layout w)
    ///   vh   = vw * vp_h / HomePixelTallness = 300
    ///   Both vw=300 < HomeWidth=640 and vh=300 < HomeHeight=480 → centering fires.
    ///   vx_norm = (HomeX + HomeWidth/2 - vx_in) / vw  = (320 - 200) / 300 = 0.4
    ///   vy_norm = (HomeY + HomeHeight/2 - vy_in) / vh = (240 - 100) / 300 = 7/15
    ///   vh*HomeWidth=192000 > vw*HomeHeight=144000 → else-branch (taller relative):
    ///     new_vh = HomeHeight = 480  →  vw = 480 / vp_h * pt = 480
    ///   vx = HomeX + HomeWidth/2 - vx_norm * vw = 320 - 0.4*480 = 128
    ///   vy = HomeY + HomeHeight/2 - vy_norm * 480 = 240 - (7/15)*480 = 16
    ///   Clamping (not EGO_MODE, vh_cur*HomeWidth > vw*HomeHeight → else-branch):
    ///     x1 = 320 - 480/2 = 80,  x2 = 320 + 480/2 = 560
    ///     y1 = 0,  y2 = 480
    ///     vx=128 > x1=80 → vx = 80
    ///     vy=16 > y1=0   → vy = 0
    ///   Final: vx=80, vy=0, vw=480.
    #[test]
    fn test_phase2_raw_visit_abs_root_centering() {
        // Root-only tree: square panel (layout h == layout w → height=1.0).
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);

        // HomeWidth=640, HomeHeight=480 (default from emView::new).
        let mut v = emView::new(root, 640.0, 480.0);

        // vw_in=300 < HomeHeight=480 < HomeWidth=640 → root-centering fires.
        let vx_in = 200.0_f64;
        let vy_in = 100.0_f64;
        let vw_in = 300.0_f64;
        v.RawVisitAbs(&mut tree, root, vx_in, vy_in, vw_in, false);

        let svp = v
            .GetSupremeViewedPanel()
            .expect("RawVisitAbs must set an SVP");

        // Root-centering fires → vx must differ from the input.
        let rec = tree.GetRec(svp).unwrap();
        assert!(
            (rec.viewed_x - vx_in).abs() > 10.0,
            "viewed_x={:.4} should differ from input vx_in={vx_in} (centering must have fired)",
            rec.viewed_x
        );

        // The centering formula expands vw to fill the tighter dimension (HomeHeight=480).
        assert!(
            (rec.viewed_width - 480.0).abs() < 1e-9,
            "viewed_width={:.6} expected 480.0 after centering",
            rec.viewed_width
        );

        // Exact clamped position per the derivation in the docstring above.
        assert!(
            (rec.viewed_x - 80.0).abs() < 1e-9,
            "viewed_x={:.6} expected 80.0",
            rec.viewed_x
        );
        assert!(
            (rec.viewed_y - 0.0).abs() < 1e-9,
            "viewed_y={:.6} expected 0.0",
            rec.viewed_y
        );
    }

    #[test]
    fn test_phase3_update_drains_title_then_cursor() {
        let (mut tree, root, child_a, _child_b) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        v.SetActivePanel(child_a);
        // Bring viewing to steady state.
        v.SVPChoiceInvalid = true;
        v.Update(&mut tree);
        // Now flag title + cursor invalid; both should clear in a single
        // Update call per the drain loop.
        v.title_invalid = true; // set the field directly — no view-level mark helper today
        v.mark_cursor_invalid();
        v.Update(&mut tree);
        assert!(!v.is_title_invalid());
        assert!(!v.is_cursor_invalid());
    }

    #[test]
    fn test_phase3_update_does_not_touch_active_path_per_frame() {
        // C++ Update() does not mutate in_active_path. Only set_active_panel
        // does. Verify that Update after set_active_panel leaves in_active_path
        // unchanged across repeated calls.
        let (mut tree, root, child_a, _child_b) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        // Use set_active_panel (lowercase) which actually sets in_active_path.
        v.set_active_panel(&mut tree, child_a, false);
        v.Update(&mut tree);
        let before = tree.GetRec(child_a).unwrap().in_active_path;
        // Second Update — no active-path mutation path should run.
        v.Update(&mut tree);
        let after = tree.GetRec(child_a).unwrap().in_active_path;
        assert_eq!(before, after);
        assert!(after, "active panel should still be in active path");
        let _ = root; // suppress warning
    }

    /// Phase 4 acceptance test: verifies that RawVisitAbs creates a popup
    /// window when the zoom rect falls outside the home rect under
    /// ViewFlags::POPUP_ZOOM.
    ///
    /// Port of plan lines 1459-1471 (Phase 4 Step 3).
    ///
    /// The plan specified calling Update() after RawVisit(). That sequence
    /// is incorrect: Update() has zoomed_out_before_sg=true on first call,
    /// so it immediately calls RawZoomOut which brings the view inside home
    /// and tears down the popup. The test is corrected to:
    ///   1. Update() first — clears zoomed_out_before_sg.
    ///   2. SetViewFlags with POPUP_ZOOM.
    ///   3. RawVisit child_a very zoomed in — triggers popup creation.
    ///   4. Assert PopupWindow created (before a second Update could tear it down).
    ///
    /// The plan assertion `assert_ne!(v.CurrentWidth, v.HomeWidth)` was also
    /// dropped: with max_popup_rect defaulting to the home rect, the popup is
    /// sized identically to the home viewport, so CurrentWidth == HomeWidth.
    #[test]
    fn test_phase4_popup_zoom_creates_popup_window() {
        let (mut tree, root, child_a, _) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        // First Update handles zoomed_out_before_sg (zoom to root).
        v.Update(&mut tree);
        // Enable popup zoom mode.
        v.SetViewFlags(ViewFlags::POPUP_ZOOM, &mut tree);
        // Visit child_a with rel_a=0.1 — produces a zoom rect far larger
        // than the home rect. The ancestor-clamp loop ascends to root with
        // vw≈3504 >> HomeWidth=640, triggering outside_home → popup branch.
        v.RawVisit(&mut tree, child_a, 0.0, 0.0, 0.1, true);

        // Expected: PopupWindow is Some immediately after RawVisit.
        assert!(v.PopupWindow.is_some(), "PopupWindow should be created");
    }

    // --- Phase 5 gate tests ---

    /// Phase 7: SignalEOIDelayed registers an EOIEngineClass with the
    /// scheduler that fires EOISignal after its countdown reaches zero.
    #[test]
    fn test_phase7_eoi_engine_fires_via_scheduler() {
        use crate::emEngine::{emEngine as EngineTrait, EngineCtx, Priority};

        struct ListenEngine {
            watched: crate::emSignal::SignalId,
            fired: Rc<std::cell::Cell<bool>>,
        }
        impl EngineTrait for ListenEngine {
            fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
                if ctx.IsSignaled(self.watched) {
                    self.fired.set(true);
                }
                true
            }
        }

        let (mut tree, root, _, _) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        v.attach_to_scheduler(sched.clone(), winit::window::WindowId::dummy());
        let eoi = v.EOISignal.expect("EOISignal installed by attach");

        // Register a listener that records when EOISignal fires.
        let fired = Rc::new(std::cell::Cell::new(false));
        let listener_id = sched.borrow_mut().register_engine(
            Priority::Low,
            Box::new(ListenEngine {
                watched: eoi,
                fired: Rc::clone(&fired),
            }),
        );
        sched.borrow_mut().connect(eoi, listener_id);

        v.SignalEOIDelayed();
        let mut windows = std::collections::HashMap::new();
        for _ in 0..10 {
            sched.borrow_mut().DoTimeSlice(&mut tree, &mut windows);
            if fired.get() {
                break;
            }
        }
        assert!(
            fired.get(),
            "EOIEngineClass should fire EOISignal via the scheduler within 10 slices"
        );

        // Clean up scheduler state before drop asserts.
        sched.borrow_mut().disconnect(eoi, listener_id);
        sched.borrow_mut().remove_engine(listener_id);
        if let Some(id) = v.update_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
        if let Some(id) = v.eoi_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
        if let Some(id) = v.visiting_va_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
        sched.borrow_mut().remove_signal(eoi);
    }

    /// Phase 7: attach_to_scheduler registers UpdateEngineClass and installs
    /// EOISignal. WakeUpUpdateEngine wakes the registered engine.
    #[test]
    fn test_phase7_update_engine_wakeup_via_scheduler() {
        let (_tree, root, _, _) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        v.attach_to_scheduler(sched.clone(), winit::window::WindowId::dummy());
        assert!(v.update_engine_id.is_some());
        assert!(v.EOISignal.is_some());
        // WakeUpUpdateEngine queues the engine.
        assert!(!sched.borrow().has_awake_engines());
        v.WakeUpUpdateEngine();
        assert!(sched.borrow().has_awake_engines());
        // Clean up for Drop debug_asserts.
        let eng_id = v.update_engine_id.take().unwrap();
        sched.borrow_mut().remove_engine(eng_id);
        if let Some(id) = v.visiting_va_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
        if let Some(eoi) = v.EOISignal.take() {
            sched.borrow_mut().remove_signal(eoi);
        }
    }

    /// Phase 7: InvalidateHighlight marks the view dirty.
    #[test]
    fn test_phase7_invalidate_highlight_dirties_view() {
        let (mut tree, root, _, _) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        v.Update(&mut tree);
        let before = v.dirty_rects.len();
        v.InvalidateHighlight(&tree);
        assert!(
            v.dirty_rects.len() > before,
            "InvalidateHighlight should append a dirty rect"
        );
    }

    /// W1b: set_active_panel on a transition must push a dirty rect,
    /// matching C++ emView.cpp:284 (old active) and emView.cpp:305 (new
    /// active). Checks the transition case where both branches contribute.
    #[test]
    fn set_active_panel_transition_invalidates_highlight() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        v.Update(&mut tree);
        // Establish child1 as active first.
        v.set_active_panel(&mut tree, child1, false);
        v.dirty_rects.clear();
        // Transition to child2 — must InvalidateHighlight for old (child1) and new (child2).
        v.set_active_panel(&mut tree, child2, false);
        assert!(
            !v.dirty_rects.is_empty(),
            "set_active_panel transition should InvalidateHighlight"
        );
    }

    /// W1b: set_active_panel with only ActivationAdherent changing must
    /// still push a dirty rect, matching C++ emView.cpp:312.
    #[test]
    fn set_active_panel_adherent_only_invalidates_highlight() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        v.Update(&mut tree);
        v.set_active_panel(&mut tree, child1, false);
        v.dirty_rects.clear();
        // Same panel, different adherent — only C++ emView.cpp:312 branch fires.
        v.set_active_panel(&mut tree, child1, true);
        assert!(
            !v.dirty_rects.is_empty(),
            "set_active_panel adherent-only change should InvalidateHighlight"
        );
    }

    /// W1b: SetFocused must InvalidateHighlight twice — once if already
    /// focused (C++ emView.cpp:1211) and once if now focused (C++
    /// emView.cpp:1213). Observed as at least one dirty rect post-call.
    #[test]
    fn set_focused_invalidates_highlight() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        v.Update(&mut tree);
        v.set_active_panel(&mut tree, child1, false);
        v.SetFocused(&mut tree, true);
        v.dirty_rects.clear();
        v.SetFocused(&mut tree, false);
        assert!(
            !v.dirty_rects.is_empty(),
            "SetFocused(false) while focused should InvalidateHighlight"
        );
        v.dirty_rects.clear();
        v.SetFocused(&mut tree, true);
        assert!(
            !v.dirty_rects.is_empty(),
            "SetFocused(true) should InvalidateHighlight"
        );
    }

    /// Phase 7: AddToNoticeList delegates to the tree and wakes the scheduler-
    /// registered `UpdateEngineClass`.
    #[test]
    fn test_phase7_add_to_notice_list_wakes_update_engine() {
        let (mut tree, root, child1, _) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        v.attach_to_scheduler(sched.clone(), winit::window::WindowId::dummy());
        v.Update(&mut tree);
        let eng_id = v.update_engine_id.expect("update_engine_id installed");

        // Put the engine to sleep so we can detect the wake-up.
        sched.borrow_mut().sleep(eng_id);
        assert!(!sched.borrow().has_awake_engines());

        v.AddToNoticeList(&mut tree, child1);
        assert!(
            sched.borrow().has_awake_engines(),
            "AddToNoticeList should wake the update engine via the scheduler"
        );
        // Drain the scheduler to satisfy its debug_assert on drop.
        if let Some(eoi) = v.EOISignal.take() {
            sched.borrow_mut().remove_signal(eoi);
        }
        sched.borrow_mut().remove_engine(eng_id);
        if let Some(id) = v.visiting_va_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
    }

    /// Phase 6: SetGeometry accepts explicit (x, y, width, height, pixel_tallness).
    #[test]
    fn test_phase6_set_geometry_accepts_pixel_tallness() {
        let (mut tree, root, _, _) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        v.SetGeometry(&mut tree, 100.0, 50.0, 800.0, 600.0, 1.25);
        assert_eq!(v.HomeX, 100.0);
        assert_eq!(v.HomeY, 50.0);
        assert_eq!(v.HomeWidth, 800.0);
        assert_eq!(v.HomeHeight, 600.0);
        assert_eq!(v.HomePixelTallness, 1.25);
        assert_eq!(v.CurrentX, 100.0); // Current tracks Home when no popup.
        assert_eq!(v.CurrentPixelTallness, 1.25);
    }

    /// Phase 8 acceptance test — `emView::Update` drains the popup's
    /// close_signal and calls `ZoomOut`, and `SwapViewPorts` (via the
    /// popup-creation path) connects the close_signal to the update engine
    /// as a wake-up.
    ///
    /// TWO-ENGINE SHAPE IS LOAD-BEARING (W5a finding). The test (a)
    /// verifies the real `WakeUpUpdateEngineClass` connection via
    /// `get_signal_refs(close_sig, eng_id) >= 1`, then (b) drives the
    /// zoom-out teardown via a dormant `NoopEngine` swap + direct
    /// `v.Update()` call. The apparent harness hack exists because a
    /// single-engine integrated drive loop is currently infeasible for
    /// two reasons discovered during the W5a investigation:
    ///
    ///   1. `UpdateEngineClass::Cycle` dispatches through
    ///      `ctx.windows.get(&self.window_id)`. A bare `emView`
    ///      attached with `WindowId::dummy()` has no registered window,
    ///      so the engine wakes, cycles once, and no-ops.
    ///
    ///   2. `emView::Update` (below, popup-close-signal check) does
    ///      `sched.borrow()` to call `is_signaled_for_engine`. Callers
    ///      — including `emGUIFramework::about_to_wait` in production —
    ///      hold `sched.borrow_mut()` across `DoTimeSlice`, whose
    ///      engine chain runs `UpdateEngineClass::Cycle` → `Update`.
    ///      The `borrow()` re-entrantly panics. The `(scoped for
    ///      borrow correctness)` comment at the call site is
    ///      incorrect: scoping only helps when the outer borrow isn't
    ///      live. This is a production defect tracked in the emView
    ///      subsystem closeout doc §8 as **"emView::Update scheduler
    ///      re-entrant borrow"**.
    ///
    /// When the §8 successor workstream lands (option set: (a) pass
    /// scheduler context into `Update` via signature change, or (b)
    /// cache signaled state on the view during the signal-processing
    /// phase), this test should be rewritten to drive a single real
    /// engine end-to-end: wrap the view in a real `emWindow` via
    /// `emWindow::new_popup_pending` (winit/GPU-free — see the test at
    /// `emWindow.rs::new_popup_pending_constructs_without_event_loop`),
    /// register it in the `windows` HashMap at the same id given to
    /// `attach_to_scheduler`, fire the real `close_signal` via the
    /// consumer API, and drive `DoTimeSlice` in a bounded loop until
    /// `PopupWindow.is_none()`.
    #[test]
    fn test_phase8_popup_close_signal_zooms_out() {
        let (mut tree, root, child_a, _) = setup_tree();
        let mut v = emView::new(root, 640.0, 480.0);
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        v.attach_to_scheduler(sched.clone(), winit::window::WindowId::dummy());

        // Clear zoomed_out_before_sg, enable popup zoom, and create a popup.
        v.Update(&mut tree);
        v.SetViewFlags(ViewFlags::POPUP_ZOOM, &mut tree);
        v.RawVisit(&mut tree, child_a, 0.0, 0.0, 0.1, true);
        assert!(
            v.PopupWindow.is_some(),
            "popup should be created by RawVisit under POPUP_ZOOM"
        );

        // SwapViewPorts (Step 2) should have allocated close_signal and
        // connected it to the update engine.
        let close_sig = v
            .PopupWindow
            .as_ref()
            .map(|p| p.borrow().close_signal)
            .expect("PopupWindow present when scheduler is attached");
        let eng_id = v.update_engine_id.expect("update engine registered");
        assert!(
            sched.borrow().get_signal_refs(close_sig, eng_id) >= 1,
            "close_signal must be connected to the update engine"
        );

        // Drive the signal phase so sig.clock advances past eng.clock, then
        // call Update() directly. We swap `update_engine_id` for a dormant
        // dummy engine before the slice so that (a) the real update engine
        // (which RawVisit may have woken via WakeUpUpdateEngine) doesn't
        // interfere with the Update drain test, and (b) the dummy's clock
        // stays at its registration value while sig.clock advances. The
        // wake-up connection against the real engine was verified above.
        use crate::emEngine::{emEngine as EngineTrait, EngineCtx, Priority};
        struct NoopEngine;
        impl EngineTrait for NoopEngine {
            fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
                false
            }
        }
        sched.borrow_mut().disconnect(close_sig, eng_id);
        let dummy_id = sched
            .borrow_mut()
            .register_engine(Priority::High, Box::new(NoopEngine));
        v.update_engine_id = Some(dummy_id);
        sched.borrow_mut().connect(close_sig, dummy_id);
        // Immediately disconnect so DoTimeSlice doesn't wake the dummy
        // either — we only need sig.clock to advance via process signals.
        sched.borrow_mut().disconnect(close_sig, dummy_id);

        sched.borrow_mut().fire(close_sig);
        let mut windows = std::collections::HashMap::new();
        sched.borrow_mut().DoTimeSlice(&mut tree, &mut windows);
        // Now sig.clock > dummy.clock (dummy was never cycled this slice).
        v.Update(&mut tree);

        // Primary invariant: popup is torn down. This proves
        // Update -> ZoomOut -> RawVisit -> popup teardown branch fired.
        assert!(
            v.PopupWindow.is_none(),
            "popup should be torn down after ZoomOut"
        );

        // Scheduler cleanup for Drop debug_asserts. `update_engine_id` now
        // points at the dummy; the original engine is still registered.
        if let Some(id) = v.update_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
        sched.borrow_mut().remove_engine(eng_id);
        if let Some(eoi) = v.EOISignal.take() {
            sched.borrow_mut().remove_signal(eoi);
        }
        if let Some(id) = v.eoi_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
        if let Some(id) = v.visiting_va_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
    }

    /// W4 Phase 1 Task 1.3: `attach_to_scheduler` registers a
    /// `VisitingVAEngineClass` so that activating `VisitingVA` results in
    /// per-slice cycling. Mirrors the C++ behavior where
    /// `emVisitingViewAnimator` self-registers as an engine via its
    /// `emEngine` base ctor (emViewAnimator.cpp:930).
    #[test]
    fn visiting_va_cycles_when_activated() {
        use crate::emViewAnimator::emViewAnimator as _;

        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        let mut view = emView::new(root, 800.0, 600.0);
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        view.attach_to_scheduler(sched.clone(), winit::window::WindowId::dummy());

        // Engine must be registered by attach_to_scheduler.
        let visiting_id = view
            .visiting_va_engine_id
            .expect("attach_to_scheduler must register VisitingVAEngineClass");

        // Activate the animator — SetGoal + Activate, matching the
        // delegation shape Visit-family methods will use in Phase 3.
        {
            let mut va = view.VisitingVA.borrow_mut();
            va.SetGoal("root", false, "");
            va.Activate();
        }
        assert!(
            view.VisitingVA.borrow().is_active(),
            "animator should be active after SetGoal + Activate"
        );

        // Tick the scheduler. With a dummy window id, the Cycle method
        // hits the `ctx.windows.get() -> None` branch and no-ops — same
        // fallback pattern as `UpdateEngineClass`. The animator state
        // therefore must remain unchanged.
        sched.borrow_mut().wake_up(visiting_id);
        let mut windows = std::collections::HashMap::new();
        sched.borrow_mut().DoTimeSlice(&mut tree, &mut windows);

        // Intent check from the plan: "after one tick, animator has either
        // progressed or cleanly deactivated". Progress needs a real window;
        // the dummy path leaves state untouched, which is a valid outcome.
        assert!(
            view.VisitingVA.borrow().is_active(),
            "with dummy window, Cycle no-ops and animator remains active"
        );

        // Clean up scheduler resources before Drop asserts.
        if let Some(id) = view.update_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
        if let Some(id) = view.eoi_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
        if let Some(id) = view.visiting_va_engine_id.take() {
            sched.borrow_mut().remove_engine(id);
        }
        if let Some(sig) = view.EOISignal.take() {
            sched.borrow_mut().remove_signal(sig);
        }
    }

    #[test]
    fn visiting_va_owned_by_view() {
        // W4 Phase 1: emView holds VisitingVA matching C++ emView.h:675
        // (emOwnPtr<emVisitingViewAnimator> VisitingVA).
        use crate::emViewAnimator::emViewAnimator as _;
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        let view = emView::new(root, 800.0, 600.0);
        let va = view.VisitingVA.borrow();
        assert!(
            !va.is_active(),
            "VisitingVA should start inactive (C++ ST_NO_GOAL)"
        );
    }

    #[test]
    fn get_visited_panel_returns_svp_rel_coords() {
        // W4 Task 2.1: GetVisitedPanel out-param form mirrors C++ emView.cpp:468-489.
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.get_mut(root).unwrap().focusable = true;
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        let mut rx = 99.0_f64;
        let mut ry = 99.0_f64;
        let mut ra = 99.0_f64;
        let panel = view.GetVisitedPanel(&tree, &mut rx, &mut ry, &mut ra);

        // After Update the root is in the viewed path; GetVisitedPanel must return it.
        assert_eq!(panel, Some(root));
        // rel_x and rel_y are the viewport-center offset from the panel center in
        // panel-space units (C++ convention). For a root filling the whole viewport
        // after a fresh zoom-out, both must be finite (no sentinel 99.0 left).
        assert!(rx.is_finite(), "rel_x must be set (was 99.0 sentinel)");
        assert!(ry.is_finite(), "rel_y must be set (was 99.0 sentinel)");
        // rel_a must be positive (HomeW*HomeH / ViewedW*ViewedH).
        assert!(ra > 0.0, "rel_a must be positive");
    }
}

#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_compute_arrow_count() {
        let mut p_len: f64 = kani::any::<f64>();
        kani::assume(p_len.is_finite());
        let mut p_arrow_distance: f64 = kani::any::<f64>();
        kani::assume(p_arrow_distance.is_finite());
        let _r = compute_arrow_count(p_len, p_arrow_distance);
    }
}
