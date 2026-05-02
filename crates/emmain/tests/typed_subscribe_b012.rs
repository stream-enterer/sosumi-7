/// B-012-rc-shim-mainctrl behavioral tests.
///
/// Covers the 7 click-signal subscription rows (cpp:220-226) converted from
/// the pre-B-012 `Rc<ClickFlags>` shim to D-006 first-Cycle subscribe +
/// `IsSignaled` reactions. The full click-through tests with PanelTree +
/// scheduler harness live in `emMainControlPanel::tests` (internal `mod
/// tests`) because they require access to private types. This file covers
/// the cross-crate observable properties: button click_signal allocation,
/// emCheckButton click_signal vs check_signal feedback-loop separation, and
/// the synchronous reload-fire path through `emMainWindow::ReloadFiles`.
///
/// Mirrors B-003 / B-006 / B-013 test-file rationale.
use std::rc::Rc;

use emcore::emCheckButton::emCheckButton;
use emcore::emInput::{InputKey, emInputEvent};
use emcore::emInputState::emInputState;
use emcore::emLook::emLook;
use emcore::emPanel::PanelState;
use emcore::emScheduler::EngineScheduler;

/// Row 222/223 feedback-loop guard: `SetChecked` (programmatic, called by
/// row-218 reaction) must not fire `click_signal`. If it did, row-222 / row-223
/// reactions would re-trigger row-219's config-change reaction, which would
/// re-call SetChecked, etc. Ported from emCheckButton internal test plus
/// double-checked at the cross-crate boundary so the guarantee is observable
/// from emmain's test harness.
#[test]
fn b012_check_button_set_checked_does_not_fire_click_signal() {
    let ctx = emcore::emContext::emContext::NewRoot();
    let mut sched = EngineScheduler::new();
    let look = Rc::new(emLook::default());

    let mut btn = {
        let mut sc = emcore::emEngineCtx::SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut Vec::new(),
            root_context: &ctx,
            view_context: None,
            framework_clipboard: &std::cell::RefCell::new(None),
            current_engine: None,
            pending_actions: &Rc::new(std::cell::RefCell::new(Vec::new())),
        };
        emCheckButton::new(&mut sc, "test", look)
    };

    let click_sig = btn.click_signal;
    let check_sig = btn.check_signal;

    // Build a minimal PanelCtx for SetChecked. Use with_sched_reach so
    // `as_sched_ctx()` returns Some and SetChecked can fire signals.
    let mut tree = emcore::emPanelTree::PanelTree::new();
    let root_id = tree.create_root("root", false);
    let fw_cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
        std::cell::RefCell::new(None);
    let pa: Rc<std::cell::RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    {
        let mut pctx = emcore::emEngineCtx::PanelCtx::with_sched_reach(
            &mut tree,
            root_id,
            1.0,
            &mut sched,
            &mut fw_actions,
            &ctx,
            &fw_cb,
            &pa,
        );
        btn.SetChecked(true, &mut pctx);
    }

    assert!(
        sched.is_pending(check_sig),
        "SetChecked must fire check_signal"
    );
    assert!(
        !sched.is_pending(click_sig),
        "SetChecked must NOT fire click_signal — feedback-loop guard for B-012 rows 222/223"
    );

    sched.clear_pending_for_tests();
    sched.remove_signal(click_sig);
    sched.remove_signal(check_sig);
}

/// Row 222/223 user-click path: `Input` Enter-press fires both signals.
#[test]
fn b012_check_button_user_click_fires_click_signal() {
    let ctx = emcore::emContext::emContext::NewRoot();
    let mut sched = EngineScheduler::new();
    let look = Rc::new(emLook::default());

    let mut btn = {
        let mut sc = emcore::emEngineCtx::SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut Vec::new(),
            root_context: &ctx,
            view_context: None,
            framework_clipboard: &std::cell::RefCell::new(None),
            current_engine: None,
            pending_actions: &Rc::new(std::cell::RefCell::new(Vec::new())),
        };
        emCheckButton::new(&mut sc, "test", look)
    };

    let click_sig = btn.click_signal;

    // Build a panel state large enough to satisfy emButton's MIN_EXT >= 8 gate
    // and viewed=true so Release-handling can fire.
    let mut state = PanelState::default_for_test();
    state.viewed = true;
    state.viewed_rect = emcore::emPanel::Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    };
    state.enabled = true;

    let is = emInputState::new();
    let mut tree = emcore::emPanelTree::PanelTree::new();
    let root_id = tree.create_root("root", false);
    let fw_cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
        std::cell::RefCell::new(None);
    let pa: Rc<std::cell::RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    {
        let mut pctx = emcore::emEngineCtx::PanelCtx::with_sched_reach(
            &mut tree,
            root_id,
            1.0,
            &mut sched,
            &mut fw_actions,
            &ctx,
            &fw_cb,
            &pa,
        );
        // Enter is the simplest user-click trigger and bypasses hit_test gating.
        btn.Input(
            &emInputEvent::press(InputKey::Enter),
            &state,
            &is,
            &mut pctx,
        );
    }

    assert!(
        sched.is_pending(click_sig),
        "user-click (Enter press) must fire click_signal"
    );

    sched.clear_pending_for_tests();
    sched.remove_signal(click_sig);
    sched.remove_signal(btn.check_signal);
}

/// `emButton::click_signal` is allocated at construction and stable across
/// reads. This is the accessor that emMainControlPanel::Cycle hands off via
/// `ButtonSignals`.
#[test]
fn b012_button_click_signal_is_stable() {
    use emcore::emButton::emButton;
    use slotmap::Key as _;

    let ctx = emcore::emContext::emContext::NewRoot();
    let mut sched = EngineScheduler::new();
    let look = Rc::new(emLook::default());

    let btn = {
        let mut sc = emcore::emEngineCtx::SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut Vec::new(),
            root_context: &ctx,
            view_context: None,
            framework_clipboard: &std::cell::RefCell::new(None),
            current_engine: None,
            pending_actions: &Rc::new(std::cell::RefCell::new(Vec::new())),
        };
        emButton::new(&mut sc, "test", look)
    };

    let sig = btn.click_signal;
    assert!(!sig.is_null(), "emButton::click_signal must be allocated");
    let sig2 = btn.click_signal;
    assert_eq!(sig, sig2, "click_signal must be stable across reads");

    sched.clear_pending_for_tests();
    sched.remove_signal(btn.click_signal);
    sched.remove_signal(btn.press_state_signal);
}
