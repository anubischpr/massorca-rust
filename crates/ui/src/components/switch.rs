//! Switch: a binary on-off setting that applies at once
//! (docs/design/components/Switch.md).
//!
//! Space toggles a focused switch and Enter does nothing (Switch.md, keyboard).

use crate::a11y;
use crate::components::button::{HOVER_GROUP, StatePreview, spinner};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    BORDER_WIDTH_DEFAULT, Color, DURATION_FAST, EASE_STANDARD, Easing, Palette, RADIUS_FULL,
    SIZE_ICON_XS, SIZE_SWITCH_KNOB, SIZE_SWITCH_TRACK_HEIGHT, SIZE_SWITCH_TRACK_WIDTH, SPACE_1,
    SPACE_3, SPACE_HALF, TYPE_BODY_SM,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    Animation, AnimationExt, App, ClickEvent, ElementId, FocusHandle, InteractiveElement,
    IntoElement, KeyboardButton, ParentElement, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Toggled, Window, div, px,
};
use std::rc::Rc;

/// The value a click or Space asks for.
type ChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// Maps `t` in [0, 1] through `easing` (handoff 2.6). Linear is the identity;
/// a cubic bezier is solved by bisection on its x curve.
pub(crate) fn ease(easing: Easing, t: f32) -> f32 {
    match easing {
        Easing::Linear => t,
        Easing::CubicBezier(x1, y1, x2, y2) => {
            let (mut low, mut high) = (0.0_f32, 1.0_f32);
            for _ in 0..32 {
                let mid = 0.5 * (low + high);
                if bezier(x1, x2, mid) < t {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            bezier(y1, y2, 0.5 * (low + high))
        }
    }
}

fn bezier(p1: f32, p2: f32, s: f32) -> f32 {
    let u = 1.0 - s;
    3.0 * u * u * s * p1 + 3.0 * u * s * s * p2 + s * s * s
}

/// The knob's left edge from the track's padding edge: at the start when off, at
/// the end when on. With the 1 px border it sits 3 px in from the outer edge.
fn knob_left(checked: bool) -> f32 {
    let inner = SIZE_SWITCH_TRACK_WIDTH - 2.0 * BORDER_WIDTH_DEFAULT;
    if checked {
        inner - SPACE_HALF - SIZE_SWITCH_KNOB
    } else {
        SPACE_HALF
    }
}

/// The value a toggle last drew and how many times it has changed. Each change
/// gets its own animation id, so it moves once and is then still (handoff 2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToggleMotion {
    pub(crate) from: bool,
    pub(crate) to: bool,
    pub(crate) changes: usize,
}

impl ToggleMotion {
    pub(crate) fn new(checked: bool) -> Self {
        Self {
            from: checked,
            to: checked,
            changes: 0,
        }
    }

    /// Records the value drawn now; a new value starts a change from the old one.
    pub(crate) fn observe(&mut self, checked: bool) {
        if checked != self.to {
            self.from = self.to;
            self.to = checked;
            self.changes += 1;
        }
    }
}

/// What the switch looks like right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visual {
    Rest,
    Hover,
    Pressed,
    Disabled,
}

/// Solid colours of the track and knob in one state (Switch.md, States).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Paint {
    track: Color,
    border: Color,
    knob: Color,
}

fn paint(checked: bool, visual: Visual, p: &Palette) -> Paint {
    match (checked, visual) {
        (_, Visual::Disabled) => Paint {
            track: p.fill.disabled,
            border: p.border.disabled,
            knob: p.text.disabled,
        },
        (false, Visual::Rest) => Paint {
            track: p.surface.sunken,
            border: p.border.strong,
            knob: p.switch.knob_off,
        },
        (false, Visual::Hover) => Paint {
            track: p.fill.hover_raised,
            border: p.border.strong,
            knob: p.text.primary,
        },
        (false, Visual::Pressed) => Paint {
            track: p.fill.pressed_raised,
            border: p.border.strong,
            knob: p.text.primary,
        },
        (true, Visual::Rest) => Paint {
            track: p.accent.r#default,
            border: p.accent.r#default,
            knob: p.text.on_accent,
        },
        (true, Visual::Hover) => Paint {
            track: p.accent.hover,
            border: p.accent.hover,
            knob: p.text.on_accent,
        },
        (true, Visual::Pressed) => Paint {
            track: p.accent.pressed,
            border: p.accent.pressed,
            knob: p.text.on_accent,
        },
    }
}

/// A line under the switch: helper text, or the error with its glyph (Switch.md, error).
pub(crate) fn helper_line(
    text: SharedString,
    colour: Color,
    glyph: Option<IconName>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .text_style(TYPE_BODY_SM)
        .text_color(colour)
        .children(glyph.map(|name| Icon::new(name).size(IconSize::XSmall).color(colour)))
        .child(text)
}

/// Enter is not a switch key: only Space and pointer clicks toggle.
fn is_enter(event: &ClickEvent) -> bool {
    matches!(event, ClickEvent::Keyboard(key) if matches!(key.button, KeyboardButton::Enter))
}

/// A binary on-off setting. The caller keeps the value and updates it in `on_change`.
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    label: Option<SharedString>,
    accessible_name: Option<SharedString>,
    helper: Option<SharedString>,
    error: Option<SharedString>,
    disabled: bool,
    loading: bool,
    preview: Option<StatePreview>,
    focus: Option<FocusHandle>,
    on_change: Option<ChangeHandler>,
}

impl std::fmt::Debug for Switch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Switch")
            .field("id", &self.id)
            .field("checked", &self.checked)
            .field("disabled", &self.disabled)
            .field("loading", &self.loading)
            .finish_non_exhaustive()
    }
}

impl Switch {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            label: None,
            accessible_name: None,
            helper: None,
            error: None,
            disabled: false,
            loading: false,
            preview: None,
            focus: None,
            on_change: None,
        }
    }

    /// The name a screen reader gives the track when no visible label names it (a row whose name is elsewhere).
    pub fn accessible_name(mut self, name: impl Into<SharedString>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// The setting's name, on the left of the track.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A line under the switch that says what the setting does.
    pub fn helper(mut self, text: impl Into<SharedString>) -> Self {
        self.helper = Some(text.into());
        self
    }

    /// A failed change: the switch keeps its previous value and shows this in danger.
    pub fn error(mut self, text: impl Into<SharedString>) -> Self {
        self.error = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The change is being saved: the knob becomes a spinner.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Forces a visual state (gallery and visual checks only).
    pub fn preview(mut self, state: StatePreview) -> Self {
        self.preview = Some(state);
        self
    }

    /// Keyboard focus lives with the caller.
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }

    /// Called with the value a click or Space asks for.
    pub fn on_change(mut self, listener: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(listener));
        self
    }

    pub fn checked(&self) -> bool {
        self.checked
    }

    /// The value a click asks for.
    pub fn toggled(&self) -> bool {
        !self.checked
    }

    /// Whether a click or Space reaches `on_change`.
    pub fn activates(&self) -> bool {
        !self.disabled && !self.loading
    }
}

impl Switch {
    /// The track is the focusable control, so it carries the switch's role, name and state.
    fn track(&self, window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
        let theme = *cx.theme();
        let p = &theme.palette;
        let checked = self.checked;
        let interactive = self.activates();
        let visual = match (self.disabled, self.preview) {
            (true, _) => Visual::Disabled,
            (false, Some(StatePreview::Hover)) => Visual::Hover,
            (false, Some(StatePreview::Pressed)) => Visual::Pressed,
            _ => Visual::Rest,
        };
        let shown = paint(checked, visual, p);
        let hover = paint(checked, Visual::Hover, p);
        let pressed = paint(checked, Visual::Pressed, p);
        let hover_knob = if interactive { hover.knob } else { shown.knob };
        // GPUI reads a tracked handle's Tab stop from the handle, not the element.
        let focus = self
            .focus
            .clone()
            .unwrap_or_else(|| {
                window
                    .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone()
            })
            .tab_stop(!self.disabled);
        let (from, to, changes) = window
            .use_keyed_state(self.id.clone(), cx, |_, _| ToggleMotion::new(checked))
            .update(cx, |motion, _| {
                motion.observe(checked);
                (motion.from, motion.to, motion.changes)
            });

        let thumb = if self.loading {
            div()
                .absolute()
                .top(px(SPACE_HALF))
                .left(px(knob_left(checked)))
                .size(px(SIZE_SWITCH_KNOB))
                .flex()
                .items_center()
                .justify_center()
                .child(spinner(SIZE_ICON_XS, shown.knob))
                .into_any_element()
        } else {
            let (start, end) = (knob_left(from), knob_left(to));
            div()
                .absolute()
                .top(px(SPACE_HALF))
                .size(px(SIZE_SWITCH_KNOB))
                .rounded(px(RADIUS_FULL))
                .bg(shown.knob)
                .group_hover(HOVER_GROUP, move |style| style.bg(hover_knob))
                .with_animation(
                    ("switch-knob", changes),
                    Animation::new(DURATION_FAST),
                    move |knob, delta| {
                        knob.left(px(start + (end - start) * ease(EASE_STANDARD, delta)))
                    },
                )
                .into_any_element()
        };

        let mut track = div()
            .id(self.id.clone())
            .role(Role::Switch)
            .aria_toggled(Toggled::from(checked))
            .group(HOVER_GROUP)
            .track_focus(&focus)
            .focus_ring(&theme)
            .flex_none()
            .w(px(SIZE_SWITCH_TRACK_WIDTH))
            .h(px(SIZE_SWITCH_TRACK_HEIGHT))
            .rounded(px(RADIUS_FULL))
            .border_1()
            .border_color(shown.border)
            .bg(shown.track)
            .when(self.preview == Some(StatePreview::Focus), |el| {
                el.shadow(box_shadows(theme.shadows.focus))
            })
            .child(thumb);
        if interactive && self.preview.is_none() {
            track = track
                .cursor_pointer()
                .hover(move |s| s.bg(hover.track).border_color(hover.border))
                .active(move |s| s.bg(pressed.track).border_color(pressed.border));
        }
        if let Some(name) = self.label.as_ref().or(self.accessible_name.as_ref()) {
            track = track.aria_label(name.clone());
        }
        if let Some(on_change) = self.on_change.clone().filter(|_| interactive) {
            let next = !checked;
            track = track.on_click(move |event: &ClickEvent, window, cx| {
                if !is_enter(event) {
                    on_change(next, window, cx);
                }
            });
        }
        a11y::disabled(a11y::busy(track, self.loading), self.disabled)
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let track = self.track(window, cx);
        let theme = *cx.theme();
        let p = &theme.palette;

        let mut row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(SPACE_3));
        if let Some(label) = self.label {
            row = row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_body()
                    .text_color(p.text.primary)
                    .child(label),
            );
        }
        let mut column = div()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .child(row.child(track));
        if let Some(error) = self.error {
            column = column.child(helper_line(
                error,
                p.status.danger_fg,
                Some(IconName::OctagonAlert),
            ));
        } else if let Some(helper) = self.helper {
            column = column.child(helper_line(helper, p.text.secondary, None));
        }
        column
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use crate::theme::Theme;
    use crate::tokens::Appearance;
    use gpui::Render;
    use std::cell::RefCell;

    fn dark() -> Theme {
        Theme::new(Appearance::Dark)
    }

    #[test]
    fn knob_sits_three_pixels_in_from_the_track_edge_off_and_on() {
        assert_eq!(knob_left(false) + BORDER_WIDTH_DEFAULT, 3.0);
        assert_eq!(
            knob_left(true) + BORDER_WIDTH_DEFAULT + SIZE_SWITCH_KNOB,
            SIZE_SWITCH_TRACK_WIDTH - 3.0
        );
    }

    #[test]
    fn linear_is_the_identity_and_the_standard_curve_starts_fast() {
        assert!((ease(Easing::Linear, 0.25) - 0.25).abs() < 1e-6);
        assert!(ease(EASE_STANDARD, 0.0).abs() < 1e-3);
        assert!((ease(EASE_STANDARD, 1.0) - 1.0).abs() < 1e-3);
        assert!(ease(EASE_STANDARD, 0.5) > 0.5);
        let mut last = 0.0;
        for step in 0..=10 {
            let value = ease(EASE_STANDARD, step as f32 / 10.0);
            assert!(value >= last - 1e-4, "not monotonic at {step}");
            last = value;
        }
    }

    #[test]
    fn a_change_animates_once_per_value() {
        let mut motion = ToggleMotion::new(true);
        motion.observe(true);
        assert_eq!(motion.changes, 0);
        motion.observe(false);
        assert_eq!((motion.from, motion.to, motion.changes), (true, false, 1));
        motion.observe(false);
        assert_eq!(motion.changes, 1);
    }

    #[test]
    fn off_track_is_sunken_with_a_strong_border_and_on_track_is_accent() {
        let t = dark();
        let off = paint(false, Visual::Rest, &t.palette);
        assert_eq!(off.track, t.palette.surface.sunken);
        assert_eq!(off.border, t.palette.border.strong);
        assert_eq!(off.knob, t.palette.switch.knob_off);
        let on = paint(true, Visual::Rest, &t.palette);
        assert_eq!(on.track, t.palette.accent.r#default);
        assert_eq!(on.knob, t.palette.text.on_accent);
    }

    #[test]
    fn hover_lights_the_off_track_and_darkens_the_on_accent() {
        let t = dark();
        let off = paint(false, Visual::Hover, &t.palette);
        assert_eq!(off.track, t.palette.fill.hover_raised);
        assert_eq!(off.knob, t.palette.text.primary);
        let on = paint(true, Visual::Hover, &t.palette);
        assert_eq!(on.track, t.palette.accent.hover);
    }

    #[test]
    fn disabled_uses_the_disabled_tokens_in_both_states() {
        let t = dark();
        for checked in [false, true] {
            let p = paint(checked, Visual::Disabled, &t.palette);
            assert_eq!(p.track, t.palette.fill.disabled);
            assert_eq!(p.border, t.palette.border.disabled);
            assert_eq!(p.knob, t.palette.text.disabled);
        }
    }

    #[test]
    fn a_toggle_asks_for_the_opposite_value_and_disabled_or_loading_do_not_activate() {
        assert!(Switch::new("a", false).toggled());
        assert!(!Switch::new("a", true).toggled());
        assert!(Switch::new("a", false).activates());
        assert!(!Switch::new("a", false).disabled(true).activates());
        assert!(!Switch::new("a", false).loading(true).activates());
    }

    /// Hosts one switch that records each value it asks for.
    struct Harness {
        focus: FocusHandle,
        asked: Rc<RefCell<Vec<bool>>>,
        disabled: bool,
    }

    impl Harness {
        fn new(
            cx: &mut gpui::Context<Self>,
            asked: Rc<RefCell<Vec<bool>>>,
            disabled: bool,
        ) -> Self {
            Self {
                focus: cx.focus_handle(),
                asked,
                disabled,
            }
        }
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let asked = self.asked.clone();
            div().child(
                div()
                    .absolute()
                    .top(gpui::px(0.))
                    .left(gpui::px(0.))
                    .debug_selector(|| "switch".into())
                    .child(
                        Switch::new("under-test", false)
                            .focus_handle(self.focus.clone())
                            .disabled(self.disabled)
                            .on_change(move |next, _, _| asked.borrow_mut().push(next)),
                    ),
            )
        }
    }

    #[gpui::test]
    fn pointer_and_space_each_toggle_and_enter_does_nothing(cx: &mut gpui::TestAppContext) {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view(|_, cx| Harness::new(cx, asked.clone(), false));
        cx.run_until_parked();
        let bounds = cx.debug_bounds("switch").expect("switch is drawn");
        cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        assert_eq!(*asked.borrow(), vec![true]);
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "enter");
        assert_eq!(asked.borrow().len(), 1, "Enter does nothing on a switch");
        press_key(cx, "space");
        assert_eq!(*asked.borrow(), vec![true, true]);
    }

    #[gpui::test]
    fn disabled_switch_ignores_pointer_and_keys(cx: &mut gpui::TestAppContext) {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view(|_, cx| Harness::new(cx, asked.clone(), true));
        cx.run_until_parked();
        let bounds = cx.debug_bounds("switch").expect("switch is drawn");
        cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "space");
        assert!(asked.borrow().is_empty());
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{
        TestAppContext,
        accesskit::{Role, Toggled},
    };

    #[gpui::test]
    fn a_switch_is_named_by_its_label_and_reports_whether_it_is_on(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                Switch::new("sync", true)
                    .label("Sync settings")
                    .track(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.role(), Role::Switch);
        assert_eq!(node.label(), Some("Sync settings"));
        assert_eq!(node.toggled(), Some(Toggled::True));
    }

    #[gpui::test]
    fn a_switch_without_a_label_is_named_by_its_accessible_name(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                Switch::new("row-switch", true)
                    .accessible_name("Enable Quick notes")
                    .track(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.role(), Role::Switch);
        assert_eq!(node.label(), Some("Enable Quick notes"));
        assert_eq!(node.toggled(), Some(Toggled::True));
    }

    #[gpui::test]
    fn a_disabled_switch_is_disabled_and_a_loading_one_is_busy(cx: &mut TestAppContext) {
        let disabled = root(cx, |window, cx| {
            reported(
                Switch::new("sync", false)
                    .label("Sync")
                    .disabled(true)
                    .track(window, cx),
            )
        })
        .expect("reported");
        assert!(disabled.is_disabled());

        let loading = root(cx, |window, cx| {
            reported(
                Switch::new("sync", false)
                    .label("Sync")
                    .loading(true)
                    .track(window, cx),
            )
        })
        .expect("reported");
        assert!(loading.is_busy());
    }
}
