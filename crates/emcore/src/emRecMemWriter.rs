//! emRecMemWriter — concrete `emRecWriter` that appends bytes to an owned
//! buffer.
//!
//! C++ reference: `src/emCore/emRec.cpp:2574-2669` (base-class formatting
//! helpers) and `emRec.cpp:2908-2936` (Mem adapter).
//! State-carrying impl of the stateless `emRecWriter` trait: formatting
//! state (`Indent`, buffer) lives on this concrete type, not the trait.

use crate::emRecReader::RecIoError;
use crate::emRecWriter::emRecWriter;

/// Mutable formatting state. C++ keeps these on the `emRecWriter` base
/// class (emRec.cpp:2487-2493); here they live on the concrete type.
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

    fn TryWriteChar(&mut self, c: u8) {
        self.buf.push(c);
    }

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
/// DIVERGED: (language-forced) The C++ split `emRecMemWriter::TryStartWriting(root, buf)` into
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
    fn TryWriteDelimiter(&mut self, c: char) -> Result<(), RecIoError> {
        // C++ stores the delimiter as a single `char` (byte). ASCII-only by
        // the lexer contract (documented on `PeekResult::Delimiter`).
        self.emitter.TryWriteChar(c as u8);
        Ok(())
    }

    fn TryWriteIdentifier(&mut self, idf: &str) -> Result<(), RecIoError> {
        self.emitter.TryWriteString(idf);
        Ok(())
    }

    fn TryWriteInt(&mut self, i: i32) -> Result<(), RecIoError> {
        self.emitter.TryWriteString(&i.to_string());
        Ok(())
    }

    fn TryWriteDouble(&mut self, d: f64) -> Result<(), RecIoError> {
        if !d.is_finite() {
            return Err(RecIoError::with_location(
                None,
                None,
                "emRec format cannot encode non-finite double",
            ));
        }
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

    // DIVERGED: (language-forced) C++ `TryWriteQuoted(const char*)` truncates at the first NUL
    // (C-string convention). Rust `&str` is UTF-8 and may legitimately carry
    // embedded NUL bytes, so we preserve them by emitting `\000` — the same
    // octal escape path that handles any other 0x00-0x1F control byte.
    // Information-preserving and symmetric with the reader (which decodes
    // `\000` back to a NUL). See emRec.cpp:2607-2637 for the C++ loop.
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

    fn TryWriteSpace(&mut self) -> Result<(), RecIoError> {
        self.emitter.TryWriteChar(b' ');
        Ok(())
    }

    fn TryWriteNewLine(&mut self) -> Result<(), RecIoError> {
        self.emitter.TryWriteChar(b'\n');
        Ok(())
    }

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
/// always has at least two digits (`E+01`, not `E+1`). Non-finite inputs
/// are rejected upstream by `TryWriteDouble`.
///
/// Approach: format once with Rust `{:.8e}` (9 significant digits rounded by
/// the stdlib, matching C's round-half-to-even on IEEE doubles), then parse
/// back the exponent to classify fixed vs scientific. This routes boundary
/// cases like `9.9999999995e9 → 1.0e10` through the same rounding the C
/// library does, so the exponent classification sees the post-round value.
fn format_g_9(d: f64) -> String {
    debug_assert!(d.is_finite(), "non-finite caught by TryWriteDouble");
    // Signed-zero path. C `%.9G` on `-0.0` (glibc) emits `"-0"`; on `+0.0`
    // it emits `"0"`. We preserve the sign here; the caller appends `.0`.
    if d == 0.0 {
        return if d.is_sign_negative() {
            "-0".to_string()
        } else {
            "0".to_string()
        };
    }

    let precision: i32 = 9;
    // `{:.8e}` → 9 sig digits, post-rounding. E.g. `9.9999999995e9` →
    // `"1.00000000e10"` (mantissa carried into next exponent).
    let sci = format!("{:.*e}", (precision - 1) as usize, d);
    // Split into mantissa and exponent on Rust's lowercase `e`.
    let (mantissa, exp_str) = sci.split_once('e').expect("{:e} always emits an 'e'");
    let exp: i32 = exp_str.parse().expect("{:e} exponent is a signed int");

    if exp < -4 || exp >= precision {
        // Scientific. Strip trailing zeros from the mantissa, reformat with
        // uppercase `E` and a two-digit signed exponent (C `%G` style).
        let m_stripped = strip_trailing_zeros(mantissa);
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}E{}{:02}", m_stripped, sign, exp.abs())
    } else {
        // Fixed. Decimals = precision - 1 - exp (clamped to 0). Refomat
        // straight from `d` with that many decimals — uses Rust's own
        // rounding, which matches IEEE round-half-to-even like glibc.
        let decimals = (precision - 1 - exp).max(0) as usize;
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
    fn format_g_9_matches_c_reference() {
        // Reference values from `printf "%.9G" <v>` on glibc — load-bearing
        // fixtures. The writer suffixes `.0` on tokens lacking both `.` and
        // `E`/`e` so they re-lex as ET_DOUBLE; we mirror that here.
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (0.5, "0.5"),
            (1e-9, "1E-09"),
            (1e9, "1E+09"),
            (1e-4, "0.0001"),
            (1e-5, "1E-05"),
            (9.99999999, "9.99999999"),
            (1234567890.0, "1.23456789E+09"),
        ];
        for (input, expected) in cases {
            let got = format_g_9(*input);
            assert_eq!(&got, expected, "format_g_9({input})");
        }
    }

    #[test]
    fn format_g_9_preserves_negative_zero() {
        // Raw token preserves sign; TryWriteDouble appends `.0`.
        assert_eq!(format_g_9(-0.0), "-0");
        assert_eq!(format_g_9(0.0), "0");

        let mut w = emRecMemWriter::new();
        w.TryWriteDouble(-0.0).unwrap();
        assert_eq!(w.into_bytes(), b"-0.0");

        let mut w = emRecMemWriter::new();
        w.TryWriteDouble(0.0).unwrap();
        assert_eq!(w.into_bytes(), b"0.0");
    }

    #[test]
    fn format_g_9_refuses_non_finite() {
        let mut w = emRecMemWriter::new();
        assert!(w.TryWriteDouble(f64::NAN).is_err());
        assert!(w.TryWriteDouble(f64::INFINITY).is_err());
        assert!(w.TryWriteDouble(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn quoted_preserves_embedded_nul_via_octal() {
        // DIVERGED from C++ (truncates at NUL). See TryWriteQuoted comment.
        let mut w = emRecMemWriter::new();
        w.TryWriteQuoted("foo\0bar").unwrap();
        assert_eq!(w.into_bytes(), b"\"foo\\000bar\"");
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
