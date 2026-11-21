//! Picker: one searchable choice from a list, or a folder to browse
//! (docs/design/components/Picker.md).
//!
//! `PickerState` holds the rules without a window: the items and their groups, the query
//! and the rows it leaves, the active row, the chosen value and the folder being shown.
//! `PickerView` draws it in an anchored popover and turns keys and clicks into events.

use crate::components::file_tree::matches_query;
use crate::components::list_row::{ListMove, move_index};
use gpui::SharedString;
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
            let (label, matches, selected) = match entry {
                PickerEntry::Parent => ("..".into(), Vec::new(), false),
                PickerEntry::Item(index) => {
                    let item = &self.items[*index];
                    if let Some(name) = &item.group
                        && item.group.as_ref() != group
                    {
                        rows.push(PickerRow::Group(name.clone()));
                    }
                    group = item.group.as_ref();
                    let selected = self.selected.as_ref() == Some(&item.key);
                    (item.label.clone(), self.match_ranges(&item.label), selected)
                }
                PickerEntry::Create(name) => (self.create_text(name), Vec::new(), false),
            };
            rows.push(PickerRow::Entry {
                entry: entry.clone(),
                label,
                matches,
                active: position == self.active,
                selected,
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
