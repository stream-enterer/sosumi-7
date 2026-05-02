// Port of C++ emMainWindow.
//
// DIVERGED: (language-forced) C++ emMainWindow creates an OS window + emMainPanel + detached
// control window + StartupEngine.  Rust creates a single emWindow with
// emMainPanel as the root panel.  StartupEngine drives staged panel creation,
// autoplay input is wired via emAutoplayViewModel, and the window is persisted
// across frames via thread_local (set_main_window / with_main_window).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use winit::event_loop::ActiveEventLoop;

use emcore::emContext::emContext;
use emcore::emEngine::{EngineId, Priority, emEngine};
use emcore::emEngineCtx::EngineCtx;
use emcore::emGUIFramework::App;
use emcore::emInput::{InputKey, emInputEvent};
use emcore::emInputHotkey::Hotkey;
use emcore::emInputState::emInputState;
use emcore::emPanelScope::PanelScope;
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
    /// C++-mirrored: `emMainWindow.cpp:163` (`ToClose=true;`) drained at `cpp:184` uses the same set-and-Cycle-drain shape.
    /// Setter context lacks the ctx required for synchronous fire; matching C++ on this point.
    /// (D-009 verified non-issue per FU-004 inventory 2026-05-02.)
    pub to_close: bool,
    /// Cache of `app.file_update_signal`, populated post-construction by
    /// `create_main_window` (which has `&mut App`). `mw::new` runs without
    /// `&App` so we cannot eager-cache at construction — the cell is
    /// interior-mutable to permit late assignment. `ReloadFiles(ectx)` reads
    /// it; falls back to lazy-allocate (D-008 A1) for test paths that build
    /// `mw` without going through `create_main_window`.
    pub(crate) file_update_signal: Cell<Option<SignalId>>,
    pub(crate) _close_signal: Option<SignalId>,
    pub(crate) _visit_identity: Option<String>,
    pub(crate) _visit_rel_x: f64,
    pub(crate) _visit_rel_y: f64,
    pub(crate) _visit_rel_a: f64,
    pub(crate) _visit_adherent: bool,
    pub(crate) _visit_subject: String,
    pub(crate) _visit_valid: bool,
    pub(crate) config: emMainWindowConfig,
    /// Rc<RefCell<>> per CLAUDE.md §Ownership (a): cross-Cycle reference — the
    /// emAutoplayControlPanel (inside emMainControlPanel) holds a clone of this Rc
    /// to read/write model state across separate Cycle invocations.
    pub(crate) autoplay_view_model: Option<Rc<RefCell<crate::emAutoplay::emAutoplayViewModel>>>,
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
            file_update_signal: Cell::new(None),
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
        if let Some(win) = self.window_id.and_then(|id| app.windows.get_mut(&id)) {
            let new_flags = win.flags ^ WindowFlags::FULLSCREEN;
            win.SetWindowFlags(new_flags);
        }
    }

    /// Port of C++ `emMainWindow::ReloadFiles` (emMainWindow.cpp:281).
    /// Fires the global file-update signal so all listening file models reload.
    ///
    /// B-012 D-007 + D-009: takes `&mut EngineCtx` and fires synchronously from
    /// the click reaction in `emMainControlPanel::Cycle`. Replaces the pre-B-012
    /// two-hop relay (`mw.to_reload = true` polled by `MainWindowEngine::Cycle`).
    /// The `file_update_signal` is eagerly cached by `create_main_window`
    /// (D-008 A1) since `mw::new` does not receive the signal id at
    /// construction time. A cache miss is a wiring bug — fail loudly rather
    /// than silently allocating an unsubscribed signal.
    pub fn ReloadFiles(&self, ectx: &mut EngineCtx<'_>) {
        let sig = self
            .file_update_signal
            .get()
            .expect("file_update_signal must be cached by create_main_window");
        ectx.fire(sig);
    }

    /// Port of C++ `emMainWindow::ToggleControlView` (emMainWindow.cpp:144-158).
    ///
    /// Toggles the control view slider between open and closed. When the slider
    /// opens, focus shifts to the control sub-view; when it closes, focus shifts
    /// to the content sub-view. This matches C++ behavior where ToggleControlView
    /// toggles between ControlView.Activate() and ContentView.Activate().
    pub fn ToggleControlView(&mut self, app: &mut App) {
        if let Some(main_id) = self.main_panel_id {
            app.home_tree_mut()
                .with_behavior_as::<emMainPanel, _>(main_id, |mp| {
                    mp.DoubleClickSlider(None);
                });
            log::debug!("ToggleControlView");
        }
    }

    /// Port of C++ `emMainWindow::Close`.
    pub fn Close(&mut self) {
        self.to_close = true;
    }

    /// Port of C++ `emMainWindow::Quit`.
    pub fn Quit(&self, app: &mut App) {
        app.scheduler.InitiateTermination();
    }

    /// Port of C++ `emMainWindow::GetTitle` (emMainWindow.cpp:87-95).
    ///
    /// C++ returns "Eagle Mode - <content view title>" when MainPanel exists
    /// and startup is complete, otherwise just "Eagle Mode".
    pub fn GetTitle(&self, app: &App) -> String {
        if self.main_panel_id.is_some()
            && self.startup_engine_id.is_none()
            && let Some(win) = self.window_id.and_then(|id| app.windows.get(&id))
        {
            let view = win.view();
            let title = view.GetTitle();
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
            if let Some(win) = self.window_id.and_then(|id| app.windows.get(&id)) {
                let view = win.view();
                let mut rel_x = 0.0;
                let mut rel_y = 0.0;
                let mut rel_a = 0.0;
                // Phase 3.5.A Task 7: read from the home window's own tree.
                let tree = win.tree();
                let panel_opt = view.GetVisitedPanel(tree, &mut rel_x, &mut rel_y, &mut rel_a);
                let identity = panel_opt.map(|p| tree.GetIdentity(p));
                let adherent = view.IsActivationAdherent();
                (identity, rel_x, rel_y, rel_a, adherent)
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
        app.pending_actions
            .borrow_mut()
            .push(Box::new(move |app, event_loop| {
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
    /// DIVERGED: (language-forced) C++ Input uses emInputEvent, Rust uses the same struct but
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
                // B-012: input-path reload bifurcates from the click-path
                // `ReloadFiles(&self, ectx)`. The input handler holds
                // `&mut App`, not `&mut EngineCtx`, so it open-codes the fire
                // (1-line) rather than threading a shim. Click-path Cycle
                // route is the canonical C++-named one.
                app.scheduler.fire(app.file_update_signal);
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
        // Thread a SchedCtx so SetAutoplaying/ContinueLastAutoplay can fire
        // ChangeSignal per D-007 (synchronous fire at mutation site).
        if let Some(ref avm_rc) = self.autoplay_view_model {
            let handled =
                app.with_sched_ctx(|sc| avm_rc.borrow_mut().Input(sc, event, input_state));
            if handled {
                return true;
            }
        }

        // Bookmark hotkeys (C++ emMainWindow.cpp:247-260).
        if let Some(ref bm_model) = self.bookmarks_model
            && let Some(hotkey) = Hotkey::from_event_and_state(event.key, input_state)
        {
            let hotkey_str = hotkey.to_string();
            // Resolve the bookmark record under a short-lived borrow, then
            // drop the borrow before mutating `app` (which re-borrows the
            // bookmarks model transitively via the home tree). Mirrors the
            // click-reaction path in `emBookmarks.rs:721-781`, but runs
            // synchronously because `&mut App` is already in scope here
            // (the click path defers via `pending_actions` only because
            // `Cycle` lacks `&mut App`).
            let dispatch = {
                let bm = bm_model.borrow();
                bm.GetRec().SearchBookmarkByHotkey(&hotkey_str).map(|rec| {
                    (
                        rec.LocationIdentity.clone(),
                        rec.LocationRelX,
                        rec.LocationRelY,
                        rec.LocationRelA,
                        rec.entry.Name.clone(),
                    )
                })
            };
            if let Some((identity, rel_x, rel_y, rel_a, subject)) = dispatch {
                if let Some(main_panel_id) = self.main_panel_id
                    && let Some(content_view_id) = app
                        .home_tree_mut()
                        .with_behavior_as::<crate::emMainPanel::emMainPanel, _>(
                            main_panel_id,
                            |mp| mp.GetContentViewPanelId(),
                        )
                        .flatten()
                {
                    app.with_home_tree_and_sched_ctx(|tree, sc| {
                        tree.with_behavior_as::<emcore::emSubViewPanel::emSubViewPanel, _>(
                            content_view_id,
                            |svp| {
                                svp.visit_by_identity(
                                    &identity, rel_x, rel_y, rel_a, true, &subject, sc,
                                );
                            },
                        );
                    });
                }
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

/// Enqueue a deferred action that runs with `&mut emMainWindow` and `&mut App`.
///
/// The closure executes during the next `pending_actions` drain on the winit
/// main loop tick. This composes the `with_main_window` thread-local accessor
/// with the framework's `pending_actions` rail so reaction bodies inside
/// `Cycle` (which has `EngineCtx` but not `&mut App`) can invoke
/// `emMainWindow` methods that require `&mut App`.
///
/// Mirrors the pattern at `emBookmarks.rs:748` and `emMainWindow::Duplicate`
/// (line 233). Use from `Cycle` reaction bodies that need to invoke
/// MainWindow methods with the `&mut App` parameter (`Duplicate`,
/// `ToggleFullscreen`, `Quit`).
pub(crate) fn enqueue_main_window_action<F>(ectx: &mut EngineCtx<'_>, action: F)
where
    F: FnOnce(&mut emMainWindow, &mut App) + 'static,
{
    ectx.pending_actions
        .borrow_mut()
        .push(Box::new(move |app, _event_loop| {
            with_main_window(|mw| action(mw, app));
        }));
}

/// Engine for emMainWindow, matching C++ emMainWindow::Cycle()
/// (emMainWindow.cpp:174-190).
pub(crate) struct MainWindowEngine {
    close_signal: SignalId,
    title_signal: Option<SignalId>,
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
            && let Some(win) = ctx.windows.get(&wid)
        {
            let view = win.view();
            let view_title = view.GetTitle();
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

        // B-012/D-009: removed the to_reload polling intermediary. Reload
        // now fires synchronously inside emMainControlPanel::Cycle via
        // `mw.ReloadFiles(ectx)`.

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
        // DIVERGED: (language-forced) C++ parses visit string into identity/relX/relY/relA fields
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

/// Phase 3.5.A Task 6.2: StartupEngine is classified Toplevel, so
/// `ctx.tree` is Some at dispatch. NOTE: Task 7 migrates App's home
/// tree into `emWindow::tree`; at Task 6 exit, `windows[wid].tree` is
/// the empty default, NOT the home tree — StartupEngine's panel
/// construction operates on the empty tree. Production startup is
/// expected to be non-functional between Task 6 and Task 7.
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
                let overlay_id = ctx
                    .tree
                    .as_deref_mut()
                    .expect("StartupEngine: Toplevel scope")
                    .create_child(
                        self.main_panel_id,
                        "startupOverlay",
                        Some(&mut *ctx.scheduler),
                    );
                ctx.tree
                    .as_deref_mut()
                    .expect("StartupEngine: Toplevel scope")
                    .set_behavior(overlay_id, Box::new(StartupOverlayPanel));
                ctx.tree
                    .as_deref_mut()
                    .expect("StartupEngine: Toplevel scope")
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
                    .as_deref_mut()
                    .expect("StartupEngine: Toplevel scope")
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.GetControlViewPanelId()
                    })
                    .flatten();
                if let Some(ctrl_id) = ctrl_view_id {
                    let ctrl_ctx = Rc::clone(&self.context);
                    let avm_rc = with_main_window(|mw| mw.autoplay_view_model.clone()).flatten();
                    ctx.tree
                        .as_deref_mut()
                        .expect("StartupEngine: Toplevel scope")
                        .with_behavior_as::<emSubViewPanel, _>(ctrl_id, |svp| {
                            let sub_tree = svp.sub_tree_mut();
                            let sub_root = sub_tree.GetRootPanel().expect("sub-view has root");
                            // C++ creates emMainControlPanel as the root panel of the
                            // control view — set behavior on sub-tree root directly,
                            // matching C++ emMainWindow.cpp:408-413 and state 6 pattern.
                            // ROOT_SAME_TALLNESS on the sub-view will update sub_root
                            // height to ControlTallness via SetGeometry.
                            sub_tree.set_behavior(
                                sub_root,
                                Box::new(emMainControlPanel::new(ctrl_ctx, avm_rc)),
                            );
                            sub_tree.fire_init_notices(sub_root, None);
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
                    .as_deref_mut()
                    .expect("StartupEngine: Toplevel scope")
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.GetContentViewPanelId()
                    })
                    .flatten();

                if let Some(content_id) = content_view_id {
                    self.content_svp_id = Some(content_id);
                    let content_ctx = Rc::clone(&self.context);
                    let content_ctx2 = Rc::clone(&self.context);
                    ctx.tree
                        .as_deref_mut()
                        .expect("StartupEngine: Toplevel scope")
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
                            let cosmos_id = sub_tree.create_child(sub_root, "", None);
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
                            sub_tree.fire_init_notices(sub_root, None);
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
                    // C++ emMainWindow.cpp:429: VisitingVA->Activate(); —
                    // F010 root cause: the Rust port previously omitted this,
                    // so anim.active stayed false and SVP::Cycle's
                    // anim.animate() early-returned at `if !self.active`,
                    // dropping the animator without ever zooming the inner
                    // view to ":". Cosmos never became Viewed → no children
                    // → blank directory listing.
                    animator.Activate();
                    let tree = ctx
                        .tree
                        .as_deref_mut()
                        .expect("StartupEngine: Toplevel scope");
                    tree.with_behavior_as::<emSubViewPanel, _>(svp_id, |svp| {
                        svp.active_animator = Some(Box::new(animator));
                    });
                    // C++ animator-IS-engine: Activate() implicitly wakes
                    // the engine that drives Cycle. Rust splits these — the
                    // animator lives on svp.active_animator, but the engine
                    // that ticks it is the SVP's PanelCycleEngine in the
                    // outer scheduler. Wake it explicitly so SVP::Cycle
                    // runs next slice and dispatches anim.animate().
                    tree.wake_panel_cycle_engine(svp_id, ctx.scheduler);
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
                        ctx.tree
                            .as_deref_mut()
                            .expect("StartupEngine: Toplevel scope")
                            .with_behavior_as::<emSubViewPanel, _>(id, |svp| {
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
                    let mut installed = false;
                    let tree = ctx
                        .tree
                        .as_deref_mut()
                        .expect("StartupEngine: Toplevel scope");
                    tree.with_behavior_as::<emSubViewPanel, _>(svp_id, |svp| {
                        if let Some(ref mut anim) = svp.active_animator {
                            anim.stop();
                        }
                        if self.visit_valid {
                            use emcore::emViewAnimator::emVisitingViewAnimator;
                            let mut animator = emVisitingViewAnimator::new(0.0, 0.0, 0.0, 1.0);
                            animator.SetGoalCoords(
                                &self.visit_identity,
                                self.visit_rel_x,
                                self.visit_rel_y,
                                self.visit_rel_a,
                                self.visit_adherent,
                                &self.visit_subject,
                            );
                            // C++ emMainWindow.cpp:450: VisitingVA->Activate();
                            // — same omission as state 7. Required for the
                            // -visit CLI arg path to actually visit.
                            animator.Activate();
                            svp.active_animator = Some(Box::new(animator));
                            installed = true;
                        }
                    });
                    if installed {
                        tree.wake_panel_cycle_engine(svp_id, ctx.scheduler);
                    }
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
                        ctx.tree
                            .as_deref_mut()
                            .expect("StartupEngine: Toplevel scope")
                            .with_behavior_as::<emSubViewPanel, _>(id, |svp| {
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
                    // `with_behavior_as` borrows `ctx.tree`, so we can't build
                    // a SchedCtx from `ctx` inside the closure. Pull the
                    // behavior out explicitly, build the SchedCtx, then put
                    // it back — matches the take/put pattern used elsewhere.
                    if let Some(mut behavior) = ctx
                        .tree
                        .as_deref_mut()
                        .expect("StartupEngine: Toplevel scope")
                        .take_behavior(svp_id)
                    {
                        if let Some(svp) = behavior.as_any_mut().downcast_mut::<emSubViewPanel>() {
                            svp.active_animator = None;
                            let mut sc = ctx.as_sched_ctx();
                            svp.raw_zoom_out(false, &mut sc);
                        }
                        ctx.tree
                            .as_deref_mut()
                            .expect("StartupEngine: Toplevel scope")
                            .put_behavior(svp_id, behavior);
                    }
                }
                let overlay_id = ctx
                    .tree
                    .as_deref_mut()
                    .expect("StartupEngine: Toplevel scope")
                    .with_behavior_as::<emMainPanel, _>(self.main_panel_id, |mp| {
                        mp.ClearStartupOverlay()
                    })
                    .flatten();
                // C++ does `delete StartupOverlay` — remove from tree.
                if let Some(id) = overlay_id {
                    ctx.tree
                        .as_deref_mut()
                        .expect("StartupEngine: Toplevel scope")
                        .remove(id, Some(&mut *ctx.scheduler));
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
                    animator.SetGoalCoords(
                        &self.visit_identity,
                        self.visit_rel_x,
                        self.visit_rel_y,
                        self.visit_rel_a,
                        self.visit_adherent,
                        &self.visit_subject,
                    );
                    ctx.tree
                        .as_deref_mut()
                        .expect("StartupEngine: Toplevel scope")
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

/// Engine that bridges content-view active-panel changes to the control sub-view.
///
/// When the active panel changes in the content sub-view, `control_panel_signal`
/// fires. This engine reacts by recreating the ContentControlPanel child inside
/// emMainControlPanel using cross-tree `create_control_panel_in`.
///
/// DIVERGED: (language-forced) C++ emMainControlPanel directly subscribes to
/// ContentView.GetControlPanelSignal() and calls ContentView.CreateControlPanel()
/// from its own Cycle(). In Rust, the outer tree is taken by the scheduler before
/// SubView-scope panel engines dispatch, so emMainControlPanel::Cycle cannot reach
/// the content sub-view. This Framework-scope engine performs the cross-tree
/// recreation instead.
pub(crate) struct ControlPanelBridge {
    control_panel_signal: SignalId,
    window_id: winit::window::WindowId,
    ctrl_panel_id: PanelId,
    content_panel_id: PanelId,
    content_ctrl_panel: Option<PanelId>,
}

impl emEngine for ControlPanelBridge {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        if !ctx.IsSignaled(self.control_panel_signal) {
            return false;
        }
        let Some(win) = ctx.windows.get_mut(&self.window_id) else {
            return false;
        };
        let mut tree = win.take_tree();

        // Take both sub-view panel behaviors from the outer tree.
        let Some(mut ctrl_box) = tree.take_behavior(self.ctrl_panel_id) else {
            win.put_tree(tree);
            return false;
        };
        let Some(mut content_box) = tree.take_behavior(self.content_panel_id) else {
            tree.put_behavior(self.ctrl_panel_id, ctrl_box);
            win.put_tree(tree);
            return false;
        };

        let ctrl_svp = ctrl_box
            .as_any_mut()
            .downcast_mut::<emSubViewPanel>()
            .expect("ctrl panel must be emSubViewPanel");
        let content_svp = content_box
            .as_any_mut()
            .downcast_mut::<emSubViewPanel>()
            .expect("content panel must be emSubViewPanel");

        let ctrl_root = ctrl_svp
            .sub_tree()
            .GetRootPanel()
            .expect("ctrl sub-tree must have a root panel");
        let active_panel = content_svp.GetSubView().GetActivePanel();
        let tallness = content_svp.GetSubView().CurrentPixelTallness;

        // Delete old ContentControlPanel from ctrl sub-tree (C++ `delete ContentControlPanel`).
        if let Some(old_ccp) = self.content_ctrl_panel.take() {
            ctrl_svp.sub_tree_mut().remove(old_ccp, None);
        }

        // Create new ContentControlPanel via cross-tree call
        // (C++ ContentView.CreateControlPanel(*this, "context")).
        // F013: thread scheduler-reach handles through so the inner
        // PanelCtx can hand behaviors (e.g. emDirPanel::CreateControlPanel)
        // a full SchedCtx when they need to construct engine-registering
        // control-panel widgets.
        let new_ccp = match active_panel {
            Some(active) => {
                let ctrl_sub_tree = ctrl_svp.sub_tree_mut();
                let content_sub_tree = content_svp.sub_tree_mut();
                content_sub_tree.create_control_panel_in(
                    active,
                    ctrl_sub_tree,
                    ctrl_root,
                    "context",
                    tallness,
                    ctx.scheduler,
                    ctx.framework_actions,
                    ctx.root_context,
                    ctx.framework_clipboard,
                    ctx.pending_actions,
                )
            }
            None => None,
        };

        // Notify emMainControlPanel about the new child (sets layout weight 21.32).
        if let Some(ccp_id) = new_ccp {
            ctrl_svp
                .sub_tree_mut()
                .with_behavior_as::<emMainControlPanel, _>(ctrl_root, |mcp| {
                    mcp.set_content_control_panel(ccp_id);
                });
            self.content_ctrl_panel = Some(ccp_id);
        }

        // Put behaviors back.
        tree.put_behavior(self.ctrl_panel_id, ctrl_box);
        tree.put_behavior(self.content_panel_id, content_box);
        win.put_tree(tree);
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

    // Phase 3.5.A Task 7: home window owns its panel tree. Build the tree
    // locally, populate it, then move it onto the emWindow via `put_tree`
    // before inserting into `App::windows`. Formerly built into `App::tree`.
    let mut home_tree = emcore::emPanelTree::PanelTree::new();

    // Create root panel in the tree. View is not yet constructed (emWindow::create
    // happens below); wire the Weak back after the window is inserted.
    let panel = emMainPanel::new(Rc::clone(&app.context), mw.config.control_tallness);
    let root_id = home_tree.create_root("root", false);
    home_tree.set_behavior(root_id, Box::new(panel));
    mw.main_panel_id = Some(root_id);

    // Port of C++ `emMainPanel::emMainPanel` constructor
    // (emMainPanel.cpp:39-41): create control view, content view, and slider
    // children immediately at construction time. C++ has these as emView
    // members instantiated inline; in Rust the creator has tree access here.
    //
    // Create outer child slots first so ids are available for emMainPanel
    // configuration. emSubViewPanel::new is called after window creation so
    // the real WindowId can be passed to PanelScope::SubView.
    let ctrl_id = home_tree.create_child(root_id, "control view", None);
    let content_id = home_tree.create_child(root_id, "content view", None);
    let slider_id = home_tree.create_child(root_id, "slider", None);
    home_tree.set_behavior(slider_id, Box::new(SliderPanel::new()));

    home_tree.with_behavior_as::<emMainPanel, _>(root_id, |mp| {
        mp.set_control_view_panel(ctrl_id);
        mp.set_content_view_panel(content_id);
        mp.set_slider_panel(slider_id);
    });

    // Determine flags
    let mut flags = WindowFlags::AUTO_DELETE;
    if mw.config.fullscreen {
        flags |= WindowFlags::FULLSCREEN;
    }

    let close_signal = app.scheduler.create_signal();
    let flags_signal = app.scheduler.create_signal();
    let focus_signal = app.scheduler.create_signal();
    let geometry_signal = app.scheduler.create_signal();
    mw._close_signal = Some(close_signal);

    // Create the window
    let mut window = emWindow::create(
        event_loop,
        app.gpu(),
        Rc::clone(&app.context),
        root_id,
        flags,
        close_signal,
        flags_signal,
        focus_signal,
        geometry_signal,
    );
    let window_id = window.winit_window().id();

    // Construct emSubViewPanels now that window_id is known. Each
    // sub-view engine is registered at PanelScope::SubView{window_id, …}
    // so the outer scheduler dispatches them through the correct window
    // tree rather than re-sleeping every frame against WindowId::dummy().
    let ctrl_svp = {
        let root_ctx = app.context.GetRootContext();
        let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let mut sc = emcore::emEngineCtx::SchedCtx {
            scheduler: &mut app.scheduler,
            framework_actions: &mut fw,
            root_context: &root_ctx,
            view_context: None,
            framework_clipboard: &app.clipboard,
            current_engine: None,
            pending_actions: &app.pending_actions,
        };
        // SP7 §3.1: sub-view's emContext parents to the outer view's
        // emContext (matching C++ emSubViewPanel.cpp:114). Previously
        // this passed app.context (root), flattening the topology —
        // corrected 2026-04-24 when instrumenting cross-view dump.
        let home_view_ctx = window.view.GetContext().clone();
        let mut svp = emSubViewPanel::new(home_view_ctx, ctrl_id, window_id, &mut sc);
        svp.set_sub_view_flags(
            ViewFlags::POPUP_ZOOM | ViewFlags::ROOT_SAME_TALLNESS | ViewFlags::NO_ACTIVE_HIGHLIGHT,
        );
        svp
    };
    home_tree.set_behavior(ctrl_id, Box::new(ctrl_svp));

    let content_svp = {
        let root_ctx = app.context.GetRootContext();
        let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let mut sc = emcore::emEngineCtx::SchedCtx {
            scheduler: &mut app.scheduler,
            framework_actions: &mut fw,
            root_context: &root_ctx,
            view_context: None,
            framework_clipboard: &app.clipboard,
            current_engine: None,
            pending_actions: &app.pending_actions,
        };
        let home_view_ctx = window.view.GetContext().clone();
        let mut svp = emSubViewPanel::new(home_view_ctx, content_id, window_id, &mut sc);
        svp.set_sub_view_flags(ViewFlags::ROOT_SAME_TALLNESS);
        svp
    };
    home_tree.set_behavior(content_id, Box::new(content_svp));

    // Phase 3.5.A Task 7: mark the root panel as view-owned and hand the
    // tree to the window (it owns it from here on). Formerly this ran as
    // `app.tree.init_panel_view(root_id, None)` after `app.windows.insert`.
    home_tree.init_panel_view(root_id, None);
    window.put_tree(home_tree);

    app.windows.insert(window_id, window);
    mw.window_id = Some(window_id);
    // First top-level emMainWindow owns the home panel tree.
    if app.home_window_id.is_none() {
        app.home_window_id = Some(window_id);
    }

    // Set outer-view flags — C++ emMainWindow ctor (emMainWindow.cpp:52-57).
    // NO_ZOOM implies NO_USER_NAVIGATION via SetViewFlags invariant.
    {
        let App {
            ref mut scheduler,
            ref mut windows,
            ref clipboard,
            ref pending_actions,
            ..
        } = *app;
        if let Some(win) = windows.get_mut(&window_id) {
            let mut tree = win.take_tree();
            {
                let v = win.view_mut();
                let root_ctx = v.GetRootContext();
                let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
                let mut sc = emcore::emEngineCtx::SchedCtx {
                    scheduler,
                    framework_actions: &mut fw,
                    root_context: &root_ctx,
                    view_context: None,
                    framework_clipboard: clipboard,
                    current_engine: None,
                    pending_actions,
                };
                v.SetViewFlags(
                    ViewFlags::ROOT_SAME_TALLNESS
                        | ViewFlags::NO_ZOOM
                        | ViewFlags::NO_FOCUS_HIGHLIGHT
                        | ViewFlags::NO_ACTIVE_HIGHLIGHT,
                    &mut tree,
                    &mut sc,
                );
            }
            win.put_tree(tree);
        }
    }

    // Acquire bookmarks model.
    mw.bookmarks_model = Some(emBookmarksModel::Acquire(&app.context));

    // Register StartupEngine with the scheduler.
    let startup_engine =
        StartupEngine::new(Rc::clone(&app.context), root_id, window_id, &mw.config);
    let engine_id = app.scheduler.register_engine(
        Box::new(startup_engine),
        Priority::Low,
        PanelScope::Toplevel(window_id),
    );
    app.scheduler.wake_up(engine_id);
    mw.startup_engine_id = Some(engine_id);

    // Register MainWindowEngine — wakes only on signals, no wake_up call
    // (C++ emMainWindow::Cycle, emMainWindow.cpp:174-190).
    let title_signal = app.scheduler.create_signal();
    if let Some(win) = app.windows.get_mut(&window_id) {
        win.view_mut().set_title_signal(title_signal);
    }
    // B-012 D-008 A1: eager-cache the file_update_signal on the emMainWindow
    // so `mw.ReloadFiles(ectx)` can fire it without a lazy-allocate path. The
    // signal originates from `App` (set at App::new), so we wire it here at
    // window creation rather than threading `app` into `mw::new`.
    mw.file_update_signal.set(Some(app.file_update_signal));

    let mw_engine = MainWindowEngine {
        close_signal,
        title_signal: Some(title_signal),
        window_id: Some(window_id),
        startup_done: false,
    };
    let mw_engine_id =
        app.scheduler
            .register_engine(Box::new(mw_engine), Priority::Low, PanelScope::Framework);
    app.scheduler.connect(close_signal, mw_engine_id);
    app.scheduler.connect(title_signal, mw_engine_id);

    // Wire control panel signal + ControlPanelBridge.
    // The bridge reacts to control panel signal (active panel changes) and
    // will update the content control panel in the control sub-view.
    let cp_signal = app.scheduler.create_signal();
    {
        let App {
            ref mut scheduler,
            ref mut windows,
            ref clipboard,
            ref pending_actions,
            ..
        } = *app;
        if let Some(win) = windows.get_mut(&window_id) {
            let scope = emcore::emPanelScope::PanelScope::Toplevel(window_id);
            // Phase 3.5.A Task 7: window owns its tree — take it out for the
            // RegisterEngines call, then put it back.
            let mut tree = win.take_tree();
            {
                let v = win.view_mut();
                let root_ctx = v.GetRootContext();
                let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
                let mut sc = emcore::emEngineCtx::SchedCtx {
                    scheduler,
                    framework_actions: &mut fw,
                    root_context: &root_ctx,
                    view_context: None,
                    framework_clipboard: clipboard,
                    current_engine: None,
                    pending_actions,
                };
                v.RegisterEngines(&mut sc, &mut tree, scope);
            }
            win.put_tree(tree);
            // Wire cp_signal to the content sub-view's emView so it fires when
            // the active panel changes (C++ ContentView.GetControlPanelSignal()).
            // The outer window's view is NOT the right target — active panel
            // changes happen in content_svp.sub_view, a separate emView object.
            let mut tree = win.take_tree();
            tree.with_behavior_as::<emSubViewPanel, _>(content_id, |svp| {
                svp.sub_view_mut().set_control_panel_signal(cp_signal);
            });
            win.put_tree(tree);
        }
    }
    let bridge = ControlPanelBridge {
        control_panel_signal: cp_signal,
        window_id,
        ctrl_panel_id: ctrl_id,
        content_panel_id: content_id,
        content_ctrl_panel: None,
    };
    let bridge_id =
        app.scheduler
            .register_engine(Box::new(bridge), Priority::Low, PanelScope::Framework);
    app.scheduler.connect(cp_signal, bridge_id);

    // Register emWindowStateSaver engine — persists window geometry.
    // Port of C++ emWindowStateSaver construction in emMainWindow constructor.
    {
        let state_path = emcore::emInstallInfo::emGetInstallPath(
            emcore::emInstallInfo::InstallDirType::UserConfig,
            "emMain",
            Some("WinState.rec"),
        )
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/eaglemode-winstate.rec"));
        let change_signal = app.scheduler.create_signal();
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
        if let Some(win) = windows.get_mut(&window_id) {
            saver.Restore(win, screen);
        }

        let saver_id =
            app.scheduler
                .register_engine(Box::new(saver), Priority::Low, PanelScope::Framework);
        app.scheduler.connect(flags_signal, saver_id);
        app.scheduler.connect(focus_signal, saver_id);
        app.scheduler.connect(geometry_signal, saver_id);
    }

    mw.autoplay_view_model = Some(Rc::new(RefCell::new(
        crate::emAutoplay::emAutoplayViewModel::new(),
    )));

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
        if let Some(win) = app.windows.get(&cw_id) {
            win.winit_window().focus_window();
            return Some(cw_id);
        }
        // Window was closed/removed — clear stale ID.
        with_main_window(|mw| {
            mw.control_window_id = None;
        });
    }

    // C++ emMainWindow.cpp:315-326: Create new control window if MainPanel exists.
    with_main_window(|mw| mw.main_panel_id).flatten()?;

    let avm_rc = with_main_window(|mw| mw.autoplay_view_model.clone()).flatten();
    let ctrl_panel = emMainControlPanel::new(Rc::clone(&app.context), avm_rc);

    // Phase 3.5.A Task 7: the control window owns its own tree (detached peer
    // of the home window — follows the same per-window pattern). Build locally,
    // populate, then `put_tree` onto the new emWindow.
    let mut ctrl_tree = emcore::emPanelTree::PanelTree::new();
    let root_id = ctrl_tree.create_root("ctrl_window_root", false);
    ctrl_tree.set_behavior(root_id, Box::new(ctrl_panel));

    let flags = WindowFlags::AUTO_DELETE;
    let close_signal = app.scheduler.create_signal();
    let flags_signal = app.scheduler.create_signal();
    let focus_signal = app.scheduler.create_signal();
    let geometry_signal = app.scheduler.create_signal();

    let mut window = emWindow::create(
        event_loop,
        app.gpu(),
        Rc::clone(&app.context),
        root_id,
        flags,
        close_signal,
        flags_signal,
        focus_signal,
        geometry_signal,
    );
    let window_id = window.winit_window().id();

    // Mark the root panel as view-owned and hand the tree to the window.
    ctrl_tree.init_panel_view(root_id, None);
    window.put_tree(ctrl_tree);

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
/// DIVERGED: (language-forced) RecreateContentPanels — C++ iterates all windows on the screen,
/// finds emMainWindow instances, and recreates each one's content panel while
/// preserving the visited location.  Rust has a single-window architecture with
/// thread-local storage, so we operate on the single main window instead.
fn RecreateContentPanels(app: &mut App) {
    let main_panel_id = match with_main_window(|mw| mw.main_panel_id).flatten() {
        Some(id) => id,
        None => return,
    };

    let content_view_id = match app
        .home_tree_mut()
        .with_behavior_as::<emMainPanel, _>(main_panel_id, |mp| mp.GetContentViewPanelId())
        .flatten()
    {
        Some(id) => id,
        None => return,
    };

    let root_ctx = Rc::clone(&app.context);

    app.with_home_tree_and_sched_ctx(|tree, sc| {
        tree.with_behavior_as::<emSubViewPanel, _>(content_view_id, |svp| {
            // Save current visit state (C++ emMainWindow.cpp:297-301).
            let mut rel_x = 0.0;
            let mut rel_y = 0.0;
            let mut rel_a = 0.0;
            let sv = svp.GetSubView();
            let panel_opt = sv.GetVisitedPanel(svp.sub_tree(), &mut rel_x, &mut rel_y, &mut rel_a);
            let identity = panel_opt
                .map(|p| svp.sub_tree().GetIdentity(p))
                .unwrap_or_default();
            // C++ emMainWindow.cpp:297, 301, 304 — snapshot title+adherent
            // before the content panel is rebuilt, then feed them back into
            // the Visit call afterward.
            let title = sv.GetTitle().to_string();
            let adherent = sv.IsActivationAdherent();
            let _ = sv;

            // Delete old content panel(s) — remove all children of sub-tree root
            // (C++ emMainWindow.cpp:302).
            let sub_root = svp.sub_root();
            let children: Vec<PanelId> = svp.sub_tree().children(sub_root).collect();
            for child in children {
                svp.sub_tree_mut().remove(child, None);
            }

            // Create new content panel (C++ emMainWindow.cpp:303).
            let sub_tree = svp.sub_tree_mut();
            let child_id = sub_tree.create_child(sub_root, "", None);
            sub_tree.set_behavior(child_id, Box::new(emMainContentPanel::new(root_ctx)));
            sub_tree.Layout(child_id, 0.0, 0.0, 1.0, 1.0, 1.0, None);

            // Restore visit (C++ emMainWindow.cpp:304).
            svp.visit_by_identity(&identity, rel_x, rel_y, rel_a, adherent, &title, sc);
        });
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

    /// FU-002: `enqueue_main_window_action` pushes exactly one closure onto the
    /// `EngineCtx::pending_actions` rail. This is the load-bearing observable
    /// for the three wired click reactions (BtNewWindow / BtFullscreen /
    /// BtQuit) — each reaction body is a one-line call to this helper, so
    /// asserting the helper enqueues asserts the reactions enqueue.
    ///
    /// The closure body itself (`mw.Duplicate(app)` / `mw.ToggleFullscreen(app)`
    /// / `mw.Quit(app)`) is covered transitively by the keyboard paths in
    /// `emMainWindow::Input` (F4 / F11 / Shift+Alt+F4) which call the same
    /// methods synchronously; we do not spin up a winit event loop here.
    #[test]
    fn fu002_enqueue_main_window_action_pushes_one_deferred_action() {
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::rc::Rc;

        use emcore::emEngine::EngineId;
        use emcore::emScheduler::EngineScheduler;

        let mut sched = EngineScheduler::new();
        let root_ctx = emContext::NewRoot();
        let mut windows: HashMap<winit::window::WindowId, emcore::emWindow::emWindow> =
            HashMap::new();
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(winit::window::WindowId, emcore::emInput::emInputEvent)> =
            Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let fw_cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));

        let mut ectx = emcore::emEngineCtx::EngineCtx {
            scheduler: &mut sched,
            tree: None,
            windows: &mut windows,
            root_context: &root_ctx,
            view_context: None,
            framework_actions: &mut fw_actions,
            pending_inputs: &mut pending_inputs,
            input_state: &mut input_state,
            framework_clipboard: &fw_cb,
            engine_id: EngineId::default(),
            pending_actions: &pa,
        };

        assert_eq!(pa.borrow().len(), 0, "precondition: queue starts empty");

        enqueue_main_window_action(&mut ectx, |_mw, _app| {
            // Body intentionally empty: the production reaction bodies call
            // `mw.Duplicate(app)` / `mw.ToggleFullscreen(app)` / `mw.Quit(app)`
            // — covered by the keyboard paths in `emMainWindow::Input`.
        });

        assert_eq!(
            pa.borrow().len(),
            1,
            "FU-002: helper must enqueue exactly one deferred action"
        );
    }
}

#[cfg(test)]
mod port_topology_tests {
    /// After `create_main_window`, every sub-view's emContext must be a
    /// child of the home view's emContext — not a sibling under the root
    /// context. Matches C++ `emSubViewPanel.cpp:114` and SP7 spec §3.1.
    #[test]
    fn sub_view_contexts_nest_under_home_view_context() {
        use emcore::emContext::emContext;
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;
        use emcore::emSubViewPanel::emSubViewPanel;
        use std::cell::RefCell;
        use std::rc::Rc;

        let root = emContext::NewRoot();
        let home_view_ctx = emContext::NewChild(&root);

        let mut outer_tree = PanelTree::new();
        let outer_root = outer_tree.create_root("root", false);
        let outer_id = outer_tree.create_child(outer_root, "slot", None);
        let mut sched = EngineScheduler::new();
        let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let wid = winit::window::WindowId::dummy();

        let mut svp = {
            let mut sc = emcore::emEngineCtx::SchedCtx {
                scheduler: &mut sched,
                framework_actions: &mut fw,
                root_context: &root,
                view_context: None,
                framework_clipboard: &cb,
                current_engine: None,
                pending_actions: &pa,
            };
            emSubViewPanel::new(Rc::clone(&home_view_ctx), outer_id, wid, &mut sc)
        };

        let sub_parent = svp
            .sub_view
            .GetContext()
            .GetParentContext()
            .expect("sub-view context must have a parent");
        let parent_is_home = Rc::ptr_eq(&sub_parent, &home_view_ctx);
        let parent_is_root = Rc::ptr_eq(&sub_parent, &root);

        // Teardown engines/panels before scheduler Drop to avoid the
        // "no dangling engines" debug_assert. Mirrors the harness in
        // crates/emcore/src/emSubViewPanel.rs `SvpTestHarness::teardown`.
        let sub_root = svp.sub_root();
        svp.sub_tree_mut().remove(sub_root, Some(&mut sched));
        if let Some(eid) = svp.sub_view.update_engine_id.take() {
            sched.remove_engine(eid);
        }
        if let Some(eid) = svp.sub_view.visiting_va_engine_id.take() {
            sched.remove_engine(eid);
        }
        if let Some(sig) = svp.sub_view.EOISignal.take() {
            sched.remove_signal(sig);
        }
        drop(svp);

        assert!(
            parent_is_home,
            "sub-view's context parent should be home_view_ctx, not root or anything else"
        );
        assert!(
            !parent_is_root,
            "sub-view's context parent must not be the root context"
        );
    }

    #[test]
    #[ignore = "requires winit event loop; real check lives in Phase 7 integration test"]
    fn create_main_window_produces_nested_subview_contexts() {
        unreachable!("see F010_subview_dump_nests_under_home_view_context in Phase 7");
    }
}
