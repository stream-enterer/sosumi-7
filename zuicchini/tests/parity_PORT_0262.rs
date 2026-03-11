use zuicchini::model::{lookup_clipboard, Context, PrivateClipboard};

#[test]
fn install_registers_clipboard_in_context() {
    let ctx = Context::new_root();
    assert!(lookup_clipboard(&ctx).is_none());
    PrivateClipboard::install(&ctx);
    assert!(lookup_clipboard(&ctx).is_some());
}
