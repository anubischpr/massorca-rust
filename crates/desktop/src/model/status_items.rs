//! Which Status bar items a window shows, as its visibility menu sets them (StatusBar spec,
//! D-SH-05). The choice lives in the shell state, so each window keeps its own.

/// One item of the Status bar, in the order the bar lays them out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusItem {
    Agents,
    Memory,
    Terminals,
    Ports,
    Hosts,
}

impl StatusItem {
    /// Every item, in bar order.
    pub const ALL: [StatusItem; 5] = [
        StatusItem::Agents,
        StatusItem::Memory,
        StatusItem::Terminals,
        StatusItem::Ports,
        StatusItem::Hosts,
    ];

    /// The name the visibility menu lists the item under.
    pub fn label(self) -> &'static str {
        match self {
            StatusItem::Agents => "Agent status",
            StatusItem::Memory => "Memory",
            StatusItem::Terminals => "Terminals",
            StatusItem::Ports => "Ports",
            StatusItem::Hosts => "Hosts",
        }
    }
}

/// The items a window hides. Every item can be hidden: the Status bar spec names none that
/// cannot be, and a hidden item stays in the visibility menu.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatusItemVisibility {
    hidden: Vec<StatusItem>,
}

impl StatusItemVisibility {
    /// The visibility that shows exactly `shown` and hides the rest.
    pub fn showing(shown: &[StatusItem]) -> Self {
        Self {
            hidden: StatusItem::ALL
                .into_iter()
                .filter(|item| !shown.contains(item))
                .collect(),
        }
    }

    pub fn is_shown(&self, item: StatusItem) -> bool {
        !self.hidden.contains(&item)
    }

    /// Hides a shown item, or shows a hidden one.
    pub fn toggle(&mut self, item: StatusItem) {
        match self.hidden.iter().position(|&hidden| hidden == item) {
            Some(index) => {
                self.hidden.remove(index);
            }
            None => self.hidden.push(item),
        }
    }
}

/// The items the visibility menu lists: the bar's five items for every data source. The argument
/// says whether the data comes from a service; the service link is no bar item of its own now
/// (it shows in the hosts item), so the list does not depend on it.
pub fn visibility_menu_items(_service_linked: bool) -> Vec<StatusItem> {
    StatusItem::ALL.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showing_hides_exactly_the_items_left_out() {
        let visibility = StatusItemVisibility::showing(&[StatusItem::Ports, StatusItem::Agents]);
        assert!(visibility.is_shown(StatusItem::Agents));
        assert!(visibility.is_shown(StatusItem::Ports));
        assert!(!visibility.is_shown(StatusItem::Memory));
        assert!(!visibility.is_shown(StatusItem::Hosts));
    }

    #[test]
    fn every_item_is_shown_until_it_is_hidden() {
        let mut visibility = StatusItemVisibility::default();
        assert!(
            StatusItem::ALL
                .into_iter()
                .all(|item| visibility.is_shown(item))
        );
        visibility.toggle(StatusItem::Memory);
        assert!(!visibility.is_shown(StatusItem::Memory));
        assert!(visibility.is_shown(StatusItem::Ports));
    }

    #[test]
    fn toggling_twice_shows_the_item_again() {
        let mut visibility = StatusItemVisibility::default();
        visibility.toggle(StatusItem::Hosts);
        visibility.toggle(StatusItem::Hosts);
        assert_eq!(visibility, StatusItemVisibility::default());
    }

    #[test]
    fn the_menu_lists_the_bar_items_for_every_data_source() {
        assert_eq!(visibility_menu_items(false), StatusItem::ALL.to_vec());
        assert_eq!(visibility_menu_items(true), StatusItem::ALL.to_vec());
    }

    #[test]
    fn the_menu_labels_follow_the_bar_copy() {
        let labels: Vec<&str> = StatusItem::ALL.into_iter().map(StatusItem::label).collect();
        assert_eq!(
            labels,
            ["Agent status", "Memory", "Terminals", "Ports", "Hosts"]
        );
    }
}
