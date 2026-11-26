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
    let (previous, rollback_offered, log_offered) = (
        item.previous_version.clone(),
        frame.snapshot.rollback_offered,
        frame.snapshot.log_offered,
    );
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
            let (entries, actions): (Vec<MenuEntry>, Vec<Option<RowAction>>) =
                row_menu(previous.as_deref(), rollback_offered, log_offered)
                    .into_iter()
                    .unzip();
            let chosen = shell.clone();
            more_menu::open(
                &menu,
                shell.clone(),
                entries,
                move |row, _, cx| match actions.get(row).copied().flatten() {
                    Some(RowAction::Rollback) => {
                        let _ =
                            chosen.update(cx, |shell, cx| shell.ask_marketplace_rollback(id, cx));
                    }
                    Some(RowAction::ViewLog) => {
                        let _ =
                            chosen.update(cx, |shell, cx| shell.ask_marketplace_run_log(id, cx));
                    }
                    Some(RowAction::Remove) => {
                        let _ =
                            chosen.update(cx, |shell, cx| shell.ask_marketplace_uninstall(id, cx));
                    }
                    None => {}
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

/// What choosing an installed row's menu entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowAction {
    Rollback,
    ViewLog,
    Remove,
}

/// The entries of an installed row's menu in the artboard's order, each with what choosing it does; a separator has
/// none. Roll back to the version an update replaced comes first and View log after it, each when the service offers
/// it (and a version is kept, for the roll back), so the menu never lists an action that does not work.
fn row_menu(
    previous: Option<&str>,
    rollback_offered: bool,
    log_offered: bool,
) -> Vec<(MenuEntry, Option<RowAction>)> {
    let mut menu: Vec<(MenuEntry, Option<RowAction>)> = Vec::new();
    if let (true, Some(version)) = (rollback_offered, previous) {
        let roll_back = MenuItem::new(format!("Roll back to {version}")).icon(IconName::History);
        menu.push((roll_back.into(), Some(RowAction::Rollback)));
    }
    if log_offered {
        let view_log = MenuItem::new("View log").icon(IconName::FileText);
        menu.push((view_log.into(), Some(RowAction::ViewLog)));
    }
    if !menu.is_empty() {
        menu.push((MenuEntry::Separator, None));
    }
    let remove = MenuItem::new("Remove").icon(IconName::Trash2).destructive();
    menu.push((remove.into(), Some(RowAction::Remove)));
    menu
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(menu: &[(MenuEntry, Option<RowAction>)]) -> Vec<String> {
        menu.iter()
            .filter_map(|(entry, _)| match entry {
                MenuEntry::Item(item) => Some(item.label().to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn roll_back_then_view_log_then_remove_in_the_artboard_order() {
        let menu = row_menu(Some("0.9.0"), true, true);
        assert_eq!(labels(&menu), ["Roll back to 0.9.0", "View log", "Remove"]);
        assert!(matches!(menu[0].1, Some(RowAction::Rollback)));
        assert!(matches!(menu[1].1, Some(RowAction::ViewLog)));
        assert!(matches!(menu[2].0, MenuEntry::Separator));
        assert!(matches!(menu[3].1, Some(RowAction::Remove)));
    }

    #[test]
    fn the_row_menu_lists_only_what_the_service_offers() {
        let cases: [(Option<&str>, bool, bool, Vec<&str>); 3] = [
            (Some("0.9.0"), false, false, vec!["Remove"]),
            (None, true, false, vec!["Remove"]),
            (None, false, true, vec!["View log", "Remove"]),
        ];
        for (previous, rollback, log, expected) in cases {
            let menu = row_menu(previous, rollback, log);
            assert_eq!(labels(&menu), expected, "{previous:?} {rollback} {log}");
        }
    }
}
