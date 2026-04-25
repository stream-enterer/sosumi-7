//! SPLIT: extracted from `emRec.rs` at Phase 4a Task 2 pre-step.
//!
//! This file hosts the textual emRec record parser/serializer (`RecStruct`,
//! `RecValue`, `parse_rec`, `write_rec`, `RecError`). In C++ this functionality
//! lives in `emRecReader` / `emRecWriter` (see `include/emCore/emRec.h:32-33`).
//! The `emRec.rs` file itself is reserved for the `emRec<T>` trait (C++
//! `class emRec`, emRec.h:52), per the File and Name Correspondence rule
//! that the primary file keeps the C++ header's primary class name.

use std::fmt;

/// Errors from emRec parse/load operations.
#[derive(Debug)]
pub enum RecError {
    Parse { line: usize, message: String },
    MissingField(String),
    InvalidValue { field: String, message: String },
    Io(std::io::Error),
}

impl fmt::Display for RecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { line, message } => write!(f, "parse error at line {line}: {message}"),
            Self::MissingField(name) => write!(f, "missing field: {name}"),
            Self::InvalidValue { field, message } => {
                write!(f, "invalid value for '{field}': {message}")
            }
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for RecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

/// A named-field record (struct in emRec terms).
#[derive(Debug, Clone)]
pub struct RecStruct {
    fields: Vec<(String, RecValue)>,
}

/// A single emRec value.
#[derive(Debug, Clone)]
pub enum RecValue {
    Bool(bool),
    Int(i32),
    Double(f64),
    Str(String),
    Struct(RecStruct),
    Array(Vec<RecValue>),
    /// Union variant: name (lowercase) + inner value.
    Union(String, Box<RecValue>),
    /// Unresolved identifier (enum/flags/alignment parts).
    Ident(String),
}

impl Default for RecStruct {
    fn default() -> Self {
        Self::new()
    }
}

impl RecStruct {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    fn push(&mut self, name: &str, val: RecValue) {
        self.fields.push((name.to_ascii_lowercase(), val));
    }

    pub fn set_bool(&mut self, name: &str, val: bool) {
        self.push(name, RecValue::Bool(val));
    }

    pub fn set_int(&mut self, name: &str, val: i32) {
        self.push(name, RecValue::Int(val));
    }

    pub fn set_double(&mut self, name: &str, val: f64) {
        self.push(name, RecValue::Double(val));
    }

    pub fn set_str(&mut self, name: &str, val: &str) {
        self.push(name, RecValue::Str(val.to_string()));
    }

    pub fn set_ident(&mut self, name: &str, val: &str) {
        self.push(name, RecValue::Ident(val.to_ascii_lowercase()));
    }

    pub fn SetValue(&mut self, name: &str, val: RecValue) {
        self.push(name, val);
    }

    /// Remove all fields with the given name (case-insensitive). Used by
    /// callers that want replace-semantics on top of the push-based
    /// `SetValue` (e.g. `emTreeDump::set_children`).
    pub fn remove_field(&mut self, name: &str) {
        self.fields.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
    }

    fn GetRec(&self, name: &str) -> Option<&RecValue> {
        self.fields
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.GetRec(name)? {
            RecValue::Bool(b) => Some(*b),
            RecValue::Int(0) => Some(false),
            RecValue::Int(1) => Some(true),
            _ => None,
        }
    }

    pub fn get_int(&self, name: &str) -> Option<i32> {
        match self.GetRec(name)? {
            RecValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn get_double(&self, name: &str) -> Option<f64> {
        match self.GetRec(name)? {
            RecValue::Double(d) => Some(*d),
            RecValue::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn get_str(&self, name: &str) -> Option<&str> {
        match self.GetRec(name)? {
            RecValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn get_struct(&self, name: &str) -> Option<&RecStruct> {
        match self.GetRec(name)? {
            RecValue::Struct(s) => Some(s),
            _ => None,
        }
    }

    pub fn get_array(&self, name: &str) -> Option<&Vec<RecValue>> {
        match self.GetRec(name)? {
            RecValue::Array(v) => Some(v),
            _ => None,
        }
    }

    pub fn get_ident(&self, name: &str) -> Option<&str> {
        match self.GetRec(name)? {
            RecValue::Ident(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn fields(&self) -> &[(String, RecValue)] {
        &self.fields
    }
}

// ─── Lexer ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Token {
    Ident(String),
    Int(i32),
    Double(f64),
    Quoted(String),
    Delim(char),
    Eof,
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            line: 1,
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            match self.peek_char() {
                Some(c) if c.is_whitespace() => {
                    self.next_char();
                }
                Some('#') => {
                    while self.peek_char().map(|c| c != '\n').unwrap_or(false) {
                        self.next_char();
                    }
                }
                _ => break,
            }
        }
    }

    fn next_token(&mut self) -> Result<(Token, usize), RecError> {
        self.skip_ws_and_comments();
        let line = self.line;
        match self.peek_char() {
            None => Ok((Token::Eof, line)),
            Some('{') | Some('}') | Some('=') | Some(':') => {
                let c = self.next_char().unwrap();
                Ok((Token::Delim(c), line))
            }
            Some('"') => {
                self.next_char();
                let s = self.read_quoted(line)?;
                Ok((Token::Quoted(s), line))
            }
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                Ok((Token::Ident(self.read_ident()), line))
            }
            Some(c) if c.is_ascii_digit() => self.read_number(line),
            Some(c @ '+') | Some(c @ '-') => {
                // Treat as number start if a digit follows; otherwise
                // treat as a delimiter (matches C++ emRecReader::TryParseNext
                // emRec.cpp:2449-2454, where lone +/- becomes ET_DELIMITER).
                // Used by emAlignmentRec ("top-right", "bottom-left", etc.).
                let rest = &self.input[self.pos + c.len_utf8()..];
                if rest.starts_with(|d: char| d.is_ascii_digit()) {
                    self.read_number(line)
                } else {
                    self.next_char();
                    Ok((Token::Delim(c), line))
                }
            }
            Some(c) => {
                // C++ treats any other unknown char as a delimiter
                // (emRec.cpp:2476-2479). Match that behavior.
                self.next_char();
                Ok((Token::Delim(c), line))
            }
        }
    }

    fn read_ident(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek_char() {
            if c.is_ascii_alphanumeric() || c == '_' {
                s.push(c);
                self.next_char();
            } else {
                break;
            }
        }
        s
    }

    fn read_number(&mut self, line: usize) -> Result<(Token, usize), RecError> {
        let mut s = String::new();
        let mut is_double = false;

        if matches!(self.peek_char(), Some('+') | Some('-')) {
            s.push(self.next_char().unwrap());
        }
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                s.push(c);
                self.next_char();
            } else {
                break;
            }
        }
        if self.peek_char() == Some('.') {
            is_double = true;
            s.push('.');
            self.next_char();
            while let Some(c) = self.peek_char() {
                if c.is_ascii_digit() {
                    s.push(c);
                    self.next_char();
                } else {
                    break;
                }
            }
        }
        if matches!(self.peek_char(), Some('e') | Some('E')) {
            is_double = true;
            s.push(self.next_char().unwrap());
            if matches!(self.peek_char(), Some('+') | Some('-')) {
                s.push(self.next_char().unwrap());
            }
            while let Some(c) = self.peek_char() {
                if c.is_ascii_digit() {
                    s.push(c);
                    self.next_char();
                } else {
                    break;
                }
            }
        }

        if is_double {
            s.parse::<f64>()
                .map(|d| (Token::Double(d), line))
                .map_err(|_| RecError::Parse {
                    line,
                    message: format!("invalid float: {s}"),
                })
        } else {
            s.parse::<i32>()
                .map(|n| (Token::Int(n), line))
                .map_err(|_| RecError::Parse {
                    line,
                    message: format!("invalid integer: {s}"),
                })
        }
    }

    fn read_quoted(&mut self, line: usize) -> Result<String, RecError> {
        let mut s = String::new();
        loop {
            match self.next_char() {
                None => {
                    return Err(RecError::Parse {
                        line,
                        message: "unterminated string".into(),
                    })
                }
                Some('"') => break,
                Some('\\') => match self.next_char() {
                    Some('n') => s.push('\n'),
                    Some('r') => s.push('\r'),
                    Some('t') => s.push('\t'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('a') => s.push('\x07'),
                    Some('b') => s.push('\x08'),
                    Some('e') => s.push('\x1b'),
                    Some('f') => s.push('\x0c'),
                    Some('v') => s.push('\x0b'),
                    Some('x') => {
                        let h1 = self.next_char().ok_or_else(|| RecError::Parse {
                            line,
                            message: "incomplete \\x escape".into(),
                        })?;
                        let h2 = self.next_char().ok_or_else(|| RecError::Parse {
                            line,
                            message: "incomplete \\x escape".into(),
                        })?;
                        let code = u8::from_str_radix(&format!("{h1}{h2}"), 16).map_err(|_| {
                            RecError::Parse {
                                line,
                                message: format!("invalid \\x{h1}{h2}"),
                            }
                        })?;
                        s.push(code as char);
                    }
                    Some(c) if ('0'..='7').contains(&c) => {
                        let mut oct = c.to_string();
                        for _ in 0..2 {
                            if self
                                .peek_char()
                                .map(|d| ('0'..='7').contains(&d))
                                .unwrap_or(false)
                            {
                                oct.push(self.next_char().unwrap());
                            } else {
                                break;
                            }
                        }
                        let code = u8::from_str_radix(&oct, 8).map_err(|_| RecError::Parse {
                            line,
                            message: format!("invalid octal \\{oct}"),
                        })?;
                        s.push(code as char);
                    }
                    Some(c) => s.push(c),
                    None => {
                        return Err(RecError::Parse {
                            line,
                            message: "incomplete escape".into(),
                        })
                    }
                },
                Some(c) => s.push(c),
            }
        }
        Ok(s)
    }
}

fn tokenize(input: &str) -> Result<Vec<(Token, usize)>, RecError> {
    let mut lexer = Lexer::new(input);
    let mut tokens = Vec::new();
    loop {
        let (tok, line) = lexer.next_token()?;
        let is_eof = matches!(tok, Token::Eof);
        tokens.push((tok, line));
        if is_eof {
            break;
        }
    }
    Ok(tokens)
}

// ─── Parser ──────────────────────────────────────────────────────────────────

struct Parser {
    tokens: Vec<(Token, usize)>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .map(|(t, _)| t)
            .unwrap_or(&Token::Eof)
    }

    fn peek_after_one(&self) -> &Token {
        self.tokens
            .get(self.pos + 1)
            .map(|(t, _)| t)
            .unwrap_or(&Token::Eof)
    }

    fn peek_line(&self) -> usize {
        self.tokens.get(self.pos).map(|(_, l)| *l).unwrap_or(0)
    }

    fn consume(&mut self) -> Token {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].0.clone();
            self.pos += 1;
            tok
        } else {
            Token::Eof
        }
    }

    fn expect_ident(&mut self) -> Result<String, RecError> {
        let line = self.peek_line();
        match self.consume() {
            Token::Ident(s) => Ok(s),
            other => Err(RecError::Parse {
                line,
                message: format!("expected identifier, got {other:?}"),
            }),
        }
    }

    fn expect_delim(&mut self, ch: char) -> Result<(), RecError> {
        let line = self.peek_line();
        match self.consume() {
            Token::Delim(c) if c == ch => Ok(()),
            other => Err(RecError::Parse {
                line,
                message: format!("expected '{ch}', got {other:?}"),
            }),
        }
    }

    fn parse_top_level(&mut self) -> Result<RecStruct, RecError> {
        // Detect whether the top level is a struct (Ident '=' ...) or a
        // top-level array of union values (Ident ':' ...).  C++ emArrayRec
        // configs (e.g. emBookmarks) use the union-array form.
        let is_array = matches!(
            (self.tokens.get(self.pos), self.tokens.get(self.pos + 1)),
            (Some((Token::Ident(_), _)), Some((Token::Delim(':'), _)))
        );

        if is_array {
            let mut elements = Vec::new();
            loop {
                match self.peek() {
                    Token::Eof => break,
                    _ => {
                        let val = self.parse_value()?;
                        elements.push(val);
                    }
                }
            }
            // Store as a synthetic "_array" field so callers can detect
            // a top-level array config.
            let fields = vec![("_array".to_string(), RecValue::Array(elements))];
            Ok(RecStruct { fields })
        } else {
            let mut fields = Vec::new();
            loop {
                match self.peek() {
                    Token::Eof => break,
                    Token::Ident(_) => {
                        let name = self.expect_ident()?.to_ascii_lowercase();
                        self.expect_delim('=')?;
                        let val = self.parse_value()?;
                        fields.push((name, val));
                    }
                    _ => {
                        let line = self.peek_line();
                        return Err(RecError::Parse {
                            line,
                            message: format!("expected field name, got {:?}", self.peek()),
                        });
                    }
                }
            }
            Ok(RecStruct { fields })
        }
    }

    fn parse_braced(&mut self) -> Result<RecValue, RecError> {
        // Peek 2 tokens to distinguish struct (Ident '=') from array.
        let is_struct = if let Some((Token::Ident(_), _)) = self.tokens.get(self.pos) {
            matches!(self.tokens.get(self.pos + 1), Some((Token::Delim('='), _)))
        } else {
            false
        };

        if is_struct {
            let mut fields = Vec::new();
            loop {
                match self.peek() {
                    Token::Delim('}') => {
                        self.consume();
                        break;
                    }
                    Token::Eof => {
                        return Err(RecError::Parse {
                            line: self.peek_line(),
                            message: "unexpected EOF in struct".into(),
                        })
                    }
                    _ => {
                        let name = self.expect_ident()?.to_ascii_lowercase();
                        self.expect_delim('=')?;
                        let val = self.parse_value()?;
                        fields.push((name, val));
                    }
                }
            }
            Ok(RecValue::Struct(RecStruct { fields }))
        } else {
            let mut elements = Vec::new();
            loop {
                match self.peek() {
                    Token::Delim('}') => {
                        self.consume();
                        break;
                    }
                    Token::Eof => {
                        return Err(RecError::Parse {
                            line: self.peek_line(),
                            message: "unexpected EOF in array".into(),
                        })
                    }
                    _ => {
                        elements.push(self.parse_value()?);
                    }
                }
            }
            Ok(RecValue::Array(elements))
        }
    }

    fn parse_value(&mut self) -> Result<RecValue, RecError> {
        let line = self.peek_line();
        match self.consume() {
            Token::Delim('{') => self.parse_braced(),
            Token::Quoted(s) => Ok(RecValue::Str(s)),
            Token::Int(n) => Ok(RecValue::Int(n)),
            Token::Double(d) => Ok(RecValue::Double(d)),
            Token::Ident(s) => {
                if matches!(self.peek(), Token::Delim(':')) {
                    self.consume();
                    let inner = self.parse_value()?;
                    Ok(RecValue::Union(s.to_ascii_lowercase(), Box::new(inner)))
                } else {
                    // Concatenate consecutive `-ident` tokens into a single
                    // hyphen-joined ident. C++ emAlignmentRec reads "bottom-left"
                    // as two idents with a '-' delimiter between. We flatten
                    // that back into one RecValue::Ident like "bottom-left".
                    let mut combined = s.to_ascii_lowercase();
                    while matches!(self.peek(), Token::Delim('-')) {
                        // Look ahead: only consume '-' if followed by an ident.
                        if let Token::Ident(_) = self.peek_after_one() {
                            self.consume(); // consume '-'
                            if let Token::Ident(next_s) = self.consume() {
                                combined.push('-');
                                combined.push_str(&next_s.to_ascii_lowercase());
                            }
                        } else {
                            break;
                        }
                    }
                    match combined.as_str() {
                        "yes" | "true" | "y" => Ok(RecValue::Bool(true)),
                        "no" | "false" | "n" => Ok(RecValue::Bool(false)),
                        _ => Ok(RecValue::Ident(combined)),
                    }
                }
            }
            Token::Eof => Err(RecError::Parse {
                line,
                message: "unexpected EOF".into(),
            }),
            Token::Delim(c) => Err(RecError::Parse {
                line,
                message: format!("unexpected '{c}'"),
            }),
        }
    }
}

// ─── Writer ──────────────────────────────────────────────────────────────────

fn format_double(d: f64) -> String {
    if d == 0.0 {
        return "0.0".into();
    }

    let exp = d.abs().log10().floor() as i32;

    let result = if (-4..9).contains(&exp) {
        let dec = (8 - exp).max(0) as usize;
        let s = format!("{:.prec$}", d, prec = dec);
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    } else {
        let s = format!("{:.8e}", d);
        let e_pos = s.find('e').unwrap();
        let mantissa = s[..e_pos].trim_end_matches('0').trim_end_matches('.');
        let exp_num: i32 = s[e_pos + 1..].parse().unwrap();
        format!("{mantissa}e{exp_num}")
    };

    if !result.contains('.') && !result.contains('e') {
        format!("{result}.0")
    } else {
        result
    }
}

fn write_value(out: &mut String, val: &RecValue, indent: usize) {
    match val {
        RecValue::Bool(b) => out.push_str(if *b { "yes" } else { "no" }),
        RecValue::Int(n) => out.push_str(&n.to_string()),
        RecValue::Double(d) => out.push_str(&format_double(*d)),
        RecValue::Str(s) => {
            out.push('"');
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    '\x07' => out.push_str("\\a"),
                    '\x08' => out.push_str("\\b"),
                    '\x1b' => out.push_str("\\e"),
                    '\x0c' => out.push_str("\\f"),
                    '\x0b' => out.push_str("\\v"),
                    c if c.is_ascii() && (c as u8) < 0x20 => {
                        out.push_str(&format!("\\x{:02x}", c as u8));
                    }
                    c => out.push(c),
                }
            }
            out.push('"');
        }
        RecValue::Ident(s) => out.push_str(s),
        RecValue::Union(name, inner) => {
            out.push_str(name);
            out.push_str(": ");
            write_value(out, inner, indent);
        }
        RecValue::Struct(s) => {
            out.push_str("{\n");
            let tab = "\t".repeat(indent + 1);
            for (name, v) in s.fields() {
                out.push_str(&tab);
                out.push_str(name);
                out.push_str(" = ");
                write_value(out, v, indent + 1);
                out.push('\n');
            }
            out.push_str(&"\t".repeat(indent));
            out.push('}');
        }
        RecValue::Array(items) => {
            out.push_str("{\n");
            let tab = "\t".repeat(indent + 1);
            for item in items {
                out.push_str(&tab);
                write_value(out, item, indent + 1);
                out.push('\n');
            }
            out.push_str(&"\t".repeat(indent));
            out.push('}');
        }
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Parse emRec text into a `RecStruct`.
///
/// An optional `#%rec:FormatName%#` header line is treated as a comment
/// and silently ignored.
pub fn parse_rec(input: &str) -> Result<RecStruct, RecError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_top_level()
}

/// Parse emRec text and verify the format header matches `format_name`.
///
/// If the file starts with `#%rec:FormatName%#`, the name must match.
/// If no header is present the format name is not checked.
pub fn parse_rec_with_format(input: &str, format_name: &str) -> Result<RecStruct, RecError> {
    let first = input.lines().next().unwrap_or("").trim();
    if first.starts_with("#%rec:") {
        let expected = format!("#%rec:{format_name}%#");
        if !first.starts_with(expected.as_str()) {
            return Err(RecError::Parse {
                line: 1,
                message: format!("expected format header '#%rec:{format_name}%#'"),
            });
        }
    }
    parse_rec(input)
}

/// Serialize a `RecStruct` to emRec text (no format header).
///
/// If the struct has a single `_array` field (top-level array config), the
/// array elements are written directly as bare values (union entries).
pub fn write_rec(rec: &RecStruct) -> String {
    let mut out = String::new();

    // Detect top-level array (synthetic `_array` field from parse_top_level).
    if rec.fields.len() == 1 && rec.fields[0].0 == "_array" {
        if let RecValue::Array(items) = &rec.fields[0].1 {
            for item in items {
                write_value(&mut out, item, 0);
                out.push_str("\n\n");
            }
            return out;
        }
    }

    for (name, val) in rec.fields() {
        out.push_str(name);
        out.push_str(" = ");
        write_value(&mut out, val, 0);
        out.push('\n');
    }
    out
}

/// Serialize a `RecStruct` to emRec text with a `#%rec:FormatName%#` header.
pub fn write_rec_with_format(rec: &RecStruct, format_name: &str) -> String {
    let mut out = format!("#%rec:{format_name}%#\n\n");
    out.push_str(&write_rec(rec));
    out
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_round_trips() {
        let mut s = RecStruct::new();
        s.set_bool("flag", true);
        s.set_int("count", -42);
        s.set_double("ratio", 1.5);
        s.set_str("name", "hello\nworld");
        s.set_ident("align", "Center");

        let text = write_rec(&s);
        let back = parse_rec(&text).unwrap();

        assert_eq!(back.get_bool("flag"), Some(true));
        assert_eq!(back.get_int("count"), Some(-42));
        assert!((back.get_double("ratio").unwrap() - 1.5).abs() < 1e-9);
        assert_eq!(back.get_str("name"), Some("hello\nworld"));
        assert_eq!(back.get_ident("align"), Some("center"));
    }

    #[test]
    fn nested_struct_round_trip() {
        let mut inner = RecStruct::new();
        inner.set_int("x", 10);
        let mut outer = RecStruct::new();
        outer.SetValue("sub", RecValue::Struct(inner));

        let text = write_rec(&outer);
        let back = parse_rec(&text).unwrap();

        assert_eq!(
            back.get_struct("sub").and_then(|s| s.get_int("x")),
            Some(10)
        );
    }

    #[test]
    fn array_round_trip() {
        let mut s = RecStruct::new();
        s.SetValue(
            "items",
            RecValue::Array(vec![RecValue::Int(1), RecValue::Int(2), RecValue::Int(3)]),
        );

        let text = write_rec(&s);
        let back = parse_rec(&text).unwrap();

        let arr = back.get_array("items").unwrap();
        assert_eq!(arr.len(), 3);
        assert!(matches!(arr[0], RecValue::Int(1)));
    }

    #[test]
    fn union_round_trip() {
        let mut s = RecStruct::new();
        s.SetValue(
            "color",
            RecValue::Union("rgb".into(), Box::new(RecValue::Int(255))),
        );

        let text = write_rec(&s);
        let back = parse_rec(&text).unwrap();

        if let Some(RecValue::Union(name, inner)) = back.GetRec("color") {
            assert_eq!(name, "rgb");
            assert!(matches!(**inner, RecValue::Int(255)));
        } else {
            panic!("expected union");
        }
    }

    #[test]
    fn bool_aliases() {
        let text = "a = yes\nb = no\nc = true\nd = false\n";
        let s = parse_rec(text).unwrap();
        assert_eq!(s.get_bool("a"), Some(true));
        assert_eq!(s.get_bool("b"), Some(false));
        assert_eq!(s.get_bool("c"), Some(true));
        assert_eq!(s.get_bool("d"), Some(false));
    }

    #[test]
    fn format_header_ignored_by_parse_rec() {
        let text = "#%rec:MyFormat%#\n\nx = 1\n";
        let s = parse_rec(text).unwrap();
        assert_eq!(s.get_int("x"), Some(1));
    }

    #[test]
    fn parse_rec_with_format_checks_name() {
        let text = "#%rec:Geometry%#\n\nx = 1\n";
        assert!(parse_rec_with_format(text, "Geometry").is_ok());
        assert!(parse_rec_with_format(text, "Other").is_err());
    }

    #[test]
    fn write_rec_with_format_round_trip() {
        let mut s = RecStruct::new();
        s.set_int("x", 100);
        let text = write_rec_with_format(&s, "Config");
        let back = parse_rec_with_format(&text, "Config").unwrap();
        assert_eq!(back.get_int("x"), Some(100));
    }

    #[test]
    fn double_format_no_decimal_appends_point_zero() {
        assert_eq!(format_double(1.0), "1.0");
        assert_eq!(format_double(100.0), "100.0");
        assert_eq!(format_double(0.0), "0.0");
    }

    #[test]
    fn case_insensitive_lookup() {
        let text = "X = 10\nY = 20\n";
        let s = parse_rec(text).unwrap();
        assert_eq!(s.get_int("x"), Some(10));
        assert_eq!(s.get_int("X"), Some(10));
        assert_eq!(s.get_int("Y"), Some(20));
    }
}
