//! Systematic interaction test for emListBox at 1x and 2x zoom, driven through
//! the full Input dispatch pipeline (PipelineTestHarness).
//!
//! Verifies that clicking on different items selects the correct item at both
//! zoom levels, using view-space coordinates derived from the panel's viewed
//! Restore and the border's content rect.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use emcore::emCursor::emCursor;
use emcore::emEngineCtx::PanelCtx;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emPainter::emPainter;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emViewRenderer::SoftwareCompositor;

use emcore::emListBox::{emListBox, SelectionMode};

use emcore::emLook::emLook;

use super::support::pipeline::PipelineTestHarness;

/// PanelBehavior wrapper for emListBox, allowing shared access via Rc<RefCell>.
///
/// Copied from `behavioral_interaction.rs` SharedListBoxPanel pattern.
struct SharedListBoxPanel {
    inner: Rc<RefCell<emListBox>>,
}

impl PanelBehavior for SharedListBoxPanel {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.inner.borrow_mut().Paint(painter, w, h, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.inner.borrow_mut().Input(event, state, input_state)
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.inner
                .borrow_mut()
                .on_focus_changed(state.in_active_path);
        }
        if flags.intersects(NoticeFlags::ENABLE_CHANGED) {
            self.inner.borrow_mut().on_enable_changed(state.enabled);
        }
    }

    fn IsOpaque(&self) -> bool {
        true
    }

    fn GetCursor(&self) -> emCursor {
        emCursor::Normal
    }
}

/// Compute the view-space Y coordinate for the vertical center of item `n`
/// (0-indexed) in a emListBox with `item_count` items.
///
/// The items are positioned within the border's content rect in panel-local
/// space (x in [0,1], y in [0,tallness]). This function:
///   1. Constructs a border matching emListBox's default config
///   2. Queries GetContentRectUnobscured in normalized panel-local space
///   3. Computes item N's center within the content rect
///   4. Maps the panel-local coordinate to view space using the viewed rect
fn item_center_view_y(
    vr: &emcore::emPanel::Rect,
    pixel_tallness: f64,
    n: usize,
    item_count: usize,
) -> f64 {
    let look = emLook::new();

    // Reconstruct the border with the same config as emListBox::new.
    let border = emBorder::new(OuterBorderType::Instrument)
        .with_inner(InnerBorderType::InputField)
        .with_how_to(true);

    // Panel-local coordinate space: x in [0, 1], y in [0, tallness].
    // tallness = (panel_pixel_h / panel_pixel_w) * pixel_tallness
    let tallness = (vr.h / vr.w) * pixel_tallness;

    let cr = border.GetContentRectUnobscured(1.0, tallness, &look);

    // Item N's center Y in panel-local space.
    let item_local_y = cr.y + (n as f64 + 0.5) / item_count as f64 * cr.h;

    // Map panel-local Y to view-space Y.
    // panel-local y in [0, tallness] maps to view-space [vr.y, vr.y + vr.h].
    vr.y + (item_local_y / tallness) * vr.h
}

/// Compute the view-space X coordinate at the horizontal center of the
/// content rect.
fn content_center_view_x(vr: &emcore::emPanel::Rect, pixel_tallness: f64) -> f64 {
    let look = emLook::new();
    let border = emBorder::new(OuterBorderType::Instrument)
        .with_inner(InnerBorderType::InputField)
        .with_how_to(true);

    let tallness = (vr.h / vr.w) * pixel_tallness;
    let cr = border.GetContentRectUnobscured(1.0, tallness, &look);

    let local_x = cr.x + cr.w * 0.5;
    vr.x + local_x * vr.w
}

#[test]
fn listbox_click_items_1x_and_2x() {
    // 1. Create PipelineTestHarness (800x600 viewport).
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // 2. Create emListBox with 5 items, SelectionMode::Single.
    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetSelectionType(SelectionMode::Single);
    lb.AddItem("item0".to_string(), "Alpha".to_string());
    lb.AddItem("item1".to_string(), "Beta".to_string());
    lb.AddItem("item2".to_string(), "Gamma".to_string());
    lb.AddItem("item3".to_string(), "Delta".to_string());
    lb.AddItem("item4".to_string(), "Epsilon".to_string());

    let lb_ref = Rc::new(RefCell::new(lb));

    // 3. Wrap in SharedListBoxPanel and add to tree.
    let panel_id = h.add_panel_with(
        root,
        "listbox",
        Box::new(SharedListBoxPanel {
            inner: lb_ref.clone(),
        }),
    );

    // 4. Tick + render (SoftwareCompositor) to populate last_w/last_h.
    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    let pt = h.view.GetCurrentPixelTallness();

    // ---------- 5. At 1x zoom ----------

    let state = h.tree.build_panel_state(panel_id, h.view.IsFocused(), pt);
    let vr = state.viewed_rect;
    let click_x = content_center_view_x(&vr, pt);

    // Click item 0
    h.click(click_x, item_center_view_y(&vr, pt, 0, 5));
    assert_eq!(
        lb_ref.borrow().GetSelectedIndex(),
        Some(0),
        "At 1x zoom: clicking item 0 should select it"
    );

    // Click item 2
    h.click(click_x, item_center_view_y(&vr, pt, 2, 5));
    assert_eq!(
        lb_ref.borrow().GetSelectedIndex(),
        Some(2),
        "At 1x zoom: clicking item 2 should select it"
    );

    // Click item 4
    h.click(click_x, item_center_view_y(&vr, pt, 4, 5));
    assert_eq!(
        lb_ref.borrow().GetSelectedIndex(),
        Some(4),
        "At 1x zoom: clicking item 4 should select it"
    );

    // ---------- 6. At 2x zoom ----------

    h.set_zoom(2.0);
    h.tick_n(5);
    compositor.render(&mut h.tree, &h.view);

    let state_2x = h.tree.build_panel_state(panel_id, h.view.IsFocused(), pt);
    let vr2 = state_2x.viewed_rect;
    let click_x_2x = content_center_view_x(&vr2, pt);

    // Click item 0
    h.click(click_x_2x, item_center_view_y(&vr2, pt, 0, 5));
    assert_eq!(
        lb_ref.borrow().GetSelectedIndex(),
        Some(0),
        "At 2x zoom: clicking item 0 should select it"
    );

    // Click item 2
    h.click(click_x_2x, item_center_view_y(&vr2, pt, 2, 5));
    assert_eq!(
        lb_ref.borrow().GetSelectedIndex(),
        Some(2),
        "At 2x zoom: clicking item 2 should select it"
    );

    // Click item 4
    h.click(click_x_2x, item_center_view_y(&vr2, pt, 4, 5));
    assert_eq!(
        lb_ref.borrow().GetSelectedIndex(),
        Some(4),
        "At 2x zoom: clicking item 4 should select it"
    );
}

// ── BP-1: emListBox selection mode behavioral parity tests ─────────────────
//
// These tests exercise every branch in C++ emListBox::SelectByInput across
// all four SelectionMode variants (ReadOnly, Single, Multi, Toggle), driven
// through the full PipelineTestHarness dispatch pipeline.
//
// C++ ref: emListBox.cpp:786-848 (SelectByInput)
//          emListBox.cpp:751-783 (ProcessItemInput)

/// Helper: create a PipelineTestHarness with a emListBox containing 5 items
/// in the given SelectionMode, render once to populate Restore, and return
/// (harness, lb_ref, panel_id, click_x, item_ys).
fn setup_listbox_harness(
    mode: SelectionMode,
) -> (
    PipelineTestHarness,
    Rc<RefCell<emListBox>>,
    emcore::emPanelTree::PanelId,
    f64,
    [f64; 5],
) {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetSelectionType(mode);
    lb.AddItem("i0".to_string(), "Alpha".to_string());
    lb.AddItem("i1".to_string(), "Beta".to_string());
    lb.AddItem("i2".to_string(), "Gamma".to_string());
    lb.AddItem("i3".to_string(), "Delta".to_string());
    lb.AddItem("i4".to_string(), "Epsilon".to_string());

    let lb_ref = Rc::new(RefCell::new(lb));
    let panel_id = h.add_panel_with(
        root,
        "listbox",
        Box::new(SharedListBoxPanel {
            inner: lb_ref.clone(),
        }),
    );

    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    let pt = h.view.GetCurrentPixelTallness();
    let state = h.tree.build_panel_state(panel_id, h.view.IsFocused(), pt);
    let vr = state.viewed_rect;
    let click_x = content_center_view_x(&vr, pt);
    let item_ys = [
        item_center_view_y(&vr, pt, 0, 5),
        item_center_view_y(&vr, pt, 1, 5),
        item_center_view_y(&vr, pt, 2, 5),
        item_center_view_y(&vr, pt, 3, 5),
        item_center_view_y(&vr, pt, 4, 5),
    ];

    (h, lb_ref, panel_id, click_x, item_ys)
}

// ── Single mode ──────────────────────────────────────────────────────────

#[test]
fn listbox_single_mode_click_selects() {
    // C++ ref: SelectByInput SINGLE_SELECTION branch — Select(itemIndex, true)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    h.click(cx, ys[2]);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(2));

    // Clicking another item replaces the selection (solely=true).
    h.click(cx, ys[4]);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(4));
    assert!(!lb.borrow().IsSelected(2));
}

#[test]
fn listbox_single_mode_shift_click_still_selects_solely() {
    // C++ ref: SINGLE_SELECTION ignores shift/ctrl — always Select(itemIndex, true).
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    h.click(cx, ys[1]);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(1));

    // Shift+Click in Single mode still selects solely.
    h.input_state.press(InputKey::Shift);
    h.click(cx, ys[3]);
    h.input_state.release(InputKey::Shift);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(3));
    assert!(!lb.borrow().IsSelected(1));
}

#[test]
fn listbox_single_mode_ctrl_click_still_selects_solely() {
    // C++ ref: SINGLE_SELECTION ignores ctrl — always Select(itemIndex, true).
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    h.click(cx, ys[0]);
    h.input_state.press(InputKey::Ctrl);
    h.click(cx, ys[2]);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(2));
    assert!(!lb.borrow().IsSelected(0));
}

// ── Multi mode ──────────────────────────────────────────────────────────

#[test]
fn listbox_multi_mode_click_selects_solely() {
    // C++ ref: MULTI_SELECTION, no shift, no ctrl -> Select(itemIndex, true)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    h.click(cx, ys[1]);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(1));

    h.click(cx, ys[3]);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(3));
    assert!(
        !lb.borrow().IsSelected(1),
        "plain click in Multi replaces selection"
    );
}

#[test]
fn listbox_multi_shift_click_extends_range() {
    // C++ ref: MULTI_SELECTION, shift=true, ctrl=false ->
    //   range from prev+1..=item (or item..=prev-1), Select(i, false)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click item 1 to set prev_input_index.
    h.click(cx, ys[1]);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(1));

    // Shift+Click item 3: should select range 2..=3 (prev+1..=clicked),
    // and item 1 stays GetChecked since those were Select(i, false) calls.
    h.input_state.press(InputKey::Shift);
    h.click(cx, ys[3]);
    h.input_state.release(InputKey::Shift);

    assert!(lb.borrow().IsSelected(1), "item 1 still selected");
    assert!(lb.borrow().IsSelected(2), "item 2 selected by shift range");
    assert!(lb.borrow().IsSelected(3), "item 3 selected by shift range");
    assert!(!lb.borrow().IsSelected(0));
    assert!(!lb.borrow().IsSelected(4));
}

#[test]
fn listbox_multi_shift_click_extends_range_backward() {
    // C++ ref: MULTI_SELECTION, shift=true, prev > itemIndex ->
    //   range item..=prev-1, Select(i, false)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click item 3 to set prev_input_index.
    h.click(cx, ys[3]);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(3));

    // Shift+Click item 1: range is 1..=2 (item..=prev-1).
    h.input_state.press(InputKey::Shift);
    h.click(cx, ys[1]);
    h.input_state.release(InputKey::Shift);

    assert!(lb.borrow().IsSelected(1), "item 1 in backward range");
    assert!(lb.borrow().IsSelected(2), "item 2 in backward range");
    assert!(lb.borrow().IsSelected(3), "item 3 still selected");
    assert!(!lb.borrow().IsSelected(0));
    assert!(!lb.borrow().IsSelected(4));
}

#[test]
fn listbox_multi_ctrl_click_toggles() {
    // C++ ref: MULTI_SELECTION, shift=false, ctrl=true -> ToggleSelection(itemIndex)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click item 1 (solely selects it).
    h.click(cx, ys[1]);
    assert_eq!(lb.borrow().GetSelectedIndices(), &[1]);

    // Ctrl+Click item 3: toggles item 3 on.
    h.input_state.press(InputKey::Ctrl);
    h.click(cx, ys[3]);
    assert!(lb.borrow().IsSelected(1), "item 1 stays");
    assert!(lb.borrow().IsSelected(3), "item 3 toggled on");

    // Ctrl+Click item 1 again: toggles item 1 off.
    h.click(cx, ys[1]);
    h.input_state.release(InputKey::Ctrl);
    assert!(!lb.borrow().IsSelected(1), "item 1 toggled off");
    assert!(lb.borrow().IsSelected(3), "item 3 remains");
}

#[test]
fn listbox_multi_shift_ctrl_click_toggles_range() {
    // C++ ref: MULTI_SELECTION, shift=true, ctrl=true ->
    //   range toggle: ToggleSelection(i) for each i in range
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click item 1 (sets prev_input_index, selects solely).
    h.click(cx, ys[1]);

    // Shift+Ctrl Click item 3: toggles items 2..=3.
    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    h.click(cx, ys[3]);
    h.input_state.release(InputKey::Shift);
    h.input_state.release(InputKey::Ctrl);

    // Items 2 and 3 were unselected, so toggle turns them on.
    assert!(lb.borrow().IsSelected(1), "item 1 stays from initial click");
    assert!(lb.borrow().IsSelected(2), "item 2 toggled on");
    assert!(lb.borrow().IsSelected(3), "item 3 toggled on");
}

// ── Toggle mode ──────────────────────────────────────────────────────────

#[test]
fn listbox_toggle_mode_click_toggles() {
    // C++ ref: TOGGLE_SELECTION, no shift -> ToggleSelection(itemIndex)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Toggle);

    h.click(cx, ys[0]);
    assert!(lb.borrow().IsSelected(0), "first click toggles on");

    h.click(cx, ys[2]);
    assert!(lb.borrow().IsSelected(0), "item 0 stays on");
    assert!(lb.borrow().IsSelected(2), "item 2 toggled on");

    // Click item 0 again to toggle it off.
    h.click(cx, ys[0]);
    assert!(!lb.borrow().IsSelected(0), "second click toggles off");
    assert!(lb.borrow().IsSelected(2), "item 2 unaffected");
}

#[test]
fn listbox_toggle_mode_ctrl_click_also_toggles() {
    // C++ ref: TOGGLE_SELECTION, ctrl has no special behavior — still
    // goes to the else branch which calls ToggleSelection(itemIndex).
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Toggle);

    h.click(cx, ys[1]);
    assert!(lb.borrow().IsSelected(1));

    // Ctrl+Click should still toggle (ctrl is not special in Toggle mode).
    h.input_state.press(InputKey::Ctrl);
    h.click(cx, ys[1]);
    h.input_state.release(InputKey::Ctrl);
    assert!(!lb.borrow().IsSelected(1), "ctrl+click still toggles off");
}

#[test]
fn listbox_toggle_shift_click_toggles_range() {
    // C++ ref: TOGGLE_SELECTION, shift=true ->
    //   range prev+1..=item, ToggleSelection(i)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Toggle);

    // Click item 1 (toggles on, sets prev).
    h.click(cx, ys[1]);
    assert!(lb.borrow().IsSelected(1));

    // Shift+Click item 4: toggles range 2..=4.
    h.input_state.press(InputKey::Shift);
    h.click(cx, ys[4]);
    h.input_state.release(InputKey::Shift);

    assert!(lb.borrow().IsSelected(1), "item 1 from initial click");
    assert!(lb.borrow().IsSelected(2), "item 2 toggled on by range");
    assert!(lb.borrow().IsSelected(3), "item 3 toggled on by range");
    assert!(lb.borrow().IsSelected(4), "item 4 toggled on by range");
    assert!(!lb.borrow().IsSelected(0));
}

// ── ReadOnly mode ────────────────────────────────────────────────────────

#[test]
fn listbox_readonly_rejects_click() {
    // C++ ref: READ_ONLY_SELECTION branch is empty (break), so no selection change.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::ReadOnly);

    h.click(cx, ys[0]);
    assert!(
        lb.borrow().GetSelectedIndices().is_empty(),
        "ReadOnly rejects click"
    );
}

#[test]
fn listbox_readonly_rejects_shift_click() {
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::ReadOnly);

    h.input_state.press(InputKey::Shift);
    h.click(cx, ys[2]);
    h.input_state.release(InputKey::Shift);
    assert!(
        lb.borrow().GetSelectedIndices().is_empty(),
        "ReadOnly rejects shift+click"
    );
}

#[test]
fn listbox_readonly_rejects_ctrl_click() {
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::ReadOnly);

    h.input_state.press(InputKey::Ctrl);
    h.click(cx, ys[2]);
    h.input_state.release(InputKey::Ctrl);
    assert!(
        lb.borrow().GetSelectedIndices().is_empty(),
        "ReadOnly rejects ctrl+click"
    );
}

// ── Double-Click (trigger) ──────────────────────────────────────────────

/// Helper: dispatch a double-Click (repeat=1) at view-space coordinates through
/// the full pipeline. This simulates what the windowing system sends for a
/// rapid second Click.
fn double_click(h: &mut PipelineTestHarness, view_x: f64, view_y: f64) {
    // First Click (repeat=0).
    let press1 = emInputEvent::press(InputKey::MouseLeft).with_mouse(view_x, view_y);
    let release1 = emInputEvent::release(InputKey::MouseLeft).with_mouse(view_x, view_y);
    h.dispatch(&press1);
    h.dispatch(&release1);
    // Second Click (repeat=1 = double-Click).
    let press2 = emInputEvent::press(InputKey::MouseLeft)
        .with_mouse(view_x, view_y)
        .with_repeat(1);
    let release2 = emInputEvent::release(InputKey::MouseLeft).with_mouse(view_x, view_y);
    h.dispatch(&press2);
    h.dispatch(&release2);
}

#[test]
fn listbox_single_mode_double_click_triggers() {
    // C++ ref: SINGLE_SELECTION — if (trigger) TriggerItem(itemIndex)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    let triggered = Rc::new(RefCell::new(None::<usize>));
    let trig_clone = triggered.clone();
    lb.borrow_mut().on_trigger = Some(Box::new(move |idx| {
        *trig_clone.borrow_mut() = Some(idx);
    }));

    double_click(&mut h, cx, ys[2]);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(2));
    assert_eq!(
        *triggered.borrow(),
        Some(2),
        "double-click triggers in Single mode"
    );
}

#[test]
fn listbox_multi_mode_double_click_triggers() {
    // C++ ref: MULTI_SELECTION — if (trigger) TriggerItem(itemIndex)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    let triggered = Rc::new(RefCell::new(None::<usize>));
    let trig_clone = triggered.clone();
    lb.borrow_mut().on_trigger = Some(Box::new(move |idx| {
        *trig_clone.borrow_mut() = Some(idx);
    }));

    double_click(&mut h, cx, ys[3]);
    assert_eq!(
        *triggered.borrow(),
        Some(3),
        "double-click triggers in Multi mode"
    );
}

#[test]
fn listbox_toggle_mode_double_click_triggers() {
    // C++ ref: TOGGLE_SELECTION — if (trigger) TriggerItem(itemIndex)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Toggle);

    let triggered = Rc::new(RefCell::new(None::<usize>));
    let trig_clone = triggered.clone();
    lb.borrow_mut().on_trigger = Some(Box::new(move |idx| {
        *trig_clone.borrow_mut() = Some(idx);
    }));

    double_click(&mut h, cx, ys[1]);
    assert_eq!(
        *triggered.borrow(),
        Some(1),
        "double-click triggers in Toggle mode"
    );
}

#[test]
fn listbox_readonly_double_click_no_trigger() {
    // C++ ref: READ_ONLY_SELECTION branch does NOT call TriggerItem.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::ReadOnly);

    let triggered = Rc::new(RefCell::new(None::<usize>));
    let trig_clone = triggered.clone();
    lb.borrow_mut().on_trigger = Some(Box::new(move |idx| {
        *trig_clone.borrow_mut() = Some(idx);
    }));

    double_click(&mut h, cx, ys[2]);
    assert!(
        lb.borrow().GetSelectedIndices().is_empty(),
        "ReadOnly: no selection"
    );
    assert_eq!(
        *triggered.borrow(),
        None,
        "ReadOnly: no trigger on double-click"
    );
}

// ── Enter key trigger (all modes) ────────────────────────────────────────

#[test]
fn listbox_single_mode_enter_triggers() {
    // C++ ref: ProcessItemInput EM_KEY_ENTER -> SelectByInput(..., trigger=true)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    let triggered = Rc::new(RefCell::new(None::<usize>));
    let trig_clone = triggered.clone();
    lb.borrow_mut().on_trigger = Some(Box::new(move |idx| {
        *trig_clone.borrow_mut() = Some(idx);
    }));

    // First Click to select and focus item 2.
    h.click(cx, ys[2]);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(2));

    // Enter triggers the focused item.
    h.press_key(InputKey::Enter);
    assert_eq!(
        *triggered.borrow(),
        Some(2),
        "Enter triggers in Single mode"
    );
}

#[test]
fn listbox_readonly_enter_no_trigger() {
    // C++ ref: READ_ONLY_SELECTION -> no trigger
    let (mut h, lb, _pid, _cx, _ys) = setup_listbox_harness(SelectionMode::ReadOnly);

    let triggered = Rc::new(RefCell::new(None::<usize>));
    let trig_clone = triggered.clone();
    lb.borrow_mut().on_trigger = Some(Box::new(move |idx| {
        *trig_clone.borrow_mut() = Some(idx);
    }));

    h.press_key(InputKey::Enter);
    assert_eq!(
        *triggered.borrow(),
        None,
        "ReadOnly: Enter does not trigger"
    );
}

// ── Ctrl+A / Shift+Ctrl+A (select all / Clear) ──────────────────────────

#[test]
fn listbox_multi_ctrl_a_selects_all() {
    // C++ ref: emListBox::Input Key('A') + Ctrl -> SelectAll()
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click first to activate the panel (keyboard events require active path).
    h.click(cx, ys[0]);

    h.input_state.press(InputKey::Ctrl);
    let press = emInputEvent::press(InputKey::Key('a')).with_chars("a");
    h.dispatch(&press);
    let release = emInputEvent::release(InputKey::Key('a'));
    h.dispatch(&release);
    h.input_state.release(InputKey::Ctrl);

    assert_eq!(lb.borrow().GetSelectedIndices(), &[0, 1, 2, 3, 4]);
}

#[test]
fn listbox_multi_shift_ctrl_a_clears() {
    // C++ ref: emListBox::Input Shift+Ctrl+A -> ClearSelection()
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Select some items first.
    h.click(cx, ys[0]);
    assert_eq!(lb.borrow().GetSelectedIndices(), &[0]);

    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    let press = emInputEvent::press(InputKey::Key('a')).with_chars("a");
    h.dispatch(&press);
    let release = emInputEvent::release(InputKey::Key('a'));
    h.dispatch(&release);
    h.input_state.release(InputKey::Shift);
    h.input_state.release(InputKey::Ctrl);

    assert!(
        lb.borrow().GetSelectedIndices().is_empty(),
        "Shift+Ctrl+A clears in Multi"
    );
}

#[test]
fn listbox_toggle_ctrl_a_selects_all() {
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Toggle);

    // Click first to activate the panel (keyboard events require active path).
    h.click(cx, ys[0]);

    h.input_state.press(InputKey::Ctrl);
    let press = emInputEvent::press(InputKey::Key('a')).with_chars("a");
    h.dispatch(&press);
    let release = emInputEvent::release(InputKey::Key('a'));
    h.dispatch(&release);
    h.input_state.release(InputKey::Ctrl);

    assert_eq!(lb.borrow().GetSelectedIndices(), &[0, 1, 2, 3, 4]);
}

#[test]
fn listbox_single_ctrl_a_no_effect() {
    // C++ ref: Ctrl+A only works in Multi/Toggle modes.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    h.click(cx, ys[1]);
    assert_eq!(lb.borrow().GetSelectedIndices(), &[1]);

    h.input_state.press(InputKey::Ctrl);
    let press = emInputEvent::press(InputKey::Key('a')).with_chars("a");
    h.dispatch(&press);
    let release = emInputEvent::release(InputKey::Key('a'));
    h.dispatch(&release);
    h.input_state.release(InputKey::Ctrl);

    // Single mode: Ctrl+A should not select all.
    assert_eq!(
        lb.borrow().GetSelectedIndices(),
        &[1],
        "Ctrl+A has no effect in Single mode"
    );
}

// ── Space key selection ──────────────────────────────────────────────────

#[test]
fn listbox_multi_space_selects_solely() {
    // C++ ref: EM_KEY_SPACE -> SelectByInput(idx, shift=false, ctrl=false, trigger=false)
    // In Multi mode without modifiers -> Select(itemIndex, true)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click item 1 to set focus.
    h.click(cx, ys[1]);
    assert_eq!(lb.borrow().GetSelectedIndices(), &[1]);

    // Press ArrowDown to move focus to item 2 (no selection change in Multi).
    h.press_key(InputKey::ArrowDown);
    // Press Space: selects focused item solely.
    h.press_key(InputKey::Space);
    assert_eq!(
        lb.borrow().GetSelectedIndices(),
        &[2],
        "Space selects solely in Multi"
    );
}

#[test]
fn listbox_toggle_space_toggles() {
    // C++ ref: EM_KEY_SPACE -> SelectByInput with no shift -> ToggleSelection
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Toggle);

    // Click item 0 to focus and toggle on.
    h.click(cx, ys[0]);
    assert!(lb.borrow().IsSelected(0));

    // Space toggles off.
    h.press_key(InputKey::Space);
    assert!(
        !lb.borrow().IsSelected(0),
        "Space toggles off in Toggle mode"
    );

    // Space toggles on again.
    h.press_key(InputKey::Space);
    assert!(lb.borrow().IsSelected(0), "Space toggles on again");
}

#[test]
fn listbox_multi_shift_space_extends_range() {
    // C++ ref: EM_KEY_SPACE + Shift -> SelectByInput(idx, shift=true, ctrl=false, false)
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click item 1 to set prev_input_index and select.
    h.click(cx, ys[1]);

    // Move focus to item 3 without selecting.
    h.press_key(InputKey::ArrowDown); // focus 2
    h.press_key(InputKey::ArrowDown); // focus 3

    // Shift+Space: extends range from prev_input(1) to focus(3).
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::Space);
    h.input_state.release(InputKey::Shift);

    assert!(lb.borrow().IsSelected(1), "item 1 from initial click");
    assert!(lb.borrow().IsSelected(2), "item 2 from shift+space range");
    assert!(lb.borrow().IsSelected(3), "item 3 from shift+space range");
}

#[test]
fn listbox_multi_ctrl_space_toggles() {
    // C++ ref: EM_KEY_SPACE + Ctrl -> SelectByInput(idx, shift=false, ctrl=true, false)
    // In Multi mode, ctrl=true -> ToggleSelection
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    h.click(cx, ys[1]);
    assert_eq!(lb.borrow().GetSelectedIndices(), &[1]);

    // Ctrl+Space on same item toggles it off.
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Space);
    h.input_state.release(InputKey::Ctrl);
    assert!(
        !lb.borrow().IsSelected(1),
        "Ctrl+Space toggles off in Multi"
    );
}

// ── BP-2: emListBox keywalk (type-ahead search) behavioral parity tests ────
//
// These tests verify the keywalk/type-to-search behavior matching C++
// emListBox::KeyWalk (emListBox.cpp:851-927).
//
// C++ behavior summary:
// - Typing printable chars (not ctrl/alt/meta) accumulates a search prefix
// - Timeout > 1000ms clears the accumulator (uses GetInputClockMS)
// - '*' prefix triggers case-insensitive substring search
// - No match clears the accumulator and calls Beep()
// - Prefix match is tried first, then fuzzy match (skip separators)
// - On match: Select(i, true) if not ReadOnly, visit the item panel
// - Focus loss (NF_FOCUS_CHANGED) clears the accumulator

/// Helper: set up a keywalk-focused harness with named items for search tests.
/// Returns (harness, lb_ref, panel_id, click_x, first_item_y).
fn setup_keywalk_harness(
    items: &[&str],
) -> (
    PipelineTestHarness,
    Rc<RefCell<emListBox>>,
    emcore::emPanelTree::PanelId,
    f64,
    f64,
) {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetSelectionType(SelectionMode::Single);
    for (i, text) in items.iter().enumerate() {
        lb.AddItem(format!("item{}", i), text.to_string());
    }

    let lb_ref = Rc::new(RefCell::new(lb));
    let panel_id = h.add_panel_with(
        root,
        "listbox",
        Box::new(SharedListBoxPanel {
            inner: lb_ref.clone(),
        }),
    );

    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    let pt = h.view.GetCurrentPixelTallness();
    let state = h.tree.build_panel_state(panel_id, h.view.IsFocused(), pt);
    let vr = state.viewed_rect;
    let click_x = content_center_view_x(&vr, pt);
    let first_y = item_center_view_y(&vr, pt, 0, items.len());

    // Click to activate the panel so keyboard events are delivered.
    h.click(click_x, first_y);

    (h, lb_ref, panel_id, click_x, first_y)
}

#[test]
fn listbox_keywalk_single_char_prefix() {
    // C++ ref: emListBox.cpp:889-891 — strncasecmp prefix match.
    // Typing a single char should select the first item whose text starts with
    // that character (case-insensitive).
    let (mut h, lb, _pid, _cx, _fy) =
        setup_keywalk_harness(&["Apple", "Banana", "Cherry", "Date", "Elderberry"]);

    // The initial Click GetChecked item 0 ("Apple"). Clear that to verify keywalk.
    lb.borrow_mut().ClearSelection();

    h.press_char('c');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(2),
        "Typing 'c' should select 'Cherry' (index 2)"
    );
}

#[test]
fn listbox_keywalk_single_char_case_insensitive() {
    // C++ ref: strncasecmp is case-insensitive.
    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["Apple", "Banana", "Cherry"]);

    lb.borrow_mut().ClearSelection();

    h.press_char('B');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(1),
        "Typing 'B' (uppercase) should select 'Banana' (case-insensitive)"
    );
}

#[test]
fn listbox_keywalk_accumulated_prefix() {
    // C++ ref: emListBox.cpp:871 — str=KeyWalkChars+event.GetChars()
    // Multiple keystrokes within the timeout accumulate a prefix.
    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["Apple", "Apricot", "Banana"]);

    lb.borrow_mut().ClearSelection();

    // Type 'a' -> Match "Apple" (first prefix match).
    h.press_char('a');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(0),
        "'a' matches 'Apple' first"
    );

    // Type 'p' -> accumulated "ap" still Match "Apple".
    h.press_char('p');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(0),
        "'ap' still matches 'Apple'"
    );

    // Type 'r' -> accumulated "apr" Match "Apricot".
    h.press_char('r');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(1),
        "'apr' matches 'Apricot'"
    );
}

#[test]
fn listbox_keywalk_star_substring_search() {
    // C++ ref: emListBox.cpp:874-888 — '*' prefix triggers substring search.
    // Typing '*' then chars does case-insensitive substring matching.
    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["Apple", "Banana", "Pineapple"]);

    lb.borrow_mut().ClearSelection();

    // Type '*' then 'n' then 'a' then 'n' -> search for substring "nan".
    h.press_char('*');
    // '*' alone Match first item (C++ behavior: empty needle always Match).
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(0),
        "'*' alone matches first item"
    );

    h.press_char('n');
    h.press_char('a');
    h.press_char('n');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(1),
        "'*nan' substring matches 'Banana'"
    );
}

#[test]
fn listbox_keywalk_star_substring_case_insensitive() {
    // C++ ref: emListBox.cpp:879-886 — the substring comparison uses tolower.
    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["FooBar", "BazQux"]);

    lb.borrow_mut().ClearSelection();

    h.press_char('*');
    h.press_char('b');
    h.press_char('a');
    h.press_char('r');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(0),
        "'*bar' substring matches 'FooBar' (case-insensitive)"
    );
}

#[test]
fn listbox_keywalk_no_match_clears_accumulator() {
    // C++ ref: emListBox.cpp:920-924 — on no match, KeyWalkChars.Clear().
    // After a failed search, the accumulator is cleared so the next keystroke
    // starts a fresh search.
    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["Apple", "Banana", "Cherry"]);

    lb.borrow_mut().ClearSelection();

    // Type 'z' -> no item starts with 'z', no match -> accumulator cleared.
    h.press_char('z');
    // No match: C++ does not change selection (KeyWalkChars cleared, no Select call).
    // The initial Click GetChecked item 0, but we cleared it. With no match,
    // nothing is GetChecked.
    assert!(
        lb.borrow().GetSelectedIndex().is_none(),
        "No match for 'z': selection unchanged (nothing selected)"
    );

    // Now type 'b' -> fresh search (not "zb") -> should match "Banana".
    h.press_char('b');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(1),
        "After no-match clear, 'b' starts fresh and matches 'Banana'"
    );
}

#[test]
fn listbox_keywalk_no_match_retains_previous_selection() {
    // C++ ref: emListBox.cpp:920-924 — on no match, only the accumulator is
    // cleared; the existing selection is NOT changed.
    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["Apple", "Banana", "Cherry"]);

    // Initial Click GetChecked item 0. Type 'b' to select "Banana".
    h.press_char('b');
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(1));

    // Type 'z' -> no match. Selection should remain on "Banana".
    h.press_char('z');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(1),
        "No-match 'z' does not change existing selection"
    );
}

#[test]
fn listbox_keywalk_focus_lost_clears_accumulator() {
    // C++ ref: emListBox.cpp:647-656 — NF_FOCUS_CHANGED with !InFocusedPath
    // clears KeyWalkChars.
    //
    // We verify this by accumulating a prefix, simulating focus loss via
    // on_focus_changed(false), then typing again and verifying fresh search.
    // Note: calling on_focus_changed directly rather than through a multi-panel
    // pipeline Click, since creating a second panel for focus stealing is
    // orthogonal to keywalk behavior.
    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["Apple", "Apricot", "Banana"]);

    lb.borrow_mut().ClearSelection();

    // Accumulate "ap" -> Match "Apple".
    h.press_char('a');
    h.press_char('p');
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(0));

    // Simulate focus loss.
    lb.borrow_mut().on_focus_changed(false);
    // Restore focus (so subsequent keystrokes are delivered).
    lb.borrow_mut().on_focus_changed(true);

    // Type 'b' -> should be a fresh search, not "apb".
    h.press_char('b');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(2),
        "After focus loss, 'b' starts fresh and matches 'Banana'"
    );
}

#[test]
fn listbox_keywalk_fuzzy_match_skips_separators() {
    // C++ ref: emListBox.cpp:893-906 — fuzzy match skips ' ', '-', '_' in
    // item text when prefix match fails.
    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["Red-Apple", "Banana"]);

    lb.borrow_mut().ClearSelection();

    // "ra" does not prefix-match "Red-Apple", but fuzzy match succeeds:
    // 'r' Match 'R', 'a' skips '-', Match 'A'.
    h.press_char('r');
    h.press_char('a');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(0),
        "'ra' fuzzy-matches 'Red-Apple' (skipping '-')"
    );
}

#[test]
fn listbox_keywalk_ctrl_chars_rejected() {
    // C++ ref: emListBox.cpp:861 — if state.GetCtrl() return (no keywalk).
    // Ctrl+key should NOT be processed as keywalk.
    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["Apple", "Banana"]);

    lb.borrow_mut().ClearSelection();

    // Ctrl+b should not trigger keywalk to select "Banana".
    h.input_state.press(InputKey::Ctrl);
    h.press_char('b');
    h.input_state.release(InputKey::Ctrl);

    assert!(
        lb.borrow().GetSelectedIndex().is_none(),
        "Ctrl+char should not trigger keywalk"
    );
}

#[test]
fn listbox_keywalk_readonly_no_selection_change() {
    // C++ ref: emListBox.cpp:912 — if (IsEnabled() && SelType != READ_ONLY_SELECTION)
    // In ReadOnly mode, keywalk finds the item but does NOT change selection.
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetSelectionType(SelectionMode::ReadOnly);
    lb.AddItem("i0".to_string(), "Apple".to_string());
    lb.AddItem("i1".to_string(), "Banana".to_string());
    lb.AddItem("i2".to_string(), "Cherry".to_string());

    let lb_ref = Rc::new(RefCell::new(lb));
    let _panel_id = h.add_panel_with(
        root,
        "listbox",
        Box::new(SharedListBoxPanel {
            inner: lb_ref.clone(),
        }),
    );

    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    // Activate the panel by clicking (ReadOnly won't select on Click).
    let pt = h.view.GetCurrentPixelTallness();
    let state = h.tree.build_panel_state(_panel_id, h.view.IsFocused(), pt);
    let vr = state.viewed_rect;
    let cx = content_center_view_x(&vr, pt);
    let fy = item_center_view_y(&vr, pt, 0, 3);
    h.click(cx, fy);

    assert!(lb_ref.borrow().GetSelectedIndices().is_empty());

    // Type 'b' -> keywalk finds "Banana" but does not select in ReadOnly mode.
    h.press_char('b');
    assert!(
        lb_ref.borrow().GetSelectedIndices().is_empty(),
        "ReadOnly mode: keywalk does not change selection"
    );
    // Focus index should still move (C++ visits the panel).
    assert_eq!(
        lb_ref.borrow().focus_index(),
        1,
        "ReadOnly mode: keywalk moves focus_index to matched item"
    );
}

#[test]
fn listbox_keywalk_timeout_clears_accumulator() {
    // C++ behavior: if GetInputClockMS() - KeyWalkClock > 1000, Clear KeyWalkChars.
    // Uses an injectable clock to simulate >1000ms elapsed between keystrokes.
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    // Shared mutable offset that the fake clock reads.
    thread_local! {
        static FAKE_OFFSET: Cell<u64> = const { Cell::new(0) };
    }

    fn fake_clock() -> Instant {
        // Anchor + offset. The anchor is fixed per thread.
        thread_local! {
            static ANCHOR: Instant = Instant::now();
        }
        ANCHOR.with(|a| *a + Duration::from_millis(FAKE_OFFSET.with(|c| c.get())))
    }

    let (mut h, lb, _pid, _cx, _fy) = setup_keywalk_harness(&["Apple", "Apricot", "Banana"]);

    lb.borrow_mut().set_keywalk_clock(fake_clock);
    lb.borrow_mut().ClearSelection();

    // Time 0ms: type 'a' -> Match "Apple".
    FAKE_OFFSET.with(|c| c.set(0));
    h.press_char('a');
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(0));

    // Advance past the 1000ms timeout, then type 'b'.
    FAKE_OFFSET.with(|c| c.set(1500));
    h.press_char('b');
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(2),
        "After timeout, 'b' starts fresh and matches 'Banana'"
    );
}

// ── BP-3: emListBox keyboard navigation (arrow keys) behavioral parity tests ──
//
// The Rust emListBox adds arrow-key focus navigation that is NOT present in the
// C++ emListBox (which relies on mouse clicks and keywalk only). These tests
// verify the Rust-specific arrow key behavior through the full pipeline.
//
// Behavior:
// - ArrowDown: focus_index += 1 (clamped to last item)
// - ArrowUp:   focus_index -= 1 (clamped to 0)
// - In Single mode: arrow keys auto-select the focused item
// - In Multi/Toggle/ReadOnly modes: arrows move focus without selecting

#[test]
fn listbox_arrow_down_moves_focus_single() {
    // In Single mode, ArrowDown moves focus AND auto-selects.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    // Click item 0 to activate panel and set initial state.
    h.click(cx, ys[0]);
    assert_eq!(lb.borrow().focus_index(), 0);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(0));

    h.press_key(InputKey::ArrowDown);
    assert_eq!(lb.borrow().focus_index(), 1, "ArrowDown moves focus to 1");
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(1),
        "Single mode: ArrowDown auto-selects"
    );

    h.press_key(InputKey::ArrowDown);
    assert_eq!(lb.borrow().focus_index(), 2);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(2));
}

#[test]
fn listbox_arrow_up_moves_focus_single() {
    // In Single mode, ArrowUp moves focus AND auto-selects.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    // Click item 2 to focus there.
    h.click(cx, ys[2]);
    assert_eq!(lb.borrow().focus_index(), 2);
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(2));

    h.press_key(InputKey::ArrowUp);
    assert_eq!(lb.borrow().focus_index(), 1, "ArrowUp moves focus to 1");
    assert_eq!(
        lb.borrow().GetSelectedIndex(),
        Some(1),
        "Single mode: ArrowUp auto-selects"
    );
}

#[test]
fn listbox_arrow_down_clamps_at_last() {
    // ArrowDown at the last item does NOT wrap — focus stays at last.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    // Click last item (index 4).
    h.click(cx, ys[4]);
    assert_eq!(lb.borrow().focus_index(), 4);

    h.press_key(InputKey::ArrowDown);
    assert_eq!(
        lb.borrow().focus_index(),
        4,
        "ArrowDown at last item clamps (no wrap)"
    );
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(4));
}

#[test]
fn listbox_arrow_up_clamps_at_first() {
    // ArrowUp at the first item does NOT wrap — focus stays at 0.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    // Click item 0.
    h.click(cx, ys[0]);
    assert_eq!(lb.borrow().focus_index(), 0);

    h.press_key(InputKey::ArrowUp);
    assert_eq!(
        lb.borrow().focus_index(),
        0,
        "ArrowUp at first item clamps (no wrap)"
    );
    assert_eq!(lb.borrow().GetSelectedIndex(), Some(0));
}

#[test]
fn listbox_arrow_down_multi_mode_no_auto_select() {
    // In Multi mode, ArrowDown moves focus but does NOT auto-select.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click item 0 to activate and select it.
    h.click(cx, ys[0]);
    assert_eq!(lb.borrow().GetSelectedIndices(), &[0]);

    h.press_key(InputKey::ArrowDown);
    assert_eq!(lb.borrow().focus_index(), 1, "ArrowDown moves focus");
    assert_eq!(
        lb.borrow().GetSelectedIndices(),
        &[0],
        "Multi mode: ArrowDown does not change selection"
    );
}

#[test]
fn listbox_arrow_up_multi_mode_no_auto_select() {
    // In Multi mode, ArrowUp moves focus but does NOT auto-select.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click item 2 to set focus there.
    h.click(cx, ys[2]);
    assert_eq!(lb.borrow().GetSelectedIndices(), &[2]);

    h.press_key(InputKey::ArrowUp);
    assert_eq!(lb.borrow().focus_index(), 1, "ArrowUp moves focus");
    assert_eq!(
        lb.borrow().GetSelectedIndices(),
        &[2],
        "Multi mode: ArrowUp does not change selection"
    );
}

#[test]
fn listbox_arrow_down_toggle_mode_no_auto_select() {
    // In Toggle mode, ArrowDown moves focus but does NOT auto-select.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Toggle);

    // Click item 0 (toggles on).
    h.click(cx, ys[0]);
    assert!(lb.borrow().IsSelected(0));

    h.press_key(InputKey::ArrowDown);
    assert_eq!(lb.borrow().focus_index(), 1);
    assert!(
        lb.borrow().IsSelected(0),
        "Toggle mode: item 0 stays selected"
    );
    assert!(
        !lb.borrow().IsSelected(1),
        "Toggle mode: ArrowDown does not toggle item 1"
    );
}

#[test]
fn listbox_arrow_down_readonly_mode_moves_focus() {
    // In ReadOnly mode, ArrowDown moves focus but cannot select anything.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::ReadOnly);

    // Click to activate panel (ReadOnly won't select).
    h.click(cx, ys[0]);
    assert!(lb.borrow().GetSelectedIndices().is_empty());
    assert_eq!(lb.borrow().focus_index(), 0);

    h.press_key(InputKey::ArrowDown);
    assert_eq!(
        lb.borrow().focus_index(),
        1,
        "ReadOnly: ArrowDown moves focus"
    );
    assert!(
        lb.borrow().GetSelectedIndices().is_empty(),
        "ReadOnly: no selection change"
    );
}

#[test]
fn listbox_arrow_traverse_full_list() {
    // Traverse all 5 items with ArrowDown, then back up with ArrowUp.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    h.click(cx, ys[0]);
    assert_eq!(lb.borrow().focus_index(), 0);

    // Down through all items.
    for expected in 1..5 {
        h.press_key(InputKey::ArrowDown);
        assert_eq!(lb.borrow().focus_index(), expected);
        assert_eq!(lb.borrow().GetSelectedIndex(), Some(expected));
    }

    // Back up through all items.
    for expected in (0..4).rev() {
        h.press_key(InputKey::ArrowUp);
        assert_eq!(lb.borrow().focus_index(), expected);
        assert_eq!(lb.borrow().GetSelectedIndex(), Some(expected));
    }
}

#[test]
fn listbox_arrow_then_space_selects_focused_multi() {
    // In Multi mode: arrow to move focus, then Space to select the focused item.
    // This is the typical keyboard-only workflow for Multi mode.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Multi);

    // Click item 0 to activate.
    h.click(cx, ys[0]);
    assert_eq!(lb.borrow().GetSelectedIndices(), &[0]);

    // Arrow to item 3.
    h.press_key(InputKey::ArrowDown); // focus 1
    h.press_key(InputKey::ArrowDown); // focus 2
    h.press_key(InputKey::ArrowDown); // focus 3
    assert_eq!(lb.borrow().focus_index(), 3);
    // Selection unchanged (still [0] from Click).
    assert_eq!(lb.borrow().GetSelectedIndices(), &[0]);

    // Ctrl+Space to toggle item 3 on (keeping item 0).
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Space);
    h.input_state.release(InputKey::Ctrl);
    assert!(lb.borrow().IsSelected(0));
    assert!(lb.borrow().IsSelected(3));
}

#[test]
fn listbox_home_jumps_to_first() {
    // Home key should move focus to the first item.
    // Not implemented in Rust emListBox::Input() — the C++ emListBox also does
    // not handle Home/End.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    h.click(cx, ys[3]);
    assert_eq!(lb.borrow().focus_index(), 3);

    h.press_key(InputKey::Home);
    assert_eq!(lb.borrow().focus_index(), 0, "Home jumps to first item");
}

#[test]
fn listbox_end_jumps_to_last() {
    // End key should move focus to the last item.
    // Not implemented in Rust emListBox::Input() — the C++ emListBox also does
    // not handle Home/End.
    let (mut h, lb, _pid, cx, ys) = setup_listbox_harness(SelectionMode::Single);

    h.click(cx, ys[0]);
    assert_eq!(lb.borrow().focus_index(), 0);

    h.press_key(InputKey::End);
    assert_eq!(lb.borrow().focus_index(), 4, "End jumps to last item");
}
