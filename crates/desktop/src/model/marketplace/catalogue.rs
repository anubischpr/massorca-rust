//! What each catalogue view lists and says: Discover, Installed, Updates and the categories
//! (teams-extensions-settings.md 7.1 and 7.4).

use super::types::{ItemState, MarketplaceItem, MarketplaceSnapshot};
use crate::model::navigation::MarketplacePage;

/// The items a view lists: its own set, filtered by the search text and sorted by name.
pub fn listed<'a>(
    snapshot: &'a MarketplaceSnapshot,
    page: MarketplacePage,
    query: &str,
) -> Vec<&'a MarketplaceItem> {
    snapshot
        .search(query)
        .into_iter()
        .filter(|item| in_view(item, page))
        .collect()
}

/// Whether an item belongs to a view. An item page is not a list, so it lists like Discover.
fn in_view(item: &MarketplaceItem, page: MarketplacePage) -> bool {
    match page {
        MarketplacePage::Discover | MarketplacePage::Item(_) => true,
        MarketplacePage::Installed => item.state.is_installed(),
        MarketplacePage::Updates => {
            matches!(
                item.state,
                ItemState::Installed {
                    update: Some(_),
                    ..
                }
            )
        }
        MarketplacePage::Category(kind) => item.kind == kind,
    }
}

/// The heading of a view.
pub fn heading(page: MarketplacePage) -> &'static str {
    match page {
        MarketplacePage::Discover | MarketplacePage::Item(_) => "Discover",
        MarketplacePage::Installed => "Installed",
        MarketplacePage::Updates => "Updates",
        MarketplacePage::Category(kind) => kind.plural(),
    }
}

/// The line under the heading. Discover and Installed use the artboards' copy
/// (bt-marketplace-catalogue.dc.html:120, bt-extension-manager.dc.html:124).
pub fn intro(page: MarketplacePage) -> &'static str {
    match page {
        MarketplacePage::Discover | MarketplacePage::Item(_) => {
            "Extensions, agents, skills and services from your sources. Each one asks for permissions before it is installed."
        }
        MarketplacePage::Installed => {
            "Everything installed from the Marketplace, created in Creator mode or added from a third party. Disabling keeps the files; removing deletes them and their settings."
        }
        MarketplacePage::Updates => "Installed items with a newer version waiting.",
        MarketplacePage::Category(_) => "Items in this category from your sources.",
    }
}

/// The count line under the search field, such as "8 items · sorted by name".
pub fn count_line(count: usize) -> String {
    let noun = if count == 1 { "item" } else { "items" };
    format!("{count} {noun} · sorted by name")
}

/// The empty text when a search matches nothing (teams-extensions-settings.md 7.1).
pub fn no_match_text(query: &str) -> String {
    format!("Nothing matches \"{}\".", query.trim())
}

/// The counts beside Installed and Updates in the left navigation.
pub fn counts(snapshot: &MarketplaceSnapshot) -> (usize, usize) {
    (snapshot.installed().len(), snapshot.updates().len())
}

/// The status line of an installed item's row (teams-extensions-settings.md 7.4), or None when it is not installed.
pub fn status_word(state: &ItemState) -> Option<String> {
    match state {
        ItemState::Installed {
            update: Some(version),
            ..
        } => Some(format!("Update available {version}")),
        ItemState::Installed { enabled: true, .. } => Some("Enabled".to_owned()),
        ItemState::Installed { enabled: false, .. } => Some("Disabled".to_owned()),
        ItemState::Installing { percent } => Some(format!("Installing, {percent}%")),
        ItemState::Failed { .. } => Some("Failed to start".to_owned()),
        ItemState::NotInstalled | ItemState::Incompatible { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::marketplace::{ItemKind, MarketplaceSource, SampleMarketplaceSource};

    fn sample() -> MarketplaceSnapshot {
        SampleMarketplaceSource::default().snapshot()
    }

    fn names(items: &[&MarketplaceItem]) -> Vec<String> {
        items.iter().map(|item| item.name.clone()).collect()
    }

    #[test]
    fn a_row_words_its_status_from_the_item_state() {
        let installed = |enabled, update: Option<&str>| ItemState::Installed {
            enabled,
            update: update.map(str::to_owned),
        };
        assert_eq!(
            status_word(&installed(true, None)).as_deref(),
            Some("Enabled")
        );
        assert_eq!(
            status_word(&installed(false, None)).as_deref(),
            Some("Disabled")
        );
        assert_eq!(
            status_word(&installed(true, Some("2.1.0"))).as_deref(),
            Some("Update available 2.1.0"),
            "a waiting update is the status, whether or not the item runs"
        );
        assert_eq!(
            status_word(&ItemState::Installing { percent: 40 }).as_deref(),
            Some("Installing, 40%")
        );
        assert_eq!(
            status_word(&ItemState::Failed {
                reason: "The CLI did not start".into()
            })
            .as_deref(),
            Some("Failed to start")
        );
        assert_eq!(status_word(&ItemState::NotInstalled), None);
    }

    #[test]
    fn installed_lists_only_installed_items_by_name() {
        assert_eq!(
            names(&listed(&sample(), MarketplacePage::Installed, "")),
            [
                "Claude",
                "Codex",
                "Model usage panel",
                "Quick notes",
                "Workspace board"
            ]
        );
    }

    #[test]
    fn updates_lists_the_installed_items_with_a_waiting_version() {
        assert_eq!(
            names(&listed(&sample(), MarketplacePage::Updates, "")),
            ["Model usage panel"]
        );
    }

    #[test]
    fn a_category_lists_only_its_kind() {
        let snapshot = sample();
        assert_eq!(
            names(&listed(
                &snapshot,
                MarketplacePage::Category(ItemKind::Agent),
                ""
            )),
            ["Claude", "Codex"]
        );
        assert_eq!(
            names(&listed(
                &snapshot,
                MarketplacePage::Category(ItemKind::Skill),
                ""
            )),
            ["Release notes"]
        );
    }

    #[test]
    fn the_search_text_narrows_every_view() {
        let snapshot = sample();
        assert_eq!(
            names(&listed(&snapshot, MarketplacePage::Discover, " github ")),
            ["GitHub"]
        );
        assert_eq!(
            names(&listed(&snapshot, MarketplacePage::Installed, "quick")),
            ["Quick notes"]
        );
        assert!(listed(&snapshot, MarketplacePage::Installed, "github").is_empty());
    }

    #[test]
    fn headings_counts_and_the_empty_text_use_the_spec_words() {
        assert_eq!(heading(MarketplacePage::Discover), "Discover");
        assert_eq!(
            heading(MarketplacePage::Category(ItemKind::Agent)),
            "Agents"
        );
        assert_eq!(counts(&sample()), (5, 1));
        assert_eq!(count_line(1), "1 item · sorted by name");
        assert_eq!(count_line(8), "8 items · sorted by name");
        assert_eq!(no_match_text(" xyz "), "Nothing matches \"xyz\".");
    }
}
