// Kani proof harnesses for formal verification.
//
// Under `cargo kani --tests`, these modules provide bounded model checking
// harnesses. Under normal `cargo test`, this is a no-op.
//
// Inline kani_private_proofs modules remain in src/emCore/*.rs because
// they test private internals via `super::*`.

#![allow(non_snake_case)]

#[cfg(kani)]
mod proofs;
#[cfg(kani)]
mod proofs_generated;
#[cfg(kani)]
mod proofs_layer3;
