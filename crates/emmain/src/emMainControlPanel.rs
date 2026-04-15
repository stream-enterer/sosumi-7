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

use emcore::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use emcore::emButton::emButton;
use emcore::emCheckButton::emCheckButton;
use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emCursor::emCursor;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLinearLayout::emLinearLayout;
use emcore::emLook::emLook;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPainter::emPainter;
use emcore::emPanelCtx::PanelCtx;
use emcore::emPanelTree::PanelId;
use emcore::emTiling::{ChildConstraint, Orientation, Spacing};

use crate::emAutoplayControlPanel::{emAutoplayControlPanel, AutoplayFlags};
use crate::emBookmarks::emBookmarksPanel;
use crate::emMainConfig::emMainConfig;

// ── Click flags ──────────────────────────────────────────────────────────────
// Shared state between button on_click callbacks and the Cycle method.
// DIVERGED: C++ uses AddWakeUpSignal / IsSignaled. Rust uses Rc<Cell<bool>>
// flags set by on_click callbacks and polled in Cycle.

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
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale =
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.button.Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        self.button.Input(event, state, input_state)
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

struct MainCheckButtonPanel {
    check_button: emCheckButton,
}

impl PanelBehavior for MainCheckButtonPanel {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale =
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.check_button
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        self.check_button.Input(event, state, input_state)
    }

    fn GetCursor(&self) -> emCursor {
        self.check_button.GetCursor()
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
    _config: Rc<RefCell<emMainConfig>>,
    border: emBorder,
    look: emLook,
    /// Top-level linear layout: 2 children (lMain, contentControlPanel).
    /// C++ SetChildWeight(0, 11.37) SetChildWeight(1, 21.32).
    layout_main: emLinearLayout,
    click_flags: Rc<ClickFlags>,
    autoplay_flags: Rc<AutoplayFlags>,
    // Panel IDs for child widgets (used for layout weight assignment).
    lmain_panel: Option<PanelId>,
    _content_control_panel: Option<PanelId>,
    /// PanelId of the content sub-view, used for wiring content control panel.
    _content_view_id: Option<PanelId>,
    children_created: bool,
}

impl emMainControlPanel {
    /// Port of C++ `emMainControlPanel` constructor.
    ///
    /// `content_view_id` is the PanelId of the content sub-view panel, used
    /// for wiring the content control panel (C++ contentControlPanel).
    pub fn new(ctx: Rc<emContext>, content_view_id: Option<PanelId>) -> Self {
        let config = emMainConfig::Acquire(&ctx);

        // C++ emMainControlPanel constructor:
        //   SetOuterBorderType(OBT_POPUP_ROOT)
        //   SetInnerBorderType(IBT_NONE)
        let border = emBorder::new(OuterBorderType::PopupRoot)
            .with_inner(InnerBorderType::None);

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

        Self {
            ctx,
            _config: config,
            border,
            look: emLook::default(),
            layout_main,
            click_flags: Rc::new(ClickFlags::default()),
            autoplay_flags: Rc::new(AutoplayFlags::default()),
            lmain_panel: None,
            _content_control_panel: None,
            _content_view_id: content_view_id,
            children_created: false,
        }
    }

    /// Create the full child widget tree matching C++ constructor.
    ///
    /// C++ top-level layout has 2 children:
    ///   child 0: lMain (weight 11.37) — contains general + bookmarks
    ///   child 1: contentControlPanel (weight 21.32) — placeholder for now
    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::new(self.look.clone());
        let flags = Rc::clone(&self.click_flags);

        // ── lMain: wraps general + bookmarks (child 0 of top-level) ──────
        let lmain = Box::new(LMainPanel::new(
            Rc::clone(&self.ctx),
            Rc::clone(&look),
            Rc::clone(&flags),
            Rc::clone(&self.autoplay_flags),
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

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale =
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border
            .paint_border(painter, w, h, &self.look, false, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
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

    fn Cycle(&mut self, _ctx: &mut PanelCtx) -> bool {
        // Poll click flags and dispatch to main window.
        // Port of C++ Cycle() signal handling.
        let flags = &self.click_flags;

        if flags.new_window.take() {
            // C++ MainWin.Duplicate() — not yet implemented, log it.
            log::info!("emMainControlPanel: New Window requested (Duplicate not yet implemented)");
        }

        if flags.fullscreen.take() {
            crate::emMainWindow::with_main_window(|mw| {
                // DIVERGED: C++ has direct MainWin reference; Rust uses
                // thread_local. ToggleFullscreen requires &mut App which
                // we don't have in Cycle. Log for now.
                log::info!(
                    "emMainControlPanel: Fullscreen toggle requested (requires App access)"
                );
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
            // DIVERGED: C++ calls MainWin.ReloadFiles() directly via signal.
            // Rust sets a flag on emMainWindow, polled by MainWindowEngine which
            // has EngineCtx access to fire the file_update_signal.
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
        let cc = self
            .border
            .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
}

// ── LMainPanel ──────────────────────────────────────────────────────────────
// C++ lMain: linear layout containing general (lAbtCfgCmd, weight 4.71) and
// bookmarks (weight 6.5).

struct LMainPanel {
    ctx: Rc<emContext>,
    look: Rc<emLook>,
    layout: emLinearLayout,
    click_flags: Rc<ClickFlags>,
    autoplay_flags: Rc<AutoplayFlags>,
    general_panel: Option<PanelId>,
    bookmarks_panel: Option<PanelId>,
    children_created: bool,
}

impl LMainPanel {
    fn new(
        ctx: Rc<emContext>,
        look: Rc<emLook>,
        click_flags: Rc<ClickFlags>,
        autoplay_flags: Rc<AutoplayFlags>,
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
            autoplay_flags,
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
            Rc::clone(&self.autoplay_flags),
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

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
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
    autoplay_flags: Rc<AutoplayFlags>,
    about_cfg_panel: Option<PanelId>,
    commands_panel: Option<PanelId>,
    children_created: bool,
}

impl GeneralPanel {
    fn new(
        ctx: Rc<emContext>,
        look: Rc<emLook>,
        click_flags: Rc<ClickFlags>,
        autoplay_flags: Rc<AutoplayFlags>,
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
            autoplay_flags,
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
            Rc::clone(&self.autoplay_flags),
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

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
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

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
}

// ── AboutPanel ───────────────────────────────────────────────────────────────
// Placeholder for "About Eagle Mode" linear group with icon + description.

struct AboutPanel;

impl PanelBehavior for AboutPanel {
    fn get_title(&self) -> Option<String> {
        Some("About Eagle Mode".to_string())
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
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

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
}

// ── CoreConfigPlaceholder ────────────────────────────────────────────────────
// Placeholder for emCoreConfigPanel.
// DIVERGED: C++ creates a full emCoreConfigPanel here. Rust defers to a
// placeholder until the core config panel is wired into emmain's panel tree.

struct CoreConfigPlaceholder;

impl PanelBehavior for CoreConfigPlaceholder {
    fn get_title(&self) -> Option<String> {
        Some("Core Config".to_string())
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        let bg = emColor::from_packed(0x515E84FF);
        let fg = emColor::from_packed(0xEFF0F4FF);
        let canvas = emColor::TRANSPARENT;
        painter.PaintRect(0.0, 0.0, w, h, bg, canvas);
        let font_h = (h * 0.12).max(0.01);
        painter.PaintText(w * 0.05, h * 0.3, "Core Configuration", font_h, 1.0, fg, canvas);
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
}

// ── CommandsPanel ────────────────────────────────────────────────────────────
// Port of C++ grCommands = new emPackGroup(lAbtCfgCmd, "commands", "Main Commands")
// Contains: New Window, Fullscreen, Reload, Autoplay, Close/Quit.

struct CommandsPanel {
    look: Rc<emLook>,
    border: emBorder,
    layout: emLinearLayout,
    click_flags: Rc<ClickFlags>,
    autoplay_flags: Rc<AutoplayFlags>,
    children_created: bool,
}

impl CommandsPanel {
    fn new(
        look: Rc<emLook>,
        click_flags: Rc<ClickFlags>,
        autoplay_flags: Rc<AutoplayFlags>,
    ) -> Self {
        Self {
            look,
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption("Main Commands"),
            // DIVERGED: C++ uses emPackGroup with PrefChildTallness(0.7).
            // Rust uses emLinearLayout vertical since emPackLayout doesn't
            // support tallness preferences in the same way.
            layout: emLinearLayout::vertical(),
            click_flags,
            autoplay_flags,
            children_created: false,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        let look = Rc::clone(&self.look);
        let flags = Rc::clone(&self.click_flags);

        // ── BtNewWindow ──
        let flag = Rc::clone(&flags);
        let mut btn_nw = emButton::new("New Window", Rc::clone(&look));
        btn_nw.SetDescription(
            "Create a new window showing the same location.\n\nHotkey: F4",
        );
        btn_nw.on_click = Some(Box::new(move || {
            flag.new_window.set(true);
        }));
        let nw_id = ctx.create_child_with(
            "new window",
            Box::new(MainButtonPanel { button: btn_nw }),
        );

        // ── BtFullscreen ──
        let flag = Rc::clone(&flags);
        let mut btn_fs = emCheckButton::new("Fullscreen", Rc::clone(&look));
        btn_fs.on_check = Some(Box::new(move |_checked| {
            flag.fullscreen.set(true);
        }));
        let fs_id = ctx.create_child_with(
            "fullscreen",
            Box::new(MainCheckButtonPanel {
                check_button: btn_fs,
            }),
        );

        // ── BtReload ──
        let flag = Rc::clone(&flags);
        let mut btn_reload = emButton::new("Reload Files", Rc::clone(&look));
        btn_reload.SetDescription(
            "Reload files and directories which are currently shown by this program.\n\nHotkey: F5",
        );
        btn_reload.on_click = Some(Box::new(move || {
            flag.reload.set(true);
        }));
        let reload_id = ctx.create_child_with(
            "reload",
            Box::new(MainButtonPanel {
                button: btn_reload,
            }),
        );

        // ── Autoplay control panel ──
        let autoplay = Box::new(emAutoplayControlPanel::new(
            Rc::clone(&look),
            Rc::clone(&self.autoplay_flags),
        ));
        let autoplay_id = ctx.create_child_with("autoplay", autoplay);

        // ── Close / Quit (lCloseQuit) ──
        let flag_close = Rc::clone(&flags);
        let mut btn_close = emButton::new("Close", Rc::clone(&look));
        btn_close.SetDescription("Close this window.\n\nHotkey: Alt+F4");
        btn_close.on_click = Some(Box::new(move || {
            flag_close.close.set(true);
        }));
        let close_id = ctx.create_child_with(
            "close",
            Box::new(MainButtonPanel {
                button: btn_close,
            }),
        );

        let flag_quit = Rc::clone(&flags);
        let mut btn_quit = emButton::new("Quit", Rc::clone(&look));
        btn_quit.SetDescription(
            "Close all windows of this process (and terminate this process).\n\nHotkey: Shift+Alt+F4",
        );
        btn_quit.on_click = Some(Box::new(move || {
            flag_quit.quit.set(true);
        }));
        let quit_id = ctx.create_child_with(
            "quit",
            Box::new(MainButtonPanel { button: btn_quit }),
        );

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

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale =
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border
            .paint_border(painter, w, h, &self.look, false, state.enabled, pixel_scale);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.create_children(ctx);
        }
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRect(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, None, Some(cr));
        let cc = self
            .border
            .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_panel_new() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainControlPanel::new(Rc::clone(&ctx), None);
        assert_eq!(
            panel.get_title(),
            Some("emMainControl".to_string())
        );
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
}
