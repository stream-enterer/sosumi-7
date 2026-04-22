//! emRec<T> — abstract scalar-field trait.
//!
//! C++ reference: emCore/emRec.h `emRec<ValueType>`.
//!
//! Observational contract: `SetValue` mutates the stored value and fires
//! `GetValueSignal`. Callers receive `&mut SchedCtx` so the fire happens
//! inline. See spec §7 D7.1.

use crate::emEngineCtx::SchedCtx;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

/// Validate a member identifier per C++ `emRec::CheckIdentifier` (emRec.cpp:173).
///
/// C++ semantics: first char must be `[A-Za-z_]`; subsequent chars (if any)
/// must be `[A-Za-z0-9_]`. An empty string fails the first-char test.
/// Invalid identifiers trigger `emFatalError`; the Rust port panics.
///
/// TODO(phase-4d+): C++ `emRec::CheckIdentifier` is a public static method
/// with potential additional callers beyond `emStructRec::AddMember`
/// (e.g., emUnionRec, emArrayRec). Current port has only the AddMember
/// call site.
pub fn CheckIdentifier(identifier: &str) {
    let bytes = identifier.as_bytes();
    let valid = !bytes.is_empty()
        && matches!(bytes[0], b'a'..=b'z' | b'A'..=b'Z' | b'_')
        && bytes[1..]
            .iter()
            .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'));
    if !valid {
        panic!("emRec: '{}' is not a valid identifier.", identifier);
    }
}

pub trait emRec<T: Clone + PartialEq>: emRecNode {
    fn GetValue(&self) -> &T;
    fn SetValue(&mut self, value: T, ctx: &mut SchedCtx<'_>);
    fn GetDefaultValue(&self) -> &T;
    fn GetValueSignal(&self) -> SignalId;

    /// Default no-bound impl; types with explicit ranges override.
    fn GetMinValue(&self) -> Option<&T> {
        None
    }
    fn GetMaxValue(&self) -> Option<&T> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // compile-time only: ensure trait shape.
    fn _assert_trait_shape<T: emRec<i64>>(_: &T) {}
}
