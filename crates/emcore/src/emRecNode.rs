//! emRecNode — base trait for the emRec hierarchy.
//!
//! C++ reference: `include/emCore/emRec.h:36` (`class emRecNode : public emUncopyable`).
//!
//! Phase 4a ports only the parent accessor. Deferred to Phase 4b+:
//! - `IsListener()` (emRec.h:42 pure virtual)
//! - `ChildChanged()` (emRec.h:43 pure virtual)
//!
//! The `emUncopyable` supertrait is elided: Rust types are move-only by default.

pub trait emRecNode {
    /// DIVERGED: C++ `emRecNode::UpperNode` is a private field accessed via
    /// friend `emRec`. Rust traits cannot express friend scope, so we expose
    /// a trait accessor instead. C++ has no public `GetParent` on `emRecNode`
    /// (only on the derived `emRec`, emRec.h:140).
    fn parent(&self) -> Option<&dyn emRecNode>;
    // TODO(phase-4b): IsListener, ChildChanged, tree-walk helpers.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rec_node_has_parent_accessor() {
        // A trait-object holder satisfies the trait shape.
        struct Fake;
        impl emRecNode for Fake {
            fn parent(&self) -> Option<&dyn emRecNode> {
                None
            }
        }
        let f = Fake;
        assert!(f.parent().is_none());
    }
}
