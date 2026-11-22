//! The secret vault's rows in Settings (decision 0071): the key and the count in the status row, then every
//! name under its scope. Only names and times are drawn; the store never holds a value, so none is drawn.

use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div};
use massorca_ui::Theme;
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::settings_row::RowStatus;
use massorca_ui::components::settings_row::{SettingsRow, SettingsSection as SettingsGroup};

use super::{SettingsFrame, value_box};
use crate::model::secret_vault::{ScopeGroup, SecretRow, VaultPage};

/// The status row: the key is its helper and the count its control. A refusal is its status.
pub(super) fn secret_status_row(
    row: SettingsRow,
    frame: &SettingsFrame<'_>,
    theme: &Theme,
) -> (SettingsRow, Option<AnyElement>) {
    match &frame.vault {
        VaultPage::Reading => (row.helper("Reading the vault."), None),
        VaultPage::Unavailable(message) => {
            (row.status(RowStatus::Error(message.clone().into())), None)
        }
        VaultPage::Listed(listing) => (
            row.helper(listing.key.clone()),
            Some(value_box(listing.counts.clone(), theme)),
        ),
    }
}

/// Every scope's names, or the empty state when the vault holds none. Nothing is drawn while it is read.
pub(super) fn secret_scopes(frame: &SettingsFrame<'_>) -> Vec<AnyElement> {
    let VaultPage::Listed(listing) = &frame.vault else {
        return Vec::new();
    };
    if listing.groups.is_empty() {
        return vec![
            EmptyState::new("settings-vault-empty", "No secrets yet")
                .note("Add a secret for an agent to use. It never appears in a chat.")
                .into_any_element(),
        ];
    }
    listing.groups.iter().map(scope_group).collect()
}

/// One scope's names, each with its dates and, for a link, the scope that owns the value.
fn scope_group(group: &ScopeGroup) -> AnyElement {
    let mut section = SettingsGroup::new(
        SharedString::from(format!("settings-vault-{}", group.scope)),
        group.title.clone(),
    );
    for row in &group.rows {
        let selector = format!("settings-vault-{}-{}", group.scope, row.name);
        section = section.row(
            div()
                .debug_selector(move || selector)
                .child(name_row(&group.scope, row)),
        );
    }
    section.into_any_element()
}

fn name_row(scope: &str, row: &SecretRow) -> SettingsRow {
    let helper = match &row.linked_from {
        Some(owner) => format!("Linked from {owner} · {}", row.detail),
        None => row.detail.clone(),
    };
    SettingsRow::new(
        SharedString::from(format!("settings-vault-{scope}-{}", row.name)),
        row.name.clone(),
    )
    .helper(helper)
}
