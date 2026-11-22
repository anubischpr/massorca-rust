//! Splitter: the draggable divider between two side-by-side regions (docs/design/components/Splitter.md).
//! The 1 px line sits centred in a 6 px hit area. A drag, the Left and Right keys (8 px, 24 px with
//! Shift), Home, End and a double click resize the region on one side. The width rules are plain
//! Rust in [`SplitterModel`]; the element draws them and reports the new [`Extent`].

use crate::theme::ActiveTheme;
use crate::tokens::{
    BORDER_FOCUS_WIDTH, BORDER_SPLITTER_HIT, BORDER_WIDTH_DEFAULT, BORDER_WIDTH_THICK,
    LAYOUT_CONVERSATION_MIN, SPACE_2, SPACE_6,
};
use gpui::{
    App, AppContext, ClickEvent, CursorStyle, DragMoveEvent, ElementId, EmptyView, Entity,
    FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseUpEvent, Orientation, ParentElement, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use std::fmt;
use std::rc::Rc;

/// Distance below a region's minimum at which dragging collapses it (design-system 4.8).
pub const SPLITTER_SNAP_PX: f32 = 40.0;
/// Edge travel of one Left or Right press, and with Shift (Splitter.md: space.2 and space.6).
pub const KEY_STEP_PX: f32 = SPACE_2;
pub const KEY_STEP_SHIFT_PX: f32 = SPACE_6;
/// Group name that lets the line react to a hover over its hit area.
const HIT_GROUP: &str = "splitter-hit";

/// Which region the splitter resizes: the one left of it (Sidebar) or right of it (Work panel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionSide {
    Left,
    Right,
}

impl RegionSide {
    /// How far the region grows when the edge moves one pixel to the right.
    fn growth(self) -> f32 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

/// A region is open at a width, or collapsed: hidden, with its splitter at the edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Extent {
    Open(f32),
    Collapsed,
}

/// The width rules of one region: its default, its minimum and maximum (layout tokens), and the
/// room the Conversation minimum leaves it (see [`room_beside_conversation`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitterLimits {
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub room: f32,
}

impl SplitterLimits {
    /// The widest the region may open: its maximum or the room left, never below its minimum.
    pub fn ceiling(self) -> f32 {
        self.max.min(self.room).max(self.min)
    }

    fn clamp(self, width: f32) -> f32 {
        width.clamp(self.min, self.ceiling())
    }
}

/// The widest a region may be while the Conversation keeps its minimum: the canvas width less
/// the other regions beside the Conversation and the Conversation's own minimum.
pub fn room_beside_conversation(canvas_width: f32, other_regions: f32) -> f32 {
    canvas_width - other_regions - LAYOUT_CONVERSATION_MIN
}

/// The state of one splitter: the region it resizes, its width rules and its extent now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitterModel {
    pub side: RegionSide,
    pub limits: SplitterLimits,
    pub extent: Extent,
}

impl SplitterModel {
    pub fn new(side: RegionSide, limits: SplitterLimits, extent: Extent) -> Self {
        Self {
            side,
            limits,
            extent,
        }
    }

    /// The extent after a pointer drag that travelled `travel` px from a press made at `start`.
    /// An open region stops at its minimum and collapses once the pointer passes it by
    /// [`SPLITTER_SNAP_PX`]; a collapsed one reopens when the pointer reaches the minimum.
    pub fn dragged(self, start: Extent, travel: f32) -> Extent {
        let from = match start {
            Extent::Open(width) => width,
            Extent::Collapsed => 0.0,
        };
        let target = from + self.side.growth() * travel;
        match start {
            Extent::Open(_) if target < self.limits.min - SPLITTER_SNAP_PX => Extent::Collapsed,
            Extent::Collapsed if target < self.limits.min => Extent::Collapsed,
            _ => Extent::Open(self.limits.clamp(target)),
        }
    }

    /// The extent after the edge moved `dx` px with a key. A key never collapses a region.
    pub fn nudged(self, dx: f32) -> Extent {
        match self.extent {
            Extent::Open(width) => self.open_at(width + self.side.growth() * dx),
            Extent::Collapsed => Extent::Collapsed,
        }
    }

    /// Home: the minimum width. A collapsed region stays collapsed.
    pub fn home(self) -> Extent {
        self.open_at(self.limits.min)
    }

    /// End: the widest allowed width. A collapsed region stays collapsed.
    pub fn end(self) -> Extent {
        self.open_at(self.limits.ceiling())
    }

    /// A double click: the default width, reopening a collapsed region.
    pub fn reset(self) -> Extent {
        Extent::Open(self.limits.clamp(self.limits.default))
    }

    /// The width a screen reader is told; 0 while collapsed.
    pub fn width(self) -> f32 {
        match self.extent {
            Extent::Open(width) => width,
            Extent::Collapsed => 0.0,
        }
    }

    fn open_at(self, width: f32) -> Extent {
        match self.extent {
            Extent::Open(_) => Extent::Open(self.limits.clamp(width)),
            Extent::Collapsed => Extent::Collapsed,
        }
    }
}

/// Reports the extent a drag, key press or double click leaves the region at.
pub type ChangeHandler = Rc<dyn Fn(Extent, &mut Window, &mut App)>;

/// Forces a visual state so the gallery can show it without a pointer or keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitterPreview {
    Hover,
    Pressed,
    Focus,
}

/// The divider between two regions. The owner keeps the model and applies each reported extent.
#[derive(IntoElement)]
pub struct Splitter {
    id: ElementId,
    region: SharedString,
    model: SplitterModel,
    focus: Option<FocusHandle>,
    preview: Option<SplitterPreview>,
    on_change: Option<ChangeHandler>,
}

impl fmt::Debug for Splitter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Splitter")
            .field("id", &self.id)
            .field("region", &self.region)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl Splitter {
    /// `region` names the resized region as the accessible name reads it: "Work panel" gives
    /// "Resize Work panel".
    pub fn new(
        id: impl Into<ElementId>,
        region: impl Into<SharedString>,
        model: SplitterModel,
    ) -> Self {
        Self {
            id: id.into(),
            region: region.into(),
            model,
            focus: None,
            preview: None,
            on_change: None,
        }
    }

    /// Keyboard focus lives with the caller; without it the splitter keeps its own.
    pub fn focus_handle(mut self, focus: FocusHandle) -> Self {
        self.focus = Some(focus);
        self
    }

    /// Forces a visual state (gallery only).
    pub fn preview(mut self, preview: SplitterPreview) -> Self {
        self.preview = Some(preview);
        self
    }

    pub fn on_change(mut self, listener: impl Fn(Extent, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(listener));
        self
    }
}

/// Where a press began: the pointer's x and the extent the region had then.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Press {
    x: f32,
    start: Extent,
}

/// What the splitter keeps between frames: its own focus stop and the press in progress.
struct SplitterState {
    focus: FocusHandle,
    press: Option<Press>,
}

/// The payload of a splitter drag; only splitter drags reach its move handler.
struct SplitterDrag;

/// Clears the press when the button comes up, inside the hit area or outside it.
fn end_press(
    state: Entity<SplitterState>,
) -> impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static {
    move |_, _, cx| state.update(cx, |state, _| state.press = None)
}

impl RenderOnce for Splitter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.theme().palette;
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| SplitterState {
            focus: cx.focus_handle(),
            press: None,
        });
        let (own_focus, press) = {
            let state = state.read(cx);
            (state.focus.clone(), state.press)
        };
        let focus = self.focus.clone().unwrap_or(own_focus);
        let model = self.model;
        let on_change = self.on_change.clone();
        let pressed = press.is_some() || self.preview == Some(SplitterPreview::Pressed);
        let previewed_hover = self.preview == Some(SplitterPreview::Hover);
        let previewed_focus = self.preview == Some(SplitterPreview::Focus);

        let (line_width, line_colour) = if pressed {
            (BORDER_WIDTH_THICK, p.accent.default)
        } else if previewed_focus {
            (BORDER_FOCUS_WIDTH, p.border.focus)
        } else if previewed_hover {
            (BORDER_WIDTH_DEFAULT, p.border.strong)
        } else {
            (BORDER_WIDTH_DEFAULT, p.border.subtle)
        };
        let mut line = div()
            .id((self.id.clone(), "line"))
            .track_focus(&focus)
            .tab_stop(true)
            .role(Role::Splitter)
            .aria_label(format!("Resize {}", self.region))
            .aria_orientation(Orientation::Vertical)
            .aria_numeric_value(f64::from(model.width()))
            .aria_min_numeric_value(f64::from(model.limits.min))
            .aria_max_numeric_value(f64::from(model.limits.ceiling()))
            .h_full()
            .w(px(line_width))
            .bg(line_colour)
            .focus_visible(|s| s.bg(p.border.focus).w(px(BORDER_FOCUS_WIDTH)))
            .on_key_down({
                let on_change = on_change.clone();
                move |event: &KeyDownEvent, window, cx| {
                    let step = if event.keystroke.modifiers.shift {
                        KEY_STEP_SHIFT_PX
                    } else {
                        KEY_STEP_PX
                    };
                    let next = match event.keystroke.key.as_str() {
                        "left" => model.nudged(-step),
                        "right" => model.nudged(step),
                        "home" => model.home(),
                        "end" => model.end(),
                        _ => return,
                    };
                    cx.stop_propagation();
                    if next != model.extent
                        && let Some(on_change) = &on_change
                    {
                        on_change(next, window, cx);
                    }
                }
            });
        if !pressed && self.preview.is_none() {
            line = line.group_hover(HIT_GROUP, |s| s.bg(p.border.strong));
        }

        div()
            .id((self.id.clone(), "hit"))
            .group(HIT_GROUP)
            .h_full()
            .w(px(BORDER_SPLITTER_HIT))
            .flex_none()
            .flex()
            .justify_center()
            .cursor(CursorStyle::ResizeLeftRight)
            .debug_selector(|| "splitter-hit".into())
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                move |event: &MouseDownEvent, _, cx| {
                    let press = Press {
                        x: f32::from(event.position.x),
                        start: model.extent,
                    };
                    state.update(cx, |state, _| state.press = Some(press));
                }
            })
            .on_mouse_up(MouseButton::Left, end_press(state.clone()))
            .on_mouse_up_out(MouseButton::Left, end_press(state.clone()))
            .on_drag(SplitterDrag, |_, _, _, cx| cx.new(|_| EmptyView))
            .on_drag_move::<SplitterDrag>({
                let state = state.clone();
                let on_change = on_change.clone();
                move |event: &DragMoveEvent<SplitterDrag>, window, cx| {
                    let Some(press) = state.read(cx).press else {
                        return;
                    };
                    let travel = f32::from(event.event.position.x) - press.x;
                    let next = model.dragged(press.start, travel);
                    if next != model.extent
                        && let Some(on_change) = &on_change
                    {
                        on_change(next, window, cx);
                    }
                }
            })
            .on_click(move |event: &ClickEvent, window, cx| {
                let next = model.reset();
                if event.click_count() == 2
                    && next != model.extent
                    && let Some(on_change) = &on_change
                {
                    on_change(next, window, cx);
                }
            })
            .child(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(room: f32) -> SplitterLimits {
        SplitterLimits {
            default: 256.0,
            min: 200.0,
            max: 400.0,
            room,
        }
    }

    fn model(side: RegionSide, width: f32) -> SplitterModel {
        SplitterModel::new(side, limits(f32::INFINITY), Extent::Open(width))
    }

    #[test]
    fn a_left_region_grows_with_the_pointer_and_a_right_one_shrinks() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(left.dragged(Extent::Open(300.0), 50.0), Extent::Open(350.0));
        let right = model(RegionSide::Right, 300.0);
        assert_eq!(
            right.dragged(Extent::Open(300.0), 50.0),
            Extent::Open(250.0)
        );
    }

    #[test]
    fn an_open_region_stops_at_its_minimum_then_collapses_past_the_snap_distance() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(
            left.dragged(Extent::Open(300.0), -90.0),
            Extent::Open(210.0)
        );
        assert_eq!(
            left.dragged(Extent::Open(300.0), -100.0),
            Extent::Open(200.0)
        );
        assert_eq!(
            left.dragged(Extent::Open(300.0), -139.0),
            Extent::Open(200.0),
            "inside the snap distance it holds at the minimum"
        );
        assert_eq!(
            left.dragged(Extent::Open(300.0), -141.0),
            Extent::Collapsed,
            "past the minimum less 40 px it collapses"
        );
    }

    #[test]
    fn a_collapsed_region_reopens_at_the_minimum_when_the_pointer_comes_back() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(left.dragged(Extent::Collapsed, 150.0), Extent::Collapsed);
        assert_eq!(left.dragged(Extent::Collapsed, 200.0), Extent::Open(200.0));
    }

    #[test]
    fn the_room_beside_the_conversation_caps_the_width() {
        assert_eq!(room_beside_conversation(1000.0, 400.0), 240.0);
        let capped = SplitterModel::new(
            RegionSide::Left,
            SplitterLimits {
                room: room_beside_conversation(1000.0, 400.0),
                ..limits(0.0)
            },
            Extent::Open(300.0),
        );
        assert_eq!(capped.limits.ceiling(), 240.0);
        assert_eq!(
            capped.dragged(Extent::Open(300.0), 100.0),
            Extent::Open(240.0)
        );
    }

    #[test]
    fn the_ceiling_never_falls_below_the_minimum() {
        assert_eq!(limits(50.0).ceiling(), 200.0);
    }

    #[test]
    fn keys_move_by_their_step_and_never_collapse_the_region() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(left.nudged(KEY_STEP_PX), Extent::Open(308.0));
        assert_eq!(left.nudged(-1000.0), Extent::Open(200.0));
        let right = model(RegionSide::Right, 300.0);
        assert_eq!(right.nudged(KEY_STEP_SHIFT_PX), Extent::Open(276.0));
        let collapsed =
            SplitterModel::new(RegionSide::Left, limits(f32::INFINITY), Extent::Collapsed);
        assert_eq!(collapsed.nudged(KEY_STEP_PX), Extent::Collapsed);
    }

    #[test]
    fn home_and_end_go_to_the_limits_and_reset_goes_to_the_default() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(left.home(), Extent::Open(200.0));
        assert_eq!(left.end(), Extent::Open(400.0));
        assert_eq!(left.reset(), Extent::Open(256.0));
        let collapsed =
            SplitterModel::new(RegionSide::Left, limits(f32::INFINITY), Extent::Collapsed);
        assert_eq!(
            collapsed.reset(),
            Extent::Open(256.0),
            "a double click reopens"
        );
        assert_eq!(collapsed.home(), Extent::Collapsed);
    }

    #[test]
    fn the_accessible_width_is_zero_while_collapsed() {
        assert_eq!(model(RegionSide::Left, 300.0).width(), 300.0);
        let collapsed =
            SplitterModel::new(RegionSide::Left, limits(f32::INFINITY), Extent::Collapsed);
        assert_eq!(collapsed.width(), 0.0);
    }
}

#[cfg(test)]
mod element_tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use gpui::{
        Context, Entity, Modifiers, MouseDownEvent, MouseUpEvent, Render, TestAppContext,
        VisualTestContext, point, px,
    };
    use std::cell::RefCell;

    type Reported = Rc<RefCell<Vec<Extent>>>;

    /// Hosts one splitter. Its owner writes each reported extent back into the model, as an app does.
    struct Harness {
        model: SplitterModel,
        focus: FocusHandle,
        reported: Reported,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let owner = cx.entity();
            let reported = self.reported.clone();
            div().w(px(900.)).h(px(120.)).flex().child(
                Splitter::new("splitter-under-test", "Sidebar", self.model)
                    .focus_handle(self.focus.clone())
                    .on_change(move |extent, _, cx| {
                        reported.borrow_mut().push(extent);
                        owner.update(cx, |harness, cx| {
                            harness.model.extent = extent;
                            cx.notify();
                        });
                    }),
            )
        }
    }

    /// A Sidebar-like region: 200 to 400 px, default 256, open at 300.
    fn sidebar() -> SplitterModel {
        SplitterModel::new(
            RegionSide::Left,
            SplitterLimits {
                default: 256.0,
                min: 200.0,
                max: 400.0,
                room: f32::INFINITY,
            },
            Extent::Open(300.0),
        )
    }

    /// Opens a window with the splitter, focuses it and returns what it reports.
    fn open(
        cx: &mut TestAppContext,
        model: SplitterModel,
    ) -> (Entity<Harness>, &mut VisualTestContext, Reported) {
        let reported: Reported = Rc::default();
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            model,
            focus: cx.focus_handle(),
            reported: Rc::clone(&reported),
        });
        cx.run_until_parked();
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        (view, cx, reported)
    }

    /// Presses a key and draws again, so the owner's new model reaches the splitter.
    fn press(cx: &mut VisualTestContext, key: &str) {
        press_key(cx, key);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    fn hit_centre(cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
        cx.debug_bounds("splitter-hit")
            .expect("the hit area is drawn")
            .center()
    }

    #[gpui::test]
    fn right_and_left_move_the_edge_eight_px_and_shift_moves_it_24(cx: &mut TestAppContext) {
        let (_view, cx, reported) = open(cx, sidebar());
        press(cx, "right");
        press(cx, "shift-right");
        press(cx, "left");
        assert_eq!(
            *reported.borrow(),
            vec![
                Extent::Open(308.0),
                Extent::Open(332.0),
                Extent::Open(324.0)
            ]
        );
    }

    #[gpui::test]
    fn keys_stop_at_the_limits_and_home_and_end_go_to_them(cx: &mut TestAppContext) {
        let (_view, cx, reported) = open(cx, sidebar());
        press(cx, "end");
        press(cx, "right");
        press(cx, "home");
        press(cx, "left");
        assert_eq!(
            *reported.borrow(),
            vec![Extent::Open(400.0), Extent::Open(200.0)],
            "End reaches 400, Right stops there, Home reaches 200, Left stops there"
        );
    }

    #[gpui::test]
    fn a_double_click_resets_the_region_to_its_default_width(cx: &mut TestAppContext) {
        let (_view, cx, reported) = open(cx, sidebar());
        let centre = hit_centre(cx);
        cx.simulate_event(MouseDownEvent {
            position: centre,
            modifiers: Modifiers::none(),
            button: gpui::MouseButton::Left,
            click_count: 2,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position: centre,
            modifiers: Modifiers::none(),
            button: gpui::MouseButton::Left,
            click_count: 2,
        });
        assert_eq!(*reported.borrow(), vec![Extent::Open(256.0)]);
    }

    #[gpui::test]
    fn dragging_resizes_the_region_and_the_release_ends_the_drag(cx: &mut TestAppContext) {
        let (_view, cx, reported) = open(cx, sidebar());
        let start = hit_centre(cx);
        let at = |dx: f32| start + point(px(dx), px(0.));
        cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(at(20.0), gpui::MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(at(40.0), gpui::MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(at(40.0), gpui::MouseButton::Left, Modifiers::none());
        assert_eq!(reported.borrow().last(), Some(&Extent::Open(340.0)));
    }

    #[gpui::test]
    fn dragging_past_the_snap_distance_collapses_the_region(cx: &mut TestAppContext) {
        let (_view, cx, reported) = open(cx, sidebar());
        let start = hit_centre(cx);
        let at = |dx: f32| start + point(px(dx), px(0.));
        cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(at(-20.0), gpui::MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(at(-150.0), gpui::MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(at(-150.0), gpui::MouseButton::Left, Modifiers::none());
        assert_eq!(reported.borrow().last(), Some(&Extent::Collapsed));
    }
}
