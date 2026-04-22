//! emFileStream — buffered file I/O, ported from emFileStream.h
//!
//! C++ emFileStream wraps FILE* with an 8KB user-space buffer for
//! efficient small reads/writes. Rust wraps std::fs::File with a
//! manual byte buffer.
//!
//! DIVERGED: (language-forced) File paths use PathBuf/&Path (not String) to handle
//! non-UTF-8 file paths safely. C++ uses emString (byte-oriented).
//!
//! DIVERGED: (language-forced) C++ mode strings ("rb", "wb") mapped to Rust
//! OpenOptions. Only "rb", "wb", "r+b", "w+b" supported.
//!
//! DIVERGED: (language-forced) TryGetFile() omitted — no safe way to expose the
//! underlying File without breaking buffer invariants.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const DEFAULT_BUF_SIZE: usize = 8192;

#[derive(Debug)]
pub enum FileStreamError {
    NotOpen,
    IoError(std::io::Error),
    InvalidMode(String),
    Eof,
}

impl fmt::Display for FileStreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOpen => write!(f, "file stream not open"),
            Self::IoError(e) => write!(f, "I/O error: {e}"),
            Self::InvalidMode(m) => write!(f, "invalid mode: {m}"),
            Self::Eof => write!(f, "unexpected end of file"),
        }
    }
}

impl std::error::Error for FileStreamError {}

impl From<std::io::Error> for FileStreamError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

pub type Result<T> = std::result::Result<T, FileStreamError>;

/// Buffered file stream matching C++ `emFileStream`.
pub struct emFileStream {
    file: Option<File>,
    path: PathBuf,
    buf: Vec<u8>,
    buf_pos: usize,
    /// For reading: number of valid bytes in buf. For writing: same as buf_pos.
    buf_end: usize,
    writing: bool,
}

impl emFileStream {
    pub fn new() -> Self {
        Self {
            file: None,
            path: PathBuf::new(),
            buf: vec![0u8; DEFAULT_BUF_SIZE],
            buf_pos: 0,
            buf_end: 0,
            writing: false,
        }
    }

    pub fn with_buf_size(buf_size: usize) -> Self {
        Self {
            file: None,
            path: PathBuf::new(),
            buf: vec![0u8; buf_size.max(64)],
            buf_pos: 0,
            buf_end: 0,
            writing: false,
        }
    }

    // --- Open / Close ---

    /// Open a file. C++ modes: "rb", "wb", "r+b", "w+b".
    pub fn TryOpen(&mut self, path: &Path, mode: &str) -> Result<()> {
        if self.file.is_some() {
            self.TryClose()?;
        }
        let file = match mode {
            "rb" | "r" => OpenOptions::new().read(true).open(path)?,
            "wb" | "w" => OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?,
            "r+b" | "r+" => OpenOptions::new().read(true).write(true).open(path)?,
            "w+b" | "w+" => OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?,
            _ => return Err(FileStreamError::InvalidMode(mode.to_string())),
        };
        self.file = Some(file);
        self.path = path.to_path_buf();
        self.buf_pos = 0;
        self.buf_end = 0;
        self.writing = false;
        Ok(())
    }

    pub fn TryClose(&mut self) -> Result<()> {
        if self.writing {
            self.TryFlush()?;
        }
        self.file = None;
        self.path = PathBuf::new();
        self.buf_pos = 0;
        self.buf_end = 0;
        self.writing = false;
        Ok(())
    }

    /// DIVERGED: (language-forced) C++ Close() ignores errors; Rust version does the same.
    pub fn Close(&mut self) {
        let _ = self.TryClose();
    }

    pub fn IsOpen(&self) -> bool {
        self.file.is_some()
    }

    // --- Seek / Tell ---

    pub fn TryTell(&mut self) -> Result<i64> {
        let f = self.file.as_mut().ok_or(FileStreamError::NotOpen)?;
        let file_pos = f.stream_position()? as i64;
        if self.writing {
            Ok(file_pos + self.buf_pos as i64)
        } else {
            Ok(file_pos - (self.buf_end as i64 - self.buf_pos as i64))
        }
    }

    pub fn TrySeek(&mut self, pos: i64) -> Result<()> {
        self.flush_and_reset_buffer()?;
        let f = self.file.as_mut().ok_or(FileStreamError::NotOpen)?;
        f.seek(SeekFrom::Start(pos as u64))?;
        Ok(())
    }

    pub fn TrySeekEnd(&mut self, pos_from_end: i64) -> Result<()> {
        self.flush_and_reset_buffer()?;
        let f = self.file.as_mut().ok_or(FileStreamError::NotOpen)?;
        f.seek(SeekFrom::End(-pos_from_end))?;
        Ok(())
    }

    pub fn TrySkip(&mut self, offset: i64) -> Result<()> {
        let pos = self.TryTell()? + offset;
        self.TrySeek(pos)
    }

    // --- Read ---

    pub fn TryRead(&mut self, buf: &mut [u8]) -> Result<()> {
        self.ensure_reading()?;
        let mut filled = 0;
        while filled < buf.len() {
            if self.buf_pos >= self.buf_end {
                self.fill_read_buffer()?;
                if self.buf_pos >= self.buf_end {
                    return Err(FileStreamError::Eof);
                }
            }
            let avail = self.buf_end - self.buf_pos;
            let need = buf.len() - filled;
            let n = avail.min(need);
            buf[filled..filled + n].copy_from_slice(&self.buf[self.buf_pos..self.buf_pos + n]);
            self.buf_pos += n;
            filled += n;
        }
        Ok(())
    }

    pub fn TryReadAtMost(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.ensure_reading()?;
        if self.buf_pos >= self.buf_end {
            self.fill_read_buffer()?;
        }
        let avail = self.buf_end - self.buf_pos;
        let n = avail.min(buf.len());
        buf[..n].copy_from_slice(&self.buf[self.buf_pos..self.buf_pos + n]);
        self.buf_pos += n;
        Ok(n)
    }

    pub fn TryReadLine(&mut self, remove_line_break: bool) -> Result<String> {
        self.ensure_reading()?;
        let mut line = String::new();
        loop {
            if self.buf_pos >= self.buf_end {
                self.fill_read_buffer()?;
                if self.buf_pos >= self.buf_end {
                    // EOF — return what we have (empty string if nothing read)
                    if line.is_empty() {
                        return Err(FileStreamError::Eof);
                    }
                    return Ok(line);
                }
            }
            let b = self.buf[self.buf_pos];
            self.buf_pos += 1;
            if b == b'\n' {
                if !remove_line_break {
                    line.push('\n');
                }
                return Ok(line);
            }
            if b == b'\r' {
                if !remove_line_break {
                    line.push('\r');
                }
                // Check for \r\n
                if self.buf_pos >= self.buf_end {
                    self.fill_read_buffer()?;
                }
                if self.buf_pos < self.buf_end && self.buf[self.buf_pos] == b'\n' {
                    self.buf_pos += 1;
                    if !remove_line_break {
                        line.push('\n');
                    }
                }
                return Ok(line);
            }
            line.push(b as char);
        }
    }

    pub fn TryReadCharOrEOF(&mut self) -> Result<i32> {
        self.ensure_reading()?;
        if self.buf_pos >= self.buf_end {
            self.fill_read_buffer()?;
            if self.buf_pos >= self.buf_end {
                return Ok(-1);
            }
        }
        let b = self.buf[self.buf_pos];
        self.buf_pos += 1;
        Ok(b as i32)
    }

    /// DIVERGED: (language-forced) C++ TryReadInt8 returns emInt8. Rust uses i8.
    pub fn TryReadInt8(&mut self) -> Result<i8> {
        Ok(self.TryReadUInt8()? as i8)
    }

    /// Read a single byte.
    pub fn TryReadUInt8(&mut self) -> Result<u8> {
        self.ensure_reading()?;
        if self.buf_pos >= self.buf_end {
            self.fill_read_buffer()?;
            if self.buf_pos >= self.buf_end {
                return Err(FileStreamError::Eof);
            }
        }
        let b = self.buf[self.buf_pos];
        self.buf_pos += 1;
        Ok(b)
    }

    // --- Write ---

    pub fn TryWrite(&mut self, data: &[u8]) -> Result<()> {
        self.ensure_writing()?;
        let mut written = 0;
        while written < data.len() {
            let space = self.buf.len() - self.buf_pos;
            if space == 0 {
                self.flush_write_buffer()?;
                continue;
            }
            let n = space.min(data.len() - written);
            self.buf[self.buf_pos..self.buf_pos + n].copy_from_slice(&data[written..written + n]);
            self.buf_pos += n;
            self.buf_end = self.buf_pos;
            written += n;
        }
        Ok(())
    }

    /// DIVERGED: (language-forced) C++ TryWrite(const char*) takes a C string. Rust uses &str.
    pub fn TryWriteStr(&mut self, s: &str) -> Result<()> {
        self.TryWrite(s.as_bytes())
    }

    pub fn TryWriteChar(&mut self, value: u8) -> Result<()> {
        self.TryWrite(&[value])
    }

    /// DIVERGED: (language-forced) C++ TryWriteInt8 takes emInt8. Rust uses i8.
    pub fn TryWriteInt8(&mut self, value: i8) -> Result<()> {
        self.TryWriteUInt8(value as u8)
    }

    pub fn TryWriteUInt8(&mut self, value: u8) -> Result<()> {
        self.TryWrite(&[value])
    }

    pub fn TryFlush(&mut self) -> Result<()> {
        if self.writing {
            self.flush_write_buffer()?;
            let f = self.file.as_mut().ok_or(FileStreamError::NotOpen)?;
            f.flush()?;
        }
        Ok(())
    }

    // --- Internal buffer management ---

    fn ensure_reading(&mut self) -> Result<()> {
        if self.file.is_none() {
            return Err(FileStreamError::NotOpen);
        }
        if self.writing {
            self.flush_write_buffer()?;
            self.writing = false;
        }
        Ok(())
    }

    fn ensure_writing(&mut self) -> Result<()> {
        if self.file.is_none() {
            return Err(FileStreamError::NotOpen);
        }
        if !self.writing {
            // If we had read-ahead data, seek back to logical position
            if self.buf_pos < self.buf_end {
                let f = self.file.as_mut().ok_or(FileStreamError::NotOpen)?;
                let rewind = self.buf_end as i64 - self.buf_pos as i64;
                f.seek(SeekFrom::Current(-rewind))?;
            }
            self.buf_pos = 0;
            self.buf_end = 0;
            self.writing = true;
        }
        Ok(())
    }

    fn fill_read_buffer(&mut self) -> Result<()> {
        let f = self.file.as_mut().ok_or(FileStreamError::NotOpen)?;
        let n = f.read(&mut self.buf)?;
        self.buf_pos = 0;
        self.buf_end = n;
        Ok(())
    }

    fn flush_write_buffer(&mut self) -> Result<()> {
        if self.buf_pos > 0 {
            let f = self.file.as_mut().ok_or(FileStreamError::NotOpen)?;
            f.write_all(&self.buf[..self.buf_pos])?;
            self.buf_pos = 0;
            self.buf_end = 0;
        }
        Ok(())
    }

    fn flush_and_reset_buffer(&mut self) -> Result<()> {
        if self.writing {
            self.flush_write_buffer()?;
        } else if self.buf_pos < self.buf_end {
            // Seek back to logical position (undo read-ahead)
            let f = self.file.as_mut().ok_or(FileStreamError::NotOpen)?;
            let rewind = self.buf_end as i64 - self.buf_pos as i64;
            f.seek(SeekFrom::Current(-rewind))?;
        }
        self.buf_pos = 0;
        self.buf_end = 0;
        self.writing = false;
        Ok(())
    }
}

// --- Endian-aware typed reads and writes (16/32/64-bit) ---

impl emFileStream {
    fn read_exact_bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut bytes = [0u8; N];
        self.TryRead(&mut bytes)?;
        Ok(bytes)
    }

    // --- Typed reads ---

    pub fn TryReadInt16LE(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadInt16BE(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadUInt16LE(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadUInt16BE(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadInt32LE(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadInt32BE(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadUInt32LE(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadUInt32BE(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadInt64LE(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadInt64BE(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadUInt64LE(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_exact_bytes()?))
    }
    pub fn TryReadUInt64BE(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.read_exact_bytes()?))
    }

    // --- Typed writes ---

    pub fn TryWriteInt16LE(&mut self, v: i16) -> Result<()> {
        self.TryWrite(&v.to_le_bytes())
    }
    pub fn TryWriteInt16BE(&mut self, v: i16) -> Result<()> {
        self.TryWrite(&v.to_be_bytes())
    }
    pub fn TryWriteUInt16LE(&mut self, v: u16) -> Result<()> {
        self.TryWrite(&v.to_le_bytes())
    }
    pub fn TryWriteUInt16BE(&mut self, v: u16) -> Result<()> {
        self.TryWrite(&v.to_be_bytes())
    }
    pub fn TryWriteInt32LE(&mut self, v: i32) -> Result<()> {
        self.TryWrite(&v.to_le_bytes())
    }
    pub fn TryWriteInt32BE(&mut self, v: i32) -> Result<()> {
        self.TryWrite(&v.to_be_bytes())
    }
    pub fn TryWriteUInt32LE(&mut self, v: u32) -> Result<()> {
        self.TryWrite(&v.to_le_bytes())
    }
    pub fn TryWriteUInt32BE(&mut self, v: u32) -> Result<()> {
        self.TryWrite(&v.to_be_bytes())
    }
    pub fn TryWriteInt64LE(&mut self, v: i64) -> Result<()> {
        self.TryWrite(&v.to_le_bytes())
    }
    pub fn TryWriteInt64BE(&mut self, v: i64) -> Result<()> {
        self.TryWrite(&v.to_be_bytes())
    }
    pub fn TryWriteUInt64LE(&mut self, v: u64) -> Result<()> {
        self.TryWrite(&v.to_le_bytes())
    }
    pub fn TryWriteUInt64BE(&mut self, v: u64) -> Result<()> {
        self.TryWrite(&v.to_be_bytes())
    }
}

impl Default for emFileStream {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for emFileStream {
    fn drop(&mut self) {
        if self.file.is_some() {
            let _ = self.TryClose();
        }
    }
}
