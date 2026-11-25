//! The Installed list: one manager row per installed extension, in one group card, each with its More menu
//! (teams-extensions-settings.md 7.4).

use super::super::Shell;
use super::MarketplaceFrame;
use super::catalogue::{card_state, item_mark, kind_icon};
use crate::model::marketplace::{MarketplaceItem, MarketplaceItemId, status_word};
use crate::model::navigation::MarketplacePage;
use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, Pixels, Point, Role, SharedString, Styled,
    Subscription, WeakEntity, Window, div, prelude::*, px,
};
use massorca_ui::ActiveTheme;
use massorca_ui::IconName;
use massorca_ui::components::extension_card::ExtensionAction;
use massorca_ui::components::extension_manager_row::ExtensionManagerRow;
use massorca_ui::components::menu::{
    MenuEntry, MenuEvent, MenuItem, MenuOrigin, MenuView, menu_overlay,
};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::tokens::RADIUS_LG;

/// The rows of an installed item's menu, in order. Roll back and View log have no service method yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowMenuRow {
    RollBack,
    ViewLog,
    Separator,
    Remove,
}

const ROW_MENU: [RowMenuRow; 4] = [
    RowMenuRow::RollBack,
    RowMenuRow::ViewLog,
    RowMenuRow::Separator,
    RowMenuRow::Remove,
];

/// The menu open on one installed row: the menu, where it opened, and the subscription that answers its choice.
#[derive(Debug)]
struct OpenRowMenu {
    menu: Entity<MenuView>,
    at: Point<Pixels>,
    _subscription: Subscription,
}

/// The menu state of one installed row, kept in the row's keyed state so the Shell holds no field for it.
#[derive(Debug, Default)]
struct RowMenu {
    open: Option<OpenRowMenu>,
}

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
            |_, _| RowMenu::default(),
        );
        let open = menu
            .read(cx)
            .open
            .as_ref()
            .map(|open| (open.menu.clone(), open.at));
        let mut cell = div()
            .relative()
            .debug_selector(move || format!("marketplace-row-{id}"));
        if index > 0 {
            cell = cell.border_t_1().border_color(p.border.subtle);
        }
        cell = cell.child(manager_row(frame, item, &menu));
        if let Some((view, at)) = open {
            cell = cell.child(
                div()
                    .absolute()
                    .child(menu_overlay(&view, MenuOrigin::At(at), cx)),
            );
        }
        group = group.child(cell);
    }
    group.into_any_element()
}

/// One installed item as a row: its name opens the item page, its switch enables or disables it, and its More
/// button opens the row's menu.
fn manager_row(
    frame: &MarketplaceFrame,
    item: &MarketplaceItem,
    menu: &Entity<RowMenu>,
) -> ExtensionManagerRow {
    let id = item.id;
    let shell = frame.shell.clone();
    let menu = menu.clone();
    let roll_back = match &item.previous_version {
        Some(version) => format!("Roll back to {version}"),
        None => "Roll back".to_owned(),
    };
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
        ExtensionAction::More => open_row_menu(&menu, id, &roll_back, shell.clone(), window, cx),
        _ => {
            let _ = shell.update(cx, |shell, cx| {
                shell.open_marketplace(MarketplacePage::Item(id), cx)
            });
        }
    })
}

/// Opens the menu of an installed row at the pointer. Remove asks the existing uninstall confirm; the entries that are
/// not available yet choose nothing. A choice or a dismissal closes the menu and redraws the Shell.
fn open_row_menu(
    menu: &Entity<RowMenu>,
    item: MarketplaceItemId,
    roll_back: &str,
    shell: WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
) {
    let entries: Vec<MenuEntry> = ROW_MENU
        .iter()
        .map(|row| menu_entry(*row, roll_back))
        .collect();
    let view = cx.new(|cx| MenuView::new(entries, window, cx).with_layer(OverlayLayer::Page));
    let at = window.mouse_position();
    let on_choice = shell.clone();
    menu.update(cx, |state, cx| {
        let subscription = cx.subscribe_in(
            &view,
            window,
            move |state: &mut RowMenu, _, event: &MenuEvent, _, cx| {
                // Taking the menu closes it, on a choice or a dismissal alike.
                let _closed = state.open.take();
                if matches!(event, MenuEvent::Chosen(choice) if ROW_MENU.get(choice.row) == Some(&RowMenuRow::Remove))
                {
                    let _ = on_choice
                        .update(cx, |shell, cx| shell.ask_marketplace_uninstall(item, cx));
                }
                let _ = on_choice.update(cx, |_, cx| cx.notify());
            },
        );
        state.open = Some(OpenRowMenu {
            menu: view,
            at,
            _subscription: subscription,
        });
        cx.notify();
    });
    let _ = shell.update(cx, |_, cx| cx.notify());
}

/// The menu row a row-menu entry draws as.
fn menu_entry(row: RowMenuRow, roll_back: &str) -> MenuEntry {
    match row {
        RowMenuRow::RollBack => MenuItem::new(roll_back.to_owned())
            .icon(IconName::History)
            .disabled("Not available yet")
            .into(),
        RowMenuRow::ViewLog => MenuItem::new("View log")
            .icon(IconName::FileText)
            .disabled("Not available yet")
            .into(),
        RowMenuRow::Separator => MenuEntry::Separator,
        RowMenuRow::Remove => MenuItem::new("Remove")
            .icon(IconName::Trash2)
            .destructive()
            .into(),
    }
}
