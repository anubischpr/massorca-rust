//! Sheet: the Android bottom sheet (docs/design/components/Sheet.md). This file holds the sheet's
//! rules as plain functions: how tall it may open, how tall its body may be, how far a handle drag
//! has travelled and whether a release closes the sheet. The element builds on them.

use crate::tokens::{
    LAYOUT_SHEET_MAX_HEIGHT, Measure, SIZE_APPBAR_ANDROID, SIZE_HIT_TOUCH,
    SIZE_SHEET_HANDLE_HEIGHT, SPACE_1, SPACE_2, SPACE_4,
};

/// Share of the sheet's height a handle drag must pass to close it on release. The spec leaves
/// the threshold open; a third is this component's choice.
pub const DISMISS_DRAG_SHARE: f32 = 1.0 / 3.0;
/// The drag handle's row: `space.2` above and below the 4 dp handle.
const HANDLE_ROW: f32 = SPACE_2 + SIZE_SHEET_HANDLE_HEIGHT + SPACE_2;

/// The tallest a sheet may open: `layout.sheet.max-height` of the window's height.
pub fn max_height(window_height: f32) -> f32 {
    match LAYOUT_SHEET_MAX_HEIGHT {
        Measure::Percent(percent) => window_height * percent / 100.0,
        Measure::Px(length) | Measure::Ch(length) => length.min(window_height),
    }
}

/// The tallest the body may be, so that the handle, header, footer and body fit the sheet.
pub fn body_max_height(window_height: f32, has_header: bool, has_footer: bool) -> f32 {
    let header = if has_header { SIZE_APPBAR_ANDROID } else { 0.0 };
    let footer = if has_footer {
        2.0 * SPACE_4 + SIZE_HIT_TOUCH
    } else {
        0.0
    };
    (max_height(window_height) - HANDLE_ROW - header - footer).max(0.0)
}

/// How far a handle drag has pulled the sheet down. The sheet never lifts above its place.
pub fn drag_travel(press_y: f32, pointer_y: f32) -> f32 {
    (pointer_y - press_y).max(0.0)
}

/// Whether releasing a handle drag closes the sheet: the travel must pass the dismiss share.
pub fn dismisses_on_release(travel: f32, sheet_height: f32) -> bool {
    sheet_height > 0.0 && travel >= sheet_height * DISMISS_DRAG_SHARE
}

/// What the sheet holds. Rows run edge to edge in the action and option variants; a content view
/// keeps `space.4` around its body.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SheetVariant {
    /// A list of actions with icons; a destructive one comes last.
    #[default]
    Action,
    /// Single-choice rows; the owner draws the check mark on the chosen row.
    Option,
    /// A view with a header and a body.
    Content,
}

impl SheetVariant {
    /// The body's (horizontal, vertical) padding.
    pub fn body_padding(self) -> (f32, f32) {
        match self {
            Self::Action | Self::Option => (0.0, SPACE_1 + SPACE_1),
            Self::Content => (SPACE_4, SPACE_4),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_opens_at_most_ninety_percent_of_the_window() {
        assert_eq!(max_height(900.0), 810.0);
    }

    #[test]
    fn the_body_leaves_room_for_the_handle_header_and_footer() {
        assert_eq!(body_max_height(900.0, false, false), 810.0 - HANDLE_ROW);
        assert_eq!(
            body_max_height(900.0, true, true),
            810.0 - HANDLE_ROW - SIZE_APPBAR_ANDROID - (2.0 * SPACE_4 + SIZE_HIT_TOUCH)
        );
        assert_eq!(body_max_height(0.0, true, true), 0.0);
    }

    #[test]
    fn a_handle_drag_never_lifts_the_sheet_above_its_place() {
        assert_eq!(drag_travel(100.0, 80.0), 0.0);
        assert_eq!(drag_travel(100.0, 150.0), 50.0);
    }

    #[test]
    fn a_release_closes_only_past_a_third_of_the_sheet() {
        assert!(!dismisses_on_release(99.0, 300.0));
        assert!(dismisses_on_release(100.0, 300.0));
        assert!(!dismisses_on_release(500.0, 0.0), "no height, no decision");
    }

    #[test]
    fn only_a_content_view_keeps_the_space_around_its_body() {
        assert_eq!(SheetVariant::Action.body_padding().0, 0.0);
        assert_eq!(SheetVariant::Option.body_padding().0, 0.0);
        assert_eq!(SheetVariant::Content.body_padding(), (SPACE_4, SPACE_4));
        assert_eq!(SheetVariant::default(), SheetVariant::Action);
    }
}
