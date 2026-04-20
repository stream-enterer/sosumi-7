use std::cell::RefCell;

/// Abstract clipboard interface matching C++ emClipboard.
///
/// Provides clipboard and selection buffer operations. Concrete
/// implementations are installed into the framework-level slot
/// (`emGUIFramework::clipboard`) via `emPrivateClipboard::Install`.
pub trait emClipboard {
    /// Put text into the clipboard or selection buffer.
    /// If `selection` is true, returns a selection ID (incrementing counter).
    /// If `selection` is false, returns 0.
    fn PutText(&mut self, text: &str, selection: bool) -> i64;

    /// Clear the clipboard or selection buffer.
    /// For selections, only clears if `selection_id` matches the current ID.
    fn Clear(&mut self, selection: bool, selection_id: i64);

    /// Get the current text from the clipboard or selection buffer.
    fn GetText(&self, selection: bool) -> String;
}

/// In-memory clipboard implementation matching C++ emPrivateClipboard.
///
/// Maintains separate clipboard and selection buffers with no platform integration.
pub struct emPrivateClipboard {
    clip_text: String,
    sel_text: String,
    sel_id: i64,
}

impl emPrivateClipboard {
    pub fn new() -> Self {
        Self {
            clip_text: String::new(),
            sel_text: String::new(),
            sel_id: 0,
        }
    }

    /// Install this clipboard into the framework's chartered clipboard slot.
    ///
    /// DIVERGED (Phase-3 Task-2): C++ `emPrivateClipboard::Install(emContext&)`
    /// installs into `emContext` via `LookupInherited`. Rust relocates the
    /// clipboard to `emGUIFramework::clipboard` (spec §3.4 / §3.6(a)), so
    /// `Install` takes the framework-owned `RefCell<Option<Box<dyn emClipboard>>>`
    /// directly. Callers pass `&app.clipboard` or the equivalent test fixture.
    pub fn Install(slot: &RefCell<Option<Box<dyn emClipboard>>>) {
        *slot.borrow_mut() = Some(Box::new(Self::new()));
    }
}

impl Default for emPrivateClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl emClipboard for emPrivateClipboard {
    fn PutText(&mut self, text: &str, selection: bool) -> i64 {
        if selection {
            self.sel_text = text.to_string();
            self.sel_id += 1;
            self.sel_id
        } else {
            self.clip_text = text.to_string();
            0
        }
    }

    fn Clear(&mut self, selection: bool, selection_id: i64) {
        if selection {
            if selection_id == self.sel_id {
                self.sel_text.clear();
                self.sel_id += 1;
            }
        } else {
            self.clip_text.clear();
        }
    }

    fn GetText(&self, selection: bool) -> String {
        if selection {
            self.sel_text.clone()
        } else {
            self.clip_text.clone()
        }
    }
}
