//! Radio group: one choice from two to five options, all visible at once
//! (docs/design/components/Radio.md).
//!
//! The group is one tab stop. Arrow keys move the selection to the next or previous enabled
//! option (selection follows the arrows) and a click on a circle or its label selects it.

use crate::components::button::HOVER_GROUP;
use crate::components::switch::helper_line;
use crate::icon::IconName;
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, Palette, RADIUS_FULL, SIZE_HIT_POINTER, SIZE_ICON_SM, SPACE_2, SPACE_HALF,
};
use gpui::{
    App, ClickEvent, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    px,
};
use std::fmt;
use std::rc::Rc;

/// The dot inside a selected circle (Radio.md, anatomy).
const DOT: f32 = 8.0;

/// The value an arrow key or a click asks for.
type ChangeHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Transparent: a dot that is not drawn.
const NONE: Color = Color::rgba8(0, 0, 0, 0.0);

/// One option of a group: its name, an optional line of explanation, and whether it can be chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioOption {
    pub label: SharedString,
    pub helper: Option<SharedString>,
    pub disabled: bool,
}

impl RadioOption {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            helper: None,
            disabled: false,
        }
    }

    /// A line under the label that says what choosing the option does.
    pub fn helper(mut self, text: impl Into<SharedString>) -> Self {
        self.helper = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The index an arrow key selects from `selected`, skipping disabled options. Down and Right
/// move forward, Up and Left move back; the selection does not wrap. With nothing selected,
/// Down takes the first enabled option and Up the last.
pub fn arrow_target(enabled: &[bool], selected: Option<usize>, key: &str) -> Option<usize> {
    let step: isize = match key {
        "down" | "right" => 1,
        "up" | "left" => -1,
        _ => return None,
    };
    let mut index = match selected {
        Some(index) => isize::try_from(index).ok()? + step,
        None if step > 0 => 0,
        None => isize::try_from(enabled.len()).ok()? - 1,
    };
    while (0..enabled.len() as isize).contains(&index) {
        if enabled[index as usize] {
            return usize::try_from(index).ok();
        }
        index += step;
    }
    None
}

/// The state a circle is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visual {
    Rest,
    Hover,
    Disabled,
}

/// Solid colours of one circle in one state (Radio.md, States).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Paint {
    fill: Color,
    border: Color,
    dot: Color,
}

fn paint(selected: bool, visual: Visual, error: bool, p: &Palette) -> Paint {
    match (selected, visual) {
        (_, Visual::Disabled) => Paint {
            fill: p.fill.disabled,
            border: p.border.disabled,
            dot: p.text.disabled,
        },
        (false, Visual::Rest) => Paint {
            fill: p.surface.sunken,
            border: if error {
                p.status.danger_fg
            } else {
                p.border.strong
            },
            dot: NONE,
        },
        (false, Visual::Hover) => Paint {
            fill: p.fill.hover_raised,
            border: if error {
                p.status.danger_fg
            } else {
                p.accent.fg
            },
            dot: NONE,
        },
        (true, Visual::Rest) => Paint {
            fill: p.surface.sunken,
            border: if error {
                p.status.danger_fg
            } else {
                p.accent.r#default
            },
            dot: p.accent.r#default,
        },
        (true, Visual::Hover) => Paint {
            fill: p.fill.hover_raised,
            border: if error {
                p.status.danger_fg
            } else {
                p.accent.hover
            },
            dot: p.accent.hover,
        },
    }
}

/// A set of options from which exactly one is chosen. The caller keeps the selection and
/// updates it in `on_change`.
#[derive(IntoElement)]
pub struct RadioGroup {
    id: SharedString,
    options: Vec<RadioOption>,
    selected: Option<usize>,
    error: Option<SharedString>,
    disabled: bool,
    focus: Option<FocusHandle>,
    on_change: Option<ChangeHandler>,
}

impl fmt::Debug for RadioGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RadioGroup")
            .field("id", &self.id)
            .field("options", &self.options)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .finish_non_exhaustive()
    }
}

impl RadioGroup {
    pub fn new(id: impl Into<SharedString>, options: Vec<RadioOption>) -> Self {
        Self {
            id: id.into(),
            options,
            selected: None,
            error: None,
            disabled: false,
            focus: None,
            on_change: None,
        }
    }

    /// The option chosen now, if any.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    /// A failed choice: the circles are outlined in danger and the line under the group says why.
    pub fn error(mut self, text: impl Into<SharedString>) -> Self {
        self.error = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Keyboard focus lives with the caller; without one the group keeps its own.
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }

    /// Called with the index of the option a click or an arrow key chooses.
    pub fn on_change(mut self, listener: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(listener));
        self
    }

    /// The options a key or click can reach: disabled ones never are.
    pub fn enabled(&self) -> Vec<bool> {
        self.options
            .iter()
            .map(|option| !self.disabled && !option.disabled)
            .collect()
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let enabled = self.enabled();
        let error = self.error.is_some();
        let element_id: ElementId = self.id.clone().into();
        // GPUI reads a tracked handle's Tab stop from the handle, not the element.
        let focus = self
            .focus
            .clone()
            .unwrap_or_else(|| {
                window
                    .use_keyed_state(element_id.clone(), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone()
            })
            .tab_stop(enabled.iter().any(|on| *on));

        let mut options = div().flex().flex_col().gap(px(SPACE_2));
        for (index, option) in self.options.iter().enumerate() {
            let chosen = self.selected == Some(index);
            let visual = if !enabled[index] {
                Visual::Disabled
            } else {
                Visual::Rest
            };
            let shown = paint(chosen, visual, error, p);
            let hover = paint(chosen, Visual::Hover, error, p);
            let mut circle = div()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .mt(px(SPACE_HALF))
                .size(px(SIZE_ICON_SM))
                .rounded(px(RADIUS_FULL))
                .border_1()
                .border_color(shown.border)
                .bg(shown.fill);
            if chosen {
                circle = circle.child(div().size(px(DOT)).rounded(px(RADIUS_FULL)).bg(shown.dot));
            }
            if enabled[index] {
                circle = circle.group_hover(HOVER_GROUP, move |style| {
                    style.bg(hover.fill).border_color(hover.border)
                });
            }

            let mut stack = div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(SPACE_HALF))
                .child(
                    div()
                        .text_body()
                        .text_color(if enabled[index] {
                            p.text.primary
                        } else {
                            p.text.secondary
                        })
                        .child(option.label.clone()),
                );
            if let Some(helper) = option.helper.clone() {
                stack = stack.child(helper_line(helper, p.text.secondary, None));
            }

            let option_id = SharedString::from(format!("{}-option-{index}", self.id));
            let mut row = div()
                .id(option_id.clone())
                .group(HOVER_GROUP)
                .role(Role::RadioButton)
                .aria_label(option.label.clone())
                .aria_selected(chosen)
                .debug_selector(move || format!("radio-option-{index}"))
                .flex()
                .items_start()
                .gap(px(SPACE_2))
                .min_h(px(SIZE_HIT_POINTER))
                .child(circle)
                .child(stack);
            if enabled[index] {
                row = row.cursor_pointer();
                if let Some(on_change) = self.on_change.clone().filter(|_| !chosen) {
                    row = row.on_click(move |_: &ClickEvent, window, cx| {
                        on_change(index, window, cx);
                    });
                }
            }
            options = options.child(row);
        }

        let mut group = div()
            .id(element_id)
            .role(Role::RadioGroup)
            .track_focus(&focus)
            .focus_ring(&theme)
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .child(options);
        if let Some(on_change) = self.on_change.filter(|_| enabled.iter().any(|on| *on)) {
            let selected = self.selected;
            group = group.on_key_down(move |event: &KeyDownEvent, window, cx| {
                if let Some(next) = arrow_target(&enabled, selected, &event.keystroke.key) {
                    on_change(next, window, cx);
                }
            });
        }
        if let Some(error) = self.error {
            group = group.child(helper_line(
                error,
                p.status.danger_fg,
                Some(IconName::OctagonAlert),
            ));
        }
        group
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::press_key;
    use crate::theme::Theme;
    use crate::tokens::Appearance;
    use gpui::{Render, VisualTestContext};
    use std::cell::{Cell, RefCell};

    fn dark() -> Theme {
        Theme::new(Appearance::Dark)
    }

    #[test]
    fn arrows_move_to_the_neighbour_and_skip_disabled_options() {
        let enabled = [true, false, true];
        assert_eq!(arrow_target(&enabled, Some(0), "down"), Some(2));
        assert_eq!(arrow_target(&enabled, Some(2), "up"), Some(0));
        assert_eq!(arrow_target(&enabled, Some(2), "down"), None, "no wrap");
        assert_eq!(arrow_target(&enabled, Some(0), "up"), None, "no wrap");
        assert_eq!(arrow_target(&enabled, Some(0), "enter"), None);
    }

    #[test]
    fn with_nothing_selected_down_takes_the_first_and_up_the_last_enabled_option() {
        let enabled = [false, true, true];
        assert_eq!(arrow_target(&enabled, None, "down"), Some(1));
        assert_eq!(arrow_target(&enabled, None, "up"), Some(2));
        assert_eq!(arrow_target(&[false, false], None, "down"), None);
    }

    #[test]
    fn unselected_circle_is_sunken_with_a_strong_border_and_a_selected_one_has_an_accent_dot() {
        let t = dark();
        let unselected = paint(false, Visual::Rest, false, &t.palette);
        assert_eq!(unselected.fill, t.palette.surface.sunken);
        assert_eq!(unselected.border, t.palette.border.strong);
        assert_eq!(unselected.dot, NONE);
        let selected = paint(true, Visual::Rest, false, &t.palette);
        assert_eq!(selected.border, t.palette.accent.r#default);
        assert_eq!(selected.dot, t.palette.accent.r#default);
        let hover = paint(false, Visual::Hover, false, &t.palette);
        assert_eq!(hover.border, t.palette.accent.fg);
    }

    #[test]
    fn disabled_and_error_circles_use_their_own_tokens() {
        let t = dark();
        for chosen in [false, true] {
            let disabled = paint(chosen, Visual::Disabled, false, &t.palette);
            assert_eq!(disabled.fill, t.palette.fill.disabled);
            assert_eq!(disabled.border, t.palette.border.disabled);
            assert_eq!(disabled.dot, t.palette.text.disabled);
            let error = paint(chosen, Visual::Rest, true, &t.palette);
            assert_eq!(error.border, t.palette.status.danger_fg, "chosen {chosen}");
        }
    }

    #[test]
    fn a_group_with_every_option_disabled_has_nothing_to_choose() {
        let options = vec![RadioOption::new("a"), RadioOption::new("b")];
        let group = RadioGroup::new("g", options.clone());
        assert_eq!(group.enabled(), vec![true, true]);
        assert_eq!(group.disabled(true).enabled(), vec![false, false]);
        let mixed = RadioGroup::new("g", vec![RadioOption::new("a").disabled(true)]);
        assert_eq!(mixed.enabled(), vec![false]);
    }

    /// Hosts one group that records each index it asks for and keeps the choice it was given.
    struct Harness {
        focus: FocusHandle,
        asked: Rc<RefCell<Vec<usize>>>,
        chosen: Rc<Cell<Option<usize>>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let (asked, chosen) = (self.asked.clone(), self.chosen.clone());
            let options = vec![
                RadioOption::new("System"),
                RadioOption::new("Dark").helper("The default look."),
                RadioOption::new("Light").disabled(true),
            ];
            div().child(
                RadioGroup::new("theme", options)
                    .selected(self.chosen.get())
                    .focus_handle(self.focus.clone())
                    .on_change(move |index, window, _| {
                        asked.borrow_mut().push(index);
                        chosen.set(Some(index));
                        window.refresh();
                    }),
            )
        }
    }

    fn harness(
        cx: &mut gpui::TestAppContext,
        chosen: Option<usize>,
    ) -> (
        gpui::Entity<Harness>,
        &mut VisualTestContext,
        Rc<RefCell<Vec<usize>>>,
    ) {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let (shared, chosen) = (asked.clone(), Rc::new(Cell::new(chosen)));
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            focus: cx.focus_handle(),
            asked: shared,
            chosen,
        });
        cx.run_until_parked();
        (view, cx, asked)
    }

    #[gpui::test]
    fn a_click_on_the_chosen_option_asks_for_nothing_and_another_option_asks_for_it(
        cx: &mut gpui::TestAppContext,
    ) {
        let (_view, cx, asked) = harness(cx, Some(0));
        let system = cx
            .debug_bounds("radio-option-0")
            .expect("the System option is drawn");
        cx.simulate_click(system.center(), gpui::Modifiers::none());
        assert!(asked.borrow().is_empty(), "System is already chosen");
        let dark = cx
            .debug_bounds("radio-option-1")
            .expect("the Dark option is drawn");
        cx.simulate_click(dark.center(), gpui::Modifiers::none());
        assert_eq!(*asked.borrow(), vec![1]);
    }

    #[gpui::test]
    fn arrow_keys_on_the_focused_group_move_the_choice_and_skip_the_disabled_option(
        cx: &mut gpui::TestAppContext,
    ) {
        let (_view, cx, asked) = harness(cx, Some(0));
        // The group is the only tab stop, so Tab focuses it; draw again so its keys are registered.
        cx.update(|window, cx| window.focus_next(cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        press_key(cx, "down");
        assert_eq!(*asked.borrow(), vec![1]);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        press_key(cx, "down");
        assert_eq!(
            *asked.borrow(),
            vec![1],
            "Light is disabled, so nothing moves"
        );
    }
}
