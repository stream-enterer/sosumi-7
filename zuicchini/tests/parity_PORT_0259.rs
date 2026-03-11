use zuicchini::model::{Clipboard, PrivateClipboard};

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
