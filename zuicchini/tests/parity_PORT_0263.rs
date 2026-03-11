use zuicchini::model::{Clipboard, PrivateClipboard};

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
