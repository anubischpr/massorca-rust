//! Sheet: the Android bottom sheet (docs/design/components/Sheet.md). It holds an action list, an
//! option list or a content view above a scrim, with a drag handle, an optional header and footer.
//! Escape, a scrim press and the close button dismiss it unless it is busy. A drag of the handle
//! closes it when released past a third of its height and snaps it back otherwise. Focus moves into
//! the sheet on open, stays inside while it is open and returns to the opener on close. Desktop uses
//! Popover and Menu instead, so the gallery shows the sheet in place.

use crate::components::popover::{DismissHandler, OpenEdge, OpenTracker, TriggerBounds};
use crate::components::skeleton::{Skeleton, SkeletonShape};
use crate::components::switch::ease;
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    DURATION_SLOW, EASE_ENTER, EASE_EXIT, LAYOUT_SHEET_MAX_HEIGHT, Measure, RADIUS_2XL,
    RADIUS_FULL, SIZE_APPBAR_ANDROID, SIZE_HIT_TOUCH, SIZE_SHEET_HANDLE_HEIGHT,
    SIZE_SHEET_HANDLE_WIDTH, SPACE_1, SPACE_2, SPACE_4, TYPE_A_BODY, TYPE_A_TITLE, Z_DIALOG,
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, DragMoveEvent, ElementId, EmptyView,
    Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Task, Window, deferred, div, px,
};
use std::fmt;
use std::rc::Rc;

/// Share of the sheet's height a handle drag must pass to close it on release. The spec leaves
/// the threshold open; a third is this component's choice.
pub const DISMISS_DRAG_SHARE: f32 = 1.0 / 3.0;
/// The drag handle's row: `space.2` above and below the 4 dp handle.
const HANDLE_ROW: f32 = SPACE_2 + SIZE_SHEET_HANDLE_HEIGHT + SPACE_2;
/// Rows of placeholder shown while a sheet loads.
const LOADING_ROWS: usize = 3;

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
    fn body_padding(self) -> (f32, f32) {
        match self {
            Self::Action | Self::Option => (0.0, SPACE_1 + SPACE_1),
            Self::Content => (SPACE_4, SPACE_4),
        }
    }
}

/// The payload of a handle drag; only handle drags reach the panel's move handler.
struct SheetDrag;

/// What the sheet keeps between frames: its open edge, focus, the opener, the animation
/// generation, a handle drag in progress and the timer that ends the closing animation.
struct SheetState {
    edges: OpenTracker,
    panel: FocusHandle,
    opener: Option<FocusHandle>,
    entered: usize,
    exiting: bool,
    exit_timer: Option<Task<()>>,
    press: Option<f32>,
    travel: f32,
    panel_bounds: TriggerBounds,
}

/// Ends the closing animation after its duration, so the sheet leaves the tree.
fn schedule_exit_end(state: &Entity<SheetState>, cx: &mut App) {
    let weak = state.downgrade();
    let timer = cx.spawn(async move |cx| {
        cx.background_executor().timer(DURATION_SLOW).await;
        weak.update(cx, |state, cx| {
            state.exiting = false;
            cx.notify();
        })
        .ok();
    });
    state.update(cx, |state, _| state.exit_timer = Some(timer));
}

/// The bottom sheet. The owner keeps `open`, handles `on_dismiss` and closes the sheet by setting
/// `open` to false, which returns focus to the opener.
#[derive(IntoElement)]
pub struct Sheet {
    id: ElementId,
    open: bool,
    variant: SheetVariant,
    title: Option<SharedString>,
    body: Vec<AnyElement>,
    footer: Option<AnyElement>,
    loading: bool,
    empty: Option<SharedString>,
    busy: bool,
    initial_focus: Option<FocusHandle>,
    inline: bool,
    on_dismiss: Option<DismissHandler>,
}

impl fmt::Debug for Sheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sheet")
            .field("id", &self.id)
            .field("open", &self.open)
            .field("variant", &self.variant)
            .field("title", &self.title)
            .field("busy", &self.busy)
            .finish_non_exhaustive()
    }
}

impl Sheet {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            open: false,
            variant: SheetVariant::default(),
            title: None,
            body: Vec::new(),
            footer: None,
            loading: false,
            empty: None,
            busy: false,
            initial_focus: None,
            inline: false,
            on_dismiss: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn variant(mut self, variant: SheetVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Shows the header with this title, which also names the sheet for assistive technology.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// One body element, in order. The body scrolls past the sheet's maximum height.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    /// Pinned below the body: the primary action, full width.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// Shows placeholder rows in place of the body while its content loads.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Shows one line with the reason the body is empty, in place of the body.
    pub fn empty(mut self, text: impl Into<SharedString>) -> Self {
        self.empty = Some(text.into());
        self
    }

    /// While busy, Escape, the scrim, the close button and a handle drag do not dismiss.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// The first control of the sheet, focused on open. Without one the panel takes focus.
    pub fn initial_focus(mut self, focus: FocusHandle) -> Self {
        self.initial_focus = Some(focus);
        self
    }

    /// Draws the panel in place, without the scrim, the overlay layer or the motion, for galleries.
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }

    pub fn on_dismiss(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for Sheet {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let reduce_motion = cx.reduce_motion();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| SheetState {
            edges: OpenTracker::default(),
            panel: cx.focus_handle(),
            opener: None,
            entered: 0,
            exiting: false,
            exit_timer: None,
            press: None,
            travel: 0.0,
            panel_bounds: TriggerBounds::default(),
        });
        let panel_focus = state.read(cx).panel.clone();
        let panel_bounds = state.read(cx).panel_bounds.clone();
        match state.update(cx, |state, _| state.edges.observe(self.open)) {
            Some(OpenEdge::Opened) => {
                let opener = window.focused(cx);
                let first = self
                    .initial_focus
                    .clone()
                    .unwrap_or_else(|| panel_focus.clone());
                state.update(cx, |state, _| {
                    state.opener = opener;
                    state.entered += 1;
                    state.exiting = false;
                    state.exit_timer = None;
                    state.press = None;
                    state.travel = 0.0;
                });
                window.focus(&first, cx);
            }
            Some(OpenEdge::Closed) => {
                let opener = state.read(cx).opener.clone();
                state.update(cx, |state, _| {
                    state.exiting = !reduce_motion;
                    state.press = None;
                    state.travel = 0.0;
                });
                if !reduce_motion {
                    schedule_exit_end(&state, cx);
                }
                if let Some(opener) = opener {
                    window.focus(&opener, cx);
                }
            }
            None => {}
        }
        let (exiting, entered, travel) = {
            let s = state.read(cx);
            (s.exiting, s.entered, s.travel)
        };
        if !self.open && !exiting {
            return div().into_any_element();
        }

        let viewport = f32::from(window.viewport_size().height);
        let busy = self.busy;
        let dismiss: DismissHandler = {
            let on_dismiss = self.on_dismiss.clone();
            Rc::new(move |window, cx| {
                if !busy && let Some(on_dismiss) = &on_dismiss {
                    on_dismiss(window, cx);
                }
            })
        };
        let has_header = self.title.is_some();
        let has_footer = self.footer.is_some();
        let (side, vertical) = self.variant.body_padding();

        let mut panel = div()
            .id((self.id.clone(), "panel"))
            .w_full()
            .max_h(px(max_height(viewport)))
            .flex()
            .flex_col()
            .bg(p.surface.overlay)
            .border_1()
            .border_color(p.border.default)
            .rounded_t(px(RADIUS_2XL))
            .shadow(box_shadows(theme.shadows.e3))
            .occlude()
            .track_focus(&panel_focus)
            .role(Role::Dialog)
            .aria_label(self.title.clone().unwrap_or_else(|| "Sheet".into()))
            .debug_selector(|| "sheet-panel".into())
            .on_key_down({
                let dismiss = dismiss.clone();
                let panel = panel_focus.clone();
                let first_stop = self
                    .initial_focus
                    .clone()
                    .unwrap_or_else(|| panel_focus.clone());
                move |event: &KeyDownEvent, window, cx| match event.keystroke.key.as_str() {
                    "escape" => {
                        cx.stop_propagation();
                        dismiss(window, cx);
                    }
                    "tab" => {
                        cx.stop_propagation();
                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                        if !panel.contains_focused(window, cx) {
                            window.focus(&first_stop, cx);
                        }
                    }
                    _ => {}
                }
            });
        let release = {
            let state = state.clone();
            let dismiss = dismiss.clone();
            let panel_bounds = panel_bounds.clone();
            move |_: &MouseUpEvent, window: &mut Window, cx: &mut App| {
                let (pressed, travel) = {
                    let s = state.read(cx);
                    (s.press.is_some(), s.travel)
                };
                let height = panel_bounds.get().map_or(0.0, |b| f32::from(b.size.height));
                if !pressed {
                    return;
                }
                state.update(cx, |state, cx| {
                    state.press = None;
                    state.travel = 0.0;
                    cx.notify();
                });
                if dismisses_on_release(travel, height) {
                    dismiss(window, cx);
                }
            }
        };
        panel = panel
            .on_mouse_up(MouseButton::Left, release.clone())
            .on_mouse_up_out(MouseButton::Left, release);

        let grab = div()
            .id((self.id.clone(), "grab"))
            .w_full()
            .flex()
            .justify_center()
            .py(px(SPACE_2))
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                move |event: &MouseDownEvent, _, cx| {
                    state.update(cx, |state, cx| {
                        state.press = Some(f32::from(event.position.y));
                        state.travel = 0.0;
                        cx.notify();
                    });
                }
            })
            .on_drag(SheetDrag, |_, _, _, cx| cx.new(|_| EmptyView))
            .on_drag_move::<SheetDrag>({
                let state = state.clone();
                move |event: &DragMoveEvent<SheetDrag>, _, cx| {
                    let pointer = f32::from(event.event.position.y);
                    state.update(cx, |state, cx| {
                        let Some(press) = state.press else {
                            return;
                        };
                        let travel = drag_travel(press, pointer);
                        if travel != state.travel {
                            state.travel = travel;
                            cx.notify();
                        }
                    });
                }
            })
            .child(
                div()
                    .w(px(SIZE_SHEET_HANDLE_WIDTH))
                    .h(px(SIZE_SHEET_HANDLE_HEIGHT))
                    .rounded(px(RADIUS_FULL))
                    .bg(p.border.strong)
                    .debug_selector(|| "sheet-handle".into()),
            );
        panel = panel.child(grab);

        if let Some(title) = self.title.clone() {
            let close = dismiss.clone();
            panel = panel.child(
                div()
                    .h(px(SIZE_APPBAR_ANDROID))
                    .flex()
                    .items_center()
                    .justify_between()
                    .pl(px(SPACE_4))
                    .pr(px(SPACE_1))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_style(TYPE_A_TITLE)
                            .text_color(p.text.primary)
                            .child(title),
                    )
                    .child(
                        div()
                            .id((self.id.clone(), "close"))
                            .role(Role::Button)
                            .aria_label("Close")
                            .w(px(SIZE_HIT_TOUCH))
                            .h(px(SIZE_HIT_TOUCH))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(RADIUS_FULL))
                            .hover(move |s| s.bg(p.fill.hover_overlay))
                            .on_click(move |_, window, cx| close(window, cx))
                            .child(
                                Icon::new(IconName::X)
                                    .size(IconSize::Medium)
                                    .color(p.text.secondary),
                            ),
                    ),
            );
        }

        let mut column = div()
            .flex()
            .flex_col()
            .px(px(side))
            .py(px(vertical))
            .text_style(TYPE_A_BODY)
            .text_color(p.text.secondary);
        if self.loading {
            for index in 0..LOADING_ROWS {
                column = column.child(
                    Skeleton::new(SkeletonShape::Row)
                        .delayed((self.id.clone(), format!("row-{index}"))),
                );
            }
        } else if let Some(empty) = self.empty.clone() {
            column = column.child(div().px(px(SPACE_4)).py(px(SPACE_4)).child(empty));
        } else {
            column = column.children(self.body);
        }
        let body_max = body_max_height(viewport, has_header, has_footer);
        panel = panel.child(
            div()
                .id((self.id.clone(), "body"))
                .flex_1()
                .min_h_0()
                .max_h(px(body_max))
                .overflow_y_scroll()
                .child(column),
        );

        if let Some(footer) = self.footer {
            panel = panel.child(div().w_full().p(px(SPACE_4)).child(footer));
        }

        if self.inline {
            return panel.into_any_element();
        }

        // The panel rises from the bottom edge on open and sinks back on close; a handle drag
        // moves it by the travel it has made.
        let panel = if reduce_motion {
            panel.top(px(travel)).into_any_element()
        } else if exiting && !self.open {
            panel
                .with_animation(
                    format!("sheet-exit-{entered}"),
                    Animation::new(DURATION_SLOW),
                    move |el, delta| el.top(px(viewport * ease(EASE_EXIT, delta))),
                )
                .into_any_element()
        } else if entered > 0 && self.open && travel == 0.0 {
            panel
                .with_animation(
                    format!("sheet-enter-{entered}"),
                    Animation::new(DURATION_SLOW),
                    move |el, delta| el.top(px(viewport * (1.0 - ease(EASE_ENTER, delta)))),
                )
                .into_any_element()
        } else {
            panel.top(px(travel)).into_any_element()
        };

        let host = panel_bounds.track(div().w_full().child(panel));
        let scrim = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(p.scrim)
            .occlude()
            .on_mouse_down(MouseButton::Left, {
                let dismiss = dismiss.clone();
                move |_, window, cx| dismiss(window, cx)
            });
        let layer = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .justify_end()
            .child(scrim)
            .child(host);
        deferred(layer)
            .priority(Z_DIALOG as usize)
            .into_any_element()
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

#[cfg(test)]
mod element_tests {
    use super::*;
    use crate::components::button::Button;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use gpui::{
        Context, Entity, Modifiers, MouseButton, Pixels, Point, Render, TestAppContext,
        VisualTestContext, point, px,
    };
    use std::cell::Cell;

    /// Hosts an opener and one sheet whose open state the test drives. The sheet holds a spacer
    /// and two buttons; its first button takes the initial focus.
    struct Harness {
        opener: FocusHandle,
        first: FocusHandle,
        second: FocusHandle,
        open: bool,
        busy: bool,
        dismissals: Rc<Cell<u32>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let dismissals = self.dismissals.clone();
            div()
                .child(Button::new("sheet-opener", "Open").focus_handle(self.opener.clone()))
                .child(
                    Sheet::new("sheet-under-test")
                        .title("Workspace actions")
                        .open(self.open)
                        .busy(self.busy)
                        .initial_focus(self.first.clone())
                        .on_dismiss(move |_, _| dismissals.set(dismissals.get() + 1))
                        .child(div().h(px(280.)))
                        .child(
                            Button::new("sheet-first", "Rename").focus_handle(self.first.clone()),
                        )
                        .child(
                            Button::new("sheet-second", "Delete").focus_handle(self.second.clone()),
                        ),
                )
        }
    }

    /// Opens the harness with the opener focused, and returns the view, the dismissal count, the
    /// opener and the first control.
    fn open_window(
        cx: &mut TestAppContext,
    ) -> (
        Entity<Harness>,
        &mut VisualTestContext,
        Rc<Cell<u32>>,
        FocusHandle,
        FocusHandle,
        FocusHandle,
    ) {
        let dismissals: Rc<Cell<u32>> = Rc::default();
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            opener: cx.focus_handle(),
            first: cx.focus_handle(),
            second: cx.focus_handle(),
            open: false,
            busy: false,
            dismissals: Rc::clone(&dismissals),
        });
        cx.run_until_parked();
        let (opener, first, second) = view.read_with(cx, |h, _| {
            (h.opener.clone(), h.first.clone(), h.second.clone())
        });
        focus_and_draw(cx, &opener);
        (view, cx, dismissals, opener, first, second)
    }

    /// Opens or closes the sheet and runs its motion to the end, so positions are final.
    fn set_open(view: &Entity<Harness>, cx: &mut VisualTestContext, open: bool) {
        view.update(cx, |harness, cx| {
            harness.open = open;
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.executor().advance_clock(DURATION_SLOW);
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// Presses at `from`, moves down by `dy` and releases. The first move past the drag threshold
    /// starts the drag, so a later move carries the travel.
    fn drag_down(cx: &mut VisualTestContext, from: Point<Pixels>, dy: Pixels) {
        cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        for step in [px(1.), dy / 2.0, dy] {
            cx.simulate_mouse_move(
                from + point(px(0.), step),
                MouseButton::Left,
                Modifiers::none(),
            );
        }
        cx.simulate_mouse_up(
            from + point(px(0.), dy),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.run_until_parked();
    }

    #[gpui::test]
    fn escape_dismisses_and_is_ignored_while_busy(cx: &mut TestAppContext) {
        let (view, cx, dismissals, _opener, _first, _second) = open_window(cx);
        set_open(&view, cx, true);
        press_key(cx, "escape");
        assert_eq!(dismissals.get(), 1);

        view.update(cx, |harness, cx| {
            harness.busy = true;
            cx.notify();
        });
        press_key(cx, "escape");
        assert_eq!(dismissals.get(), 1, "Escape does nothing while busy");
    }

    #[gpui::test]
    fn a_press_on_the_scrim_dismisses_and_a_press_on_the_panel_does_not(cx: &mut TestAppContext) {
        let (view, cx, dismissals, _opener, _first, _second) = open_window(cx);
        set_open(&view, cx, true);
        let panel = cx.debug_bounds("sheet-panel").expect("the panel is drawn");
        cx.simulate_click(panel.origin + point(px(12.), px(12.)), Modifiers::none());
        assert_eq!(
            dismissals.get(),
            0,
            "a press on the panel is not outside it"
        );
        cx.simulate_click(point(px(4.), px(4.)), Modifiers::none());
        assert_eq!(dismissals.get(), 1, "a press on the scrim dismisses");
    }

    #[gpui::test]
    fn opening_moves_focus_into_the_sheet_and_closing_returns_it_to_the_opener(
        cx: &mut TestAppContext,
    ) {
        let (view, cx, _dismissals, opener, first, _second) = open_window(cx);
        set_open(&view, cx, true);
        cx.update(|window, _| assert!(first.is_focused(window), "the first control has the focus"));
        set_open(&view, cx, false);
        cx.update(|window, _| assert!(opener.is_focused(window), "focus is back on the opener"));
    }

    #[gpui::test]
    fn tab_keeps_the_focus_inside_the_sheet(cx: &mut TestAppContext) {
        let (view, cx, _dismissals, opener, first, second) = open_window(cx);
        set_open(&view, cx, true);
        for _ in 0..4 {
            press_key(cx, "tab");
            cx.update(|window, _| {
                assert!(!opener.is_focused(window), "Tab never reaches the opener");
                assert!(
                    first.is_focused(window) || second.is_focused(window),
                    "focus stays on the sheet's controls"
                );
            });
        }
    }

    #[gpui::test]
    fn a_handle_drag_past_a_third_closes_and_a_short_one_snaps_back(cx: &mut TestAppContext) {
        let (view, cx, dismissals, _opener, _first, _second) = open_window(cx);
        set_open(&view, cx, true);
        let handle = cx
            .debug_bounds("sheet-handle")
            .expect("the handle is drawn")
            .center();
        let height = cx
            .debug_bounds("sheet-panel")
            .expect("the panel is drawn")
            .size
            .height;

        drag_down(cx, handle, px(10.));
        assert_eq!(dismissals.get(), 0, "a short drag snaps back");

        drag_down(cx, handle, height * (1.0 / 3.0) + px(10.));
        assert_eq!(
            dismissals.get(),
            1,
            "a drag past a third of the sheet closes it"
        );
    }
}
