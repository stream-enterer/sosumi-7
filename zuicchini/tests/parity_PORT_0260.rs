use zuicchini::model::{Clipboard, PrivateClipboard};

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
