use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::model::{lookup_clipboard, Clipboard, Context, PrivateClipboard};

#[test]
fn clipboard_trait_exists_and_is_object_safe() {
    let cb: Rc<RefCell<dyn Clipboard>> = Rc::new(RefCell::new(PrivateClipboard::new()));
    // Verify trait object works
    cb.borrow_mut().put_text("test", false);
    assert_eq!(cb.borrow().get_text(false), "test");
}

#[test]
fn lookup_inherited_walks_parent_chain() {
    let root = Context::new_root();
    PrivateClipboard::install(&root);

    let child = Context::new_child(&root);
    let grandchild = Context::new_child(&child);

    // LookupInherited from grandchild should find root's clipboard
    let cb = lookup_clipboard(&grandchild).expect("should find clipboard");
    cb.borrow_mut().put_text("inherited", false);
    assert_eq!(cb.borrow().get_text(false), "inherited");
}

#[test]
fn put_text_clipboard() {
    let mut cb = PrivateClipboard::new();
    let id = cb.put_text("hello", false);
    assert_eq!(id, 0); // clipboard always returns 0
    assert_eq!(cb.get_text(false), "hello");
}

#[test]
fn put_text_selection() {
    let mut cb = PrivateClipboard::new();
    let id1 = cb.put_text("sel1", true);
    assert!(id1 > 0);
    let id2 = cb.put_text("sel2", true);
    assert!(id2 > id1); // monotonically increasing
    assert_eq!(cb.get_text(true), "sel2");
}

#[test]
fn clear_clipboard() {
    let mut cb = PrivateClipboard::new();
    cb.put_text("data", false);
    cb.clear(false, 0);
    assert_eq!(cb.get_text(false), "");
}

#[test]
fn clear_selection_matching_id() {
    let mut cb = PrivateClipboard::new();
    let id = cb.put_text("sel", true);
    cb.clear(true, id);
    assert_eq!(cb.get_text(true), "");
}

#[test]
fn clear_selection_wrong_id_is_noop() {
    let mut cb = PrivateClipboard::new();
    let id = cb.put_text("sel", true);
    cb.clear(true, id - 1); // wrong ID
    assert_eq!(cb.get_text(true), "sel"); // still there
}

#[test]
fn get_text_empty_clipboard() {
    let cb = PrivateClipboard::new();
    assert_eq!(cb.get_text(false), "");
    assert_eq!(cb.get_text(true), "");
}

#[test]
fn get_text_independent_buffers() {
    let mut cb = PrivateClipboard::new();
    cb.put_text("clip", false);
    cb.put_text("sel", true);
    assert_eq!(cb.get_text(false), "clip");
    assert_eq!(cb.get_text(true), "sel");
}

#[test]
fn install_registers_clipboard_in_context() {
    let ctx = Context::new_root();
    assert!(lookup_clipboard(&ctx).is_none());
    PrivateClipboard::install(&ctx);
    assert!(lookup_clipboard(&ctx).is_some());
}

#[test]
fn private_clipboard_default() {
    let cb = PrivateClipboard::default();
    assert_eq!(cb.get_text(false), "");
    assert_eq!(cb.get_text(true), "");
}

#[test]
fn private_clipboard_separate_buffers() {
    let mut cb = PrivateClipboard::new();
    cb.put_text("clipboard_data", false);
    assert_eq!(cb.get_text(false), "clipboard_data");
    assert_eq!(cb.get_text(true), ""); // selection is independent
}

#[test]
fn private_clipboard_install_idempotent() {
    let ctx = Context::new_root();
    PrivateClipboard::install(&ctx);
    let cb1 = lookup_clipboard(&ctx).unwrap();
    cb1.borrow_mut().put_text("test", false);

    // Re-install replaces (matching C++ behavior where re-install overwrites)
    PrivateClipboard::install(&ctx);
    let cb2 = lookup_clipboard(&ctx).unwrap();
    // New clipboard has empty state
    assert_eq!(cb2.borrow().get_text(false), "");
}

#[test]
fn full_clipboard_lifecycle() {
    let mut cb = PrivateClipboard::new();

    // Put and get clipboard
    cb.put_text("clip1", false);
    assert_eq!(cb.get_text(false), "clip1");

    // Put and get selection
    let sel_id = cb.put_text("sel1", true);
    assert_eq!(cb.get_text(true), "sel1");

    // Clear selection with correct ID
    cb.clear(true, sel_id);
    assert_eq!(cb.get_text(true), "");

    // Clipboard unaffected
    assert_eq!(cb.get_text(false), "clip1");

    // Clear clipboard
    cb.clear(false, 0);
    assert_eq!(cb.get_text(false), "");
}

#[test]
fn selection_id_increments_on_clear() {
    let mut cb = PrivateClipboard::new();
    let id1 = cb.put_text("s1", true);
    cb.clear(true, id1); // clears and increments sel_id
    let id2 = cb.put_text("s2", true);
    assert!(id2 > id1 + 1); // id incremented by clear too
}
