use std::cell::RefCell;
use std::rc::Rc;

use super::context::Context;

/// Abstract clipboard interface matching C++ emClipboard.
///
/// Provides clipboard and selection buffer operations. Concrete
/// implementations are installed into a Context via `install`.
pub trait Clipboard {
    /// Put text into the clipboard or selection buffer.
    /// If `selection` is true, returns a selection ID (incrementing counter).
    /// If `selection` is false, returns 0.
    fn put_text(&mut self, text: &str, selection: bool) -> i64;

    /// Clear the clipboard or selection buffer.
    /// For selections, only clears if `selection_id` matches the current ID.
    fn clear(&mut self, selection: bool, selection_id: i64);

    /// Get the current text from the clipboard or selection buffer.
    fn get_text(&self, selection: bool) -> String;
}

/// In-memory clipboard implementation matching C++ emPrivateClipboard.
///
/// Maintains separate clipboard and selection buffers with no platform integration.
pub struct PrivateClipboard {
    clip_text: String,
    sel_text: String,
    sel_id: i64,
}

impl PrivateClipboard {
    pub fn new() -> Self {
        Self {
            clip_text: String::new(),
            sel_text: String::new(),
            sel_id: 0,
        }
    }

    /// Install this clipboard into the given context.
    /// Port of C++ emPrivateClipboard::Install(emContext&).
    pub fn install(context: &Rc<Context>) {
        let clipboard: Rc<RefCell<dyn Clipboard>> = Rc::new(RefCell::new(Self::new()));
        context.set_clipboard(clipboard);
    }
}

impl Default for PrivateClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard for PrivateClipboard {
    fn put_text(&mut self, text: &str, selection: bool) -> i64 {
        if selection {
            self.sel_text = text.to_string();
            self.sel_id += 1;
            self.sel_id
        } else {
            self.clip_text = text.to_string();
            0
        }
    }

    fn clear(&mut self, selection: bool, selection_id: i64) {
        if selection {
            if selection_id == self.sel_id {
                self.sel_text.clear();
                self.sel_id += 1;
            }
        } else {
            self.clip_text.clear();
        }
    }

    fn get_text(&self, selection: bool) -> String {
        if selection {
            self.sel_text.clone()
        } else {
            self.clip_text.clone()
        }
    }
}

/// Look up the installed clipboard by walking the context hierarchy.
/// Port of C++ emClipboard::LookupInherited(emContext&).
pub fn lookup_clipboard(context: &Rc<Context>) -> Option<Rc<RefCell<dyn Clipboard>>> {
    context.lookup_clipboard()
}
