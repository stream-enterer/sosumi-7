// SPLIT: Split from emRec.h — record type definitions extracted
use std::path::Path;

use crate::emColor::emColor;
use crate::emRecParser::{
    parse_rec, parse_rec_with_format, write_rec, write_rec_with_format, RecError, RecStruct,
    RecValue,
};
use crate::emTiling::Alignment;

// ---- RecListener ----

/// Callback ID returned by `RecListenerList::add`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RecListenerId(u64);

/// A list of listeners that are notified when a record changes.
///
/// Port of C++ `emRecListener`. In the C++ code, listeners form a linked-list
/// chain attached to the record tree. In Rust we use a simple callback list
/// since the single-threaded Rc/RefCell ownership model makes linked-list
/// listener chains unnecessarily complex.
pub struct RecListenerList {
    next_id: u64,
    listeners: Vec<(RecListenerId, Box<dyn Fn()>)>,
}

impl Default for RecListenerList {
    fn default() -> Self {
        Self::new()
    }
}

impl RecListenerList {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            listeners: Vec::new(),
        }
    }

    /// Register a callback. Returns an ID that can be passed to `remove`.
    pub fn add(&mut self, callback: impl Fn() + 'static) -> RecListenerId {
        let id = RecListenerId(self.next_id);
        self.next_id += 1;
        self.listeners.push((id, Box::new(callback)));
        id
    }

    /// Remove a previously registered listener.
    pub fn remove(&mut self, id: RecListenerId) {
        self.listeners.retain(|(lid, _)| *lid != id);
    }

    /// Notify all listeners that the record changed.
    pub fn notify(&self) {
        for (_, cb) in &self.listeners {
            cb();
        }
    }

    /// Returns `true` if there are no listeners registered.
    pub fn IsEmpty(&self) -> bool {
        self.listeners.is_empty()
    }
}

// ---- emAlignmentRec ----

/// A record wrapping an `Alignment` enum value.
///
/// Port of C++ `emAlignmentRec`. Stores a current value and a default value,
/// and can be serialized to/from emRec format as an identifier.
#[derive(Clone, Debug)]
pub struct emAlignmentRec {
    value: Alignment,
    default_value: Alignment,
}

impl emAlignmentRec {
    pub fn new(default_value: Alignment) -> Self {
        Self {
            value: default_value,
            default_value,
        }
    }

    pub fn GetRec(&self) -> Alignment {
        self.value
    }

    pub fn Set(&mut self, value: Alignment) {
        self.value = value;
    }

    pub fn SetToDefault(&mut self) {
        self.value = self.default_value;
    }

    pub fn IsSetToDefault(&self) -> bool {
        self.value == self.default_value
    }

    pub fn default_value(&self) -> Alignment {
        self.default_value
    }

    /// Read from a `RecValue` (expected to be an `Ident`). C++ emAlignmentRec
    /// stores a bitmask combining TOP/BOTTOM/LEFT/RIGHT/CENTER. The Rust
    /// `Alignment` enum is single-axis, so we accept hyphen-joined forms
    /// like "bottom-left" and collapse to a single value by preferring the
    /// axis that corresponds to Start/End. For symmetric combinations we
    /// return Center.
    pub fn FromRecValue(val: &RecValue) -> Result<Alignment, RecError> {
        match val {
            RecValue::Ident(s) => {
                // Parse individual tokens from hyphen-joined form.
                let mut has_top = false;
                let mut has_bottom = false;
                let mut has_left = false;
                let mut has_right = false;
                let mut has_center = false;
                let mut has_stretch = false;
                for part in s.split('-') {
                    match part {
                        "top" => has_top = true,
                        "bottom" => has_bottom = true,
                        "left" => has_left = true,
                        "right" => has_right = true,
                        "center" => has_center = true,
                        "stretch" | "fill" => has_stretch = true,
                        "start" => has_left = true,
                        "end" => has_right = true,
                        "" => {}
                        other => {
                            return Err(RecError::InvalidValue {
                                field: "alignment".into(),
                                message: format!("unknown alignment part: {other}"),
                            });
                        }
                    }
                }
                if has_stretch {
                    return Ok(Alignment::Stretch);
                }
                // Prefer horizontal axis (left/right) for single-value enum.
                if has_left {
                    return Ok(Alignment::Start);
                }
                if has_right {
                    return Ok(Alignment::End);
                }
                if has_top {
                    return Ok(Alignment::Start);
                }
                if has_bottom {
                    return Ok(Alignment::End);
                }
                if has_center {
                    return Ok(Alignment::Center);
                }
                Err(RecError::InvalidValue {
                    field: "alignment".into(),
                    message: format!("unknown alignment: {s}"),
                })
            }
            _ => Err(RecError::InvalidValue {
                field: "alignment".into(),
                message: "expected identifier".into(),
            }),
        }
    }

    /// Convert to a `RecValue` identifier.
    pub fn ToRecValue(alignment: Alignment) -> RecValue {
        let s = match alignment {
            Alignment::Start => "start",
            Alignment::Center => "center",
            Alignment::End => "end",
            Alignment::Stretch => "stretch",
        };
        RecValue::Ident(s.into())
    }
}

impl Default for emAlignmentRec {
    fn default() -> Self {
        Self::new(Alignment::Center)
    }
}

// ---- emColorRec ----

/// A record wrapping a `emColor` value.
///
/// Port of C++ `emColorRec`. Stores a current value, a default value, and
/// whether the alpha channel should be serialized.
#[derive(Clone, Debug)]
pub struct emColorRec {
    value: emColor,
    default_value: emColor,
    have_alpha: bool,
}

impl emColorRec {
    pub fn new(default_value: emColor, have_alpha: bool) -> Self {
        let value = if have_alpha {
            default_value
        } else {
            emColor::rgba(
                default_value.GetRed(),
                default_value.GetGreen(),
                default_value.GetBlue(),
                255,
            )
        };
        Self {
            value,
            default_value: value,
            have_alpha,
        }
    }

    pub fn GetRec(&self) -> emColor {
        self.value
    }

    pub fn Set(&mut self, value: emColor) {
        if self.have_alpha {
            self.value = value;
        } else {
            self.value = emColor::rgba(value.GetRed(), value.GetGreen(), value.GetBlue(), 255);
        }
    }

    pub fn SetToDefault(&mut self) {
        self.value = self.default_value;
    }

    pub fn IsSetToDefault(&self) -> bool {
        self.value == self.default_value
    }

    pub fn HaveAlpha(&self) -> bool {
        self.have_alpha
    }

    /// Read a color from an emRec struct field.
    ///
    /// Expects a struct with fields `r`, `g`, `b`, and optionally `a`,
    /// each an integer 0..255.
    pub fn FromRecStruct(rec: &RecStruct, have_alpha: bool) -> Result<emColor, RecError> {
        let r = rec
            .get_int("r")
            .ok_or_else(|| RecError::MissingField("r".into()))? as u8;
        let g = rec
            .get_int("g")
            .ok_or_else(|| RecError::MissingField("g".into()))? as u8;
        let b = rec
            .get_int("b")
            .ok_or_else(|| RecError::MissingField("b".into()))? as u8;
        let a = if have_alpha {
            rec.get_int("a").unwrap_or(255) as u8
        } else {
            255
        };
        Ok(emColor::rgba(r, g, b, a))
    }

    /// Write a color to a `RecStruct`.
    pub fn ToRecStruct(color: emColor, have_alpha: bool) -> RecStruct {
        let mut s = RecStruct::new();
        s.set_int("r", color.GetRed() as i32);
        s.set_int("g", color.GetGreen() as i32);
        s.set_int("b", color.GetBlue() as i32);
        if have_alpha {
            s.set_int("a", color.GetAlpha() as i32);
        }
        s
    }
}

impl Default for emColorRec {
    fn default() -> Self {
        Self::new(emColor::BLACK, false)
    }
}

// ---- emRecFileReader / emRecFileWriter ----

/// Convenience wrapper for reading an emRec tree from a file.
///
/// Port of C++ `emRecFileReader`. Provides a simpler API than the C++ version
/// since Rust does not need the incremental read/continue/quit protocol.
pub struct emRecFileReader;

impl emRecFileReader {
    /// Read an emRec file and parse it into a `RecStruct`.
    pub fn read(path: &Path) -> Result<RecStruct, RecError> {
        let content = std::fs::read_to_string(path).map_err(RecError::Io)?;
        parse_rec(&content)
    }

    /// Read an emRec file, verifying the format header matches `format_name`.
    pub fn read_with_format(path: &Path, format_name: &str) -> Result<RecStruct, RecError> {
        let content = std::fs::read_to_string(path).map_err(RecError::Io)?;
        parse_rec_with_format(&content, format_name)
    }
}

/// Convenience wrapper for writing an emRec tree to a file.
///
/// Port of C++ `emRecFileWriter`. Provides a simpler API than the C++ version
/// since Rust does not need the incremental write/continue/quit protocol.
pub struct emRecFileWriter;

impl emRecFileWriter {
    /// Write a `RecStruct` to a file (no format header).
    pub fn write(path: &Path, rec: &RecStruct) -> Result<(), RecError> {
        let content = write_rec(rec);
        std::fs::write(path, content).map_err(RecError::Io)
    }

    /// Write a `RecStruct` to a file with a `#%rec:FormatName%#` header.
    pub fn write_with_format(
        path: &Path,
        rec: &RecStruct,
        format_name: &str,
    ) -> Result<(), RecError> {
        let content = write_rec_with_format(rec, format_name);
        std::fs::write(path, content).map_err(RecError::Io)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    #[test]
    fn listener_add_notify_remove() {
        let counter = Rc::new(Cell::new(0u32));
        let mut list = RecListenerList::new();

        let c = counter.clone();
        let id = list.add(move || c.set(c.get() + 1));

        list.notify();
        assert_eq!(counter.get(), 1);

        list.notify();
        assert_eq!(counter.get(), 2);

        list.remove(id);
        list.notify();
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn alignment_rec_default() {
        let rec = emAlignmentRec::default();
        assert_eq!(rec.GetRec(), Alignment::Center);
        assert!(rec.IsSetToDefault());
    }

    #[test]
    fn alignment_rec_set_get() {
        let mut rec = emAlignmentRec::new(Alignment::Start);
        assert_eq!(rec.GetRec(), Alignment::Start);
        rec.Set(Alignment::End);
        assert_eq!(rec.GetRec(), Alignment::End);
        assert!(!rec.IsSetToDefault());
        rec.SetToDefault();
        assert!(rec.IsSetToDefault());
    }

    #[test]
    fn alignment_rec_value_round_trip() {
        for align in [
            Alignment::Start,
            Alignment::Center,
            Alignment::End,
            Alignment::Stretch,
        ] {
            let val = emAlignmentRec::ToRecValue(align);
            let parsed = emAlignmentRec::FromRecValue(&val).unwrap();
            assert_eq!(parsed, align);
        }
    }

    #[test]
    fn color_rec_default() {
        let rec = emColorRec::default();
        assert_eq!(rec.GetRec(), emColor::BLACK);
        assert!(!rec.HaveAlpha());
        assert!(rec.IsSetToDefault());
    }

    #[test]
    fn color_rec_set_get() {
        let mut rec = emColorRec::new(emColor::RED, false);
        assert_eq!(rec.GetRec(), emColor::RED);
        rec.Set(emColor::BLUE);
        assert_eq!(rec.GetRec(), emColor::BLUE);
        assert!(!rec.IsSetToDefault());
        rec.SetToDefault();
        assert!(rec.IsSetToDefault());
    }

    #[test]
    fn color_rec_opaque_forces_alpha_255() {
        let mut rec = emColorRec::new(emColor::BLACK, false);
        rec.Set(emColor::rgba(100, 200, 50, 128));
        assert_eq!(rec.GetRec().GetAlpha(), 255);
    }

    #[test]
    fn color_rec_with_alpha() {
        let mut rec = emColorRec::new(emColor::TRANSPARENT, true);
        rec.Set(emColor::rgba(100, 200, 50, 128));
        assert_eq!(rec.GetRec().GetAlpha(), 128);
    }

    #[test]
    fn color_rec_struct_round_trip() {
        let color = emColor::rgba(10, 20, 30, 255);
        let s = emColorRec::ToRecStruct(color, false);
        let parsed = emColorRec::FromRecStruct(&s, false).unwrap();
        assert_eq!(parsed, color);
    }

    #[test]
    fn color_rec_struct_with_alpha_round_trip() {
        let color = emColor::rgba(10, 20, 30, 128);
        let s = emColorRec::ToRecStruct(color, true);
        let parsed = emColorRec::FromRecStruct(&s, true).unwrap();
        assert_eq!(parsed, color);
    }
}
