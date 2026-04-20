use std::cell::RefCell;

use emcore::emClipboard::{emClipboard, emPrivateClipboard};

/// Build a framework-style clipboard slot for testing.
///
/// DIVERGED (Phase-3 Task-2): C++ `emClipboard::LookupInherited(emContext&)` walks
/// the `emContext` parent chain. Rust relocates clipboard onto `emGUIFramework`
/// (spec §3.4 / §3.6(a)), so tests install into a standalone
/// `RefCell<Option<Box<dyn emClipboard>>>` that mirrors the framework slot.
fn make_slot() -> RefCell<Option<Box<dyn emClipboard>>> {
    RefCell::new(None)
}

#[test]
fn clipboard_trait_exists_and_is_object_safe() {
    let mut cb: Box<dyn emClipboard> = Box::new(emPrivateClipboard::new());
    cb.PutText("test", false);
    assert_eq!(cb.GetText(false), "test");
}

#[test]
fn install_populates_framework_slot() {
    // Phase-3 Task-2: clipboard lives on the framework, not emContext. The
    // "lookup" pattern becomes a direct borrow of the framework-owned slot.
    let slot = make_slot();
    emPrivateClipboard::Install(&mut slot.borrow_mut());
    slot.borrow_mut()
        .as_mut()
        .expect("installed")
        .PutText("inherited", false);
    assert_eq!(slot.borrow().as_ref().unwrap().GetText(false), "inherited");
}

#[test]
fn put_text_clipboard() {
    let mut cb = emPrivateClipboard::new();
    let id = cb.PutText("hello", false);
    assert_eq!(id, 0); // clipboard always returns 0
    assert_eq!(cb.GetText(false), "hello");
}

#[test]
fn put_text_selection() {
    let mut cb = emPrivateClipboard::new();
    let id1 = cb.PutText("sel1", true);
    assert!(id1 > 0);
    let id2 = cb.PutText("sel2", true);
    assert!(id2 > id1); // monotonically increasing
    assert_eq!(cb.GetText(true), "sel2");
}

#[test]
fn clear_clipboard() {
    let mut cb = emPrivateClipboard::new();
    cb.PutText("data", false);
    cb.Clear(false, 0);
    assert_eq!(cb.GetText(false), "");
}

#[test]
fn clear_selection_matching_id() {
    let mut cb = emPrivateClipboard::new();
    let id = cb.PutText("sel", true);
    cb.Clear(true, id);
    assert_eq!(cb.GetText(true), "");
}

#[test]
fn clear_selection_wrong_id_is_noop() {
    let mut cb = emPrivateClipboard::new();
    let id = cb.PutText("sel", true);
    cb.Clear(true, id - 1); // wrong ID
    assert_eq!(cb.GetText(true), "sel"); // still there
}

#[test]
fn get_text_empty_clipboard() {
    let cb = emPrivateClipboard::new();
    assert_eq!(cb.GetText(false), "");
    assert_eq!(cb.GetText(true), "");
}

#[test]
fn get_text_independent_buffers() {
    let mut cb = emPrivateClipboard::new();
    cb.PutText("clip", false);
    cb.PutText("sel", true);
    assert_eq!(cb.GetText(false), "clip");
    assert_eq!(cb.GetText(true), "sel");
}

#[test]
fn install_populates_empty_slot() {
    let slot = make_slot();
    assert!(slot.borrow().is_none());
    emPrivateClipboard::Install(&mut slot.borrow_mut());
    assert!(slot.borrow().is_some());
}

#[test]
fn private_clipboard_default() {
    let cb = emPrivateClipboard::default();
    assert_eq!(cb.GetText(false), "");
    assert_eq!(cb.GetText(true), "");
}

#[test]
fn private_clipboard_separate_buffers() {
    let mut cb = emPrivateClipboard::new();
    cb.PutText("clipboard_data", false);
    assert_eq!(cb.GetText(false), "clipboard_data");
    assert_eq!(cb.GetText(true), ""); // selection is independent
}

#[test]
fn private_clipboard_install_replaces() {
    let slot = make_slot();
    emPrivateClipboard::Install(&mut slot.borrow_mut());
    slot.borrow_mut().as_mut().unwrap().PutText("test", false);

    // Re-Install replaces (matching C++ behavior where re-Install overwrites)
    emPrivateClipboard::Install(&mut slot.borrow_mut());
    // New clipboard has empty state
    assert_eq!(slot.borrow().as_ref().unwrap().GetText(false), "");
}

#[test]
fn full_clipboard_lifecycle() {
    let mut cb = emPrivateClipboard::new();

    // Put and GetRec clipboard
    cb.PutText("clip1", false);
    assert_eq!(cb.GetText(false), "clip1");

    // Put and GetRec selection
    let sel_id = cb.PutText("sel1", true);
    assert_eq!(cb.GetText(true), "sel1");

    // Clear selection with correct ID
    cb.Clear(true, sel_id);
    assert_eq!(cb.GetText(true), "");

    // emClipboard unaffected
    assert_eq!(cb.GetText(false), "clip1");

    // Clear clipboard
    cb.Clear(false, 0);
    assert_eq!(cb.GetText(false), "");
}

#[test]
fn selection_id_increments_on_clear() {
    let mut cb = emPrivateClipboard::new();
    let id1 = cb.PutText("s1", true);
    cb.Clear(true, id1); // clears and increments sel_id
    let id2 = cb.PutText("s2", true);
    assert!(id2 > id1 + 1); // id incremented by Clear too
}
