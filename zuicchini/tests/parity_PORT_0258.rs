use zuicchini::model::{lookup_clipboard, Context, PrivateClipboard};

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
