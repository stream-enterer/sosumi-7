// Port of C++ emMain/emMainControlPanel
// Sidebar panel containing window control buttons and bookmarks.
//
// C++ emMainControlPanel extends emLinearGroup and builds a deep widget tree
// (emButton, emCheckButton, emLinearGroup, emPackGroup, etc.).
// Rust replicates this structure using emLinearLayout for child arrangement,
// emBorder for border painting, and real emButton/emCheckButton widgets wrapped
// in PanelBehavior adapters.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use emcore::emBorder::{InnerBorderType, OuterBorderType, emBorder};
use emcore::emButton::emButton;
use emcore::emCheckButton::emCheckButton;
use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emCursor::emCursor;
use emcore::emEngineCtx::{EngineCtx, PanelCtx};
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLinearLayout::emLinearLayout;
use emcore::emLook::emLook;
use emcore::emPainter::emPainter;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::PanelId;
use emcore::emTiling::{ChildConstraint, Orientation, Spacing};
use emcore::emWindow::WindowFlags;

use crate::emAutoplay::emAutoplayViewModel;
use crate::emAutoplayControlPanel::emAutoplayControlPanel;
use crate::emBookmarks::emBookmarksPanel;
use crate::emMainConfig::emMainConfig;

// ── Click flags ──────────────────────────────────────────────────────────────
// Shared state between button on_click callbacks and the Cycle method.

#[derive(Default)]
struct ClickFlags {
    new_window: Cell<bool>,
    fullscreen: Cell<bool>,
    auto_hide_control_view: Cell<bool>,
    auto_hide_slider: Cell<bool>,
    reload: Cell<bool>,
    close: Cell<bool>,
    quit: Cell<bool>,
}

// ── ButtonPanel ──────────────────────────────────────────────────────────────
// PanelBehavior wrapper for emButton (mirrors emcore's pub(crate) ButtonPanel).

struct MainButtonPanel {
    button: emButton,
}

impl PanelBehavior for MainButtonPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.button
            .Paint(painter, canvas_color, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.button.Input(event, state, input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.button.GetCursor()
    }

    fn get_title(&self) -> Option<String> {
        Some(self.button.GetCaption().to_string())
    }
}

// ── CheckButtonPanel ─────────────────────────────────────────────────────────
// PanelBehavior wrapper for emCheckButton.
//
// The inner emCheckButton is held behind Rc<RefCell<>> so that
// emMainControlPanel::Cycle can read/write check state via its own
// bt_fullscreen field (Rc<RefCell<>> justification (b): context-registry-style
// shared widget handle per CLAUDE.md §Ownership).

struct MainCheckButtonPanel {
    check_button: Rc<RefCell<emCheckButton>>,
}

impl PanelBehavior for MainCheckButtonPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.check_button.borrow_mut().Paint(
            painter,
            canvas_color,
            w,
            h,
            state.enabled,
            pixel_scale,
        );
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.check_button
            .borrow_mut()
            .Input(event, state, input_state, ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.check_button.borrow().GetCursor()
    }
}

// ── emMainControlPanel ───────────────────────────────────────────────────────

/// Sidebar panel with window control buttons and bookmarks.
///
/// Port of C++ `emMainControlPanel` (extends `emLinearGroup`).
/// Uses emBorder for border painting and emLinearLayout for child arrangement,
/// matching C++ emLinearGroup's inheritance chain.
pub struct emMainControlPanel {
    ctx: Rc<emContext>,
    /// Renamed from `_config` (removing underscore that suppressed unused warning
    /// now that Cycle actively reads config state for row-219 reaction).
    /// Matches C++ member name `MainConfig`.
    /// Rc<RefCell<>> justification (b): context-registry typed singleton shared via Acquire.
    config: Rc<RefCell<emMainConfig>>,
    border: emBorder,
    look: emLook,
    /// Top-level linear layout: 2 children (lMain, contentControlPanel).
    /// C++ SetChildWeight(0, 11.37) SetChildWeight(1, 21.32).
    layout_main: emLinearLayout,
    click_flags: Rc<ClickFlags>,
    autoplay_model: Rc<RefCell<emAutoplayViewModel>>,
    // Panel IDs for child widgets (used for layout weight assignment).
    lmain_panel: Option<PanelId>,
    content_ctrl_panel: Option<PanelId>,
    children_created: bool,
    /// D-006 first-Cycle init guard. Set to true after signal connections are
    /// established. Mirrors C++ constructor's AddWakeUpSignal calls (rows 218-219).
    subscribed_init: bool,
    /// Port of C++ `emMainControlPanel::BtFullscreen` (emMainControlPanel.h).
    /// Rc<RefCell<>> justification (b): context-registry-style shared widget
    /// handle — emMainControlPanel::Cycle reads/writes check state; the panel
    /// tree's MainCheckButtonPanel holds the same Rc for paint/input dispatch.
    /// Populated by create_children; None until first LayoutChildren.
    pub(crate) bt_fullscreen: Option<Rc<RefCell<emCheckButton>>>,
    /// Port of C++ `emMainControlPanel::BtAutoHideControlView`.
    /// C++ places this widget inside BtFullscreen->HaveAux() in an emRasterGroup.
    /// Rust has no HaveAux/emRasterGroup port yet, so the button is created as a
    /// detached emCheckButton (not in the panel tree). Populated by create_children;
    /// paint/input wiring deferred until HaveAux/emRasterGroup infrastructure is
    /// ported.
    /// Rc<RefCell<>> justification (b): shared widget handle for Cycle mutation + future panel-tree placement.
    pub(crate) bt_auto_hide_control_view: Option<Rc<RefCell<emCheckButton>>>,
    /// Port of C++ `emMainControlPanel::BtAutoHideSlider`. Same deferred port as
    /// bt_auto_hide_control_view — detached until HaveAux/emRasterGroup infrastructure ports.
    /// Rc<RefCell<>> justification (b): shared widget handle for Cycle mutation + future panel-tree placement.
    pub(crate) bt_auto_hide_slider: Option<Rc<RefCell<emCheckButton>>>,
}

impl emMainControlPanel {
    /// Port of C++ `emMainControlPanel` constructor.
    ///
    /// `autoplay_model`: the `Rc<RefCell<emAutoplayViewModel>>` owned by
    /// `emMainWindow`. Pass `None` only in tests that construct a panel without
    /// a parent window (a fresh model will be created as fallback).
    pub fn new(
        ctx: Rc<emContext>,
        autoplay_model: Option<Rc<RefCell<emAutoplayViewModel>>>,
    ) -> Self {
        let config = emMainConfig::Acquire(&ctx);

        // C++ emMainControlPanel constructor:
        //   SetOuterBorderType(OBT_POPUP_ROOT)
        //   SetInnerBorderType(IBT_NONE)
        let border = emBorder::new(OuterBorderType::PopupRoot).with_inner(InnerBorderType::None);

        // C++ layout:
        //   SetMinCellCount(2)
        //   SetOrientationThresholdTallness(1.0)
        //   SetChildWeight(0, 11.37)
        //   SetChildWeight(1, 21.32)
        //   SetInnerSpace(0.0098, 0.0098)
        let layout_main = emLinearLayout {
            orientation: Orientation::Adaptive {
                tallness_threshold: 1.0,
            },
            spacing: Spacing {
                inner_h: 0.0098,
                inner_v: 0.0098,
                ..Spacing::default()
            },
            min_cell_count: 2,
            ..emLinearLayout::horizontal()
        };

        // Accept the window's shared model or create a standalone one (test path).
        let autoplay_model =
            autoplay_model.unwrap_or_else(|| Rc::new(RefCell::new(emAutoplayViewModel::new())));

        Self {
            ctx,
            config,
            border,
            look: emLook::default(),
            layout_main,
            click_flags: Rc::new(ClickFlags::default()),
            autoplay_model,
            lmain_panel: None,
            content_ctrl_panel: None,
            children_created: false,
            subscribed_init: false,
            bt_fullscreen: None,
            bt_auto_hide_control_view: None,
            bt_auto_hide_slider: None,
        }
    }

    /// Test helper: return a clone of the shared ViewModel Rc so integration
    /// tests can verify instance identity (C1 regression check) without making
    /// the field pub. Named `_for_test` to signal test-only intent; compiled
    /// unconditionally because integration tests link the library in non-test
    /// mode and `#[cfg(test)]` would not gate it correctly across the crate
    /// boundary.
    pub fn autoplay_model_for_test(&self) -> Rc<RefCell<emAutoplayViewModel>> {
        Rc::clone(&self.autoplay_model)
    }

    /// Called by ControlPanelBridge after cross-tree CreateControlPanel.
    /// Sets the content control panel child with weight 21.32.
    pub(crate) fn set_content_control_panel(&mut self, id: PanelId) {
        self.content_ctrl_panel = Some(id);
        self.layout_main.set_child_constraint(
            id,
            ChildConstraint {
                weight: 21.32,
                ..Default::default()
            },
        );
    }

    /// Create the full child widget tree matching C++ constructor.
    ///
    /// C++ top-level layout has 2 children:
    ///   child 0: lMain (weight 11.37) — contains general + bookmarks
    ///   child 1: contentControlPanel (weight 21.32) — placeholder for now
    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::new(self.look.clone());
        let flags = Rc::clone(&self.click_flags);

        // ── Allocate shared button handles ────────────────────────────────
        // These Rc<RefCell<emCheckButton>> are shared between emMainControlPanel
        // (for Cycle reactions) and CommandsPanel (for paint/input dispatch of
        // bt_fullscreen). The auto_hide buttons are detached — see field doc.
        //
        // emCheckButton::new requires a scheduler-backed ConstructCtx to allocate
        // check_signal. Use as_sched_ctx() so layout-only test contexts (no
        // scheduler) degrade gracefully: buttons remain None and Cycle reactions
        // are skipped until a real scheduler is present. In production the panel
        // always has a scheduler by first LayoutChildren.
        let bt_fullscreen_opt: Option<Rc<RefCell<emCheckButton>>> =
            ctx.as_sched_ctx().map(|mut sched| {
                let mut btn_fs = emCheckButton::new(&mut sched, "Fullscreen", Rc::clone(&look));
                btn_fs.SetDescription(
                    "Switch between fullscreen mode and normal window mode.\n\nHotkey: F11",
                );
                Rc::new(RefCell::new(btn_fs))
            });
        self.bt_fullscreen = bt_fullscreen_opt.clone();

        // BtAutoHideControlView and BtAutoHideSlider live inside BtFullscreen->HaveAux()
        // in an emRasterGroup in C++. Rust has no HaveAux/emRasterGroup port yet.
        // The buttons are created here as detached emCheckButton instances (not in the
        // panel tree). Their check state is updated by the Cycle reaction and tested by
        // typed_subscribe_b006; paint/input wiring is deferred until
        // HaveAux/emRasterGroup infrastructure is ported.
        let bt_auto_hide_control_view_opt: Option<Rc<RefCell<emCheckButton>>> =
            ctx.as_sched_ctx().map(|mut sched| {
                Rc::new(RefCell::new(emCheckButton::new(
                    &mut sched,
                    "Auto-Hide Control View",
                    Rc::clone(&look),
                )))
            });
        self.bt_auto_hide_control_view = bt_auto_hide_control_view_opt.clone();

        let bt_auto_hide_slider_opt: Option<Rc<RefCell<emCheckButton>>> =
            ctx.as_sched_ctx().map(|mut sched| {
                Rc::new(RefCell::new(emCheckButton::new(
                    &mut sched,
                    "Auto-Hide Slider",
                    Rc::clone(&look),
                )))
            });
        self.bt_auto_hide_slider = bt_auto_hide_slider_opt.clone();

        // ── lMain: wraps general + bookmarks (child 0 of top-level) ──────
        let lmain = Box::new(LMainPanel::new(
            Rc::clone(&self.ctx),
            Rc::clone(&look),
            Rc::clone(&flags),
            Rc::clone(&self.autoplay_model),
            bt_fullscreen_opt,
        ));
        let lmain_id = ctx.create_child_with("lMain", lmain);
        self.lmain_panel = Some(lmain_id);

        // C++ top-level: child 0 (lMain weight 11.37) child 1 (contentControlPanel weight 21.32)
        self.layout_main.set_child_constraint(
            lmain_id,
            ChildConstraint {
                weight: 11.37,
                ..Default::default()
            },
        );

        self.children_created = true;
    }
}

impl PanelBehavior for emMainControlPanel {
    /// Port of C++ `emMainControlPanel::GetTitle`.
    fn get_title(&self) -> Option<String> {
        Some("emMainControl".to_string())
    }

    fn IsOpaque(&self) -> bool {
        true
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            false,
            state.enabled,
            pixel_scale,
        );
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        use emcore::emInput::InputKey;
        // Escape no-modifier: toggle control view (C++ emMainWindow.cpp:230-237).
        if event.key == InputKey::Escape
            && !input_state.GetShift()
            && !input_state.GetCtrl()
            && !input_state.GetAlt()
        {
            log::info!("ToggleControlView");
            return true;
        }
        false
    }

    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        // ── D-006 first-Cycle init: subscribe to model signals ────────────
        // Mirrors C++ constructor rows 218-219 (emMainControlPanel.cpp:218-219):
        //   AddWakeUpSignal(MainWin.GetWindowFlagsSignal())  [row 218]
        //   AddWakeUpSignal(MainConfig->GetChangeSignal())   [row 219]
        // Row 217 (ContentView.GetControlPanelSignal) is handled by
        // ControlPanelBridge — see DIVERGED block in emMainWindow.rs:819-824.
        if !self.subscribed_init {
            let eid = ectx.id();
            // Row 218: window flags signal. Access path: thread-local emMainWindow
            // → window_id → ectx.windows[wid].GetWindowFlagsSignal().
            // (emMainWindow does not expose GetWindowFlagsSignal directly;
            // the signal lives on emWindow which is keyed in ectx.windows.)
            if let Some(wid) = crate::emMainWindow::with_main_window(|mw| mw.window_id).flatten()
                && let Some(sig) = ectx.windows.get(&wid).map(|w| w.GetWindowFlagsSignal())
            {
                ectx.connect(sig, eid);
            }
            // Row 219: config change signal.
            let cfg_sig = self.config.borrow().GetChangeSignal();
            ectx.connect(cfg_sig, eid);
            self.subscribed_init = true;
        }

        // ── Row 218 reaction: update bt_fullscreen check state ────────────
        // Mirrors C++ Cycle 253-255:
        //   if (IsSignaled(MainWin.GetWindowFlagsSignal()))
        //     BtFullscreen->SetChecked((MainWin.GetWindowFlags()&WF_FULLSCREEN)!=0);
        let flags_signal_opt = crate::emMainWindow::with_main_window(|mw| mw.window_id)
            .flatten()
            .and_then(|wid| ectx.windows.get(&wid).map(|w| w.GetWindowFlagsSignal()));
        if let Some(flags_signal) = flags_signal_opt
            && ectx.IsSignaled(flags_signal)
        {
            let is_fs = crate::emMainWindow::with_main_window(|mw| mw.window_id)
                .flatten()
                .and_then(|wid| {
                    ectx.windows
                        .get(&wid)
                        .map(|w| w.flags.contains(WindowFlags::FULLSCREEN))
                })
                .unwrap_or(false);
            if let Some(bt) = &self.bt_fullscreen {
                bt.borrow_mut().SetChecked(is_fs, ctx);
            }
        }

        // ── Row 219 reaction: update auto-hide check buttons from config ──
        // Mirrors C++ Cycle 257-260:
        //   if (IsSignaled(MainConfig->GetChangeSignal()))
        //     BtAutoHideControlView->SetChecked(MainConfig->AutoHideControlView);
        //     BtAutoHideSlider->SetChecked(MainConfig->AutoHideSlider);
        let cfg_sig = self.config.borrow().GetChangeSignal();
        if ectx.IsSignaled(cfg_sig) {
            let auto_hide_cv = self.config.borrow().GetAutoHideControlView();
            let auto_hide_sl = self.config.borrow().GetAutoHideSlider();
            if let Some(bt) = &self.bt_auto_hide_control_view {
                bt.borrow_mut().SetChecked(auto_hide_cv, ctx);
            }
            if let Some(bt) = &self.bt_auto_hide_slider {
                bt.borrow_mut().SetChecked(auto_hide_sl, ctx);
            }
        }

        // Poll click flags and dispatch to main window.
        // Port of C++ Cycle() signal handling.
        let flags = &self.click_flags;

        if flags.new_window.take() {
            // C++ MainWin.Duplicate() — not yet implemented, log it.
            log::info!("emMainControlPanel: New Window requested (Duplicate not yet implemented)");
        }

        if flags.fullscreen.take() {
            crate::emMainWindow::with_main_window(|mw| {
                log::info!("emMainControlPanel: Fullscreen toggle requested (requires App access)");
                let _ = mw;
            });
        }

        if flags.auto_hide_control_view.take() {
            log::info!("emMainControlPanel: AutoHideControlView toggled");
        }

        if flags.auto_hide_slider.take() {
            log::info!("emMainControlPanel: AutoHideSlider toggled");
        }

        if flags.reload.take() {
            crate::emMainWindow::with_main_window(|mw| {
                mw.to_reload = true;
            });
        }

        if flags.close.take() {
            crate::emMainWindow::with_main_window(|mw| {
                mw.Close();
            });
        }

        if flags.quit.take() {
            log::info!(
                "emMainControlPanel: Quit requested (requires App access for InitiateTermination)"
            );
        }

        false
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.create_children(ctx);
        }

        let r = ctx.layout_rect();
        let cr = self.border.GetContentRect(r.w, r.h, &self.look);
        self.layout_main.do_layout_skip(ctx, None, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

// ── LMainPanel ──────────────────────────────────────────────────────────────
// C++ lMain: linear layout containing general (lAbtCfgCmd, weight 4.71) and
// bookmarks (weight 6.5).

struct LMainPanel {
    ctx: Rc<emContext>,
    look: Rc<emLook>,
    layout: emLinearLayout,
    click_flags: Rc<ClickFlags>,
    autoplay_model: Rc<RefCell<emAutoplayViewModel>>,
    /// Shared fullscreen button handle threaded from emMainControlPanel.
    /// None in layout-only test contexts (no scheduler).
    bt_fullscreen: Option<Rc<RefCell<emCheckButton>>>,
    general_panel: Option<PanelId>,
    bookmarks_panel: Option<PanelId>,
    children_created: bool,
}

impl LMainPanel {
    fn new(
        ctx: Rc<emContext>,
        look: Rc<emLook>,
        click_flags: Rc<ClickFlags>,
        autoplay_model: Rc<RefCell<emAutoplayViewModel>>,
        bt_fullscreen: Option<Rc<RefCell<emCheckButton>>>,
    ) -> Self {
        Self {
            ctx,
            look,
            layout: emLinearLayout {
                orientation: Orientation::Adaptive {
                    tallness_threshold: 1.0,
                },
                spacing: Spacing {
                    inner_h: 0.07,
                    inner_v: 0.07,
                    ..Spacing::default()
                },
                ..emLinearLayout::horizontal()
            },
            click_flags,
            autoplay_model,
            bt_fullscreen,
            general_panel: None,
            bookmarks_panel: None,
            children_created: false,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        // Child 0: general (lAbtCfgCmd) — weight 4.71
        let general = Box::new(GeneralPanel::new(
            Rc::clone(&self.ctx),
            Rc::clone(&self.look),
            Rc::clone(&self.click_flags),
            Rc::clone(&self.autoplay_model),
            self.bt_fullscreen.clone(),
        ));
        let general_id = ctx.create_child_with("general", general);
        self.general_panel = Some(general_id);

        // Child 1: bookmarks — weight 6.5
        let bookmarks = Box::new(emBookmarksPanel::new(Rc::clone(&self.ctx)));
        let bm_id = ctx.create_child_with("bookmarks", bookmarks);
        self.bookmarks_panel = Some(bm_id);

        // C++ lMain: SetChildWeight(0, 4.71) SetChildWeight(1, 6.5)
        self.layout.set_child_constraint(
            general_id,
            ChildConstraint {
                weight: 4.71,
                ..Default::default()
            },
        );
        self.layout.set_child_constraint(
            bm_id,
            ChildConstraint {
                weight: 6.5,
                ..Default::default()
            },
        );

        self.children_created = true;
    }
}

impl PanelBehavior for LMainPanel {
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.create_children(ctx);
        }
        let cc = ctx.GetCanvasColor();
        ctx.set_all_children_canvas_color(cc);
        self.layout.do_layout_skip(ctx, None, None);
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

// ── GeneralPanel ─────────────────────────────────────────────────────────────
// Matches C++ lMain's lAbtCfgCmd child. Contains "About", config, and commands.
// Layout: adaptive threshold 0.8, child 0 (lAbtCfg weight 1.5),
//         child 1 (grCommands weight 3.05).

struct GeneralPanel {
    ctx: Rc<emContext>,
    look: Rc<emLook>,
    layout: emLinearLayout,
    click_flags: Rc<ClickFlags>,
    autoplay_model: Rc<RefCell<emAutoplayViewModel>>,
    /// Shared fullscreen button handle threaded from emMainControlPanel.
    /// None in layout-only test contexts.
    bt_fullscreen: Option<Rc<RefCell<emCheckButton>>>,
    about_cfg_panel: Option<PanelId>,
    commands_panel: Option<PanelId>,
    children_created: bool,
}

impl GeneralPanel {
    fn new(
        ctx: Rc<emContext>,
        look: Rc<emLook>,
        click_flags: Rc<ClickFlags>,
        autoplay_model: Rc<RefCell<emAutoplayViewModel>>,
        bt_fullscreen: Option<Rc<RefCell<emCheckButton>>>,
    ) -> Self {
        Self {
            ctx,
            look,
            layout: emLinearLayout {
                orientation: Orientation::Adaptive {
                    tallness_threshold: 0.8,
                },
                spacing: Spacing {
                    inner_h: 0.07,
                    inner_v: 0.07,
                    ..Spacing::default()
                },
                ..emLinearLayout::horizontal()
            },
            click_flags,
            autoplay_model,
            bt_fullscreen,
            about_cfg_panel: None,
            commands_panel: None,
            children_created: false,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        // Child 0: About + CoreConfig (lAbtCfg)
        let about_cfg = Box::new(AboutCfgPanel::new(Rc::clone(&self.ctx)));
        let about_cfg_id = ctx.create_child_with("t", about_cfg);
        self.about_cfg_panel = Some(about_cfg_id);

        // Child 1: Main Commands (grCommands)
        let commands = Box::new(CommandsPanel::new(
            Rc::clone(&self.look),
            Rc::clone(&self.click_flags),
            Rc::clone(&self.autoplay_model),
            self.bt_fullscreen.clone(),
        ));
        let commands_id = ctx.create_child_with("commands", commands);
        self.commands_panel = Some(commands_id);

        // C++ lAbtCfgCmd: SetChildWeight(0, 1.5) SetChildWeight(1, 3.05)
        self.layout.set_child_constraint(
            about_cfg_id,
            ChildConstraint {
                weight: 1.5,
                ..Default::default()
            },
        );
        self.layout.set_child_constraint(
            commands_id,
            ChildConstraint {
                weight: 3.05,
                ..Default::default()
            },
        );

        self.children_created = true;
    }
}

impl PanelBehavior for GeneralPanel {
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.create_children(ctx);
        }
        let cc = ctx.GetCanvasColor();
        ctx.set_all_children_canvas_color(cc);
        self.layout.do_layout_skip(ctx, None, None);
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

// ── AboutCfgPanel ────────────────────────────────────────────────────────────
// C++ lAbtCfg: about + core config. Adaptive layout, threshold 0.5.

struct AboutCfgPanel {
    _ctx: Rc<emContext>,
    layout: emLinearLayout,
    children_created: bool,
}

impl AboutCfgPanel {
    fn new(ctx: Rc<emContext>) -> Self {
        Self {
            _ctx: ctx,
            layout: emLinearLayout {
                orientation: Orientation::Adaptive {
                    tallness_threshold: 0.5,
                },
                spacing: Spacing {
                    inner_h: 0.16,
                    inner_v: 0.16,
                    ..Spacing::default()
                },
                ..emLinearLayout::horizontal()
            },
            children_created: false,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        // Child 0: About panel (placeholder label).
        let about = Box::new(AboutPanel);
        let about_id = ctx.create_child_with("about", about);

        // Child 1: Core config panel (placeholder).
        let cfg = Box::new(CoreConfigPlaceholder);
        let cfg_id = ctx.create_child_with("core config", cfg);

        // C++ lAbtCfg: SetChildWeight(0, 1.15) SetChildWeight(1, 1.85)
        self.layout.set_child_constraint(
            about_id,
            ChildConstraint {
                weight: 1.15,
                ..Default::default()
            },
        );
        self.layout.set_child_constraint(
            cfg_id,
            ChildConstraint {
                weight: 1.85,
                ..Default::default()
            },
        );

        self.children_created = true;
    }
}

impl PanelBehavior for AboutCfgPanel {
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.create_children(ctx);
        }
        let cc = ctx.GetCanvasColor();
        ctx.set_all_children_canvas_color(cc);
        self.layout.do_layout_skip(ctx, None, None);
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

// ── AboutPanel ───────────────────────────────────────────────────────────────
// Placeholder for "About Eagle Mode" linear group with icon + description.

struct AboutPanel;

impl PanelBehavior for AboutPanel {
    fn get_title(&self) -> Option<String> {
        Some("About Eagle Mode".to_string())
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        let bg = emColor::from_packed(0x515E84FF);
        let fg = emColor::from_packed(0xEFF0F4FF);
        let canvas = emColor::TRANSPARENT;
        painter.PaintRect(0.0, 0.0, w, h, bg, canvas);

        let about_text = concat!(
            "This is Eagle Mode (Rust port)\n",
            "\n",
            "Copyright (C) 2001-2026 Oliver Hamann.\n",
            "\n",
            "Homepage: http://eaglemode.sourceforge.net/\n",
            "\n",
            "This program is free software: you can redistribute it and/or modify it under\n",
            "the terms of the GNU General Public License version 3 as published by the\n",
            "Free Software Foundation.\n",
        );

        let font_h = (h * 0.08).max(0.01);
        let text_y = h * 0.1;
        painter.PaintText(w * 0.05, text_y, about_text, font_h, 1.0, fg, canvas);
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

// ── CoreConfigPlaceholder ────────────────────────────────────────────────────
// Placeholder for emCoreConfigPanel.
// BLOCKED: C++ creates a full emCoreConfigPanel here. Rust defers to a
// placeholder until the core config panel is wired into emmain's panel tree.

struct CoreConfigPlaceholder;

impl PanelBehavior for CoreConfigPlaceholder {
    fn get_title(&self) -> Option<String> {
        Some("Core Config".to_string())
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        let bg = emColor::from_packed(0x515E84FF);
        let fg = emColor::from_packed(0xEFF0F4FF);
        let canvas = emColor::TRANSPARENT;
        painter.PaintRect(0.0, 0.0, w, h, bg, canvas);
        let font_h = (h * 0.12).max(0.01);
        painter.PaintText(
            w * 0.05,
            h * 0.3,
            "Core Configuration",
            font_h,
            1.0,
            fg,
            canvas,
        );
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

// ── CommandsPanel ────────────────────────────────────────────────────────────
// Port of C++ grCommands = new emPackGroup(lAbtCfgCmd, "commands", "Main Commands")
// Contains: New Window, Fullscreen, Reload, Autoplay, Close/Quit.

struct CommandsPanel {
    look: Rc<emLook>,
    border: emBorder,
    layout: emLinearLayout,
    click_flags: Rc<ClickFlags>,
    autoplay_model: Rc<RefCell<emAutoplayViewModel>>,
    /// Shared fullscreen button handle from emMainControlPanel.
    /// Rc<RefCell<>> justification (b): context-registry-style shared widget
    /// handle per CLAUDE.md §Ownership. Placed in MainCheckButtonPanel for
    /// paint/input dispatch; emMainControlPanel::Cycle reads/writes check state.
    /// None in layout-only test contexts (no scheduler at create_children time).
    bt_fullscreen: Option<Rc<RefCell<emCheckButton>>>,
    children_created: bool,
}

impl CommandsPanel {
    fn new(
        look: Rc<emLook>,
        click_flags: Rc<ClickFlags>,
        autoplay_model: Rc<RefCell<emAutoplayViewModel>>,
        bt_fullscreen: Option<Rc<RefCell<emCheckButton>>>,
    ) -> Self {
        Self {
            look,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Main Commands"),
            // DIVERGED: (language-forced) C++ uses emPackGroup with PrefChildTallness(0.7).
            // Rust uses emLinearLayout vertical since emPackLayout doesn't
            // support tallness preferences in the same way.
            layout: emLinearLayout::vertical(),
            click_flags,
            autoplay_model,
            bt_fullscreen,
            children_created: false,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::clone(&self.look);
        let flags = Rc::clone(&self.click_flags);

        // ── BtNewWindow ──
        let flag = Rc::clone(&flags);
        let mut btn_nw = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "New Window", Rc::clone(&look))
        };
        btn_nw.SetDescription("Create a new window showing the same location.\n\nHotkey: F4");
        btn_nw.on_click = Some(Box::new(
            move |(), _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                flag.new_window.set(true);
            },
        ));
        let nw_id =
            ctx.create_child_with("new window", Box::new(MainButtonPanel { button: btn_nw }));

        // ── BtFullscreen ──
        // Use the shared Rc<RefCell<emCheckButton>> threaded from emMainControlPanel.
        // The on_check callback sets click_flags.fullscreen; the D-006 row-218
        // reaction in emMainControlPanel::Cycle sets check state from window flags.
        // If no button was allocated (layout-only test context), fall back to
        // creating a standalone button for the panel tree only (click-through wire
        // is not available in that context; row-218 reaction requires a scheduler).
        let bt_fs = if let Some(ref bt) = self.bt_fullscreen {
            Rc::clone(bt)
        } else {
            let mut sched = ctx.as_sched_ctx().expect("sched for fallback BtFullscreen");
            Rc::new(RefCell::new(emCheckButton::new(
                &mut sched,
                "Fullscreen",
                Rc::clone(&look),
            )))
        };
        {
            let flag = Rc::clone(&flags);
            bt_fs.borrow_mut().on_check = Some(Box::new(
                move |_checked, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                    flag.fullscreen.set(true);
                },
            ));
        }
        let fs_id = ctx.create_child_with(
            "fullscreen",
            Box::new(MainCheckButtonPanel {
                check_button: bt_fs,
            }),
        );

        // ── BtReload ──
        let flag = Rc::clone(&flags);
        let mut btn_reload = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Reload Files", Rc::clone(&look))
        };
        btn_reload.SetDescription(
            "Reload files and directories which are currently shown by this program.\n\nHotkey: F5",
        );
        btn_reload.on_click = Some(Box::new(
            move |(), _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                flag.reload.set(true);
            },
        ));
        let reload_id =
            ctx.create_child_with("reload", Box::new(MainButtonPanel { button: btn_reload }));

        // ── Autoplay control panel ──
        let autoplay = Box::new(emAutoplayControlPanel::new(
            Rc::clone(&look),
            Rc::clone(&self.autoplay_model),
        ));
        let autoplay_id = ctx.create_child_with("autoplay", autoplay);

        // ── Close / Quit (lCloseQuit) ──
        let flag_close = Rc::clone(&flags);
        let mut btn_close = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Close", Rc::clone(&look))
        };
        btn_close.SetDescription("Close this window.\n\nHotkey: Alt+F4");
        btn_close.on_click = Some(Box::new(
            move |(), _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                flag_close.close.set(true);
            },
        ));
        let close_id =
            ctx.create_child_with("close", Box::new(MainButtonPanel { button: btn_close }));

        let flag_quit = Rc::clone(&flags);
        let mut btn_quit = {
            let mut sched = ctx.as_sched_ctx().expect("sched");
            emButton::new(&mut sched, "Quit", Rc::clone(&look))
        };
        btn_quit.SetDescription(
            "Close all windows of this process (and terminate this process).\n\nHotkey: Shift+Alt+F4",
        );
        btn_quit.on_click = Some(Box::new(
            move |(), _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
                flag_quit.quit.set(true);
            },
        ));
        let quit_id = ctx.create_child_with("quit", Box::new(MainButtonPanel { button: btn_quit }));

        // C++ grCommands child weights:
        //   0: new window (1.0), 1: fullscreen (1.09), 2: reload (1.0),
        //   3: autoplay (2.09), 4: close_quit (1.0)
        // Close and Quit are in a sub-layout in C++ (lCloseQuit), but here
        // we flatten them into the main commands layout with adjusted weights.
        // C++ close_quit weight 1.0 split between close (1.0) and quit (0.8).
        let total_cq = 1.0 + 0.8;
        self.layout.set_child_constraint(
            nw_id,
            ChildConstraint {
                weight: 1.0,
                ..Default::default()
            },
        );
        self.layout.set_child_constraint(
            fs_id,
            ChildConstraint {
                weight: 1.09,
                ..Default::default()
            },
        );
        self.layout.set_child_constraint(
            reload_id,
            ChildConstraint {
                weight: 1.0,
                ..Default::default()
            },
        );
        self.layout.set_child_constraint(
            autoplay_id,
            ChildConstraint {
                weight: 2.09,
                ..Default::default()
            },
        );
        self.layout.set_child_constraint(
            close_id,
            ChildConstraint {
                weight: 1.0 / total_cq,
                ..Default::default()
            },
        );
        self.layout.set_child_constraint(
            quit_id,
            ChildConstraint {
                weight: 0.8 / total_cq,
                ..Default::default()
            },
        );

        self.children_created = true;
    }
}

impl PanelBehavior for CommandsPanel {
    fn get_title(&self) -> Option<String> {
        Some("Main Commands".to_string())
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            false,
            state.enabled,
            pixel_scale,
        );
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.create_children(ctx);
        }
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRect(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, None, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_panel_new() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainControlPanel::new(Rc::clone(&ctx), None);
        assert_eq!(panel.get_title(), Some("emMainControl".to_string()));
    }

    #[test]
    fn test_control_panel_opaque() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainControlPanel::new(Rc::clone(&ctx), None);
        assert!(panel.IsOpaque());
    }

    #[test]
    fn test_control_panel_behavior() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainControlPanel::new(Rc::clone(&ctx), None);
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn test_click_flags_default() {
        let flags = ClickFlags::default();
        assert!(!flags.new_window.get());
        assert!(!flags.fullscreen.get());
        assert!(!flags.reload.get());
        assert!(!flags.close.get());
        assert!(!flags.quit.get());
    }

    #[test]
    fn test_click_flag_roundtrip() {
        let flags = Rc::new(ClickFlags::default());
        flags.close.set(true);
        assert!(flags.close.take());
        assert!(!flags.close.get());
    }

    #[test]
    fn test_title_matches_cpp() {
        // C++ GetTitle returns "emMainControl"
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainControlPanel::new(ctx, None);
        assert_eq!(panel.get_title(), Some("emMainControl".to_string()));
    }

    // ── B-006 click-through tests ─────────────────────────────────────────
    // Verify D-006 row-218 and row-219 signal wiring end-to-end.
    //
    // Lives here (not in typed_subscribe_b006.rs) because the test requires
    // access to private types (the emMainWindow thread-local) and uses the
    // `unsafe` scheduler-alias pattern common to B-003's click-through test.
    // Mirrors the B-003 precedent for click-through tests in
    // emAutoplayControlPanel::tests::bt_autoplay_check_drives_set_autoplaying.

    /// B-006 §Row 218 click-through.
    ///
    /// Fires the window-flags signal and verifies that Cycle sets
    /// `bt_fullscreen.IsChecked()` to match the FULLSCREEN flag.
    /// Mirrors C++ Cycle 253-255:
    ///   if (IsSignaled(MainWin.GetWindowFlagsSignal()))
    ///     BtFullscreen->SetChecked((MainWin.GetWindowFlags()&WF_FULLSCREEN)!=0)
    #[test]
    fn row_218_flags_signal_sets_bt_fullscreen() {
        use emcore::emEngineCtx::{EngineCtx, PanelCtx};
        use emcore::emScheduler::EngineScheduler;
        use emcore::emWindow::{WindowFlags, emWindow};
        use std::collections::HashMap;

        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut sched = EngineScheduler::new();

        // ── Allocate signals for the fake emWindow ──
        let close_sig = sched.create_signal();
        let flags_sig = sched.create_signal();
        let focus_sig = sched.create_signal();
        let geom_sig = sched.create_signal();
        let win_id = winit::window::WindowId::dummy();

        // Build a fake emWindow with FULLSCREEN flag set. The flags_signal
        // matches what the Cycle init block will look up via GetWindowFlagsSignal.
        let mut win = emWindow::new_popup_pending(
            Rc::clone(&root_ctx),
            WindowFlags::empty(),
            "test_fs".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            emcore::emColor::emColor::TRANSPARENT,
        );
        win.flags = WindowFlags::FULLSCREEN;
        let mut windows: HashMap<winit::window::WindowId, emWindow> = HashMap::new();
        windows.insert(win_id, win);

        // Register emMainWindow so with_main_window succeeds.
        let mut mw = crate::emMainWindow::emMainWindow::new(
            Rc::clone(&root_ctx),
            crate::emMainWindow::emMainWindowConfig::default(),
        );
        mw.window_id = Some(win_id);
        crate::emMainWindow::set_main_window(mw);

        // Build panel and tree.
        let mut panel = emMainControlPanel::new(Rc::clone(&root_ctx), None);
        let mut tree = emcore::emPanelTree::PanelTree::new();
        let root_id = tree.create_root_deferred_view("cp_218");
        tree.set_panel_view(root_id);
        tree.register_engine_for_public(root_id, Some(&mut sched));
        let engine_id = tree.panel_engine_id_pub(root_id).expect("engine");

        let fw_cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let pa: Rc<std::cell::RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();

        // Run create_children so bt_fullscreen is allocated.
        {
            let mut pctx = PanelCtx::with_sched_reach(
                &mut tree,
                root_id,
                1.0,
                &mut sched,
                &mut fw_actions,
                &root_ctx,
                &fw_cb,
                &pa,
            );
            panel.create_children(&mut pctx);
        }
        assert!(panel.bt_fullscreen.is_some(), "bt_fullscreen allocated");

        // Connect engine to flags_signal and fire it. The subscribed_init=false
        // means the Cycle init block will re-connect (idempotent).
        sched.connect(flags_sig, engine_id);
        sched.fire(flags_sig);
        sched.flush_signals_for_test();

        // Drive Cycle manually with a hand-built EngineCtx + PanelCtx.
        let mut pending_inputs: Vec<(winit::window::WindowId, emcore::emInput::emInputEvent)> =
            Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();

        // SAFETY: ectx.scheduler and pctx.scheduler alias the same EngineScheduler.
        // Single-threaded; mirrors PanelCycleEngine's identical unsafe split.
        let sched_ptr: *mut EngineScheduler = &mut sched;
        let mut pctx =
            PanelCtx::with_scheduler(&mut tree, root_id, 1.0, unsafe { &mut *sched_ptr });
        let mut ectx = EngineCtx {
            scheduler: &mut sched,
            tree: None,
            windows: &mut windows,
            root_context: &root_ctx,
            framework_actions: &mut fw_actions,
            pending_inputs: &mut pending_inputs,
            input_state: &mut input_state,
            framework_clipboard: &fw_cb,
            engine_id,
            pending_actions: &pa,
        };
        panel.Cycle(&mut ectx, &mut pctx);

        assert!(
            panel
                .bt_fullscreen
                .as_ref()
                .expect("bt_fullscreen present")
                .borrow()
                .IsChecked(),
            "bt_fullscreen must be checked when flags_signal fires with FULLSCREEN set"
        );

        // Cleanup: remove all engines and signals so EngineScheduler drops cleanly.
        let all_ids = tree.panel_ids();
        for pid in all_ids {
            if let Some(eid) = tree.panel_engine_id_pub(pid) {
                sched.remove_engine(eid);
            }
        }
        sched.disconnect(flags_sig, engine_id);
        sched.remove_signal(close_sig);
        sched.remove_signal(flags_sig);
        sched.remove_signal(focus_sig);
        sched.remove_signal(geom_sig);
        sched.abort_all_pending();
    }

    /// B-006 §Row 219 click-through.
    ///
    /// Fires the config-change signal and verifies that Cycle sets
    /// `bt_auto_hide_control_view` and `bt_auto_hide_slider` check states.
    /// Mirrors C++ Cycle 257-260:
    ///   if (IsSignaled(MainConfig->GetChangeSignal()))
    ///     BtAutoHideControlView->SetChecked(MainConfig->AutoHideControlView);
    ///     BtAutoHideSlider->SetChecked(MainConfig->AutoHideSlider);
    #[test]
    fn row_219_config_signal_sets_auto_hide_buttons() {
        use emcore::emEngineCtx::{EngineCtx, PanelCtx};
        use emcore::emScheduler::EngineScheduler;
        use std::collections::HashMap;

        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut sched = EngineScheduler::new();

        // ── Allocate a real change signal for the config singleton ──
        // emMainConfig::Acquire creates the config with SignalId::null() because
        // no scheduler is available at registration time. Override it with a
        // real signal so the Cycle's IsSignaled check works.
        let cfg_change_sig = sched.create_signal();
        let win_id = winit::window::WindowId::dummy();

        // Register emMainWindow (row-218 subscribe path also runs; provide a
        // real flags_signal so the init block can connect it too).
        let flags_sig = sched.create_signal();
        let close_sig = sched.create_signal();
        let focus_sig = sched.create_signal();
        let geom_sig = sched.create_signal();
        let win = emcore::emWindow::emWindow::new_popup_pending(
            Rc::clone(&root_ctx),
            emcore::emWindow::WindowFlags::empty(),
            "test_cfg".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            emcore::emColor::emColor::TRANSPARENT,
        );
        let mut windows: HashMap<winit::window::WindowId, emcore::emWindow::emWindow> =
            HashMap::new();
        windows.insert(win_id, win);

        let mut mw = crate::emMainWindow::emMainWindow::new(
            Rc::clone(&root_ctx),
            crate::emMainWindow::emMainWindowConfig::default(),
        );
        mw.window_id = Some(win_id);
        crate::emMainWindow::set_main_window(mw);

        // Build panel with scheduler-backed config signal.
        let mut panel = emMainControlPanel::new(Rc::clone(&root_ctx), None);

        // Override the config's null signal with the real test signal.
        panel
            .config
            .borrow_mut()
            .set_change_signal_for_test(cfg_change_sig);

        // Mutate config so the reaction reads the updated values.
        panel.config.borrow_mut().SetAutoHideControlView(true);
        panel.config.borrow_mut().SetAutoHideSlider(true);

        // Build panel tree and engine.
        let mut tree = emcore::emPanelTree::PanelTree::new();
        let root_id = tree.create_root_deferred_view("cp_219");
        tree.set_panel_view(root_id);
        tree.register_engine_for_public(root_id, Some(&mut sched));
        let engine_id = tree.panel_engine_id_pub(root_id).expect("engine");

        let fw_cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let pa: Rc<std::cell::RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();

        {
            let mut pctx = PanelCtx::with_sched_reach(
                &mut tree,
                root_id,
                1.0,
                &mut sched,
                &mut fw_actions,
                &root_ctx,
                &fw_cb,
                &pa,
            );
            panel.create_children(&mut pctx);
        }
        assert!(
            panel.bt_auto_hide_control_view.is_some(),
            "bt_auto_hide_control_view allocated"
        );
        assert!(
            panel.bt_auto_hide_slider.is_some(),
            "bt_auto_hide_slider allocated"
        );

        // Connect engine and fire the config change signal.
        sched.connect(cfg_change_sig, engine_id);
        sched.fire(cfg_change_sig);
        sched.flush_signals_for_test();

        let mut pending_inputs: Vec<(winit::window::WindowId, emcore::emInput::emInputEvent)> =
            Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();

        let sched_ptr: *mut EngineScheduler = &mut sched;
        let mut pctx =
            PanelCtx::with_scheduler(&mut tree, root_id, 1.0, unsafe { &mut *sched_ptr });
        let mut ectx = EngineCtx {
            scheduler: &mut sched,
            tree: None,
            windows: &mut windows,
            root_context: &root_ctx,
            framework_actions: &mut fw_actions,
            pending_inputs: &mut pending_inputs,
            input_state: &mut input_state,
            framework_clipboard: &fw_cb,
            engine_id,
            pending_actions: &pa,
        };
        // subscribed_init=true: skip the init block (we connected manually above).
        panel.subscribed_init = true;
        panel.Cycle(&mut ectx, &mut pctx);

        assert!(
            panel
                .bt_auto_hide_control_view
                .as_ref()
                .expect("present")
                .borrow()
                .IsChecked(),
            "bt_auto_hide_control_view must be checked when cfg fires with AutoHideControlView=true"
        );
        assert!(
            panel
                .bt_auto_hide_slider
                .as_ref()
                .expect("present")
                .borrow()
                .IsChecked(),
            "bt_auto_hide_slider must be checked when cfg fires with AutoHideSlider=true"
        );

        // Cleanup.
        let all_ids = tree.panel_ids();
        for pid in all_ids {
            if let Some(eid) = tree.panel_engine_id_pub(pid) {
                sched.remove_engine(eid);
            }
        }
        sched.disconnect(cfg_change_sig, engine_id);
        sched.remove_signal(cfg_change_sig);
        sched.remove_signal(close_sig);
        sched.remove_signal(flags_sig);
        sched.remove_signal(focus_sig);
        sched.remove_signal(geom_sig);
        sched.abort_all_pending();
    }
}
