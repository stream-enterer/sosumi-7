// Port of C++ emMainWindow.
//
// DIVERGED: C++ emMainWindow creates an OS window + emMainPanel + detached
// control window + StartupEngine.  Rust creates a single ZuiWindow with
// emMainPanel as the root panel.  StartupEngine drives staged panel creation,
// autoplay input is wired via emAutoplayViewModel, and the window is persisted
// across frames via thread_local (set_main_window / with_main_window).

use std::cell::RefCell;
use std::rc::Rc;

use winit::event_loop::ActiveEventLoop;

use emcore::emContext::emContext;
use emcore::emEngine::{emEngine, EngineCtx, EngineId, Priority};
use emcore::emGUIFramework::App;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emPanelTree::PanelId;
use emcore::emSignal::SignalId;
use emcore::emWindow::{WindowFlags, ZuiWindow};

use crate::emMainControlPanel::emMainControlPanel;
use crate::emMainPanel::emMainPanel;

/// Configuration for creating an emMainWindow.
pub struct emMainWindowConfig {
    pub geometry: Option<String>, // "WxH+X+Y"
    pub fullscreen: bool,
    pub visit: Option<String>,
    pub control_tallness: f64,
}

impl Default for emMainWindowConfig {
    fn default() -> Self {
        Self {
            geometry: None,
            fullscreen: false,
            visit: None,
            control_tallness: 5.0,
        }
    }
}

/// Port of C++ `emMainWindow` (emMainWindow.cpp:28-84).
///
/// Holds window state: panel IDs, startup engine, visit parameters, and close
/// handling.
pub struct emMainWindow {
    pub(crate) window_id: Option<winit::window::WindowId>,
    pub(crate) _ctx: Rc<emContext>,
    pub(crate) main_panel_id: Option<PanelId>,
    pub(crate) _control_panel_id: Option<PanelId>,
    pub(crate) _content_panel_id: Option<PanelId>,
    pub(crate) startup_engine_id: Option<EngineId>,
    pub to_close: bool,
    pub(crate) _close_signal: Option<SignalId>,
    pub(crate) _visit_identity: Option<String>,
    pub(crate) _visit_rel_x: f64,
    pub(crate) _visit_rel_y: f64,
    pub(crate) _visit_rel_a: f64,
    pub(crate) _visit_adherent: bool,
    pub(crate) _visit_subject: String,
    pub(crate) _visit_valid: bool,
    pub(crate) config: emMainWindowConfig,
    pub(crate) autoplay_view_model: Option<crate::emAutoplay::emAutoplayViewModel>,
}

impl emMainWindow {
    pub(crate) fn new(ctx: Rc<emContext>, config: emMainWindowConfig) -> Self {
        Self {
            window_id: None,
            _ctx: ctx,
            main_panel_id: None,
            _control_panel_id: None,
            _content_panel_id: None,
            startup_engine_id: None,
            to_close: false,
            _close_signal: None,
            _visit_identity: None,
            _visit_rel_x: 0.0,
            _visit_rel_y: 0.0,
            _visit_rel_a: 0.0,
            _visit_adherent: false,
            _visit_subject: String::new(),
            _visit_valid: false,
            config,
            autoplay_view_model: None,
        }
    }

    /// Port of C++ `emMainWindow::ToggleFullscreen`.
    pub fn ToggleFullscreen(&self, app: &mut App) {
        if let Some(win) = self.window_id.and_then(|id| app.windows.get_mut(&id)) {
            let new_flags = win.flags ^ WindowFlags::FULLSCREEN;
            win.SetWindowFlags(new_flags);
        }
    }

    /// Port of C++ `emMainWindow::ReloadFiles`.
    pub fn ReloadFiles(&self) {
        log::info!("emMainWindow::ReloadFiles");
    }

    /// Port of C++ `emMainWindow::ToggleControlView` (emMainWindow.cpp:144-158).
    ///
    /// DIVERGED: ToggleControlView — C++ toggles focus between control view and
    /// content view (two separate emView instances inside emMainPanel).  Rust uses
    /// a single ZuiWindow with a slider; toggling the control view is implemented
    /// by calling `DoubleClickSlider()` which opens/closes the slider, producing
    /// the same user-visible effect.
    pub fn ToggleControlView(&mut self, app: &mut App) {
        if let Some(main_id) = self.main_panel_id {
            app.tree
                .with_behavior_as::<emMainPanel, _>(main_id, |mp| {
                    mp.DoubleClickSlider();
                });
        }
    }

    /// Port of C++ `emMainWindow::Close`.
    pub fn Close(&mut self) {
        self.to_close = true;
    }

    /// Port of C++ `emMainWindow::Quit`.
    pub fn Quit(&self, app: &App) {
        app.scheduler.borrow_mut().InitiateTermination();
    }

    /// Port of C++ `emMainWindow::GetTitle` (emMainWindow.cpp:87-95).
    ///
    /// C++ returns "Eagle Mode - <content view title>" when MainPanel exists
    /// and startup is complete, otherwise just "Eagle Mode".
    pub fn GetTitle(&self) -> String {
        if self.main_panel_id.is_some() && self.startup_engine_id.is_none() {
            // DIVERGED: GetTitle — C++ reads MainPanel->GetContentView().GetTitle()
            // which returns the visited panel's title.  Rust doesn't have the
            // dual-view architecture, so we return the static title.  A future
            // enhancement can read the content panel's title from the tree.
            "Eagle Mode".to_string()
        } else {
            "Eagle Mode".to_string()
        }
    }

    /// Port of C++ `emMainWindow::Duplicate` (emMainWindow.cpp:98-129).
    ///
    /// DIVERGED: Duplicate — C++ creates a new OS window visiting the same
    /// content panel location.  Rust uses a single ZuiWindow architecture and
    /// does not support multi-window.  This is a no-op with a log message.
    pub fn Duplicate(&self) {
        log::info!("emMainWindow::Duplicate — multi-window not supported in Rust port");
    }

    /// Port of C++ `emMainWindow::Input` (emMainWindow.cpp:193-263).
    ///
    /// DIVERGED: C++ Input uses emInputEvent, Rust uses the same struct but
    /// reads modifier state from emInputState (matching C++ behavior of
    /// checking the global input state rather than per-event modifiers).
    pub fn handle_input(
        &mut self,
        event: &emInputEvent,
        input_state: &emInputState,
        app: &mut App,
    ) -> bool {
        // C++ eats all input during startup (emMainWindow.cpp:197-201).
        if self.startup_engine_id.is_some() {
            return true;
        }

        let handled = match event.key {
            // F4 no modifier: Duplicate window (C++ emMainWindow.cpp:205-208)
            InputKey::F4
                if !input_state.GetShift()
                    && !input_state.GetCtrl()
                    && !input_state.GetAlt() =>
            {
                self.Duplicate();
                true
            }
            // Alt+F4: Close (C++ emMainWindow.cpp:209-212)
            InputKey::F4
                if !input_state.GetShift()
                    && !input_state.GetCtrl()
                    && input_state.GetAlt() =>
            {
                self.Close();
                true
            }
            // Shift+Alt+F4: Quit (C++ emMainWindow.cpp:213-216)
            InputKey::F4
                if input_state.GetShift()
                    && !input_state.GetCtrl()
                    && input_state.GetAlt() =>
            {
                self.Quit(app);
                true
            }
            // F5 no modifier: Reload (C++ emMainWindow.cpp:219-222)
            InputKey::F5
                if !input_state.GetShift()
                    && !input_state.GetCtrl()
                    && !input_state.GetAlt() =>
            {
                self.ReloadFiles();
                true
            }
            // F11 no modifier: Toggle fullscreen (C++ emMainWindow.cpp:225-228)
            InputKey::F11
                if !input_state.GetShift()
                    && !input_state.GetCtrl()
                    && !input_state.GetAlt() =>
            {
                self.ToggleFullscreen(app);
                true
            }
            // Escape no modifier: Toggle control view (C++ emMainWindow.cpp:230-237)
            InputKey::Escape
                if !input_state.GetShift()
                    && !input_state.GetCtrl()
                    && !input_state.GetAlt() =>
            {
                self.ToggleControlView(app);
                true
            }
            _ => false,
        };

        if handled {
            return true;
        }

        // Delegate to autoplay view model (handles F12 toggle).
        if let Some(ref mut avm) = self.autoplay_view_model
            && avm.Input(event, input_state)
        {
            return true;
        }

        // DIVERGED: Bookmark hotkeys — C++ searches BookmarksModel for matching
        // hotkeys and visits the bookmark location (emMainWindow.cpp:247-260).
        // Rust does not yet have BookmarksModel integration; bookmark hotkeys
        // are not handled here.

        false
    }
}

thread_local! {
    static MAIN_WINDOW: RefCell<Option<emMainWindow>> = const { RefCell::new(None) };
}

/// Store the main window for frame-loop access.
pub fn set_main_window(mw: emMainWindow) {
    MAIN_WINDOW.with(|cell| {
        *cell.borrow_mut() = Some(mw);
    });
}

/// Access the main window from the frame loop.
pub fn with_main_window<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut emMainWindow) -> R,
{
    MAIN_WINDOW.with(|cell| {
        cell.borrow_mut().as_mut().map(f)
    })
}

/// Startup engine registered with the scheduler.
///
/// Port of C++ `emMainWindow::StartupEngineClass` (emMainWindow.cpp:362-485).
/// States 0-6 drive panel creation; states 7-11 drive the startup zoom
/// animation.  Directly manipulates the panel tree and windows via `EngineCtx`,
/// matching the C++ design where the engine holds references and acts directly.
pub(crate) struct StartupEngine {
    state: u8,
    main_panel_id: PanelId,
    window_id: winit::window::WindowId,
    visit_valid: bool,
    visit_identity: String,
    visit_rel_x: f64,
    visit_rel_y: f64,
    visit_rel_a: f64,
    visit_adherent: bool,
    visit_subject: String,
    clock: std::time::Instant,
}

impl StartupEngine {
    pub(crate) fn new(
        main_panel_id: PanelId,
        window_id: winit::window::WindowId,
        visit: Option<String>,
    ) -> Self {
        // DIVERGED: C++ parses visit string into identity/relX/relY/relA fields
        // at construction time (emMainWindow.cpp:338-361).  Rust stores the raw
        // visit string as identity; bookmark-based visit is filled in at state 4
        // (Task 4).
        let (visit_valid, visit_identity) = match visit {
            Some(v) if !v.is_empty() => (true, v),
            _ => (false, String::new()),
        };
        Self {
            state: 0,
            main_panel_id,
            window_id,
            visit_valid,
            visit_identity,
            visit_rel_x: 0.0,
            visit_rel_y: 0.0,
            visit_rel_a: 0.0,
            visit_adherent: false,
            visit_subject: String::new(),
            clock: std::time::Instant::now(),
        }
    }
}

impl emEngine for StartupEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        match self.state {
            // States 0-2: idle wake-ups (C++ emMainWindow.cpp:367-375).
            0..=2 => {
                self.state += 1;
                true
            }
            // State 3: Set startup overlay (C++ emMainWindow.cpp:376-390).
            // MainPanel is already created before the engine starts.
            3 => {
                ctx.tree
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.SetStartupOverlay(true);
                    });
                self.state += 1;
                true
            }
            // State 4: Bookmark search placeholder (C++ emMainWindow.cpp:391-406).
            // Task 4 fills this in with BookmarksModel integration.
            4 => {
                self.state += 1;
                !ctx.IsTimeSliceAtEnd()
            }
            // State 5: Create control panel (C++ emMainWindow.cpp:407-415).
            5 => {
                ctx.tree
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.advance_creation_stage();
                    });
                self.state += 1;
                !ctx.IsTimeSliceAtEnd()
            }
            // State 6: Create content panel (C++ emMainWindow.cpp:416-422).
            6 => {
                ctx.tree
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.advance_creation_stage();
                    });
                self.state += 1;
                !ctx.IsTimeSliceAtEnd()
            }
            // State 7: Create visiting animator, zoom to ":" fullsized
            // (C++ emMainWindow.cpp:423-432).
            7 => {
                if let Some(win) = ctx.windows.get_mut(&self.window_id) {
                    use emcore::emViewAnimator::emVisitingViewAnimator;
                    let mut animator =
                        emVisitingViewAnimator::new(0.0, 0.0, 0.0, 1.0);
                    animator.SetAnimated(false);
                    animator.SetGoalFullsized(":", false, false, "");
                    win.active_animator = Some(Box::new(animator));
                }
                self.clock = std::time::Instant::now();
                self.state += 1;
                !ctx.IsTimeSliceAtEnd()
            }
            // State 8: Wait up to 2s or until animator inactive
            // (C++ emMainWindow.cpp:433-438).
            8 => {
                let still_active = ctx
                    .windows
                    .get(&self.window_id)
                    .and_then(|w| w.active_animator.as_ref())
                    .map(|a| a.is_active())
                    .unwrap_or(false);
                if self.clock.elapsed().as_millis() < 2000 && still_active {
                    return true;
                }
                self.state += 1;
                true
            }
            // State 9: Stop current animator; set visit goal if valid
            // (C++ emMainWindow.cpp:439-454).
            9 => {
                if let Some(win) = ctx.windows.get_mut(&self.window_id) {
                    if let Some(ref mut anim) = win.active_animator {
                        anim.stop();
                    }
                    if self.visit_valid {
                        use emcore::emViewAnimator::emVisitingViewAnimator;
                        let mut animator =
                            emVisitingViewAnimator::new(0.0, 0.0, 0.0, 1.0);
                        animator.set_goal_rel(
                            &self.visit_identity,
                            self.visit_rel_x,
                            self.visit_rel_y,
                            self.visit_rel_a,
                            self.visit_adherent,
                            &self.visit_subject,
                        );
                        win.active_animator = Some(Box::new(animator));
                    }
                }
                self.clock = std::time::Instant::now();
                self.state += 1;
                !ctx.IsTimeSliceAtEnd()
            }
            // State 10: Wait up to 2s, then clean up overlay and animator
            // (C++ emMainWindow.cpp:455-465).
            10 => {
                let still_active = ctx
                    .windows
                    .get(&self.window_id)
                    .and_then(|w| w.active_animator.as_ref())
                    .map(|a| a.is_active())
                    .unwrap_or(false);
                if self.clock.elapsed().as_millis() < 2000 && still_active {
                    return true;
                }
                // Clean up animator and zoom out.
                if let Some(win) = ctx.windows.get_mut(&self.window_id) {
                    win.active_animator = None;
                    win.view_mut().RawZoomOut(ctx.tree);
                }
                ctx.tree
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.SetStartupOverlay(false);
                    });
                self.clock = std::time::Instant::now();
                self.state += 1;
                true
            }
            // State 11 (default): 100ms pause, final visit, engine stops
            // (C++ emMainWindow.cpp:466-484).
            _ => {
                if self.clock.elapsed().as_millis() < 100 {
                    return true;
                }
                // DIVERGED: C++ calls ContentView.Visit() with identity-based
                // navigation and sets the active panel.  Rust's emView::Visit()
                // takes a PanelId, not an identity string.  The visiting animator
                // already navigated to the goal; skip the redundant final visit
                // until identity-based Visit is ported.
                false // engine stops permanently
            }
        }
    }
}

/// Create an emMainWindow: inserts the root emMainPanel into the panel tree,
/// allocates signals, creates the ZuiWindow, and registers a StartupEngine.
///
/// Called from the setup callback inside the `App` event loop.
pub fn create_main_window(
    app: &mut App,
    event_loop: &ActiveEventLoop,
    config: emMainWindowConfig,
) -> emMainWindow {
    let mut mw = emMainWindow::new(Rc::clone(&app.context), config);

    // Create root panel in the tree
    let panel = emMainPanel::new(Rc::clone(&app.context), mw.config.control_tallness);
    let root_id = app.tree.create_root("root");
    app.tree.set_behavior(root_id, Box::new(panel));
    mw.main_panel_id = Some(root_id);

    // Determine flags
    let mut flags = WindowFlags::AUTO_DELETE;
    if mw.config.fullscreen {
        flags |= WindowFlags::FULLSCREEN;
    }

    let close_signal = app.scheduler.borrow_mut().create_signal();
    let flags_signal = app.scheduler.borrow_mut().create_signal();
    mw._close_signal = Some(close_signal);

    // Create the window
    let window = ZuiWindow::create(
        event_loop,
        app.gpu(),
        root_id,
        flags,
        close_signal,
        flags_signal,
    );
    let window_id = window.winit_window.id();
    app.windows.insert(window_id, window);
    mw.window_id = Some(window_id);

    // Register StartupEngine with the scheduler.
    let startup_engine = StartupEngine::new(root_id, window_id, mw.config.visit.clone());
    let engine_id = app
        .scheduler
        .borrow_mut()
        .register_engine(Priority::Low, Box::new(startup_engine));
    app.scheduler.borrow_mut().wake_up(engine_id);
    mw.startup_engine_id = Some(engine_id);

    mw.autoplay_view_model = Some(crate::emAutoplay::emAutoplayViewModel::new());

    mw
}

/// Create a detached control window.
///
/// Port of C++ `emMainWindow::CreateControlWindow` (emMainWindow.cpp:309-327).
/// Creates a second OS window with `WF_AUTO_DELETE`, hosting an
/// `emMainControlPanel`.
///
/// Triggered by the `"ccw"` cheat code in `DoCustomCheat`.
///
/// Note: Full wiring (raise existing window, link to content view) requires
/// Phase 3's startup engine integration. This establishes the API shape.
pub fn create_control_window(
    app: &mut App,
    event_loop: &ActiveEventLoop,
) -> Option<winit::window::WindowId> {
    let ctrl_panel = emMainControlPanel::new(Rc::clone(&app.context));
    let root_id = app.tree.create_root("ctrl_window_root");
    app.tree.set_behavior(root_id, Box::new(ctrl_panel));

    let flags = WindowFlags::AUTO_DELETE;
    let close_signal = app.scheduler.borrow_mut().create_signal();
    let flags_signal = app.scheduler.borrow_mut().create_signal();

    let window = ZuiWindow::create(
        event_loop,
        app.gpu(),
        root_id,
        flags,
        close_signal,
        flags_signal,
    );
    let window_id = window.winit_window.id();
    app.windows.insert(window_id, window);
    Some(window_id)
}

/// Handle a custom cheat code.
///
/// Port of C++ `emMainWindow::DoCustomCheat` (emMainWindow.cpp:266-277).
///
/// Recognized cheats:
/// - `"rcp"`: Recreate content panels (see `RecreateContentPanels`).
/// - `"ccw"`: Create a detached control window.
pub fn do_custom_cheat(cheat: &str, app: &mut App, event_loop: &ActiveEventLoop) {
    match cheat {
        "rcp" => {
            RecreateContentPanels(app);
        }
        "ccw" => {
            create_control_window(app, event_loop);
        }
        _ => {
            log::debug!("Unknown cheat code: {cheat}");
        }
    }
}

/// Port of C++ `emMainWindow::RecreateContentPanels` (emMainWindow.cpp:280-306).
///
/// DIVERGED: RecreateContentPanels — C++ iterates all windows on the screen,
/// finds emMainWindow instances, and recreates each one's content panel while
/// preserving the visited location.  Rust has a single-window architecture with
/// thread-local storage; this logs the request but does not yet recreate panels.
fn RecreateContentPanels(_app: &mut App) {
    log::info!("emMainWindow::RecreateContentPanels — not yet implemented in Rust port");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = emMainWindowConfig::default();
        assert!(!config.fullscreen);
        assert!(config.visit.is_none());
        assert!(config.geometry.is_none());
        assert!((config.control_tallness - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_emMainWindow_new() {
        let ctx = emContext::NewRoot();
        let config = emMainWindowConfig::default();
        let mw = emMainWindow::new(ctx, config);
        assert!(mw.window_id.is_none());
        assert!(mw.main_panel_id.is_none());
        assert!(mw.startup_engine_id.is_none());
        assert!(!mw.to_close);
        assert!(!mw._visit_valid);
        assert!(!mw._visit_adherent);
        assert_eq!(mw._visit_rel_x, 0.0);
        assert_eq!(mw._visit_rel_y, 0.0);
        assert_eq!(mw._visit_rel_a, 0.0);
        assert!(mw._visit_subject.is_empty());
    }

    #[test]
    fn test_close_sets_flag() {
        let ctx = emContext::NewRoot();
        let config = emMainWindowConfig::default();
        let mut mw = emMainWindow::new(ctx, config);
        assert!(!mw.to_close);
        mw.Close();
        assert!(mw.to_close);
    }
}
