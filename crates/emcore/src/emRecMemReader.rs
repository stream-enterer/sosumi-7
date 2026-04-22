//! emRecMemReader — concrete `emRecReader` that consumes bytes from an
//! owned slice.
//!
//! C++ reference: `src/emCore/emRec.cpp:2205-2480` (lexer) and
//! `emRec.cpp:2862-2901` (Mem adapter).
//! State-carrying impl of the stateless `emRecReader` trait: lexer state
//! (position, lookahead element, line counter) lives on this concrete type.
//! `#%rec:…%` magic-header handling is deferred to Task 6 (emConfigModel).

use crate::emRecReader::{emRecReader, ElementType, PeekResult, RecIoError};

/// Mirrors the lexer state C++ keeps on the `emRecReader` base class
/// (emRec.cpp:1974-1988). One `Lexer` per reader instance; private.
///
/// Owns its byte buffer by value. C++ keeps a `FILE*` / `const char*` plus a
/// length and scans in-place; the Rust port folds the byte buffer into the
/// lexer so the concrete `emRecMemReader` is `'static` and composable by
/// file-backed wrappers (see `emRecFileReader`). The buffer is accessed by
/// index (`pos`), never by borrowing a subslice across `&mut self` calls.
struct Lexer {
    src: Vec<u8>,
    pos: usize,

    /// Line associated with the most recently *consumed* element —
    /// C++ `emRecReader::Line`, emRec.h:1655. Updated by the public
    /// consume methods (`TryReadDelimiter`, etc.).
    line: u32,
    /// `true` when the lookahead element has been consumed and must be
    /// re-parsed on the next request. C++ `NextEaten`.
    next_eaten: bool,
    /// Line associated with the lookahead element. C++ `NextLine`.
    next_line: u32,
    next_type: ElementType,
    next_delimiter: u8,
    next_buf: Vec<u8>,
    next_int: i32,
    next_double: f64,
    /// -1 signals EOF; 0..=255 is a byte; mirror of C++ `NextChar` which is
    /// an `int` holding `(unsigned char)` or `-1`.
    next_char: i32,
}

impl Lexer {
    fn new(src: Vec<u8>) -> Self {
        let mut l = Self {
            src,
            pos: 0,
            line: 1,
            next_eaten: true,
            next_line: 1,
            next_type: ElementType::End,
            next_delimiter: 0,
            next_buf: Vec::new(),
            next_int: 0,
            next_double: 0.0,
            next_char: -1,
        };
        // C++ `TryStartReading` primes the first lookahead char
        // (emRec.cpp:2034). The Mem-backed lexer does the same inline.
        l.TryNextChar();
        l
    }

    /// `emRecReader::TryNextChar` (emRec.cpp:2205-2211) — backed by
    /// `emRecMemReader::TryRead` (emRec.cpp:2877-2888).
    fn TryNextChar(&mut self) {
        if self.pos < self.src.len() {
            self.next_char = self.src[self.pos] as i32;
            self.pos += 1;
        } else {
            self.next_char = -1;
        }
    }

    /// `emRecReader::TryParseNext` (emRec.cpp:2214-2480).
    fn TryParseNext(&mut self) -> Result<(), RecIoError> {
        self.next_eaten = false;

        // Parse white spaces, comments, and EOF.
        loop {
            if self.next_char >= 0 && self.next_char <= 0x20 {
                if self.next_char == 0x20 || self.next_char == 0x09 {
                    self.TryNextChar();
                } else if self.next_char == 0x0a {
                    self.next_line += 1;
                    self.TryNextChar();
                } else if self.next_char == 0x0d {
                    self.next_line += 1;
                    self.TryNextChar();
                    if self.next_char == 0x0a {
                        self.TryNextChar();
                    }
                } else {
                    // Lone control bytes: skip silently per C++.
                    self.TryNextChar();
                }
            } else if self.next_char == -1 {
                self.next_type = ElementType::End;
                return Ok(());
            } else if self.next_char == b'#' as i32 {
                loop {
                    self.TryNextChar();
                    if self.next_char == b'\n' as i32
                        || self.next_char == b'\r' as i32
                        || self.next_char == -1
                    {
                        break;
                    }
                }
            } else {
                break;
            }
        }

        // Parse identifier.
        if is_ident_start(self.next_char) {
            self.next_buf.clear();
            loop {
                if self.next_buf.len() > 10_000_000 {
                    self.line = self.next_line;
                    return Err(self.elem_error("Identifier too long."));
                }
                self.next_buf.push(self.next_char as u8);
                self.TryNextChar();
                if !is_ident_cont(self.next_char) {
                    break;
                }
            }
            self.next_type = ElementType::Identifier;
            return Ok(());
        }

        // Parse quoted string.
        if self.next_char == b'"' as i32 {
            self.next_buf.clear();
            loop {
                self.TryNextChar();
                if self.next_char == b'"' as i32 {
                    self.TryNextChar();
                    break;
                }
                if self.next_char == b'\\' as i32 {
                    self.next_buf.push(b'\\');
                    self.TryNextChar();
                }
                if self.next_char == -1 {
                    self.line = self.next_line;
                    return Err(self.elem_error("Unterminated string."));
                }
                // Line counting inside strings: CR or LF that isn't the
                // LF half of a CRLF just counted (emRec.cpp:2297-2300).
                if self.next_char == 0x0d
                    || (self.next_char == 0x0a
                        && (self.next_buf.is_empty() || *self.next_buf.last().unwrap() != 0x0d))
                {
                    self.next_line += 1;
                }
                if self.next_buf.len() > 10_000_000 {
                    self.line = self.next_line;
                    return Err(self.elem_error("String too long."));
                }
                self.next_buf.push(self.next_char as u8);
            }
            self.next_buf = resolve_escapes(&self.next_buf);
            self.next_type = ElementType::Quoted;
            return Ok(());
        }

        // Parse numeric constant or fall back to delimiter.
        if (self.next_char >= b'0' as i32 && self.next_char <= b'9' as i32)
            || self.next_char == b'-' as i32
            || self.next_char == b'+' as i32
            || self.next_char == b'.' as i32
        {
            self.next_buf.clear();
            let mut seen_digit = false;
            let mut is_double = false;
            if self.next_char == b'+' as i32 || self.next_char == b'-' as i32 {
                self.next_buf.push(self.next_char as u8);
                self.TryNextChar();
            }
            while self.next_char >= b'0' as i32 && self.next_char <= b'9' as i32 {
                seen_digit = true;
                if self.next_buf.len() > 100 {
                    self.line = self.next_line;
                    return Err(self.elem_error("Numeric constant too long."));
                }
                self.next_buf.push(self.next_char as u8);
                self.TryNextChar();
            }
            if self.next_char == b'.' as i32 {
                is_double = true;
                self.next_buf.push(b'.');
                self.TryNextChar();
                while self.next_char >= b'0' as i32 && self.next_char <= b'9' as i32 {
                    seen_digit = true;
                    if self.next_buf.len() > 100 {
                        self.line = self.next_line;
                        return Err(self.elem_error("Numeric constant too long."));
                    }
                    self.next_buf.push(self.next_char as u8);
                    self.TryNextChar();
                }
            }
            if self.next_char == b'E' as i32 || self.next_char == b'e' as i32 {
                is_double = true;
                self.next_buf.push(self.next_char as u8);
                self.TryNextChar();
                if self.next_char == b'+' as i32 || self.next_char == b'-' as i32 {
                    self.next_buf.push(self.next_char as u8);
                    self.TryNextChar();
                }
                if !(self.next_char >= b'0' as i32 && self.next_char <= b'9' as i32) {
                    self.line = self.next_line;
                    return Err(self.elem_error("Syntax error."));
                }
                while self.next_char >= b'0' as i32 && self.next_char <= b'9' as i32 {
                    if self.next_buf.len() > 100 {
                        self.line = self.next_line;
                        return Err(self.elem_error("Numeric constant too long."));
                    }
                    self.next_buf.push(self.next_char as u8);
                    self.TryNextChar();
                }
            }

            // Lone '.', '-', '+' → delimiter (emRec.cpp:2449-2454).
            if self.next_buf.len() == 1 {
                let b0 = self.next_buf[0];
                if b0 == b'.' || b0 == b'-' || b0 == b'+' {
                    self.next_delimiter = b0;
                    self.next_type = ElementType::Delimiter;
                    return Ok(());
                }
            }
            if !seen_digit {
                self.line = self.next_line;
                return Err(self.elem_error("Syntax error."));
            }
            // C `sscanf("%d", …)` rejects leading '+'; `%lf` accepts it.
            // Mirror that by stripping a leading '+' for the int path.
            let offset: usize = if self.next_buf[0] == b'+' { 1 } else { 0 };
            // Parse the buffer into owned values before touching
            // `self.line` / `self.elem_error` (which need a fresh &self
            // borrow). Holding a slice across the mutation would violate
            // the aliasing rules; copy first.
            let parsed_s = std::str::from_utf8(&self.next_buf[offset..]).ok();
            if is_double {
                match parsed_s.and_then(|s| s.parse::<f64>().ok()) {
                    Some(v) => {
                        self.next_double = v;
                        self.next_type = ElementType::Double;
                    }
                    None => {
                        self.line = self.next_line;
                        return Err(self.elem_error("Syntax error."));
                    }
                }
            } else {
                match parsed_s.and_then(|s| s.parse::<i32>().ok()) {
                    Some(v) => {
                        self.next_int = v;
                        self.next_type = ElementType::Int;
                    }
                    None => {
                        self.line = self.next_line;
                        return Err(self.elem_error("Syntax error."));
                    }
                }
            }
            return Ok(());
        }

        // Everything else is a delimiter (emRec.cpp:2476-2479).
        self.next_delimiter = self.next_char as u8;
        self.next_type = ElementType::Delimiter;
        self.TryNextChar();
        Ok(())
    }

    /// Return a tagged `PeekResult` for the current lookahead.
    fn peek_result(&self) -> PeekResult {
        match self.next_type {
            ElementType::Delimiter => PeekResult::Delimiter(self.next_delimiter as char),
            ElementType::Identifier => PeekResult::Identifier,
            ElementType::Int => PeekResult::Int,
            ElementType::Double => PeekResult::Double,
            ElementType::Quoted => PeekResult::Quoted,
            ElementType::End => PeekResult::End,
        }
    }

    fn elem_error(&self, text: &str) -> RecIoError {
        RecIoError::with_location(
            Some(Self::source_name().to_string()),
            Some(self.line as usize),
            text,
        )
    }

    fn source_name() -> &'static str {
        // Mirrors C++ `emRecMemReader::GetSourceName` (emRec.cpp:2898-2901).
        "rec memory buffer"
    }
}

/// `[A-Za-z_]`.
fn is_ident_start(c: i32) -> bool {
    (c >= b'a' as i32 && c <= b'z' as i32)
        || (c >= b'A' as i32 && c <= b'Z' as i32)
        || c == b'_' as i32
}

/// `[A-Za-z0-9_]`.
fn is_ident_cont(c: i32) -> bool {
    is_ident_start(c) || (c >= b'0' as i32 && c <= b'9' as i32)
}

/// In-place second-pass escape resolution — mirrors emRec.cpp:2310-2378.
/// Input is the raw quoted body (with backslash pairs still present);
/// output is the decoded byte sequence.
fn resolve_escapes(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let n = raw.len();
    let mut i = 0;
    while i < n {
        if raw[i] != b'\\' {
            out.push(raw[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= n {
            // Unreachable in practice: the lexer rejects an unterminated
            // string earlier ("Unterminated string"), so a body ending in a
            // lone backslash never reaches `resolve_escapes`. Emit the
            // backslash literally as a defensive fallback.
            out.push(b'\\');
            break;
        }
        let e = raw[i];
        match e {
            b'a' => {
                out.push(0x07);
                i += 1;
            }
            b'b' => {
                out.push(0x08);
                i += 1;
            }
            b'e' => {
                out.push(0x1b);
                i += 1;
            }
            b'f' => {
                out.push(0x0c);
                i += 1;
            }
            b'n' => {
                out.push(0x0a);
                i += 1;
            }
            b'r' => {
                out.push(0x0d);
                i += 1;
            }
            b't' => {
                out.push(0x09);
                i += 1;
            }
            b'v' => {
                out.push(0x0b);
                i += 1;
            }
            b'x' => {
                // \xHH — up to two hex digits. If the first nibble
                // isn't hex, emit "\x" literally and back up (C++ does
                // the same; emRec.cpp:2351-2355).
                i += 1;
                let h1 = hex_digit(raw.get(i).copied());
                if let Some(mut k) = h1 {
                    i += 1;
                    if let Some(h2) = hex_digit(raw.get(i).copied()) {
                        k = (k << 4) | h2;
                        i += 1;
                    }
                    out.push(k);
                } else {
                    out.push(b'\\');
                    out.push(b'x');
                }
            }
            b'0'..=b'7' => {
                // Up to three octal digits.
                let mut k: u8 = e - b'0';
                i += 1;
                if let Some(&c1) = raw.get(i) {
                    if (b'0'..=b'7').contains(&c1) {
                        k = (k << 3) | (c1 - b'0');
                        i += 1;
                        if let Some(&c2) = raw.get(i) {
                            if (b'0'..=b'7').contains(&c2) {
                                k = (k << 3) | (c2 - b'0');
                                i += 1;
                            }
                        }
                    }
                }
                out.push(k);
            }
            _ => {
                // Unknown escape — pass the following char through
                // literally (C++ drops the backslash; emRec.cpp:2371-2373).
                out.push(e);
                i += 1;
            }
        }
    }
    out
}

fn hex_digit(b: Option<u8>) -> Option<u8> {
    match b? {
        c @ b'0'..=b'9' => Some(c - b'0'),
        c @ b'a'..=b'f' => Some(c - b'a' + 10),
        c @ b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Byte-buffer-backed emRecReader. Equivalent to C++ `emRecMemReader`
/// (emRec.cpp:2862-2901).
///
/// DIVERGED: (language-forced) C++ exposes `TryStartReading(root, buf, len)` which hands the
/// buffer off to `emRecReader::TryStartReading`, which then optionally
/// consumes the `#%rec:<name>%` magic when `root->GetFormatName()` is
/// non-null. The Rust port constructs the reader with the byte slice and
/// defers the magic-header handling to a later task (see the Task 1 doc
/// comment and the Phase 4d plan) — atomic-type reads such as `emBoolRec`
/// never carry a format name.
pub struct emRecMemReader {
    lexer: Lexer,
}

impl emRecMemReader {
    /// Construct from a byte slice; the buffer is copied into the reader.
    /// Kept as the 1:1 C++ constructor name for File and Name Correspondence
    /// (C++ `emRecMemReader::emRecMemReader(...)` stores a borrowed pointer,
    /// but Rust's single-threaded owned-buffer model is cleaner here and
    /// matches how Phase 4d composes the file reader — see
    /// [`from_vec`](Self::from_vec)).
    pub fn new(buf: &[u8]) -> Self {
        Self::from_vec(buf.to_vec())
    }

    /// Construct from an owned byte vector without copying. Used by
    /// [`crate::emRecFileReader`] to hand off the file contents directly.
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self {
            lexer: Lexer::new(bytes),
        }
    }

    /// Construct + validate the `#%rec:<expected_format>%` magic header.
    ///
    /// Mirrors the header-consumption branch of C++
    /// `emRecReader::TryStartReading` (emRec.cpp:2004-2042): when
    /// `root->GetFormatName()` is non-null, the reader reads exactly
    /// `#%rec:FormatName%` from the front of the stream and errors if the
    /// bytes mismatch. The trailing `#` that conventionally appears in Eagle
    /// Mode files (e.g. `#%rec:emVirtualCosmosItem%#`) is NOT part of the
    /// magic; it lands on the next `TryNextChar` call and is absorbed by the
    /// lexer's `#`-to-end-of-line comment path (emRec.cpp:2266-2273).
    pub fn with_format_header(bytes: &[u8], expected_format: &str) -> Result<Self, RecIoError> {
        Self::with_format_header_vec(bytes.to_vec(), expected_format, None)
    }

    /// Like [`with_format_header`] but consumes an owned buffer. `source_name`
    /// is carried into the mismatch error for parity with
    /// [`crate::emRecFileReader::open_with_format`].
    pub(crate) fn with_format_header_vec(
        bytes: Vec<u8>,
        expected_format: &str,
        source_name: Option<String>,
    ) -> Result<Self, RecIoError> {
        let magic = {
            let mut m = Vec::with_capacity(7 + expected_format.len());
            m.extend_from_slice(b"#%rec:");
            m.extend_from_slice(expected_format.as_bytes());
            m.push(b'%');
            m
        };
        if bytes.len() < magic.len() || bytes[..magic.len()] != magic[..] {
            return Err(RecIoError::with_location(
                source_name,
                Some(1),
                format!("File format is not \"rec:{}\".", expected_format),
            ));
        }
        // Skip the magic prefix; the lexer will handle the rest (including
        // any trailing `#...\n` comment).
        let rest = bytes[magic.len()..].to_vec();
        Ok(Self {
            lexer: Lexer::new(rest),
        })
    }
}

impl emRecReader for emRecMemReader {
    fn TryPeekNext(&mut self) -> Result<PeekResult, RecIoError> {
        if self.lexer.next_eaten {
            self.lexer.TryParseNext()?;
        }
        Ok(self.lexer.peek_result())
    }

    fn TryReadDelimiter(&mut self) -> Result<char, RecIoError> {
        if self.lexer.next_eaten {
            self.lexer.TryParseNext()?;
        }
        self.lexer.line = self.lexer.next_line;
        self.lexer.next_eaten = true;
        if self.lexer.next_type != ElementType::Delimiter {
            return Err(self.lexer.elem_error("Delimiter expected."));
        }
        Ok(self.lexer.next_delimiter as char)
    }

    fn TryReadCertainDelimiter(&mut self, delimiter: char) -> Result<(), RecIoError> {
        if self.lexer.next_eaten {
            self.lexer.TryParseNext()?;
        }
        self.lexer.line = self.lexer.next_line;
        self.lexer.next_eaten = true;
        if self.lexer.next_type != ElementType::Delimiter
            || self.lexer.next_delimiter as char != delimiter
        {
            return Err(self.lexer.elem_error(&format!("'{}' expected.", delimiter)));
        }
        Ok(())
    }

    fn TryReadIdentifier(&mut self) -> Result<String, RecIoError> {
        if self.lexer.next_eaten {
            self.lexer.TryParseNext()?;
        }
        self.lexer.line = self.lexer.next_line;
        self.lexer.next_eaten = true;
        if self.lexer.next_type != ElementType::Identifier {
            return Err(self.lexer.elem_error("Identifier expected."));
        }
        // Identifiers are ASCII by the lexer contract — from_utf8 is
        // infallible in practice but we check to be defensive.
        String::from_utf8(self.lexer.next_buf.clone())
            .map_err(|_| self.lexer.elem_error("Non-ASCII identifier."))
    }

    fn TryReadInt(&mut self) -> Result<i32, RecIoError> {
        if self.lexer.next_eaten {
            self.lexer.TryParseNext()?;
        }
        self.lexer.line = self.lexer.next_line;
        self.lexer.next_eaten = true;
        if self.lexer.next_type != ElementType::Int {
            return Err(self.lexer.elem_error("Integer expected."));
        }
        Ok(self.lexer.next_int)
    }

    fn TryReadDouble(&mut self) -> Result<f64, RecIoError> {
        if self.lexer.next_eaten {
            self.lexer.TryParseNext()?;
        }
        self.lexer.line = self.lexer.next_line;
        self.lexer.next_eaten = true;
        match self.lexer.next_type {
            ElementType::Int => Ok(self.lexer.next_int as f64),
            ElementType::Double => Ok(self.lexer.next_double),
            _ => Err(self.lexer.elem_error("Floating point number expected.")),
        }
    }

    fn TryReadQuoted(&mut self) -> Result<String, RecIoError> {
        if self.lexer.next_eaten {
            self.lexer.TryParseNext()?;
        }
        self.lexer.line = self.lexer.next_line;
        self.lexer.next_eaten = true;
        if self.lexer.next_type != ElementType::Quoted {
            return Err(self.lexer.elem_error("Quoted string expected."));
        }
        // Quoted bodies are 8-bit-clean in C++; Rust `String` needs UTF-8.
        // The vast majority of rec files are UTF-8; fall back to a
        // lossy-decode with a syntax-error wrapper if the bytes are not.
        String::from_utf8(self.lexer.next_buf.clone())
            .map_err(|_| self.lexer.elem_error("Non-UTF-8 quoted string."))
    }

    fn ThrowElemError(&self, text: &str) -> RecIoError {
        RecIoError::with_location(
            Some(Lexer::source_name().to_string()),
            Some(self.lexer.line as usize),
            text,
        )
    }

    fn ThrowSyntaxError(&self) -> RecIoError {
        self.ThrowElemError("Syntax error.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_and_read_identifier() {
        let mut r = emRecMemReader::new(b"hello");
        assert_eq!(r.TryPeekNext().unwrap(), PeekResult::Identifier);
        assert_eq!(r.TryReadIdentifier().unwrap(), "hello");
        assert_eq!(r.TryPeekNext().unwrap(), PeekResult::End);
    }

    #[test]
    fn read_int_and_double() {
        let mut r = emRecMemReader::new(b"  42   -3.5  ");
        assert_eq!(r.TryReadInt().unwrap(), 42);
        assert!((r.TryReadDouble().unwrap() - (-3.5)).abs() < 1e-12);
    }

    #[test]
    fn read_quoted_with_escapes() {
        let mut r = emRecMemReader::new(b"\"a\\nb\\tc\\\"d\\\\e\"");
        assert_eq!(r.TryReadQuoted().unwrap(), "a\nb\tc\"d\\e");
    }

    #[test]
    fn read_octal_and_hex_escapes() {
        let mut r = emRecMemReader::new(b"\"\\001\\x41\"");
        assert_eq!(r.TryReadQuoted().unwrap(), "\x01A");
    }

    #[test]
    fn line_comments_skipped() {
        let mut r = emRecMemReader::new(b"# header comment\nfoo");
        assert_eq!(r.TryReadIdentifier().unwrap(), "foo");
    }

    #[test]
    fn lone_dot_is_delimiter() {
        let mut r = emRecMemReader::new(b".");
        assert_eq!(r.TryReadDelimiter().unwrap(), '.');
    }

    #[test]
    fn delimiter_passthrough() {
        let mut r = emRecMemReader::new(b"{x}");
        assert_eq!(r.TryReadDelimiter().unwrap(), '{');
        assert_eq!(r.TryReadIdentifier().unwrap(), "x");
        assert_eq!(r.TryReadDelimiter().unwrap(), '}');
    }

    #[test]
    fn certain_delimiter_enforces_match() {
        let mut r = emRecMemReader::new(b"=");
        assert!(r.TryReadCertainDelimiter('=').is_ok());

        let mut r = emRecMemReader::new(b"=");
        assert!(r.TryReadCertainDelimiter(';').is_err());
    }

    #[test]
    fn format_header_consumes_magic_and_trailing_comment() {
        // Exact Eagle Mode shape: `#%rec:Foo%#<newline><body>`.
        let mut r = emRecMemReader::with_format_header(b"#%rec:Foo%#\nhello 42", "Foo").unwrap();
        assert_eq!(r.TryReadIdentifier().unwrap(), "hello");
        assert_eq!(r.TryReadInt().unwrap(), 42);
    }

    #[test]
    fn format_header_rejects_mismatch() {
        let err = match emRecMemReader::with_format_header(b"#%rec:Bar%#\nfoo", "Foo") {
            Ok(_) => panic!("expected mismatch error"),
            Err(e) => e,
        };
        let msg = err.to_string();
        assert!(msg.contains("rec:Foo"), "{msg}");
    }

    #[test]
    fn format_header_rejects_missing() {
        let err = match emRecMemReader::with_format_header(b"foo = 1", "Foo") {
            Ok(_) => panic!("expected missing-header error"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("rec:Foo"));
    }

    #[test]
    fn line_counts_across_newlines() {
        let mut r = emRecMemReader::new(b"\n\n\nfoo");
        let _ = r.TryReadIdentifier().unwrap();
        // Line of the last-consumed element, per C++.
        assert_eq!(r.lexer.line, 4);
    }
}
