// SPLIT: C++ `include/emCore/emStd1.h:464-485` defines `emAlignment` (typedef emByte)
// alongside many unrelated utilities in emStd1.h/.cpp. Per Modules' "one primary
// type per file" rule, the alignment typedef + constants + string conversions
// live here; other emStd1 items remain in `emStd1.rs`.
//
// C++ references:
//   - Typedef + constants: `include/emCore/emStd1.h:464-478`
//   - Conversion fn decls: `include/emCore/emStd1.h:481-482`
//   - Conversion fn impls: `src/emCore/emStd1.cpp:840-901`

/// C++ `typedef emByte emAlignment;` (`emStd1.h:478`).
pub type emAlignment = u8;

pub const EM_ALIGN_CENTER: emAlignment = 0;
pub const EM_ALIGN_TOP: emAlignment = 1 << 0;
pub const EM_ALIGN_BOTTOM: emAlignment = 1 << 1;
pub const EM_ALIGN_LEFT: emAlignment = 1 << 2;
pub const EM_ALIGN_RIGHT: emAlignment = 1 << 3;
pub const EM_ALIGN_TOP_LEFT: emAlignment = EM_ALIGN_TOP | EM_ALIGN_LEFT;
pub const EM_ALIGN_TOP_RIGHT: emAlignment = EM_ALIGN_TOP | EM_ALIGN_RIGHT;
pub const EM_ALIGN_BOTTOM_LEFT: emAlignment = EM_ALIGN_BOTTOM | EM_ALIGN_LEFT;
pub const EM_ALIGN_BOTTOM_RIGHT: emAlignment = EM_ALIGN_BOTTOM | EM_ALIGN_RIGHT;

/// C++ `emAlignmentToString` (`emStd1.cpp:843-864`). 16-entry table indexed by
/// `alignment & 15`; upper bits of the byte are ignored.
pub fn emAlignmentToString(alignment: emAlignment) -> &'static str {
    const TAB: [&str; 16] = [
        "center",
        "top",
        "bottom",
        "top-bottom",
        "left",
        "top-left",
        "bottom-left",
        "top-bottom-left",
        "right",
        "top-right",
        "bottom-right",
        "top-bottom-right",
        "left-right",
        "top-left-right",
        "bottom-left-right",
        "top-bottom-left-right",
    ];
    TAB[(alignment & 15) as usize]
}

/// C++ `emStringToAlignment` (`emStd1.cpp:867-901`). Walks the string, skipping
/// non-letter bytes; on each letter, matches one of top/bottom/left/right/center
/// case-insensitively and ORs the corresponding bit (center contributes nothing).
/// Unrecognized letter runs terminate parsing.
pub fn emStringToAlignment(s: &str) -> emAlignment {
    let bytes = s.as_bytes();
    let mut a: emAlignment = 0;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let is_letter = c.is_ascii_alphabetic();
        if !is_letter {
            i += 1;
            continue;
        }
        if starts_with_ci(&bytes[i..], b"top") {
            i += 3;
            a |= EM_ALIGN_TOP;
        } else if starts_with_ci(&bytes[i..], b"bottom") {
            i += 6;
            a |= EM_ALIGN_BOTTOM;
        } else if starts_with_ci(&bytes[i..], b"left") {
            i += 4;
            a |= EM_ALIGN_LEFT;
        } else if starts_with_ci(&bytes[i..], b"right") {
            i += 5;
            a |= EM_ALIGN_RIGHT;
        } else if starts_with_ci(&bytes[i..], b"center") {
            i += 6;
        } else {
            break;
        }
    }
    a
}

// C++ uses `strncasecmp`; needle is ASCII so byte-wise eq_ignore_ascii_case is exact.
fn starts_with_ci(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len() && hay[..needle.len()].eq_ignore_ascii_case(needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitwise_or_composition() {
        assert_eq!(EM_ALIGN_TOP | EM_ALIGN_LEFT, EM_ALIGN_TOP_LEFT);
        assert_eq!(EM_ALIGN_TOP | EM_ALIGN_RIGHT, EM_ALIGN_TOP_RIGHT);
        assert_eq!(EM_ALIGN_BOTTOM | EM_ALIGN_LEFT, EM_ALIGN_BOTTOM_LEFT);
        assert_eq!(EM_ALIGN_BOTTOM | EM_ALIGN_RIGHT, EM_ALIGN_BOTTOM_RIGHT);
        assert_eq!(EM_ALIGN_CENTER, 0);
        assert_eq!(EM_ALIGN_TOP, 1);
        assert_eq!(EM_ALIGN_BOTTOM, 2);
        assert_eq!(EM_ALIGN_LEFT, 4);
        assert_eq!(EM_ALIGN_RIGHT, 8);
    }

    #[test]
    fn to_string_all_16_entries() {
        // C++ `emStd1.cpp:845-862` table, in order.
        assert_eq!(emAlignmentToString(0), "center");
        assert_eq!(emAlignmentToString(1), "top");
        assert_eq!(emAlignmentToString(2), "bottom");
        assert_eq!(emAlignmentToString(3), "top-bottom");
        assert_eq!(emAlignmentToString(4), "left");
        assert_eq!(emAlignmentToString(5), "top-left");
        assert_eq!(emAlignmentToString(6), "bottom-left");
        assert_eq!(emAlignmentToString(7), "top-bottom-left");
        assert_eq!(emAlignmentToString(8), "right");
        assert_eq!(emAlignmentToString(9), "top-right");
        assert_eq!(emAlignmentToString(10), "bottom-right");
        assert_eq!(emAlignmentToString(11), "top-bottom-right");
        assert_eq!(emAlignmentToString(12), "left-right");
        assert_eq!(emAlignmentToString(13), "top-left-right");
        assert_eq!(emAlignmentToString(14), "bottom-left-right");
        assert_eq!(emAlignmentToString(15), "top-bottom-left-right");
    }

    #[test]
    fn to_string_masks_upper_bits() {
        // C++ `tab[alignment&15]` — bit 4+ ignored.
        assert_eq!(emAlignmentToString(0xF0), "center");
        assert_eq!(emAlignmentToString(0xFF), "top-bottom-left-right");
    }

    #[test]
    fn round_trip_all_named_values() {
        for &a in &[
            EM_ALIGN_CENTER,
            EM_ALIGN_TOP,
            EM_ALIGN_BOTTOM,
            EM_ALIGN_LEFT,
            EM_ALIGN_RIGHT,
            EM_ALIGN_TOP_LEFT,
            EM_ALIGN_TOP_RIGHT,
            EM_ALIGN_BOTTOM_LEFT,
            EM_ALIGN_BOTTOM_RIGHT,
        ] {
            let s = emAlignmentToString(a);
            let parsed = emStringToAlignment(s);
            assert_eq!(parsed, a, "round-trip failed for {a:#x} via \"{s}\"");
        }
    }

    #[test]
    fn string_to_alignment_case_insensitive() {
        assert_eq!(emStringToAlignment("TOP"), EM_ALIGN_TOP);
        assert_eq!(emStringToAlignment("Top-Left"), EM_ALIGN_TOP_LEFT);
        assert_eq!(emStringToAlignment("BOTTOM-RIGHT"), EM_ALIGN_BOTTOM_RIGHT);
    }

    #[test]
    fn string_to_alignment_empty_and_null_like() {
        assert_eq!(emStringToAlignment(""), EM_ALIGN_CENTER);
    }

    #[test]
    fn string_to_alignment_skips_non_letters() {
        // C++ loop skips any byte outside [A-Za-z].
        assert_eq!(emStringToAlignment("  top, left!"), EM_ALIGN_TOP_LEFT);
    }

    #[test]
    fn string_to_alignment_breaks_on_unknown_word() {
        // Unrecognized letter run terminates parsing; prior bits kept.
        assert_eq!(emStringToAlignment("top foo bottom"), EM_ALIGN_TOP);
    }

    #[test]
    fn string_to_alignment_center_contributes_zero() {
        assert_eq!(emStringToAlignment("center"), EM_ALIGN_CENTER);
        assert_eq!(emStringToAlignment("center-top"), EM_ALIGN_TOP);
    }
}
