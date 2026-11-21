//! Checkbox: one independent option, checked, unchecked or indeterminate
//! (docs/design/components/Checkbox.md).
//!
//! Space toggles a focused checkbox and Enter does nothing (Checkbox.md, keyboard).

use crate::components::button::{HOVER_GROUP, StatePreview, glyph};
use crate::components::switch::{ToggleMotion, ease, helper_line};
use crate::icon::{IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    Color, DURATION_FAST, EASE_STANDARD, Palette, RADIUS_MD, SIZE_HIT_POINTER, SIZE_ICON_SM,
    SPACE_2, SPACE_HALF,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    Animation, AnimationExt, App, ClickEvent, ElementId, FocusHandle, InteractiveElement,
    IntoElement, KeyboardButton, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use std::rc::Rc;

/// The value a click or Space asks for.
type ChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// Transparent: a mark that is not drawn.
const NONE: Color = Color::rgba8(0, 0, 0, 0.0);

/// The state the box shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Unchecked,
    Checked,
    Indeterminate,
}

impl Mark {
    /// Partly checked wins over checked: a parent with some children selected.
    fn of(checked: bool, indeterminate: bool) -> Self {
        if indeterminate {
            Self::Indeterminate
        } else if checked {
            Self::Checked
        } else {
            Self::Unchecked
        }
    }

    /// The glyph in the box: a tick when checked, a dash when partly checked.
    fn icon(self) -> Option<IconName> {
        match self {
            Self::Unchecked => None,
            Self::Checked => Some(IconName::Check),
            Self::Indeterminate => Some(IconName::Minus),
        }
    }
}

/// What the checkbox looks like right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visual {
    Rest,
    Hover,
    Disabled,
}

/// Solid colours of the box in one state (Checkbox.md, States).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Paint {
    fill: Color,
    border: Color,
    mark: Color,
}

fn paint(mark: Mark, visual: Visual, error: bool, p: &Palette) -> Paint {
    match (mark, visual) {
        (_, Visual::Disabled) => Paint {
            fill: p.fill.disabled,
            border: p.border.disabled,
            mark: p.text.disabled,
        },
        (Mark::Unchecked, Visual::Hover) => Paint {
            fill: p.fill.hover_raised,
            border: p.accent.fg,
            mark: NONE,
        },
        (Mark::Unchecked, Visual::Rest) if error => Paint {
            fill: p.surface.sunken,
            border: p.status.danger_fg,
            mark: NONE,
        },
        (Mark::Unchecked, Visual::Rest) => Paint {
            fill: p.surface.sunken,
            border: p.border.strong,
            mark: NONE,
        },
        (_, Visual::Hover) => Paint {
            fill: p.accent.hover,
            border: p.accent.hover,
            mark: p.text.on_accent,
        },
        (_, Visual::Rest) => Paint {
            fill: p.accent.r#default,
            border: p.accent.r#default,
            mark: p.text.on_accent,
        },
    }
}

/// Enter is not a checkbox key: only Space and pointer clicks toggle.
fn is_enter(event: &ClickEvent) -> bool {
    matches!(event, ClickEvent::Keyboard(key) if matches!(key.button, KeyboardButton::Enter))
}

/// The value an indeterminate parent sets on every child when clicked: clear them
/// all if any is checked, otherwise select them all (Checkbox.md, indeterminate).
pub fn indeterminate_click_target(children: &[bool]) -> bool {
    !children.iter().any(|checked| *checked)
}

/// An independent option. The caller keeps the value and updates it in `on_change`.
/// A partly checked parent sends `on_change` a value it should ignore: set its
/// children to `indeterminate_click_target` instead.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    indeterminate: bool,
    label: Option<SharedString>,
    helper: Option<SharedString>,
    error: Option<SharedString>,
    disabled: bool,
    preview: Option<StatePreview>,
    focus: Option<FocusHandle>,
    on_change: Option<ChangeHandler>,
}

impl std::fmt::Debug for Checkbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checkbox")
            .field("id", &self.id)
            .field("checked", &self.checked)
            .field("indeterminate", &self.indeterminate)
            .field("disabled", &self.disabled)
            .finish_non_exhaustive()
    }
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            indeterminate: false,
            label: None,
            helper: None,
            error: None,
            disabled: false,
            preview: None,
            focus: None,
            on_change: None,
        }
    }

    /// Partly checked: a parent whose children are only some selected.
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    /// The option's name, to the right of the box.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A line under the label that says what the option changes.
    pub fn helper(mut self, text: impl Into<SharedString>) -> Self {
        self.helper = Some(text.into());
        self
    }

    /// A failed choice: the box is outlined in danger and the line says why.
    pub fn error(mut self, text: impl Into<SharedString>) -> Self {
        self.error = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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

    /// The value a click asks for (a partly checked parent: see `indeterminate_click_target`).
    pub fn toggled(&self) -> bool {
        !self.checked
    }

    /// Whether a click or Space reaches `on_change`.
    pub fn activates(&self) -> bool {
        !self.disabled
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let checked = self.checked;
        let interactive = self.activates();
        let mark = Mark::of(checked, self.indeterminate);
        let error = self.error.is_some();
        let visual = match (self.disabled, self.preview) {
            (true, _) => Visual::Disabled,
            (false, Some(StatePreview::Hover)) => Visual::Hover,
            _ => Visual::Rest,
        };
        let shown = paint(mark, visual, error, p);
        let hover = if interactive {
            paint(mark, Visual::Hover, error, p)
        } else {
            shown
        };
        let focus = self.focus.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone()
        });
        let changes = window
            .use_keyed_state(self.id.clone(), cx, |_, _| ToggleMotion::new(checked))
            .update(cx, |motion, _| {
                motion.observe(checked);
                motion.changes
            });

        // The tick fades in on each change; before the first change it is simply drawn.
        let mark_glyph = mark.icon().map(|name| {
            let drawn = glyph(name, IconSize::XSmall, shown.mark, shown.mark);
            if changes == 0 {
                drawn.into_any_element()
            } else {
                drawn
                    .with_animation(
                        ("checkbox-mark", changes),
                        Animation::new(DURATION_FAST),
                        |mark, delta| mark.opacity(ease(EASE_STANDARD, delta)),
                    )
                    .into_any_element()
            }
        });
        let mut boxed = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .mt(px(SPACE_HALF))
            .size(px(SIZE_ICON_SM))
            .rounded(px(RADIUS_MD))
            .border_1()
            .border_color(shown.border)
            .bg(shown.fill)
            .children(mark_glyph);
        if interactive && self.preview.is_none() {
            boxed = boxed.group_hover(HOVER_GROUP, move |style| {
                style.bg(hover.fill).border_color(hover.border)
            });
        }

        let mut stack = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(SPACE_HALF));
        if let Some(label) = self.label {
            stack = stack.child(div().text_body().text_color(p.text.primary).child(label));
        }
        if let Some(error) = self.error {
            stack = stack.child(helper_line(
                error,
                p.status.danger_fg,
                Some(IconName::OctagonAlert),
            ));
        } else if let Some(helper) = self.helper {
            stack = stack.child(helper_line(helper, p.text.secondary, None));
        }

        let mut row = div()
            .id(self.id.clone())
            .group(HOVER_GROUP)
            .track_focus(&focus)
            .tab_stop(!self.disabled)
            .focus_ring(&theme)
            .flex()
            .items_start()
            .gap(px(SPACE_2))
            .min_h(px(SIZE_HIT_POINTER))
            .when(self.preview == Some(StatePreview::Focus), |el| {
                el.shadow(box_shadows(theme.shadows.focus))
            })
            .child(boxed)
            .child(stack);
        if interactive && self.preview.is_none() {
            row = row.cursor_pointer();
        }
        if let Some(on_change) = self.on_change.filter(|_| interactive) {
            let next = !checked;
            row = row.on_click(move |event: &ClickEvent, window, cx| {
                if !is_enter(event) {
                    on_change(next, window, cx);
                }
            });
        }
        row
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
    fn indeterminate_click_clears_every_child_when_one_is_checked_and_selects_all_otherwise() {
        assert!(!indeterminate_click_target(&[true, false]));
        assert!(!indeterminate_click_target(&[true, true]));
        assert!(indeterminate_click_target(&[false, false]));
        assert!(indeterminate_click_target(&[]));
    }

    #[test]
    fn the_mark_follows_the_state_and_partly_checked_wins_over_checked() {
        assert_eq!(Mark::of(false, false).icon(), None);
        assert_eq!(Mark::of(true, false).icon(), Some(IconName::Check));
        assert_eq!(Mark::of(false, true).icon(), Some(IconName::Minus));
        assert_eq!(Mark::of(true, true).icon(), Some(IconName::Minus));
    }

    #[test]
    fn unchecked_box_is_sunken_with_a_strong_border_and_an_accent_hover_outline() {
        let t = dark();
        let rest = paint(Mark::Unchecked, Visual::Rest, false, &t.palette);
        assert_eq!(rest.fill, t.palette.surface.sunken);
        assert_eq!(rest.border, t.palette.border.strong);
        let hover = paint(Mark::Unchecked, Visual::Hover, false, &t.palette);
        assert_eq!(hover.fill, t.palette.fill.hover_raised);
        assert_eq!(hover.border, t.palette.accent.fg);
    }

    #[test]
    fn checked_and_partly_checked_boxes_fill_with_accent_and_draw_on_accent() {
        let t = dark();
        for mark in [Mark::Checked, Mark::Indeterminate] {
            let rest = paint(mark, Visual::Rest, false, &t.palette);
            assert_eq!(rest.fill, t.palette.accent.r#default, "{mark:?}");
            assert_eq!(rest.mark, t.palette.text.on_accent, "{mark:?}");
            let hover = paint(mark, Visual::Hover, false, &t.palette);
            assert_eq!(hover.fill, t.palette.accent.hover, "{mark:?}");
        }
    }

    #[test]
    fn an_error_outlines_the_unchecked_box_in_danger() {
        let t = dark();
        let error = paint(Mark::Unchecked, Visual::Rest, true, &t.palette);
        assert_eq!(error.border, t.palette.status.danger_fg);
        assert_eq!(error.fill, t.palette.surface.sunken);
    }

    #[test]
    fn disabled_boxes_use_the_disabled_tokens_in_every_state() {
        let t = dark();
        for mark in [Mark::Unchecked, Mark::Checked, Mark::Indeterminate] {
            let p = paint(mark, Visual::Disabled, false, &t.palette);
            assert_eq!(p.fill, t.palette.fill.disabled);
            assert_eq!(p.border, t.palette.border.disabled);
            assert_eq!(p.mark, t.palette.text.disabled);
        }
    }

    #[test]
    fn a_click_asks_for_the_opposite_value_and_a_disabled_box_does_not_activate() {
        assert!(Checkbox::new("a", false).toggled());
        assert!(!Checkbox::new("a", true).toggled());
        assert!(Checkbox::new("a", false).activates());
        assert!(!Checkbox::new("a", false).disabled(true).activates());
    }

    /// Hosts one checkbox that records each value it asks for.
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
                    .debug_selector(|| "checkbox".into())
                    .child(
                        Checkbox::new("under-test", false)
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
        let bounds = cx.debug_bounds("checkbox").expect("checkbox is drawn");
        cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        assert_eq!(*asked.borrow(), vec![true]);
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "enter");
        assert_eq!(asked.borrow().len(), 1, "Enter does nothing on a checkbox");
        press_key(cx, "space");
        assert_eq!(*asked.borrow(), vec![true, true]);
    }

    #[gpui::test]
    fn disabled_checkbox_ignores_pointer_and_keys(cx: &mut gpui::TestAppContext) {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view(|_, cx| Harness::new(cx, asked.clone(), true));
        cx.run_until_parked();
        let bounds = cx.debug_bounds("checkbox").expect("checkbox is drawn");
        cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "space");
        assert!(asked.borrow().is_empty());
    }
}
