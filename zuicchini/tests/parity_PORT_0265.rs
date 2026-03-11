use zuicchini::model::{Clipboard, PrivateClipboard};

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
