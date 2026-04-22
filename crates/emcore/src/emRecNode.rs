//! emRecNode — base trait for the emRec hierarchy.
//!
//! C++ reference: `include/emCore/emRec.h:36` (`class emRecNode : public emUncopyable`).
//!
//! Phase 4d Task 3 widens the trait with TryRead/TryWrite so compound
//! containers (`emUnionRec`, `emArrayRec`, `emTArrayRec`, and user-derived
//! structs) can dispatch persistence through `dyn emRecNode`.
//!
//! The `emUncopyable` supertrait is elided: Rust types are move-only by
//! default.

use crate::emEngineCtx::SchedCtx;
use crate::emRecReader::{emRecReader, RecIoError};
use crate::emRecWriter::emRecWriter;
use crate::emSignal::SignalId;

pub trait emRecNode {
    /// DIVERGED: (language-forced) C++ `emRecNode::UpperNode` is a private field accessed via
    /// friend `emRec`. Rust traits cannot express friend scope, so we expose
    /// a trait accessor instead. C++ has no public `GetParent` on `emRecNode`
    /// (only on the derived `emRec`, emRec.h:140).
    fn parent(&self) -> Option<&dyn emRecNode>;

    /// DIVERGED: (language-forced) C++ `emRec::Changed()` (emRec.h:243-246) walks the parent
    /// chain per-fire via `UpperNode->ChildChanged()`. Rust reifies that chain
    /// as a `Vec<SignalId>` per primitive (see ADR
    /// 2026-04-21-phase-4b-listener-tree-adr.md — R5 reified signal chain).
    /// Compounds will call `register_aggregate` at `add_field`/`SetVariant`/
    /// `SetCount` time to splice their aggregate signal into every descendant
    /// leaf. Lives on `emRecNode` (not `emRec<T>`) so compounds can forward
    /// through `&mut dyn emRecNode` without the value-type parameter bleeding
    /// into object-safety.
    fn register_aggregate(&mut self, sig: SignalId);

    /// DIVERGED: (language-forced) C++ has no single accessor — `emRecListener::SetListenedRec`
    /// (emRec.cpp:242-268) splices itself into `UpperNode` directly, observing
    /// every `ChildChanged()` walk without identifying a specific signal.
    /// Rust reifies the observed channel as a single `SignalId`: for a
    /// primitive this is its value signal; for a compound (Phase 4c Tasks 3-5)
    /// this will be its aggregate signal. `emRecListener` connects its engine
    /// to this signal via the scheduler. Trait-level method so
    /// `emRecListener::SetListenedRec(Option<&dyn emRecNode>)` stays
    /// non-generic over the primitive's value type `T`.
    fn listened_signal(&self) -> SignalId;

    /// Atomic read of this record's serialised body from `reader`. Fires
    /// value signals via `ctx` on successful assignment. Mirrors C++
    /// `emRec::TryStartReading` + `TryContinueReading` fused into one call
    /// (DIVERGED — see individual concrete types for the rationale; the
    /// scheduler is cooperative at a coarser granularity than the C++
    /// per-element yield).
    fn TryRead(
        &mut self,
        reader: &mut dyn emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError>;

    /// Atomic write of this record's serialised body to `writer`. Mirrors
    /// C++ `emRec::TryStartWriting` + `TryContinueWriting` fused into one
    /// call — see `TryRead` for the DIVERGED rationale.
    fn TryWrite(&self, writer: &mut dyn emRecWriter) -> Result<(), RecIoError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emRecReader::PeekResult;

    /// Minimal `emRecReader` stub for compile-time dyn-safety checks —
    /// `TryRead` impls in test-doubles need a concrete reader to typecheck
    /// `&mut dyn emRecReader`, so a trait-covering stub keeps the check
    /// honest without dragging the full mem-reader into the test.
    struct StubReader;
    impl emRecReader for StubReader {
        fn TryPeekNext(&mut self) -> Result<PeekResult, RecIoError> {
            Ok(PeekResult::End)
        }
        fn TryReadDelimiter(&mut self) -> Result<char, RecIoError> {
            Err(self.ThrowSyntaxError())
        }
        fn TryReadCertainDelimiter(&mut self, _d: char) -> Result<(), RecIoError> {
            Err(self.ThrowSyntaxError())
        }
        fn TryReadIdentifier(&mut self) -> Result<String, RecIoError> {
            Err(self.ThrowSyntaxError())
        }
        fn TryReadInt(&mut self) -> Result<i32, RecIoError> {
            Err(self.ThrowSyntaxError())
        }
        fn TryReadDouble(&mut self) -> Result<f64, RecIoError> {
            Err(self.ThrowSyntaxError())
        }
        fn TryReadQuoted(&mut self) -> Result<String, RecIoError> {
            Err(self.ThrowSyntaxError())
        }
        fn ThrowElemError(&self, text: &str) -> RecIoError {
            RecIoError::with_location(None, None, text)
        }
        fn ThrowSyntaxError(&self) -> RecIoError {
            self.ThrowElemError("syntax error")
        }
    }

    #[test]
    fn rec_node_has_parent_accessor() {
        struct Fake;
        impl emRecNode for Fake {
            fn parent(&self) -> Option<&dyn emRecNode> {
                None
            }
            fn register_aggregate(&mut self, _sig: SignalId) {}
            fn listened_signal(&self) -> SignalId {
                SignalId::default()
            }
            fn TryRead(
                &mut self,
                reader: &mut dyn emRecReader,
                _ctx: &mut SchedCtx<'_>,
            ) -> Result<(), RecIoError> {
                Err(reader.ThrowSyntaxError())
            }
            fn TryWrite(&self, _writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
                Ok(())
            }
        }
        let f = Fake;
        assert!(f.parent().is_none());
        // dyn-compat smoke test: coerce to &mut dyn emRecNode.
        let mut fake = Fake;
        let n: &mut dyn emRecNode = &mut fake;
        let mut r = StubReader;
        // TryWrite via dyn is safe (just asserts dyn-compat of the trait).
        struct NullWriter;
        impl emRecWriter for NullWriter {
            fn TryWriteDelimiter(&mut self, _c: char) -> Result<(), RecIoError> {
                Ok(())
            }
            fn TryWriteIdentifier(&mut self, _: &str) -> Result<(), RecIoError> {
                Ok(())
            }
            fn TryWriteInt(&mut self, _: i32) -> Result<(), RecIoError> {
                Ok(())
            }
            fn TryWriteDouble(&mut self, _: f64) -> Result<(), RecIoError> {
                Ok(())
            }
            fn TryWriteQuoted(&mut self, _: &str) -> Result<(), RecIoError> {
                Ok(())
            }
            fn TryWriteSpace(&mut self) -> Result<(), RecIoError> {
                Ok(())
            }
            fn TryWriteNewLine(&mut self) -> Result<(), RecIoError> {
                Ok(())
            }
            fn TryWriteIndent(&mut self) -> Result<(), RecIoError> {
                Ok(())
            }
            fn IncIndent(&mut self) {}
            fn DecIndent(&mut self) {}
        }
        let mut w = NullWriter;
        n.TryWrite(&mut w).unwrap();
        // Poke the stub reader to silence dead-code.
        let _ = r.TryPeekNext();
    }
}
