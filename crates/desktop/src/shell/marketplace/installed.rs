//! The Installed list: one manager row per installed extension, in one group card, each with its More menu
//! (teams-extensions-settings.md 7.4).

use super::MarketplaceFrame;
use super::catalogue::{card_state, item_mark, kind_icon};
use super::more_menu::{self, MoreMenu};
use crate::model::marketplace::{MarketplaceItem, status_word};
use crate::model::navigation::MarketplacePage;
use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, Role, SharedString, Styled, Window, div,
    prelude::*, px,
};
use massorca_ui::ActiveTheme;
use massorca_ui::IconName;
use massorca_ui::components::extension_card::ExtensionAction;
use massorca_ui::components::extension_manager_row::ExtensionManagerRow;
use massorca_ui::components::menu::{MenuEntry, MenuItem};
use massorca_ui::tokens::RADIUS_LG;

/// The rows of the Installed list in one group card; each row after the first sits under a hairline.
pub(super) fn installed_rows(
    frame: &MarketplaceFrame,
    items: &[&MarketplaceItem],
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let p = theme.palette;
    let mut group = div()
        .id("marketplace-installed")
        .flex()
        .flex_col()
        .rounded(px(RADIUS_LG))
        .border_1()
        .border_color(p.border.default)
        .bg(p.surface.raised)
        .role(Role::List)
        .aria_label("Installed extensions");
    for (index, item) in items.iter().enumerate() {
        let id = item.id.0;
        let menu = window.use_keyed_state(
            SharedString::from(format!("marketplace-row-menu-{id}")),
            cx,
            |_, _| MoreMenu::default(),
        );
        let mut cell = div()
            .relative()
            .debug_selector(move || format!("marketplace-row-{id}"));
        if index > 0 {
            cell = cell.border_t_1().border_color(p.border.subtle);
        }
        cell = cell.child(manager_row(frame, item, &menu));
        if let Some(overlay) = MoreMenu::overlay(&menu, cx) {
            cell = cell.child(div().absolute().child(overlay));
        }
        group = group.child(cell);
    }
    group.into_any_element()
}

/// One installed item as a row: its name opens the item page, its switch enables or disables it, and its More button
/// opens the row's menu.
fn manager_row(
    frame: &MarketplaceFrame,
    item: &MarketplaceItem,
    menu: &Entity<MoreMenu>,
) -> ExtensionManagerRow {
    let id = item.id;
    let shell = frame.shell.clone();
    let menu = menu.clone();
    let mut row = ExtensionManagerRow::new(
        SharedString::from(format!("marketplace-row-{}", item.id.0)),
        item.name.clone(),
        card_state(item),
    )
    .publisher(item.publisher.clone())
    .version(item.version.clone())
    .contribution(item.adds.join(", "))
    .status_word(status_word(&item.state).unwrap_or_default())
    .icon(kind_icon(item.kind));
    if let Some(mark) = item_mark(item) {
        row = row.agent(mark);
    }
    row.on_action(move |action, window, cx| match action {
        ExtensionAction::Toggle(enabled) => {
            let _ = shell.update(cx, |shell, cx| {
                shell.set_marketplace_enabled(id, enabled, cx)
            });
        }
        ExtensionAction::More => {
            // Remove is the only entry, so any choice asks for it.
            let remove = shell.clone();
            more_menu::open(
                &menu,
                shell.clone(),
                row_menu_entries(),
                move |_, _, cx| {
                    let _ = remove.update(cx, |shell, cx| shell.ask_marketplace_uninstall(id, cx));
                },
                window,
                cx,
            );
        }
        _ => {
            let _ = shell.update(cx, |shell, cx| {
                shell.open_marketplace(MarketplacePage::Item(id), cx)
            });
        }
    })
}

/// The entries of an installed row's menu: Remove only. Roll back and View log join it when extensions.rollback and
/// extensions.log exist, so the menu never lists an action that does not work.
fn row_menu_entries() -> Vec<MenuEntry> {
    vec![
        MenuItem::new("Remove")
            .icon(IconName::Trash2)
            .destructive()
            .into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_menu_lists_only_remove() {
        let entries = row_menu_entries();
        let [MenuEntry::Item(remove)] = entries.as_slice() else {
            panic!("the row menu lists exactly one entry, Remove");
        };
        assert_eq!(remove.label().to_string(), "Remove");
        assert!(!remove.is_disabled(), "Remove works, so it is not disabled");
    }
}
