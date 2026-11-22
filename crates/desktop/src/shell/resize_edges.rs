//! Resize grabs along the window edges, for the frame the app draws itself (client-side
//! decorations on Linux). Each grab starts a platform resize from its edge.

use gpui::{
    CursorStyle, Decorations, Div, MouseButton, MouseDownEvent, ResizeEdge, Tiling, Window, div,
    prelude::*, px,
};
use massorca_ui::tokens::{SIZE_TITLEBAR_CONTROL_WIDTH, SIZE_TITLEBAR_WINDOWS};

/// How far a side grab reaches in from its edge, in pixels.
const EDGE: f32 = 4.0;
/// The side of a corner grab, in pixels.
const CORNER: f32 = 8.0;
/// The strip at the top right that the three window controls take; the top grab stops short of it.
const CONTROLS: f32 = 3.0 * SIZE_TITLEBAR_CONTROL_WIDTH;

/// One grab: the edge it resizes from and its rectangle inside the window.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Grab {
    edge: ResizeEdge,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// The grab layer for a window the app frames, or `None` when the platform draws the frame.
pub(super) fn resize_grabs(window: &Window) -> Option<Div> {
    let Decorations::Client { tiling } = window.window_decorations() else {
        return None;
    };
    let size = window.viewport_size();
    let grabs = grabs(f32::from(size.width), f32::from(size.height), tiling);
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .children(grabs.into_iter().map(grab_element)),
    )
}

/// The grabs for a window of `width` by `height` pixels, without the edges the platform tiles.
fn grabs(width: f32, height: f32, tiling: Tiling) -> Vec<Grab> {
    let bar = SIZE_TITLEBAR_WINDOWS;
    let (top, bottom) = (!tiling.top, !tiling.bottom);
    let (left, right) = (!tiling.left, !tiling.right);
    let mut grabs = Vec::new();
    let mut add = |enabled: bool, edge, (x, y): (f32, f32), (w, h): (f32, f32)| {
        if enabled {
            grabs.push(Grab {
                edge,
                x,
                y,
                width: w.max(0.0),
                height: h.max(0.0),
            });
        }
    };
    add(
        top && left,
        ResizeEdge::TopLeft,
        (0.0, 0.0),
        (CORNER, CORNER),
    );
    add(
        top,
        ResizeEdge::Top,
        (CORNER, 0.0),
        (width - CONTROLS - CORNER, EDGE),
    );
    add(
        bottom && left,
        ResizeEdge::BottomLeft,
        (0.0, height - CORNER),
        (CORNER, CORNER),
    );
    add(
        bottom,
        ResizeEdge::Bottom,
        (CORNER, height - EDGE),
        (width - 2.0 * CORNER, EDGE),
    );
    add(
        bottom && right,
        ResizeEdge::BottomRight,
        (width - CORNER, height - CORNER),
        (CORNER, CORNER),
    );
    // The right grab starts below the title bar so the window controls keep their clicks.
    add(
        right,
        ResizeEdge::Right,
        (width - EDGE, bar),
        (EDGE, height - bar - CORNER),
    );
    add(
        left,
        ResizeEdge::Left,
        (0.0, CORNER),
        (EDGE, height - 2.0 * CORNER),
    );
    grabs
}

fn grab_element(grab: Grab) -> Div {
    div()
        .absolute()
        .left(px(grab.x))
        .top(px(grab.y))
        .w(px(grab.width))
        .h(px(grab.height))
        .cursor(cursor(grab.edge))
        .on_mouse_down(
            MouseButton::Left,
            move |_: &MouseDownEvent, window: &mut Window, _| window.start_window_resize(grab.edge),
        )
}

fn cursor(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f32 = 1440.0;
    const HEIGHT: f32 = 900.0;

    fn edges(tiling: Tiling) -> Vec<ResizeEdge> {
        grabs(WIDTH, HEIGHT, tiling)
            .into_iter()
            .map(|grab| grab.edge)
            .collect()
    }

    fn floating_grab(edge: ResizeEdge) -> Grab {
        grabs(WIDTH, HEIGHT, Tiling::default())
            .into_iter()
            .find(|grab| grab.edge == edge)
            .expect("a floating window has a grab on this edge")
    }

    #[test]
    fn a_floating_window_grabs_every_edge_but_the_top_right_corner() {
        let edges = edges(Tiling::default());
        assert_eq!(edges.len(), 7);
        assert!(!edges.contains(&ResizeEdge::TopRight));
    }

    #[test]
    fn a_fully_tiled_window_grabs_nothing() {
        assert!(edges(Tiling::tiled()).is_empty());
    }

    #[test]
    fn a_tiled_side_leaves_that_edge_and_its_corners() {
        let left_tiled = Tiling {
            left: true,
            ..Tiling::default()
        };
        let edges = edges(left_tiled);
        assert!(!edges.contains(&ResizeEdge::Left));
        assert!(!edges.contains(&ResizeEdge::TopLeft));
        assert!(!edges.contains(&ResizeEdge::BottomLeft));
        assert!(edges.contains(&ResizeEdge::Right));
    }

    #[test]
    fn the_top_grab_stops_before_the_window_controls() {
        let top = floating_grab(ResizeEdge::Top);
        assert!(top.x + top.width <= WIDTH - CONTROLS);
    }

    #[test]
    fn the_right_grab_starts_below_the_title_bar() {
        assert!(floating_grab(ResizeEdge::Right).y >= SIZE_TITLEBAR_WINDOWS);
    }
}
