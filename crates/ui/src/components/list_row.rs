//! ListRow: one row in a list or a tree (docs/design/components/ListRow.md).
//!
//! The row answers its own keys: Enter and Space arrive as a click, Right and
//! Left toggle a tree row, and Shift+F10 or the Menu key open its context menu.
//! Up, Down, Home and End pass through to the list, which owns the roving focus
//! (`list_move` and `move_index`).

use crate::a11y;
use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind, NameTooltip};
use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::button::StatePreview;
use crate::components::skeleton::{LineWidth, Skeleton, SkeletonShape};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::Surface;
use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{
    BORDER_WIDTH_THICK, Color, DURATION_TOOLTIP_DELAY, Palette, RADIUS_MD,
    SIZE_CONTROL_COMFORTABLE, SIZE_CONTROL_COMPACT, SIZE_CONTROL_DEFAULT, SIZE_ICON_SM,
    SIZE_ROW_TWO_LINE, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY, TYPE_BODY_SM, TYPE_BODY_STRONG,
    TextStyleToken,
};
use gpui::{
    AnyElement, App, AppContext, ClickEvent, ElementId, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Visibility, Window, div, px, relative,
};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// Group the root sets, so the hover action shows while the row is hovered.
const ROW_GROUP: &str = "list-row";

/// Indent of one tree level (`space.3`).
pub const TREE_INDENT: f32 = SPACE_3;

/// Fully transparent: a row at rest shows its surface.
const NONE: Color = Color::rgba8(0, 0, 0, 0.0);

/// Lines of text in a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowLines {
    /// The secondary line sits beside the title.
    #[default]
    One,
    /// The secondary line sits under the title (`size.row.two-line`).
    Two,
}

/// Height of a one-line row (`size.control.*`). Two-line rows are always 44 px.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowDensity {
    Compact,
    #[default]
    Default,
    Comfortable,
}

impl RowDensity {
    fn height(self) -> f32 {
        match self {
            Self::Compact => SIZE_CONTROL_COMPACT,
            Self::Default => SIZE_CONTROL_DEFAULT,
            Self::Comfortable => SIZE_CONTROL_COMFORTABLE,
        }
    }
}

/// The height of a row with `lines` lines at `density`.
pub fn row_height(lines: RowLines, density: RowDensity) -> f32 {
    match lines {
        RowLines::One => density.height(),
        RowLines::Two => SIZE_ROW_TWO_LINE,
    }
}

/// A move between the rows of a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListMove {
    Previous,
    Next,
    First,
    Last,
}

/// The move a key asks for in a list. Left and Right belong to tree rows.
pub fn list_move(key: &str) -> Option<ListMove> {
    match key {
        "up" => Some(ListMove::Previous),
        "down" => Some(ListMove::Next),
        "home" => Some(ListMove::First),
        "end" => Some(ListMove::Last),
        _ => None,
    }
}

/// The row a move lands on. Moves stop at the ends; an empty list has no row.
pub fn move_index(current: usize, count: usize, movement: ListMove) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match movement {
        ListMove::Previous => current.saturating_sub(1).min(last),
        ListMove::Next => current.saturating_add(1).min(last),
        ListMove::First => 0,
        ListMove::Last => last,
    })
}

/// The expanded value a key sets on a tree row: Right opens a closed row and Left
/// closes an open one. `None` leaves the key to the list (Left goes to the parent).
pub fn tree_toggle(expanded: Option<bool>, key: &str) -> Option<bool> {
    match (expanded, key) {
        (Some(false), "right") => Some(true),
        (Some(true), "left") => Some(false),
        _ => None,
    }
}

/// The fills of a row at rest, on hover and when pressed (solid, handoff 2.1).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fills {
    rest: Color,
    hover: Color,
    pressed: Color,
}

fn row_fills(surface: Surface, selected: bool, theme: &Theme) -> Fills {
    let state = surface.fills(theme);
    if selected {
        Fills {
            rest: state.selected,
            hover: state.selected,
            pressed: state.pressed,
        }
    } else {
        Fills {
            rest: NONE,
            hover: state.hover,
            pressed: state.pressed,
        }
    }
}

/// The text colours of a row. Disabled wins over dimmed, and a usage colour tints
/// only an enabled title.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ink {
    title: Color,
    secondary: Color,
    icon: Color,
}

fn row_ink(p: &Palette, disabled: bool, dimmed: bool, title: Option<Color>) -> Ink {
    if disabled {
        Ink {
            title: p.text.disabled,
            secondary: p.text.disabled,
            icon: p.text.disabled,
        }
    } else if dimmed {
        Ink {
            title: p.text.tertiary,
            secondary: p.text.tertiary,
            icon: p.text.tertiary,
        }
    } else {
        Ink {
            title: title.unwrap_or(p.text.primary),
            secondary: p.text.secondary,
            icon: p.text.secondary,
        }
    }
}

/// What a row shows at its leading edge.
pub enum RowLeading {
    /// A Lucide icon at `size.icon.sm`.
    Icon(IconName),
    /// An agent's status glyph at the given size.
    Status(AgentState, AgentStatusSize),
    /// An agent's own mark (A17, A84).
    Agent(AgentKind, AgentIconSize),
    /// Any element, such as an avatar or a dot.
    Element(AnyElement),
}

impl RowLeading {
    fn element(self, ink: Ink) -> AnyElement {
        match self {
            Self::Icon(name) => Icon::new(name)
                .size(IconSize::Small)
                .color(ink.icon)
                .into_any_element(),
            Self::Status(state, size) => AgentStatus::new(state).size(size).into_any_element(),
            Self::Agent(kind, size) => AgentIcon::new(kind).size(size).into_any_element(),
            Self::Element(element) => element,
        }
    }
}

impl fmt::Debug for RowLeading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Icon(name) => f.debug_tuple("Icon").field(name).finish(),
            Self::Status(state, size) => f.debug_tuple("Status").field(state).field(size).finish(),
            Self::Agent(kind, size) => f.debug_tuple("Agent").field(kind).field(size).finish(),
            Self::Element(_) => f.write_str("Element"),
        }
    }
}

/// The leading item an error row gives its alert glyph: the status glyph, else the first item.
fn error_slot(leading: &[RowLeading]) -> Option<usize> {
    leading
        .iter()
        .position(|item| matches!(item, RowLeading::Status(..)))
        .or_else(|| (!leading.is_empty()).then_some(0))
}

/// The leading elements of an error row: the alert glyph takes its slot and every other item stays (A84).
fn error_leading(leading: Vec<RowLeading>, ink: Ink, palette: &Palette) -> Vec<AnyElement> {
    let slot = error_slot(&leading);
    let alert = || {
        Icon::new(IconName::OctagonAlert)
            .size(IconSize::Small)
            .color(palette.status.danger_fg)
            .into_any_element()
    };
    let mut elements: Vec<AnyElement> = leading
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            if Some(index) == slot {
                alert()
            } else {
                item.element(ink)
            }
        })
        .collect();
    if slot.is_none() {
        elements.push(alert());
    }
    elements
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type MenuHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One row of a list or tree. The caller keeps the state (selection, expansion) and
/// answers `on_click`, `on_toggle` and `on_context_menu`.
#[derive(IntoElement)]
pub struct ListRow {
    id: ElementId,
    title: SharedString,
    secondary: Option<SharedString>,
    secondary_icon: Option<IconName>,
    leading: Vec<RowLeading>,
    trailing: Option<AnyElement>,
    hover_action: Option<AnyElement>,
    surface: Surface,
    lines: RowLines,
    density: RowDensity,
    level: usize,
    indent: f32,
    expanded: Option<bool>,
    selected: bool,
    disabled: bool,
    dimmed: bool,
    loading: bool,
    label_placeholder: bool,
    error: Option<SharedString>,
    title_strong: bool,
    title_colour: Option<Color>,
    tooltip: Option<SharedString>,
    preview: Option<StatePreview>,
    tab_stop: bool,
    focus: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
    on_toggle: Option<ToggleHandler>,
    on_context_menu: Option<MenuHandler>,
}

impl fmt::Debug for ListRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListRow")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .field("loading", &self.loading)
            .finish_non_exhaustive()
    }
}

impl ListRow {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            secondary: None,
            secondary_icon: None,
            leading: Vec::new(),
            trailing: None,
            hover_action: None,
            surface: Surface::default(),
            lines: RowLines::default(),
            density: RowDensity::default(),
            level: 0,
            indent: 0.0,
            expanded: None,
            selected: false,
            disabled: false,
            dimmed: false,
            loading: false,
            label_placeholder: false,
            error: None,
            title_strong: false,
            title_colour: None,
            tooltip: None,
            preview: None,
            tab_stop: true,
            focus: None,
            on_click: None,
            on_toggle: None,
            on_context_menu: None,
        }
    }

    /// The line under the title (one line beside it, or under it in a two-line row).
    pub fn secondary(mut self, text: impl Into<SharedString>) -> Self {
        self.secondary = Some(text.into());
        self
    }

    /// A 12 px icon before the secondary line, such as the branch a child session runs on (WS-35).
    pub fn secondary_icon(mut self, icon: IconName) -> Self {
        self.secondary_icon = Some(icon);
        self
    }

    /// Adds an item to the leading edge; items sit in the order they are added.
    pub fn leading(mut self, item: RowLeading) -> Self {
        self.leading.push(item);
        self
    }

    /// The trailing slot, always shown.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// An action shown only while the row is hovered or focused (desktop).
    pub fn hover_action(mut self, element: impl IntoElement) -> Self {
        self.hover_action = Some(element.into_any_element());
        self
    }

    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    pub fn lines(mut self, lines: RowLines) -> Self {
        self.lines = lines;
        self
    }

    pub fn density(mut self, density: RowDensity) -> Self {
        self.density = density;
        self
    }

    /// The tree level: indents by `TREE_INDENT` per level and reports the level to assistive tech.
    pub fn level(mut self, level: usize) -> Self {
        self.level = level;
        self.indent = level as f32 * TREE_INDENT;
        self
    }

    /// Extra indent in pixels, beyond the padding (for example a child session, A85).
    pub fn indent(mut self, pixels: f32) -> Self {
        self.indent = pixels;
        self
    }

    /// Makes the row a tree row with a disclosure chevron. `true` means expanded.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Dims the row to tertiary text, for example a host that cannot be reached (A10).
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// Shows a skeleton in place of the content while the row's data loads.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// A loading row draws one bar where its label goes, not a circle and two lines (sidebar rows).
    pub fn label_placeholder(mut self, label_only: bool) -> Self {
        self.label_placeholder = label_only;
        self
    }

    /// A failed row: an octagon-alert glyph replaces the leading items and the
    /// reason becomes the secondary line. The row keeps its fills.
    pub fn error(mut self, reason: impl Into<SharedString>) -> Self {
        self.error = Some(reason.into());
        self
    }

    /// Titles in `body-strong` even when the row is not selected (for example a project).
    pub fn title_strong(mut self, strong: bool) -> Self {
        self.title_strong = strong;
        self
    }

    /// The style the title is set in: `body-strong` when selected or asked for, else `body`.
    pub(crate) fn title_style(&self) -> TextStyleToken {
        if self.selected || self.title_strong {
            TYPE_BODY_STRONG
        } else {
            TYPE_BODY
        }
    }

    /// Colours an enabled title, for example with a usage colour (P29).
    pub fn title_colour(mut self, colour: Color) -> Self {
        self.title_colour = Some(colour);
        self
    }

    /// The full name, shown on hover when a long title is truncated.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Forces a visual state (gallery and visual checks only).
    pub fn preview(mut self, state: StatePreview) -> Self {
        self.preview = Some(state);
        self
    }

    /// Whether the list gives this row its Tab stop (roving focus keeps one per list).
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// Keyboard focus lives with the caller; without a handle the row keeps its own.
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }

    /// Called on a pointer click, and on Enter or Space (a keyboard click).
    pub fn on_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(listener));
        self
    }

    /// Called with the value Right or Left asks for on a tree row.
    pub fn on_toggle(mut self, listener: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(listener));
        self
    }

    /// Called on a right click, and on Shift+F10 or the Menu key.
    pub fn on_context_menu(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_context_menu = Some(Rc::new(listener));
        self
    }

    /// Whether a click or key reaches the row's listeners.
    pub fn activates(&self) -> bool {
        !self.disabled && !self.loading
    }
}

/// A trailing slot that takes its own clicks, so clicking it does not also open the row.
fn slot(id: &ElementId, part: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(ElementId::NamedChild(Arc::new(id.clone()), part.into()))
        .flex_none()
        .on_click(|_, _, cx| cx.stop_propagation())
}

fn shown(mut style: StyleRefinement) -> StyleRefinement {
    style.visibility = Some(Visibility::Visible);
    style
}

/// An action hidden at rest and shown while its row is hovered or has keyboard focus.
fn hover_slot(id: &ElementId, action: AnyElement) -> impl IntoElement {
    let mut slot = slot(id, "hover-action").child(action);
    slot.style().visibility = Some(Visibility::Hidden);
    slot.group_hover(ROW_GROUP, shown).in_focus(shown)
}

impl RenderOnce for ListRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let title_style = self.title_style();
        let theme = *cx.theme();
        let p = theme.palette;
        let height = row_height(self.lines, self.density);
        if self.loading {
            // A placeholder: it has an id, so it can take the row's type, but no role.
            let shape = if self.label_placeholder {
                div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_2))
                    .flex_1()
                    .min_w_0()
                    .child(div().w(px(SIZE_ICON_SM)).flex_none())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Skeleton::line(LineWidth::Partial)),
                    )
                    .into_any_element()
            } else {
                Skeleton::new(SkeletonShape::Row).into_any_element()
            };
            return a11y::disabled(
                div()
                    .id(self.id.clone())
                    .debug_selector(|| "row-placeholder".into())
                    .flex()
                    .items_center()
                    .h(px(height))
                    .px(px(SPACE_2))
                    .child(shape),
                self.disabled,
            );
        }
        let interactive = self.activates();
        let fills = row_fills(self.surface, self.selected, &theme);
        let ink = row_ink(&p, self.disabled, self.dimmed, self.title_colour);
        // GPUI reads a tracked handle's Tab stop from the handle, not from the element.
        let focus = self
            .focus
            .clone()
            .unwrap_or_else(|| {
                window
                    .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone()
            })
            .tab_stop(self.tab_stop && interactive);
        let (rest, border) = match self.preview {
            Some(StatePreview::Hover) => (fills.hover, NONE),
            Some(StatePreview::Pressed) => (fills.pressed, NONE),
            Some(StatePreview::Focus) => (fills.rest, p.border.focus),
            None => (fills.rest, NONE),
        };
        let name = match &self.secondary {
            Some(text) => format!("{}, {text}", self.title),
            None => self.title.to_string(),
        };
        let chevron = if self.disabled {
            p.text.disabled
        } else {
            p.text.tertiary
        };

        let mut root = div()
            .id(self.id.clone())
            .group(ROW_GROUP)
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .h(px(height))
            .pl(px(SPACE_2 + self.indent))
            .pr(px(SPACE_2))
            .rounded(px(RADIUS_MD))
            .border_2()
            .border_color(border)
            .focus_visible(move |s| s.border_color(p.border.focus))
            .bg(rest)
            .role(if self.expanded.is_some() {
                Role::TreeItem
            } else {
                Role::ListItem
            })
            .aria_label(name)
            .aria_selected(self.selected);
        if let Some(open) = self.expanded {
            root = root.aria_expanded(open).aria_level(self.level + 1);
        }
        if interactive && self.preview.is_none() {
            root = root
                .cursor_pointer()
                .hover(move |s| s.bg(fills.hover))
                .active(move |s| s.bg(fills.pressed));
        }
        if let Some(on_click) = self.on_click.clone().filter(|_| interactive) {
            root = root.on_click(move |event: &ClickEvent, window, cx| on_click(event, window, cx));
        }
        if interactive {
            let menu = self.on_context_menu.clone();
            let toggle = self.on_toggle.clone();
            let expanded = self.expanded;
            if let Some(open) = menu.clone() {
                root =
                    root.on_mouse_down(MouseButton::Right, move |_, window, cx| open(window, cx));
            }
            root = root.on_key_down(move |event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let opens_menu = key == "menu" || (key == "f10" && event.keystroke.modifiers.shift);
                if opens_menu {
                    if let Some(menu) = &menu {
                        menu(window, cx);
                        cx.stop_propagation();
                    }
                } else if let (Some(next), Some(toggle)) = (tree_toggle(expanded, key), &toggle) {
                    toggle(next, window, cx);
                    cx.stop_propagation();
                }
            });
        }
        if let Some(text) = self.tooltip {
            root = root
                .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
                .tooltip(move |_, cx| cx.new(|_| NameTooltip(text.clone())).into());
        }

        // An error's reason replaces the secondary line, and its icon with it.
        let (leading, secondary, secondary_icon) = match self.error {
            Some(reason) => (error_leading(self.leading, ink, &p), Some(reason), None),
            None => (
                self.leading
                    .into_iter()
                    .map(|item| item.element(ink))
                    .collect(),
                self.secondary,
                self.secondary_icon,
            ),
        };
        let secondary = secondary.map(|text| {
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_1))
                .min_w_0()
                .children(secondary_icon.map(|icon| {
                    div()
                        .flex_shrink_0()
                        .debug_selector(|| "row-secondary-icon".into())
                        .child(Icon::new(icon).size(IconSize::XSmall).color(ink.secondary))
                }))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_style(TYPE_BODY_SM)
                        .text_color(ink.secondary)
                        .child(text),
                )
        });
        // A name keeps its width up to 60 percent of a one-line row, so its secondary line truncates first.
        let keeps_title = secondary.is_some() && self.lines == RowLines::One;
        let mut title = div()
            .min_w_0()
            .truncate()
            .text_style(title_style)
            .text_color(ink.title)
            .debug_selector(|| "row-title".into())
            .child(self.title);
        if keeps_title {
            title = title.flex_shrink_0().max_w(relative(0.6));
        }
        let body = match self.lines {
            RowLines::One => div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .flex_1()
                .min_w_0()
                .child(title)
                .children(secondary),
            RowLines::Two => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(title)
                .children(secondary),
        };

        root = root.children(leading).child(body);
        if let Some(trailing) = self.trailing {
            root = root.child(slot(&self.id, "trailing").child(trailing));
        }
        if let Some(action) = self.hover_action {
            root = root.child(hover_slot(&self.id, action));
        }
        if let Some(open) = self.expanded {
            let glyph = if open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            };
            root = root.child(Icon::new(glyph).size(IconSize::XSmall).color(chevron));
        }
        if self.selected {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(px(BORDER_WIDTH_THICK))
                    .bg(p.accent.fg),
            );
        }
        a11y::disabled(root, self.disabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use crate::tokens::Appearance;
    use gpui::{Modifiers, Render};
    use std::cell::RefCell;

    fn dark() -> Theme {
        Theme::new(Appearance::Dark)
    }

    #[test]
    fn one_line_rows_follow_the_density_and_two_line_rows_are_44_px() {
        assert_eq!(row_height(RowLines::One, RowDensity::Compact), 24.0);
        assert_eq!(row_height(RowLines::One, RowDensity::Default), 28.0);
        assert_eq!(row_height(RowLines::One, RowDensity::Comfortable), 32.0);
        assert_eq!(row_height(RowLines::Two, RowDensity::Compact), 44.0);
    }

    #[test]
    fn moves_stop_at_both_ends_and_an_empty_list_has_no_row() {
        assert_eq!(move_index(0, 0, ListMove::Next), None);
        assert_eq!(move_index(0, 3, ListMove::Previous), Some(0));
        assert_eq!(move_index(0, 3, ListMove::Next), Some(1));
        assert_eq!(move_index(2, 3, ListMove::Next), Some(2));
        assert_eq!(move_index(1, 3, ListMove::First), Some(0));
        assert_eq!(move_index(1, 3, ListMove::Last), Some(2));
        assert_eq!(
            move_index(9, 3, ListMove::Previous),
            Some(2),
            "a stale index clamps"
        );
    }

    #[test]
    fn only_up_down_home_and_end_are_list_moves() {
        assert_eq!(list_move("up"), Some(ListMove::Previous));
        assert_eq!(list_move("down"), Some(ListMove::Next));
        assert_eq!(list_move("home"), Some(ListMove::First));
        assert_eq!(list_move("end"), Some(ListMove::Last));
        assert_eq!(list_move("right"), None);
        assert_eq!(list_move("enter"), None);
    }

    #[test]
    fn right_opens_a_closed_row_and_left_closes_an_open_one() {
        assert_eq!(tree_toggle(Some(false), "right"), Some(true));
        assert_eq!(tree_toggle(Some(true), "left"), Some(false));
        assert_eq!(
            tree_toggle(Some(true), "right"),
            None,
            "the list moves to the child"
        );
        assert_eq!(
            tree_toggle(Some(false), "left"),
            None,
            "the list moves to the parent"
        );
        assert_eq!(
            tree_toggle(None, "right"),
            None,
            "a plain row does not toggle"
        );
    }

    #[test]
    fn tree_levels_indent_by_space_three() {
        assert_eq!(TREE_INDENT, 12.0);
        let row = ListRow::new("a", "b").level(2);
        assert_eq!(row.indent, 24.0);
        assert_eq!(row.level, 2);
    }

    #[test]
    fn selected_rows_take_the_selected_fill_and_others_stay_transparent() {
        let t = dark();
        let selected = row_fills(Surface::Frame, true, &t);
        assert_eq!(selected.rest, t.palette.fill.selected_frame);
        assert_eq!(selected.hover, t.palette.fill.selected_frame);
        let rest = row_fills(Surface::Frame, false, &t);
        assert_eq!(rest.rest, NONE);
        assert_eq!(rest.hover, t.palette.fill.hover_frame);
        assert_eq!(rest.pressed, t.palette.fill.pressed_frame);
    }

    #[test]
    fn disabled_wins_over_dimmed_and_a_usage_colour_tints_only_an_enabled_title() {
        let p = Palette::DARK;
        let usage = p.usage.ring_2;
        assert_eq!(row_ink(&p, false, false, None).title, p.text.primary);
        assert_eq!(row_ink(&p, false, false, Some(usage)).title, usage);
        assert_eq!(row_ink(&p, false, true, Some(usage)).title, p.text.tertiary);
        let disabled = row_ink(&p, true, true, Some(usage));
        assert_eq!(disabled.title, p.text.disabled);
        assert_eq!(disabled.icon, p.text.disabled);
    }

    #[test]
    fn an_error_row_replaces_its_status_glyph_and_keeps_the_agent_mark() {
        let session = [
            RowLeading::Status(AgentState::Error, AgentStatusSize::XSmall),
            RowLeading::Agent(AgentKind::Gemini, AgentIconSize::Small),
        ];
        assert_eq!(error_slot(&session), Some(0));
        assert_eq!(error_slot(&[RowLeading::Icon(IconName::Folder)]), Some(0));
        assert_eq!(error_slot(&[]), None);
    }

    /// Hosts a child session row that runs on its own branch.
    struct BranchHarness;

    impl Render for BranchHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div().w(px(400.)).child(
                ListRow::new("child", "Feed endpoints")
                    .surface(Surface::Frame)
                    .secondary("feat/feed")
                    .secondary_icon(IconName::GitBranch),
            )
        }
    }

    #[gpui::test]
    fn a_secondary_icon_sits_before_the_secondary_line(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| BranchHarness);
        cx.run_until_parked();
        let icon = cx
            .debug_bounds("row-secondary-icon")
            .expect("the icon is drawn");
        let title = cx.debug_bounds("row-title").expect("the title is drawn");
        assert!(icon.origin.x > title.origin.x + title.size.width);
    }

    /// Hosts one host row in a column of `width`, to measure the width of its name.
    struct WidthHarness {
        width: f32,
    }

    impl Render for WidthHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div().w(px(self.width)).child(
                ListRow::new("host", "oppm-api")
                    .surface(Surface::Frame)
                    .leading(RowLeading::Icon(IconName::Server))
                    .secondary("Last known state, 5 min ago"),
            )
        }
    }

    #[gpui::test]
    fn a_name_keeps_its_width_when_its_secondary_line_squeezes_the_row(
        cx: &mut gpui::TestAppContext,
    ) {
        let (view, cx) = cx.add_window_view(|_, _| WidthHarness { width: 400. });
        cx.run_until_parked();
        let wide = f32::from(
            cx.debug_bounds("row-title")
                .expect("title is drawn")
                .size
                .width,
        );
        view.update(cx, |harness, cx| {
            harness.width = 200.;
            cx.notify();
        });
        cx.run_until_parked();
        let narrow = f32::from(
            cx.debug_bounds("row-title")
                .expect("title is drawn")
                .size
                .width,
        );
        assert!(
            (wide - narrow).abs() < 0.5,
            "the name shrank from {wide} to {narrow} px"
        );
    }

    /// Hosts one row that logs what it is asked to do.
    struct Harness {
        focus: FocusHandle,
        log: Rc<RefCell<Vec<String>>>,
        expanded: Option<bool>,
        disabled: bool,
    }

    impl Harness {
        fn new(
            cx: &mut gpui::Context<Self>,
            log: Rc<RefCell<Vec<String>>>,
            expanded: Option<bool>,
            disabled: bool,
        ) -> Self {
            Self {
                focus: cx.focus_handle(),
                log,
                expanded,
                disabled,
            }
        }
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let (click, toggle, menu) = (self.log.clone(), self.log.clone(), self.log.clone());
            let mut row = ListRow::new("under-test", "Row")
                .focus_handle(self.focus.clone())
                .disabled(self.disabled)
                .on_click(move |event, _, _| {
                    let kind = if matches!(event, ClickEvent::Keyboard(_)) {
                        "keyboard click"
                    } else {
                        "click"
                    };
                    click.borrow_mut().push(kind.into());
                })
                .on_context_menu(move |_, _| menu.borrow_mut().push("menu".into()));
            if let Some(open) = self.expanded {
                row = row.expanded(open).on_toggle(move |next, _, _| {
                    toggle.borrow_mut().push(format!("toggle {next}"));
                });
            }
            div().child(
                div()
                    .absolute()
                    .top(px(0.))
                    .left(px(0.))
                    .debug_selector(|| "row".into())
                    .child(row),
            )
        }
    }

    fn harness(
        cx: &mut gpui::TestAppContext,
        expanded: Option<bool>,
        disabled: bool,
    ) -> (
        Rc<RefCell<Vec<String>>>,
        gpui::Entity<Harness>,
        &mut gpui::VisualTestContext,
    ) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let shared = log.clone();
        let (view, cx) =
            cx.add_window_view(|_, cx| Harness::new(cx, shared.clone(), expanded, disabled));
        cx.run_until_parked();
        (log, view, cx)
    }

    #[gpui::test]
    fn pointer_click_opens_and_right_click_opens_the_menu(cx: &mut gpui::TestAppContext) {
        let (log, _view, cx) = harness(cx, None, false);
        let bounds = cx.debug_bounds("row").expect("row is drawn");
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.simulate_mouse_down(bounds.center(), MouseButton::Right, Modifiers::none());
        assert_eq!(*log.borrow(), vec!["click", "menu"]);
    }

    #[gpui::test]
    fn enter_and_space_are_keyboard_clicks_and_shift_f10_opens_the_menu(
        cx: &mut gpui::TestAppContext,
    ) {
        let (log, view, cx) = harness(cx, None, false);
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "enter");
        press_key(cx, "space");
        press_key(cx, "shift-f10");
        press_key(cx, "menu");
        assert_eq!(
            *log.borrow(),
            vec!["keyboard click", "keyboard click", "menu", "menu"]
        );
    }

    #[gpui::test]
    fn right_and_left_toggle_only_the_matching_tree_state(cx: &mut gpui::TestAppContext) {
        let (log, view, cx) = harness(cx, Some(false), false);
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "left");
        press_key(cx, "down");
        assert!(
            log.borrow().is_empty(),
            "a closed row ignores Left and Down"
        );
        press_key(cx, "right");
        assert_eq!(*log.borrow(), vec!["toggle true"]);
    }

    #[gpui::test]
    fn left_closes_an_open_tree_row(cx: &mut gpui::TestAppContext) {
        let (log, view, cx) = harness(cx, Some(true), false);
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "left");
        assert_eq!(*log.borrow(), vec!["toggle false"]);
    }

    #[gpui::test]
    fn disabled_rows_ignore_pointer_and_keys(cx: &mut gpui::TestAppContext) {
        let (log, _view, cx) = harness(cx, None, true);
        let bounds = cx.debug_bounds("row").expect("row is drawn");
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.simulate_mouse_down(bounds.center(), MouseButton::Right, Modifiers::none());
        assert!(log.borrow().is_empty());
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn a_row_is_a_list_item_named_by_its_title(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(ListRow::new("row", "oppm-api").render(window, cx))
        })
        .expect("reported");
        assert_eq!(node.role(), Role::ListItem);
        assert_eq!(node.label(), Some("oppm-api"));
    }

    #[gpui::test]
    fn a_selected_row_reports_its_selection(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                ListRow::new("row", "oppm-api")
                    .selected(true)
                    .render(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.is_selected(), Some(true));
    }

    #[gpui::test]
    fn an_expandable_row_is_a_tree_item_with_its_level_and_state(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                ListRow::new("group", "Prototypes")
                    .level(1)
                    .expanded(false)
                    .render(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.role(), Role::TreeItem);
        assert_eq!(node.is_expanded(), Some(false));
        assert_eq!(node.level(), Some(2));
    }

    #[gpui::test]
    fn a_disabled_row_is_disabled(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                ListRow::new("row", "oppm-api")
                    .disabled(true)
                    .render(window, cx),
            )
        })
        .expect("reported");
        assert!(node.is_disabled());
    }
}
