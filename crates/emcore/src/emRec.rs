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
