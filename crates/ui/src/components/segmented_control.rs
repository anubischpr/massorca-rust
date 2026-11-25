//! SegmentedControl: two to four mutually exclusive options, joined as segments
//! (docs/design/components/SegmentedControl.md).
//!
//! The group is one tab stop. Left, Up, Right and Down move focus and select at
//! once, wrapping at the ends and skipping disabled segments; Home and End go to
//! the first and last enabled segment. Enter and Space select the focused segment
//! through the click they produce.

use crate::a11y;
use crate::components::agent_icon::NameTooltip;
use crate::components::button::StatePreview;
use crate::components::tooltip::{Tooltip, tooltip_view};
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, DURATION_TOOLTIP_DELAY, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM,
    SIZE_CONTROL_COMFORTABLE, SIZE_CONTROL_COMPACT, SIZE_CONTROL_DEFAULT, SIZE_ICON_SM, SPACE_1,
    SPACE_2, SPACE_HALF, TYPE_BODY, TYPE_LABEL, TextStyleToken,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, ClickEvent, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Toggled, Window, div, px,
};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// The narrowest a segment gets (a literal in SegmentedControl.md); a longer label widens its segment.
const SEGMENT_MIN_WIDTH: f32 = 64.0;

/// Transparent: a segment at rest shows its track.
const NONE: Color = Color::rgba8(0, 0, 0, 0.0);

/// The two looks of the control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentedStyle {
    /// A 1 px outline on a raised track, for forms, dialogs and panels.
    #[default]
    Outlined,
    /// No outline on a sunken track; the selected fill identifies it (A30, A31).
    Sunken,
}

/// The track height and label size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentedSize {
    Compact,
    #[default]
    Default,
    Comfortable,
}

impl SegmentedSize {
    fn track(self) -> f32 {
        match self {
            Self::Compact => SIZE_CONTROL_COMPACT,
            Self::Default => SIZE_CONTROL_DEFAULT,
            Self::Comfortable => SIZE_CONTROL_COMFORTABLE,
        }
    }

    fn label(self) -> TextStyleToken {
        match self {
            Self::Comfortable => TYPE_BODY,
            Self::Compact | Self::Default => TYPE_LABEL,
        }
    }
}

/// The segment a key moves the selection to from `current`. Left and Up go back,
/// Right and Down go on; both wrap and skip disabled segments. Home and End go to
/// the first and last enabled segment. `None` when the key is not a move or no
/// other segment is enabled.
pub fn segment_key(current: usize, enabled: &[bool], key: &str) -> Option<usize> {
    let count = enabled.len();
    let step: isize = match key {
        "left" | "up" => -1,
        "right" | "down" => 1,
        "home" => return enabled.iter().position(|&on| on),
        "end" => return enabled.iter().rposition(|&on| on),
        _ => return None,
    };
    let mut index = current.min(count.checked_sub(1)?);
    for _ in 1..count {
        index = (index as isize + step).rem_euclid(count as isize) as usize;
        if enabled[index] {
            return Some(index);
        }
    }
    None
}

/// The segment that takes the group's Tab stop: the selected one when it is enabled,
/// otherwise the first enabled one.
pub fn tab_segment(selected: usize, enabled: &[bool]) -> Option<usize> {
    match enabled.get(selected) {
        Some(true) => Some(selected),
        _ => enabled.iter().position(|&on| on),
    }
}

/// What a segment shows: at rest, under the pointer, or pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visual {
    Rest,
    Hover,
    Pressed,
}

/// The solid fill and label of one segment (handoff 2.1).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Paint {
    fill: Color,
    label: Color,
}

/// A selected segment keeps its fill under the pointer; disabled wins over everything.
fn paint(p: &Palette, selected: bool, strong: bool, disabled: bool, visual: Visual) -> Paint {
    if disabled {
        let fill = if selected {
            p.fill.selected_raised
        } else {
            NONE
        };
        return Paint {
            fill,
            label: p.text.disabled,
        };
    }
    match (selected, strong, visual) {
        (true, true, _) => Paint {
            fill: p.accent.r#default,
            label: p.text.on_accent,
        },
        (true, false, _) => Paint {
            fill: p.fill.selected_raised,
            label: p.text.primary,
        },
        (false, _, Visual::Rest) => Paint {
            fill: NONE,
            label: p.text.secondary,
        },
        (false, _, Visual::Hover) => Paint {
            fill: p.fill.hover_raised,
            label: p.text.primary,
        },
        (false, _, Visual::Pressed) => Paint {
            fill: p.fill.pressed_raised,
            label: p.text.primary,
        },
    }
}

/// The track fill: the raised surface when outlined, the sunken one otherwise, and the
/// disabled fill for a disabled control.
fn track_fill(p: &Palette, style: SegmentedStyle, disabled: bool) -> Color {
    match (disabled, style) {
        (true, _) => p.fill.disabled,
        (false, SegmentedStyle::Outlined) => p.surface.raised,
        (false, SegmentedStyle::Sunken) => p.surface.sunken,
    }
}

/// One option: its name, which is its accessible name and tooltip, and what it shows.
#[derive(Clone, Debug)]
pub struct Segment {
    name: SharedString,
    label: Option<SharedString>,
    icon: Option<IconName>,
    disabled: bool,
    tooltip: Option<SharedString>,
}

impl Segment {
    /// A text option; its label is also its name.
    pub fn text(label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            name: label.clone(),
            label: Some(label),
            icon: None,
            disabled: false,
            tooltip: None,
        }
    }

    /// An icon with a label, for an icon that adds meaning.
    pub fn icon_text(icon: IconName, label: impl Into<SharedString>) -> Self {
        Self {
            icon: Some(icon),
            ..Self::text(label)
        }
    }

    /// An icon alone. Its name is required: it is the tooltip and the accessible name.
    pub fn icon_only(icon: IconName, name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            label: None,
            icon: Some(icon),
            disabled: false,
            tooltip: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A tooltip on hover and the segment's description, such as why a disabled segment is off. It replaces an
    /// icon-only segment's name tooltip.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn tooltip_text(&self) -> Option<&SharedString> {
        self.tooltip.as_ref()
    }
}

type ChangeHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Two to four mutually exclusive options. The caller keeps the selected index and
/// updates it in `on_change`.
#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    group_name: SharedString,
    segments: Vec<Segment>,
    selected: usize,
    style: SegmentedStyle,
    size: SegmentedSize,
    strong: bool,
    disabled: bool,
    preview: Option<(usize, StatePreview)>,
    on_change: Option<ChangeHandler>,
}

impl fmt::Debug for SegmentedControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SegmentedControl")
            .field("id", &self.id)
            .field("selected", &self.selected)
            .field("segments", &self.segments)
            .field("disabled", &self.disabled)
            .finish_non_exhaustive()
    }
}

impl SegmentedControl {
    /// `group_name` says what the control chooses ("Search in", "View"); it is the
    /// group's accessible name, since the control has no visible label of its own.
    pub fn new(
        id: impl Into<ElementId>,
        group_name: impl Into<SharedString>,
        segments: Vec<Segment>,
    ) -> Self {
        Self {
            id: id.into(),
            group_name: group_name.into(),
            segments,
            selected: 0,
            style: SegmentedStyle::default(),
            size: SegmentedSize::default(),
            strong: false,
            disabled: false,
            preview: None,
            on_change: None,
        }
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    pub fn style(mut self, style: SegmentedStyle) -> Self {
        self.style = style;
        self
    }

    pub fn size(mut self, size: SegmentedSize) -> Self {
        self.size = size;
        self
    }

    /// The strong selected style: the accent fill. At most once per view.
    pub fn strong(mut self, strong: bool) -> Self {
        self.strong = strong;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Forces a state on one segment (gallery and visual checks only).
    pub fn preview(mut self, index: usize, state: StatePreview) -> Self {
        self.preview = Some((index, state));
        self
    }

    /// Called with the index a click, Enter, Space or an arrow key selects.
    pub fn on_change(mut self, listener: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(listener));
        self
    }
}

/// The element id of segment `index`; its focus handle is keyed by the same id.
fn segment_id(group: &ElementId, index: usize) -> ElementId {
    ElementId::NamedChild(Arc::new(group.clone()), format!("segment-{index}").into())
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let count = self.segments.len();
        let selected = self.selected.min(count.saturating_sub(1));
        let enabled: Vec<bool> = self
            .segments
            .iter()
            .map(|segment| !segment.disabled && !self.disabled)
            .collect();
        let tab = tab_segment(selected, &enabled);
        let handles: Vec<FocusHandle> = (0..count)
            .map(|index| {
                // GPUI reads a tracked handle's Tab stop from the handle, not the element.
                window
                    .use_keyed_state(segment_id(&self.id, index), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone()
                    .tab_stop(tab == Some(index))
            })
            .collect();
        let outlined = self.style == SegmentedStyle::Outlined;
        let radius = if outlined { RADIUS_MD } else { RADIUS_LG };

        let arrow_handles = handles.clone();
        let arrow_enabled = enabled.clone();
        let arrow_change = self.on_change.clone();
        let mut track = div()
            .id(self.id.clone())
            .role(Role::RadioGroup)
            .aria_label(self.group_name.clone())
            .flex()
            .h(px(self.size.track()))
            .p(px(SPACE_HALF))
            .rounded(px(radius))
            .bg(track_fill(&p, self.style, self.disabled))
            .when(outlined, |track| {
                track.border_1().border_color(p.border.strong)
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let current = arrow_handles
                    .iter()
                    .position(|handle| handle.is_focused(window))
                    .unwrap_or(selected);
                let Some(next) = segment_key(current, &arrow_enabled, &event.keystroke.key) else {
                    return;
                };
                cx.stop_propagation();
                arrow_handles[next].focus(window, cx);
                if let (true, Some(change)) = (next != selected, &arrow_change) {
                    change(next, window, cx);
                }
            });

        for (index, segment) in self.segments.into_iter().enumerate() {
            let is_selected = index == selected;
            let is_enabled = enabled[index];
            let shown = match self.preview {
                Some((at, StatePreview::Hover)) if at == index => Visual::Hover,
                Some((at, StatePreview::Pressed)) if at == index => Visual::Pressed,
                _ => Visual::Rest,
            };
            let ink = paint(&p, is_selected, self.strong, !is_enabled, shown);
            let hover = paint(&p, is_selected, self.strong, !is_enabled, Visual::Hover).fill;
            let pressed = paint(&p, is_selected, self.strong, !is_enabled, Visual::Pressed).fill;
            let label_style = match self.style {
                SegmentedStyle::Sunken if is_selected => TextStyleToken {
                    weight: 600,
                    ..self.size.label()
                },
                _ => self.size.label(),
            };
            let focus_shown =
                matches!(self.preview, Some((at, StatePreview::Focus)) if at == index);

            let mut content = Vec::new();
            if let Some(icon) = segment.icon {
                content.push(
                    Icon::new(icon)
                        .size(IconSize::Small)
                        .color(ink.label)
                        .into_any_element(),
                );
            }
            if let Some(label) = segment.label.clone() {
                // The label keeps its width down to the segment floor, so a segment grows to fit it and
                // a short label never truncates (SegmentedControl.md; the floor is measured inside the label).
                let icon_width = if segment.icon.is_some() {
                    SIZE_ICON_SM + SPACE_1
                } else {
                    0.0
                };
                let floor = SEGMENT_MIN_WIDTH - 2.0 * SPACE_2 - icon_width;
                content.push(
                    div()
                        .debug_selector(move || format!("segment-label-{index}"))
                        .min_w(px(floor))
                        .truncate()
                        .child(label)
                        .into_any_element(),
                );
            }
            let mut element = div()
                .id(segment_id(&self.id, index))
                .debug_selector(move || format!("segment-{index}"))
                .role(Role::RadioButton)
                .aria_label(segment.name.clone())
                .aria_toggled(Toggled::from(is_selected))
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(SPACE_1))
                .px(px(SPACE_2))
                .rounded(px(RADIUS_SM))
                .border_2()
                .border_color(if focus_shown { p.border.focus } else { NONE })
                .bg(ink.fill)
                .text_style(label_style)
                .text_color(ink.label)
                .children(content);
            if segment.label.is_none() {
                element = element.min_w(px(SEGMENT_MIN_WIDTH));
            }
            if is_enabled {
                element = element
                    .track_focus(&handles[index])
                    .focus_visible(move |style| style.border_color(p.border.focus));
                if self.preview.is_none() {
                    element = element
                        .cursor_pointer()
                        .hover(move |style| style.bg(hover))
                        .active(move |style| style.bg(pressed));
                }
                if let Some(change) = self.on_change.clone() {
                    let pointer = change.clone();
                    element = element
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            if !is_selected {
                                pointer(index, window, cx);
                            }
                        })
                        .on_click(move |event: &ClickEvent, window, cx| {
                            let keyboard = matches!(event, ClickEvent::Keyboard(_));
                            if keyboard && !is_selected {
                                change(index, window, cx);
                            }
                        });
                }
            }
            if let Some(text) = segment.tooltip.clone() {
                element = element
                    .aria_description(text.clone())
                    .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
                    .tooltip(tooltip_view(Tooltip::new(text)));
            } else if segment.label.is_none() {
                let name = segment.name.clone();
                element = element
                    .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
                    .tooltip(move |_, cx| cx.new(|_| NameTooltip(name.clone())).into());
            }
            track = track.child(a11y::disabled(element, !is_enabled));
        }
        a11y::disabled(track, self.disabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::press_key;
    use gpui::Render;
    use std::cell::RefCell;

    #[test]
    fn arrows_wrap_at_both_ends() {
        let all = [true, true, true];
        assert_eq!(segment_key(2, &all, "right"), Some(0));
        assert_eq!(segment_key(0, &all, "left"), Some(2));
        assert_eq!(segment_key(0, &all, "up"), Some(2));
        assert_eq!(segment_key(2, &all, "down"), Some(0));
        assert_eq!(segment_key(0, &all, "right"), Some(1));
    }

    #[test]
    fn arrows_skip_disabled_segments() {
        assert_eq!(segment_key(0, &[true, false, true], "right"), Some(2));
        assert_eq!(segment_key(2, &[true, false, true], "left"), Some(0));
        assert_eq!(segment_key(0, &[true, false, false], "right"), None);
    }

    #[test]
    fn home_and_end_go_to_the_first_and_last_enabled_segment() {
        let enabled = [false, true, true, false];
        assert_eq!(segment_key(2, &enabled, "home"), Some(1));
        assert_eq!(segment_key(1, &enabled, "end"), Some(2));
    }

    #[test]
    fn other_keys_and_empty_groups_do_not_move() {
        assert_eq!(segment_key(0, &[true, true], "enter"), None);
        assert_eq!(segment_key(0, &[], "right"), None);
        assert_eq!(segment_key(0, &[true], "right"), None);
    }

    #[test]
    fn the_tab_stop_is_the_selected_segment_or_the_first_enabled_one() {
        assert_eq!(tab_segment(1, &[true, true]), Some(1));
        assert_eq!(tab_segment(0, &[false, true, true]), Some(1));
        assert_eq!(tab_segment(0, &[false, false]), None);
    }

    #[test]
    fn the_selected_segment_is_the_quiet_or_strong_fill_and_others_stay_clear() {
        let p = Palette::DARK;
        let quiet = paint(&p, true, false, false, Visual::Rest);
        assert_eq!(quiet.fill, p.fill.selected_raised);
        assert_eq!(quiet.label, p.text.primary);
        let strong = paint(&p, true, true, false, Visual::Rest);
        assert_eq!(strong.fill, p.accent.r#default);
        assert_eq!(strong.label, p.text.on_accent);
        let rest = paint(&p, false, true, false, Visual::Rest);
        assert_eq!(rest.fill, NONE, "strong only styles the selected segment");
        assert_eq!(rest.label, p.text.secondary);
    }

    #[test]
    fn hover_lights_an_unselected_segment_but_not_the_selected_one() {
        let p = Palette::LIGHT;
        assert_eq!(
            paint(&p, false, false, false, Visual::Hover).fill,
            p.fill.hover_raised
        );
        assert_eq!(
            paint(&p, true, false, false, Visual::Hover).fill,
            p.fill.selected_raised
        );
    }

    #[test]
    fn disabled_segments_use_the_disabled_label_and_the_disabled_track() {
        let p = Palette::DARK;
        assert_eq!(
            paint(&p, false, false, true, Visual::Hover).label,
            p.text.disabled
        );
        assert_eq!(
            track_fill(&p, SegmentedStyle::Outlined, true),
            p.fill.disabled
        );
        assert_eq!(
            track_fill(&p, SegmentedStyle::Sunken, false),
            p.surface.sunken
        );
    }

    #[test]
    fn the_track_heights_follow_the_control_tokens() {
        assert_eq!(SegmentedSize::Compact.track(), 24.0);
        assert_eq!(SegmentedSize::Default.track(), 28.0);
        assert_eq!(SegmentedSize::Comfortable.track(), 32.0);
        assert_eq!(SegmentedSize::default(), SegmentedSize::Default);
        assert_eq!(SegmentedStyle::default(), SegmentedStyle::Outlined);
    }

    /// Hosts a sunken two-segment control, each segment with an icon and a label, in a track of `width`.
    struct WidthHarness {
        width: f32,
    }

    impl Render for WidthHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div().w(px(self.width)).child(
                SegmentedControl::new(
                    "mode",
                    "Mode",
                    vec![
                        Segment::icon_text(IconName::MessageSquare, "Chat"),
                        Segment::icon_text(IconName::SquareTerminal, "Terminal"),
                    ],
                )
                .style(SegmentedStyle::Sunken),
            )
        }
    }

    #[gpui::test]
    fn a_label_keeps_its_width_in_a_narrow_track(cx: &mut gpui::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, _| WidthHarness { width: 400. });
        cx.run_until_parked();
        let wide = f32::from(
            cx.debug_bounds("segment-label-1")
                .expect("label is drawn")
                .size
                .width,
        );
        view.update(cx, |harness, cx| {
            harness.width = 120.;
            cx.notify();
        });
        cx.run_until_parked();
        let narrow = f32::from(
            cx.debug_bounds("segment-label-1")
                .expect("label is drawn")
                .size
                .width,
        );
        assert!(
            (wide - narrow).abs() < 0.5,
            "the label was cut from {wide} to {narrow} px"
        );
    }

    /// Hosts a control of three options, the middle one disabled, that logs each index it asks for.
    struct Harness {
        log: Rc<RefCell<Vec<usize>>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let log = self.log.clone();
            let segments = vec![
                Segment::text("Names"),
                Segment::text("Content").disabled(true),
                Segment::text("Both"),
            ];
            div().child(
                SegmentedControl::new("under-test", "Search in", segments)
                    .selected(0)
                    .on_change(move |index, _, _| log.borrow_mut().push(index)),
            )
        }
    }

    #[gpui::test]
    fn a_pointer_press_selects_the_segment_once(cx: &mut gpui::TestAppContext) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (_view, cx) = cx.add_window_view(|_, _| Harness { log: log.clone() });
        cx.run_until_parked();
        let bounds = cx.debug_bounds("segment-2").expect("segment is drawn");
        cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        assert_eq!(*log.borrow(), vec![2]);
    }

    #[gpui::test]
    fn arrow_keys_move_focus_and_select_and_skip_the_disabled_segment(
        cx: &mut gpui::TestAppContext,
    ) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (_view, cx) = cx.add_window_view(|_, _| Harness { log: log.clone() });
        cx.run_until_parked();
        // The group's one tab stop is the selected segment; draw so its keys are registered.
        cx.update(|window, cx| window.focus_next(cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        press_key(cx, "right");
        assert_eq!(
            *log.borrow(),
            vec![2],
            "right skips the disabled middle segment"
        );
        press_key(cx, "left");
        press_key(cx, "right");
        assert_eq!(
            *log.borrow(),
            vec![2, 2],
            "left went back to the first segment"
        );
    }

    #[test]
    fn a_segment_keeps_its_disabled_state_and_its_tooltip() {
        let chat = Segment::icon_text(IconName::MessageSquare, "Chat")
            .disabled(true)
            .tooltip("Chat sessions need a newer service.");
        assert!(chat.is_disabled());
        assert_eq!(
            chat.tooltip_text().map(|text| text.as_ref()),
            Some("Chat sessions need a newer service.")
        );
        let terminal = Segment::text("Terminal");
        assert!(!terminal.is_disabled());
        assert_eq!(terminal.tooltip_text(), None);
    }

    /// Hosts a control whose first segment is off, with the reason as its tooltip.
    struct TooltipHarness;

    impl Render for TooltipHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let segments = vec![
                Segment::text("Chat")
                    .disabled(true)
                    .tooltip("Chat sessions need a newer service."),
                Segment::text("Terminal"),
            ];
            div().child(SegmentedControl::new("mode", "Mode", segments).selected(1))
        }
    }

    #[gpui::test]
    fn a_disabled_segment_shows_its_tooltip_on_hover(cx: &mut gpui::TestAppContext) {
        let (_view, cx) = cx.add_window_view(|_, _| TooltipHarness);
        cx.run_until_parked();
        let bounds = cx.debug_bounds("segment-0").expect("segment is drawn");
        cx.simulate_mouse_move(bounds.center(), None, gpui::Modifiers::none());
        cx.executor().advance_clock(DURATION_TOOLTIP_DELAY);
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("tooltip-panel").is_some(),
            "the reason shows on hover over the disabled segment"
        );
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn the_control_is_a_radio_group_named_by_what_it_chooses(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                SegmentedControl::new(
                    "search-in",
                    "Search in",
                    vec![Segment::text("Files"), Segment::text("Sessions")],
                )
                .render(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.role(), Role::RadioGroup);
        assert_eq!(node.label(), Some("Search in"));
    }

    #[gpui::test]
    fn a_disabled_control_is_disabled(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                SegmentedControl::new("search-in", "Search in", vec![Segment::text("Files")])
                    .disabled(true)
                    .render(window, cx),
            )
        })
        .expect("reported");
        assert!(node.is_disabled());
    }
}
