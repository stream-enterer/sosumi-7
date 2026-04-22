//! emRecReader — abstract trait for reading a tree of records from a source.
//!
//! C++ reference: `include/emCore/emRec.h:1545-1660` (class declaration) and
//! `src/emCore/emRec.cpp` (implementation).
//!
//! Phase 4d Task 1 ports only the per-element primitive API (the
//! "for implementing emRec derivatives" section, emRec.h:1569-1620). The
//! state-machine driver methods (`TryStartReading`, `TryContinueReading`,
//! `TryFinishReading`, `QuitReading`, `GetRootRec`, `GetSourceName`, and the
//! protected `TryRead`/`TryClose`) are deferred to later Phase 4d tasks — they
//! live one layer above the primitives and are consumed by
//! `emRec::TryRead(reader, ctx)`.
//!
//! NOT `RecError` from `emRecParser.rs`: that error type belongs to the
//! legacy text-parsing tree (`RecValue`/`RecStruct`) and is intentionally
//! distinct. Do not conflate.

use std::error::Error;
use std::fmt;

/// Kind of syntactical element peeked at the reader cursor.
///
// DIVERGED: (language-forced) C++ `emRecReader::TryPeekNext(char *pDelimiter=NULL)` returns the
// `ElementType` enum and uses an output pointer to optionally receive the
// delimiter character. Rust folds the out-parameter into the `Delimiter`
// variant via [`PeekResult`]; the `ElementType` enum mirrors the C++ enum
// names with the `ET_` prefix dropped (Rust namespaces variants by enum type).
// See `emRec.h:1571-1578`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// Maps to `ET_DELIMITER`.
    Delimiter,
    /// Maps to `ET_IDENTIFIER`.
    Identifier,
    /// Maps to `ET_INT`.
    Int,
    /// Maps to `ET_DOUBLE`.
    Double,
    /// Maps to `ET_QUOTED`.
    Quoted,
    /// Maps to `ET_END`.
    End,
}

/// Result of `TryPeekNext`. Folds C++'s `ET_DELIMITER`-plus-out-pointer pair
/// into a single Rust value.
///
// DIVERGED: (language-forced) C++ uses a `char *pDelimiter` out-param alongside the returned
// `ElementType`; Rust attaches the delimiter character to the `Delimiter`
// variant instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeekResult {
    /// ASCII-only delimiter, per C++ `emRecReader` lexer (non-ASCII input is
    /// rejected earlier in the lex).
    Delimiter(char),
    Identifier,
    Int,
    Double,
    Quoted,
    End,
}

impl PeekResult {
    /// Strip the delimiter payload, yielding the bare [`ElementType`].
    pub fn element_type(self) -> ElementType {
        match self {
            PeekResult::Delimiter(_) => ElementType::Delimiter,
            PeekResult::Identifier => ElementType::Identifier,
            PeekResult::Int => ElementType::Int,
            PeekResult::Double => ElementType::Double,
            PeekResult::Quoted => ElementType::Quoted,
            PeekResult::End => ElementType::End,
        }
    }
}

/// Error returned by emRecReader/emRecWriter primitives.
///
/// Mirrors the "file name + line + text" message shape that
/// `emRecReader::ThrowElemError` assembles (`emRec.h:1612-1614`).
///
/// Distinct from `emRecParser::RecError`, which belongs to the legacy
/// `RecValue`/`RecStruct` text parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecIoError {
    pub(crate) source_name: Option<String>,
    pub(crate) line: Option<usize>,
    pub(crate) message: String,
}

impl RecIoError {
    pub fn with_location(
        source_name: Option<String>,
        line: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            source_name,
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for RecIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.source_name, self.line) {
            (Some(src), Some(line)) => write!(f, "{}:{}: {}", src, line, self.message),
            (Some(src), None) => write!(f, "{}: {}", src, self.message),
            (None, Some(line)) => write!(f, "line {}: {}", line, self.message),
            (None, None) => f.write_str(&self.message),
        }
    }
}

impl Error for RecIoError {}

/// Per-element primitive reader API.
///
/// Mirrors the "for implementing emRec derivatives" section of
/// `emRecReader` (`emRec.h:1569-1620`). Dyn-compatible: concrete rec types
/// hold `&mut dyn emRecReader` across `TryRead` calls.
pub trait emRecReader {
    /// `ElementType emRecReader::TryPeekNext(char *pDelimiter)` — peek the
    /// type of the next syntactical element without consuming it.
    fn TryPeekNext(&mut self) -> Result<PeekResult, RecIoError>;

    /// `char emRecReader::TryReadDelimiter()`.
    fn TryReadDelimiter(&mut self) -> Result<char, RecIoError>;

    /// `void emRecReader::TryReadCertainDelimiter(char delimiter)` — consume
    /// the next element, requiring it to equal `delimiter`.
    fn TryReadCertainDelimiter(&mut self, delimiter: char) -> Result<(), RecIoError>;

    /// `const char * emRecReader::TryReadIdentifier()` — consume and return
    /// the next element as an identifier.
    ///
    /// C++ returns a pointer into an internal scratch buffer; Rust returns an
    /// owned `String`. This is idiom adaptation (not a divergence): the
    /// buffer lifetime contract is invisible to callers.
    fn TryReadIdentifier(&mut self) -> Result<String, RecIoError>;

    /// `int emRecReader::TryReadInt()` — consume and return the next element
    /// as an integer. Width matches C++ `int` (32-bit on all supported
    /// targets), per the pixel-arithmetic fidelity rule.
    fn TryReadInt(&mut self) -> Result<i32, RecIoError>;

    /// `double emRecReader::TryReadDouble()`.
    fn TryReadDouble(&mut self) -> Result<f64, RecIoError>;

    /// `const char * emRecReader::TryReadQuoted()` — owned `String` in Rust,
    /// same idiom note as `TryReadIdentifier`.
    fn TryReadQuoted(&mut self) -> Result<String, RecIoError>;

    /// `void emRecReader::ThrowElemError(const char *text) const` —
    /// construct an error tagged with the current source name + line.
    ///
    // DIVERGED: (language-forced) C++ throws; Rust returns the constructed error so callers
    // can `?` it. The helper is still named `ThrowElemError` for File and
    // Name Correspondence.
    fn ThrowElemError(&self, text: &str) -> RecIoError;

    /// `void emRecReader::ThrowSyntaxError() const` — shorthand for
    /// `ThrowElemError("syntax error")`.
    ///
    // DIVERGED: (language-forced) returns instead of throwing; see `ThrowElemError`.
    fn ThrowSyntaxError(&self) -> RecIoError;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyReader;

    impl emRecReader for DummyReader {
        fn TryPeekNext(&mut self) -> Result<PeekResult, RecIoError> {
            unimplemented!()
        }
        fn TryReadDelimiter(&mut self) -> Result<char, RecIoError> {
            unimplemented!()
        }
        fn TryReadCertainDelimiter(&mut self, _delimiter: char) -> Result<(), RecIoError> {
            unimplemented!()
        }
        fn TryReadIdentifier(&mut self) -> Result<String, RecIoError> {
            unimplemented!()
        }
        fn TryReadInt(&mut self) -> Result<i32, RecIoError> {
            unimplemented!()
        }
        fn TryReadDouble(&mut self) -> Result<f64, RecIoError> {
            unimplemented!()
        }
        fn TryReadQuoted(&mut self) -> Result<String, RecIoError> {
            unimplemented!()
        }
        fn ThrowElemError(&self, text: &str) -> RecIoError {
            RecIoError::with_location(None, None, text)
        }
        fn ThrowSyntaxError(&self) -> RecIoError {
            self.ThrowElemError("syntax error")
        }
    }

    fn assert_dyn_safe(_r: &mut dyn emRecReader) {}

    #[test]
    fn trait_is_dyn_safe_and_method_list_compiles() {
        let mut d = DummyReader;
        assert_dyn_safe(&mut d);
        // Exercise the default-to-error helpers (safe — no unimplemented!()).
        let err = d.ThrowSyntaxError();
        assert_eq!(err.message, "syntax error");
    }

    #[test]
    fn peek_result_element_type_projection() {
        assert_eq!(
            PeekResult::Delimiter(';').element_type(),
            ElementType::Delimiter
        );
        assert_eq!(
            PeekResult::Identifier.element_type(),
            ElementType::Identifier
        );
        assert_eq!(PeekResult::Int.element_type(), ElementType::Int);
        assert_eq!(PeekResult::Double.element_type(), ElementType::Double);
        assert_eq!(PeekResult::Quoted.element_type(), ElementType::Quoted);
        assert_eq!(PeekResult::End.element_type(), ElementType::End);
    }

    #[test]
    fn rec_io_error_display_formats() {
        let e = RecIoError::with_location(Some("foo.rec".into()), Some(12), "bad");
        assert_eq!(format!("{}", e), "foo.rec:12: bad");
        let e = RecIoError::with_location(Some("foo.rec".into()), None, "bad");
        assert_eq!(format!("{}", e), "foo.rec: bad");
        let e = RecIoError::with_location(None, Some(7), "bad");
        assert_eq!(format!("{}", e), "line 7: bad");
        let e = RecIoError::with_location(None, None, "bad");
        assert_eq!(format!("{}", e), "bad");
    }
}
