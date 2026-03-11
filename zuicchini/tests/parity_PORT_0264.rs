use zuicchini::model::{lookup_clipboard, Context, PrivateClipboard};

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
