//! The secret vault's rows in Settings (decision 0071): the key and the count in the status row, then every
//! name under its scope. Only names and times are drawn; the store never holds a value, so none is drawn.

use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use massorca_ui::Theme;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::settings_row::RowStatus;
use massorca_ui::components::settings_row::{SettingsRow, SettingsSection as SettingsGroup};
use massorca_ui::tokens::SPACE_2;

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

/// The Add row, then every scope's names, or the empty state when the vault holds none. While the vault is read
/// only the Add row is drawn.
pub(super) fn secret_scopes(frame: &SettingsFrame<'_>) -> Vec<AnyElement> {
    let mut groups = vec![add_group(frame)];
    let VaultPage::Listed(listing) = &frame.vault else {
        return groups;
    };
    if listing.groups.is_empty() {
        groups.push(
            EmptyState::new("settings-vault-empty", "No secrets yet")
                .note("Add a secret for an agent to use. It never appears in a chat.")
                .into_any_element(),
        );
        return groups;
    }
    groups.extend(listing.groups.iter().map(|group| scope_group(group, frame)));
    groups
}

/// The Add row: the dialog chooses the scope. Off while the service is not connected.
fn add_group(frame: &SettingsFrame<'_>) -> AnyElement {
    let connected = frame.connection.is_connected();
    let shell = frame.shell.clone();
    let button = Button::new("settings-vault-add", "Add secret")
        .variant(ButtonVariant::Primary)
        .disabled(!connected)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_add_secret(cx));
        });
    let mut row = SettingsRow::new("settings-vault-add-row", "Add a secret")
        .helper(
            "Agents get it as an environment variable when they run. It is stored on This PC and never goes into the chat.",
        )
        .control(button);
    if !connected {
        row = row.disabled_reason("Can't reach This PC. Nothing can be saved until it is back.");
    }
    SettingsGroup::new("settings-vault-add-group", "Secrets")
        .row(row)
        .into_any_element()
}

/// One scope's names, each with its dates and, for a link, the scope that owns the value. An owned name can be
/// rotated here; a link is read-only.
fn scope_group(group: &ScopeGroup, frame: &SettingsFrame<'_>) -> AnyElement {
    let mut section = SettingsGroup::new(
        SharedString::from(format!("settings-vault-{}", group.scope)),
        group.title.clone(),
    );
    for row in &group.rows {
        let mut item = name_row(&group.scope, row);
        if row.linked_from.is_none() {
            let actions = div()
                .flex()
                .flex_row()
                .gap(px(SPACE_2))
                .child(rotate_button(&group.scope, &row.name, frame))
                .child(delete_button(
                    &group.scope,
                    &row.name,
                    &row.linked_scopes,
                    frame,
                ));
            item = item.control(actions);
        }
        let selector = format!("settings-vault-{}-{}", group.scope, row.name);
        section = section.row(div().debug_selector(move || selector).child(item));
    }
    section.into_any_element()
}

/// Rotate: opens the dialog that replaces the owned value. Off while the service is not connected.
fn rotate_button(scope: &str, name: &str, frame: &SettingsFrame<'_>) -> AnyElement {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    let shell = frame.shell.clone();
    Button::new(
        SharedString::from(format!("settings-vault-rotate-{scope}-{name}")),
        "Rotate",
    )
    .variant(ButtonVariant::Secondary)
    .disabled(!frame.connection.is_connected())
    .on_click(move |_, cx| {
        let _ = shell.update(cx, |shell, cx| {
            shell.open_rotate_secret(scope.clone(), name.clone(), cx)
        });
    })
    .into_any_element()
}

/// Delete: opens the confirm that names the scopes losing the secret. Off while the service is not connected.
fn delete_button(
    scope: &str,
    name: &str,
    linked_scopes: &[String],
    frame: &SettingsFrame<'_>,
) -> AnyElement {
    let (scope, name, linked) = (scope.to_owned(), name.to_owned(), linked_scopes.to_vec());
    let shell = frame.shell.clone();
    Button::new(
        SharedString::from(format!("settings-vault-delete-{scope}-{name}")),
        "Delete",
    )
    .variant(ButtonVariant::Danger)
    .disabled(!frame.connection.is_connected())
    .on_click(move |_, cx| {
        let _ = shell.update(cx, |shell, cx| {
            shell.open_remove_secret(scope.clone(), name.clone(), linked.clone(), cx)
        });
    })
    .into_any_element()
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
