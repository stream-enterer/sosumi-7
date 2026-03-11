use zuicchini::model::{Clipboard, PrivateClipboard};

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
