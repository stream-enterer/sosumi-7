use std::fmt;

/// Mouse cursor appearance.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum emCursor {
    Normal,
    Invisible,
    Wait,
    Crosshair,
    Text,
    Hand,
    ArrowN,
    ArrowS,
    ArrowE,
    ArrowW,
    ArrowNE,
    ArrowNW,
    ArrowSE,
    ArrowSW,
    ResizeNS,
    ResizeEW,
    ResizeNESW,
    ResizeNWSE,
    Move,
}

impl emCursor {
    // DIVERGED: Get — returns Self (identity for enum); C++ returns int id
    // DIVERGED: ToString — renamed to `as_str`; `ToString` conflicts with Rust std::string::ToString trait

    /// Return this cursor variant. C++ `emCursor::Get()` returns the int id;
    /// Rust returns the enum variant itself.
    pub fn Get(self) -> Self {
        self
    }
    /// Display name for this cursor type.
    pub fn emInputKeyToString(self) -> &'static str {
        match self {
            emCursor::Normal => "Normal",
            emCursor::Invisible => "Invisible",
            emCursor::Wait => "Wait",
            emCursor::Crosshair => "Crosshair",
            emCursor::Text => "Text",
            emCursor::Hand => "Hand",
            emCursor::ArrowN => "ArrowN",
            emCursor::ArrowS => "ArrowS",
            emCursor::ArrowE => "ArrowE",
            emCursor::ArrowW => "ArrowW",
            emCursor::ArrowNE => "ArrowNE",
            emCursor::ArrowNW => "ArrowNW",
            emCursor::ArrowSE => "ArrowSE",
            emCursor::ArrowSW => "ArrowSW",
            emCursor::ResizeNS => "ResizeNS",
            emCursor::ResizeEW => "ResizeEW",
            emCursor::ResizeNESW => "ResizeNESW",
            emCursor::ResizeNWSE => "ResizeNWSE",
            emCursor::Move => "Move",
        }
    }

    /// Map the C++-style cursor enum to a winit 0.30 CursorIcon.
    pub fn to_winit_cursor(self) -> winit::window::CursorIcon {
        use winit::window::CursorIcon;
        match self {
            emCursor::Normal => CursorIcon::Default,
            // winit has no true invisible cursor; Default is the closest
            // fallback (emWindow can call `set_cursor_visible(false)` for a
            // stricter mapping in the future).
            emCursor::Invisible => CursorIcon::Default,
            emCursor::Wait => CursorIcon::Wait,
            emCursor::Crosshair => CursorIcon::Crosshair,
            emCursor::Text => CursorIcon::Text,
            emCursor::Hand => CursorIcon::Pointer,
            emCursor::ArrowN => CursorIcon::NResize,
            emCursor::ArrowS => CursorIcon::SResize,
            emCursor::ArrowE => CursorIcon::EResize,
            emCursor::ArrowW => CursorIcon::WResize,
            emCursor::ArrowNE => CursorIcon::NeResize,
            emCursor::ArrowNW => CursorIcon::NwResize,
            emCursor::ArrowSE => CursorIcon::SeResize,
            emCursor::ArrowSW => CursorIcon::SwResize,
            emCursor::ResizeNS => CursorIcon::NsResize,
            emCursor::ResizeEW => CursorIcon::EwResize,
            emCursor::ResizeNESW => CursorIcon::NeswResize,
            emCursor::ResizeNWSE => CursorIcon::NwseResize,
            emCursor::Move => CursorIcon::Move,
        }
    }
}

impl fmt::Display for emCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.emInputKeyToString())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_returns_self() {
        let c = emCursor::Hand;
        assert_eq!(c.Get(), emCursor::Hand);
    }
}
