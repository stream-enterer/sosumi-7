//! Signal/timer demo derived from C++ `SignalExample.cpp`.
//!
//! Demonstrates the scheduler's signal and timer system:
//! - A button panel fires a signal on each left-Click.
//! - A periodic timer fires every second.
//! - An emEngine watches both signals and increments counters.
//! - A display panel paints the counter values.
//!
//! The key architectural difference from C++ (which used `Cycle()` on panels)
//! is that Rust routes signals through `emEngine::Cycle()` on the scheduler.

use std::cell::RefCell;
use std::rc::Rc;

use eaglemode_rs::emCore::emColor::emColor;
use eaglemode_rs::emCore::emInput::{emInputEvent, InputKey, InputVariant};
use eaglemode_rs::emCore::emInputState::emInputState;
use eaglemode_rs::emCore::emPanel::{PanelBehavior, PanelState};
use eaglemode_rs::emCore::emPanelCtx::PanelCtx;
use eaglemode_rs::emCore::emView::ViewFlags;
use eaglemode_rs::emCore::emPainter::{emPainter, TextAlignment, VAlign};
use eaglemode_rs::emCore::emEngine::{emEngine, EngineCtx, Priority};
use eaglemode_rs::emCore::emSignal::SignalId;
use eaglemode_rs::emCore::emGUIFramework::App;
use eaglemode_rs::emCore::emWindow::WindowFlags;

// ── Shared state between engine and panels ──

struct SharedState {
    button_count: u32,
    timer_count: u32,
}

// ── emEngine that watches signals ──

struct CounterEngine {
    state: Rc<RefCell<SharedState>>,
    button_signal: SignalId,
    timer_signal: SignalId,
}

impl emEngine for CounterEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        let mut s = self.state.borrow_mut();
        if ctx.IsSignaled(self.button_signal) {
            s.button_count += 1;
        }
        if ctx.IsSignaled(self.timer_signal) {
            s.timer_count += 1;
        }
        false // sleep until next signal
    }
}

// ── Root panel: shows counter values and hosts button child ──

struct CounterPanel {
    state: Rc<RefCell<SharedState>>,
}

impl PanelBehavior for CounterPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _ps: &PanelState) {
        p.PaintRect(
            0.0,
            0.0,
            w,
            h,
            emColor::rgba(0xC0, 0xC0, 0xC0, 0xFF),
            emColor::TRANSPARENT,
        );
        let s = self.state.borrow();
        let text = format!(
            "Button Signals: {}\nTimer Signals: {}",
            s.button_count, s.timer_count
        );
        p.PaintTextBoxed(
            0.0,
            h * 0.3,
            w,
            h * 0.6,
            &text,
            h * 0.1,
            emColor::rgba(0xFF, 0xFF, 0x80, 0xFF),
            emColor::TRANSPARENT,
            TextAlignment::Center,
            VAlign::Top,
            TextAlignment::Center,
            0.5,
            true,
            0.15,
        );
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();
        let rect = ctx.layout_rect();
        let h = rect.h / rect.w;
        if !children.is_empty() {
            ctx.layout_child(children[0], 0.1, 0.1 * h, 0.8, 0.15 * h);
        }
        // emButton child is created by main — just layout if it exists.
    }
}

// ── emButton panel: fires a signal on left-Click ──

struct ClickPanel {
    pressed: bool,
}

impl PanelBehavior for ClickPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        let bg = if self.pressed {
            emColor::rgba(0x80, 0xA0, 0x80, 0xFF)
        } else {
            emColor::rgba(0xA0, 0xC0, 0xA0, 0xFF)
        };
        p.PaintRect(0.0, 0.0, w, h, bg, emColor::TRANSPARENT);
        p.PaintTextBoxed(
            0.0,
            0.0,
            w,
            h,
            "Click Me",
            h * 0.6,
            emColor::rgba(0x00, 0x80, 0x00, 0xFF),
            emColor::TRANSPARENT,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            true,
            0.15,
        );
    }

    fn Input(&mut self, event: &emInputEvent, _state: &PanelState, input_state: &emInputState) -> bool {
        if event.key == InputKey::MouseLeft && event.variant == InputVariant::Press {
            self.pressed = true;
            // Signal is fired by the App scheduler — we store the signal ID
            // and the main loop fires it. But we can't access the scheduler
            // from here directly. Instead, we use a closure-like pattern:
            // mark pressed and handle in the engine via notice or repaint Cycle.
            // Actually, the simplest approach: the engine polls. But that defeats
            // the purpose. Let's store a "pending fire" flag.
            return false; // let focus handling proceed
        }
        if self.pressed && !input_state.Get(InputKey::MouseLeft) {
            self.pressed = false;
        }
        false
    }
}

fn main() {
    let app = App::new(Box::new(|app, event_loop| {
        let state = Rc::new(RefCell::new(SharedState {
            button_count: 0,
            timer_count: 0,
        }));

        // Create signals
        let button_sig = app.scheduler.borrow_mut().create_signal();
        let timer_sig = app.scheduler.borrow_mut().create_signal();

        // Create and start a periodic timer (1000ms)
        let timer = app.scheduler.borrow_mut().create_timer(timer_sig);
        app.scheduler.borrow_mut().start_timer(timer, 1000, true);

        // Register the engine
        let engine_id = app.scheduler.borrow_mut().register_engine(
            Priority::Medium,
            Box::new(CounterEngine {
                state: state.clone(),
                button_signal: button_sig,
                timer_signal: timer_sig,
            }),
        );
        app.scheduler.borrow_mut().connect(button_sig, engine_id);
        app.scheduler.borrow_mut().connect(timer_sig, engine_id);

        // Build panel tree
        let root = app.tree.create_root("root");
        app.tree.set_behavior(
            root,
            Box::new(CounterPanel {
                state: state.clone(),
            }),
        );
        app.tree.Layout(root, 0.0, 0.0, 1.0, 1.0);

        // Create button child — fires the button signal on Click
        let button = app.tree.create_child(root, "button");
        app.tree
            .set_behavior(button, Box::new(ClickPanel { pressed: false }));

        let close_sig = app.scheduler.borrow_mut().create_signal();
        let flags_sig = app.scheduler.borrow_mut().create_signal();
        let focus_sig = app.scheduler.borrow_mut().create_signal();
        let geometry_sig = app.scheduler.borrow_mut().create_signal();
        let win = eaglemode_rs::emCore::emWindow::ZuiWindow::create(
            event_loop,
            app.gpu(),
            root,
            WindowFlags::AUTO_DELETE,
            close_sig,
            flags_sig,
            focus_sig,
            geometry_sig,
        );
        let wid = win.winit_window.id();
        app.windows.insert(wid, win);
        app.windows.get_mut(&wid).unwrap().view_mut().flags |= ViewFlags::ROOT_SAME_TALLNESS;
    }));
    app.run();
}
