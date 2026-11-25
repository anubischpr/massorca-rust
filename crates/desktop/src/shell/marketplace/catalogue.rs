//! The catalogue: its left navigation, and the cards of Discover, Installed, Updates and each
//! category (teams-extensions-settings.md 7.1 and 7.4).

use super::MarketplaceFrame;
use super::host_pick::host_picker_row;
use super::item::problem_banner;
use super::item_images;
use crate::model::agent_sessions::agent_of_kind;
use crate::model::marketplace::{
    ItemKind, ItemState, MarketplaceItem, count_line, counts, heading, intro, listed,
    no_match_text, problem_alert,
};
use crate::model::navigation::MarketplacePage;
use crate::shell::rail;
use gpui::{
    AnyElement, App, IntoElement, ParentElement, Role, SharedString, Styled, Window, div,
    prelude::*, px,
};
use massorca_ui::components::agent_icon::AgentKind;
use massorca_ui::components::badge::Badge;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::extension_card::{ExtensionAction, ExtensionCard, ExtensionState};
use massorca_ui::components::list_row::{ListRow, RowLeading};
use massorca_ui::tokens::{
    SPACE_1, SPACE_2, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_HEADING, TYPE_LABEL,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

/// The width of the left navigation (teams-extensions-settings.md 6.1, layout.settings.nav).
const NAV_WIDTH: f32 = 232.0;
/// A catalogue card is at least this wide (teams-extensions-settings.md 7.1).
const CARD_WIDTH: f32 = 280.0;
/// The search field of the catalogue (teams-extensions-settings.md 7.1: 280 px).
const SEARCH_WIDTH: f32 = 280.0;

/// The Lucide icon of each kind of item.
pub(super) fn kind_icon(kind: ItemKind) -> IconName {
    match kind {
        ItemKind::Extension => IconName::Puzzle,
        ItemKind::Agent => IconName::Bot,
        ItemKind::Skill => IconName::WandSparkles,
        ItemKind::Service => IconName::Server,
        ItemKind::Theme => IconName::Palette,
    }
}

/// One row of the left navigation: what it opens and what it counts.
struct NavEntry {
    id: &'static str,
    label: &'static str,
    icon: IconName,
    page: MarketplacePage,
    count: Option<usize>,
}

/// The left navigation: Discover, Installed and Updates with their counts, then the categories.
pub(super) fn nav_column(
    frame: &MarketplaceFrame,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (installed, updates) = counts(&frame.snapshot);
    let theme = *cx.theme();
    let mut entries = vec![
        NavEntry {
            id: "marketplace-nav-discover",
            label: "Discover",
            icon: IconName::Store,
            page: MarketplacePage::Discover,
            count: None,
        },
        NavEntry {
            id: "marketplace-nav-installed",
            label: "Installed",
            icon: IconName::CircleCheck,
            page: MarketplacePage::Installed,
            count: Some(installed),
        },
        NavEntry {
            id: "marketplace-nav-updates",
            label: "Updates",
            icon: IconName::RefreshCw,
            page: MarketplacePage::Updates,
            count: Some(updates),
        },
    ];
    for kind in ItemKind::ALL {
        entries.push(NavEntry {
            id: category_id(kind),
            label: kind.plural(),
            icon: kind_icon(kind),
            page: MarketplacePage::Category(kind),
            count: None,
        });
    }
    let mut column = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .w(px(NAV_WIDTH))
        .flex_none()
        .min_h_0()
        .p(px(SPACE_4))
        .border_r_1()
        .border_color(theme.palette.border.subtle);
    for (index, entry) in entries.into_iter().enumerate() {
        if index == 3 {
            column = column.child(
                div()
                    .pt(px(SPACE_4))
                    .text_style(TYPE_LABEL)
                    .text_color(theme.palette.text.tertiary)
                    .child("Categories"),
            );
        }
        column = column.child(nav_row(frame, entry, window, cx));
    }
    column.into_any_element()
}

/// The selector id of a category's row.
fn category_id(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Extension => "marketplace-nav-extensions",
        ItemKind::Agent => "marketplace-nav-agents",
        ItemKind::Skill => "marketplace-nav-skills",
        ItemKind::Service => "marketplace-nav-services",
        ItemKind::Theme => "marketplace-nav-themes",
    }
}

/// One row of the left navigation; a press opens its view.
fn nav_row(
    frame: &MarketplaceFrame,
    entry: NavEntry,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let key = SharedString::from(entry.id);
    let focus = window
        .use_keyed_state(key.clone(), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone();
    let mut row = ListRow::new(key, entry.label)
        .leading(RowLeading::Icon(entry.icon))
        .selected(entry.page == frame.page)
        .focus_handle(focus)
        .tab_stop(true);
    if let Some(count) = entry.count {
        row = row.trailing(Badge::count(u32::try_from(count).unwrap_or(u32::MAX)));
    }
    let shell = frame.shell.clone();
    let page = entry.page;
    div()
        .debug_selector(move || entry.id.to_owned())
        .child(row.on_click(move |_, _, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_marketplace(page, cx));
        }))
        .into_any_element()
}

/// The words of the empty state of a view: the search that matched nothing, or the view itself.
fn empty_copy(page: MarketplacePage, query: &str) -> (String, &'static str) {
    if !query.trim().is_empty() {
        return (
            no_match_text(query),
            "Try a word from the name, the publisher or the description.",
        );
    }
    match page {
        MarketplacePage::Installed => (
            "Nothing installed yet".to_owned(),
            "Items you install from Discover show here.",
        ),
        MarketplacePage::Updates => (
            "No updates waiting".to_owned(),
            "Installed items show here when a newer version is ready.",
        ),
        _ => (
            "Nothing to show yet".to_owned(),
            "Extensions, agents, skills and services from your sources show here.",
        ),
    }
}

/// The header, the search and the cards of one catalogue view.
pub(super) fn catalogue_view(
    frame: &MarketplaceFrame,
    page: MarketplacePage,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let items = listed(&frame.snapshot, page, &frame.query);
    let title = heading(page);
    let header = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .child(
            div()
                .id("marketplace-heading")
                .role(Role::Heading)
                .aria_label(title)
                .text_style(TYPE_HEADING)
                .text_color(p.text.primary)
                .child(title),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.secondary)
                .child(intro(page)),
        );
    let toolbar = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(SPACE_2))
        .child(div().w(px(SEARCH_WIDTH)).child(frame.search.clone()))
        .child(host_picker_row(frame, cx))
        .child(add_extension(frame))
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.tertiary)
                .child(count_line(items.len())),
        );
    let body = if items.is_empty() {
        let (title, explanation) = empty_copy(page, &frame.query);
        EmptyState::new("marketplace-empty", title)
            .explanation(explanation)
            .into_any_element()
    } else {
        let mut grid = div().flex().flex_wrap().gap(px(SPACE_4));
        for item in items {
            let key = item.id.0;
            let mut cell = div()
                .w(px(CARD_WIDTH))
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .debug_selector(move || format!("marketplace-item-{key}"));
            if let Some(alert) = problem_alert(item) {
                cell = cell.child(problem_banner(item.id, &alert));
            }
            grid = grid.child(cell.child(card(frame, item, cx)));
        }
        grid.into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .min_w_0()
        .child(header)
        .child(toolbar)
        .child(body)
        .into_any_element()
}

/// The Add extension control: a package in a folder on This computer, reviewed before it installs.
fn add_extension(frame: &MarketplaceFrame) -> AnyElement {
    let shell = frame.shell.clone();
    Button::new("marketplace-add-extension", "Add extension")
        .variant(ButtonVariant::Secondary)
        .leading_icon(IconName::Plus)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_add_extension(cx));
        })
        .into_any_element()
}

/// The state of a card's footer: the one action that fits the item's state.
fn card_state(item: &MarketplaceItem) -> ExtensionState {
    match &item.state {
        ItemState::NotInstalled => ExtensionState::Catalog,
        ItemState::Installed { enabled, update } => ExtensionState::Installed {
            enabled: *enabled,
            update: update.clone().map(SharedString::from),
        },
        ItemState::Installing { percent } => ExtensionState::Installing { percent: *percent },
        ItemState::Failed { reason } => ExtensionState::Failed {
            reason: SharedString::from(reason.clone()),
        },
        ItemState::Incompatible { requires } => ExtensionState::Incompatible {
            requires: SharedString::from(requires.clone()),
        },
    }
}

/// The mark an agent's card draws: the one the sidebar and model picker draw for its kind. None when the desktop
/// has no mark for the kind, and for an item that is not an agent.
fn agent_mark(agent_kind: Option<&str>) -> Option<AgentKind> {
    let mark = rail::agent_kind(agent_of_kind(agent_kind?));
    (mark != AgentKind::Unknown).then_some(mark)
}

/// The mark to draw for an item wherever it appears: its agent's own mark, if it has one.
pub(super) fn item_mark(item: &MarketplaceItem) -> Option<AgentKind> {
    agent_mark(item.agent_kind.as_deref())
}

/// One item as a card. Its Switch changes the sample; every other press opens the item's page,
/// where its actions live (Review and install is not available yet).
fn card(frame: &MarketplaceFrame, item: &MarketplaceItem, cx: &mut App) -> ExtensionCard {
    let id = item.id;
    let shell = frame.shell.clone();
    let mut card = ExtensionCard::new(
        SharedString::from(format!("marketplace-card-{}", item.id.0)),
        item.name.clone(),
        card_state(item),
    )
    .publisher(item.publisher.clone())
    .source("Marketplace")
    .category(item.category.clone())
    .version(item.version.clone())
    .description(item.description.clone())
    .icon(kind_icon(item.kind))
    .settings(item.has_settings);
    card = match item_mark(item) {
        Some(mark) => card.agent(mark),
        None => match item_images::logo(frame, item, cx) {
            Some(logo) => card.logo(logo),
            None => card,
        },
    };
    for add in &item.adds {
        card = card.add(IconName::Plus, add.clone());
    }
    card.on_action(move |action, _, cx| {
        let _ = shell.update(cx, |shell, cx| match action {
            ExtensionAction::Toggle(enabled) => shell.set_marketplace_enabled(id, enabled, cx),
            ExtensionAction::Update => shell.update_package(id, cx),
            _ => shell.open_marketplace(MarketplacePage::Item(id), cx),
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_with_a_desktop_mark_draws_that_mark() {
        assert_eq!(
            agent_mark(Some("gentle-shell")),
            Some(AgentKind::GentleShell)
        );
        assert_eq!(agent_mark(Some("claude")), Some(AgentKind::Claude));
        assert_eq!(agent_mark(Some("gemini")), Some(AgentKind::Gemini));
        assert_eq!(agent_mark(Some("opencode")), Some(AgentKind::OpenCode));
        assert_eq!(agent_mark(Some("cursor")), Some(AgentKind::Cursor));
    }

    #[test]
    fn a_kind_without_a_mark_and_a_non_agent_have_none() {
        assert_eq!(agent_mark(Some("my-own-kind")), None);
        assert_eq!(agent_mark(None), None);
    }
}
