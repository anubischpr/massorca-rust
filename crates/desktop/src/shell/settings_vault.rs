//! The secret vault's page in Settings (decision 0071): the title with Add secret at the top right, the search
//! and the scope filter, then one card per scope. Only names and times are drawn; the store never holds a value.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, SharedString, div, px};
use massorca_ui::components::badge::Badge;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::tokens::{
    RADIUS_LG, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_MONO_SM,
    TYPE_TITLE_SM,
};
use massorca_ui::{ActiveTheme, Icon, IconName, IconSize, TextStyled, Theme};

use super::SettingsFrame;
use crate::model::secret_vault::{
    ScopeGroup, SecretRow, VaultPage, VaultScopeFilter, filtered_groups, group_count_line,
};
use crate::model::settings_sections::SettingsSection;
use crate::shell::Shell;

/// The page: the header, the toolbar, the status line, then the cards of the scopes the filter keeps.
pub(super) fn vault_page(frame: &SettingsFrame<'_>, cx: &mut App) -> Vec<AnyElement> {
    let theme = *cx.theme();
    let mut page = vec![header(frame, &theme), toolbar(frame, &theme)];
    page.extend(status_line(frame, &theme));
    let VaultPage::Listed(listing) = &frame.vault else {
        return page;
    };
    if listing.groups.is_empty() {
        page.push(
            EmptyState::new("settings-vault-empty", "No secrets yet")
                .note("Add a secret for an agent to use. It never appears in a chat.")
                .into_any_element(),
        );
        return page;
    }
    let groups = filtered_groups(
        &listing.groups,
        &frame.model.vault_query,
        frame.model.vault_scope,
    );
    if groups.is_empty() {
        page.push(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child("No name matches the search and the scope.")
                .into_any_element(),
        );
    }
    page.extend(groups.iter().map(|group| scope_card(group, frame, &theme)));
    page
}

/// The title, its note, and Add secret at the top right. Add is off until the service is connected.
fn header(frame: &SettingsFrame<'_>, theme: &Theme) -> AnyElement {
    let connected = frame.connection.is_connected();
    let shell = frame.shell.clone();
    let add = Button::new("settings-vault-add", "Add secret")
        .variant(ButtonVariant::Primary)
        .leading_icon(IconName::Plus)
        .disabled(!connected)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_add_secret(cx));
        });
    let note = SettingsSection::Vault.note().unwrap_or_default();
    div()
        .flex()
        .items_start()
        .gap(px(SPACE_3))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(SPACE_1))
                .child(
                    div()
                        .text_style(TYPE_TITLE_SM)
                        .text_color(theme.palette.text.primary)
                        .child("Secret vault"),
                )
                .child(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(theme.palette.text.secondary)
                        .child(note),
                ),
        )
        .child(add)
        .into_any_element()
}

/// The search over names, the scope filter, and the count of names and scopes at the right end.
fn toolbar(frame: &SettingsFrame<'_>, theme: &Theme) -> AnyElement {
    let segments: Vec<Segment> = VaultScopeFilter::ALL
        .iter()
        .map(|filter| Segment::text(filter.label()))
        .collect();
    let selected = VaultScopeFilter::ALL
        .iter()
        .position(|filter| *filter == frame.model.vault_scope)
        .unwrap_or(0);
    let shell = frame.shell.clone();
    let scope = SegmentedControl::new("settings-vault-scope", "Scope", segments)
        .selected(selected)
        .on_change(move |index, _, cx| {
            let Some(filter) = VaultScopeFilter::ALL.get(index).copied() else {
                return;
            };
            let _ = shell.update(cx, |shell, cx| shell.set_vault_scope(filter, cx));
        });
    let counts = match &frame.vault {
        VaultPage::Listed(listing) => listing.counts.clone(),
        VaultPage::Reading | VaultPage::Unavailable(_) => String::new(),
    };
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .child(div().w(px(260.)).child(frame.vault_search.clone()))
        .child(scope)
        .child(
            div()
                .ml_auto()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child(counts),
        )
        .into_any_element()
}

/// What the page says while the vault is read, refused or locked, and why Add secret is off.
fn status_line(frame: &SettingsFrame<'_>, theme: &Theme) -> Option<AnyElement> {
    let text = match &frame.vault {
        VaultPage::Reading => Some("Reading the vault.".to_owned()),
        VaultPage::Unavailable(message) => Some(message.clone()),
        VaultPage::Listed(listing) if listing.locked => {
            Some("The vault is locked: its key will not open.".to_owned())
        }
        VaultPage::Listed(_) => None,
    };
    let text = text.or_else(|| {
        (!frame.connection.is_connected())
            .then(|| "Can't reach This PC. Nothing can be saved until it is back.".to_owned())
    })?;
    Some(
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.secondary)
            .child(text)
            .into_any_element(),
    )
}

/// One scope's card: its icon and title with the count, then each name in it.
fn scope_card(group: &ScopeGroup, frame: &SettingsFrame<'_>, theme: &Theme) -> AnyElement {
    let p = &theme.palette;
    let header = div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .child(
            Icon::new(scope_icon(&group.scope))
                .size(IconSize::Small)
                .color(p.text.secondary),
        )
        .child(
            div()
                .text_style(TYPE_BODY_STRONG)
                .text_color(p.text.primary)
                .child(group.title.clone()),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.tertiary)
                .child(format!("· {}", group_count_line(group))),
        );
    let mut card = div()
        .flex()
        .flex_col()
        .bg(p.surface.raised)
        .border_1()
        .border_color(p.border.default)
        .rounded(px(RADIUS_LG))
        .child(header);
    for row in &group.rows {
        card = card.child(name_row(row, &group.title, frame, theme));
    }
    card.into_any_element()
}

/// A project's folder, a team's mark, or the globe for Global.
fn scope_icon(scope: &str) -> IconName {
    if scope.starts_with("project:") {
        IconName::Folder
    } else if scope.starts_with("team:") {
        IconName::Users
    } else {
        IconName::Globe
    }
}

/// One name: the name in mono, its owner or link pill, its dates, and for an owned name Rotate and a trash icon.
fn name_row(
    row: &SecretRow,
    scope_title: &str,
    frame: &SettingsFrame<'_>,
    theme: &Theme,
) -> AnyElement {
    let p = &theme.palette;
    let connected = frame.connection.is_connected();
    let pill = match &row.linked_from {
        Some(owner) => format!("Linked from {owner}"),
        None => format!("Owner · {scope_title}"),
    };
    let mut actions = div().flex().items_center().gap(px(SPACE_2));
    if row.linked_from.is_none() {
        actions = actions
            .child(rotate_button(&row.scope, &row.name, frame, connected))
            .child(delete_button(
                &row.scope,
                &row.name,
                &row.linked_scopes,
                frame,
                connected,
            ));
    }
    let selector = format!("settings-vault-{}-{}", row.scope, row.name);
    div()
        .debug_selector(move || selector)
        .flex()
        .items_center()
        .gap(px(SPACE_3))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .border_t_1()
        .border_color(p.border.subtle)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(SPACE_1))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(SPACE_2))
                        .child(
                            div()
                                .text_style(TYPE_MONO_SM)
                                .text_color(p.text.primary)
                                .child(row.name.clone()),
                        )
                        .child(Badge::text(pill)),
                )
                .child(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(p.text.secondary)
                        .child(row.detail.clone()),
                ),
        )
        .child(actions)
        .into_any_element()
}

/// Rotate: opens the dialog that replaces the owned value. Off while the service is not connected.
fn rotate_button(
    scope: &str,
    name: &str,
    frame: &SettingsFrame<'_>,
    connected: bool,
) -> AnyElement {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    let shell = frame.shell.clone();
    Button::new(
        SharedString::from(format!("settings-vault-rotate-{scope}-{name}")),
        "Rotate",
    )
    .variant(ButtonVariant::Secondary)
    .leading_icon(IconName::RefreshCw)
    .disabled(!connected)
    .on_click(move |_, cx| {
        let _ = shell.update(cx, |shell, cx| {
            shell.open_rotate_secret(scope.clone(), name.clone(), cx)
        });
    })
    .into_any_element()
}

/// Delete: a trash icon, named for its secret, that opens the confirm naming the scopes losing it.
fn delete_button(
    scope: &str,
    name: &str,
    linked_scopes: &[String],
    frame: &SettingsFrame<'_>,
    connected: bool,
) -> AnyElement {
    let (scope, name, linked) = (scope.to_owned(), name.to_owned(), linked_scopes.to_vec());
    let selector = format!("settings-vault-delete-{scope}-{name}");
    let shell = frame.shell.clone();
    let button = IconButton::new(
        SharedString::from(selector.clone()),
        IconName::Trash2,
        format!("Delete {name}"),
    )
    .disabled(!connected)
    .on_click(move |_, cx| {
        let _ = shell.update(cx, |shell, cx| {
            shell.open_remove_secret(scope.clone(), name.clone(), linked.clone(), cx)
        });
    });
    div()
        .debug_selector(move || selector)
        .child(button)
        .into_any_element()
}

impl Shell {
    /// The search over names: the text the field settled on.
    pub(in crate::shell) fn set_vault_query(&mut self, text: &str, cx: &mut Context<Self>) {
        self.settings.vault_query = text.to_owned();
        cx.notify();
    }

    /// The scope kinds the page shows.
    pub(in crate::shell) fn set_vault_scope(
        &mut self,
        filter: VaultScopeFilter,
        cx: &mut Context<Self>,
    ) {
        self.settings.vault_scope = filter;
        cx.notify();
    }
}
