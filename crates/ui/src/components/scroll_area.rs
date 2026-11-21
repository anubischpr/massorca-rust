//! ScrollArea: the scroll container for lists, trees, transcripts and wide tables
//! (docs/design/components/ScrollArea.md). This file holds the scroll rules as plain functions: the
//! thumb's geometry, the amount each key scrolls and the clamp at either end. The element that draws
//! the area and its thumb builds on them.

use crate::tokens::{SIZE_CONTROL_DEFAULT, SIZE_SCROLLBAR_MIN_THUMB};

/// Time after scrolling stops before an overlay thumb hides (design-system 4.8).
pub const SCROLLBAR_HIDE_DELAY_MS: u64 = 1000;
/// Distance one Up, Down, Left or Right press scrolls: the 28 px desktop row.
pub const ROW_STEP_PX: f32 = SIZE_CONTROL_DEFAULT;

/// Which directions a scroll area scrolls. Each thumb shows only while its direction overflows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAxis {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

impl ScrollAxis {
    pub fn supports(self, track: Track) -> bool {
        match track {
            Track::Vertical => self != Self::Horizontal,
            Track::Horizontal => self != Self::Vertical,
        }
    }
}

/// The direction a thumb runs along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    Vertical,
    Horizontal,
}

/// Where a thumb sits along its track, in px from the track's start, and how long it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thumb {
    pub start: f32,
    pub length: f32,
}

/// The thumb of a track `viewport` px long over content that overflows it by `overflow` px, with
/// `scrolled` px of the overflow already passed. None when the content fits.
pub fn thumb(viewport: f32, overflow: f32, scrolled: f32) -> Option<Thumb> {
    if viewport <= 0.0 || overflow <= 0.0 {
        return None;
    }
    let content = viewport + overflow;
    let minimum = SIZE_SCROLLBAR_MIN_THUMB.min(viewport);
    let length = (viewport * viewport / content).clamp(minimum, viewport);
    let progress = (scrolled / overflow).clamp(0.0, 1.0);
    Some(Thumb {
        start: (viewport - length) * progress,
        length,
    })
}

/// The scrolled amount after a drag of `delta` px from where the drag began at `from`.
pub fn dragged_scroll(viewport: f32, overflow: f32, from: f32, delta: f32) -> f32 {
    let Some(thumb) = thumb(viewport, overflow, from) else {
        return 0.0;
    };
    let travel = viewport - thumb.length;
    if travel <= 0.0 {
        return from;
    }
    (from + delta * overflow / travel).clamp(0.0, overflow)
}

/// What one key asks: a step in px along its track, or a jump to one end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyScroll {
    By(f32),
    Start,
    End,
}

/// The scroll a key asks for. Up and Down step a row, PageUp and PageDown step a viewport of
/// `viewport` px, Home and End jump to the ends, and Left and Right step a row across.
pub fn key_scroll(key: &str, viewport: f32) -> Option<(Track, KeyScroll)> {
    Some(match key {
        "up" => (Track::Vertical, KeyScroll::By(-ROW_STEP_PX)),
        "down" => (Track::Vertical, KeyScroll::By(ROW_STEP_PX)),
        "pageup" => (Track::Vertical, KeyScroll::By(-viewport)),
        "pagedown" => (Track::Vertical, KeyScroll::By(viewport)),
        "home" => (Track::Vertical, KeyScroll::Start),
        "end" => (Track::Vertical, KeyScroll::End),
        "left" => (Track::Horizontal, KeyScroll::By(-ROW_STEP_PX)),
        "right" => (Track::Horizontal, KeyScroll::By(ROW_STEP_PX)),
        _ => return None,
    })
}

/// The scrolled amount after `scroll`, kept between the top (0) and the end (`overflow`).
pub fn scrolled_after(scrolled: f32, overflow: f32, scroll: KeyScroll) -> f32 {
    match scroll {
        KeyScroll::By(step) => (scrolled + step).clamp(0.0, overflow),
        KeyScroll::Start => 0.0,
        KeyScroll::End => overflow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thumb_is_the_visible_share_of_the_track_and_never_below_the_minimum() {
        let t = thumb(200.0, 1200.0, 0.0).expect("content overflows");
        assert!((t.length - 200.0 * 200.0 / 1400.0).abs() < 1e-4);
        assert_eq!(t.start, 0.0);
        assert_eq!(thumb(200.0, 10_000.0, 0.0).unwrap().length, 24.0);
        assert_eq!(
            thumb(200.0, 0.0, 0.0),
            None,
            "content that fits has no thumb"
        );
    }

    #[test]
    fn the_thumb_follows_the_scrolled_share_of_the_overflow() {
        let end = thumb(200.0, 200.0, 200.0).unwrap();
        assert!(
            (end.start - (200.0 - end.length)).abs() < 1e-4,
            "at the end it sits at the bottom"
        );
        let half = thumb(200.0, 200.0, 100.0).unwrap();
        assert!((half.start - (200.0 - half.length) / 2.0).abs() < 1e-4);
    }

    #[test]
    fn dragging_the_thumb_moves_the_content_in_proportion() {
        let viewport = 200.0;
        let overflow = 1200.0;
        let length = thumb(viewport, overflow, 0.0).unwrap().length;
        let travel = viewport - length;
        let halfway = dragged_scroll(viewport, overflow, 0.0, travel / 2.0);
        assert!((halfway - 600.0).abs() < 1e-3);
        assert_eq!(
            dragged_scroll(viewport, overflow, 0.0, travel * 3.0),
            1200.0,
            "clamps at the end"
        );
        assert_eq!(
            dragged_scroll(viewport, overflow, 600.0, -10_000.0),
            0.0,
            "clamps at the top"
        );
    }

    #[test]
    fn keys_map_to_rows_pages_and_ends_on_their_track() {
        assert_eq!(
            key_scroll("down", 200.0),
            Some((Track::Vertical, KeyScroll::By(ROW_STEP_PX)))
        );
        assert_eq!(
            key_scroll("pagedown", 200.0),
            Some((Track::Vertical, KeyScroll::By(200.0)))
        );
        assert_eq!(
            key_scroll("end", 200.0),
            Some((Track::Vertical, KeyScroll::End))
        );
        assert_eq!(
            key_scroll("right", 200.0),
            Some((Track::Horizontal, KeyScroll::By(ROW_STEP_PX)))
        );
        assert_eq!(key_scroll("enter", 200.0), None);
    }

    #[test]
    fn scrolling_stops_at_the_top_and_the_end() {
        assert_eq!(scrolled_after(0.0, 100.0, KeyScroll::By(-28.0)), 0.0);
        assert_eq!(scrolled_after(90.0, 100.0, KeyScroll::By(28.0)), 100.0);
        assert_eq!(scrolled_after(50.0, 100.0, KeyScroll::Start), 0.0);
        assert_eq!(scrolled_after(50.0, 100.0, KeyScroll::End), 100.0);
    }

    #[test]
    fn only_the_axes_the_area_scrolls_have_thumbs() {
        assert!(ScrollAxis::Vertical.supports(Track::Vertical));
        assert!(!ScrollAxis::Vertical.supports(Track::Horizontal));
        assert!(ScrollAxis::Both.supports(Track::Horizontal));
    }
}
