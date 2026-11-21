//! Menu and context menu: actions or options in an anchored overlay (docs/design/components/Menu.md).
//! The menu is an entity that owns the keyboard model: Up and Down move the highlight, Home and
//! End jump to the ends, a letter jumps to the next row that starts with it, Right and Left open
//! and close one submenu level, Enter or Space chooses, and Escape closes one level and returns
//! focus to the opener. Items do not take focus one by one; the panel does.

use crate::icon::IconName;
use gpui::SharedString;

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
