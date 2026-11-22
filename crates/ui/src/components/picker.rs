//! Picker: one searchable choice from a list, or a folder to browse
//! (docs/design/components/Picker.md).
//!
//! `PickerState` holds the rules without a window: the items and their groups, the query
//! and the rows it leaves, the active row, the chosen value and the folder being shown.
//! `PickerView` draws it in an anchored popover and turns keys and clicks into events.

use crate::components::button::{Button, ButtonVariant};
use crate::components::empty_state::{EmptyState, EmptyStateVariant};
use crate::components::error_state::{ErrorState, ErrorStateVariant};
use crate::components::file_tree::matches_query;
use crate::components::list_row::{ListMove, move_index};
use crate::components::popover::{Popover, PopoverVariant, PopoverWidth};
use crate::components::search_field::{SearchField, SearchFieldEvent};
use crate::components::skeleton::{LineWidth, Skeleton};
use crate::components::tooltip::{Tooltip, tooltip_view};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::{FocusRing, StateFill, Surface};
use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{
    Color, RADIUS_MD, SIZE_CONTROL_COMPACT, SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, TYPE_BODY,
    TYPE_CAPTION, TYPE_MONO_SM, TYPE_OVERLINE,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, ClickEvent, Context, ElementId, Entity, EventEmitter, FocusHandle,
    HighlightStyle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, Role,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, StyledText, Subscription,
    UniformListScrollHandle, Window, div, px, uniform_list,
};
use std::fmt;
use std::ops::Range;

/// Folder crumbs shown in full; a longer path collapses its middle into "...".
const CRUMBS_SHOWN: usize = 4;

/// One choice. `key` is what the owner gets back; `group` is the overline it sits under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerItem {
    pub key: SharedString,
    pub label: SharedString,
    pub group: Option<SharedString>,
    /// In folder mode, whether Right can enter it.
    pub folder: bool,
}

impl PickerItem {
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            group: None,
            folder: false,
        }
    }

    /// The overline this item sits under. Items of one group are drawn together.
    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn folder(mut self) -> Self {
        self.folder = true;
        self
    }
}

/// Whether the list is ready, still loading its items, or failed to load them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickerStatus {
    #[default]
    Ready,
    /// Three skeleton rows under the search field.
    Loading,
    /// "Could not load …" with Retry; the last list stays above it.
    Error,
}

/// A row that can be chosen, in display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerEntry {
    /// The folder above the one shown (folder mode).
    Parent,
    /// An item, by its index in the item list.
    Item(usize),
    /// Creates the typed name, such as `Create branch "feat/x"`.
    Create(SharedString),
}

/// A row as drawn: a group's overline, or an entry with its highlights.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerRow {
    Group(SharedString),
    Entry {
        entry: PickerEntry,
        label: SharedString,
        /// Byte ranges of the label that the query matches.
        matches: Vec<Range<usize>>,
        /// The keyboard's row. After a query it is the top match.
        active: bool,
        /// The current value.
        selected: bool,
        /// A folder row, which shows the folder glyph in folder mode.
        folder: bool,
    },
}

/// What choosing an entry asks the owner to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerChoice {
    /// An item became the value, and the panel closes.
    Item(SharedString),
    /// Create the typed name, and the panel closes.
    Create(SharedString),
    /// Show another folder. The owner lists it and sets the location.
    Navigate(SharedString),
}

/// The rules of a picker: its items, query, active row, value and folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerState {
    noun: SharedString,
    folder: bool,
    location: SharedString,
    creatable: Option<SharedString>,
    items: Vec<PickerItem>,
    query: String,
    selected: Option<SharedString>,
    active: usize,
    status: PickerStatus,
    note: Option<SharedString>,
}

impl PickerState {
    /// `noun` names what is chosen in the copy, such as "branches".
    pub fn new(noun: impl Into<SharedString>, items: Vec<PickerItem>) -> Self {
        Self {
            noun: noun.into(),
            folder: false,
            location: "".into(),
            creatable: None,
            items,
            query: String::new(),
            selected: None,
            active: 0,
            status: PickerStatus::Ready,
            note: None,
        }
    }

    /// Folder mode: the items are the children of `location`, and a row goes up.
    pub fn folder(mut self, location: impl Into<SharedString>) -> Self {
        self.folder = true;
        self.location = location.into();
        self
    }

    /// Adds a Create row for the typed name when nothing matches, such as "Create branch".
    pub fn creatable(mut self, label: impl Into<SharedString>) -> Self {
        self.creatable = Some(label.into());
        self
    }

    /// Starts with the item `key` as the value. An unknown key is ignored.
    pub fn selected(mut self, key: &str) -> Self {
        if self.items.iter().any(|item| text(&item.key) == key) {
            self.selected = Some(key.into());
        }
        self
    }

    /// A note under the header, such as "Last known state, 5 min ago".
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn noun(&self) -> &SharedString {
        &self.noun
    }

    pub fn is_folder(&self) -> bool {
        self.folder
    }

    pub fn location(&self) -> &SharedString {
        &self.location
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn status(&self) -> PickerStatus {
        self.status
    }

    pub fn header_note(&self) -> Option<&SharedString> {
        self.note.as_ref()
    }

    pub fn selected_key(&self) -> Option<&SharedString> {
        self.selected.as_ref()
    }

    pub fn selected_item(&self) -> Option<&PickerItem> {
        let key = self.selected.as_ref()?;
        self.items.iter().find(|item| &item.key == key)
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn set_status(&mut self, status: PickerStatus) {
        self.status = status;
    }

    pub fn set_note(&mut self, note: Option<SharedString>) {
        self.note = note;
    }

    /// Replaces the items. The value stays, even when its item is gone.
    pub fn set_items(&mut self, items: Vec<PickerItem>) {
        self.items = items;
        self.clamp_active();
    }

    /// Folder mode: shows another folder, with its top row active.
    pub fn set_location(&mut self, location: impl Into<SharedString>) {
        self.location = location.into();
        self.active = 0;
    }

    /// A new query starts again at the top match.
    pub fn set_query(&mut self, query: &str) {
        if self.query != query {
            self.query = query.to_owned();
            self.active = 0;
        }
    }

    /// Moves the active row by one, to an end, or by a page. Moves stop at the ends.
    pub fn move_active(&mut self, movement: ListMove) {
        let count = self.entries().len();
        if let Some(index) = move_index(self.active, count, movement) {
            self.active = index;
        }
    }

    /// The entry under the active row.
    pub fn active_entry(&self) -> Option<PickerEntry> {
        self.entries().into_iter().nth(self.active)
    }

    /// Every row that can be chosen, in display order. Folder mode starts with the row
    /// above; a query keeps the matching items, or only the Create row when none match.
    pub fn entries(&self) -> Vec<PickerEntry> {
        let mut entries = Vec::new();
        if self.folder && parent_location(text(&self.location)).is_some() {
            entries.push(PickerEntry::Parent);
        }
        let matching: Vec<usize> = (0..self.items.len())
            .filter(|&index| self.matches(&self.items[index].label))
            .collect();
        entries.extend(matching.iter().map(|&index| PickerEntry::Item(index)));
        if matching.is_empty() && !self.query.is_empty() && self.creatable.is_some() {
            entries.push(PickerEntry::Create(self.query.as_str().into()));
        }
        entries
    }

    /// Whether the list has nothing to choose: no items and no Create row.
    pub fn is_empty(&self) -> bool {
        self.entries()
            .iter()
            .all(|entry| *entry == PickerEntry::Parent)
    }

    /// The rows as drawn: an overline before each new group, then the entries.
    pub fn rows(&self) -> Vec<PickerRow> {
        let entries = self.entries();
        let mut rows = Vec::with_capacity(entries.len());
        let mut group: Option<&SharedString> = None;
        for (position, entry) in entries.iter().enumerate() {
            let (label, matches, selected, folder) = match entry {
                PickerEntry::Parent => ("..".into(), Vec::new(), false, true),
                PickerEntry::Item(index) => {
                    let item = &self.items[*index];
                    if let Some(name) = &item.group
                        && item.group.as_ref() != group
                    {
                        rows.push(PickerRow::Group(name.clone()));
                    }
                    group = item.group.as_ref();
                    let selected = self.selected.as_ref() == Some(&item.key);
                    let matches = self.match_ranges(&item.label);
                    (item.label.clone(), matches, selected, item.folder)
                }
                PickerEntry::Create(name) => (self.create_text(name), Vec::new(), false, false),
            };
            rows.push(PickerRow::Entry {
                entry: entry.clone(),
                label,
                matches,
                active: position == self.active,
                selected,
                folder,
            });
        }
        rows
    }

    /// What choosing `entry` asks the owner to do. Nothing for the folder above when the
    /// shown folder has no parent.
    pub fn choose(&mut self, entry: &PickerEntry) -> Option<PickerChoice> {
        match entry {
            PickerEntry::Parent => {
                let parent = parent_location(text(&self.location))?;
                Some(PickerChoice::Navigate(parent.into()))
            }
            PickerEntry::Item(index) => {
                let key = self.items.get(*index)?.key.clone();
                self.selected = Some(key.clone());
                Some(PickerChoice::Item(key))
            }
            PickerEntry::Create(name) => Some(PickerChoice::Create(name.clone())),
        }
    }

    /// Enter: chooses the active row.
    pub fn choose_active(&mut self) -> Option<PickerChoice> {
        let entry = self.active_entry()?;
        self.choose(&entry)
    }

    /// Right in folder mode: enters the active folder.
    pub fn enter_folder(&self) -> Option<PickerChoice> {
        match self.active_entry()? {
            PickerEntry::Item(index) if self.folder && self.items[index].folder => {
                Some(PickerChoice::Navigate(self.items[index].key.clone()))
            }
            _ => None,
        }
    }

    /// Left, or Backspace on an empty search, in folder mode: goes up one folder.
    pub fn leave_folder(&self) -> Option<PickerChoice> {
        if !self.folder {
            return None;
        }
        let parent = parent_location(text(&self.location))?;
        Some(PickerChoice::Navigate(parent.into()))
    }

    /// The folder's path in crumbs. A long path keeps its first crumb and the last two,
    /// with "..." between them.
    pub fn crumbs(&self) -> Vec<SharedString> {
        let parts: Vec<&str> = text(&self.location)
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        if parts.len() <= CRUMBS_SHOWN {
            return parts.into_iter().map(SharedString::from).collect();
        }
        let mut crumbs = vec![SharedString::from(parts[0]), "...".into()];
        crumbs.extend(
            parts[parts.len() - 2..]
                .iter()
                .map(|part| SharedString::from(*part)),
        );
        crumbs
    }

    /// "No branches yet", or "No branches match …" for the query; "This folder is empty" in
    /// folder mode.
    pub fn empty_title(&self) -> SharedString {
        if self.folder {
            "This folder is empty".into()
        } else if self.query.is_empty() {
            format!("No {} yet", self.noun).into()
        } else {
            format!("No {} match \"{}\"", self.noun, self.query).into()
        }
    }

    /// The copy of a failed load, which offers Retry.
    pub fn error_title(&self) -> SharedString {
        format!("Could not load {}.", self.noun).into()
    }

    /// The loading line that assistive tech announces once.
    pub fn loading_text(&self) -> SharedString {
        format!("Loading {}", self.noun).into()
    }

    fn matches(&self, label: &SharedString) -> bool {
        self.query.is_empty() || !matches_query(text(label), &self.query).is_empty()
    }

    fn match_ranges(&self, label: &SharedString) -> Vec<Range<usize>> {
        if self.query.is_empty() {
            Vec::new()
        } else {
            matches_query(text(label), &self.query)
        }
    }

    fn create_text(&self, name: &SharedString) -> SharedString {
        let verb = self.creatable.as_ref().map_or("Create", text);
        format!("{verb} \"{}\"", text(name)).into()
    }

    fn clamp_active(&mut self) {
        let count = self.entries().len();
        self.active = self.active.min(count.saturating_sub(1));
    }
}

fn text(value: &SharedString) -> &str {
    value.as_ref()
}

/// The folder above `location`: "/" above "/a", and none above the root or a relative
/// name without a slash.
pub fn parent_location(location: &str) -> Option<String> {
    let trimmed = location.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let (head, _) = trimmed.rsplit_once('/')?;
    Some(if head.is_empty() {
        "/".to_owned()
    } else {
        head.to_owned()
    })
}

/// Rows the list shows before it scrolls.
const MAX_VISIBLE_ROWS: usize = 8;
/// Skeleton rows under the search field while the items load.
const LOADING_ROWS: usize = 3;
/// Fully transparent: no fill at rest, and no ring unless the row is active.
const CLEAR: Color = Color::rgba8(0, 0, 0, 0.0);

/// What a picker reports to its owner. The picker has already moved its value or its folder
/// by the time the owner hears of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerEvent {
    /// An item became the value. Read it with `state().selected_item()`.
    Chosen(SharedString),
    /// The Create row was chosen, with the typed name.
    Created(SharedString),
    /// Another folder is shown. The owner lists it and calls `set_location`.
    Navigate(SharedString),
    /// The query changed.
    Query(SharedString),
    /// The footer action was pressed.
    Footer,
    /// Retry was pressed in the error state.
    Retry,
}

/// A field that opens a searchable panel in an anchored popover. The entity owns its
/// [`PickerState`] and emits [`PickerEvent`]s.
pub struct PickerView {
    id: ElementId,
    label: SharedString,
    placeholder: SharedString,
    state: PickerState,
    footer: Option<SharedString>,
    disabled: bool,
    open: bool,
    trigger_focus: FocusHandle,
    search: Entity<SearchField>,
    scroll: UniformListScrollHandle,
    _search_events: Subscription,
}

impl EventEmitter<PickerEvent> for PickerView {}

impl fmt::Debug for PickerView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PickerView")
            .field("open", &self.open)
            .finish_non_exhaustive()
    }
}

impl PickerView {
    /// `label` is the field's accessible name, such as "Base branch". `placeholder` names the
    /// choice in the trigger while nothing is chosen, such as "Choose a base branch".
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
        state: PickerState,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = state.query().to_owned();
        let search_placeholder = format!("Search {}", state.noun());
        let search = cx.new(|cx| SearchField::with_query(search_placeholder, &query, cx));
        let search_events = cx.subscribe(&search, |this, _, event: &SearchFieldEvent, cx| {
            this.on_search(event, cx);
        });
        Self {
            id: id.into(),
            label: label.into(),
            placeholder: placeholder.into(),
            state,
            footer: None,
            disabled: false,
            open: false,
            trigger_focus: cx.focus_handle(),
            search,
            scroll: UniformListScrollHandle::new(),
            _search_events: search_events,
        }
    }

    /// One ghost action under the list, for something outside it, such as "Browse folders".
    pub fn footer(mut self, label: impl Into<SharedString>) -> Self {
        self.footer = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Starts with the panel open, for the gallery and visual checks.
    pub fn open_at_start(mut self) -> Self {
        self.open = true;
        self
    }

    pub fn state(&self) -> &PickerState {
        &self.state
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_items(&mut self, items: Vec<PickerItem>, cx: &mut Context<Self>) {
        self.state.set_items(items);
        cx.notify();
    }

    pub fn set_status(&mut self, status: PickerStatus, cx: &mut Context<Self>) {
        self.state.set_status(status);
        cx.notify();
    }

    /// Folder mode: shows `location`. The owner lists its items and calls `set_items`.
    pub fn set_location(&mut self, location: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.state.set_location(location);
        cx.notify();
    }

    fn on_search(&mut self, event: &SearchFieldEvent, cx: &mut Context<Self>) {
        if let SearchFieldEvent::Search(query) = event {
            self.state.set_query(query);
            cx.emit(PickerEvent::Query(query.clone()));
            cx.notify();
        }
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.set_query("", window, cx));
    }

    fn open_panel(&mut self, cx: &mut Context<Self>) {
        if !self.disabled && !self.open {
            self.open = true;
            cx.notify();
        }
    }

    /// Hides the panel. The popover has already returned focus to the trigger.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss(cx);
        window.focus(&self.trigger_focus, cx);
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(window, cx);
        } else {
            self.open_panel(cx);
        }
    }

    /// A value or a creation closes the panel; a folder change keeps it open.
    fn commit(&mut self, choice: PickerChoice, window: &mut Window, cx: &mut Context<Self>) {
        match choice {
            PickerChoice::Item(key) => {
                cx.emit(PickerEvent::Chosen(key));
                self.close(window, cx);
            }
            PickerChoice::Create(name) => {
                cx.emit(PickerEvent::Created(name));
                self.close(window, cx);
            }
            PickerChoice::Navigate(path) => {
                self.clear_search(window, cx);
                cx.emit(PickerEvent::Navigate(path));
                cx.notify();
            }
        }
    }

    fn choose_entry(&mut self, entry: &PickerEntry, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(choice) = self.state.choose(entry) {
            self.commit(choice, window, cx);
        }
    }

    fn reveal_active(&self) {
        let row = self
            .state
            .rows()
            .iter()
            .position(|row| matches!(row, PickerRow::Entry { active: true, .. }));
        if let Some(row) = row {
            self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        }
    }

    /// Keys of the open panel, caught before the search field so that Escape closes the
    /// panel even with a query. Left, Right and Backspace act on folders only while the
    /// search is empty; otherwise they move the caret.
    fn on_panel_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let free = self.state.query().is_empty() && self.state.is_folder();
        let handled = match key {
            "down" | "up" => {
                let movement = if key == "down" {
                    ListMove::Next
                } else {
                    ListMove::Previous
                };
                self.state.move_active(movement);
                self.reveal_active();
                true
            }
            "enter" => {
                if let Some(choice) = self.state.choose_active() {
                    self.commit(choice, window, cx);
                }
                true
            }
            "escape" => {
                self.close(window, cx);
                true
            }
            "right" if free => {
                if let Some(choice) = self.state.enter_folder() {
                    self.commit(choice, window, cx);
                }
                true
            }
            "left" | "backspace" if free => {
                if let Some(choice) = self.state.leave_folder() {
                    self.commit(choice, window, cx);
                }
                true
            }
            _ => false,
        };
        if handled {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn on_trigger_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if event.keystroke.key == "down" {
            self.open_panel(cx);
            cx.stop_propagation();
        }
    }

    fn panel(&self, entity: &Entity<Self>, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let p = theme.palette;
        let mut panel = div()
            .id((self.id.clone(), "panel"))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.on_panel_key(event, window, cx);
            }))
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .p(px(SPACE_2))
            .text_style(TYPE_BODY)
            .text_color(p.text.primary);
        if let Some(note) = self.state.header_note() {
            panel = panel.child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.tertiary)
                    .child(note.clone()),
            );
        }
        panel = panel.child(self.search.clone());
        if self.state.is_folder() {
            let crumbs = self
                .state
                .crumbs()
                .iter()
                .map(|crumb| crumb.to_string())
                .collect::<Vec<_>>()
                .join(" / ");
            panel = panel.child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_MONO_SM)
                    .text_color(p.text.tertiary)
                    .child(crumbs),
            );
        }
        panel = panel.child(self.body(entity, theme));
        if let Some(label) = self.footer.clone() {
            panel = panel.child(self.footer_button(entity, label));
        }
        panel.into_any_element()
    }

    fn body(&self, entity: &Entity<Self>, theme: &Theme) -> AnyElement {
        let status = self.state.status();
        if status == PickerStatus::Loading {
            return self.loading();
        }
        let mut body = div().flex().flex_col().min_h_0();
        if status == PickerStatus::Error {
            body = body.child(self.error(entity));
        }
        let rows = self.state.rows();
        if !rows.is_empty() {
            body = body.child(self.list(entity, theme, rows));
        }
        if status == PickerStatus::Ready && self.state.is_empty() {
            body = body.child(self.empty());
        }
        body.into_any_element()
    }

    fn loading(&self) -> AnyElement {
        div()
            .id((self.id.clone(), "loading"))
            .debug_selector(|| "picker-loading".into())
            .role(Role::Status)
            .aria_label(self.state.loading_text())
            .flex()
            .flex_col()
            .children((0..LOADING_ROWS).map(|_| {
                div()
                    .flex()
                    .items_center()
                    .h(px(SIZE_CONTROL_COMPACT))
                    .px(px(SPACE_2))
                    .child(Skeleton::line(LineWidth::Most))
            }))
            .into_any_element()
    }

    fn error(&self, entity: &Entity<Self>) -> AnyElement {
        let retry = entity.clone();
        ErrorState::new((self.id.clone(), "error"), self.state.error_title())
            .variant(ErrorStateVariant::Section)
            .on_retry(move |_, cx| retry.update(cx, |_, cx| cx.emit(PickerEvent::Retry)))
            .into_any_element()
    }

    fn empty(&self) -> AnyElement {
        div()
            .debug_selector(|| "picker-empty".into())
            .p(px(SPACE_2))
            .child(
                EmptyState::new((self.id.clone(), "empty"), self.state.empty_title())
                    .variant(EmptyStateVariant::Compact),
            )
            .into_any_element()
    }

    fn list(&self, entity: &Entity<Self>, theme: &Theme, rows: Vec<PickerRow>) -> AnyElement {
        let count = rows.len();
        let height = count.min(MAX_VISIBLE_ROWS) as f32 * SIZE_CONTROL_COMPACT;
        let (view, theme) = (entity.clone(), *theme);
        div()
            .id((self.id.clone(), "listbox"))
            .role(Role::ListBox)
            .aria_label(self.label.clone())
            .h(px(height))
            .child(
                uniform_list((self.id.clone(), "rows"), count, move |range, _, _| {
                    range
                        .map(|index| render_row(&view, &theme, index, &rows[index]))
                        .collect()
                })
                .track_scroll(&self.scroll)
                .size_full(),
            )
            .into_any_element()
    }

    fn footer_button(&self, entity: &Entity<Self>, label: SharedString) -> AnyElement {
        let entity = entity.clone();
        Button::new((self.id.clone(), "footer"), label)
            .variant(ButtonVariant::Ghost)
            .on_click(move |_, cx| entity.update(cx, |_, cx| cx.emit(PickerEvent::Footer)))
            .into_any_element()
    }
}

impl Render for PickerView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let entity = cx.entity();
        let value = self.state.selected_item().map(|item| item.label.clone());
        let shown = value.clone().unwrap_or_else(|| self.placeholder.clone());
        let (fill, ink, edge) = if self.disabled {
            (p.fill.disabled, p.text.disabled, p.border.disabled)
        } else {
            let ink = if value.is_some() {
                p.text.primary
            } else {
                p.text.tertiary
            };
            let edge = if self.open {
                p.border.focus
            } else {
                p.border.strong
            };
            (p.surface.sunken, ink, edge)
        };
        let mut trigger = div()
            .id((self.id.clone(), "trigger"))
            .debug_selector(|| "picker-trigger".into())
            .track_focus(&self.trigger_focus)
            .tab_stop(!self.disabled)
            .focus_ring(&theme)
            .role(Role::ComboBox)
            .aria_label(self.label.clone())
            .aria_expanded(self.open)
            .flex()
            .items_center()
            .justify_between()
            .gap(px(SPACE_2))
            .h(px(SIZE_CONTROL_DEFAULT))
            .px(px(SPACE_2))
            .rounded(px(RADIUS_MD))
            .bg(fill)
            .border_color(edge)
            .when(self.open, |el| el.border_2())
            .when(!self.open, |el| el.border_1())
            .text_style(TYPE_BODY)
            .text_color(ink)
            .child(div().flex_1().min_w_0().truncate().child(shown))
            .child(
                Icon::new(IconName::ChevronsUpDown)
                    .size(IconSize::Small)
                    .color(if self.open {
                        p.accent.fg
                    } else {
                        p.text.secondary
                    }),
            );
        if !self.disabled {
            trigger = trigger
                .cursor_pointer()
                .state_fills(&theme, Surface::Raised)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.toggle(window, cx);
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    this.on_trigger_key(event, cx);
                }));
        }
        let dismiss = entity.clone();
        let panel = self.panel(&entity, &theme, cx);
        Popover::new((self.id.clone(), "popover"), trigger)
            .open(self.open)
            .variant(PopoverVariant::Picker)
            .width(PopoverWidth::Medium)
            .trigger_focus(self.trigger_focus.clone())
            .initial_focus(self.search.read(cx).focus_handle(cx))
            .on_dismiss(move |_, cx| dismiss.update(cx, |this, cx| this.dismiss(cx)))
            .child(panel)
    }
}

/// One row of the list: an overline, or an entry with its highlights and its glyph.
fn render_row(
    view: &Entity<PickerView>,
    theme: &Theme,
    index: usize,
    row: &PickerRow,
) -> AnyElement {
    let p = theme.palette;
    match row {
        PickerRow::Group(name) => div()
            .id(("picker-group", index))
            .h(px(SIZE_CONTROL_COMPACT))
            .px(px(SPACE_2))
            .flex()
            .items_center()
            .text_style(TYPE_OVERLINE)
            .text_color(p.text.tertiary)
            .child(name.clone())
            .into_any_element(),
        PickerRow::Entry {
            entry,
            label,
            matches,
            active,
            selected,
            folder,
        } => {
            let fills = Surface::Overlay.fills(theme);
            let rest = if *selected { fills.selected } else { CLEAR };
            let highlight = if *active {
                p.r#match.active_bg
            } else {
                p.r#match.bg
            };
            let highlights = matches.iter().map(|range| {
                (
                    range.clone(),
                    HighlightStyle {
                        background_color: Some(highlight.into()),
                        ..Default::default()
                    },
                )
            });
            let (click_view, chosen) = (view.clone(), entry.clone());
            let mut line = div()
                .id(("picker-row", index))
                .debug_selector(move || format!("picker-row-{index}"))
                .role(Role::ListBoxOption)
                .aria_label(label.clone())
                .aria_selected(*selected)
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .h(px(SIZE_CONTROL_COMPACT))
                .px(px(SPACE_2))
                .rounded(px(RADIUS_MD))
                .border_2()
                .border_color(if *active { p.border.focus } else { CLEAR })
                .bg(rest)
                .hover(move |s| s.bg(fills.hover))
                .active(move |s| s.bg(fills.pressed))
                .cursor_pointer()
                .tooltip(tooltip_view(Tooltip::new(label.clone())))
                .on_click(move |_, window, cx| {
                    click_view.update(cx, |this, cx| this.choose_entry(&chosen, window, cx));
                });
            let glyph = match entry {
                PickerEntry::Parent => Some(IconName::ArrowLeft),
                PickerEntry::Item(_) if *folder => Some(IconName::Folder),
                PickerEntry::Item(_) => None,
                PickerEntry::Create(_) => Some(IconName::Plus),
            };
            if let Some(glyph) = glyph {
                line = line.child(
                    Icon::new(glyph)
                        .size(IconSize::Small)
                        .color(p.text.secondary),
                );
            }
            line = line.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(p.text.primary)
                    .child(StyledText::new(label.clone()).with_highlights(highlights)),
            );
            if *selected {
                line = line.child(
                    Icon::new(IconName::Check)
                        .size(IconSize::XSmall)
                        .color(p.accent.fg),
                );
            }
            line.into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One line per row: `[overline]` for a group, the label for an entry.
    fn labels(state: &PickerState) -> Vec<String> {
        state
            .rows()
            .iter()
            .map(|row| match row {
                PickerRow::Group(name) => format!("[{name}]"),
                PickerRow::Entry { label, .. } => label.to_string(),
            })
            .collect()
    }

    fn highlights(state: &PickerState) -> Vec<Vec<Range<usize>>> {
        state
            .rows()
            .into_iter()
            .filter_map(|row| match row {
                PickerRow::Entry { matches, .. } => Some(matches),
                PickerRow::Group(_) => None,
            })
            .collect()
    }

    fn branches() -> PickerState {
        PickerState::new(
            "branches",
            vec![
                PickerItem::new("main", "main"),
                PickerItem::new("feature/map", "feature/map"),
                PickerItem::new("release", "release"),
            ],
        )
    }

    #[test]
    fn items_of_one_group_sit_under_one_overline() {
        let state = PickerState::new(
            "branches",
            vec![
                PickerItem::new("a", "Alpha").group("RECENT"),
                PickerItem::new("b", "Beta").group("RECENT"),
                PickerItem::new("c", "Gamma").group("ALL"),
                PickerItem::new("d", "Delta"),
            ],
        );
        assert_eq!(
            labels(&state),
            ["[RECENT]", "Alpha", "Beta", "[ALL]", "Gamma", "Delta"].map(String::from)
        );
    }

    #[test]
    fn a_query_keeps_the_matching_items_and_marks_their_characters() {
        let mut state = branches();
        state.set_query("ma");
        assert_eq!(labels(&state), ["main", "feature/map"].map(String::from));
        assert_eq!(highlights(&state), vec![vec![0..2], vec![8..10]]);
        let first_active = state.rows().iter().any(
            |row| matches!(row, PickerRow::Entry { active: true, label, .. } if label == "main"),
        );
        assert!(first_active, "the top match is the active row");
    }

    #[test]
    fn the_active_row_moves_and_stops_at_both_ends() {
        let mut state = branches();
        state.move_active(ListMove::Next);
        state.move_active(ListMove::Next);
        state.move_active(ListMove::Next);
        assert_eq!(state.active(), 2, "stops at the last row");
        state.move_active(ListMove::Previous);
        assert_eq!(state.active(), 1);
        state.move_active(ListMove::First);
        assert_eq!(state.active(), 0);
        state.move_active(ListMove::Last);
        assert_eq!(state.active(), 2);
    }

    #[test]
    fn a_creatable_picker_offers_only_the_create_row_when_nothing_matches() {
        let mut state = branches().creatable("Create branch");
        state.set_query("feat/x");
        assert_eq!(state.entries(), vec![PickerEntry::Create("feat/x".into())]);
        assert_eq!(
            labels(&state),
            ["Create branch \"feat/x\""].map(String::from)
        );
        assert!(!state.is_empty());
        let plain = {
            let mut plain = branches();
            plain.set_query("feat/x");
            plain
        };
        assert!(
            plain.is_empty(),
            "without creation there is nothing to choose"
        );
    }

    #[test]
    fn choosing_an_item_makes_it_the_value_and_marks_its_row() {
        let mut state = branches();
        state.move_active(ListMove::Next);
        assert_eq!(
            state.choose_active(),
            Some(PickerChoice::Item("feature/map".into()))
        );
        assert_eq!(
            state.selected_key(),
            Some(&SharedString::from("feature/map"))
        );
        let selected: Vec<bool> = state
            .rows()
            .iter()
            .filter_map(|row| match row {
                PickerRow::Entry { selected, .. } => Some(*selected),
                PickerRow::Group(_) => None,
            })
            .collect();
        assert_eq!(selected, [false, true, false]);
    }

    #[test]
    fn folder_mode_goes_up_with_a_parent_row_and_moves_in_and_out_of_folders() {
        let mut state = PickerState::new(
            "folders",
            vec![
                PickerItem::new("/projects/massorca/crates", "crates").folder(),
                PickerItem::new("/projects/massorca/README.md", "README.md"),
            ],
        )
        .folder("/projects/massorca");
        assert_eq!(
            labels(&state),
            ["..", "crates", "README.md"].map(String::from)
        );
        assert_eq!(state.enter_folder(), None, "the parent row is not a folder");
        state.move_active(ListMove::Next);
        assert_eq!(
            state.enter_folder(),
            Some(PickerChoice::Navigate("/projects/massorca/crates".into()))
        );
        state.move_active(ListMove::Next);
        assert_eq!(state.enter_folder(), None, "a file cannot be entered");
        assert_eq!(
            state.leave_folder(),
            Some(PickerChoice::Navigate("/projects".into()))
        );
        state.move_active(ListMove::First);
        assert_eq!(
            state.choose_active(),
            Some(PickerChoice::Navigate("/projects".into()))
        );
    }

    #[test]
    fn the_root_has_no_parent_row_and_nothing_to_leave_to() {
        let state = PickerState::new("folders", Vec::new()).folder("/");
        assert!(state.is_empty());
        assert_eq!(state.leave_folder(), None);
    }

    #[test]
    fn crumbs_collapse_the_middle_of_a_long_path() {
        let short = PickerState::new("folders", Vec::new()).folder("/a/b/c");
        assert_eq!(short.crumbs(), ["a", "b", "c"].map(SharedString::from));
        let long = PickerState::new("folders", Vec::new()).folder("/a/b/c/d/e");
        assert_eq!(
            long.crumbs(),
            ["a", "...", "d", "e"].map(SharedString::from)
        );
    }

    #[test]
    fn the_copy_names_what_is_chosen() {
        let mut state = branches();
        state.set_items(Vec::new());
        assert_eq!(state.empty_title(), SharedString::from("No branches yet"));
        state.set_query("x");
        assert_eq!(
            state.empty_title(),
            SharedString::from("No branches match \"x\"")
        );
        assert_eq!(
            state.error_title(),
            SharedString::from("Could not load branches.")
        );
        assert_eq!(state.loading_text(), SharedString::from("Loading branches"));
        let folder = PickerState::new("folders", Vec::new()).folder("/a");
        assert_eq!(
            folder.empty_title(),
            SharedString::from("This folder is empty")
        );
    }

    #[test]
    fn a_shorter_listing_keeps_the_value_and_clamps_the_active_row() {
        let mut state = branches().selected("release");
        state.move_active(ListMove::Last);
        state.set_items(vec![PickerItem::new("main", "main")]);
        assert_eq!(state.active(), 0);
        assert_eq!(state.selected_key(), Some(&SharedString::from("release")));
        assert_eq!(state.selected_item(), None, "the chosen item is gone");
    }

    #[test]
    fn parent_locations_stop_at_the_root() {
        assert_eq!(parent_location("/"), None);
        assert_eq!(parent_location("/a"), Some("/".to_owned()));
        assert_eq!(parent_location("/a/b/"), Some("/a".to_owned()));
        assert_eq!(parent_location("a"), None);
        assert_eq!(parent_location("a/b"), Some("a".to_owned()));
    }
}

#[cfg(test)]
mod element_tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use crate::components::search_field::SEARCH_DEBOUNCE_MS;
    use gpui::{Modifiers, Pixels, Point, TestAppContext, VisualTestContext};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    type Events = Rc<RefCell<Vec<PickerEvent>>>;

    fn branches() -> PickerState {
        PickerState::new(
            "branches",
            vec![
                PickerItem::new("main", "main"),
                PickerItem::new("feature/map", "feature/map"),
                PickerItem::new("release", "release"),
            ],
        )
    }

    fn folders() -> PickerState {
        PickerState::new(
            "folders",
            vec![
                PickerItem::new("/projects/massorca/crates", "crates").folder(),
                PickerItem::new("/projects/massorca/README.md", "README.md"),
            ],
        )
        .folder("/projects/massorca")
    }

    /// A picker in a window, with its events recorded and its first frame drawn.
    fn picker(
        cx: &mut TestAppContext,
        state: PickerState,
    ) -> (Entity<PickerView>, Events, &mut VisualTestContext) {
        let (view, cx) = cx.add_window_view(|_, cx| {
            PickerView::new("branch", "Base branch", "Choose a base branch", state, cx)
        });
        let events: Events = Rc::default();
        let sink = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(&view, move |_, event: &PickerEvent, _| {
                sink.borrow_mut().push(event.clone());
            })
            .detach();
        });
        draw(cx);
        (view, events, cx)
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    fn centre(cx: &mut VisualTestContext, selector: &'static str) -> Point<Pixels> {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is drawn"))
            .center()
    }

    /// Opens the panel with a pointer click on the trigger, and draws it.
    fn open_by_click(cx: &mut VisualTestContext) {
        let trigger = centre(cx, "picker-trigger");
        cx.simulate_click(trigger, Modifiers::none());
        cx.run_until_parked();
        draw(cx);
    }

    fn is_open(view: &Entity<PickerView>, cx: &VisualTestContext) -> bool {
        view.read_with(cx, |picker, _| picker.is_open())
    }

    #[gpui::test]
    fn a_click_opens_the_panel_and_a_row_click_chooses_its_item(cx: &mut TestAppContext) {
        let (view, events, cx) = picker(cx, branches());
        open_by_click(cx);
        assert!(is_open(&view, cx));
        let row = centre(cx, "picker-row-1");
        cx.simulate_click(row, Modifiers::none());
        cx.run_until_parked();
        assert!(!is_open(&view, cx));
        assert_eq!(
            *events.borrow(),
            vec![PickerEvent::Chosen("feature/map".into())]
        );
        assert_eq!(
            view.read_with(cx, |picker, _| picker.state().selected_key().cloned()),
            Some("feature/map".into())
        );
    }

    #[gpui::test]
    fn down_opens_and_the_keys_move_and_choose_from_the_panel(cx: &mut TestAppContext) {
        let (view, events, cx) = picker(cx, branches());
        let trigger = view.read_with(cx, |picker, _| picker.trigger_focus.clone());
        focus_and_draw(cx, &trigger);
        press_key(cx, "down");
        draw(cx);
        assert!(is_open(&view, cx));
        press_key(cx, "down");
        press_key(cx, "enter");
        assert_eq!(
            *events.borrow(),
            vec![PickerEvent::Chosen("feature/map".into())]
        );
    }

    #[gpui::test]
    fn escape_closes_the_panel_with_a_query_and_returns_focus_to_the_trigger(
        cx: &mut TestAppContext,
    ) {
        let (view, events, cx) = picker(cx, branches());
        open_by_click(cx);
        cx.simulate_input("ma");
        let typed = view.read_with(cx, |picker, cx| picker.search.read(cx).text(cx).to_owned());
        assert_eq!(typed, "ma");
        press_key(cx, "escape");
        assert!(!is_open(&view, cx));
        let trigger = view.read_with(cx, |picker, _| picker.trigger_focus.clone());
        cx.update(|window, _| assert!(trigger.is_focused(window)));
        assert!(events.borrow().is_empty(), "nothing was chosen");
    }

    #[gpui::test]
    fn typing_filters_the_entries_after_the_pause(cx: &mut TestAppContext) {
        let clock = cx.executor();
        let (view, events, cx) = picker(cx, branches());
        open_by_click(cx);
        cx.simulate_input("ma");
        clock.advance_clock(Duration::from_millis(SEARCH_DEBOUNCE_MS));
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |picker, _| picker.state().entries().len()),
            2,
            "main and feature/map match"
        );
        assert!(events.borrow().contains(&PickerEvent::Query("ma".into())));
    }

    #[gpui::test]
    fn an_empty_list_says_why_and_loading_shows_placeholders(cx: &mut TestAppContext) {
        let (view, _events, cx) = picker(cx, PickerState::new("branches", Vec::new()));
        open_by_click(cx);
        assert!(cx.debug_bounds("picker-empty").is_some());
        view.update(cx, |picker, cx| {
            picker.set_status(PickerStatus::Loading, cx)
        });
        draw(cx);
        assert!(cx.debug_bounds("picker-loading").is_some());
        assert!(cx.debug_bounds("picker-empty").is_none());
    }

    #[gpui::test]
    fn an_error_keeps_the_last_rows_and_retry_reports_retry(cx: &mut TestAppContext) {
        let (view, events, cx) = picker(cx, branches());
        open_by_click(cx);
        view.update(cx, |picker, cx| picker.set_status(PickerStatus::Error, cx));
        draw(cx);
        assert!(
            cx.debug_bounds("picker-row-0").is_some(),
            "the last list stays above the error"
        );
        let retry = centre(cx, "error-retry");
        cx.simulate_click(retry, Modifiers::none());
        assert_eq!(*events.borrow(), vec![PickerEvent::Retry]);
    }

    #[gpui::test]
    fn right_enters_the_active_folder_and_the_panel_stays_open(cx: &mut TestAppContext) {
        let (view, events, cx) = picker(cx, folders());
        open_by_click(cx);
        press_key(cx, "down");
        press_key(cx, "right");
        assert_eq!(
            *events.borrow(),
            vec![PickerEvent::Navigate("/projects/massorca/crates".into())]
        );
        assert!(is_open(&view, cx));
    }

    #[gpui::test]
    fn the_create_row_reports_the_typed_name(cx: &mut TestAppContext) {
        let clock = cx.executor();
        let (_view, events, cx) = picker(cx, branches().creatable("Create branch"));
        open_by_click(cx);
        cx.simulate_input("feat/x");
        clock.advance_clock(Duration::from_millis(SEARCH_DEBOUNCE_MS));
        cx.run_until_parked();
        draw(cx);
        let create = centre(cx, "picker-row-0");
        cx.simulate_click(create, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            events.borrow().last(),
            Some(&PickerEvent::Created("feat/x".into()))
        );
    }
}
