// Port of C++ emMainWindow.
//
// DIVERGED: C++ emMainWindow creates an OS window + emMainPanel + detached
// control window + StartupEngine.  Rust creates a single emWindow with
// emMainPanel as the root panel.  StartupEngine drives staged panel creation,
// autoplay input is wired via emAutoplayViewModel, and the window is persisted
// across frames via thread_local (set_main_window / with_main_window).

use std::cell::RefCell;
use std::rc::Rc;

use winit::event_loop::ActiveEventLoop;

use emcore::emContext::emContext;
use emcore::emEngine::{EngineCtx, EngineId, Priority, emEngine};
use emcore::emGUIFramework::App;
use emcore::emInput::{InputKey, emInputEvent};
use emcore::emInputHotkey::Hotkey;
use emcore::emInputState::emInputState;
use emcore::emPanelTree::PanelId;
use emcore::emSignal::SignalId;
use emcore::emWindow::{WindowFlags, emWindow};
use emcore::emWindowStateSaver::emWindowStateSaver;

use emcore::emSubViewPanel::emSubViewPanel;

use emcore::emView::ViewFlags;

use crate::emBookmarks::emBookmarksModel;
use crate::emMainContentPanel::emMainContentPanel;
use crate::emMainControlPanel::emMainControlPanel;
use crate::emMainPanel::emMainPanel;
use crate::emMainPanel::{SliderPanel, StartupOverlayPanel};

/// Configuration for creating an emMainWindow.
pub struct emMainWindowConfig {
    pub geometry: Option<String>, // "WxH+X+Y"
    pub fullscreen: bool,
    pub visit: Option<String>,
    pub visit_rel_x: f64,
    pub visit_rel_y: f64,
    pub visit_rel_a: f64,
    pub visit_adherent: bool,
    pub control_tallness: f64,
}

impl Default for emMainWindowConfig {
    fn default() -> Self {
        Self {
            geometry: None,
            fullscreen: false,
            visit: None,
            visit_rel_x: 0.0,
            visit_rel_y: 0.0,
            visit_rel_a: 0.0,
            visit_adherent: false,
            control_tallness: 0.0538,
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
    /// Window ID of the detached control window created by `CreateControlWindow`.
    pub(crate) control_window_id: Option<winit::window::WindowId>,
    pub(crate) startup_engine_id: Option<EngineId>,
    pub to_close: bool,
    pub to_reload: bool,
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
    pub(crate) bookmarks_model: Option<Rc<RefCell<emBookmarksModel>>>,
}

impl emMainWindow {
    pub(crate) fn new(ctx: Rc<emContext>, config: emMainWindowConfig) -> Self {
        Self {
            window_id: None,
            _ctx: ctx,
            main_panel_id: None,
            _control_panel_id: None,
            _content_panel_id: None,
            control_window_id: None,
            startup_engine_id: None,
            to_close: false,
            to_reload: false,
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
            bookmarks_model: None,
        }
    }

    /// Port of C++ `emMainWindow::ToggleFullscreen`.
    pub fn ToggleFullscreen(&self, app: &mut App) {
        if let Some(rc) = self.window_id.and_then(|id| app.windows.get(&id)) {
            let mut win = rc.borrow_mut();
            let new_flags = win.flags ^ WindowFlags::FULLSCREEN;
            win.SetWindowFlags(new_flags);
        }
    }

    /// Port of C++ `emMainWindow::ReloadFiles`.
    /// Fires the global file-update signal so all listening file models reload.
    pub fn ReloadFiles(&self, app: &App) {
        app.scheduler.borrow_mut().fire(app.file_update_signal);
    }

    /// Port of C++ `emMainWindow::ToggleControlView` (emMainWindow.cpp:144-158).
    ///
    /// Toggles the control view slider between open and closed. When the slider
    /// opens, focus shifts to the control sub-view; when it closes, focus shifts
    /// to the content sub-view. This matches C++ behavior where ToggleControlView
    /// toggles between ControlView.Activate() and ContentView.Activate().
    pub fn ToggleControlView(&mut self, app: &mut App) {
        if let Some(main_id) = self.main_panel_id {
            app.tree.with_behavior_as::<emMainPanel, _>(main_id, |mp| {
                mp.DoubleClickSlider();
            });
            log::debug!("ToggleControlView");
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
    pub fn GetTitle(&self, app: &App) -> String {
        if self.main_panel_id.is_some()
            && self.startup_engine_id.is_none()
            && let Some(rc) = self.window_id.and_then(|id| app.windows.get(&id))
        {
            let win = rc.borrow();
            let title = win.view().GetTitle();
            if !title.is_empty() {
                return format!("Eagle Mode - {title}");
            }
        }
        "Eagle Mode".to_string()
    }

    /// Port of C++ `emMainWindow::Duplicate` (emMainWindow.cpp:98-129).
    ///
    /// Extracts the current visited panel identity, relative position, and
    /// adherence from the content view, then queues a deferred action to
    /// create a new window visiting the same location.
    pub fn Duplicate(&self, app: &mut App) {
        // Extract visit info from the current window's view (C++ emMainWindow.cpp:112-117).
        let (visit_identity, rel_x, rel_y, rel_a, adherent) =
            if let Some(rc) = self.window_id.and_then(|id| app.windows.get(&id)) {
                let win = rc.borrow();
                let view = win.view();
                let visit = view.current_visit();
                let identity = app.tree.GetIdentity(visit.panel);
                let adherent = view.IsActivationAdherent();
                (
                    Some(identity),
                    visit.rel_x,
                    visit.rel_y,
                    visit.rel_a,
                    adherent,
                )
            } else {
                (None, 0.0, 0.0, 0.0, false)
            };

        let config = emMainWindowConfig {
            visit: visit_identity,
            fullscreen: false,
            ..Default::default()
        };

        // Store visit params for the StartupEngine to use.
        let dup_rel_x = rel_x;
        let dup_rel_y = rel_y;
        let dup_rel_a = rel_a;
        let dup_adherent = adherent;

        // Queue deferred window creation (needs &ActiveEventLoop).
        app.pending_actions.push(Box::new(move |app, event_loop| {
            let mut dup_config = config;
            // Encode full visit params into the config's visit_rel fields.
            dup_config.visit_rel_x = dup_rel_x;
            dup_config.visit_rel_y = dup_rel_y;
            dup_config.visit_rel_a = dup_rel_a;
            dup_config.visit_adherent = dup_adherent;
            let mw = create_main_window(app, event_loop, dup_config);
            set_main_window(mw);
            log::info!("emMainWindow::Duplicate — created new window");
        }));
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
                if !input_state.GetShift() && !input_state.GetCtrl() && !input_state.GetAlt() =>
            {
                self.Duplicate(app);
                true
            }
            // Alt+F4: Close (C++ emMainWindow.cpp:209-212)
            InputKey::F4
                if !input_state.GetShift() && !input_state.GetCtrl() && input_state.GetAlt() =>
            {
                self.Close();
                true
            }
            // Shift+Alt+F4: Quit (C++ emMainWindow.cpp:213-216)
            InputKey::F4
                if input_state.GetShift() && !input_state.GetCtrl() && input_state.GetAlt() =>
            {
                self.Quit(app);
                true
            }
            // F5 no modifier: Reload (C++ emMainWindow.cpp:219-222)
            InputKey::F5
                if !input_state.GetShift() && !input_state.GetCtrl() && !input_state.GetAlt() =>
            {
                self.ReloadFiles(app);
                true
            }
            // F11 no modifier: Toggle fullscreen (C++ emMainWindow.cpp:225-228)
            InputKey::F11
                if !input_state.GetShift() && !input_state.GetCtrl() && !input_state.GetAlt() =>
            {
                self.ToggleFullscreen(app);
                true
            }
            // Escape no modifier: Toggle control view (C++ emMainWindow.cpp:230-237)
            InputKey::Escape
                if !input_state.GetShift() && !input_state.GetCtrl() && !input_state.GetAlt() =>
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

        // Bookmark hotkeys (C++ emMainWindow.cpp:247-260).
        if let Some(ref bm_model) = self.bookmarks_model
            && let Some(hotkey) = Hotkey::from_event_and_state(event.key, input_state)
        {
            let hotkey_str = hotkey.to_string();
            let bm = bm_model.borrow();
            if let Some(rec) = bm.GetRec().SearchBookmarkByHotkey(&hotkey_str) {
                // DIVERGED: C++ calls MainPanel->GetContentView().Visit() with
                // identity-based navigation. Rust uses the visiting animator
                // directly on the window. Full wiring requires identity-based
                // Visit on emView (not yet ported).
                log::info!("Bookmark hotkey {}: visit {}", hotkey_str, rec.entry.Name);
                return true;
            }
        }

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
    MAIN_WINDOW.with(|cell| cell.borrow_mut().as_mut().map(f))
}

/// Engine for emMainWindow, matching C++ emMainWindow::Cycle()
/// (emMainWindow.cpp:174-190).
pub(crate) struct MainWindowEngine {
    close_signal: SignalId,
    title_signal: Option<SignalId>,
    file_update_signal: SignalId,
    window_id: Option<winit::window::WindowId>,
    startup_done: bool,
}

impl emEngine for MainWindowEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        // Check close signal (C++ emMainWindow.cpp:180-181).
        if ctx.IsSignaled(self.close_signal) {
            with_main_window(|mw| {
                mw.to_close = true;
            });
        }

        // Check title signal — update window title to "Eagle Mode - <content title>".
        if let Some(title_sig) = self.title_signal
            && ctx.IsSignaled(title_sig)
            && let Some(wid) = self.window_id
            && let Some(rc) = ctx.windows.get(&wid)
        {
            let win = rc.borrow();
            let view_title = win.view().GetTitle();
            let title = if view_title.is_empty() {
                "Eagle Mode".to_string()
            } else {
                format!("Eagle Mode - {view_title}")
            };
            win.winit_window().set_title(&title);
        }

        // Check if startup is now done.
        if !self.startup_done {
            let done = with_main_window(|mw| mw.startup_engine_id.is_none()).unwrap_or(false);
            if done {
                self.startup_done = true;
            }
        }

        // Poll reload flag set by emMainControlPanel::Cycle and fire the
        // file_update_signal (C++ emMainWindow::ReloadFiles).
        let to_reload = with_main_window(|mw| mw.to_reload).unwrap_or(false);
        if to_reload {
            with_main_window(|mw| {
                mw.to_reload = false;
            });
            ctx.fire(self.file_update_signal);
        }

        // Self-delete if to_close (C++ emMainWindow.cpp:184-187).
        let to_close = with_main_window(|mw| mw.to_close).unwrap_or(false);
        if to_close {
            return false;
        }

        false // Sleep until signaled
    }
}

/// Startup engine registered with the scheduler.
///
/// Port of C++ `emMainWindow::StartupEngineClass` (emMainWindow.cpp:362-485).
/// States 0-6 drive panel creation; states 7-11 drive the startup zoom
/// animation.  Directly manipulates the panel tree and windows via `EngineCtx`,
/// matching the C++ design where the engine holds references and acts directly.
pub(crate) struct StartupEngine {
    state: u8,
    context: Rc<emContext>,
    main_panel_id: PanelId,
    _window_id: winit::window::WindowId,
    visit_valid: bool,
    visit_identity: String,
    visit_rel_x: f64,
    visit_rel_y: f64,
    visit_rel_a: f64,
    visit_adherent: bool,
    visit_subject: String,
    clock: std::time::Instant,
    /// Cached content sub-view panel ID (set in state 6).
    content_svp_id: Option<PanelId>,
}

impl StartupEngine {
    pub(crate) fn new(
        context: Rc<emContext>,
        main_panel_id: PanelId,
        window_id: winit::window::WindowId,
        config: &emMainWindowConfig,
    ) -> Self {
        // DIVERGED: C++ parses visit string into identity/relX/relY/relA fields
        // at construction time (emMainWindow.cpp:338-361).  Rust stores the raw
        // visit string as identity; bookmark-based visit is filled in at state 4.
        let (visit_valid, visit_identity) = match config.visit {
            Some(ref v) if !v.is_empty() => (true, v.clone()),
            _ => (false, String::new()),
        };
        Self {
            state: 0,
            context,
            main_panel_id,
            _window_id: window_id,
            visit_valid,
            visit_identity,
            visit_rel_x: config.visit_rel_x,
            visit_rel_y: config.visit_rel_y,
            visit_rel_a: config.visit_rel_a,
            visit_adherent: config.visit_adherent,
            visit_subject: String::new(),
            clock: std::time::Instant::now(),
            content_svp_id: None,
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
            // C++ `MainPanel->SetStartupOverlay(true)` creates the overlay
            // child directly on the main panel. In Rust the engine has tree
            // access, so we create the child here and hand its id to emMainPanel.
            3 => {
                let overlay_id = ctx.tree.create_child(self.main_panel_id, "startupOverlay");
                ctx.tree
                    .set_behavior(overlay_id, Box::new(StartupOverlayPanel));
                ctx.tree
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.set_startup_overlay(overlay_id);
                    });
                self.state += 1;
                true
            }
            // State 4: Load bookmarks model, search start location
            // (C++ emMainWindow.cpp:391-406).
            4 => {
                let bm = emBookmarksModel::Acquire(&self.context);
                if !self.visit_valid
                    && let Some(rec) = bm.borrow().GetRec().SearchStartLocation()
                {
                    self.visit_valid = true;
                    self.visit_identity = rec.LocationIdentity.clone();
                    self.visit_rel_x = rec.LocationRelX;
                    self.visit_rel_y = rec.LocationRelY;
                    self.visit_rel_a = rec.LocationRelA;
                    self.visit_adherent = true;
                    self.visit_subject = rec.entry.Name.clone();
                }
                self.state += 1;
                // C++ falls through to state 5 if time permits; always stays awake.
                true
            }
            // State 5: Create control panel directly in control sub-view's sub-tree
            // (C++ emMainWindow.cpp:407-415).
            5 => {
                let ctrl_view_id = ctx
                    .tree
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.GetControlViewPanelId()
                    })
                    .flatten();
                let content_view_id = ctx
                    .tree
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.GetContentViewPanelId()
                    })
                    .flatten();
                if let Some(ctrl_id) = ctrl_view_id {
                    let ctrl_ctx = Rc::clone(&self.context);
                    ctx.tree
                        .with_behavior_as::<emSubViewPanel, _>(ctrl_id, |svp| {
                            let sub_tree = svp.sub_tree_mut();
                            let sub_root = sub_tree.GetRootPanel().expect("sub-view has root");
                            let child_id = sub_tree.create_child(sub_root, "ctrl");
                            sub_tree.set_behavior(
                                child_id,
                                Box::new(emMainControlPanel::new(ctrl_ctx, content_view_id)),
                            );
                            // C++ control tallness matches the parent's control_tallness
                            // C++ control panel fills the control view; tallness matches
                            // ControlTallness (0.0538) set on emMainPanel.
                            sub_tree.Layout(child_id, 0.0, 0.0, 1.0, 0.0538, 1.0);
                        });
                }

                self.state += 1;
                // C++ falls through to state 6 if time permits; always stays awake.
                true
            }
            // State 6: Create content panel directly in content sub-view's sub-tree
            // (C++ emMainWindow.cpp:416-422).
            6 => {
                let content_view_id = ctx
                    .tree
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.GetContentViewPanelId()
                    })
                    .flatten();

                if let Some(content_id) = content_view_id {
                    self.content_svp_id = Some(content_id);
                    let content_ctx = Rc::clone(&self.context);
                    let content_ctx2 = Rc::clone(&self.context);
                    ctx.tree
                        .with_behavior_as::<emSubViewPanel, _>(content_id, |svp| {
                            let sub_tree = svp.sub_tree_mut();
                            let sub_root = sub_tree.GetRootPanel().expect("sub-view has root");
                            // C++ creates emMainContentPanel as the root panel
                            // of the content view — set behavior on sub-tree root
                            // directly (no extra child level).
                            sub_tree.set_behavior(
                                sub_root,
                                Box::new(emMainContentPanel::new(content_ctx)),
                            );
                            // C++ emMainContentPanel constructor creates the
                            // cosmos panel directly (not via AutoExpand). Do
                            // the same here so the cosmos is permanent
                            // (not marked created_by_ae) and won't be deleted
                            // by AutoShrink.
                            let cosmos_id = sub_tree.create_child(sub_root, "");
                            sub_tree.set_behavior(
                                cosmos_id,
                                Box::new(crate::emVirtualCosmos::emVirtualCosmosPanel::new(
                                    content_ctx2,
                                )),
                            );
                            sub_tree.with_behavior_as::<emMainContentPanel, _>(sub_root, |mcp| {
                                mcp.set_cosmos_panel(cosmos_id);
                            });
                            // Re-fire init notices so the new behavior gets
                            // notice + LayoutChildren calls.
                            sub_tree.fire_init_notices(sub_root);
                        });
                }

                self.state += 1;
                // C++ falls through to state 7 if time permits; always stays awake.
                true
            }
            // State 7: Create visiting animator on content sub-view, zoom to ":"
            // (C++ emMainWindow.cpp:423-432).
            // C++: VisitingVA=new emVisitingViewAnimator(ContentView)
            7 => {
                if let Some(svp_id) = self.content_svp_id {
                    use emcore::emViewAnimator::emVisitingViewAnimator;
                    let mut animator = emVisitingViewAnimator::new(0.0, 0.0, 0.0, 1.0);
                    animator.SetAnimated(false);
                    animator.SetGoalFullsized(":", false, false, "");
                    ctx.tree
                        .with_behavior_as::<emSubViewPanel, _>(svp_id, |svp| {
                            svp.active_animator = Some(Box::new(animator));
                        });
                }
                self.clock = std::time::Instant::now();
                self.state += 1;
                // C++ falls through to state 8 if time permits; always stays awake.
                true
            }
            // State 8: Wait up to 2s or until animator inactive
            // (C++ emMainWindow.cpp:433-438).
            8 => {
                let still_active = self
                    .content_svp_id
                    .and_then(|id| {
                        ctx.tree.with_behavior_as::<emSubViewPanel, _>(id, |svp| {
                            svp.active_animator
                                .as_ref()
                                .map(|a| a.is_active())
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);
                if self.clock.elapsed().as_millis() < 2000 && still_active {
                    return true;
                }
                self.state += 1;
                true
            }
            // State 9: Deactivate animator; set visit goal if valid
            // (C++ emMainWindow.cpp:439-454).
            9 => {
                if let Some(svp_id) = self.content_svp_id {
                    ctx.tree
                        .with_behavior_as::<emSubViewPanel, _>(svp_id, |svp| {
                            if let Some(ref mut anim) = svp.active_animator {
                                anim.stop();
                            }
                            if self.visit_valid {
                                use emcore::emViewAnimator::emVisitingViewAnimator;
                                let mut animator = emVisitingViewAnimator::new(0.0, 0.0, 0.0, 1.0);
                                animator.set_goal_rel(
                                    &self.visit_identity,
                                    self.visit_rel_x,
                                    self.visit_rel_y,
                                    self.visit_rel_a,
                                    self.visit_adherent,
                                    &self.visit_subject,
                                );
                                svp.active_animator = Some(Box::new(animator));
                            }
                        });
                }
                self.clock = std::time::Instant::now();
                self.state += 1;
                // C++ falls through to state 10 if time permits; always stays awake.
                true
            }
            // State 10: Wait up to 2s, then clean up overlay and animator
            // (C++ emMainWindow.cpp:455-465).
            10 => {
                let still_active = self
                    .content_svp_id
                    .and_then(|id| {
                        ctx.tree.with_behavior_as::<emSubViewPanel, _>(id, |svp| {
                            svp.active_animator
                                .as_ref()
                                .map(|a| a.is_active())
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);
                if self.clock.elapsed().as_millis() < 2000 && still_active {
                    return true;
                }
                // Clean up animator and zoom out on content sub-view.
                // C++: VisitingVA.Reset(); ContentView.RawZoomOut();
                if let Some(svp_id) = self.content_svp_id {
                    ctx.tree
                        .with_behavior_as::<emSubViewPanel, _>(svp_id, |svp| {
                            svp.active_animator = None;
                            let (view, tree) = svp.view_and_tree_mut();
                            view.RawZoomOut(tree, false);
                        });
                }
                let overlay_id = ctx
                    .tree
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.ClearStartupOverlay()
                    })
                    .flatten();
                // C++ does `delete StartupOverlay` — remove from tree.
                if let Some(id) = overlay_id {
                    ctx.tree.remove(id);
                }
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
                // Final visit on content sub-view (C++ ContentView.Visit()).
                if self.visit_valid
                    && let Some(svp_id) = self.content_svp_id
                {
                    // C++ emMainWindow default case:
                    //   ContentView.Visit(identity, relX, relY, relA, adherent, subject)
                    // which creates an animated visit (new
                    // VisitingViewAnimator with the goal).
                    use emcore::emViewAnimator::emVisitingViewAnimator;
                    let mut animator = emVisitingViewAnimator::new(0.0, 0.0, 0.0, 1.0);
                    animator.set_goal_rel(
                        &self.visit_identity,
                        self.visit_rel_x,
                        self.visit_rel_y,
                        self.visit_rel_a,
                        self.visit_adherent,
                        &self.visit_subject,
                    );
                    ctx.tree
                        .with_behavior_as::<emSubViewPanel, _>(svp_id, |svp| {
                            svp.active_animator = Some(Box::new(animator));
                        });
                }
                // Store startup_engine_id = None in main window to indicate startup is done.
                with_main_window(|mw| {
                    mw.startup_engine_id = None;
                });
                false // engine stops permanently
            }
        }
    }
}

/// Engine that bridges control panel signal to control sub-view.
///
/// When the active panel changes in the content view, the control panel signal
/// fires. This engine reacts by logging the change (full wiring to recreate
/// the content control panel will be added later).
pub(crate) struct ControlPanelBridge {
    control_panel_signal: SignalId,
    _ctrl_view_id: PanelId,
    _content_view_id: PanelId,
}

impl emEngine for ControlPanelBridge {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        if ctx.IsSignaled(self.control_panel_signal) {
            log::debug!("ControlPanelBridge: control panel signal fired");
        }
        false
    }
}

/// Create an emMainWindow: inserts the root emMainPanel into the panel tree,
/// allocates signals, creates the emWindow, and registers a StartupEngine.
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

    // Port of C++ `emMainPanel::emMainPanel` constructor
    // (emMainPanel.cpp:39-41): create control view, content view, and slider
    // children immediately at construction time. C++ has these as emView
    // members instantiated inline; in Rust the creator has tree access here.
    let mut ctrl_svp = emSubViewPanel::new();
    ctrl_svp.set_sub_view_flags(
        ViewFlags::POPUP_ZOOM | ViewFlags::ROOT_SAME_TALLNESS | ViewFlags::NO_ACTIVE_HIGHLIGHT,
    );
    let ctrl_id = app.tree.create_child(root_id, "control view");
    app.tree.set_behavior(ctrl_id, Box::new(ctrl_svp));

    let mut content_svp = emSubViewPanel::new();
    content_svp.set_sub_view_flags(ViewFlags::ROOT_SAME_TALLNESS);
    let content_id = app.tree.create_child(root_id, "content view");
    app.tree.set_behavior(content_id, Box::new(content_svp));

    let slider_id = app.tree.create_child(root_id, "slider");
    app.tree
        .set_behavior(slider_id, Box::new(SliderPanel::new()));

    app.tree.with_behavior_as::<emMainPanel, _>(root_id, |mp| {
        mp.set_control_view_panel(ctrl_id);
        mp.set_content_view_panel(content_id);
        mp.set_slider_panel(slider_id);
    });

    // Determine flags
    let mut flags = WindowFlags::AUTO_DELETE;
    if mw.config.fullscreen {
        flags |= WindowFlags::FULLSCREEN;
    }

    let close_signal = app.scheduler.borrow_mut().create_signal();
    let flags_signal = app.scheduler.borrow_mut().create_signal();
    let focus_signal = app.scheduler.borrow_mut().create_signal();
    let geometry_signal = app.scheduler.borrow_mut().create_signal();
    mw._close_signal = Some(close_signal);

    // Create the window
    let window = emWindow::create(
        event_loop,
        app.gpu(),
        root_id,
        flags,
        close_signal,
        flags_signal,
        focus_signal,
        geometry_signal,
    );
    let window_id = window.borrow().winit_window().id();
    app.windows.insert(window_id, window);
    mw.window_id = Some(window_id);

    // Acquire bookmarks model.
    mw.bookmarks_model = Some(emBookmarksModel::Acquire(&app.context));

    // Register StartupEngine with the scheduler.
    let startup_engine =
        StartupEngine::new(Rc::clone(&app.context), root_id, window_id, &mw.config);
    let engine_id = app
        .scheduler
        .borrow_mut()
        .register_engine(Priority::Low, Box::new(startup_engine));
    app.scheduler.borrow_mut().wake_up(engine_id);
    mw.startup_engine_id = Some(engine_id);

    // Register MainWindowEngine — wakes only on signals, no wake_up call
    // (C++ emMainWindow::Cycle, emMainWindow.cpp:174-190).
    let title_signal = app.scheduler.borrow_mut().create_signal();
    if let Some(rc) = app.windows.get(&window_id) {
        rc.borrow_mut().view_mut().set_title_signal(title_signal);
    }
    let mw_engine = MainWindowEngine {
        close_signal,
        title_signal: Some(title_signal),
        file_update_signal: app.file_update_signal,
        window_id: Some(window_id),
        startup_done: false,
    };
    let mw_engine_id = app
        .scheduler
        .borrow_mut()
        .register_engine(Priority::Low, Box::new(mw_engine));
    app.scheduler
        .borrow_mut()
        .connect(close_signal, mw_engine_id);
    app.scheduler
        .borrow_mut()
        .connect(title_signal, mw_engine_id);

    // Wire control panel signal + ControlPanelBridge.
    // The bridge reacts to control panel signal (active panel changes) and
    // will update the content control panel in the control sub-view.
    let cp_signal = app.scheduler.borrow_mut().create_signal();
    if let Some(rc) = app.windows.get(&window_id) {
        let mut win = rc.borrow_mut();
        win.view_mut()
            .attach_to_scheduler(Rc::clone(&app.scheduler), window_id);
        win.view_mut().set_control_panel_signal(cp_signal);
    }
    // We don't yet have the sub-view panel IDs (created during LayoutChildren),
    // so use a dummy PanelId(0) for now — the bridge only uses the signal.
    let bridge = ControlPanelBridge {
        control_panel_signal: cp_signal,
        _ctrl_view_id: root_id,
        _content_view_id: root_id,
    };
    let bridge_id = app
        .scheduler
        .borrow_mut()
        .register_engine(Priority::Low, Box::new(bridge));
    app.scheduler.borrow_mut().connect(cp_signal, bridge_id);

    // Register emWindowStateSaver engine — persists window geometry.
    // Port of C++ emWindowStateSaver construction in emMainWindow constructor.
    {
        let state_path = emcore::emInstallInfo::emGetInstallPath(
            emcore::emInstallInfo::InstallDirType::UserConfig,
            "emMain",
            Some("WinState.rec"),
        )
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/eaglemode-winstate.rec"));
        let change_signal = app.scheduler.borrow_mut().create_signal();
        let saver = emWindowStateSaver::new(
            window_id,
            state_path,
            flags_signal,
            focus_signal,
            geometry_signal,
            false, // allowRestoreFullscreen
            change_signal,
        );

        // Restore geometry before registering (C++ does this in the constructor).
        // Destructure app to allow simultaneous mutable window + immutable
        // screen borrows (disjoint field borrowing).
        let App {
            ref screen,
            ref mut windows,
            ..
        } = *app;
        let screen = screen.as_ref().expect("Screen not initialized");
        if let Some(rc) = windows.get(&window_id) {
            saver.Restore(&mut rc.borrow_mut(), screen);
        }

        let saver_id = app
            .scheduler
            .borrow_mut()
            .register_engine(Priority::Low, Box::new(saver));
        app.scheduler.borrow_mut().connect(flags_signal, saver_id);
        app.scheduler.borrow_mut().connect(focus_signal, saver_id);
        app.scheduler
            .borrow_mut()
            .connect(geometry_signal, saver_id);
    }

    mw.autoplay_view_model = Some(crate::emAutoplay::emAutoplayViewModel::new());

    mw
}

/// Create a detached control window.
///
/// Port of C++ `emMainWindow::CreateControlWindow` (emMainWindow.cpp:309-327).
/// If a control window already exists and is still alive, raises it.
/// Otherwise creates a new OS window with `WF_AUTO_DELETE`, hosting an
/// `emMainControlPanel` linked to the content sub-view.
///
/// Triggered by the `"ccw"` cheat code in `DoCustomCheat`.
pub fn create_control_window(
    app: &mut App,
    event_loop: &ActiveEventLoop,
) -> Option<winit::window::WindowId> {
    // C++ emMainWindow.cpp:311-313: If ControlWindow exists, raise it.
    let existing_id = with_main_window(|mw| mw.control_window_id).flatten();
    if let Some(cw_id) = existing_id {
        if let Some(rc) = app.windows.get(&cw_id) {
            rc.borrow().winit_window().focus_window();
            return Some(cw_id);
        }
        // Window was closed/removed — clear stale ID.
        with_main_window(|mw| {
            mw.control_window_id = None;
        });
    }

    // C++ emMainWindow.cpp:315-326: Create new control window if MainPanel exists.
    let main_panel_id = with_main_window(|mw| mw.main_panel_id).flatten()?;

    // Get the content sub-view panel ID from emMainPanel.
    let content_view_id = app
        .tree
        .with_behavior_as::<emMainPanel, _>(main_panel_id, |mp| mp.GetContentViewPanelId())
        .flatten();

    let ctrl_panel = emMainControlPanel::new(Rc::clone(&app.context), content_view_id);
    let root_id = app.tree.create_root("ctrl_window_root");
    app.tree.set_behavior(root_id, Box::new(ctrl_panel));

    let flags = WindowFlags::AUTO_DELETE;
    let close_signal = app.scheduler.borrow_mut().create_signal();
    let flags_signal = app.scheduler.borrow_mut().create_signal();
    let focus_signal = app.scheduler.borrow_mut().create_signal();
    let geometry_signal = app.scheduler.borrow_mut().create_signal();

    let window = emWindow::create(
        event_loop,
        app.gpu(),
        root_id,
        flags,
        close_signal,
        flags_signal,
        focus_signal,
        geometry_signal,
    );
    let window_id = window.borrow().winit_window().id();
    app.windows.insert(window_id, window);

    // Store the control window ID for raise-if-existing logic.
    with_main_window(|mw| {
        mw.control_window_id = Some(window_id);
    });

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
/// thread-local storage, so we operate on the single main window instead.
fn RecreateContentPanels(app: &mut App) {
    let main_panel_id = match with_main_window(|mw| mw.main_panel_id).flatten() {
        Some(id) => id,
        None => return,
    };

    let content_view_id = match app
        .tree
        .with_behavior_as::<emMainPanel, _>(main_panel_id, |mp| mp.GetContentViewPanelId())
        .flatten()
    {
        Some(id) => id,
        None => return,
    };

    let ctx = Rc::clone(&app.context);

    app.tree
        .with_behavior_as::<emSubViewPanel, _>(content_view_id, |svp| {
            // Save current visit state (C++ emMainWindow.cpp:297-301).
            let visit = svp.GetSubView().current_visit();
            let identity = svp.sub_tree().GetIdentity(visit.panel);
            let rel_x = visit.rel_x;
            let rel_y = visit.rel_y;
            let rel_a = visit.rel_a;

            // Delete old content panel(s) — remove all children of sub-tree root
            // (C++ emMainWindow.cpp:302).
            let sub_root = svp.sub_root();
            let children: Vec<PanelId> = svp.sub_tree().children(sub_root).collect();
            for child in children {
                svp.sub_tree_mut().remove(child);
            }

            // Create new content panel (C++ emMainWindow.cpp:303).
            let sub_tree = svp.sub_tree_mut();
            let child_id = sub_tree.create_child(sub_root, "");
            sub_tree.set_behavior(child_id, Box::new(emMainContentPanel::new(ctx)));
            sub_tree.Layout(child_id, 0.0, 0.0, 1.0, 1.0, 1.0);

            // Restore visit (C++ emMainWindow.cpp:304).
            svp.visit_by_identity(&identity, rel_x, rel_y, rel_a);
        });

    log::info!("emMainWindow::RecreateContentPanels — content panels recreated");
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
        assert!((config.control_tallness - 0.0538).abs() < 1e-10);
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
