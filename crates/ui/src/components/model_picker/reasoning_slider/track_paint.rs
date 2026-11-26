//! Paints the reasoning track from its own bounds: the brand fill, the top effects, the stops and the
//! thumb. Layout rounds every child to whole device pixels, so a thumb laid out as a child can land off
//! the track's axis at 125 or 150 percent; placing each one from the track's bounds keeps them on it.

#[cfg(test)]
use std::cell::RefCell;

use gpui::{
    BorderStyle, Bounds, BoxShadow, Corners, Edges, Hsla, Pixels, Point, Window, fill,
    linear_color_stop, linear_gradient, point, px, quad, size,
};

use super::effects;
use super::stops::{STOP_INSET, stop_fraction};
use crate::tokens::SIZE_REASONING_SLIDER_THUMB;

/// The stop dot's diameter, in px.
const STOP_DOT: f32 = 4.0;

#[cfg(test)]
thread_local! {
    /// The thumb rectangle the last paint drew, which the tests read back.
    static PAINTED_THUMB: RefCell<Option<Bounds<Pixels>>> = const { RefCell::new(None) };
}

/// The thumb rectangle the last paint drew.
#[cfg(test)]
pub(in crate::components::model_picker) fn painted_thumb() -> Option<Bounds<Pixels>> {
    PAINTED_THUMB.with(|cell| cell.borrow().clone())
}

/// Everything the track paints, captured when the slider renders.
pub(super) struct TrackPaint {
    /// Where the thumb and fill are: 0 at the bottom stop, 1 at the top one, eased while it moves.
    pub fraction: f32,
    pub count: usize,
    pub selected: usize,
    pub top: bool,
    pub first: Hsla,
    pub second: Hsla,
    pub thumb: Hsla,
    pub thumb_ring: Hsla,
    pub stop_on: Hsla,
    pub stop_off: Hsla,
    pub thumb_shadows: Vec<BoxShadow>,
    pub elapsed: f32,
    pub reduced: bool,
}

/// The track's frame in logical px, read from its snapped bounds.
#[derive(Clone, Copy)]
struct Frame {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Frame {
    fn of(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        }
    }

    /// The distance from the track's top to the stop at `fraction`.
    fn stop_y(&self, fraction: f32) -> f32 {
        STOP_INSET + (1.0 - fraction) * (self.height - 2.0 * STOP_INSET)
    }

    /// The window point `x` and `y` px into the track.
    fn at(&self, x: f32, y: f32) -> Point<Pixels> {
        point(px(self.x + x), px(self.y + y))
    }
}

/// Paints the track into `bounds`, the rectangle the track itself was laid out in.
pub(super) fn paint_track(bounds: Bounds<Pixels>, paint: &TrackPaint, window: &mut Window) {
    let frame = Frame::of(bounds);
    paint_fill(frame, paint, window);
    if paint.top {
        effects::paint_top(bounds, paint.elapsed, paint.reduced, window);
    }
    paint_stops(frame, paint, window);
    paint_thumb(frame, paint, window);
}

/// The brand fill, from the selected stop plus 14 px down to the bottom of the track.
fn paint_fill(frame: Frame, paint: &TrackPaint, window: &mut Window) {
    let top = frame.stop_y(paint.fraction) - STOP_INSET;
    let height = frame.height - top;
    let gradient = linear_gradient(
        0.,
        linear_color_stop(paint.first, 0.),
        linear_color_stop(paint.second, 1.),
    );
    let at = Bounds::new(frame.at(0.0, top), size(px(frame.width), px(height)));
    window.paint_quad(
        fill(at, gradient).corner_radii(Corners::all(px(frame.width.min(height) / 2.0))),
    );
}

fn paint_stops(frame: Frame, paint: &TrackPaint, window: &mut Window) {
    let left = (frame.width - STOP_DOT) / 2.0;
    for index in 0..paint.count {
        let colour = if index <= paint.selected {
            paint.stop_on
        } else {
            paint.stop_off
        };
        let centre = frame.stop_y(stop_fraction(index, paint.count));
        let at = Bounds::new(
            frame.at(left, centre - STOP_DOT / 2.0),
            size(px(STOP_DOT), px(STOP_DOT)),
        );
        window.paint_quad(fill(at, colour).corner_radii(Corners::all(px(STOP_DOT / 2.0))));
    }
}

/// The thumb, centred on the track's own middle line: its left edge is the track's centre less the radius.
fn paint_thumb(frame: Frame, paint: &TrackPaint, window: &mut Window) {
    let radius = SIZE_REASONING_SLIDER_THUMB / 2.0;
    let centre = frame.stop_y(paint.fraction);
    let at = Bounds::new(
        frame.at(frame.width / 2.0 - radius, centre - radius),
        size(px(2.0 * radius), px(2.0 * radius)),
    );
    window.paint_drop_shadows(at, Corners::all(px(radius)), &paint.thumb_shadows);
    window.paint_quad(quad(
        at,
        Corners::all(px(radius)),
        paint.thumb,
        Edges::all(px(1.0)),
        paint.thumb_ring,
        BorderStyle::default(),
    ));
    #[cfg(test)]
    PAINTED_THUMB.with(|cell| *cell.borrow_mut() = Some(at));
}
