use std::collections::HashMap;
use std::rc::Rc;

use crate::emColor::emColor;
use crate::emContext::emContext;
use crate::emSignal::SignalId;

/// An observable value that tracks whether it changed on set.
///
/// `set()` returns `true` when the value actually changed, allowing the caller
/// to fire the associated signal via the scheduler.
pub struct WatchedVar<T: PartialEq> {
    value: T,
    signal_id: SignalId,
}

impl<T: PartialEq> WatchedVar<T> {
    pub fn new(value: T, signal_id: SignalId) -> Self {
        Self { value, signal_id }
    }

    pub fn GetRec(&self) -> &T {
        &self.value
    }

    /// Replace the value. Returns `true` if it actually changed.
    pub fn Set(&mut self, new_value: T) -> bool {
        if self.value == new_value {
            return false;
        }
        self.value = new_value;
        true
    }

    pub fn signal_id(&self) -> SignalId {
        self.signal_id
    }
}

/// Port of C++ `emVarModel<emColor>::GetAndRemove`. Retrieves and removes
/// the stored color for `key` from the root context's var store. Returns
/// `default` if absent.
pub fn GetAndRemove(ctx: &Rc<emContext>, key: &str, default: emColor) -> emColor {
    let store = ctx.acquire::<HashMap<String, emColor>>("emVarModel/emColor", HashMap::new);
    let result = store.borrow_mut().remove(key).unwrap_or(default);
    result
}

/// Port of C++ `emVarModel<emColor>::Set`. Inserts `value` into the root
/// context's var store under `key`.
///
/// `min_lifetime` mirrors C++ `SetMinCommonLifetime(minLifetime)` — the number
/// of seconds the model should survive after all references drop. Rust uses
/// plain `HashMap` storage keyed on the context, so there are no per-key model
/// objects and no ref-count lifetime to manage. The parameter is accepted for
/// API parity and ignored.
pub fn Set(ctx: &Rc<emContext>, key: &str, value: emColor, _min_lifetime: usize) {
    let store = ctx.acquire::<HashMap<String, emColor>>("emVarModel/emColor", HashMap::new);
    store.borrow_mut().insert(key.to_string(), value);
}

#[cfg(test)]
mod tests_var_model {
    use super::*;

    fn make_ctx() -> Rc<emContext> {
        emContext::NewRoot()
    }

    #[test]
    fn get_and_remove_returns_default_when_absent() {
        let ctx = make_ctx();
        let default = emColor::rgba(1, 2, 3, 4);
        let result = GetAndRemove(&ctx, "key1", default);
        assert_eq!(result, default);
    }

    #[test]
    fn set_then_get_and_remove_roundtrips() {
        let ctx = make_ctx();
        let color = emColor::rgba(10, 20, 30, 255);
        Set(&ctx, "key2", color, 10);
        let got = GetAndRemove(&ctx, "key2", emColor::BLACK);
        assert_eq!(got, color);
        let again = GetAndRemove(&ctx, "key2", emColor::BLACK);
        assert_eq!(again, emColor::BLACK);
    }
}
