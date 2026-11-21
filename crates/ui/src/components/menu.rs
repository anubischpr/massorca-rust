//! Menu and context menu: actions or options in an anchored overlay (docs/design/components/Menu.md).
//! The menu is an entity that owns the keyboard model: Up and Down move the highlight, Home and
//! End jump to the ends, a letter jumps to the next row that starts with it, Right and Left open
//! and close one submenu level, Enter or Space chooses, and Escape closes one level and returns
//! focus to the opener. Items do not take focus one by one; the panel does.

use crate::components::kbd::Kbd;
use crate::components::popover::{OverlayLayer, fade_in, overlay_surface};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::{StateFill, Surface};
use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{
    LAYOUT_MENU_MAX_WIDTH, LAYOUT_MENU_MIN_WIDTH, RADIUS_LG, RADIUS_MD, SIZE_CONTROL_DEFAULT,
    SIZE_ICON_SM, SPACE_1, SPACE_2, TYPE_BODY, TYPE_CAPTION, TYPE_OVERLINE,
};
use gpui::relative;
use gpui::{
    Anchor, AnyElement, App, Bounds, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, MouseDownEvent, ParentElement, Pixels, Point,
    Render, SharedString, StatefulInteractiveElement, Styled, Task, Window, anchored, deferred,
    div, point, px, transparent_black,
};
use std::fmt;
use std::time::Duration;

/// Pointer rest time before a submenu opens or closes on hover (handoff 2.7).
pub const SUBMENU_HOVER_DELAY_MS: u64 = 150;

/// One row of a menu.
#[derive(Clone, Debug)]
pub enum MenuEntry {
    /// An action, a checkable option or a submenu.
    Item(MenuItem),
    /// A divider between groups of rows.
    Separator,
    /// A small label that names the rows below it.
    Group(SharedString),
}

impl From<MenuItem> for MenuEntry {
    fn from(item: MenuItem) -> Self {
        Self::Item(item)
    }
}

/// An action row. A disabled row stays visible and shows its reason under the label.
#[derive(Clone, Debug)]
pub struct MenuItem {
    label: SharedString,
    icon: Option<IconName>,
    shortcut: Option<SharedString>,
    disabled_reason: Option<SharedString>,
    checked: Option<bool>,
    destructive: bool,
    submenu: Option<Vec<MenuEntry>>,
}

impl MenuItem {
    /// A verb phrase, such as "Rename" or "Copy path".
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled_reason: None,
            checked: None,
            destructive: false,
            submenu: None,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A shortcut shown as a Kbd on the right, for example `Mod+R`.
    pub fn shortcut(mut self, binding: impl Into<SharedString>) -> Self {
        self.shortcut = Some(binding.into());
        self
    }

    /// Disables the row; `reason` says why, in visible text under the label.
    pub fn disabled(mut self, reason: impl Into<SharedString>) -> Self {
        self.disabled_reason = Some(reason.into());
        self
    }

    /// Makes the row checkable, with its current state.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Marks the row as destructive: danger colour; name what is lost in the label.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Makes the row open a submenu of one level.
    pub fn submenu(mut self, entries: Vec<MenuEntry>) -> Self {
        self.submenu = Some(entries);
        self
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled_reason.is_some()
    }

    /// The checked state of a checkable row; `None` for a plain row.
    pub fn is_checked(&self) -> Option<bool> {
        self.checked
    }

    pub fn has_submenu(&self) -> bool {
        self.submenu.is_some()
    }

    pub fn submenu_entries(&self) -> &[MenuEntry] {
        self.submenu.as_deref().unwrap_or(&[])
    }
}

/// Whether a row can take the keyboard highlight: an enabled item.
fn selectable(entry: &MenuEntry) -> bool {
    matches!(entry, MenuEntry::Item(item) if !item.is_disabled())
}

/// The first row the highlight can rest on.
pub fn first_selectable(entries: &[MenuEntry]) -> Option<usize> {
    entries.iter().position(selectable)
}

/// The last row the highlight can rest on.
pub fn last_selectable(entries: &[MenuEntry]) -> Option<usize> {
    entries.iter().rposition(selectable)
}

/// The next selectable row after `current` in the given direction, wrapping at the ends.
pub fn step(entries: &[MenuEntry], current: Option<usize>, forward: bool) -> Option<usize> {
    let count = entries.len();
    let Some(start) = current else {
        return if forward {
            first_selectable(entries)
        } else {
            last_selectable(entries)
        };
    };
    (1..=count).find_map(|offset| {
        let index = if forward {
            (start + offset) % count
        } else {
            (start + count - offset % count) % count
        };
        selectable(&entries[index]).then_some(index)
    })
}

/// The next selectable row whose label starts with `letter` (case-insensitive), searching
/// after `current` and wrapping back to it last.
pub fn type_ahead(entries: &[MenuEntry], current: Option<usize>, letter: char) -> Option<usize> {
    let count = entries.len();
    let start = current.map_or(0, |index| index + 1);
    let wanted = letter.to_lowercase().next()?;
    (0..count).find_map(|offset| {
        let index = (start + offset) % count;
        let MenuEntry::Item(item) = &entries[index] else {
            return None;
        };
        let first = item.label().chars().next()?.to_lowercase().next()?;
        (selectable(&entries[index]) && first == wanted).then_some(index)
    })
}

/// Pointer rest over a row. Only the latest row may act when its timer fires, so a pointer that
/// moved on, or left, cancels the earlier one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HoverIntent {
    pending: Option<usize>,
}

impl HoverIntent {
    pub fn rest_on(&mut self, row: usize) {
        self.pending = Some(row);
    }

    pub fn leave(&mut self) {
        self.pending = None;
    }

    /// Whether the timer for `row` may act; the intent is spent once it does.
    pub fn confirm(&mut self, row: usize) -> bool {
        if self.pending == Some(row) {
            self.pending = None;
            true
        } else {
            false
        }
    }
}

/// Which list the keyboard and the pointer are working in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Level {
    /// The menu's own rows.
    Main,
    /// The open submenu of the row at `parent`.
    Child { parent: usize },
}

/// The submenu that is open, with its own highlight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OpenSubmenu {
    parent: usize,
    active: Option<usize>,
}

/// A row that was chosen: `row` in the menu, and `child` in its submenu when it had one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuChoice {
    pub row: usize,
    pub child: Option<usize>,
}

/// What the menu reports to its owner. The owner drops the menu after either event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEvent {
    /// An enabled row was chosen; the menu closed and focus went back to the opener.
    Chosen(MenuChoice),
    /// The menu closed without a choice (Escape, Tab or a click outside).
    Dismissed,
}

/// An open menu. Create it with `cx.new(|cx| MenuView::new(entries, window, cx))` and place it
/// with [`menu_overlay`].
pub struct MenuView {
    entries: Vec<MenuEntry>,
    focus: FocusHandle,
    opener: Option<FocusHandle>,
    active: Option<usize>,
    submenu: Option<OpenSubmenu>,
    hover: HoverIntent,
    /// A trigger whose clicks do not count as outside clicks, so it can toggle the menu.
    ignored_outside: Option<Bounds<Pixels>>,
    min_width: f32,
    empty_text: SharedString,
    layer: OverlayLayer,
    _hover_timer: Option<Task<()>>,
}

impl EventEmitter<MenuEvent> for MenuView {}

impl fmt::Debug for MenuView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MenuView")
            .field("rows", &self.entries.len())
            .field("active", &self.active)
            .field("submenu", &self.submenu)
            .finish_non_exhaustive()
    }
}

impl MenuView {
    /// Opens the menu: remembers the focused element as the opener and moves focus to the
    /// panel, with the highlight on the first enabled row.
    pub fn new(entries: Vec<MenuEntry>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let opener = window.focused(cx);
        let active = first_selectable(&entries);
        window.focus(&focus, cx);
        Self {
            entries,
            focus,
            opener,
            active,
            submenu: None,
            hover: HoverIntent::default(),
            ignored_outside: None,
            min_width: LAYOUT_MENU_MIN_WIDTH,
            empty_text: "Nothing to do here yet".into(),
            layer: OverlayLayer::default(),
            _hover_timer: None,
        }
    }

    /// The layer the menu paints on. A menu opened from inside a dialog takes
    /// [`OverlayLayer::Dialog`] so that it paints above the dialog's scrim.
    pub fn with_layer(mut self, layer: OverlayLayer) -> Self {
        self.layer = layer;
        self
    }

    /// The layer the menu paints on.
    pub fn layer(&self) -> OverlayLayer {
        self.layer
    }

    /// Minimum panel width, kept within the menu width range; a Select passes its trigger width.
    pub fn with_min_width(mut self, width: f32) -> Self {
        self.min_width = width.clamp(LAYOUT_MENU_MIN_WIDTH, LAYOUT_MENU_MAX_WIDTH);
        self
    }

    /// The line shown instead of rows when there are none, saying why.
    pub fn with_empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_text = text.into();
        self
    }

    /// Starts the highlight on `index` when that row can take it.
    pub fn with_highlight(mut self, index: Option<usize>) -> Self {
        if let Some(index) = index.filter(|index| selectable_at(&self.entries, *index)) {
            self.active = Some(index);
        }
        self
    }

    /// The highlighted row of the menu's own list, or of the open submenu.
    pub fn highlighted(&self) -> Option<usize> {
        self.active_at(self.level())
    }

    /// Whether a submenu is open.
    pub fn submenu_open(&self) -> bool {
        self.submenu.is_some()
    }

    /// Keeps clicks inside `bounds` from dismissing the menu (the trigger that toggles it).
    pub fn ignore_outside_clicks_in(&mut self, bounds: Option<Bounds<Pixels>>) {
        self.ignored_outside = bounds;
    }

    fn level(&self) -> Level {
        match self.submenu {
            Some(open) => Level::Child {
                parent: open.parent,
            },
            None => Level::Main,
        }
    }

    fn entries_at(&self, level: Level) -> &[MenuEntry] {
        match level {
            Level::Main => &self.entries,
            Level::Child { parent } => match self.entries.get(parent) {
                Some(MenuEntry::Item(item)) => item.submenu_entries(),
                _ => &[],
            },
        }
    }

    fn active_at(&self, level: Level) -> Option<usize> {
        match level {
            Level::Main => self.active,
            Level::Child { .. } => self.submenu.and_then(|open| open.active),
        }
    }

    fn set_active_at(&mut self, level: Level, index: Option<usize>, cx: &mut Context<Self>) {
        match level {
            Level::Main => self.active = index,
            Level::Child { .. } => {
                if let Some(open) = self.submenu.as_mut() {
                    open.active = index;
                }
            }
        }
        cx.notify();
    }

    fn move_highlight(&mut self, forward: bool, cx: &mut Context<Self>) {
        let level = self.level();
        let next = step(self.entries_at(level), self.active_at(level), forward);
        self.set_active_at(level, next, cx);
    }

    fn jump_highlight(&mut self, to_end: bool, cx: &mut Context<Self>) {
        let level = self.level();
        let entries = self.entries_at(level);
        let next = if to_end {
            last_selectable(entries)
        } else {
            first_selectable(entries)
        };
        self.set_active_at(level, next, cx);
    }

    fn type_to_highlight(&mut self, letter: char, cx: &mut Context<Self>) -> bool {
        let level = self.level();
        let next = type_ahead(self.entries_at(level), self.active_at(level), letter);
        match next {
            Some(_) => {
                self.set_active_at(level, next, cx);
                true
            }
            None => false,
        }
    }

    /// Opens the submenu of `row` with its first enabled row highlighted; false when the row
    /// has none or is disabled.
    pub fn open_submenu(&mut self, row: usize, cx: &mut Context<Self>) -> bool {
        let Some(MenuEntry::Item(item)) = self.entries.get(row) else {
            return false;
        };
        if item.is_disabled() || !item.has_submenu() {
            return false;
        }
        let active = first_selectable(item.submenu_entries());
        self.submenu = Some(OpenSubmenu {
            parent: row,
            active,
        });
        self.active = Some(row);
        cx.notify();
        true
    }

    fn close_submenu(&mut self, cx: &mut Context<Self>) -> bool {
        match self.submenu.take() {
            Some(open) => {
                self.active = Some(open.parent);
                cx.notify();
                true
            }
            None => false,
        }
    }

    /// Chooses a row of `level`: a submenu row opens its submenu, anything else completes.
    fn choose(&mut self, level: Level, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let (disabled, opens) = match self.entries_at(level).get(index) {
            Some(MenuEntry::Item(item)) => (item.is_disabled(), item.has_submenu()),
            _ => return,
        };
        if disabled {
            return;
        }
        match level {
            Level::Main if opens => {
                self.open_submenu(index, cx);
            }
            Level::Main => self.complete(
                MenuChoice {
                    row: index,
                    child: None,
                },
                window,
                cx,
            ),
            Level::Child { parent } => self.complete(
                MenuChoice {
                    row: parent,
                    child: Some(index),
                },
                window,
                cx,
            ),
        }
    }

    fn complete(&mut self, choice: MenuChoice, window: &mut Window, cx: &mut Context<Self>) {
        self.restore_focus(window, cx);
        cx.emit(MenuEvent::Chosen(choice));
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.restore_focus(window, cx);
        cx.emit(MenuEvent::Dismissed);
    }

    fn restore_focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(opener) = &self.opener {
            window.focus(opener, cx);
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let level = self.level();
        let handled = match key {
            "down" => {
                self.move_highlight(true, cx);
                true
            }
            "up" => {
                self.move_highlight(false, cx);
                true
            }
            "home" => {
                self.jump_highlight(false, cx);
                true
            }
            "end" => {
                self.jump_highlight(true, cx);
                true
            }
            "right" => match self.active_at(level) {
                Some(row) if matches!(level, Level::Main) => self.open_submenu(row, cx),
                _ => false,
            },
            "left" => self.close_submenu(cx),
            "enter" | "space" => {
                if let Some(index) = self.active_at(level) {
                    self.choose(level, index, window, cx);
                }
                true
            }
            "escape" => {
                if !self.close_submenu(cx) {
                    self.dismiss(window, cx);
                }
                true
            }
            "tab" => {
                self.dismiss(window, cx);
                false
            }
            _ => {
                let mut chars = key.chars();
                match (chars.next(), chars.next()) {
                    (Some(letter), None)
                        if letter.is_alphanumeric()
                            && !modifiers.control
                            && !modifiers.alt
                            && !modifiers.platform =>
                    {
                        self.type_to_highlight(letter, cx)
                    }
                    _ => false,
                }
            }
        };
        if handled {
            cx.stop_propagation();
        }
    }

    fn hover_row(&mut self, level: Level, row: usize, cx: &mut Context<Self>) {
        if selectable_at(self.entries_at(level), row) {
            self.set_active_at(level, Some(row), cx);
        }
        if matches!(level, Level::Main) {
            self.hover.rest_on(row);
            let delay = cx
                .background_executor()
                .timer(Duration::from_millis(SUBMENU_HOVER_DELAY_MS));
            self._hover_timer = Some(cx.spawn(async move |this, cx| {
                delay.await;
                this.update(cx, |this, cx| this.hover_settled(row, cx)).ok();
            }));
        }
    }

    fn hover_settled(&mut self, row: usize, cx: &mut Context<Self>) {
        if !self.hover.confirm(row) {
            return;
        }
        let opens =
            matches!(self.entries.get(row), Some(MenuEntry::Item(item)) if item.has_submenu());
        if opens {
            self.open_submenu(row, cx);
        } else if self.submenu.is_some_and(|open| open.parent != row) {
            self.close_submenu(cx);
        }
    }

    fn leave_row(&mut self) {
        self.hover.leave();
    }

    fn dismiss_outside(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        if self
            .ignored_outside
            .is_some_and(|bounds| bounds.contains(&event.position))
        {
            return;
        }
        cx.emit(MenuEvent::Dismissed);
    }
}

fn selectable_at(entries: &[MenuEntry], index: usize) -> bool {
    entries.get(index).is_some_and(selectable)
}

impl Render for MenuView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let panel = overlay_surface(&theme, RADIUS_LG, theme.shadows.e2)
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .min_w(px(self.min_width))
            .max_w(px(LAYOUT_MENU_MAX_WIDTH))
            .p(px(SPACE_1))
            .debug_selector(|| "menu-panel".into())
            .occlude()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.on_key(event, window, cx);
            }))
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.dismiss_outside(event, cx);
            }));

        let body: Vec<AnyElement> = if self.entries.is_empty() {
            vec![
                div()
                    .px(px(SPACE_2))
                    .py(px(SPACE_1))
                    .text_style(TYPE_CAPTION)
                    .text_color(theme.palette.text.secondary)
                    .child(self.empty_text.clone())
                    .into_any_element(),
            ]
        } else {
            self.render_rows(Level::Main, &theme, cx)
        };
        fade_in("menu-enter", panel.children(body), cx)
    }
}

impl MenuView {
    fn render_rows(&self, level: Level, theme: &Theme, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let entries = self.entries_at(level);
        let active = self.active_at(level);
        let open_parent = self.submenu.map(|open| open.parent);
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                MenuEntry::Separator => div()
                    .h(px(1.))
                    .my(px(SPACE_1))
                    .bg(theme.palette.border.subtle)
                    .into_any_element(),
                MenuEntry::Group(label) => div()
                    .px(px(SPACE_2))
                    .pt(px(SPACE_1))
                    .text_style(TYPE_OVERLINE)
                    .text_color(theme.palette.text.tertiary)
                    .child(label.clone())
                    .into_any_element(),
                MenuEntry::Item(item) => {
                    // With a submenu open, the keyboard works there: its parent row keeps the
                    // highlight to show which row is open.
                    let highlighted = match level {
                        Level::Main => {
                            active == Some(index)
                                && open_parent.is_none_or(|parent| parent == index)
                        }
                        Level::Child { .. } => active == Some(index),
                    };
                    let opened = matches!(level, Level::Main) && open_parent == Some(index);
                    let row = self.render_row(level, index, item, highlighted, theme, cx);
                    if opened {
                        let child_rows =
                            self.render_rows(Level::Child { parent: index }, theme, cx);
                        div()
                            .relative()
                            .child(row)
                            .child(
                                overlay_surface(theme, RADIUS_LG, theme.shadows.e2)
                                    .absolute()
                                    .top(px(-SPACE_1))
                                    .left(relative(1.))
                                    .ml(px(SPACE_2))
                                    .flex()
                                    .flex_col()
                                    .min_w(px(LAYOUT_MENU_MIN_WIDTH))
                                    .max_w(px(LAYOUT_MENU_MAX_WIDTH))
                                    .p(px(SPACE_1))
                                    .debug_selector(|| "menu-submenu".into())
                                    .children(child_rows),
                            )
                            .into_any_element()
                    } else {
                        row
                    }
                }
            })
            .collect()
    }

    fn render_row(
        &self,
        level: Level,
        index: usize,
        item: &MenuItem,
        highlighted: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = &theme.palette;
        let disabled = item.is_disabled();
        let label_colour = if disabled {
            p.text.disabled
        } else if item.destructive {
            p.status.danger_fg
        } else {
            p.text.primary
        };
        let icon_colour = if disabled {
            p.text.disabled
        } else if item.destructive {
            p.status.danger_fg
        } else if highlighted {
            p.text.primary
        } else {
            p.text.secondary
        };
        let fills = Surface::Overlay.fills(theme);
        let leading = match item.checked {
            Some(true) => Icon::new(IconName::Check)
                .size(IconSize::Small)
                .color(p.accent.fg)
                .into_any_element(),
            _ => div().w(px(SIZE_ICON_SM)).flex_none().into_any_element(),
        };
        let mut row = div()
            .id(SharedString::from(format!("menu-row-{level:?}-{index}")))
            .debug_selector(move || match level {
                Level::Main => format!("menu-item-{index}"),
                Level::Child { .. } => format!("menu-child-{index}"),
            })
            .relative()
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .min_h(px(SIZE_CONTROL_DEFAULT))
            .px(px(SPACE_2))
            .py(px(SPACE_1))
            .rounded(px(RADIUS_MD))
            .border_2()
            .border_color(if highlighted && !disabled {
                Hsla::from(p.border.focus)
            } else {
                transparent_black()
            })
            .text_style(TYPE_BODY)
            .text_color(label_colour)
            .child(leading);
        if let Some(name) = item.icon {
            row = row.child(Icon::new(name).size(IconSize::Small).color(icon_colour));
        }
        let mut label = div().flex_1().min_w_0().flex().flex_col().child(
            div()
                .min_w_0()
                .truncate()
                .text_color(label_colour)
                .child(item.label.clone()),
        );
        if let Some(reason) = &item.disabled_reason {
            label = label.child(
                div()
                    .min_w_0()
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.tertiary)
                    .child(reason.clone()),
            );
        }
        row = row.child(label);
        if let Some(shortcut) = &item.shortcut {
            row = row.child(Kbd::new(shortcut.clone()).disabled(disabled));
        }
        if item.has_submenu() {
            row = row.child(
                Icon::new(IconName::ChevronRight)
                    .size(IconSize::XSmall)
                    .color(p.text.secondary),
            );
        }
        if highlighted && !disabled {
            row = row.bg(fills.selected);
        }
        if disabled {
            return row.into_any_element();
        }
        row = row.state_fills(theme, Surface::Overlay);
        row = row.on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
            if *hovered {
                this.hover_row(level, index, cx);
            } else {
                this.leave_row();
            }
        }));
        row = row.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            this.choose(level, index, window, cx);
        }));
        row.into_any_element()
    }
}

/// Where an overlay menu opens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuOrigin {
    /// At a window point, for a context menu opened by a right click.
    At(Point<Pixels>),
    /// Below a trigger's bounds, flipping above when there is no room. Clicks inside the
    /// trigger do not dismiss the menu, so the trigger can close it.
    Below(Bounds<Pixels>),
}

/// Places `menu` as an overlay at `origin`, on the overlay layer above the content.
pub fn menu_overlay(menu: &Entity<MenuView>, origin: MenuOrigin, cx: &mut App) -> AnyElement {
    let priority = menu.read(cx).layer().priority();
    let layer = match origin {
        MenuOrigin::At(point) => {
            menu.update(cx, |menu, _| menu.ignore_outside_clicks_in(None));
            anchored()
                .position(point)
                .snap_to_window_with_margin(px(SPACE_2))
        }
        MenuOrigin::Below(trigger) => {
            menu.update(cx, |menu, _| menu.ignore_outside_clicks_in(Some(trigger)));
            anchored()
                .attach(trigger, Anchor::BottomLeft)
                .anchor(Anchor::TopLeft)
                .offset(point(px(0.), px(SPACE_1)))
        }
    };
    deferred(layer.child(menu.clone()))
        .priority(priority)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str) -> MenuEntry {
        MenuItem::new(label.to_owned()).into()
    }

    fn disabled(label: &str) -> MenuEntry {
        MenuItem::new(label.to_owned())
            .disabled("Host unreachable")
            .into()
    }

    /// Enabled rows at 0 and 4; a separator, a disabled row and a group label between them.
    fn sample() -> Vec<MenuEntry> {
        vec![
            item("Rename"),
            MenuEntry::Separator,
            disabled("Delete"),
            MenuEntry::Group("More".into()),
            item("Copy path"),
        ]
    }

    #[test]
    fn only_enabled_items_take_the_highlight() {
        let entries = sample();
        assert_eq!(first_selectable(&entries), Some(0));
        assert_eq!(last_selectable(&entries), Some(4));
        assert_eq!(first_selectable(&[disabled("A")]), None);
    }

    #[test]
    fn arrows_skip_separators_groups_and_disabled_rows_and_wrap() {
        let entries = sample();
        assert_eq!(step(&entries, Some(0), true), Some(4));
        assert_eq!(step(&entries, Some(4), true), Some(0), "wraps forward");
        assert_eq!(step(&entries, Some(0), false), Some(4), "wraps back");
        assert_eq!(step(&entries, None, true), Some(0));
        assert_eq!(step(&entries, None, false), Some(4));
        assert_eq!(step(&[], Some(0), true), None);
    }

    #[test]
    fn a_letter_jumps_to_the_next_enabled_row_that_starts_with_it() {
        let entries = sample();
        assert_eq!(type_ahead(&entries, None, 'c'), Some(4));
        assert_eq!(type_ahead(&entries, None, 'R'), Some(0), "case-insensitive");
        assert_eq!(
            type_ahead(&entries, Some(4), 'r'),
            Some(0),
            "wraps to the start"
        );
        assert_eq!(
            type_ahead(&entries, Some(0), 'd'),
            None,
            "disabled rows are skipped"
        );
        assert_eq!(type_ahead(&entries, None, 'z'), None);
    }

    #[test]
    fn a_pointer_that_moved_on_cancels_the_earlier_submenu_timer() {
        let mut hover = HoverIntent::default();
        hover.rest_on(4);
        assert!(!hover.confirm(0), "the pointer is on another row");
        hover.rest_on(4);
        assert!(hover.confirm(4));
        assert!(!hover.confirm(4), "a confirmed intent is spent");
        hover.rest_on(0);
        hover.leave();
        assert!(!hover.confirm(0), "leaving the rows cancels it");
    }

    #[test]
    fn items_know_their_disabled_reason_and_submenu() {
        let plain = MenuItem::new("Copy");
        assert!(!plain.is_disabled());
        assert!(!plain.has_submenu());
        let parent = MenuItem::new("Open with").submenu(vec![item("Terminal")]);
        assert!(parent.has_submenu());
        assert_eq!(parent.submenu_entries().len(), 1);
        assert!(
            MenuItem::new("Stop")
                .disabled("Nothing running")
                .is_disabled()
        );
    }
}

#[cfg(test)]
mod element_tests {
    use super::*;
    use crate::components::button::Button;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use gpui::{AppContext, Context, Render, TestAppContext, VisualTestContext};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Events = Rc<RefCell<Vec<MenuEvent>>>;

    /// Hosts an opener button and, once opened, the menu placed at a window point.
    struct Harness {
        opener: FocusHandle,
        menu: Option<Entity<MenuView>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let overlay = self
                .menu
                .clone()
                .map(|menu| menu_overlay(&menu, MenuOrigin::At(point(px(8.), px(48.))), cx));
            div()
                .p(px(SPACE_2))
                .child(Button::new("opener", "Actions").focus_handle(self.opener.clone()))
                .children(overlay)
        }
    }

    fn entries() -> Vec<MenuEntry> {
        vec![
            MenuItem::new("Rename").icon(IconName::Pencil).into(),
            MenuItem::new("Duplicate").into(),
            MenuEntry::Separator,
            MenuItem::new("Stop agent")
                .disabled("Nothing is running")
                .into(),
            MenuItem::new("Open with")
                .submenu(vec![
                    MenuItem::new("Terminal").into(),
                    MenuItem::new("Editor").into(),
                ])
                .into(),
            MenuItem::new("Delete workspace (uncommitted changes lost)")
                .destructive()
                .into(),
        ]
    }

    /// Opens a menu over `view`, records its events and draws it.
    fn open(
        view: &Entity<Harness>,
        cx: &mut VisualTestContext,
        events: &Events,
    ) -> Entity<MenuView> {
        let menu = cx.update(|window, cx| cx.new(|cx| MenuView::new(entries(), window, cx)));
        let log = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(&menu, move |_, event: &MenuEvent, _| {
                log.borrow_mut().push(*event);
            })
            .detach();
        });
        view.update(cx, |harness, cx| {
            harness.menu = Some(menu.clone());
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        menu
    }

    #[gpui::test]
    fn arrows_skip_disabled_rows_enter_opens_a_submenu_and_choosing_returns_focus(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            opener: cx.focus_handle(),
            menu: None,
        });
        cx.run_until_parked();
        let opener = view.read_with(cx, |h, _| h.opener.clone());
        focus_and_draw(cx, &opener);
        let events: Events = Rc::default();
        let menu = open(&view, cx, &events);

        let panel = menu.read_with(cx, |m, _| m.focus.clone());
        cx.update(|window, _| assert!(panel.is_focused(window), "the menu takes focus"));
        assert_eq!(menu.read_with(cx, |m, _| m.highlighted()), Some(0));

        press_key(cx, "down");
        press_key(cx, "down");
        assert_eq!(
            menu.read_with(cx, |m, _| m.highlighted()),
            Some(4),
            "skips rows 2 and 3"
        );

        press_key(cx, "enter");
        assert!(menu.read_with(cx, |m, _| m.submenu_open()));
        press_key(cx, "down");
        press_key(cx, "enter");
        assert_eq!(
            *events.borrow(),
            vec![MenuEvent::Chosen(MenuChoice {
                row: 4,
                child: Some(1),
            })]
        );
        cx.update(|window, _| assert!(opener.is_focused(window), "focus returns to the opener"));
    }

    #[gpui::test]
    fn escape_closes_one_level_at_a_time_and_returns_focus(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            opener: cx.focus_handle(),
            menu: None,
        });
        cx.run_until_parked();
        let opener = view.read_with(cx, |h, _| h.opener.clone());
        focus_and_draw(cx, &opener);
        let events: Events = Rc::default();
        let menu = open(&view, cx, &events);

        menu.update(cx, |m, cx| {
            m.open_submenu(4, cx);
        });
        cx.run_until_parked();
        press_key(cx, "escape");
        assert!(!menu.read_with(cx, |m, _| m.submenu_open()));
        assert!(
            events.borrow().is_empty(),
            "the first Escape only closes the submenu"
        );

        press_key(cx, "escape");
        assert_eq!(*events.borrow(), vec![MenuEvent::Dismissed]);
        cx.update(|window, _| assert!(opener.is_focused(window)));
    }

    #[gpui::test]
    fn typing_a_letter_moves_to_the_next_row_that_starts_with_it(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            opener: cx.focus_handle(),
            menu: None,
        });
        cx.run_until_parked();
        let opener = view.read_with(cx, |h, _| h.opener.clone());
        focus_and_draw(cx, &opener);
        let events: Events = Rc::default();
        let menu = open(&view, cx, &events);

        press_key(cx, "d");
        assert_eq!(menu.read_with(cx, |m, _| m.highlighted()), Some(1));
        press_key(cx, "d");
        assert_eq!(menu.read_with(cx, |m, _| m.highlighted()), Some(5));
    }

    #[gpui::test]
    fn clicking_an_enabled_row_chooses_it_and_a_disabled_row_does_nothing(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            opener: cx.focus_handle(),
            menu: None,
        });
        cx.run_until_parked();
        let opener = view.read_with(cx, |h, _| h.opener.clone());
        focus_and_draw(cx, &opener);
        let events: Events = Rc::default();
        let _menu = open(&view, cx, &events);

        let disabled = cx
            .debug_bounds("menu-item-3")
            .expect("disabled row is drawn")
            .center();
        cx.simulate_click(disabled, gpui::Modifiers::none());
        assert!(events.borrow().is_empty());

        let duplicate = cx
            .debug_bounds("menu-item-1")
            .expect("enabled row is drawn")
            .center();
        cx.simulate_click(duplicate, gpui::Modifiers::none());
        assert_eq!(
            *events.borrow(),
            vec![MenuEvent::Chosen(MenuChoice {
                row: 1,
                child: None
            })]
        );
    }

    #[gpui::test]
    fn the_panel_draws_every_row_of_the_entries(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            opener: cx.focus_handle(),
            menu: None,
        });
        cx.run_until_parked();
        let opener = view.read_with(cx, |h, _| h.opener.clone());
        focus_and_draw(cx, &opener);
        let events: Events = Rc::default();
        let _menu = open(&view, cx, &events);
        assert!(cx.debug_bounds("menu-panel").is_some());
        assert!(
            cx.debug_bounds("menu-item-5").is_some(),
            "the last row is drawn"
        );
    }

    #[gpui::test]
    fn a_submenu_opens_beside_the_panel_not_over_it(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            opener: cx.focus_handle(),
            menu: None,
        });
        cx.run_until_parked();
        let opener = view.read_with(cx, |h, _| h.opener.clone());
        focus_and_draw(cx, &opener);
        let events: Events = Rc::default();
        let menu = open(&view, cx, &events);
        menu.update(cx, |m, cx| {
            m.open_submenu(4, cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        let panel = cx.debug_bounds("menu-panel").expect("the panel is drawn");
        let submenu = cx
            .debug_bounds("menu-submenu")
            .expect("the submenu is drawn");
        assert!(
            submenu.origin.x >= panel.right(),
            "submenu starts at {:?}, the panel ends at {:?}",
            submenu.origin.x,
            panel.right()
        );
    }
}
