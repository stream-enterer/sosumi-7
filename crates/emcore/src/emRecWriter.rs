//! emRecWriter — abstract trait for writing a tree of records to a target.
//!
//! C++ reference: `include/emCore/emRec.h:1667-1750` (class declaration) and
//! `src/emCore/emRec.cpp` (implementation).
//!
//! Phase 4d Task 1 ports only the per-element primitive API (the
//! "for implementing emRec derivatives" section, emRec.h:1691-1724). The
//! state-machine driver methods (`TryStartWriting`, `TryContinueWriting`,
//! `TryFinishWriting`, `QuitWriting`, `GetRootRec`, and the protected
//! `TryWrite`/`TryClose`) are deferred to later Phase 4d tasks.
//!
//! Errors use [`RecIoError`] from the sibling [`emRecReader`] module — the
//! reader and writer share one error type, mirroring their shared C++ error
//! pathway.
//!
//! [`emRecReader`]: crate::emRecReader

use crate::emRecReader::RecIoError;

/// Per-element primitive writer API.
///
/// Mirrors the "for implementing emRec derivatives" section of
/// `emRecWriter` (`emRec.h:1691-1724`). Dyn-compatible: concrete rec types
/// hold `&mut dyn emRecWriter` across `TryWrite` calls.
pub trait emRecWriter {
    /// `void emRecWriter::TryWriteDelimiter(char c)`.
    fn TryWriteDelimiter(&mut self, c: char) -> Result<(), RecIoError>;

    /// `void emRecWriter::TryWriteIdentifier(const char *idf)`.
    fn TryWriteIdentifier(&mut self, idf: &str) -> Result<(), RecIoError>;

    /// `void emRecWriter::TryWriteInt(int i)` — width matches C++ `int`
    /// (32-bit), per the pixel-arithmetic fidelity rule.
    fn TryWriteInt(&mut self, i: i32) -> Result<(), RecIoError>;

    /// `void emRecWriter::TryWriteDouble(double d)`.
    fn TryWriteDouble(&mut self, d: f64) -> Result<(), RecIoError>;

    /// `void emRecWriter::TryWriteQuoted(const char *q)`.
    fn TryWriteQuoted(&mut self, q: &str) -> Result<(), RecIoError>;

    /// `void emRecWriter::TryWriteSpace()`.
    fn TryWriteSpace(&mut self) -> Result<(), RecIoError>;

    /// `void emRecWriter::TryWriteNewLine()`.
    fn TryWriteNewLine(&mut self) -> Result<(), RecIoError>;

    /// `void emRecWriter::TryWriteIndent()` — emit one tabulator per current
    /// indent level (to be called at the beginning of a new line).
    fn TryWriteIndent(&mut self) -> Result<(), RecIoError>;

    /// `void emRecWriter::IncIndent()` — increase indent level; infallible
    /// in both C++ (inline field increment) and Rust.
    fn IncIndent(&mut self);

    /// `void emRecWriter::DecIndent()` — decrease indent level; infallible.
    fn DecIndent(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyWriter {
        indent: i32,
    }

    impl emRecWriter for DummyWriter {
        fn TryWriteDelimiter(&mut self, _c: char) -> Result<(), RecIoError> {
            unimplemented!()
        }
        fn TryWriteIdentifier(&mut self, _idf: &str) -> Result<(), RecIoError> {
            unimplemented!()
        }
        fn TryWriteInt(&mut self, _i: i32) -> Result<(), RecIoError> {
            unimplemented!()
        }
        fn TryWriteDouble(&mut self, _d: f64) -> Result<(), RecIoError> {
            unimplemented!()
        }
        fn TryWriteQuoted(&mut self, _q: &str) -> Result<(), RecIoError> {
            unimplemented!()
        }
        fn TryWriteSpace(&mut self) -> Result<(), RecIoError> {
            unimplemented!()
        }
        fn TryWriteNewLine(&mut self) -> Result<(), RecIoError> {
            unimplemented!()
        }
        fn TryWriteIndent(&mut self) -> Result<(), RecIoError> {
            unimplemented!()
        }
        fn IncIndent(&mut self) {
            self.indent += 1;
        }
        fn DecIndent(&mut self) {
            self.indent -= 1;
        }
    }

    fn assert_dyn_safe(_w: &mut dyn emRecWriter) {}

    #[test]
    fn trait_is_dyn_safe_and_method_list_compiles() {
        let mut w = DummyWriter { indent: 0 };
        assert_dyn_safe(&mut w);
        w.IncIndent();
        w.IncIndent();
        w.DecIndent();
        assert_eq!(w.indent, 1);
    }
}
