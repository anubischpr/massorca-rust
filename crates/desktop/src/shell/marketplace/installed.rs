//! The Installed list, also called Your apps (SKL-27): one manager row per installed extension, tool or agent, and one per
//! installed skill or connected hosted service, in one group card. Each row's More menu lists only the actions that work
//! for it (teams-extensions-settings.md 7.4).

use std::collections::BTreeMap;

use super::MarketplaceFrame;
use super::catalogue::{card_state, empty_copy, item_mark, kind_icon};
use super::more_menu::{self, MoreMenu};
use super::skills::{SkillsPanel, SkillsSection};
use crate::model::marketplace::{
    AppAction, AppRow, HostTarget, ItemKind, ManagedRow, MarketplaceItem, MarketplaceItemId,
    MarketplaceSnapshot, RowOffers, ServicesState, SkillRow, row_offers, service_apps, skill_apps,
    status_word,
};
use crate::model::navigation::MarketplacePage;
use crate::service::{PlacementAnswer, PlacementOutcome, Refusal};
use crate::shell::Shell;
use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, Role, SharedString, Styled, Window, div,
    prelude::*, px,
};
use massorca_ui::ActiveTheme;
use massorca_ui::IconName;
use massorca_ui::TextStyled;
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::extension_card::{ExtensionAction, ExtensionState};
use massorca_ui::components::extension_manager_row::ExtensionManagerRow;
use massorca_ui::components::menu::{MenuEntry, MenuItem};
use massorca_ui::tokens::{RADIUS_LG, SPACE_4, TYPE_BODY_SM};

/// Your apps for the picked host: its installed skills and connected services, and one sentence for each part that could
/// not be read, such as a host that predates skill placement.
#[derive(Debug, Default)]
pub(super) struct YourApps {
    pub(super) rows: Vec<AppRow>,
    pub(super) notes: Vec<String>,
}

/// The Your apps rows and notes of the shown host, from the answers the Skills and Services reads hold.
pub(super) fn your_apps(shell: &Shell, skills: &SkillsSection, cx: &App) -> YourApps {
    let state = shell.store.read(cx).state();
    let mut notes = Vec::new();
    let found: &[SkillRow] = match &skills.panel {
        SkillsPanel::Found { rows, .. } => rows,
        SkillsPanel::UpdateNeeded(sentence) | SkillsPanel::Refused(sentence) => {
            add_note(&mut notes, sentence.clone());
            &[]
        }
        SkillsPanel::Scanning => &[],
    };
    let placed: &[ManagedRow] = if skills.local { &skills.placed } else { &[] };
    if let (
        true,
        Some(PlacementAnswer {
            outcome: PlacementOutcome::Refused(refusal),
            ..
        }),
    ) = (skills.local, state.managed.as_ref())
    {
        add_note(&mut notes, placement_note(refusal));
    }
    let mut rows = skill_apps(found, placed, skills.local);
    let services_host = match shell.shown_target() {
        HostTarget::Local => None,
        HostTarget::Paired(name) => Some(name),
    };
    let view = &shell.marketplace_services;
    if view.host() == services_host.as_deref() {
        match &view.state {
            ServicesState::Ready(services) => rows.extend(service_apps(services)),
            ServicesState::Failed(sentence) => add_note(&mut notes, sentence.clone()),
            ServicesState::Loading => {}
        }
    }
    YourApps { rows, notes }
}

/// The sentence for a placed-skills read that was refused. A host that lacks the capability, or predates it, asks for an
/// update, as discovery does; any other refusal says what happened.
fn placement_note(refusal: &Refusal) -> String {
    match refusal.code() {
        Some("service.capability_missing" | "method.not_found") => {
            "Update MassOrca first.".to_owned()
        }
        _ => refusal.sentence(),
    }
}

/// One sentence per cause, even when two reads report the same one, such as an unconnected service.
fn add_note(notes: &mut Vec<String>, sentence: String) {
    if !notes.contains(&sentence) {
        notes.push(sentence);
    }
}

/// What each installed item's More menu offers, by item, as the service and this computer can do it now.
pub(super) fn row_offers_of(shell: &Shell, cx: &App) -> BTreeMap<MarketplaceItemId, RowOffers> {
    let snapshot: &MarketplaceSnapshot = shell.model.marketplace();
    let connected = shell.service.is_some();
    snapshot
        .installed()
        .into_iter()
        .map(|item| {
            let updatable = shell.updatable_program(item.id, cx).is_some();
            (item.id, row_offers(item, snapshot, updatable, connected))
        })
        .collect()
}

/// The apps a search keeps, by name or detail, the way the list keeps packages.
pub(super) fn matching_apps<'a>(apps: &'a [AppRow], query: &str) -> Vec<&'a AppRow> {
    let query = query.trim().to_lowercase();
    apps.iter()
        .filter(|app| {
            query.is_empty()
                || app.name.to_lowercase().contains(&query)
                || app.detail.to_lowercase().contains(&query)
        })
        .collect()
}

/// The Installed page's body: the group card with every row, or the empty state, then the notes of what could not be read.
pub(super) fn installed_body(
    frame: &MarketplaceFrame,
    items: &[&MarketplaceItem],
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let apps = matching_apps(&frame.apps.rows, &frame.query);
    let mut body = div().flex().flex_col().gap(px(SPACE_4));
    if items.is_empty() && apps.is_empty() {
        let (title, explanation) = empty_copy(MarketplacePage::Installed, &frame.query);
        body = body.child(
            EmptyState::new("marketplace-empty", title)
                .explanation(explanation)
                .into_any_element(),
        );
    } else {
        body = body.child(installed_rows(frame, items, &apps, window, cx));
    }
    let theme = *cx.theme();
    let p = theme.palette;
    for note in &frame.apps.notes {
        body = body.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.tertiary)
                .child(note.clone()),
        );
    }
    body.into_any_element()
}

/// The rows of the Installed list in one group card; each row after the first sits under a hairline.
fn installed_rows(
    frame: &MarketplaceFrame,
    items: &[&MarketplaceItem],
    apps: &[&AppRow],
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
    let mut index = 0;
    for item in items {
        let id = item.id.0;
        let menu = window.use_keyed_state(
            SharedString::from(format!("marketplace-row-menu-{id}")),
            cx,
            |_, _| MoreMenu::default(),
        );
        let offers = frame.offers.get(&item.id).copied().unwrap_or_default();
        let mut cell = div()
            .relative()
            .debug_selector(move || format!("marketplace-row-{id}"));
        if index > 0 {
            cell = cell.border_t_1().border_color(p.border.subtle);
        }
        cell = cell.child(manager_row(frame, item, offers, &menu));
        if let Some(overlay) = MoreMenu::overlay(&menu, cx) {
            cell = cell.child(div().absolute().child(overlay));
        }
        group = group.child(cell);
        index += 1;
    }
    for app in apps {
        let menu = window.use_keyed_state(
            SharedString::from(format!("marketplace-app-menu-{}", app.key)),
            cx,
            |_, _| MoreMenu::default(),
        );
        let key = app.key.clone();
        let mut cell = div()
            .relative()
            .debug_selector(move || format!("marketplace-app-{key}"));
        if index > 0 {
            cell = cell.border_t_1().border_color(p.border.subtle);
        }
        cell = cell.child(app_row(frame, app, &menu));
        if let Some(overlay) = MoreMenu::overlay(&menu, cx) {
            cell = cell.child(div().absolute().child(overlay));
        }
        group = group.child(cell);
        index += 1;
    }
    group.into_any_element()
}

/// One installed item as a row: its name opens the item page, its switch enables or disables it, and its More button
/// opens the row's menu, which is left out when no action works.
fn manager_row(
    frame: &MarketplaceFrame,
    item: &MarketplaceItem,
    offers: RowOffers,
    menu: &Entity<MoreMenu>,
) -> ExtensionManagerRow {
    let id = item.id;
    let shell = frame.shell.clone();
    let menu = menu.clone();
    let previous = item.previous_version.clone();
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
    if row_menu(previous.as_deref(), offers).is_empty() {
        row = row.without_more();
    }
    row.on_action(move |action, window, cx| match action {
        ExtensionAction::Toggle(enabled) => {
            let _ = shell.update(cx, |shell, cx| {
                shell.set_marketplace_enabled(id, enabled, cx)
            });
        }
        ExtensionAction::More => {
            let (entries, actions): (Vec<MenuEntry>, Vec<Option<RowAction>>) =
                row_menu(previous.as_deref(), offers).into_iter().unzip();
            let chosen = shell.clone();
            more_menu::open(
                &menu,
                shell.clone(),
                entries,
                move |row, _, cx| match actions.get(row).copied().flatten() {
                    Some(RowAction::Update) => {
                        let _ = chosen.update(cx, |shell, cx| shell.open_program_update(id, cx));
                    }
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

/// One installed skill or hosted service: its name opens its category, and its More button offers the one action it has.
fn app_row(frame: &MarketplaceFrame, app: &AppRow, menu: &Entity<MoreMenu>) -> ExtensionManagerRow {
    let shell = frame.shell.clone();
    let menu = menu.clone();
    let (action, kind) = (app.action.clone(), app.kind);
    let mut row = ExtensionManagerRow::new(
        SharedString::from(format!("marketplace-app-{}", app.key)),
        app.name.clone(),
        ExtensionState::Installed {
            enabled: true,
            update: None,
        },
    )
    .contribution(app.detail.clone())
    .status_word(app.status)
    .icon(kind_icon(kind))
    .without_switch();
    if action.is_none() {
        row = row.without_more();
    }
    row.on_action(move |event, window, cx| match (event, action.clone()) {
        (ExtensionAction::More, Some(action)) => {
            let entry: MenuEntry = match &action {
                AppAction::RemovePlacement(_) => MenuItem::new("Remove placement").destructive(),
                AppAction::Disconnect(_) => MenuItem::new("Disconnect"),
            }
            .into();
            let chosen = shell.clone();
            more_menu::open(
                &menu,
                shell.clone(),
                vec![entry],
                move |_, _, cx| {
                    let _ = chosen.update(cx, |shell, cx| shell.run_app_action(action.clone(), cx));
                },
                window,
                cx,
            );
        }
        (ExtensionAction::More, None) => {}
        _ => {
            let _ = shell.update(cx, |shell, cx| {
                shell.open_marketplace(MarketplacePage::Category(kind), cx)
            });
        }
    })
}

/// What choosing an installed row's menu entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RowAction {
    Update,
    Rollback,
    ViewLog,
    Remove,
}

/// The entries of an installed row's menu in the artboard's order, each with what choosing it does; a separator has
/// none. Update comes first, then Roll back to the version an update replaced and View log, each only when offered,
/// then Remove after a separator. An entry is listed only when it works.
fn row_menu(previous: Option<&str>, offers: RowOffers) -> Vec<(MenuEntry, Option<RowAction>)> {
    let mut menu: Vec<(MenuEntry, Option<RowAction>)> = Vec::new();
    if offers.update {
        let update = MenuItem::new("Update").icon(IconName::RefreshCw);
        menu.push((update.into(), Some(RowAction::Update)));
    }
    if let (true, Some(version)) = (offers.rollback, previous) {
        let roll_back = MenuItem::new(format!("Roll back to {version}")).icon(IconName::History);
        menu.push((roll_back.into(), Some(RowAction::Rollback)));
    }
    if offers.log {
        let view_log = MenuItem::new("View log").icon(IconName::FileText);
        menu.push((view_log.into(), Some(RowAction::ViewLog)));
    }
    if offers.remove {
        if !menu.is_empty() {
            menu.push((MenuEntry::Separator, None));
        }
        let remove = MenuItem::new("Remove").icon(IconName::Trash2).destructive();
        menu.push((remove.into(), Some(RowAction::Remove)));
    }
    menu
}

impl Shell {
    /// Runs the one action an app row's menu offers: a placement's removal is confirmed on the Skills page, and a
    /// disconnect goes to the service.
    pub(super) fn run_app_action(&mut self, action: AppAction, cx: &mut gpui::Context<Self>) {
        match action {
            AppAction::RemovePlacement(name) => {
                self.open_marketplace(MarketplacePage::Category(ItemKind::Skill), cx);
                self.ask_remove_placement(name, cx);
            }
            AppAction::Disconnect(service) => self.disconnect_service(service, cx),
        }
    }
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

    fn all() -> RowOffers {
        RowOffers {
            update: true,
            rollback: true,
            log: true,
            remove: true,
        }
    }

    #[test]
    fn update_roll_back_view_log_then_remove_in_the_artboard_order() {
        let menu = row_menu(Some("0.9.0"), all());
        assert_eq!(
            labels(&menu),
            ["Update", "Roll back to 0.9.0", "View log", "Remove"]
        );
        assert!(matches!(menu[0].1, Some(RowAction::Update)));
        assert!(matches!(menu[1].1, Some(RowAction::Rollback)));
        assert!(matches!(menu[2].1, Some(RowAction::ViewLog)));
        assert!(matches!(menu[3].0, MenuEntry::Separator));
        assert!(matches!(menu[4].1, Some(RowAction::Remove)));
    }

    #[test]
    fn the_row_menu_lists_only_the_actions_that_work() {
        let remove_only = RowOffers {
            remove: true,
            ..RowOffers::default()
        };
        let cases: [(Option<&str>, RowOffers, Vec<&str>); 4] = [
            (Some("0.9.0"), remove_only, vec!["Remove"]),
            (
                None,
                RowOffers {
                    rollback: true,
                    remove: true,
                    ..RowOffers::default()
                },
                vec!["Remove"],
            ),
            (
                None,
                RowOffers {
                    log: true,
                    remove: true,
                    ..RowOffers::default()
                },
                vec!["View log", "Remove"],
            ),
            (None, RowOffers::default(), vec![]),
        ];
        for (previous, offers, expected) in cases {
            let menu = row_menu(previous, offers);
            assert_eq!(labels(&menu), expected, "{previous:?} {offers:?}");
        }
    }

    #[test]
    fn a_program_update_is_offered_without_remove_when_nothing_else_works() {
        let update_only = RowOffers {
            update: true,
            ..RowOffers::default()
        };
        assert_eq!(labels(&row_menu(None, update_only)), ["Update"]);
    }

    #[test]
    fn a_placed_skill_read_refused_by_an_older_host_asks_for_an_update() {
        let missing = Refusal::Failed {
            code: "method.not_found".to_owned(),
            message: "method skills.managed not found".to_owned(),
        };
        assert_eq!(placement_note(&missing), "Update MassOrca first.");
        let other = Refusal::Failed {
            code: "service.failed".to_owned(),
            message: "The skill folder is locked.".to_owned(),
        };
        assert_eq!(placement_note(&other), other.sentence());
    }

    #[test]
    fn an_app_search_matches_the_name_and_the_detail() {
        let apps = vec![AppRow {
            kind: ItemKind::Skill,
            key: "skill:notes".to_owned(),
            name: "Notes".to_owned(),
            detail: "Claude".to_owned(),
            status: "Installed",
            action: None,
        }];
        assert_eq!(matching_apps(&apps, "clau").len(), 1);
        assert_eq!(matching_apps(&apps, "nOt").len(), 1);
        assert!(matching_apps(&apps, "gitea").is_empty());
    }
}
