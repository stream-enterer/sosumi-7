use std::cell::RefCell;
use std::rc::Rc;

use crate::foundation::{Color, Rect};
use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::layout::linear::LinearLayout;
use crate::layout::raster::RasterLayout;
use crate::render::{Painter, BORDER_EDGES_ONLY};

use super::border::{Border, OuterBorderType};
use super::look::Look;
use super::toolkit_images::with_toolkit_images;

/// Shared state for a group of radio buttons enforcing mutual exclusion.
///
/// This is the Rust equivalent of C++ `emRadioButton::Mechanism`. It tracks
/// button membership via indices and manages the checked state with
/// recursion-safe logic matching the C++ `SetCheckIndex` implementation.
pub struct RadioGroup {
    /// Index of the currently checked button, or `None`.
    selected: Option<usize>,
    /// Number of radio buttons currently registered in this group.
    count: usize,
    pub on_select: Option<Box<dyn FnMut(Option<usize>)>>,
}

impl RadioGroup {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            selected: None,
            count: 0,
            on_select: None,
        }))
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Number of radio buttons currently in this group.
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn is_valid_index(&self, index: usize) -> bool {
        index < self.count
    }

    /// Select the button at `index`, unchecking any previously selected button.
    pub fn select(&mut self, index: usize) {
        self.selected = Some(index);
        if let Some(cb) = &mut self.on_select {
            cb(Some(index));
        }
    }

    /// Set the check index directly, or clear the selection with `None`.
    ///
    /// Matches C++ `Mechanism::SetCheckIndex`. When index is out of bounds
    /// (>= count), the selection is cleared.
    pub fn set_check_index(&mut self, index: Option<usize>) {
        let normalized = match index {
            Some(i) if i < self.count => Some(i),
            _ => None,
        };
        if self.selected == normalized {
            return;
        }
        self.selected = normalized;
        if let Some(cb) = &mut self.on_select {
            cb(normalized);
        }
    }

    /// Remove the button at `index` from the group.
    ///
    /// Re-indexes remaining buttons (buttons with index > removed index have
    /// their logical index decremented). If the removed button was checked,
    /// clears the selection. If the checked button had a higher index, its
    /// index is decremented to match the new layout.
    ///
    /// Matches C++ `Mechanism::RemoveByIndex`.
    pub fn remove_by_index(&mut self, index: usize) {
        if index >= self.count {
            return;
        }
        self.count -= 1;

        let selection_changed = if let Some(check_idx) = self.selected {
            if check_idx == index {
                // Removed the checked button
                self.selected = None;
                true
            } else if check_idx > index {
                // Checked button shifted down
                self.selected = Some(check_idx - 1);
                true
            } else {
                false
            }
        } else {
            false
        };

        if selection_changed {
            if let Some(cb) = &mut self.on_select {
                cb(self.selected);
            }
        }
    }

    /// Add multiple buttons to the group at once.
    ///
    /// Port of C++ `emRadioButton::Mechanism::AddAll(emPanel* parent)`.
    /// In C++, this iterates panel children and dynamic_casts to RadioButton.
    /// In Rust, since buttons register themselves in `RadioButton::new()`,
    /// this method registers `n` additional button slots for buttons that
    /// were created outside the normal constructor flow.
    pub fn add_all(&mut self, n: usize) {
        self.count += n;
    }

    /// Get the button index at the given position in the group.
    ///
    /// Port of C++ `emRadioButton::Mechanism::GetButton(int)`.
    /// In C++, returns a pointer to the RadioButton at `index`.
    /// In Rust, validates the index and returns it (since buttons are
    /// identified by their index in the group).
    pub fn get_button(&self, index: usize) -> Option<usize> {
        if index < self.count {
            Some(index)
        } else {
            None
        }
    }

    /// Remove all buttons from the group.
    ///
    /// If a button was checked, clears the selection and fires the signal.
    /// Individual buttons' checked states are NOT modified (matching C++
    /// `Mechanism::RemoveAll`).
    pub fn remove_all(&mut self) {
        let had_selection = self.selected.is_some();
        self.count = 0;
        if had_selection {
            self.selected = None;
            if let Some(cb) = &mut self.on_select {
                cb(None);
            }
        }
    }
}

/// Radio button widget -- mutually exclusive selection within a group.
pub struct RadioButton {
    border: Border,
    look: Rc<Look>,
    group: Rc<RefCell<RadioGroup>>,
    index: usize,
}

impl RadioButton {
    pub fn new(
        caption: &str,
        look: Rc<Look>,
        group: Rc<RefCell<RadioGroup>>,
        index: usize,
    ) -> Self {
        group.borrow_mut().count += 1;
        Self {
            border: Border::new(OuterBorderType::InstrumentMoreRound)
                .with_caption(caption)
                .with_label_in_border(false),
            look,
            group,
            index,
        }
    }

    /// The index of this button within its group.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Update the index (used after `remove_by_index` re-indexes the group).
    pub fn set_index(&mut self, index: usize) {
        self.index = index;
    }

    pub fn is_selected(&self) -> bool {
        self.group.borrow().selected == Some(self.index)
    }

    /// Set the checked state of this radio button, synchronizing with the
    /// group mechanism.
    ///
    /// Matches C++ `emRadioButton::CheckChanged` behaviour:
    /// - If `checked` is true, tells the mechanism to select this button
    ///   (unchecking any previously selected button).
    /// - If `checked` is false and this button is currently selected in the
    ///   mechanism, clears the mechanism's selection.
    pub fn set_checked(&mut self, checked: bool) {
        if checked {
            self.group.borrow_mut().select(self.index);
        } else if self.is_selected() {
            self.group.borrow_mut().set_check_index(None);
        }
    }

    /// Paint using the non-boxed C++ DoButton path (emButton.cpp:343-421).
    ///
    /// RadioButton renders as a normal button (face + centered label).
    /// When checked (ShownChecked=true), the label is slightly shrunk and
    /// a ButtonChecked overlay is painted instead of the normal Button overlay.
    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true);

        // C++ DoButton non-boxed path: GetContentRoundRect, clamp r.
        let (cr, r) = self.border.content_round_rect(w, h, &self.look);
        let r = r.max(cr.w.min(cr.h) * self.border.border_scaling * 0.223);

        // Face inset: d = (14/264) * r (C++ line 348).
        let d = (14.0 / 264.0) * r;
        let fx = cr.x + d;
        let fy = cr.y + d;
        let fw = cr.w - 2.0 * d;
        let fh = cr.h - 2.0 * d;
        let fr = (r - d).max(0.0);

        let face_color = self.look.button_bg_color;
        painter.paint_round_rect(fx, fy, fw, fh, fr, face_color);
        painter.set_canvas_color(face_color);

        // Label inside face with padding (C++ lines 370-391).
        let d_min = fw.min(fh) * 0.1;
        let dx = (r * 0.7).max(d_min);
        let dy = (r * 0.4).max(d_min);
        let mut lx = fx + dx;
        let mut ly = fy + dy;
        let mut lw = fw - 2.0 * dx;
        let mut lh = fh - 2.0 * dy;

        let checked = self.is_selected();
        if checked {
            // C++ line 378: ShownChecked → scale 0.983.
            let s = 0.983;
            lx += (1.0 - s) * 0.5 * lw;
            lw *= s;
            ly += (1.0 - s) * 0.5 * lh;
            lh *= s;
        }
        self.border.paint_label_colored(
            painter,
            Rect::new(lx, ly, lw, lh),
            &self.look,
            self.look.button_fg_color,
            true,
        );

        // Button overlay image (C++ lines 393-421).
        with_toolkit_images(|img| {
            if checked {
                // ShownChecked: ButtonChecked overlay (C++ lines 402-409).
                painter.paint_border_image(
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
                    Color::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            } else {
                // Normal: Button overlay (C++ lines 411-420).
                let extra = (658.0 - 648.0) / 264.0 * r;
                painter.paint_border_image(
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
                    Color::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            }
        });
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        match event.key {
            InputKey::MouseLeft if event.variant == InputVariant::Release => {
                self.group.borrow_mut().select(self.index);
                true
            }
            InputKey::Space if event.variant == InputVariant::Release => {
                self.group.borrow_mut().select(self.index);
                true
            }
            _ => false,
        }
    }

    pub fn get_cursor(&self) -> Cursor {
        Cursor::Hand
    }

    /// Whether this radio button provides how-to help text.
    /// Matches C++ `emRadioButton::HasHowTo` (inherited, always true).
    pub fn has_how_to(&self) -> bool {
        true
    }

    /// Help text describing how to use this radio button.
    ///
    /// Chains the border's base how-to with check-button + radio-button
    /// specific sections. Matches C++ `emRadioButton::GetHowTo`.
    pub fn get_how_to(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.get_howto(enabled, focusable);
        text.push_str(HOWTO_CHECK_BUTTON);
        if self.is_selected() {
            text.push_str(HOWTO_CHECKED);
        } else {
            text.push_str(HOWTO_NOT_CHECKED);
        }
        text.push_str(HOWTO_RADIO_BUTTON);
        text
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let th = 13.0;
        let tw = Painter::measure_text_width(&self.border.caption, th);
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
    pub layout: LinearLayout,
    pub group: Rc<RefCell<RadioGroup>>,
}

impl RadioLinearGroup {
    pub fn horizontal() -> Self {
        Self {
            layout: LinearLayout::horizontal(),
            group: RadioGroup::new(),
        }
    }

    pub fn vertical() -> Self {
        Self {
            layout: LinearLayout::vertical(),
            group: RadioGroup::new(),
        }
    }
}

pub struct RadioRasterGroup {
    pub layout: RasterLayout,
    pub group: Rc<RefCell<RadioGroup>>,
}

impl Default for RadioRasterGroup {
    fn default() -> Self {
        Self {
            layout: RasterLayout::default(),
            group: RadioGroup::new(),
        }
    }
}

impl RadioRasterGroup {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Drop for RadioButton {
    fn drop(&mut self) {
        let mut group = self.group.borrow_mut();
        if group.count > 0 {
            group.count -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radio_group_mutual_exclusion() {
        let look = Look::new();
        let group = RadioGroup::new();

        let mut r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = RadioButton::new("B", look.clone(), group.clone(), 1);
        let mut r2 = RadioButton::new("C", look, group.clone(), 2);

        assert!(!r0.is_selected());
        assert!(!r1.is_selected());
        assert!(!r2.is_selected());

        r0.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(r0.is_selected());
        assert!(!r1.is_selected());

        r2.input(&InputEvent::release(InputKey::MouseLeft));
        assert!(!r0.is_selected());
        assert!(r2.is_selected());

        r1.input(&InputEvent::release(InputKey::Space));
        assert!(!r0.is_selected());
        assert!(r1.is_selected());
        assert!(!r2.is_selected());
    }

    #[test]
    fn radio_group_callback() {
        let group = RadioGroup::new();
        let selections = Rc::new(RefCell::new(Vec::new()));
        let sel_clone = selections.clone();
        group.borrow_mut().on_select = Some(Box::new(move |idx| {
            sel_clone.borrow_mut().push(idx);
        }));

        let look = Look::new();
        let mut r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = RadioButton::new("B", look, group.clone(), 1);

        r0.input(&InputEvent::release(InputKey::MouseLeft));
        r1.input(&InputEvent::release(InputKey::MouseLeft));
        assert_eq!(*selections.borrow(), vec![Some(0), Some(1)]);
    }

    #[test]
    fn count_tracks_construction_and_drop() {
        let look = Look::new();
        let group = RadioGroup::new();
        assert_eq!(group.borrow().count(), 0);

        let r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
        assert_eq!(group.borrow().count(), 1);

        let r1 = RadioButton::new("B", look.clone(), group.clone(), 1);
        assert_eq!(group.borrow().count(), 2);

        drop(r0);
        assert_eq!(group.borrow().count(), 1);

        drop(r1);
        assert_eq!(group.borrow().count(), 0);
    }

    #[test]
    fn index_returns_correct_value() {
        let look = Look::new();
        let group = RadioGroup::new();
        let r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
        let r1 = RadioButton::new("B", look, group.clone(), 5);
        assert_eq!(r0.index(), 0);
        assert_eq!(r1.index(), 5);
    }

    // --- New tests for D-WIDGET-08 ---

    #[test]
    fn set_checked_selects_in_group() {
        let look = Look::new();
        let group = RadioGroup::new();
        let mut r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = RadioButton::new("B", look, group.clone(), 1);

        // set_checked(true) selects this button
        r0.set_checked(true);
        assert!(r0.is_selected());
        assert!(!r1.is_selected());
        assert_eq!(group.borrow().selected(), Some(0));

        // set_checked(true) on another button switches selection
        r1.set_checked(true);
        assert!(!r0.is_selected());
        assert!(r1.is_selected());
        assert_eq!(group.borrow().selected(), Some(1));

        // set_checked(false) on the selected button clears selection
        r1.set_checked(false);
        assert!(!r0.is_selected());
        assert!(!r1.is_selected());
        assert_eq!(group.borrow().selected(), None);
    }

    #[test]
    fn set_checked_false_on_unselected_is_noop() {
        let look = Look::new();
        let group = RadioGroup::new();
        let mut r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
        let mut r1 = RadioButton::new("B", look, group.clone(), 1);

        r0.set_checked(true);
        assert_eq!(group.borrow().selected(), Some(0));

        // set_checked(false) on a non-selected button does nothing
        r1.set_checked(false);
        assert_eq!(group.borrow().selected(), Some(0));
        assert!(r0.is_selected());
    }

    #[test]
    fn remove_by_index_clears_checked() {
        let group = RadioGroup::new();
        {
            let mut g = group.borrow_mut();
            g.count = 3;
            g.select(1); // button at index 1 is checked
        }

        // Remove the checked button
        group.borrow_mut().remove_by_index(1);
        assert_eq!(group.borrow().count(), 2);
        assert_eq!(group.borrow().selected(), None);
    }

    #[test]
    fn remove_by_index_decrements_checked() {
        let group = RadioGroup::new();
        {
            let mut g = group.borrow_mut();
            g.count = 4;
            g.select(3); // button at index 3 is checked
        }

        // Remove button at index 1 (before the checked one)
        group.borrow_mut().remove_by_index(1);
        assert_eq!(group.borrow().count(), 3);
        // Checked index should have decremented from 3 to 2
        assert_eq!(group.borrow().selected(), Some(2));
    }

    #[test]
    fn remove_by_index_no_change_when_checked_before() {
        let group = RadioGroup::new();
        {
            let mut g = group.borrow_mut();
            g.count = 4;
            g.select(0); // button at index 0 is checked
        }

        // Remove button at index 2 (after the checked one)
        group.borrow_mut().remove_by_index(2);
        assert_eq!(group.borrow().count(), 3);
        assert_eq!(group.borrow().selected(), Some(0));
    }

    #[test]
    fn remove_by_index_out_of_bounds_is_noop() {
        let group = RadioGroup::new();
        {
            let mut g = group.borrow_mut();
            g.count = 2;
            g.select(0);
        }
        group.borrow_mut().remove_by_index(5);
        assert_eq!(group.borrow().count(), 2);
        assert_eq!(group.borrow().selected(), Some(0));
    }

    #[test]
    fn remove_by_index_fires_callback() {
        let group = RadioGroup::new();
        let signals = Rc::new(RefCell::new(Vec::new()));
        let sig_clone = signals.clone();
        {
            let mut g = group.borrow_mut();
            g.count = 3;
            g.select(1);
            g.on_select = Some(Box::new(move |idx| {
                sig_clone.borrow_mut().push(idx);
            }));
        }

        // Remove checked button — should fire callback with None
        group.borrow_mut().remove_by_index(1);
        assert_eq!(*signals.borrow(), vec![None]);
    }

    #[test]
    fn remove_all_clears_everything() {
        let group = RadioGroup::new();
        let signals = Rc::new(RefCell::new(Vec::new()));
        let sig_clone = signals.clone();
        {
            let mut g = group.borrow_mut();
            g.count = 3;
            g.select(1);
            g.on_select = Some(Box::new(move |idx| {
                sig_clone.borrow_mut().push(idx);
            }));
        }

        group.borrow_mut().remove_all();
        assert_eq!(group.borrow().count(), 0);
        assert_eq!(group.borrow().selected(), None);
        assert_eq!(*signals.borrow(), vec![None]);
    }

    #[test]
    fn remove_all_no_signal_if_nothing_checked() {
        let group = RadioGroup::new();
        let signals = Rc::new(RefCell::new(Vec::new()));
        let sig_clone = signals.clone();
        {
            let mut g = group.borrow_mut();
            g.count = 3;
            // No selection
            g.on_select = Some(Box::new(move |idx| {
                sig_clone.borrow_mut().push(idx);
            }));
        }

        group.borrow_mut().remove_all();
        assert_eq!(group.borrow().count(), 0);
        assert!(signals.borrow().is_empty());
    }

    #[test]
    fn set_check_index_out_of_bounds_clears() {
        let group = RadioGroup::new();
        {
            let mut g = group.borrow_mut();
            g.count = 2;
            g.select(0);
        }

        // Out of bounds normalizes to None
        group.borrow_mut().set_check_index(Some(5));
        assert_eq!(group.borrow().selected(), None);
    }

    #[test]
    fn set_check_index_same_is_noop() {
        let group = RadioGroup::new();
        let signals = Rc::new(RefCell::new(Vec::new()));
        let sig_clone = signals.clone();
        {
            let mut g = group.borrow_mut();
            g.count = 3;
            g.select(1);
            g.on_select = Some(Box::new(move |idx| {
                sig_clone.borrow_mut().push(idx);
            }));
        }

        // Setting same index is a no-op
        group.borrow_mut().set_check_index(Some(1));
        assert!(signals.borrow().is_empty());
    }
}
