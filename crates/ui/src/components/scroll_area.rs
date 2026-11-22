//! ScrollArea: the scroll container for lists, trees, transcripts and wide tables
//! (docs/design/components/ScrollArea.md). The viewport scrolls its content under a thin overlay
//! thumb with no track fill. The wheel, the keys, a thumb drag and a click on the track all scroll
//! it. The thumb geometry and the key rules are plain functions below; the element draws them.

use crate::components::switch::ease;
use crate::theme::ActiveTheme;
use crate::tokens::{
    Color, DURATION_FAST, EASE_STANDARD, RADIUS_XS, SIZE_CONTROL_DEFAULT, SIZE_SCROLLBAR_MIN_THUMB,
    SIZE_SCROLLBAR_THICKNESS, SPACE_HALF,
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, Context, DragMoveEvent, ElementId,
    EmptyView, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement, Pixels, Point, RenderOnce, ScrollHandle,
    StatefulInteractiveElement, Styled, Task, Window, div, point, px,
};
use std::fmt;
use std::time::Duration;

/// No colour: the focus ring's rest state, which keeps the 2 px inset border without drawing it.
const NONE: Color = Color::rgba8(0, 0, 0, 0.0);

/// Time after scrolling stops before an overlay thumb hides (design-system 4.8).
pub const SCROLLBAR_HIDE_DELAY_MS: u64 = 1000;
/// Distance one Up, Down, Left or Right press scrolls: the 28 px desktop row.
pub const ROW_STEP_PX: f32 = SIZE_CONTROL_DEFAULT;
/// Gap between a thumb and the edge it runs along (`space.half`).
const THUMB_INSET: f32 = SPACE_HALF;

/// Which directions a scroll area scrolls. Each thumb shows only while its direction overflows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAxis {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

impl ScrollAxis {
    fn supports(self, track: Track) -> bool {
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

/// Forces a visual state so the gallery can show it without the pointer or the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollPreview {
    Thumbs,
    Focus,
}

/// A scroll container with a thin overlay thumb. Give it a fixed size; the content inside scrolls.
#[derive(IntoElement)]
pub struct ScrollArea {
    id: ElementId,
    axis: ScrollAxis,
    children: Vec<AnyElement>,
    handle: Option<ScrollHandle>,
    focus: Option<FocusHandle>,
    focusable: bool,
    preview: Option<ScrollPreview>,
}

impl fmt::Debug for ScrollArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScrollArea")
            .field("id", &self.id)
            .field("axis", &self.axis)
            .field("focusable", &self.focusable)
            .field("children", &self.children.len())
            .finish_non_exhaustive()
    }
}

impl ScrollArea {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            axis: ScrollAxis::default(),
            children: Vec::new(),
            handle: None,
            focus: None,
            focusable: true,
            preview: None,
        }
    }

    /// Keyboard focus lives with the caller; without it the area keeps its own.
    pub fn focus_handle(mut self, focus: FocusHandle) -> Self {
        self.focus = Some(focus);
        self
    }

    pub fn axis(mut self, axis: ScrollAxis) -> Self {
        self.axis = axis;
        self
    }

    /// One content element, in order. Pass a handle to scroll the area from outside.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    pub fn handle(mut self, handle: ScrollHandle) -> Self {
        self.handle = Some(handle);
        self
    }

    /// Whether the viewport takes a Tab stop. Turn it off when the content has focusable
    /// controls: those take Tab themselves, and the arrow keys then reach the area from them.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// Forces a visual state (gallery only).
    pub fn preview(mut self, preview: ScrollPreview) -> Self {
        self.preview = Some(preview);
        self
    }
}

/// A thumb drag in progress: the track, the pointer along it and the scrolled amount at press.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Drag {
    track: Track,
    pointer: f32,
    scrolled: f32,
}

/// What the area keeps between frames: its scroll handle, focus stop, thumb visibility and drag.
struct ScrollAreaState {
    handle: ScrollHandle,
    focus: FocusHandle,
    shown: bool,
    generation: usize,
    hovered: bool,
    hide: Option<Task<()>>,
    drag: Option<Drag>,
}

impl ScrollAreaState {
    /// Hides the thumb once scrolling has stopped, unless the pointer still holds it.
    fn hide_if_idle(&mut self, cx: &mut Context<Self>) {
        if self.hovered || self.drag.is_some() || !self.shown {
            return;
        }
        self.shown = false;
        self.generation += 1;
        cx.notify();
    }
}

/// Shows the thumb and restarts its hide timer. Each call replaces the timer, so the thumb stays
/// for the full delay after the last scroll.
fn reveal(state: &Entity<ScrollAreaState>, cx: &mut App) {
    let weak = state.downgrade();
    let hide = cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(SCROLLBAR_HIDE_DELAY_MS))
            .await;
        weak.update(cx, |state, cx| state.hide_if_idle(cx)).ok();
    });
    state.update(cx, |state, cx| {
        if !state.shown {
            state.shown = true;
            state.generation += 1;
        }
        state.hide = Some(hide);
        cx.notify();
    });
}

/// The payload of a thumb drag; the thumb's own move handler reads which track it belongs to.
struct DraggedThumb;

fn along(track: Track, point: Point<Pixels>) -> f32 {
    match track {
        Track::Vertical => f32::from(point.y),
        Track::Horizontal => f32::from(point.x),
    }
}

fn viewport_len(handle: &ScrollHandle, track: Track) -> f32 {
    let size = handle.bounds().size;
    match track {
        Track::Vertical => f32::from(size.height),
        Track::Horizontal => f32::from(size.width),
    }
}

fn origin_along(handle: &ScrollHandle, track: Track) -> f32 {
    along(track, handle.bounds().origin)
}

fn overflow_len(handle: &ScrollHandle, track: Track) -> f32 {
    let max = handle.max_offset();
    match track {
        Track::Vertical => f32::from(max.y),
        Track::Horizontal => f32::from(max.x),
    }
}

/// The scrolled amount along `track`. Offsets are negative once a list has moved.
fn scrolled_len(handle: &ScrollHandle, track: Track) -> f32 {
    -along(track, handle.offset())
}

fn thumb_on(handle: &ScrollHandle, track: Track) -> Option<Thumb> {
    thumb(
        viewport_len(handle, track),
        overflow_len(handle, track),
        scrolled_len(handle, track),
    )
}

/// Sets the scrolled amount along `track`, keeping the other axis where it is.
fn set_scrolled(handle: &ScrollHandle, track: Track, scrolled: f32) {
    let offset = handle.offset();
    let value = px(-scrolled);
    let next = match track {
        Track::Vertical => point(offset.x, value),
        Track::Horizontal => point(value, offset.y),
    };
    handle.set_offset(next);
}

/// Ends a press: a drag stops and the thumb hides after its delay, unless the pointer is over it.
fn end_press(
    state: Entity<ScrollAreaState>,
) -> impl Fn(&MouseUpEvent, &mut gpui::Window, &mut App) + 'static {
    move |_, _, cx| {
        state.update(cx, |state, _| state.drag = None);
        reveal(&state, cx);
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.theme().palette;
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| ScrollAreaState {
            handle: ScrollHandle::new(),
            focus: cx.focus_handle(),
            shown: false,
            generation: 0,
            hovered: false,
            hide: None,
            drag: None,
        });
        let (own_handle, own_focus, shown, generation, drag) = {
            let state = state.read(cx);
            (
                state.handle.clone(),
                state.focus.clone(),
                state.shown,
                state.generation,
                state.drag,
            )
        };
        let handle = self.handle.clone().unwrap_or(own_handle);
        let focus = self.focus.clone().unwrap_or(own_focus);
        let previewed = self.preview;
        let shown = shown || previewed == Some(ScrollPreview::Thumbs);
        // Thumbs take the pointer only while visible, so a hidden one never covers content.
        let interactive = shown || drag.is_some();
        let axis = self.axis;
        let reduce_motion = cx.reduce_motion();

        let mut viewport = div()
            .id(self.id.clone())
            .size_full()
            .track_scroll(&handle)
            .track_focus(&focus)
            .tab_stop(self.focusable)
            .border_2()
            .border_color(if previewed == Some(ScrollPreview::Focus) {
                p.border.focus
            } else {
                NONE
            })
            .focus_visible(|s| s.border_color(p.border.focus));
        viewport = match axis {
            ScrollAxis::Vertical => viewport.overflow_y_scroll(),
            ScrollAxis::Horizontal => viewport.overflow_x_scroll(),
            ScrollAxis::Both => viewport.overflow_scroll(),
        };
        viewport = viewport
            .on_scroll({
                let state = state.clone();
                move |_, _, cx| reveal(&state, cx)
            })
            .on_key_down({
                let state = state.clone();
                let handle = handle.clone();
                let focus = focus.clone();
                move |event: &KeyDownEvent, window, cx| {
                    if !focus.is_focused(window) {
                        return;
                    }
                    let page = viewport_len(&handle, Track::Vertical);
                    let Some((track, scroll)) = key_scroll(event.keystroke.key.as_str(), page)
                    else {
                        return;
                    };
                    if !axis.supports(track) {
                        return;
                    }
                    cx.stop_propagation();
                    let next = scrolled_after(
                        scrolled_len(&handle, track),
                        overflow_len(&handle, track),
                        scroll,
                    );
                    set_scrolled(&handle, track, next);
                    reveal(&state, cx);
                }
            })
            .children(self.children);

        let mut root = div()
            .id((self.id.clone(), "root"))
            .relative()
            .size_full()
            .on_hover({
                let state = state.clone();
                move |hovered: &bool, _, cx| {
                    state.update(cx, |state, _| state.hovered = *hovered);
                    reveal(&state, cx);
                }
            })
            .on_mouse_up(MouseButton::Left, end_press(state.clone()))
            .on_mouse_up_out(MouseButton::Left, end_press(state.clone()))
            .child(viewport);

        for track in [Track::Vertical, Track::Horizontal] {
            if !axis.supports(track) {
                continue;
            }
            let Some(geometry) = thumb_on(&handle, track) else {
                continue;
            };
            let colour = p.border.strong;
            let (strip, bar) = match track {
                Track::Vertical => (
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_0()
                        .w(px(SIZE_SCROLLBAR_THICKNESS + 2.0 * THUMB_INSET)),
                    div()
                        .absolute()
                        .top(px(geometry.start))
                        .right(px(THUMB_INSET))
                        .w(px(SIZE_SCROLLBAR_THICKNESS))
                        .h(px(geometry.length)),
                ),
                Track::Horizontal => (
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(SIZE_SCROLLBAR_THICKNESS + 2.0 * THUMB_INSET)),
                    div()
                        .absolute()
                        .left(px(geometry.start))
                        .bottom(px(THUMB_INSET))
                        .h(px(SIZE_SCROLLBAR_THICKNESS))
                        .w(px(geometry.length)),
                ),
            };
            let (thumb_name, track_name) = match track {
                Track::Vertical => ("thumb-v", "track-v"),
                Track::Horizontal => ("thumb-h", "track-h"),
            };
            let mut bar = bar
                .id((self.id.clone(), thumb_name))
                .debug_selector(move || thumb_name.into())
                .rounded(px(RADIUS_XS))
                .bg(colour)
                .opacity(if shown { 1.0 } else { 0.0 });
            if interactive {
                bar = bar
                    .on_mouse_down(MouseButton::Left, {
                        let state = state.clone();
                        let handle = handle.clone();
                        move |event: &MouseDownEvent, _, cx| {
                            // No stop_propagation here: GPUI starts the drag from this element's
                            // own press. The track ignores a press inside the thumb.
                            let press = Drag {
                                track,
                                pointer: along(track, event.position),
                                scrolled: scrolled_len(&handle, track),
                            };
                            state.update(cx, |state, cx| {
                                state.drag = Some(press);
                                cx.notify();
                            });
                        }
                    })
                    .on_drag(DraggedThumb, |_, _, _, cx| cx.new(|_| EmptyView))
                    .on_drag_move::<DraggedThumb>({
                        let state = state.clone();
                        let handle = handle.clone();
                        move |event: &DragMoveEvent<DraggedThumb>, _, cx| {
                            let Some(drag) = state.read(cx).drag else {
                                return;
                            };
                            if drag.track != track {
                                return;
                            }
                            let delta = along(track, event.event.position) - drag.pointer;
                            let next = dragged_scroll(
                                viewport_len(&handle, track),
                                overflow_len(&handle, track),
                                drag.scrolled,
                                delta,
                            );
                            set_scrolled(&handle, track, next);
                            state.update(cx, |_, cx| cx.notify());
                        }
                    });
            }
            // A change of visibility bumps `generation`, which restarts the fade; the first frame
            // draws the thumb at its resting opacity.
            let bar = if generation > 0 && !reduce_motion {
                let fading_in = shown;
                let fade = format!(
                    "thumb-fade-{}-{generation}",
                    if track == Track::Vertical { "v" } else { "h" }
                );
                bar.with_animation(
                    (self.id.clone(), fade),
                    Animation::new(DURATION_FAST),
                    move |el, delta| {
                        let t = ease(EASE_STANDARD, delta);
                        el.opacity(if fading_in { t } else { 1.0 - t })
                    },
                )
                .into_any_element()
            } else {
                bar.into_any_element()
            };
            let mut strip = strip;
            if interactive {
                strip = strip.on_mouse_down(MouseButton::Left, {
                    let state = state.clone();
                    let handle = handle.clone();
                    move |event: &MouseDownEvent, _, cx| {
                        let pointer = along(track, event.position) - origin_along(&handle, track);
                        let Some(geometry) = thumb_on(&handle, track) else {
                            return;
                        };
                        let page = viewport_len(&handle, track);
                        let step = if pointer < geometry.start {
                            -page
                        } else if pointer > geometry.start + geometry.length {
                            page
                        } else {
                            return;
                        };
                        let next = scrolled_after(
                            scrolled_len(&handle, track),
                            overflow_len(&handle, track),
                            KeyScroll::By(step),
                        );
                        set_scrolled(&handle, track, next);
                        reveal(&state, cx);
                    }
                });
            }
            root = root.child(strip.debug_selector(move || track_name.into()).child(bar));
        }
        root
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

#[cfg(test)]
mod element_tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use gpui::{
        Context, Entity, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext, div,
        point, px,
    };

    /// The viewport's height and the 50 rows that overflow it.
    const VIEWPORT: f32 = 200.0;
    const ROWS: usize = 50;
    const OVERFLOW: f32 = ROWS as f32 * ROW_STEP_PX - VIEWPORT;

    /// Hosts one fixed-size scroll area over 50 rows, and a second control the tests can focus.
    struct Harness {
        area: FocusHandle,
        other: FocusHandle,
        handle: ScrollHandle,
        preview: Option<ScrollPreview>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = div().flex().flex_col().children(
                (0..ROWS).map(|row| div().h(px(ROW_STEP_PX)).child(format!("Row {row}"))),
            );
            let mut area = ScrollArea::new("scroll-under-test")
                .handle(self.handle.clone())
                .focus_handle(self.area.clone())
                .child(rows);
            if let Some(preview) = self.preview {
                area = area.preview(preview);
            }
            div()
                .flex()
                .child(div().w(px(240.)).h(px(VIEWPORT)).child(area))
                .child(
                    div()
                        .id("other-control")
                        .track_focus(&self.other)
                        .tab_stop(true)
                        .size(px(10.)),
                )
        }
    }

    /// Opens the harness with the area focused, and returns its handle, its focus and the other control's.
    fn open(
        cx: &mut TestAppContext,
        preview: Option<ScrollPreview>,
    ) -> (
        Entity<Harness>,
        &mut VisualTestContext,
        ScrollHandle,
        FocusHandle,
        FocusHandle,
    ) {
        let handle = ScrollHandle::new();
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            area: cx.focus_handle(),
            other: cx.focus_handle(),
            handle: handle.clone(),
            preview,
        });
        cx.run_until_parked();
        let (area, other) = view.read_with(cx, |h, _| (h.area.clone(), h.other.clone()));
        focus_and_draw(cx, &area);
        (view, cx, handle, area, other)
    }

    /// Presses a key, then draws, so the next press sees the bounds of the new frame.
    fn press(cx: &mut VisualTestContext, key: &str) {
        press_key(cx, key);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    fn scrolled(handle: &ScrollHandle) -> f32 {
        -f32::from(handle.offset().y)
    }

    #[gpui::test]
    fn keys_scroll_the_area_only_while_it_has_the_focus(cx: &mut TestAppContext) {
        let (_view, cx, handle, area, other) = open(cx, None);
        focus_and_draw(cx, &other);
        press(cx, "down");
        assert_eq!(scrolled(&handle), 0.0, "another control has the focus");
        focus_and_draw(cx, &area);
        press(cx, "down");
        assert_eq!(scrolled(&handle), ROW_STEP_PX);
    }

    #[gpui::test]
    fn page_keys_move_a_viewport_and_home_and_end_reach_the_ends(cx: &mut TestAppContext) {
        let (_view, cx, handle, _area, _other) = open(cx, None);
        press(cx, "pagedown");
        assert_eq!(scrolled(&handle), VIEWPORT);
        press(cx, "down");
        assert_eq!(scrolled(&handle), VIEWPORT + ROW_STEP_PX);
        press(cx, "end");
        assert_eq!(scrolled(&handle), OVERFLOW);
        press(cx, "down");
        assert_eq!(scrolled(&handle), OVERFLOW, "the end stops the scroll");
        press(cx, "home");
        assert_eq!(scrolled(&handle), 0.0);
        press(cx, "pageup");
        assert_eq!(scrolled(&handle), 0.0, "the top stops it");
    }

    #[gpui::test]
    fn dragging_the_thumb_scrolls_the_content_in_proportion(cx: &mut TestAppContext) {
        let (_view, cx, handle, _area, _other) = open(cx, Some(ScrollPreview::Thumbs));
        let thumb = cx
            .debug_bounds("thumb-v")
            .expect("the thumb is drawn")
            .center();
        let length = VIEWPORT * VIEWPORT / (VIEWPORT + OVERFLOW);
        let half_travel = (VIEWPORT - length) / 2.0;
        // The first move past the drag threshold starts the drag, so a later move carries it.
        cx.simulate_mouse_down(thumb, MouseButton::Left, Modifiers::none());
        for step in [1.0, half_travel / 2.0, half_travel] {
            cx.simulate_mouse_move(
                thumb + point(px(0.), px(step)),
                MouseButton::Left,
                Modifiers::none(),
            );
        }
        cx.simulate_mouse_up(
            thumb + point(px(0.), px(half_travel)),
            MouseButton::Left,
            Modifiers::none(),
        );
        assert!(
            (scrolled(&handle) - OVERFLOW / 2.0).abs() < 1.0,
            "half the travel is half the overflow, got {}",
            scrolled(&handle)
        );
    }

    #[gpui::test]
    fn a_click_on_the_track_below_the_thumb_pages_down(cx: &mut TestAppContext) {
        let (_view, cx, handle, _area, _other) = open(cx, Some(ScrollPreview::Thumbs));
        let track = cx.debug_bounds("track-v").expect("the track is drawn");
        let below = point(track.center().x, track.bottom() - px(5.));
        cx.simulate_click(below, Modifiers::none());
        assert_eq!(scrolled(&handle), VIEWPORT);
    }
}
