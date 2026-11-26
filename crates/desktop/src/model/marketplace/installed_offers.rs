//! What an installed row's More menu offers: only the actions that work for its item (teams-extensions-settings.md 7.4).

use super::types::{MarketplaceItem, MarketplaceSnapshot};

/// Each More menu action, offered only when it works for the item now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowOffers {
    /// Updates a host program MassOrca installed (decision 0151).
    pub update: bool,
    /// Rolls back to the version an update replaced, when the service keeps it.
    pub rollback: bool,
    /// Shows the package's run log.
    pub log: bool,
    /// Removes the item: a package on a connected service, or any sample item while nothing is connected.
    pub remove: bool,
}

/// The offers of one installed item. `updatable` is the program update check for it; `connected` is whether a
/// service answers, since only a service can change the packages it lists.
pub fn row_offers(
    item: &MarketplaceItem,
    snapshot: &MarketplaceSnapshot,
    updatable: bool,
    connected: bool,
) -> RowOffers {
    let package = item.extension.is_some();
    RowOffers {
        update: updatable,
        rollback: package && snapshot.rollback_offered && item.previous_version.is_some(),
        log: package && snapshot.log_offered,
        remove: package || !connected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::marketplace::extension_items::tests::{items, package};
    use massorca_protocol::methods::extensions::ExtensionState;

    fn snapshot(rollback: bool, log: bool) -> MarketplaceSnapshot {
        MarketplaceSnapshot {
            rollback_offered: rollback,
            log_offered: log,
            ..MarketplaceSnapshot::default()
        }
    }

    fn installed_package() -> MarketplaceItem {
        let entry = package("acme.notes", "extensions", ExtensionState::Enabled);
        items(&[entry], &[], &[]).remove(0)
    }

    #[test]
    fn a_package_offers_only_what_the_service_can_do() {
        let mut item = installed_package();
        item.previous_version = Some("0.9.0".to_owned());
        let offers = row_offers(&item, &snapshot(true, true), false, true);
        assert_eq!(
            offers,
            RowOffers {
                update: false,
                rollback: true,
                log: true,
                remove: true,
            }
        );
        let plain = row_offers(&item, &snapshot(false, false), false, true);
        assert!(!plain.rollback && !plain.log && plain.remove);
    }

    #[test]
    fn an_agent_without_a_package_cannot_be_removed_or_logged_on_a_connected_service() {
        let mut item = installed_package();
        item.extension = None;
        item.previous_version = Some("0.9.0".to_owned());
        let offers = row_offers(&item, &snapshot(true, true), false, true);
        assert_eq!(offers, RowOffers::default());
    }

    #[test]
    fn a_sample_item_stays_removable_while_nothing_is_connected() {
        let mut item = installed_package();
        item.extension = None;
        let offers = row_offers(&item, &snapshot(true, true), false, false);
        assert!(offers.remove);
    }

    #[test]
    fn update_follows_the_program_update_check() {
        let item = installed_package();
        assert!(row_offers(&item, &MarketplaceSnapshot::default(), true, true).update);
    }
}
