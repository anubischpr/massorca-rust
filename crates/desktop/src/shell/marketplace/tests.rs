//! Element tests of the Marketplace pages: the catalogue, an item page's parts, its actions and its
//! dialogs. They run on the sample source, so every change they make stays in the sample.

use crate::data::SampleDataSource;
use crate::model::marketplace::{
    AccountId, ItemState, MarketplaceItemId, MarketplaceSnapshot, SignInStep,
};
use crate::model::navigation::{MarketplacePage, Page};
use crate::model::teams::SampleTeamsSource;
use crate::shell::Shell;
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use massorca_ui::{Appearance, Theme};

/// The eight sample items in id order; each one has a card in Discover.
const CARDS: [&str; 8] = [
    "marketplace-item-1",
    "marketplace-item-2",
    "marketplace-item-3",
    "marketplace-item-4",
    "marketplace-item-5",
    "marketplace-item-6",
    "marketplace-item-7",
    "marketplace-item-8",
];

/// The parts of a page that the nine-part order is checked against, from the top down.
const PARTS: [&str; 7] = [
    "marketplace-part-logo",
    "marketplace-part-banner",
    "marketplace-part-connection",
    "marketplace-part-information",
    "marketplace-part-settings",
    "marketplace-part-advanced",
    "marketplace-part-danger",
];

/// The sign-in address the sample offers.
const SIGN_IN_URL: &str = "https://example.com/sign-in";

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| Theme::set(Appearance::Dark, cx));
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// Clicks the centre of the element with this debug selector, after a fresh draw.
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    draw(cx);
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
}

/// Whether an element with this debug selector is drawn, after a fresh draw.
fn shown(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    draw(cx);
    cx.debug_bounds(selector).is_some()
}

fn open_shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    setup(cx);
    cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx))
}

fn page(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Page {
    shell.read_with(cx, |shell, _| shell.model().navigation().page)
}

fn snapshot(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> MarketplaceSnapshot {
    shell.read_with(cx, |shell, _| shell.model().marketplace().clone())
}

fn sign_in_step(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<SignInStep> {
    snapshot(shell, cx).sign_in.map(|flow| flow.step)
}

fn item_state(shell: &Entity<Shell>, cx: &mut VisualTestContext, id: u32) -> ItemState {
    snapshot(shell, cx)
        .item(MarketplaceItemId(id))
        .expect("the sample has the item")
        .state
        .clone()
}

/// Types a code into the sign-in field, as the person would before pressing Continue.
fn type_code(shell: &Entity<Shell>, cx: &mut VisualTestContext, code: &'static str) {
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell
                .marketplace_code
                .update(cx, |field, cx| field.set_text(code, window, cx));
        });
    });
}

/// Opens the Marketplace from the rail, narrows the catalogue to one item and presses its name.
fn open_item(shell: &Entity<Shell>, cx: &mut VisualTestContext, query: &str) {
    click(cx, "rail-marketplace");
    shell.update(cx, |shell, cx| shell.set_marketplace_query(query, cx));
    click(cx, "name");
}

#[gpui::test]
fn the_rail_opens_the_catalogue_with_every_sample_item(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    click(cx, "rail-marketplace");
    assert_eq!(
        page(&shell, cx),
        Page::Marketplace(MarketplacePage::Discover)
    );
    for card in CARDS {
        assert!(shown(cx, card), "{card} is listed in Discover");
    }
}

#[gpui::test]
fn the_installed_view_lists_only_installed_items(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    click(cx, "rail-marketplace");
    click(cx, "marketplace-nav-installed");
    assert_eq!(
        page(&shell, cx),
        Page::Marketplace(MarketplacePage::Installed)
    );
    assert!(shown(cx, "marketplace-item-1"), "Claude is installed");
    assert!(
        !shown(cx, "marketplace-item-3"),
        "GitHub is found on this PC but not installed"
    );
}

#[gpui::test]
fn a_search_narrows_the_catalogue(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    click(cx, "rail-marketplace");
    shell.update(cx, |shell, cx| shell.set_marketplace_query("github", cx));
    assert!(shown(cx, "marketplace-item-3"));
    assert!(!shown(cx, "marketplace-item-1"));
}

#[gpui::test]
fn an_item_page_shows_its_nine_parts_in_the_owners_order(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    open_item(&shell, cx, "claude");
    assert_eq!(
        page(&shell, cx),
        Page::Marketplace(MarketplacePage::Item(MarketplaceItemId(1)))
    );
    draw(cx);
    let mut above = f32::MIN;
    for part in PARTS {
        let top = f32::from(
            cx.debug_bounds(part)
                .unwrap_or_else(|| panic!("no part {part}"))
                .origin
                .y,
        );
        assert!(top > above, "{part} sits below the part before it");
        above = top;
    }
}

#[gpui::test]
fn copy_link_puts_the_items_source_on_the_clipboard(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    open_item(&shell, cx, "claude");
    click(cx, "button:marketplace-copy-link");
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(
        copied.as_deref(),
        Some("https://example.com/marketplace/claude")
    );
}

#[gpui::test]
fn make_active_moves_the_active_account_in_the_sample(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    open_item(&shell, cx, "claude");
    click(cx, "button:marketplace-make-active-2");
    let snap = snapshot(&shell, cx);
    let active: Vec<u32> = snap
        .accounts_for("claude")
        .into_iter()
        .filter(|account| account.active)
        .map(|account| account.id.0)
        .collect();
    assert_eq!(active, vec![2]);
}

#[gpui::test]
fn removing_an_account_a_session_uses_is_refused_under_its_row(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    open_item(&shell, cx, "claude");
    click(cx, "button:marketplace-remove-1");
    assert!(
        snapshot(&shell, cx)
            .accounts
            .iter()
            .any(|account| account.id == AccountId(1)),
        "the account a session uses stays"
    );
    let refusal = shell.read_with(cx, |shell, _| shell.marketplace_error.clone());
    assert_eq!(
        refusal,
        Some((
            Some(AccountId(1)),
            "A live session uses this account. Stop the session, then remove it.".to_owned()
        ))
    );
    click(cx, "button:marketplace-remove-2");
    assert!(
        !snapshot(&shell, cx)
            .accounts
            .iter()
            .any(|account| account.id == AccountId(2)),
        "an account no session uses is removed from the sample"
    );
}

#[gpui::test]
fn the_danger_zone_confirms_before_uninstalling_the_sample_item(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    open_item(&shell, cx, "quick notes");
    click(cx, "button:marketplace-uninstall");
    assert!(
        shell.read_with(cx, |shell, _| shell.marketplace_uninstall.is_some()),
        "the confirm is open"
    );
    click(cx, "dialog-cancel");
    assert!(
        matches!(item_state(&shell, cx, 6), ItemState::Installed { .. }),
        "cancelling keeps the item"
    );
    click(cx, "button:marketplace-uninstall");
    click(cx, "dialog-action");
    assert_eq!(item_state(&shell, cx, 6), ItemState::NotInstalled);
}

#[gpui::test]
fn the_sign_in_dialog_follows_each_stage_in_the_sample(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    open_item(&shell, cx, "claude");
    click(cx, "button:marketplace-add-account");
    assert_eq!(
        sign_in_step(&shell, cx),
        Some(SignInStep::WaitingBrowser {
            url: SIGN_IN_URL.to_owned()
        })
    );
    click(cx, "dialog-action");
    assert_eq!(sign_in_step(&shell, cx), Some(SignInStep::WaitingCode));
    type_code(&shell, cx, "482913");
    click(cx, "dialog-action");
    assert_eq!(sign_in_step(&shell, cx), Some(SignInStep::Done));
    click(cx, "dialog-action");
    assert_eq!(sign_in_step(&shell, cx), None, "Done closes the dialog");
    assert_eq!(
        snapshot(&shell, cx).accounts_for("claude").len(),
        3,
        "a finished sign-in adds an account to the sample"
    );
}

#[gpui::test]
fn settings_lists_the_installed_agents_and_each_opens_its_page(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    shell.update(cx, |shell, cx| {
        shell.model.open_page(Page::Settings);
        cx.notify();
    });
    click(cx, "settings-nav-Agents");
    assert!(
        shown(cx, "button:settings-agent-open-2"),
        "Gemini CLI is installed"
    );
    assert!(
        !shown(cx, "button:settings-agent-open-3"),
        "GitHub is not installed"
    );
    click(cx, "button:settings-agent-open-1");
    assert_eq!(
        page(&shell, cx),
        Page::Marketplace(MarketplacePage::Item(MarketplaceItemId(1)))
    );
}

#[gpui::test]
fn an_expired_code_fails_the_sign_in_and_try_again_restarts_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    open_item(&shell, cx, "claude");
    click(cx, "button:marketplace-add-account");
    click(cx, "dialog-action");
    type_code(&shell, cx, "expired");
    click(cx, "dialog-action");
    assert_eq!(
        sign_in_step(&shell, cx),
        Some(SignInStep::Failed {
            reason: "The code expired. Try again.".to_owned()
        })
    );
    click(cx, "dialog-action");
    assert_eq!(
        sign_in_step(&shell, cx),
        Some(SignInStep::WaitingBrowser {
            url: SIGN_IN_URL.to_owned()
        }),
        "Try again starts the sign-in from the browser step"
    );
}
