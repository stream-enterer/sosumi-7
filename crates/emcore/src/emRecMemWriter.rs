//! emRecMemWriter — concrete `emRecWriter` that appends bytes to an owned
//! buffer.
//!
//! C++ reference: `emRec.h:1869-1913` (class decl) and
//! `src/emCore/emRec.cpp:2908-2936` (implementation). In C++ this is a thin
//! source/sink adapter over the `emRecWriter` base class, which holds the
//! formatting state (`Indent`, the write helpers). In Rust the `emRecWriter`
//! trait (Task 1) is interface-only, so the formatting state lives in a
//! private [`Emitter`] struct embedded in `emRecMemWriter` — this mirrors
//! the C++ base class fields, just owned by the concrete writer instead of
//! inherited.
//!
//! Format fidelity — see C++ `emRecWriter` body (emRec.cpp:2574-2669):
//!   * Integer: `sprintf("%d", i)`.
//!   * Double:  `sprintf("%.9G", d)`, then append ".0" if the result
//!     contains neither '.' nor 'E' nor 'e'.
//!   * Quoted:  surround with `"`, emit `\n \r \t \a \b \f \v \\ \"`
//!     backslash escapes for the common controls, `\NNN` octal triplet for
//!     any other 0x00-0x1F or 0x7F-0x9F byte, pass 0x20-0x7E (except `"` /
//!     `\`) and 0xA0+ through verbatim (the latter preserves UTF-8
//!     continuation bytes).
//!   * Indent: Tab character per current indent level.

use crate::emRecReader::RecIoError;
use crate::emRecWriter::emRecWriter;

/// Mirrors the mutable formatting state C++ keeps on the `emRecWriter` base
/// class (emRec.cpp:2487-2493 — `Indent` field plus the `TryWriteChar` /
/// `TryWriteString` helpers). Private: only [`emRecMemWriter`] exposes it.
struct Emitter {
    buf: Vec<u8>,
    indent: i32,
}

impl Emitter {
    fn new() -> Self {
        Self {
            buf: Vec::new(),
            indent: 0,
        }
    }

    /// Mirrors `emRecWriter::TryWriteChar` (emRec.cpp:2660-2663).
    fn TryWriteChar(&mut self, c: u8) {
        self.buf.push(c);
    }

    /// Mirrors `emRecWriter::TryWriteString` (emRec.cpp:2666-2669).
    fn TryWriteString(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
    }
}

/// Byte-buffer-backed emRecWriter. Equivalent to C++ `emRecMemWriter`
/// (emRec.cpp:2908-2936) but owns the output `Vec<u8>` directly — C++ takes
/// an external `emArray<char> &` because its caller (`emRecMemFileModel`)
/// wants to retain ownership. The Rust port collapses that indirection:
/// call [`into_bytes`](emRecMemWriter::into_bytes) at end of writing.
///
/// DIVERGED: The C++ split `emRecMemWriter::TryStartWriting(root, buf)` into
/// external-buf + base-class orchestration isn't useful in Rust. Callers
/// drive writes directly through the concrete rec type's `TryWrite` method;
/// the state-machine driver (`TryStartWriting` / `TryContinueWriting`) is
/// deferred to a later task (see Task 1 doc comment).
pub struct emRecMemWriter {
    emitter: Emitter,
}

impl emRecMemWriter {
    pub fn new() -> Self {
        Self {
            emitter: Emitter::new(),
        }
    }

    /// Consume the writer and return the accumulated bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.emitter.buf
    }
}

impl Default for emRecMemWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl emRecWriter for emRecMemWriter {
    /// `emRecWriter::TryWriteDelimiter` (emRec.cpp:2574-2577).
    fn TryWriteDelimiter(&mut self, c: char) -> Result<(), RecIoError> {
        // C++ stores the delimiter as a single `char` (byte). ASCII-only by
        // the lexer contract (documented on `PeekResult::Delimiter`).
        self.emitter.TryWriteChar(c as u8);
        Ok(())
    }

    /// `emRecWriter::TryWriteIdentifier` (emRec.cpp:2580-2583).
    fn TryWriteIdentifier(&mut self, idf: &str) -> Result<(), RecIoError> {
        self.emitter.TryWriteString(idf);
        Ok(())
    }

    /// `emRecWriter::TryWriteInt` (emRec.cpp:2586-2592) — `sprintf("%d", i)`.
    fn TryWriteInt(&mut self, i: i32) -> Result<(), RecIoError> {
        self.emitter.TryWriteString(&i.to_string());
        Ok(())
    }

    /// `emRecWriter::TryWriteDouble` (emRec.cpp:2595-2604) —
    /// `sprintf("%.9G", d)` + `.0` tail if the formatted text has neither
    /// `.` nor `E` nor `e`.
    fn TryWriteDouble(&mut self, d: f64) -> Result<(), RecIoError> {
        let mut s = format_g_9(d);
        if !s
            .as_bytes()
            .iter()
            .any(|&b| b == b'.' || b == b'E' || b == b'e')
        {
            s.push_str(".0");
        }
        self.emitter.TryWriteString(&s);
        Ok(())
    }

    /// `emRecWriter::TryWriteQuoted` (emRec.cpp:2607-2637). Passes bytes
    /// 0x20-0x7E (except `"` / `\`) and 0xA0-0xFF through unchanged;
    /// escapes the common controls by letter and the rest via `\NNN` octal.
    fn TryWriteQuoted(&mut self, q: &str) -> Result<(), RecIoError> {
        self.emitter.TryWriteChar(b'"');
        for &c in q.as_bytes() {
            // Passthrough band: printable ASCII (minus `"` / `\`) OR bytes
            // 0xA0+ (UTF-8 continuation bytes, Latin-1 upper half). Matches
            // the loop termination in C++ emRec.cpp:2614-2617.
            let printable = (0x20..=0x7E).contains(&c) && c != b'"' && c != b'\\';
            if printable || c >= 0xA0 {
                self.emitter.TryWriteChar(c);
            } else {
                match c {
                    b'"' => self.emitter.TryWriteString("\\\""),
                    b'\\' => self.emitter.TryWriteString("\\\\"),
                    0x07 => self.emitter.TryWriteString("\\a"),
                    0x08 => self.emitter.TryWriteString("\\b"),
                    0x0C => self.emitter.TryWriteString("\\f"),
                    b'\n' => self.emitter.TryWriteString("\\n"),
                    b'\r' => self.emitter.TryWriteString("\\r"),
                    b'\t' => self.emitter.TryWriteString("\\t"),
                    0x0B => self.emitter.TryWriteString("\\v"),
                    _ => {
                        // C++: \NNN three-digit octal (emRec.cpp:2631-2633).
                        self.emitter.TryWriteChar(b'\\');
                        self.emitter.TryWriteChar(b'0' + ((c >> 6) & 7));
                        self.emitter.TryWriteChar(b'0' + ((c >> 3) & 7));
                        self.emitter.TryWriteChar(b'0' + (c & 7));
                    }
                }
            }
        }
        self.emitter.TryWriteChar(b'"');
        Ok(())
    }

    /// `emRecWriter::TryWriteSpace` (emRec.cpp:2640-2643).
    fn TryWriteSpace(&mut self) -> Result<(), RecIoError> {
        self.emitter.TryWriteChar(b' ');
        Ok(())
    }

    /// `emRecWriter::TryWriteNewLine` (emRec.cpp:2646-2649).
    fn TryWriteNewLine(&mut self) -> Result<(), RecIoError> {
        self.emitter.TryWriteChar(b'\n');
        Ok(())
    }

    /// `emRecWriter::TryWriteIndent` (emRec.cpp:2652-2657).
    fn TryWriteIndent(&mut self) -> Result<(), RecIoError> {
        for _ in 0..self.emitter.indent {
            self.emitter.TryWriteChar(b'\t');
        }
        Ok(())
    }

    fn IncIndent(&mut self) {
        self.emitter.indent += 1;
    }

    fn DecIndent(&mut self) {
        self.emitter.indent -= 1;
    }
}

/// Emulate C's `sprintf("%.9G", d)`.
///
/// `%G` = `%E` when the exponent is <-4 or >= precision; `%F` otherwise.
/// Trailing zeros are stripped (unlike `%g` / `%e`), and the exponent
/// always has at least two digits (`E+01`, not `E+1`).
fn format_g_9(d: f64) -> String {
    if d.is_nan() {
        return "NAN".to_string();
    }
    if d.is_infinite() {
        return if d.is_sign_negative() {
            "-INF".to_string()
        } else {
            "INF".to_string()
        };
    }
    if d == 0.0 {
        // C sprintf("%.9G", 0.0) → "0"; the caller appends ".0".
        return "0".to_string();
    }

    let precision: i32 = 9;
    let abs = d.abs();
    let exp_floor = abs.log10().floor() as i32;
    // %G spec: use %E when exp < -4 or exp >= precision; else %F.
    if exp_floor < -4 || exp_floor >= precision {
        // Scientific. Normalise mantissa to [1, 10).
        let mantissa = d / 10f64.powi(exp_floor);
        // Render with (precision-1) digits after the decimal, then strip
        // trailing zeros (C's `%G` behaviour).
        let digits = (precision - 1) as usize;
        let m_str = format!("{:.*}", digits, mantissa);
        let m_stripped = strip_trailing_zeros(&m_str);
        let sign = if exp_floor < 0 { '-' } else { '+' };
        format!("{}E{}{:02}", m_stripped, sign, exp_floor.abs())
    } else {
        // Fixed. Precision in %G is total significant digits, so decimals
        // = precision - 1 - exp_floor (floor to zero).
        let decimals = (precision - 1 - exp_floor).max(0) as usize;
        let s = format!("{:.*}", decimals, d);
        strip_trailing_zeros(&s).to_string()
    }
}

/// Strip trailing zeros from the fractional part; if the result ends in
/// `.`, strip that too. C's `%G` does this.
fn strip_trailing_zeros(s: &str) -> &str {
    if !s.contains('.') {
        return s;
    }
    let s = s.trim_end_matches('0');
    s.trim_end_matches('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_format_matches_sprintf_d() {
        let mut w = emRecMemWriter::new();
        w.TryWriteInt(-42).unwrap();
        assert_eq!(w.into_bytes(), b"-42");
    }

    #[test]
    fn double_format_appends_point_zero_when_needed() {
        // Integral values → %.9G produces "1", we append ".0".
        let mut w = emRecMemWriter::new();
        w.TryWriteDouble(1.0).unwrap();
        assert_eq!(w.into_bytes(), b"1.0");

        let mut w = emRecMemWriter::new();
        w.TryWriteDouble(0.0).unwrap();
        assert_eq!(w.into_bytes(), b"0.0");

        // Fractional values keep the decimal point — no suffix appended.
        let mut w = emRecMemWriter::new();
        w.TryWriteDouble(1.5).unwrap();
        assert_eq!(w.into_bytes(), b"1.5");
    }

    #[test]
    fn double_format_uses_scientific_for_large_exponents() {
        // exp >= 9 → scientific per %G rules. 1e10 has exp 10 ≥ 9 ⇒ %E.
        let mut w = emRecMemWriter::new();
        w.TryWriteDouble(1.0e10).unwrap();
        let bytes = w.into_bytes();
        assert!(
            bytes.starts_with(b"1E+10") || bytes.starts_with(b"1E+010"),
            "got {:?}",
            bytes,
        );
    }

    #[test]
    fn quoted_escapes_controls_and_passes_high_bytes() {
        let mut w = emRecMemWriter::new();
        w.TryWriteQuoted("a\nb\tc\"d\\e").unwrap();
        assert_eq!(w.into_bytes(), b"\"a\\nb\\tc\\\"d\\\\e\"");

        // 0x01 — no named escape, goes through \NNN octal (001).
        let mut w = emRecMemWriter::new();
        w.TryWriteQuoted("\x01").unwrap();
        assert_eq!(w.into_bytes(), b"\"\\001\"");

        // 0xA0+ bytes (UTF-8 continuation) pass through unchanged. "ä" is
        // 0xC3 0xA4 in UTF-8 — both are in the 0xA0+ passthrough band.
        let mut w = emRecMemWriter::new();
        w.TryWriteQuoted("ä").unwrap();
        assert_eq!(w.into_bytes(), b"\"\xc3\xa4\"");
    }

    #[test]
    fn indent_writes_tabs() {
        let mut w = emRecMemWriter::new();
        w.IncIndent();
        w.IncIndent();
        w.TryWriteIndent().unwrap();
        assert_eq!(w.into_bytes(), b"\t\t");
    }

    #[test]
    fn identifier_and_delimiter_and_space_and_newline() {
        let mut w = emRecMemWriter::new();
        w.TryWriteIdentifier("foo").unwrap();
        w.TryWriteSpace().unwrap();
        w.TryWriteDelimiter('=').unwrap();
        w.TryWriteSpace().unwrap();
        w.TryWriteInt(7).unwrap();
        w.TryWriteNewLine().unwrap();
        assert_eq!(w.into_bytes(), b"foo = 7\n");
    }
}
