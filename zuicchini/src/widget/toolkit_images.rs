use std::cell::OnceCell;

use crate::foundation::{load_tga, Image};

pub(crate) struct ToolkitImages {
    pub group_border: Image,
    pub button_border: Image,
    pub popup_border: Image,
    pub group_inner_border: Image,
    pub io_field: Image,
    pub custom_rect_border: Image,
    pub button: Image,
    pub button_pressed: Image,
    pub splitter: Image,
    pub splitter_pressed: Image,
    pub check_box: Image,
    pub check_box_pressed: Image,
    pub radio_box: Image,
    pub radio_box_pressed: Image,
}

fn decode(data: &[u8], name: &str, expected_w: u32, expected_h: u32) -> Image {
    let img = load_tga(data).unwrap_or_else(|e| panic!("failed to decode {name}: {e}"));
    assert_eq!(
        (img.width(), img.height()),
        (expected_w, expected_h),
        "{name} dimensions mismatch: got {}x{}, expected {expected_w}x{expected_h}",
        img.width(),
        img.height(),
    );
    img
}

impl ToolkitImages {
    fn load() -> Self {
        Self {
            group_border: decode(
                include_bytes!("../../res/toolkit/GroupBorder.tga"),
                "GroupBorder",
                592,
                592,
            ),
            button_border: decode(
                include_bytes!("../../res/toolkit/ButtonBorder.tga"),
                "ButtonBorder",
                704,
                704,
            ),
            popup_border: decode(
                include_bytes!("../../res/toolkit/PopupBorder.tga"),
                "PopupBorder",
                320,
                320,
            ),
            group_inner_border: decode(
                include_bytes!("../../res/toolkit/GroupInnerBorder.tga"),
                "GroupInnerBorder",
                470,
                470,
            ),
            io_field: decode(
                include_bytes!("../../res/toolkit/IOField.tga"),
                "IOField",
                572,
                572,
            ),
            custom_rect_border: decode(
                include_bytes!("../../res/toolkit/CustomRectBorder.tga"),
                "CustomRectBorder",
                450,
                450,
            ),
            button: decode(
                include_bytes!("../../res/toolkit/Button.tga"),
                "Button",
                658,
                658,
            ),
            button_pressed: decode(
                include_bytes!("../../res/toolkit/ButtonPressed.tga"),
                "ButtonPressed",
                648,
                648,
            ),
            splitter: decode(
                include_bytes!("../../res/toolkit/Splitter.tga"),
                "Splitter",
                300,
                300,
            ),
            splitter_pressed: decode(
                include_bytes!("../../res/toolkit/SplitterPressed.tga"),
                "SplitterPressed",
                300,
                300,
            ),
            check_box: decode(
                include_bytes!("../../res/toolkit/CheckBox.tga"),
                "CheckBox",
                380,
                380,
            ),
            check_box_pressed: decode(
                include_bytes!("../../res/toolkit/CheckBoxPressed.tga"),
                "CheckBoxPressed",
                380,
                380,
            ),
            radio_box: decode(
                include_bytes!("../../res/toolkit/RadioBox.tga"),
                "RadioBox",
                380,
                380,
            ),
            radio_box_pressed: decode(
                include_bytes!("../../res/toolkit/RadioBoxPressed.tga"),
                "RadioBoxPressed",
                380,
                380,
            ),
        }
    }
}

thread_local! {
    static TOOLKIT: OnceCell<ToolkitImages> = const { OnceCell::new() };
}

pub(crate) fn with_toolkit_images<R>(f: impl FnOnce(&ToolkitImages) -> R) -> R {
    TOOLKIT.with(|cell| f(cell.get_or_init(ToolkitImages::load)))
}
