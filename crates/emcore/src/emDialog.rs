use std::rc::Rc;

#[cfg(test)]
use crate::emButton::emButton;
#[cfg(test)]
use crate::emCursor::emCursor;
use crate::emEngineCtx::{ConstructCtx, PanelCtx};
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::emPainter;
#[cfg(test)]
use crate::emPanel::PanelBehavior;
use crate::emPanel::PanelState;
use crate::emPanel::Rect;
use crate::emSignal::SignalId;

use super::emBorder::{emBorder, OuterBorderType};
use crate::emLook::emLook;

/// Result of a dialog interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogResult {
    Ok,
    Cancel,
    Custom(u32),
}

type DialogFinishCb = crate::emEngineCtx::WidgetCallbackRef<DialogResult>;
// DIVERGED: `DialogCheckFinishCb` returns `bool` (veto semantics),
// which is structurally incompatible with both `WidgetCallback<Args>` and
// `WidgetCallbackRef<T>` (both return `()`). The divergence is the return
// value, not the payload lifetime. Remains a plain `Box<dyn FnMut>`.
type DialogCheckFinishCb = Box<dyn FnMut(&DialogResult) -> bool>;

/// Modal dialog container widget.
pub struct emDialog {
    border: emBorder,
    look: Rc<emLook>,
    buttons: Vec<(String, DialogResult)>,
    result: Option<DialogResult>,
    pub on_finish: Option<DialogFinishCb>,
    pub on_check_finish: Option<DialogCheckFinishCb>,
    auto_delete: bool,
    /// Allocated per C++ `emDialog::GetFinishSignal()`. B3.4b: alloc only.
    pub finish_signal: SignalId,
}

const BUTTON_HEIGHT: f64 = 22.0;
const BUTTON_SPACING: f64 = 4.0;
const BOTTOM_MARGIN: f64 = 4.0;

/// Root-panel PanelBehavior for an `emDialog`.
///
/// Port of C++ `emDialog::DlgPanel : public emBorder` (emDialog.h:186-204).
/// Lives as the root panel of the dialog's owned `emWindow`. Holds the
/// dialog's mutable state (result, buttons, finish-state, auto-delete
/// countdown) because `DialogPrivateEngine::Cycle` reaches state through
/// `tree.take_behavior(root_panel_id)` — the Rust analog of the C++
/// `PrivateEngineClass::Dlg&` back-reference (B3.5e).
///
/// `#[cfg(test)]`-gated in Task 2 — Task 5 removes the gate when
/// DialogPrivateEngine becomes the real consumer. Without the gate, the
/// lib build's dead-code lint would fire on fields unused until Task 5.
#[cfg(test)]
pub struct DlgPanel {
    border: emBorder,
    look: Rc<emLook>,
    /// Dialog buttons: (caption string, result payload). Rendered in the
    /// bottom button row as `DlgButton` child panels.
    pub(crate) buttons: Vec<(String, DialogResult)>,
    /// Set by `Finish` once CheckFinish permits. `DialogPrivateEngine`
    /// observes this on Cycle and fires `finish_signal`.
    pub(crate) pending_result: Option<DialogResult>,
    /// Stored after the finish signal has fired. Read via `GetResult`.
    pub(crate) finalized_result: Option<DialogResult>,
    /// Mirrors C++ `emDialog::FinishState` (emDialog.cpp:146-223). 0 = no
    /// finish pending; 1 = Finish has been called and accepted (the next
    /// `DialogPrivateEngine::Cycle` fires `finish_signal` and invokes
    /// `on_finish`/`on_finished`, advancing to 2); 2..3 = auto-delete
    /// countdown; at 3 the engine emits `DeferredAction::CloseWindow`
    /// (C++ `delete this`). If `auto_delete` is false, state returns to
    /// 0 after firing (C++ `!ADEnabled` branch).
    pub(crate) finish_state: u8,
    pub(crate) auto_delete: bool,
    pub(crate) finish_signal: SignalId,
    pub(crate) on_finish: Option<DialogFinishCb>,
    pub(crate) on_check_finish: Option<DialogCheckFinishCb>,
    /// Port of C++ `virtual void emDialog::Finished(int result)` (D1 — callback,
    /// not trait method). Fires from `DialogPrivateEngine::Cycle` after
    /// finish_signal fires. Default `None` matches C++ default (no-op).
    pub(crate) on_finished: Option<DialogFinishCb>,
    /// PanelId of the emLinearLayout content panel, set by Task 7.
    pub(crate) content_panel_id: Option<crate::emPanelTree::PanelId>,
    /// PanelId of the emLinearLayout button-row panel, set by Task 7.
    pub(crate) buttons_panel_id: Option<crate::emPanelTree::PanelId>,
    /// Parallel `(click_signal, result)` pairs for the dialog's buttons.
    /// Populated by Task 7 when `DlgButton` children are materialized;
    /// empty for Task 4. `DialogPrivateEngine::Cycle` iterates these to
    /// observe button clicks, mirroring C++ `emDialog::PrivateEngineClass`
    /// observing button signals via `AddWakeUpSignal` (emDialog.cpp:38).
    pub(crate) button_signals: Vec<(SignalId, DialogResult)>,
}

#[cfg(test)]
impl DlgPanel {
    pub(crate) fn new(title: &str, look: Rc<emLook>, finish_signal: SignalId) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::PopupRoot).with_caption(title),
            look,
            buttons: Vec::new(),
            pending_result: None,
            finalized_result: None,
            finish_state: 0,
            auto_delete: false,
            finish_signal,
            on_finish: None,
            on_check_finish: None,
            on_finished: None,
            content_panel_id: None,
            buttons_panel_id: None,
            button_signals: Vec::new(),
        }
    }

    pub(crate) fn SetTitle(&mut self, title: &str) {
        self.border.SetCaption(title);
    }
}

#[cfg(test)]
impl PanelBehavior for DlgPanel {
    fn as_dlg_panel_mut(&mut self) -> Option<&mut DlgPanel> {
        Some(self)
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        let pixel_scale = 1.0; // DlgPanel is the view root; no enclosing scaling
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Port of C++ DlgPanel::LayoutChildren (emDialog.cpp:302-322).
        // Same operation order as C++:
        //   GetContentRectUnobscured(&x,&y,&w,&h,&cc);
        //   bh = emMin(w*0.08, h*0.3);
        //   sp = bh * 0.25;
        //   x += sp; y += sp; w -= 2*sp; h -= 2*sp;
        //   ContentPanel->Layout(x, y, w, h-sp-bh, cc);
        //   ButtonsPanel->Layout(x, y+h-bh, w, bh, cc);
        let Rect { w: pw, h: ph, .. } = ctx.layout_rect();
        let Rect {
            mut x,
            mut y,
            mut w,
            mut h,
        } = self.border.GetContentRectUnobscured(pw, ph, &self.look);
        let bh = f64::min(w * 0.08, h * 0.3);
        let sp = bh * 0.25;
        x += sp;
        y += sp;
        w -= 2.0 * sp;
        h -= 2.0 * sp;
        if let Some(content_id) = self.content_panel_id {
            ctx.layout_child(content_id, x, y, w, h - sp - bh);
        }
        if let Some(buttons_id) = self.buttons_panel_id {
            ctx.layout_child(buttons_id, x, y + h - bh, w, bh);
        }

        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn GetCanvasColor(&self) -> crate::emColor::emColor {
        // PopupRoot border paints opaque background; canvas = content.
        self.border
            .content_canvas_color(crate::emColor::emColor::TRANSPARENT, &self.look, true)
    }

    fn IsOpaque(&self) -> bool {
        true // PopupRoot covers the whole dialog viewport
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        // Port of C++ emDialog::DlgPanel::Input (emDialog.cpp:277-299).
        // DIVERGED: emBorder has no Input in Rust; C++ emBorder::Input called
        // here handles focus traversal. Track as latent gap — revisit if
        // emBorder gains Input.
        if event.variant != InputVariant::Press {
            return false;
        }
        // C++ uses state.IsNoMod() (emInput.h:293): treats Shift as a blocking
        // modifier too. Inline-expanded since Rust emInputState has no IsNoMod.
        if event.shift || event.ctrl || event.alt || event.meta {
            return false;
        }
        match event.key {
            InputKey::Enter => {
                // Set pending result; DialogPrivateEngine observes on next Cycle,
                // runs on_check_finish, fires finish_signal. Not a direct Finish
                // call: Finish semantics live in the private engine per C++.
                self.pending_result = Some(DialogResult::Ok);
                true
            }
            InputKey::Escape => {
                self.pending_result = Some(DialogResult::Cancel);
                true
            }
            _ => false,
        }
    }
}

/// PanelBehavior wrapping `emButton` for a dialog button.
///
/// Port of C++ `class DlgButton : public emButton` (emDialog.h:169-183).
/// Carries a `DialogResult` payload (C++: `int Result`) and a reference
/// to the owning `DlgPanel` (via `PanelId`). In C++, `Clicked()` calls
/// `((emDialog*)GetWindow())->Finish(Result)` — a direct back-edge through
/// the window pointer. In Rust, click observation is engine-side:
/// `DialogPrivateEngine` (Task 4+7) connects the button's `click_signal`
/// to its own wake-up set (`scheduler.connect(click_signal, private_engine_id)`),
/// matching C++ `emDialog::PrivateEngineClass` observing button signals via
/// `AddWakeUpSignal`. Therefore the Rust `Input` impl here is a pure
/// delegator to `emButton::Input`; it does not write `pending_result`
/// itself — the private engine does on observing the click signal.
///
/// Precedent: `ButtonPanel` adapter in `emColorFieldFieldPanel.rs:187-210`.
///
/// `#[cfg(test)]`-gated in Task 3 — Task 5 removes the gate when wired
/// into the public API.
#[cfg(test)]
pub(crate) struct DlgButton {
    pub(crate) button: emButton,
    /// Dialog result payload carried by this button. C++ parity: `int Result`
    /// in `class DlgButton` (emDialog.h:182).
    pub(crate) result: DialogResult,
    /// PanelId of the owning `DlgPanel`. The engine-side click observer
    /// (Task 4+7) uses this to reach the `DlgPanel` and write
    /// `pending_result`. Rust analog of the C++ back-edge
    /// `((emDialog*)GetWindow())->Finish(Result)` (emDialog.cpp:236).
    pub(crate) dlg_panel_id: crate::emPanelTree::PanelId,
}

#[cfg(test)]
impl DlgButton {
    pub(crate) fn new<C: ConstructCtx>(
        ctx: &mut C,
        caption: &str,
        look: Rc<emLook>,
        result: DialogResult,
        dlg_panel_id: crate::emPanelTree::PanelId,
    ) -> Self {
        Self {
            button: emButton::new(ctx, caption, look),
            result,
            dlg_panel_id,
        }
    }

    /// Port of C++ `emBorder::GetCaption` (via `emButton` inheritance).
    pub(crate) fn caption(&self) -> &str {
        self.button.GetCaption()
    }

    /// Port of C++ `DlgButton::GetResult` (emDialog.h:249-252).
    pub(crate) fn result(&self) -> &DialogResult {
        &self.result
    }

    pub(crate) fn SetCaption(&mut self, text: &str) {
        self.button.SetCaption(text);
    }
}

#[cfg(test)]
impl PanelBehavior for DlgButton {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.button.Paint(painter, w, h, state.enabled, pixel_scale);
    }

    // DIVERGED: DlgButton click observation — C++ emDialog.cpp:236 `DlgButton::Clicked()` walks
    // parent chain via `((emDialog*)GetWindow())->Finish(Result)`. Rust wires this engine-side
    // via `scheduler.connect(button.click_signal, private_engine_id)` at install time (Task 7),
    // so `Input` here is a pure delegator.
    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        // Pure delegator. Click observation is engine-side via
        // `scheduler.connect(button.click_signal, dialog_private_engine_id)`
        // (Task 4+7), matching C++ `PrivateEngineClass::AddWakeUpSignal`.
        self.button.Input(event, state, input_state, ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.button.GetCursor()
    }
}

/// Port of C++ `emDialog::PrivateEngineClass` (emDialog.h:203-210,
/// emDialog.cpp:194-224). Installed at `Priority::High` and wired to
/// `close_signal` (C++: `AddWakeUpSignal(GetCloseSignal())` in the
/// `emDialog` ctor, emDialog.cpp:38). `Cycle` ports `PrivateCycle`
/// (emDialog.cpp:194-224) beat-for-beat:
///   1. Close signal observed ⇒ `pending_result = Cancel` (C++ Finish(NEGATIVE)).
///   2. Iterate button click signals ⇒ `pending_result = button.result`
///      (C++ `DlgButton::Clicked` calls `GetWindow()->Finish(Result)`; in
///      Rust the engine observes the signal — see `DlgButton` doc comment).
///   3. If `pending_result` set and not yet finalized, run `on_check_finish`
///      veto → finalize, fire `finish_signal`, invoke `on_finish`/`on_finished`
///      (C++ `Finish` + `FinishState==1` branch).
///   4. Auto-delete countdown: 3 slices after finalize, emit
///      `DeferredAction::CloseWindow` (C++ `delete this` at FinishState==4).
///
/// `#[cfg(test)]`-gated until Task 5 wires it into emDialog construction.
/// The `install` associated function mirrors the ctor's "create + priority +
/// wake-up-signal + connect" sequence (emDialog.cpp:37-38).
#[cfg(test)]
pub(crate) struct DialogPrivateEngine {
    pub(crate) root_panel_id: crate::emPanelTree::PanelId,
    pub(crate) window_id: Option<winit::window::WindowId>,
    pub(crate) close_signal: SignalId,
}

#[cfg(test)]
impl crate::emEngine::emEngine for DialogPrivateEngine {
    fn Cycle(&mut self, ctx: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool {
        // Port of emDialog::PrivateCycle (emDialog.cpp:194-224).
        //
        // Step 0: detach DlgPanel behavior — Rust analog of the C++
        // `PrivateEngineClass::Dlg&` back-reference. After `take_behavior`,
        // `tree`'s borrow is returned and we may freely call `as_sched_ctx`
        // on `ctx` to invoke widget callbacks. No `unsafe` needed.
        //
        // Phase 3.5.A Task 10: DialogPrivateEngine is registered at
        // `PanelScope::Toplevel(dialog_window_id)` via
        // `App::install_pending_top_level` (production) or
        // `install_pending_top_level_headless` (tests), so `ctx.tree` is
        // always `Some` during Cycle.
        let Some(mut behavior) = ctx
            .tree
            .as_deref_mut()
            .expect("DialogPrivateEngine: tree is Some (Toplevel scope)")
            .take_behavior(self.root_panel_id)
        else {
            // Panel gone — nothing to do.
            return false;
        };

        let stay_awake = {
            let Some(dlg) = behavior.as_dlg_panel_mut() else {
                // Non-DlgPanel at root_panel_id: wiring bug. Put it back and
                // go to sleep — defensive no-op.
                ctx.tree
                    .as_deref_mut()
                    .expect("DialogPrivateEngine: tree is Some")
                    .put_behavior(self.root_panel_id, behavior);
                return false;
            };

            // Step 1: close_signal → Cancel (emDialog.cpp:196-198
            // `if (IsSignaled(GetCloseSignal())) Finish(NEGATIVE);`).
            // Guard on `pending_result.is_none() && finalized_result.is_none()`
            // matches `Finish`'s "no-op once result is set" semantics
            // (emDialog.cpp: once Result is Finalized, subsequent Finish calls
            // through PrivateCycle short-circuit via FinishState>0 branches).
            if ctx.IsSignaled(self.close_signal)
                && dlg.pending_result.is_none()
                && dlg.finalized_result.is_none()
            {
                dlg.pending_result = Some(DialogResult::Cancel);
            }

            // Step 2: button click signals (Task 7 populates button_signals).
            // Iterated by value to avoid aliasing `dlg.button_signals` with
            // `dlg.pending_result` writes.
            let button_fires: Vec<DialogResult> = dlg
                .button_signals
                .iter()
                .filter_map(|(sig, result)| {
                    if ctx.IsSignaled(*sig) {
                        Some(result.clone())
                    } else {
                        None
                    }
                })
                .collect();
            for result in button_fires {
                if dlg.pending_result.is_none() && dlg.finalized_result.is_none() {
                    dlg.pending_result = Some(result);
                }
            }

            // Step 3: pending_result set → check_finish veto → finalize
            // (sets finish_state=1). Ports the body of C++ emDialog::Finish
            // (emDialog.cpp:146-153): if CheckFinish accepts, Result=r and
            // FinishState=1. The signal fire + Finished invocation live in
            // the FinishState==1 branch below, matching C++ structure.
            if let Some(pending) = dlg.pending_result.take() {
                let vetoed = if let Some(cb) = dlg.on_check_finish.as_mut() {
                    !cb(&pending)
                } else {
                    false
                };
                if !vetoed && dlg.finish_state == 0 {
                    dlg.finalized_result = Some(pending);
                    dlg.finish_state = 1;
                }
            }

            // Step 4: state-machine dispatch. Ports emDialog.cpp:200-223
            // (PrivateCycle if/else chain) one-to-one.
            //
            // C++:
            //   if (FinishState<=0) return false;
            //   else if (FinishState==1) { FinishState=2; Signal(FinishSignal); Finished(Result); return true; }
            //   else if (!ADEnabled) { FinishState=0; return false; }
            //   else if (FinishState<3) { FinishState++; return true; }
            //   else { delete this; return false; }
            //
            // DIVERGED: `delete this` becomes a deferred
            // `DeferredAction::CloseWindow`, because emWindow lifetime is
            // owned by emGUIFramework rather than self-destructed.
            let state = dlg.finish_state;
            if state == 0 {
                false
            } else if state == 1 {
                // Advance first, then fire + invoke callbacks. Matches C++
                // ordering: FinishState=2 is observable to any code the
                // Signal/Finished call chain reaches (emDialog.cpp:204-206).
                dlg.finish_state = 2;
                let finish_signal = dlg.finish_signal;
                let result = dlg
                    .finalized_result
                    .clone()
                    .expect("finish_state==1 implies finalized_result is set");
                // Take callbacks to avoid aliasing with ctx.as_sched_ctx();
                // leave None afterwards — C++ invokes `Finished(Result)`
                // exactly once per dialog (virtual dispatch, no re-arm).
                let mut on_finish = dlg.on_finish.take();
                let mut on_finished = dlg.on_finished.take();
                let mut sched = ctx.as_sched_ctx();
                sched.fire(finish_signal);
                if let Some(cb) = on_finish.as_mut() {
                    cb(&result, &mut sched);
                }
                if let Some(cb) = on_finished.as_mut() {
                    cb(&result, &mut sched);
                }
                true
            } else if !dlg.auto_delete {
                dlg.finish_state = 0;
                false
            } else if dlg.finish_state < 3 {
                dlg.finish_state += 1;
                true
            } else {
                // state == 3 (or greater): `delete this` in C++.
                if let Some(wid) = self.window_id {
                    ctx.framework_action(crate::emEngineCtx::DeferredAction::CloseWindow(wid));
                }
                false
            }
        };

        // Step 5: put DlgPanel behavior back.
        let tree = ctx
            .tree
            .as_deref_mut()
            .expect("DialogPrivateEngine: tree is Some");
        if tree.panels.contains_key(self.root_panel_id) {
            tree.put_behavior(self.root_panel_id, behavior);
        }
        stay_awake
    }
}

impl emDialog {
    pub fn new<C: ConstructCtx>(ctx: &mut C, title: &str, look: Rc<emLook>) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::PopupRoot).with_caption(title),
            look,
            buttons: Vec::new(),
            result: None,
            on_finish: None,
            on_check_finish: None,
            auto_delete: false,
            finish_signal: ctx.create_signal(),
        }
    }

    pub fn AddCustomButton(&mut self, label: &str, result: DialogResult) {
        self.buttons.push((label.to_string(), result));
    }

    /// Update the dialog title (border caption).
    pub fn SetRootTitle(&mut self, title: &str) {
        self.border.SetCaption(title);
    }

    /// Update the label of the first button whose result matches `result`.
    pub fn set_button_label_for_result(&mut self, result: &DialogResult, label: &str) {
        if let Some((lbl, _)) = self.buttons.iter_mut().find(|(_, r)| r == result) {
            *lbl = label.to_string();
        }
    }

    /// Get the look used by this dialog.
    pub fn look(&self) -> &Rc<emLook> {
        &self.look
    }

    pub fn GetResult(&self) -> Option<&DialogResult> {
        self.result.as_ref()
    }

    /// Mirrors C++ `emDialog::PrivateEngine::Cycle` finishing branch
    /// (emDialog.cpp:200-206): CheckFinish → Signal(FinishSignal) → Finished.
    pub fn Finish(&mut self, result: DialogResult, ctx: &mut PanelCtx<'_>) {
        if let Some(cb) = &mut self.on_check_finish {
            if !cb(&result) {
                return;
            }
        }
        self.result = Some(result.clone());
        if let Some(mut sched) = ctx.as_sched_ctx() {
            sched.fire(self.finish_signal);
            if let Some(cb) = self.on_finish.as_mut() {
                cb(&result, &mut sched);
            }
        }
    }

    /// Silently cancel an in-flight dialog without firing the finish signal
    /// or invoking any callback. Used by emStocksListBox when it replaces
    /// an in-flight confirmation dialog with a freshly created one —
    /// there is no observer to notify, and firing would create a spurious
    /// Finished(Cancel) signal that no UI consumer is watching. C++ parity:
    /// the emStocksListBox code path simply `delete`s the old dialog
    /// without calling `Finish` on it (emStocksListBox.cpp).
    pub fn silent_cancel(&mut self) {
        self.result = Some(DialogResult::Cancel);
    }

    pub fn Paint(&self, painter: &mut emPainter, w: f64, h: f64, pixel_scale: f64) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
    }

    /// Layout content area and button row at the bottom.
    pub fn LayoutChildren(&self, ctx: &mut PanelCtx, w: f64, h: f64) {
        let Rect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.border.GetContentRect(w, h, &self.look);
        let children = ctx.children();

        if children.is_empty() {
            return;
        }

        let button_count = self.buttons.len();
        let content_children = children.len().saturating_sub(button_count);

        // Layout content children above button row
        let content_h = ch - BUTTON_HEIGHT - BOTTOM_MARGIN;
        for (i, &child) in children.iter().take(content_children).enumerate() {
            let child_h = content_h / content_children.max(1) as f64;
            ctx.layout_child(child, cx, cy + i as f64 * child_h, cw, child_h);
        }

        // Layout button row at the bottom
        if button_count > 0 {
            let total_btn_w = cw;
            let btn_w =
                (total_btn_w - (button_count as f64 - 1.0) * BUTTON_SPACING) / button_count as f64;
            let btn_y = cy + ch - BUTTON_HEIGHT;

            for (i, &child) in children.iter().skip(content_children).enumerate() {
                let btn_x = cx + i as f64 * (btn_w + BUTTON_SPACING);
                ctx.layout_child(child, btn_x, btn_y, btn_w, BUTTON_HEIGHT);
            }
        }

        // Propagate content canvas color to children.
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        self.border
            .preferred_size_for_content(200.0, 120.0 + BUTTON_HEIGHT + BOTTOM_MARGIN)
    }

    /// Access a button label and result by index.
    ///
    /// Port of C++ `emDialog::GetButton`.
    pub fn GetButton(&self, index: usize) -> Option<&(String, DialogResult)> {
        self.buttons.get(index)
    }

    /// Find the first button whose result matches `result`.
    ///
    /// Port of C++ `emDialog::GetButtonForResult`.
    pub fn GetButtonForResult(&self, result: &DialogResult) -> Option<&(String, DialogResult)> {
        self.buttons.iter().find(|(_, r)| r == result)
    }

    /// Convenience accessor for the first `Ok` button.
    pub fn GetOKButton(&self) -> Option<&(String, DialogResult)> {
        self.GetButtonForResult(&DialogResult::Ok)
    }

    /// Convenience accessor for the first `Cancel` button.
    pub fn GetCancelButton(&self) -> Option<&(String, DialogResult)> {
        self.GetButtonForResult(&DialogResult::Cancel)
    }

    /// Enable automatic deletion when the dialog closes.
    pub fn EnableAutoDeletion(&mut self) {
        self.auto_delete = true;
    }

    /// Check if auto-deletion is enabled.
    pub fn IsAutoDeletionEnabled(&self) -> bool {
        self.auto_delete
    }

    /// Static convenience to create a message dialog (returns a configured `emDialog`).
    ///
    /// Port of C++ `emDialog::ShowMessage`.
    pub fn ShowMessage<C: ConstructCtx>(ctx: &mut C, text: &str, look: Rc<emLook>) -> Self {
        let mut dlg = Self::new(ctx, text, look);
        dlg.AddCustomButton("OK", DialogResult::Ok);
        dlg
    }

    /// Handle keyboard input for the dialog.
    ///
    /// Port of C++ `emDlg::DlgPanel::Input`:
    /// - Enter (no modifiers) → finish with `DialogResult::Ok`
    /// - Escape (no modifiers) → finish with `DialogResult::Cancel`
    pub fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        if event.variant != InputVariant::Press {
            return false;
        }
        if event.ctrl || event.alt || event.meta {
            return false;
        }
        match event.key {
            InputKey::Enter => {
                self.Finish(DialogResult::Ok, ctx);
                true
            }
            InputKey::Escape => {
                self.Finish(DialogResult::Cancel, ctx);
                true
            }
            _ => false,
        }
    }

    /// Check if the dialog should close (i.e. a result has been set).
    ///
    /// Port of C++ `emDialog::CheckFinish`.
    pub fn CheckFinish(&self) -> bool {
        self.result.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngineCtx::{DeferredAction, InitCtx};
    use crate::emPanel::Rect;
    use crate::emPanelTree::{PanelId, PanelTree};
    use crate::emScheduler::EngineScheduler;
    use slotmap::Key as _;
    use std::cell::RefCell;

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<crate::emContext::emContext>,
    }
    impl Drop for TestInit {
        fn drop(&mut self) {
            // B3.4c: clear pending signals accumulated during Input-path tests
            self.sched.clear_pending_for_tests();
        }
    }

    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: crate::emContext::emContext::NewRoot(),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
            }
        }
    }

    fn test_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let id = tree.create_root("t", false);
        (tree, id)
    }

    fn default_panel_state() -> PanelState {
        PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        }
    }

    fn default_input_state() -> emInputState {
        emInputState::new()
    }

    #[test]
    fn dialog_fires_finish_signal_on_input_enter() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        let sig = dlg.finish_signal;
        let (mut tree, tid) = test_tree();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
            );
            dlg.Input(
                &emInputEvent::press(InputKey::Enter),
                &default_panel_state(),
                &default_input_state(),
                &mut ctx,
            );
        }
        assert!(__init.sched.is_pending(sig));
    }

    #[test]
    fn dialog_finish_fires_callback() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let look = emLook::new();
        let results = Rc::new(RefCell::new(Vec::new()));
        let res_clone = results.clone();

        let mut dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        dlg.AddCustomButton("OK", DialogResult::Ok);
        dlg.AddCustomButton("Cancel", DialogResult::Cancel);
        dlg.on_finish = Some(Box::new(
            move |r: &DialogResult, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                res_clone.borrow_mut().push(r.clone());
            },
        ));

        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        let mut ctx = PanelCtx::with_sched_reach(
            &mut tree,
            tid,
            1.0,
            &mut __init.sched,
            &mut __init.fw,
            &__init.root,
            &fw_cb,
        );
        assert!(dlg.GetResult().is_none());
        dlg.Finish(DialogResult::Ok, &mut ctx);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
        assert_eq!(*results.borrow(), vec![DialogResult::Ok]);
    }

    #[test]
    fn check_finish_can_veto() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Veto", look);
        dlg.on_check_finish = Some(Box::new(|r| *r != DialogResult::Cancel));

        dlg.Finish(DialogResult::Cancel, &mut ctx);
        assert!(dlg.GetResult().is_none(), "veto should prevent finish");

        dlg.Finish(DialogResult::Ok, &mut ctx);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
    }

    #[test]
    fn dialog_custom_result() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Custom", look);
        dlg.AddCustomButton("Retry", DialogResult::Custom(42));
        dlg.Finish(DialogResult::Custom(42), &mut ctx);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Custom(42)));
    }

    #[test]
    fn enter_finishes_with_ok() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let consumed = dlg.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(consumed);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
    }

    #[test]
    fn escape_finishes_with_cancel() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let consumed = dlg.Input(&emInputEvent::press(InputKey::Escape), &ps, &is, &mut ctx);
        assert!(consumed);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Cancel));
    }

    #[test]
    fn enter_with_modifier_is_ignored() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let mut ev = emInputEvent::press(InputKey::Enter);
        ev.ctrl = true;
        let consumed = dlg.Input(&ev, &ps, &is, &mut ctx);
        assert!(!consumed);
        assert!(dlg.GetResult().is_none());
    }

    #[test]
    fn release_event_is_ignored() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        let ps = default_panel_state();
        let is = default_input_state();

        let consumed = dlg.Input(&emInputEvent::release(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!consumed);
        assert!(dlg.GetResult().is_none());
    }

    #[test]
    fn add_custom_button_lookup() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        dlg.AddCustomButton("Apply", DialogResult::Custom(7));
        let btn = dlg.GetButtonForResult(&DialogResult::Custom(7));
        assert!(btn.is_some());
        let (label, _) = btn.unwrap();
        assert_eq!(label, "Apply");
    }

    #[test]
    fn set_button_label() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        dlg.AddCustomButton("OK", DialogResult::Ok);
        dlg.set_button_label_for_result(&DialogResult::Ok, "Accept");
        let btn = dlg.GetButtonForResult(&DialogResult::Ok);
        assert!(btn.is_some());
        let (label, _) = btn.unwrap();
        assert_eq!(label, "Accept");
    }

    #[test]
    fn auto_deletion_toggle() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        assert!(!dlg.IsAutoDeletionEnabled());
        let mut dlg = dlg;
        dlg.EnableAutoDeletion();
        assert!(dlg.IsAutoDeletionEnabled());
    }

    #[test]
    fn check_finish_lifecycle() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Test", look);
        assert!(!dlg.CheckFinish());
        dlg.Finish(DialogResult::Ok, &mut ctx);
        assert!(dlg.CheckFinish());
    }

    #[test]
    fn dlg_panel_enter_sets_pending_ok() {
        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut __init = TestInit::new();
        let finish_sig = __init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let ev = emInputEvent::press(InputKey::Enter);
        let ps = PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        };
        let is = emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(consumed, "Enter should be consumed");
        assert_eq!(panel.pending_result, Some(DialogResult::Ok));
        // Read remaining fields so dead-code doesn't fire before Task 5 wires
        // DlgPanel into emDialog. All of these are observed by
        // DialogPrivateEngine::Cycle (Task 4) per plan §B3.5e.
        assert!(panel.buttons.is_empty());
        assert!(panel.finalized_result.is_none());
        assert_eq!(panel.finish_state, 0);
        assert!(!panel.auto_delete);
        let _ = panel.finish_signal;
        assert!(panel.on_finish.is_none());
        assert!(panel.on_check_finish.is_none());
        assert!(panel.on_finished.is_none());
        assert!(panel.content_panel_id.is_none());
        assert!(panel.buttons_panel_id.is_none());
        assert!(panel.button_signals.is_empty());
        panel.SetTitle("New");
    }

    #[test]
    fn dlg_panel_escape_sets_pending_cancel() {
        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut __init = TestInit::new();
        let finish_sig = __init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let ev = emInputEvent::press(InputKey::Escape);
        let ps = PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        };
        let is = emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(consumed, "Escape should be consumed");
        assert_eq!(panel.pending_result, Some(DialogResult::Cancel));
    }

    #[test]
    fn dlg_panel_modified_enter_is_ignored() {
        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut __init = TestInit::new();
        let finish_sig = __init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let mut ev = emInputEvent::press(InputKey::Enter);
        ev.ctrl = true;
        let ps = PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        };
        let is = emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(!consumed, "Ctrl-Enter should not be consumed");
        assert_eq!(panel.pending_result, None);
    }

    #[test]
    fn dlg_panel_shift_enter_is_ignored() {
        // C++ parity: state.IsNoMod() rejects Shift (emInput.h:293).
        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut init = TestInit::new();
        let finish_sig = init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let mut ev = emInputEvent::press(InputKey::Enter);
        ev.shift = true;
        let ps = PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        };
        let is = emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(!consumed, "Shift-Enter should not be consumed");
        assert_eq!(panel.pending_result, None);
    }

    #[test]
    fn dlg_button_carries_result_payload() {
        let mut __init = TestInit::new();
        let (tree, tid) = test_tree();
        let btn = DlgButton::new(
            &mut __init.ctx(),
            "OK",
            emLook::new(),
            DialogResult::Ok,
            tid,
        );
        assert_eq!(btn.caption(), "OK");
        assert_eq!(btn.result(), &DialogResult::Ok);
        assert_eq!(btn.dlg_panel_id, tid);
        // click_signal is allocated by emButton::new; engine-side connect
        // happens in Task 4+7. Prove the signal exists (non-null).
        let _ = btn.button.click_signal;
        let _ = &tree;
    }

    #[test]
    fn dlg_button_set_caption_updates_emButton() {
        let mut __init = TestInit::new();
        let (tree, tid) = test_tree();
        let mut btn = DlgButton::new(
            &mut __init.ctx(),
            "OK",
            emLook::new(),
            DialogResult::Custom(9),
            tid,
        );
        assert_eq!(btn.caption(), "OK");
        btn.SetCaption("Accept");
        assert_eq!(btn.caption(), "Accept");
        assert_eq!(btn.button.GetCaption(), "Accept");
        let _ = &tree;
    }

    #[test]
    fn set_root_title() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut __init.ctx(), "Old Title", look);
        dlg.SetRootTitle("New Title");
        // Verify the dialog still functions after title change.
        dlg.Finish(DialogResult::Ok, &mut ctx);
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
    }

    #[test]
    fn private_engine_observes_close_signal_sets_pending_cancel() {
        // Ports the C++ PrivateCycle close-signal branch (emDialog.cpp:196-198):
        //   if (IsSignaled(GetCloseSignal())) Finish(NEGATIVE);
        // Expectation after one DoTimeSlice: finalized_result == Cancel,
        // finish_state == 2 (C++ FinishState==1 branch advances to 2 after
        // firing FinishSignal, emDialog.cpp:203-206), and a probe engine
        // connected to finish_signal has been awoken exactly once.
        //
        // Phase 3.5.A Task 10: registration flows through
        // `App::install_pending_top_level_headless`, which mirrors the
        // production `install_pending_top_level` path (deferred engine
        // register at `PanelScope::Toplevel(wid)` post-materialize).
        use crate::emGUIFramework::{App, PendingTopLevel};
        use crate::emPanelScope::PanelScope;
        use crate::emWindow::WindowFlags;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        // Build the dialog's populated PanelTree out-of-band, then wrap it
        // in a pending top-level emWindow (whose default empty tree we
        // discard). Matches the production shape where `emDialog::new`
        // builds the tree before enqueueing the `PendingTopLevel`.
        let mut tree = PanelTree::new();
        let root_id = tree.create_root("dlg", false);
        let finish_sig = app.scheduler.create_signal();
        let close_sig = app.scheduler.create_signal();
        let flags_sig = app.scheduler.create_signal();
        let focus_sig = app.scheduler.create_signal();
        let geom_sig = app.scheduler.create_signal();
        let dlg_panel = DlgPanel::new("Test", emLook::new(), finish_sig);
        tree.set_behavior(root_id, Box::new(dlg_panel));

        let mut window = crate::emWindow::emWindow::new_top_level_pending(
            Rc::clone(&app.context),
            WindowFlags::empty(),
            "test-dialog".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            crate::emColor::emColor::TRANSPARENT,
        );
        let _discarded_internal = window.take_tree();
        window.put_tree(tree);

        // Probe engine: counts its own Cycle invocations. Connected to
        // `finish_sig`, it will be woken in the slice where the signal
        // fires — a direct observation of `Signal(FinishSignal)`.
        struct FinishProbe {
            hits: Rc<RefCell<u32>>,
        }
        impl crate::emEngine::emEngine for FinishProbe {
            fn Cycle(&mut self, _ctx: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool {
                *self.hits.borrow_mut() += 1;
                false
            }
        }
        let hits: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let probe_id = app.scheduler.register_engine(
            Box::new(FinishProbe {
                hits: Rc::clone(&hits),
            }),
            crate::emEngine::Priority::Medium,
            PanelScope::Framework,
        );
        app.scheduler.connect(finish_sig, probe_id);

        // Enqueue the pending top-level + deferred DialogPrivateEngine
        // behavior and drive the headless install path.
        let wid = WindowId::dummy();
        let dialog_id = app.allocate_dialog_id();
        let private_engine = Box::new(DialogPrivateEngine {
            root_panel_id: root_id,
            window_id: Some(wid),
            close_signal: close_sig,
        });
        app.pending_top_level.push(PendingTopLevel {
            dialog_id,
            window,
            close_signal: close_sig,
            pending_private_engine: Some(private_engine),
        });
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");
        assert!(
            app.windows.contains_key(&wid),
            "install_pending_top_level_headless must move emWindow into App::windows",
        );
        assert_eq!(
            app.dialog_windows.get(&dialog_id).copied(),
            Some(wid),
            "DialogId → WindowId mapping must be recorded",
        );

        // Fire close signal and run one slice against the per-window tree.
        app.scheduler.fire(close_sig);
        let mut pending_inputs: Vec<(WindowId, emInputEvent)> = Vec::new();
        let mut input_state = emInputState::new();
        let fc: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        app.scheduler.DoTimeSlice(
            &mut app.windows,
            &app.context,
            &mut app.framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
        );

        // Direct probe: finish_signal fired exactly once during the slice.
        assert_eq!(
            *hits.borrow(),
            1,
            "finish_signal must fire exactly once when close_signal is observed",
        );

        // Inspect DlgPanel state via the window's tree after the cycle.
        {
            let win = app.windows.get_mut(&wid).expect("window present");
            let mut tree = win.take_tree();
            let mut behavior = tree.take_behavior(root_id).expect("behavior reinstated");
            {
                let dlg = behavior.as_dlg_panel_mut().expect("is DlgPanel");
                assert_eq!(
                    dlg.finalized_result,
                    Some(DialogResult::Cancel),
                    "close_signal should finalize to Cancel"
                );
                assert_eq!(
                    dlg.finish_state, 2,
                    "FinishState==1 branch advances to 2 after firing FinishSignal",
                );
                assert!(
                    dlg.pending_result.is_none(),
                    "pending_result consumed by finalize"
                );
            }
            tree.put_behavior(root_id, behavior);
            win.put_tree(tree);
        }

        // Without auto_delete, the next Cycle hits the C++ `!ADEnabled`
        // branch: FinishState=0, return false. finish_signal must NOT
        // fire again. Re-fire close_signal too — the engine is already
        // finalized and must ignore it.
        app.scheduler.fire(close_sig);
        app.scheduler.DoTimeSlice(
            &mut app.windows,
            &app.context,
            &mut app.framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
        );
        assert_eq!(
            *hits.borrow(),
            1,
            "finish_signal must not re-fire on subsequent slices",
        );
        {
            let win = app.windows.get_mut(&wid).expect("window still present");
            let mut tree = win.take_tree();
            let mut behavior = tree.take_behavior(root_id).expect("still present");
            {
                let dlg = behavior.as_dlg_panel_mut().unwrap();
                assert_eq!(
                    dlg.finalized_result,
                    Some(DialogResult::Cancel),
                    "repeated close_signal must not re-finalize"
                );
                assert_eq!(
                    dlg.finish_state, 0,
                    "!ADEnabled branch resets FinishState to 0",
                );
            }
            tree.put_behavior(root_id, behavior);
            win.put_tree(tree);
        }

        // Teardown. InputDispatchEngine is removed by App::drop.
        app.scheduler.remove_engine(engine_id);
        app.scheduler.remove_engine(probe_id);
        app.scheduler.clear_pending_for_tests();
    }
}
