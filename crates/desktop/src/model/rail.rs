//! The Activity rail's items: what each one is, what it opens, and the order it is laid out
//! and roved through (D-SH-09 to D-SH-16, handoff 5.3).

use super::navigation::{MarketplacePage, Page};

/// An item that opens a full page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailEntry {
    Tasks,
    Calendar,
    Marketplace,
    Settings,
}

impl RailEntry {
    pub fn page(self) -> Page {
        match self {
            RailEntry::Tasks => Page::Tasks,
            RailEntry::Calendar => Page::Calendar,
            RailEntry::Marketplace => Page::Marketplace(MarketplacePage::Discover),
            RailEntry::Settings => Page::Settings,
        }
    }

    /// Whether this entry is the one lit for `page`: every Marketplace view lights Marketplace.
    pub fn shows(self, page: Page) -> bool {
        match (self.page(), page) {
            (Page::Marketplace(_), Page::Marketplace(_)) => true,
            (target, current) => target == current,
        }
    }

    /// The name, which is also the tooltip.
    pub fn label(self) -> &'static str {
        match self {
            RailEntry::Tasks => "Tasks",
            RailEntry::Calendar => "Calendar",
            RailEntry::Marketplace => "Marketplace",
            RailEntry::Settings => "Settings",
        }
    }
}

/// One item of the rail, in layout order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailItem {
    /// The mode button: the current mode, which a press switches.
    Mode,
    Entry(RailEntry),
    /// The theme toggle, below the spacer.
    Theme,
    /// The usage ring of the provider at this position in the snapshot.
    Usage(usize),
}

/// The rail's items top to bottom: the mode and the top entries, the theme toggle, one ring
/// per provider, then Settings.
pub fn rail_items(usage_rings: usize) -> Vec<RailItem> {
    let mut items = vec![
        RailItem::Mode,
        RailItem::Entry(RailEntry::Tasks),
        RailItem::Entry(RailEntry::Calendar),
        RailItem::Entry(RailEntry::Marketplace),
        RailItem::Theme,
    ];
    items.extend((0..usage_rings).map(RailItem::Usage));
    items.push(RailItem::Entry(RailEntry::Settings));
    items
}

/// The theme toggle's tooltip: it names the theme the press switches to (D-SH-14).
pub fn theme_label(dark: bool) -> &'static str {
    if dark {
        "Switch to light theme"
    } else {
        "Switch to dark theme"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_are_laid_out_top_to_bottom_with_one_ring_per_provider() {
        assert_eq!(
            rail_items(2),
            vec![
                RailItem::Mode,
                RailItem::Entry(RailEntry::Tasks),
                RailItem::Entry(RailEntry::Calendar),
                RailItem::Entry(RailEntry::Marketplace),
                RailItem::Theme,
                RailItem::Usage(0),
                RailItem::Usage(1),
                RailItem::Entry(RailEntry::Settings),
            ]
        );
    }

    #[test]
    fn every_marketplace_view_lights_the_marketplace_entry_only() {
        let installed = Page::Marketplace(MarketplacePage::Installed);
        assert!(RailEntry::Marketplace.shows(installed));
        assert!(
            RailEntry::Marketplace.shows(Page::Marketplace(MarketplacePage::Item(
                crate::model::marketplace::MarketplaceItemId(1)
            )))
        );
        assert!(!RailEntry::Tasks.shows(installed));
        assert!(!RailEntry::Marketplace.shows(Page::Settings));
    }

    #[test]
    fn each_entry_opens_its_own_full_page() {
        assert_eq!(RailEntry::Tasks.page(), Page::Tasks);
        assert_eq!(RailEntry::Calendar.page(), Page::Calendar);
        assert_eq!(
            RailEntry::Marketplace.page(),
            Page::Marketplace(MarketplacePage::Discover)
        );
        assert_eq!(RailEntry::Settings.page(), Page::Settings);
        assert_eq!(RailEntry::Marketplace.label(), "Marketplace");
    }

    #[test]
    fn the_theme_toggle_names_the_other_theme() {
        assert_eq!(theme_label(true), "Switch to light theme");
        assert_eq!(theme_label(false), "Switch to dark theme");
    }
}
