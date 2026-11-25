//! The Installed list: one manager row per installed extension, in one group card (teams-extensions-settings.md 7.4).

use super::MarketplaceFrame;
use super::catalogue::{card_state, item_mark, kind_icon};
use crate::model::marketplace::{MarketplaceItem, status_word};
use crate::model::navigation::MarketplacePage;
use gpui::{
    AnyElement, App, IntoElement, ParentElement, Role, SharedString, Styled, div, prelude::*, px,
};
use massorca_ui::ActiveTheme;
use massorca_ui::components::extension_card::ExtensionAction;
use massorca_ui::components::extension_manager_row::ExtensionManagerRow;
use massorca_ui::tokens::RADIUS_LG;

/// The rows of the Installed list in one group card; each row after the first sits under a hairline.
pub(super) fn installed_rows(
    frame: &MarketplaceFrame,
    items: &[&MarketplaceItem],
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
        let mut cell = div().debug_selector(move || format!("marketplace-row-{id}"));
        if index > 0 {
            cell = cell.border_t_1().border_color(p.border.subtle);
        }
        group = group.child(cell.child(manager_row(frame, item)));
    }
    group.into_any_element()
}

/// One installed item as a row: its name opens the item page, and its switch enables or disables it.
fn manager_row(frame: &MarketplaceFrame, item: &MarketplaceItem) -> ExtensionManagerRow {
    let id = item.id;
    let shell = frame.shell.clone();
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
    row.on_action(move |action, _, cx| {
        let _ = shell.update(cx, |shell, cx| match action {
            ExtensionAction::Toggle(enabled) => shell.set_marketplace_enabled(id, enabled, cx),
            _ => shell.open_marketplace(MarketplacePage::Item(id), cx),
        });
    })
}
