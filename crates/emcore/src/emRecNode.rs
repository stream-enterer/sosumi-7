//! emRecNode — base trait for the emRec hierarchy.
//!
//! C++ reference: emCore/emRec.h lines for `emRecNode`.

pub trait emRecNode {
    fn parent(&self) -> Option<&dyn emRecNode>;
    // additional tree-walk methods ported as callers need them in Phase 4b+
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
