//! The reasoning slider: the Model picker's reasoning levels as a vertical, snapping slider with one
//! stop per level the model declares. It draws the track, the stops, the brand fill, the thumb and the
//! level labels, and reports each change to its owner, which keeps the popover open.

mod brand;
mod stops;

use std::rc::Rc;

use gpui::accesskit::Orientation;
use gpui::{
    App, ClickEvent, ElementId, FocusHandle, Hsla, InteractiveElement, IntoElement, KeyBinding,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, actions, div, px, relative,
};

pub use brand::{BrandColours, brand_colours, name_colour};
pub use stops::{has_slider, is_top_level};
use stops::{STOP_INSET, Slide, nearest_stop, stop_fraction, value_text};

use crate::components::popover::TriggerBounds;
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    RADIUS_FULL, SIZE_REASONING_SLIDER_THUMB, SIZE_REASONING_SLIDER_TRACK, TYPE_BODY,
    TYPE_BODY_STRONG,
};

/// The key context of the slider. Its keys are bound here, so they win over the picker's list keys.
pub const KEY_CONTEXT: &str = "ReasoningSlider";

actions!(
    reasoning_slider,
    [RaiseLevel, LowerLevel, FirstLevel, LastLevel]
);

/// Binds the slider's keys in its own key context: Up and Right raise, Down and Left lower, Home and
/// Page Down go to the bottom level, End and Page Up to the top one.
pub fn bind_keys(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", RaiseLevel, context),
        KeyBinding::new("right", RaiseLevel, context),
        KeyBinding::new("down", LowerLevel, context),
        KeyBinding::new("left", LowerLevel, context),
        KeyBinding::new("home", FirstLevel, context),
        KeyBinding::new("pagedown", FirstLevel, context),
        KeyBinding::new("end", LastLevel, context),
        KeyBinding::new("pageup", LastLevel, context),
    ]);
}

/// Reports the level the user picked: its index in the model's levels.
pub type Pick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// The slider of one model: its levels, the selected one, the fill colours and the focus it takes.
#[derive(IntoElement)]
pub struct ReasoningSlider {
    pub id: ElementId,
    pub levels: Vec<SharedString>,
    pub selected: usize,
    pub brand: BrandColours,
    /// The accessible name, such as "Reasoning for Opus 5.5".
    pub label: SharedString,
    pub focus: FocusHandle,
    /// The track's bounds from its last paint, which a pointer position is read against.
    pub track: TriggerBounds,
    pub on_change: Pick,
}

/// Moves the selection by `slide` and reports the new level when it changes.
fn slide_to(
    slide: Slide,
    selected: usize,
    count: usize,
    pick: &Pick,
    window: &mut Window,
    cx: &mut App,
) {
    let next = slide.apply(selected, count);
    if next != selected {
        pick(next, window, cx);
    }
}

/// Selects the stop nearest to the pointer's height `y` on the track, and reports it when it changes.
fn pick_at(
    track: &TriggerBounds,
    y: Pixels,
    count: usize,
    selected: usize,
    pick: &Pick,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(bounds) = track.get() else {
        return;
    };
    let offset = f32::from(bounds.bottom() - y);
    let next = nearest_stop(offset, f32::from(bounds.size.height), count);
    if next != selected {
        pick(next, window, cx);
    }
}

impl RenderOnce for ReasoningSlider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let count = self.levels.len();
        if !has_slider(count) {
            return div().into_any_element();
        }
        let selected = self.selected.min(count - 1);
        let fraction = stop_fraction(selected, count);
        let tertiary: Hsla = p.text.tertiary.into();
        let pick = self.on_change.clone();
        let value: SharedString = value_text(&self.levels[selected], selected, count).into();

        let fill = div()
            .absolute()
            .top(relative(1.0 - fraction))
            .mt(px(-STOP_INSET))
            .bottom(px(-STOP_INSET))
            .left_0()
            .right_0()
            .rounded(px(RADIUS_FULL))
            .overflow_hidden()
            .bg(gpui::linear_gradient(
                0.,
                gpui::linear_color_stop(self.brand.first, 0.),
                gpui::linear_color_stop(self.brand.second, 1.),
            ));
        let stops = (0..count).map(|index| {
            let colour = if index <= selected {
                gpui::white().opacity(0.7)
            } else {
                tertiary.opacity(0.45)
            };
            div()
                .id(("reasoning-stop", index))
                .absolute()
                .top(relative(1.0 - stop_fraction(index, count)))
                .mt(px(-2.))
                .left(px((SIZE_REASONING_SLIDER_TRACK - 4.) / 2.))
                .size(px(4.))
                .rounded(px(2.))
                .bg(colour)
        });
        let thumb = div()
            .absolute()
            .top(relative(1.0 - fraction))
            .mt(px(-SIZE_REASONING_SLIDER_THUMB / 2.))
            .left(px((SIZE_REASONING_SLIDER_TRACK - SIZE_REASONING_SLIDER_THUMB) / 2.))
            .size(px(SIZE_REASONING_SLIDER_THUMB))
            .rounded(px(RADIUS_FULL))
            .bg(p.slider.thumb)
            .border_1()
            .border_color(Hsla::from(p.border.default).opacity(0.5))
            .shadow(box_shadows(theme.shadows.e1));
        let lane = div()
            .absolute()
            .top(px(STOP_INSET))
            .bottom(px(STOP_INSET))
            .left_0()
            .right_0()
            .child(fill)
            .children(stops)
            .child(thumb);

        let track = div()
            .id(self.id.clone())
            .size_full()
            .relative()
            .rounded(px(RADIUS_FULL))
            .bg(p.surface.sunken)
            .border_1()
            .border_color(p.border.subtle)
            .role(Role::Slider)
            .aria_label(self.label.clone())
            .aria_value(value)
            .aria_numeric_value((selected + 1) as f64)
            .aria_orientation(Orientation::Vertical)
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .debug_selector(|| "reasoning-slider".to_owned())
            .child(lane)
            .on_action({
                let pick = pick.clone();
                move |_: &RaiseLevel, window, cx| {
                    slide_to(Slide::Raise, selected, count, &pick, window, cx)
                }
            })
            .on_action({
                let pick = pick.clone();
                move |_: &LowerLevel, window, cx| {
                    slide_to(Slide::Lower, selected, count, &pick, window, cx)
                }
            })
            .on_action({
                let pick = pick.clone();
                move |_: &FirstLevel, window, cx| {
                    slide_to(Slide::First, selected, count, &pick, window, cx)
                }
            })
            .on_action({
                let pick = pick.clone();
                move |_: &LastLevel, window, cx| {
                    slide_to(Slide::Last, selected, count, &pick, window, cx)
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let (pick, track, focus) = (pick.clone(), self.track.clone(), self.focus.clone());
                move |event: &MouseDownEvent, window, cx| {
                    window.focus(&focus, cx);
                    pick_at(&track, event.position.y, count, selected, &pick, window, cx);
                }
            })
            .on_mouse_move({
                let (pick, track) = (pick.clone(), self.track.clone());
                move |event: &MouseMoveEvent, window, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        pick_at(&track, event.position.y, count, selected, &pick, window, cx);
                    }
                }
            });

        let labels = (0..count).map(|index| {
            let active = index == selected;
            let pick = pick.clone();
            div()
                .id(("reasoning-level", index))
                .debug_selector(move || format!("reasoning-level-{index}"))
                .absolute()
                .top(relative(1.0 - stop_fraction(index, count)))
                .mt(px(-10.))
                .h(px(20.))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_style(if active { TYPE_BODY_STRONG } else { TYPE_BODY })
                .text_color(if active { p.text.primary } else { p.text.tertiary })
                .on_click(move |_: &ClickEvent, window, cx| pick(index, window, cx))
                .child(self.levels[index].clone())
        });
        let label_lane = div()
            .absolute()
            .top(px(STOP_INSET))
            .bottom(px(STOP_INSET))
            .left(px(SIZE_REASONING_SLIDER_TRACK + STOP_INSET))
            .right_0()
            .children(labels);

        let host = self
            .track
            .track(
                div()
                    .relative()
                    .flex_none()
                    .w(px(SIZE_REASONING_SLIDER_TRACK))
                    .h_full(),
            )
            .child(track);
        div()
            .relative()
            .flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(host)
            .child(label_lane)
            .into_any_element()
    }
}
