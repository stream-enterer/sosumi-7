use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::model::{Clipboard, PrivateClipboard};

#[test]
fn clipboard_trait_exists_and_is_object_safe() {
    let cb: Rc<RefCell<dyn Clipboard>> = Rc::new(RefCell::new(PrivateClipboard::new()));
    // Verify trait object works
    cb.borrow_mut().put_text("test", false);
    assert_eq!(cb.borrow().get_text(false), "test");
}
