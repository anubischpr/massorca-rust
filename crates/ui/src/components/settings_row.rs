//! Settings row and section: one setting with its label, helper line and control, grouped with
//! its neighbours on a settings page (docs/design/components/SettingsRow.md).
//!
//! The caller builds the control (a Switch, TextField, Select, RadioGroup or Button) and hands it
//! over. The row lays it out, offers a reset when asked and says what the change is doing; it never
//! holds a value of its own.

use std::fmt;
use std::rc::Rc;

use crate::a11y;
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::components::progress::Spinner;
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled, Theme, box_shadows};
use crate::tokens::{
    LAYOUT_SETTINGS_CONTROL_MAX, LAYOUT_SETTINGS_CONTROL_MIN, RADIUS_FULL, RADIUS_LG, RADIUS_MD,
    SIZE_CONTROL_DEFAULT, SIZE_ICON_MD, SIZE_ROW_SETTINGS, SIZE_ROW_SETTINGS_HELPER,
    SIZE_SWITCH_KNOB, SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_HALF, TYPE_BODY_SM, TYPE_CAPTION,
    TYPE_TITLE_SM,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};

/// What a row shows under its control while a change is in hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowStatus {
    /// The service has the change; the spinner appears only after a short delay.
    Saving,
    /// The service confirmed the change. The caller clears the status after two seconds.
    Saved,
    /// The change was refused. The message says why, and the previous value stays in effect.
    Error(SharedString),
    /// The setting has no backing yet. The message says what is missing; no glyph, so it never
    /// reads as a failure.
    Unavailable(SharedString),
}

impl RowStatus {
    /// The words the row shows for this status.
    pub fn text(&self) -> SharedString {
        match self {
            Self::Saving => "Saving".into(),
            Self::Saved => "Saved".into(),
            Self::Error(message) | Self::Unavailable(message) => message.clone(),
        }
    }
}

/// The height a row takes at least: 48 px with a helper line, 40 px with a label alone.
pub fn row_min_height(has_helper: bool) -> f32 {
    if has_helper {
        SIZE_ROW_SETTINGS_HELPER
    } else {
        SIZE_ROW_SETTINGS
    }
}

/// Called when the reset button is pressed.
type ResetHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One setting: the label on the left, the control on the right, and a status line below.
#[derive(IntoElement)]
pub struct SettingsRow {
    id: SharedString,
    label: SharedString,
    helper: Option<SharedString>,
    disabled_reason: Option<SharedString>,
    control: Option<AnyElement>,
    reset: Option<ResetHandler>,
    status: Option<RowStatus>,
    stacked: bool,
    selected: bool,
}

impl fmt::Debug for SettingsRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SettingsRow")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("status", &self.status)
            .field("stacked", &self.stacked)
            .finish_non_exhaustive()
    }
}

impl SettingsRow {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            helper: None,
            disabled_reason: None,
            control: None,
            reset: None,
            status: None,
            stacked: false,
            selected: false,
        }
    }

    /// What the setting does, under the label.
    pub fn helper(mut self, text: impl Into<SharedString>) -> Self {
        self.helper = Some(text.into());
        self
    }

    /// Why the control cannot be used. It replaces the helper and dims the label.
    pub fn disabled_reason(mut self, text: impl Into<SharedString>) -> Self {
        self.disabled_reason = Some(text.into());
        self
    }

    /// The control: a Switch, TextField, Select, RadioGroup or Button.
    pub fn control(mut self, element: impl IntoElement) -> Self {
        self.control = Some(element.into_any_element());
        self
    }

    /// Shows a reset button beside the control; the caller offers it only when the value differs
    /// from the default.
    pub fn reset(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.reset = Some(Rc::new(listener));
        self
    }

    pub fn status(mut self, status: RowStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// The label above the control, full width, for narrow pages (below 560 px).
    pub fn stacked(mut self, stacked: bool) -> Self {
        self.stacked = stacked;
        self
    }

    /// Outlines the row after a search jumps to it.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for SettingsRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let disabled = self.disabled_reason.is_some();
        let helper = self.disabled_reason.clone().or(self.helper.clone());
        let described = helper.clone();
        let row_id: ElementId = self.id.clone().into();

        let mut label_column = div().flex().flex_col().gap(px(SPACE_HALF)).min_w_0().child(
            div()
                .text_body()
                .text_color(if disabled {
                    p.text.secondary
                } else {
                    p.text.primary
                })
                .child(self.label.clone()),
        );
        if let Some(text) = helper {
            label_column = label_column.child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(if disabled {
                        p.text.tertiary
                    } else {
                        p.text.secondary
                    })
                    .line_clamp(3)
                    .child(text),
            );
        }

        let mut control_slot = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_end()
            .gap(px(SPACE_2))
            .min_w(px(LAYOUT_SETTINGS_CONTROL_MIN))
            .max_w(px(LAYOUT_SETTINGS_CONTROL_MAX));
        if let Some(on_reset) = self.reset {
            let reset_name = format!("Reset {} to default", self.label);
            control_slot = control_slot.child(
                IconButton::new(
                    SharedString::from(format!("{}-reset", self.id)),
                    IconName::RotateCcw,
                    reset_name,
                )
                .variant(IconButtonVariant::Ghost)
                .on_click(move |window, cx| on_reset(window, cx)),
            );
        }
        if let Some(control) = self.control {
            control_slot = control_slot.child(control);
        }

        let body = if self.stacked {
            div()
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .child(label_column)
                .child(control_slot.w_full().max_w_full().min_w_0().justify_start())
        } else {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(SPACE_4))
                .child(label_column.flex_1())
                .child(control_slot)
        };

        let mut row = div()
            .id(row_id)
            .role(Role::Group)
            .aria_label(self.label.clone())
            .min_h(px(row_min_height(self.helper.is_some() || disabled)))
            .px(px(SPACE_4))
            .py(px(SPACE_3))
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .child(body)
            .when(self.selected, |el| el.bg(p.accent.subtle_raised));
        if let Some(status) = self.status {
            row = row.child(status_line(&self.id, &status, &theme));
        }
        if let Some(text) = described {
            row = row.aria_description(text);
        }
        row
    }
}

/// The status under the control: a spinner while saving, a check when saved, a glyph and the
/// message when refused.
fn status_line(id: &SharedString, status: &RowStatus, theme: &crate::theme::Theme) -> AnyElement {
    let p = &theme.palette;
    let (color, glyph): (_, Option<AnyElement>) = match status {
        RowStatus::Saving => (
            p.text.tertiary,
            Some(
                Spinner::new()
                    .size(IconSize::XSmall)
                    .color(p.text.tertiary)
                    .delayed(SharedString::from(format!("{id}-saving")))
                    .into_any_element(),
            ),
        ),
        RowStatus::Saved => (
            p.status.success_fg,
            Some(
                Icon::new(IconName::Check)
                    .size(IconSize::XSmall)
                    .color(p.status.success_fg)
                    .into_any_element(),
            ),
        ),
        RowStatus::Error(_) => (
            p.status.danger_fg,
            Some(
                Icon::new(IconName::OctagonAlert)
                    .size(IconSize::XSmall)
                    .color(p.status.danger_fg)
                    .into_any_element(),
            ),
        ),
        RowStatus::Unavailable(_) => (p.text.tertiary, None),
    };
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .text_style(TYPE_BODY_SM)
        .text_color(color)
        .children(glyph)
        .child(status.text())
        .into_any_element()
}

/// A text field that cannot be typed in yet. It shows `value`, or nothing.
pub fn unavailable_input(
    id: SharedString,
    name: SharedString,
    theme: &Theme,
    value: Option<&str>,
) -> AnyElement {
    let text = SharedString::from(value.unwrap_or_default().to_owned());
    disabled_box(id, name, Role::TextInput, theme, text, None)
}

/// A dropdown that cannot be opened yet. It shows its value, or "Choose" until one is set.
pub fn unavailable_select(
    id: SharedString,
    name: SharedString,
    theme: &Theme,
    value: Option<&str>,
) -> AnyElement {
    let text = SharedString::from(value.unwrap_or("Choose").to_owned());
    disabled_box(
        id,
        name,
        Role::ComboBox,
        theme,
        text,
        Some(IconName::ChevronDown),
    )
}

/// A number field that cannot be changed yet: the value, a decrease and an increase button, and
/// the unit beside them.
pub fn unavailable_stepper(
    id: SharedString,
    name: SharedString,
    theme: &Theme,
    value: Option<&str>,
    unit: &'static str,
) -> AnyElement {
    let p = &theme.palette;
    let step = |icon: IconName, direction: &str| {
        IconButton::new(
            SharedString::from(format!("{id}-{direction}")),
            icon,
            format!("{direction} {name}"),
        )
        .variant(IconButtonVariant::Ghost)
        .disabled(true)
    };
    let text = SharedString::from(value.unwrap_or_default().to_owned());
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .w_full()
        .child(step(IconName::Minus, "Decrease"))
        .child(disabled_box(
            id.clone(),
            name.clone(),
            Role::SpinButton,
            theme,
            text,
            None,
        ))
        .child(step(IconName::Plus, "Increase"))
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(p.text.tertiary)
                .child(unit),
        )
        .into_any_element()
}

/// A slider that cannot be moved yet: its track, the knob at `position` (0 to 1) and its value.
pub fn unavailable_slider(
    id: SharedString,
    name: SharedString,
    theme: &Theme,
    value: &'static str,
    position: f32,
) -> AnyElement {
    let p = &theme.palette;
    let knob = div()
        .absolute()
        .top(px((SPACE_1 - SIZE_SWITCH_KNOB) / 2.0))
        .left(relative(position.clamp(0.0, 1.0)))
        .size(px(SIZE_SWITCH_KNOB))
        .rounded(px(RADIUS_FULL))
        .bg(p.text.disabled);
    let track = div()
        .id(id)
        .role(Role::Slider)
        .aria_label(name)
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(SPACE_1))
        .rounded(px(RADIUS_FULL))
        .bg(p.fill.disabled)
        .child(knob);
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_3))
        .w_full()
        .child(a11y::disabled(track, true))
        .child(div().text_body().text_color(p.text.tertiary).child(value))
        .into_any_element()
}

/// Accent swatches that cannot be picked yet: `count` neutral disks, the first (the default) ringed.
pub fn unavailable_swatches(theme: &Theme, count: usize) -> AnyElement {
    let p = &theme.palette;
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .children((0..count).map(|index| {
            let swatch = div()
                .size(px(SIZE_ICON_MD))
                .rounded(px(RADIUS_FULL))
                .bg(p.fill.disabled)
                .border_1()
                .border_color(p.border.strong);
            if index == 0 {
                swatch.border_2().border_color(p.text.tertiary)
            } else {
                swatch
            }
        }))
        .into_any_element()
}

/// The header of a table whose rows are not available yet. The first column takes the spare width.
pub fn unavailable_table_header(theme: &Theme, columns: &[&'static str]) -> AnyElement {
    let p = &theme.palette;
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_4))
        .w_full()
        .pb(px(SPACE_1))
        .border_b_1()
        .border_color(p.border.subtle)
        .children(columns.iter().enumerate().map(|(index, column)| {
            let cell = div()
                .text_style(TYPE_CAPTION)
                .text_color(p.text.tertiary)
                .child(*column);
            if index == 0 {
                cell.flex_1().min_w_0()
            } else {
                cell.w(px(LAYOUT_SETTINGS_CONTROL_MIN))
            }
        }))
        .into_any_element()
}

/// A field or dropdown box in the disabled tokens. `chevron` marks a dropdown.
fn disabled_box(
    id: SharedString,
    name: SharedString,
    role: Role,
    theme: &Theme,
    text: SharedString,
    chevron: Option<IconName>,
) -> AnyElement {
    let p = &theme.palette;
    let mut field = div()
        .id(id)
        .role(role)
        .aria_label(name)
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .justify_between()
        .gap(px(SPACE_2))
        .h(px(SIZE_CONTROL_DEFAULT))
        .px(px(SPACE_3))
        .rounded(px(RADIUS_MD))
        .border_1()
        .border_color(p.border.subtle)
        .bg(p.fill.disabled)
        .text_body()
        .text_color(p.text.disabled)
        .child(div().min_w_0().truncate().child(text));
    if let Some(icon) = chevron {
        field = field.child(
            Icon::new(icon)
                .size(IconSize::XSmall)
                .color(p.text.disabled),
        );
    }
    a11y::disabled(field, true).into_any_element()
}

/// A group of rows under a title (SettingsRow.md, Section).
#[derive(IntoElement)]
pub struct SettingsSection {
    id: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    rows: Vec<AnyElement>,
}

impl fmt::Debug for SettingsSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SettingsSection")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("rows", &self.rows.len())
            .finish_non_exhaustive()
    }
}

impl SettingsSection {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            rows: Vec::new(),
        }
    }

    /// Up to two lines under the title.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn row(mut self, row: impl IntoElement) -> Self {
        self.rows.push(row.into_any_element());
        self
    }
}

impl RenderOnce for SettingsSection {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let count = self.rows.len();
        let mut group = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .bg(p.surface.raised)
            .border_1()
            .border_color(p.border.default)
            .rounded(px(RADIUS_LG))
            .shadow(box_shadows(theme.shadows.e1));
        for (index, row) in self.rows.into_iter().enumerate() {
            let mut cell = div().w_full().child(row);
            if index + 1 < count {
                cell = cell.border_b_1().border_color(p.border.subtle);
            }
            group = group.child(cell);
        }
        div()
            .flex()
            .flex_col()
            .gap(px(SPACE_3))
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(SPACE_1))
                    .child(
                        div()
                            .text_style(TYPE_TITLE_SM)
                            .text_color(p.text.primary)
                            .child(self.title),
                    )
                    .when_some(self.description, |el, text| {
                        el.child(
                            div()
                                .text_style(TYPE_BODY_SM)
                                .text_color(p.text.secondary)
                                .line_clamp(2)
                                .child(text),
                        )
                    }),
            )
            .child(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::switch::Switch;
    use crate::theme::Theme;
    use crate::tokens::Appearance;
    use gpui::{Modifiers, Render, VisualTestContext};
    use std::cell::Cell;

    #[test]
    fn each_status_says_its_own_words() {
        assert_eq!(RowStatus::Saving.text(), "Saving");
        assert_eq!(RowStatus::Saved.text(), "Saved");
        assert_eq!(
            RowStatus::Error("Enter a folder path.".into()).text(),
            "Enter a folder path."
        );
        assert_eq!(
            RowStatus::Unavailable("Not available yet".into()).text(),
            "Not available yet"
        );
    }

    #[test]
    fn a_row_with_a_helper_is_the_taller_one() {
        assert_eq!(row_min_height(true), SIZE_ROW_SETTINGS_HELPER);
        assert_eq!(row_min_height(false), SIZE_ROW_SETTINGS);
        assert!(row_min_height(true) > row_min_height(false));
    }

    /// Hosts one row whose reset button counts its presses. The row and its switch are wrapped
    /// so the test can find them; the component itself draws no test selectors.
    struct Harness {
        resets: Rc<Cell<u32>>,
        stacked: bool,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let resets = self.resets.clone();
            div().w(px(640.)).child(
                div().debug_selector(|| "row".into()).child(
                    SettingsRow::new("row", "Show usage")
                        .helper("Counts the agents you run.")
                        .control(
                            div()
                                .debug_selector(|| "row-switch".into())
                                .child(Switch::new("row-toggle", true)),
                        )
                        .reset(move |_, _| resets.set(resets.get() + 1))
                        .status(RowStatus::Saved)
                        .stacked(self.stacked),
                ),
            )
        }
    }

    fn harness(
        cx: &mut gpui::TestAppContext,
        stacked: bool,
    ) -> (gpui::Entity<Harness>, &mut VisualTestContext, Rc<Cell<u32>>) {
        cx.update(|cx| Theme::set(Appearance::Dark, cx));
        let resets = Rc::new(Cell::new(0));
        let shared = resets.clone();
        let (view, cx) = cx.add_window_view(|_, _| Harness {
            resets: shared,
            stacked,
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (view, cx, resets)
    }

    #[gpui::test]
    fn the_reset_button_sits_left_of_the_control_and_fires_once(cx: &mut gpui::TestAppContext) {
        let (_view, cx, resets) = harness(cx, false);
        let switch = cx.debug_bounds("row-switch").expect("the control is drawn");
        // The reset button ends one gap (8 px) left of the switch; a point 12 px left of the
        // switch lies inside it whatever its exact width.
        let on_reset = gpui::point(switch.origin.x - gpui::px(12.), switch.center().y);
        cx.simulate_click(on_reset, Modifiers::none());
        assert_eq!(resets.get(), 1);
    }

    #[gpui::test]
    fn a_wide_row_puts_the_control_beside_the_label(cx: &mut gpui::TestAppContext) {
        let (_view, cx, _) = harness(cx, false);
        let row = cx.debug_bounds("row").expect("the row is drawn");
        let control = cx.debug_bounds("row-switch").expect("the control is drawn");
        assert!(
            control.origin.x > row.origin.x + row.size.width / 2.0,
            "right of the label"
        );
        assert!(
            control.origin.y < row.origin.y + row.size.height / 2.0,
            "level with the label"
        );
    }

    #[gpui::test]
    fn a_stacked_row_puts_the_control_below_the_label(cx: &mut gpui::TestAppContext) {
        let (_view, cx, _) = harness(cx, true);
        let row = cx.debug_bounds("row").expect("the row is drawn");
        let control = cx.debug_bounds("row-switch").expect("the control is drawn");
        assert!(
            control.origin.y > row.origin.y + row.size.height / 2.0,
            "under the label"
        );
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn a_settings_row_is_a_group_named_by_its_label_and_described_by_its_helper(
        cx: &mut TestAppContext,
    ) {
        let node = root(cx, |window, cx| {
            reported(
                SettingsRow::new("row", "Show usage")
                    .helper("Counts the agents you run.")
                    .control(div())
                    .render(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.role(), Role::Group);
        assert_eq!(node.label(), Some("Show usage"));
        assert_eq!(node.description(), Some("Counts the agents you run."));
    }

    #[gpui::test]
    fn a_row_that_is_off_is_described_by_the_reason(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                SettingsRow::new("row", "Usage data")
                    .helper("Counts what you use.")
                    .disabled_reason("Not available yet")
                    .control(div())
                    .render(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.description(), Some("Not available yet"));
    }
}
