use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::emColor::emColor;
use crate::emCursor::emCursor;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emLinearLayout::emLinearLayout;
use crate::emPainter::{emPainter, BORDER_EDGES_ONLY};
use crate::emPanel::PanelState;
use crate::emPanel::Rect;
use crate::emRasterLayout::emRasterLayout;

use super::emBorder::{emBorder, OuterBorderType};
use crate::emBorder::with_toolkit_images;
use crate::emButton::{HOWTO_BUTTON, HOWTO_EOI_BUTTON};
use crate::emEngineCtx::{ConstructCtx, PanelCtx, WidgetCallback};
use crate::emLook::emLook;
use crate::emSignal::SignalId;

/// Shared state for a group of radio buttons enforcing mutual exclusion.
///
/// This is the Rust equivalent of C++ `emRadioButton::Mechanism`. It tracks
/// button membership via shared index cells and manages the checked state with
/// recursion-safe logic matching the C++ `SetCheckIndex` implementation.
pub struct RadioGroup {
    /// Index of the currently checked button, or `None`.
    selected: Option<usize>,
    /// Live index cells for each registered button, enabling re-indexing on removal.
    buttons: Vec<Rc<Cell<usize>>>,
    pub on_select: Option<WidgetCallback<Option<usize>>>,
    /// Allocated at construction per C++ `emRadioButton::Mechanism::GetCheckSignal()`.
    /// B3.4b allocates; B3.4c fires on selection change.
    pub check_signal: SignalId,
}

impl RadioGroup {
    pub fn new<C: ConstructCtx>(ctx: &mut C) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            selected: None,
            buttons: Vec::new(),
            on_select: None,
            check_signal: ctx.create_signal(),
        }))
    }

    pub fn GetChecked(&self) -> Option<usize> {
        self.selected
    }

    /// Number of radio buttons currently in this group.
    pub fn GetCount(&self) -> usize {
        self.buttons.len()
    }

    pub fn is_valid_index(&self, index: usize) -> bool {
        index < self.buttons.len()
    }

    /// Select the button at `index`, unchecking any previously selected button.
    /// No-op if already selected (matches C++ recursion guard / no-change check).
    /// Mirrors C++ `Mechanism::SetCheckIndex` (emRadioButton.cpp:112-115):
    /// `CheckSignal.Signal(*scheduler)` → `CheckChanged`.
    pub fn SetChecked(&mut self, index: usize, ctx: &mut PanelCtx<'_>) {
        if self.selected == Some(index) {
            return;
        }
        self.selected = Some(index);
        if let Some(mut sched) = ctx.as_sched_ctx() {
            sched.fire(self.check_signal);
            if let Some(cb) = self.on_select.as_mut() {
                cb(Some(index), &mut sched);
            }
        }
    }

    /// Set the check index directly, or clear the selection with `None`.
    /// Matches C++ `Mechanism::SetCheckIndex`. When index is out of bounds
    /// (>= count), the selection is cleared.
    pub fn SetCheckIndex(&mut self, index: Option<usize>, ctx: &mut PanelCtx<'_>) {
        let normalized = match index {
            Some(i) if i < self.buttons.len() => Some(i),
            _ => None,
        };
        if self.selected == normalized {
            return;
        }
        self.selected = normalized;
        if let Some(mut sched) = ctx.as_sched_ctx() {
            sched.fire(self.check_signal);
            if let Some(cb) = self.on_select.as_mut() {
                cb(normalized, &mut sched);
            }
        }
    }

    /// Remove the button at `index` from the group.
    ///
    /// Re-indexes remaining buttons (buttons with index > removed index have
    /// their logical index decremented). If the removed button was checked,
    /// clears the selection. If the checked button had a higher index, its
    /// index is decremented to match the new layout.
    ///
    /// Matches C++ `Mechanism::RemoveByIndex`. Fires CheckSignal + on_select
    /// callback if the selection changed as a result of the removal.
    pub fn RemoveByIndex(&mut self, index: usize, ctx: &mut PanelCtx<'_>) {
        if index >= self.buttons.len() {
            return;
        }
        self.buttons.remove(index);

        // Decrement all cells with index > removed_index
        for c in &self.buttons {
            if c.get() > index {
                c.set(c.get() - 1);
            }
        }

        let (selection_changed, new_selection) = if let Some(check_idx) = self.selected {
            if check_idx == index {
                self.selected = None;
                (true, None)
            } else if check_idx > index {
                self.selected = Some(check_idx - 1);
                (true, self.selected)
            } else {
                (false, self.selected)
            }
        } else {
            (false, None)
        };

        if selection_changed {
            if let Some(mut sched) = ctx.as_sched_ctx() {
                sched.fire(self.check_signal);
                if let Some(cb) = self.on_select.as_mut() {
                    cb(new_selection, &mut sched);
                }
            }
        }
    }

    /// Register a new button in the group, returning a shared index cell.
    pub fn register(&mut self) -> Rc<Cell<usize>> {
        let idx = self.buttons.len();
        let cell = Rc::new(Cell::new(idx));
        self.buttons.push(cell.clone());
        cell
    }

    /// Deregister a button from the group by its shared index cell.
    ///
    /// Re-indexes remaining buttons and adjusts selection, matching
    /// C++ `Mechanism::RemoveByIndex` behaviour.
    pub fn deregister(&mut self, cell: &Rc<Cell<usize>>) {
        let removed_index = cell.get();
        // Remove the matching cell by pointer identity
        self.buttons.retain(|c| !Rc::ptr_eq(c, cell));
        // Decrement all cells with index > removed_index
        for c in &self.buttons {
            if c.get() > removed_index {
                c.set(c.get() - 1);
            }
        }
        // Adjust selection
        match self.selected {
            Some(s) if s == removed_index => {
                self.selected = None;
                // C++ parity: `Mechanism::RemoveByIndex` fires CheckSignal
                // when a checked button is removed, but Rust's `deregister`
                // is invoked from the emRadioButton `Drop` impl which has
                // no scheduler reach — matching C++ destructor flow, where
                // `delete emRadioButton` path does not invoke the signal
                // (buttons are destroyed during panel teardown). Callers
                // that need a signal fire on explicit removal use the
                // ctx-bearing `RemoveByIndex` instead.
            }
            Some(s) if s > removed_index => {
                self.selected = Some(s - 1);
            }
            _ => {}
        }
    }

    /// Add multiple buttons to the group at once.
    ///
    /// Port of C++ `emRadioButton::Mechanism::AddAll(emPanel* parent)`.
    /// In C++, this iterates panel children and dynamic_casts to emRadioButton.
    /// In Rust, since buttons register themselves in `emRadioButton::new()`,
    /// this method registers `n` additional button slots for buttons that
    /// were created outside the normal constructor flow.
    pub fn AddAll(&mut self, n: usize) {
        let base = self.buttons.len();
        for i in 0..n {
            self.buttons.push(Rc::new(Cell::new(base + i)));
        }
    }

    /// Get the button index at the given position in the group.
    ///
    /// Port of C++ `emRadioButton::Mechanism::GetButton(int)`.
    /// In C++, returns a pointer to the emRadioButton at `index`.
    /// In Rust, validates the index and returns it (since buttons are
    /// identified by their index in the group).
    pub fn GetButton(&self, index: usize) -> Option<usize> {
        if index < self.buttons.len() {
            Some(index)
        } else {
            None
        }
    }

    /// Find the index of a radio button by its identifier.
    ///
    /// In C++ this searches by pointer; in Rust buttons are identified by
    /// their index, so this validates the index is within bounds.
    ///
    /// Port of C++ `emRadioButton::Mechanism::GetIndexOf`.
    pub fn GetIndexOf(&self, id: usize) -> Option<usize> {
        if id < self.buttons.len() {
            Some(id)
        } else {
            None
        }
    }

    /// Remove all buttons from the group.
    ///
    /// If a button was checked, clears the selection and fires the signal.
    /// Individual buttons' checked states are NOT modified (matching C++
    /// `Mechanism::RemoveAll`).
    pub fn RemoveAll(&mut self, ctx: &mut PanelCtx<'_>) {
        let had_selection = self.selected.is_some();
        self.buttons.clear();
        if had_selection {
            self.selected = None;
            if let Some(mut sched) = ctx.as_sched_ctx() {
                sched.fire(self.check_signal);
                if let Some(cb) = self.on_select.as_mut() {
                    cb(None, &mut sched);
                }
            }
        }
    }
}

/// Radio button widget -- mutually exclusive selection within a group.
pub struct emRadioButton {
    border: emBorder,
    look: Rc<emLook>,
    group: Rc<RefCell<RadioGroup>>,
    index_cell: Rc<Cell<usize>>,
    pressed: bool,
    /// Cached enabled state from the last paint call. Gates input handling.
    enabled: bool,
    last_w: f64,
    last_h: f64,
}

impl emRadioButton {
    pub fn new(
        caption: &str,
        look: Rc<emLook>,
        group: Rc<RefCell<RadioGroup>>,
        _index: usize,
    ) -> Self {
        let index_cell = group.borrow_mut().register();
        let mut border = emBorder::new(OuterBorderType::InstrumentMoreRound)
            .with_caption(caption)
            .with_label_in_border(false)
            .with_how_to(true);
        // C++ emButton constructor: SetLabelAlignment(EM_ALIGN_CENTER)
        border.SetLabelAlignment(crate::emPainter::TextAlignment::Center);
        Self {
            border,
            look,
            group,
            index_cell,
            pressed: false,
            enabled: true,
            last_w: 0.0,
            last_h: 0.0,
        }
    }

    /// The index of this button within its group.
    pub fn index(&self) -> usize {
        self.index_cell.get()
    }

    /// Update the index (used after `remove_by_index` re-indexes the group).
    pub fn set_index(&mut self, index: usize) {
        self.index_cell.set(index);
    }

    pub fn IsSelected(&self) -> bool {
        self.group.borrow().selected == Some(self.index_cell.get())
    }

    /// Set the checked state of this radio button, synchronizing with the
    /// group mechanism.
    ///
    /// Matches C++ `emRadioButton::CheckChanged` behaviour:
    /// - If `checked` is true, tells the mechanism to select this button
    ///   (unchecking any previously selected button).
    /// - If `checked` is false and this button is currently selected in the
    ///   mechanism, clears the mechanism's selection.
    pub fn set_checked(&mut self, checked: bool, ctx: &mut PanelCtx<'_>) {
        if checked {
            self.group
                .borrow_mut()
                .SetChecked(self.index_cell.get(), ctx);
        } else if self.IsSelected() {
            self.group.borrow_mut().SetCheckIndex(None, ctx);
        }
    }

    /// Paint using the C++ DoButton non-boxed path (emButton.cpp:343-421).
    ///
    /// emRadioButton renders as a normal button (face + centered label).
    /// ShownRadioed=true only affects which border image is used (ButtonChecked
    /// vs ButtonPressed vs Button). ShownBoxed is false for radio buttons.
    pub fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        enabled: bool,
        pixel_scale: f64,
    ) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;
        self.border.how_to_text = self.GetHowTo(enabled, true);
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            false,
            true,
            pixel_scale,
        );
        let canvas_color = self
            .border
            .content_canvas_color(canvas_color, &self.look, enabled);

        // C++ DoButton non-boxed path: GetContentRoundRect, clamp r.
        let (cr, r) = self.border.GetContentRoundRect(w, h, &self.look);
        let r = r.max(cr.w.min(cr.h) * self.border.border_scaling * 0.223);

        // Face inset: d = (1 - (264-14)/264) * r = (14/264) * r (C++ line 348).
        let d = (14.0 / 264.0) * r;
        let fx = cr.x + d;
        let fy = cr.y + d;
        let fw = cr.w - 2.0 * d;
        let fh = cr.h - 2.0 * d;
        let fr = r - d;

        // C++ emButton.cpp:361: ButtonBgColor for non-boxed path.
        let face_color = self.look.button_bg_color;
        painter.PaintRoundRect(fx, fy, fw, fh, fr, fr, face_color, canvas_color);

        // Label inside face with padding (C++ lines 370-391).
        let d_min = fw.min(fh) * 0.1;
        let dx = (r * 0.7).max(d_min);
        let dy = (r * 0.4).max(d_min);
        let mut lx = fx + dx;
        let mut ly = fy + dy;
        let mut lw = fw - 2.0 * dx;
        let mut lh = fh - 2.0 * dy;

        let checked = self.IsSelected();
        // C++ line 377-382: Pressed -> 0.98, ShownChecked -> 0.983.
        if self.pressed || checked {
            let s = if self.pressed { 0.98 } else { 0.983 };
            lx += (1.0 - s) * 0.5 * lw;
            lw *= s;
            ly += (1.0 - s) * 0.5 * lh;
            lh *= s;
        }
        let label_color = if enabled {
            self.look.button_fg_color
        } else {
            self.look.button_fg_color.GetTransparented(75.0)
        };
        self.border.paint_label_colored(
            painter,
            face_color,
            Rect::new(lx, ly, lw, lh),
            &self.look,
            label_color,
            true,
        );

        // Border image overlay (C++ lines 393-421).
        with_toolkit_images(|img| {
            if self.pressed {
                painter.PaintBorderImage(
                    cr.x,
                    cr.y,
                    cr.w,
                    cr.h,
                    360.0 / 264.0 * r,
                    374.0 / 264.0 * r,
                    r,
                    r,
                    &img.button_pressed,
                    360,
                    374,
                    264,
                    264,
                    255,
                    emColor::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            } else if checked {
                painter.PaintBorderImage(
                    cr.x,
                    cr.y,
                    cr.w,
                    cr.h,
                    340.0 / 264.0 * r,
                    374.0 / 264.0 * r,
                    r,
                    r,
                    &img.button_checked,
                    340,
                    374,
                    264,
                    264,
                    255,
                    emColor::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            } else {
                let extra = (658.0 - 648.0) / 264.0 * r;
                painter.PaintBorderImage(
                    cr.x,
                    cr.y,
                    cr.w + extra,
                    cr.h + extra,
                    278.0 / 264.0 * r,
                    278.0 / 264.0 * r,
                    278.0 / 264.0 * r,
                    278.0 / 264.0 * r,
                    &img.button,
                    278,
                    278,
                    278,
                    278,
                    255,
                    emColor::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            }
        });
    }

    /// Hit test matching C++ `emButton::CheckMouse` non-boxed path (emButton.cpp:354-359).
    fn hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let (cr, r) = self.border.GetContentRoundRect(1.0, tallness, &self.look);
        let r = r.max(cr.w.min(cr.h) * self.border.border_scaling * 0.223);
        let d = (14.0 / 264.0) * r;
        let face = Rect::new(cr.x + d, cr.y + d, cr.w - 2.0 * d, cr.h - 2.0 * d);
        let fr = r - d;
        let dx = ((face.x - mx).max(mx - face.x - face.w) + fr).max(0.0);
        let dy = ((face.y - my).max(my - face.y - face.h) + fr).max(0.0);
        dx * dx + dy * dy <= fr * fr
    }

    pub fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        _input_state: &emInputState,
        ctx: &mut crate::emEngineCtx::PanelCtx,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        // RUST_ONLY: (language-forced-utility) widget_utils.rs -- debug trace aid, no C++ equivalent
        let trace = {
            use std::sync::OnceLock;
            static ENABLED: OnceLock<bool> = OnceLock::new();
            *ENABLED.get_or_init(|| std::env::var("TRACE_INPUT").is_ok())
        };
        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    // C++ emButton.cpp:82: (state.IsNoMod() || state.IsShiftMod())
                    if event.ctrl || event.alt || event.meta {
                        return false;
                    }
                    // C++ emButton.cpp:84: GetViewCondition(VCT_MIN_EXT) >= 8.0
                    let min_ext = state.viewed_rect.w.min(state.viewed_rect.h);
                    if min_ext < 8.0 {
                        return false;
                    }
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        eprintln!(
                            "    [RadioButton {:?}] Press mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed
                        );
                    }
                    if !hit {
                        return false;
                    }
                    self.pressed = true;
                    true
                }
                InputVariant::Release => {
                    if !self.pressed {
                        return false;
                    }
                    // C++ emButton.cpp:101: IsViewed check on release.
                    if !state.viewed {
                        self.pressed = false;
                        return true;
                    }
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        eprintln!(
                            "    [RadioButton {:?}] Release mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed={} selected_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed, self.IsSelected()
                        );
                    }
                    self.pressed = false;
                    if hit {
                        self.group
                            .borrow_mut()
                            .SetChecked(self.index_cell.get(), ctx);
                    }
                    true
                }
                _ => false,
            },
            // C++ emButton.cpp:113-119: Enter only, instant Click().
            // Gated on (IsNoMod || IsShiftMod).
            InputKey::Enter
                if event.variant == InputVariant::Press
                    && !event.alt
                    && !event.meta
                    && !event.ctrl
                    && state.viewed_rect.w.min(state.viewed_rect.h) >= 8.0 =>
            {
                self.group
                    .borrow_mut()
                    .SetChecked(self.index_cell.get(), ctx);
                true
            }
            _ => false,
        }
    }

    pub fn GetCursor(&self) -> emCursor {
        emCursor::Normal
    }

    /// Whether this radio button provides how-to help text.
    /// Matches C++ `emRadioButton::HasHowTo` (inherited, always true).
    pub fn HasHowTo(&self) -> bool {
        true
    }

    /// Help text describing how to use this radio button.
    ///
    /// Chains the border's base how-to with check-button + radio-button
    /// specific sections. Matches C++ `emRadioButton::GetHowTo`.
    pub fn GetHowTo(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.GetHowTo(enabled, focusable);
        text.push_str(HOWTO_BUTTON);
        text.push_str(HOWTO_EOI_BUTTON);
        text.push_str(HOWTO_CHECK_BUTTON);
        if self.IsSelected() {
            text.push_str(HOWTO_CHECKED);
        } else {
            text.push_str(HOWTO_NOT_CHECKED);
        }
        text.push_str(HOWTO_RADIO_BUTTON);
        text
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let th = 13.0;
        let tw = emPainter::measure_text_width(&self.border.caption, th);
        self.border.preferred_size_for_content(tw + 8.0, th + 4.0)
    }
}

/// C++ `emCheckButton::HowToCheckButton` (shared with check button).
const HOWTO_CHECK_BUTTON: &str = "\n\n\
    CHECK BUTTON\n\n\
    This button can have checked or unchecked state. Usually this is a yes-or-no\n\
    answer to a question. Whenever the button is triggered, the check state toggles.\n";

/// C++ `emCheckButton::HowToChecked`.
const HOWTO_CHECKED: &str = "\n\n\
    CHECKED\n\n\
    Currently this check button is checked.\n";

/// C++ `emCheckButton::HowToNotChecked`.
const HOWTO_NOT_CHECKED: &str = "\n\n\
    UNCHECKED\n\n\
    Currently this check button is not checked.\n";

/// C++ `emRadioButton::HowToRadioButton`.
const HOWTO_RADIO_BUTTON: &str = "\n\n\
    RADIO BUTTON\n\n\
    This is a radio button. It is a check button with changed behavior: In a set of\n\
    radio buttons, only one button can have checked state. When triggering a radio\n\
    button, that button is checked and all the other radio buttons of the set are\n\
    unchecked. There is no way to uncheck a radio button directly.\n";

pub struct RadioLinearGroup {
    pub layout: emLinearLayout,
    pub group: Rc<RefCell<RadioGroup>>,
}

impl RadioLinearGroup {
    pub fn horizontal<C: ConstructCtx>(ctx: &mut C) -> Self {
        Self {
            layout: emLinearLayout::horizontal(),
            group: RadioGroup::new(ctx),
        }
    }

    pub fn vertical<C: ConstructCtx>(ctx: &mut C) -> Self {
        Self {
            layout: emLinearLayout::vertical(),
            group: RadioGroup::new(ctx),
        }
    }
}

pub struct RadioRasterGroup {
    pub layout: emRasterLayout,
    pub group: Rc<RefCell<RadioGroup>>,
}

impl RadioRasterGroup {
    pub fn new<C: ConstructCtx>(ctx: &mut C) -> Self {
        Self {
            layout: emRasterLayout::default(),
            group: RadioGroup::new(ctx),
        }
    }
}

impl Drop for emRadioButton {
    fn drop(&mut self) {
        self.group.borrow_mut().deregister(&self.index_cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngineCtx::{DeferredAction, InitCtx, PanelCtx};
    use crate::emPanel::Rect;
    use crate::emPanelTree::{PanelId, PanelTree};
    use crate::emScheduler::EngineScheduler;
    use slotmap::Key as _;

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

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<crate::emContext::emContext>,
        pa: Rc<RefCell<Vec<crate::emEngineCtx::FrameworkDeferredAction>>>,
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
                pa: Rc::new(RefCell::new(Vec::new())),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
                pending_actions: &self.pa,
            }
        }
    }

    #[test]
    fn radio_group_mutual_exclusion() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());

        let mut r0 = emRadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = emRadioButton::new("B", look.clone(), group.clone(), 1);
        let mut r2 = emRadioButton::new("C", look, group.clone(), 2);
        let ps = default_panel_state();
        let is = default_input_state();

        assert!(!r0.IsSelected());
        assert!(!r1.IsSelected());
        assert!(!r2.IsSelected());

        // Enter is instant: selects on press, no release needed.
        r0.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(r0.IsSelected()); // Selected immediately on press
        assert!(!r1.IsSelected());

        r2.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!r0.IsSelected());
        assert!(r2.IsSelected());

        r1.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!r0.IsSelected());
        assert!(r1.IsSelected());
        assert!(!r2.IsSelected());
    }

    #[test]
    fn pressed_state_tracks_press_release() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        // Enter is instant -- no visual press state. Verify pressed stays false.
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());
        let mut r0 = emRadioButton::new("A", look, group.clone(), 0);
        let ps = default_panel_state();
        let is = default_input_state();
        assert!(!r0.pressed);
        r0.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!r0.pressed); // Enter selects instantly, no press state
        assert!(r0.IsSelected()); // But the selection did happen
    }

    #[test]
    fn radio_group_callback() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let group = RadioGroup::new(&mut __init.ctx());
        let selections = Rc::new(RefCell::new(Vec::new()));
        let sel_clone = selections.clone();
        group.borrow_mut().on_select = Some(Box::new(
            move |idx, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                sel_clone.borrow_mut().push(idx);
            },
        ));

        let look = emLook::new();
        let mut r0 = emRadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = emRadioButton::new("B", look, group.clone(), 1);
        let ps = default_panel_state();
        let is = default_input_state();

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
                &__init.pa,
            );
            // Enter is instant: each press fires the callback immediately.
            r0.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
            r1.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        }
        assert_eq!(*selections.borrow(), vec![Some(0), Some(1)]);
    }

    #[test]
    fn radio_group_fires_check_signal_on_input() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let group = RadioGroup::new(&mut __init.ctx());
        let sig = group.borrow().check_signal;
        let look = emLook::new();
        let mut r0 = emRadioButton::new("A", look, group.clone(), 0);
        let ps = default_panel_state();
        let is = default_input_state();
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
                &__init.pa,
            );
            r0.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        }
        assert!(__init.sched.is_pending(sig));
    }

    #[test]
    fn count_tracks_construction_and_drop() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());
        assert_eq!(group.borrow().GetCount(), 0);

        let r0 = emRadioButton::new("A", look.clone(), group.clone(), 0);
        assert_eq!(group.borrow().GetCount(), 1);

        let r1 = emRadioButton::new("B", look.clone(), group.clone(), 1);
        assert_eq!(group.borrow().GetCount(), 2);

        drop(r0);
        assert_eq!(group.borrow().GetCount(), 1);

        drop(r1);
        assert_eq!(group.borrow().GetCount(), 0);
    }

    #[test]
    fn index_returns_correct_value() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());
        let r0 = emRadioButton::new("A", look.clone(), group.clone(), 0);
        let r1 = emRadioButton::new("B", look, group.clone(), 1);
        assert_eq!(r0.index(), 0);
        assert_eq!(r1.index(), 1);
    }

    // --- New tests for D-WIDGET-08 ---

    #[test]
    fn set_checked_selects_in_group() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());
        let mut r0 = emRadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = emRadioButton::new("B", look, group.clone(), 1);

        // set_checked(true) selects this button
        r0.set_checked(true, &mut ctx);
        assert!(r0.IsSelected());
        assert!(!r1.IsSelected());
        assert_eq!(group.borrow().GetChecked(), Some(0));

        // set_checked(true) on another button switches selection
        r1.set_checked(true, &mut ctx);
        assert!(!r0.IsSelected());
        assert!(r1.IsSelected());
        assert_eq!(group.borrow().GetChecked(), Some(1));

        // set_checked(false) on the selected button clears selection
        r1.set_checked(false, &mut ctx);
        assert!(!r0.IsSelected());
        assert!(!r1.IsSelected());
        assert_eq!(group.borrow().GetChecked(), None);
    }

    #[test]
    fn set_checked_false_on_unselected_is_noop() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());
        let mut r0 = emRadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = emRadioButton::new("B", look, group.clone(), 1);

        r0.set_checked(true, &mut ctx);
        assert_eq!(group.borrow().GetChecked(), Some(0));

        // set_checked(false) on a non-selected button does nothing
        r1.set_checked(false, &mut ctx);
        assert_eq!(group.borrow().GetChecked(), Some(0));
        assert!(r0.IsSelected());
    }

    #[test]
    fn remove_by_index_clears_checked() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let group = RadioGroup::new(&mut __init.ctx());
        {
            let mut g = group.borrow_mut();
            g.AddAll(3);
            g.SetChecked(1, &mut ctx); // button at index 1 is checked
        }

        // Remove the checked button
        group.borrow_mut().RemoveByIndex(1, &mut ctx);
        assert_eq!(group.borrow().GetCount(), 2);
        assert_eq!(group.borrow().GetChecked(), None);
    }

    #[test]
    fn remove_by_index_decrements_checked() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let group = RadioGroup::new(&mut __init.ctx());
        {
            let mut g = group.borrow_mut();
            g.AddAll(4);
            g.SetChecked(3, &mut ctx); // button at index 3 is checked
        }

        // Remove button at index 1 (before the checked one)
        group.borrow_mut().RemoveByIndex(1, &mut ctx);
        assert_eq!(group.borrow().GetCount(), 3);
        // Checked index should have decremented from 3 to 2
        assert_eq!(group.borrow().GetChecked(), Some(2));
    }

    #[test]
    fn remove_by_index_no_change_when_checked_before() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let group = RadioGroup::new(&mut __init.ctx());
        {
            let mut g = group.borrow_mut();
            g.AddAll(4);
            g.SetChecked(0, &mut ctx); // button at index 0 is checked
        }

        // Remove button at index 2 (after the checked one)
        group.borrow_mut().RemoveByIndex(2, &mut ctx);
        assert_eq!(group.borrow().GetCount(), 3);
        assert_eq!(group.borrow().GetChecked(), Some(0));
    }

    #[test]
    fn remove_by_index_out_of_bounds_is_noop() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let group = RadioGroup::new(&mut __init.ctx());
        {
            let mut g = group.borrow_mut();
            g.AddAll(2);
            g.SetChecked(0, &mut ctx);
        }
        group.borrow_mut().RemoveByIndex(5, &mut ctx);
        assert_eq!(group.borrow().GetCount(), 2);
        assert_eq!(group.borrow().GetChecked(), Some(0));
    }

    #[test]
    fn remove_by_index_fires_callback() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let group = RadioGroup::new(&mut __init.ctx());
        let signals = Rc::new(RefCell::new(Vec::new()));
        let sig_clone = signals.clone();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        let mut ctx = PanelCtx::with_sched_reach(
            &mut tree,
            tid,
            1.0,
            &mut __init.sched,
            &mut __init.fw,
            &__init.root,
            &fw_cb,
            &__init.pa,
        );
        {
            let mut g = group.borrow_mut();
            g.AddAll(3);
            g.SetChecked(1, &mut ctx);
            g.on_select = Some(Box::new(
                move |idx, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                    sig_clone.borrow_mut().push(idx);
                },
            ));
        }

        // Remove checked button -- should fire callback with None
        group.borrow_mut().RemoveByIndex(1, &mut ctx);
        assert_eq!(*signals.borrow(), vec![None]);
    }

    #[test]
    fn remove_all_clears_everything() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let group = RadioGroup::new(&mut __init.ctx());
        let signals = Rc::new(RefCell::new(Vec::new()));
        let sig_clone = signals.clone();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        let mut ctx = PanelCtx::with_sched_reach(
            &mut tree,
            tid,
            1.0,
            &mut __init.sched,
            &mut __init.fw,
            &__init.root,
            &fw_cb,
            &__init.pa,
        );
        {
            let mut g = group.borrow_mut();
            g.AddAll(3);
            g.SetChecked(1, &mut ctx);
            g.on_select = Some(Box::new(
                move |idx, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                    sig_clone.borrow_mut().push(idx);
                },
            ));
        }

        group.borrow_mut().RemoveAll(&mut ctx);
        assert_eq!(group.borrow().GetCount(), 0);
        assert_eq!(group.borrow().GetChecked(), None);
        assert_eq!(*signals.borrow(), vec![None]);
    }

    #[test]
    fn remove_all_no_signal_if_nothing_checked() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let group = RadioGroup::new(&mut __init.ctx());
        let signals = Rc::new(RefCell::new(Vec::new()));
        let sig_clone = signals.clone();
        {
            let mut g = group.borrow_mut();
            g.AddAll(3);
            // No selection
            g.on_select = Some(Box::new(
                move |idx, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                    sig_clone.borrow_mut().push(idx);
                },
            ));
        }

        group.borrow_mut().RemoveAll(&mut ctx);
        assert_eq!(group.borrow().GetCount(), 0);
        assert!(signals.borrow().is_empty());
    }

    #[test]
    fn set_check_index_out_of_bounds_clears() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let group = RadioGroup::new(&mut __init.ctx());
        {
            let mut g = group.borrow_mut();
            g.AddAll(2);
            g.SetChecked(0, &mut ctx);
        }

        // Out of bounds normalizes to None
        group.borrow_mut().SetCheckIndex(Some(5), &mut ctx);
        assert_eq!(group.borrow().GetChecked(), None);
    }

    #[test]
    fn set_check_index_same_is_noop() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let group = RadioGroup::new(&mut __init.ctx());
        let signals = Rc::new(RefCell::new(Vec::new()));
        let sig_clone = signals.clone();
        {
            let mut g = group.borrow_mut();
            g.AddAll(3);
            g.SetChecked(1, &mut ctx);
            g.on_select = Some(Box::new(
                move |idx, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                    sig_clone.borrow_mut().push(idx);
                },
            ));
        }

        // Setting same index is a no-op
        group.borrow_mut().SetCheckIndex(Some(1), &mut ctx);
        assert!(signals.borrow().is_empty());
    }

    #[test]
    fn drop_middle_button_reindexes_remaining() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());

        let r0 = emRadioButton::new("A", look.clone(), group.clone(), 0);
        let r1 = emRadioButton::new("B", look.clone(), group.clone(), 1);
        let r2 = emRadioButton::new("C", look, group.clone(), 2);

        // Select the last button
        group.borrow_mut().SetChecked(2, &mut ctx);
        assert!(r2.IsSelected());
        assert_eq!(r2.index(), 2);

        // Drop the middle button
        drop(r1);

        // r2's index should have been decremented
        assert_eq!(r2.index(), 1);
        assert_eq!(group.borrow().GetCount(), 2);
        // Selection should have shifted from 2 to 1
        assert_eq!(group.borrow().GetChecked(), Some(1));
        assert!(r2.IsSelected());
        assert!(!r0.IsSelected());
    }

    #[test]
    fn drop_selected_button_clears_selection() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());

        let r0 = emRadioButton::new("A", look.clone(), group.clone(), 0);
        let r1 = emRadioButton::new("B", look.clone(), group.clone(), 1);
        let r2 = emRadioButton::new("C", look, group.clone(), 2);

        group.borrow_mut().SetChecked(1, &mut ctx);
        assert!(r1.IsSelected());

        drop(r1);

        assert_eq!(group.borrow().GetCount(), 2);
        assert_eq!(group.borrow().GetChecked(), None);
        assert!(!r0.IsSelected());
        assert!(!r2.IsSelected());
        // r2's index should have been decremented
        assert_eq!(r2.index(), 1);
    }
}
