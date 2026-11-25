//! Gallery stories for the Marketplace components: the item header, the extension manager row, the invocation
//! banner, the account list, the account sign-in dialog and the danger zone.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Story, variant};
use crate::components::account_connection_list::{
    AccountConnectionList, AccountRow, AccountStatus,
};
use crate::components::account_sign_in_dialog::{AccountSignInDialog, SignInStage};
use crate::components::button::{Button, ButtonVariant};
use crate::components::danger_zone::DangerZone;
use crate::components::extension_card::ExtensionState;
use crate::components::extension_manager_row::ExtensionManagerRow;
use crate::components::invocation_banner::InvocationBanner;
use crate::components::marketplace_item_header::MarketplaceItemHeader;
use crate::icon::IconName;
use crate::theme::ActiveTheme;
use crate::tokens::{RADIUS_LG, SPACE_4, SPACE_6};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Extension manager row",
            spec: "ExtensionManagerRow.md",
            render: manager_rows,
        },
        Story {
            name: "Marketplace item header",
            spec: "ExtensionCard.md",
            render: item_header,
        },
        Story {
            name: "Invocation banner",
            spec: "Banner.md",
            render: invocation_banner,
        },
        Story {
            name: "Account connection list",
            spec: "SettingsRow.md",
            render: account_connection_list,
        },
        Story {
            name: "Account sign-in dialog",
            spec: "Dialog.md",
            render: account_sign_in_dialog,
        },
        Story {
            name: "Danger zone",
            spec: "Card.md",
            render: danger_zone,
        },
    ]
}

/// The Installed list's group card with one row per state: enabled, an update waiting, disabled, installing, failed.
fn manager_rows(_: &mut Window, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let p = theme.palette;
    let row = |key: &'static str, name: &'static str, state: ExtensionState, word: &'static str| {
        ExtensionManagerRow::new(key, name, state)
            .publisher("MassOrca extras")
            .version("1.2.0")
            .contribution("Sidebar section")
            .status_word(word)
            .icon(IconName::Puzzle)
    };
    div()
        .flex()
        .flex_col()
        .rounded(px(RADIUS_LG))
        .border_1()
        .border_color(p.border.default)
        .bg(p.surface.raised)
        .overflow_hidden()
        .child(row(
            "gallery-enabled",
            "Workspace board",
            ExtensionState::Installed {
                enabled: true,
                update: None,
            },
            "Enabled",
        ))
        .child(row(
            "gallery-update",
            "Model usage panel",
            ExtensionState::Installed {
                enabled: true,
                update: Some("2.1.0".into()),
            },
            "Update available 2.1.0",
        ))
        .child(row(
            "gallery-disabled",
            "Quick notes",
            ExtensionState::Installed {
                enabled: false,
                update: None,
            },
            "Disabled",
        ))
        .child(row(
            "gallery-installing",
            "Installing extension",
            ExtensionState::Installing { percent: 40 },
            "Installing, 40%",
        ))
        .child(row(
            "gallery-failed",
            "Log tail",
            ExtensionState::Failed {
                reason: "The CLI did not start".into(),
            },
            "Failed to start",
        ))
        .into_any_element()
}

fn item_header(_: &mut Window, cx: &mut App) -> AnyElement {
    let connected = MarketplaceItemHeader::new("story-header-connected", "Claude", IconName::Bot)
        .state("Connected")
        .meta("MassOrca · Marketplace · 1.0.0")
        .copy_link(Button::new("story-copy-connected", "Copy link").variant(ButtonVariant::Ghost))
        .host(div().child("This computer, Connected"));
    let plugin = MarketplaceItemHeader::new("story-header-plugin", "GitHub", IconName::Puzzle)
        .state("Not installed")
        .meta("Third party · Marketplace · 1.3.0")
        .copy_link(Button::new("story-copy-plugin", "Copy link").variant(ButtonVariant::Ghost))
        .host(div().child("This computer, Connected"))
        .action(Button::new("story-review", "Review and install").disabled(true));
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .child(variant("Connected agent", connected, cx))
        .child(variant("Plugin not installed", plugin, cx))
        .into_any_element()
}

fn invocation_banner(_: &mut Window, cx: &mut App) -> AnyElement {
    let banner = InvocationBanner::new("story-banner", "@Claude")
        .example("@Claude review the open changes on this branch")
        .example("@Claude explain the failing test in this project")
        .example("@Claude summarise today's sessions");
    variant("Three examples", banner, cx)
}

fn account_connection_list(_: &mut Window, cx: &mut App) -> AnyElement {
    let list = AccountConnectionList::new("story-accounts")
        .account(
            AccountRow::new("story-account-1", "Work account", AccountStatus::SignedIn)
                .identity("Signed in as user@example.com")
                .active(true)
                .used_by("Used by Project one")
                .check(Button::new("story-check-1", "Check now").variant(ButtonVariant::Ghost))
                .remove(Button::new("story-remove-1", "Remove").variant(ButtonVariant::Ghost)),
        )
        .account(
            AccountRow::new("story-account-2", "Second account", AccountStatus::Expired)
                .identity("Last signed in as second@example.com")
                .used_by("Not used by any project")
                .error("A live session uses this account. Stop the session, then remove it.")
                .make_active(Button::new("story-activate-2", "Make active"))
                .check(Button::new("story-check-2", "Check now").variant(ButtonVariant::Ghost))
                .remove(Button::new("story-remove-2", "Remove").variant(ButtonVariant::Ghost)),
        )
        .empty("No accounts yet. Add one to use this agent.")
        .add_account(Button::new("story-add", "Add account"));
    div()
        .w(px(560.0))
        .child(variant("Two accounts, one active", list, cx))
        .into_any_element()
}

fn account_sign_in_dialog(_: &mut Window, cx: &mut App) -> AnyElement {
    let waiting = AccountSignInDialog::new(
        "story-sign-in-browser",
        "Claude",
        SignInStage::WaitingBrowser {
            url: "https://example.com/sign-in".into(),
        },
    );
    let failed = AccountSignInDialog::new(
        "story-sign-in-failed",
        "Claude",
        SignInStage::Failed {
            reason: "The code expired. Try again.".into(),
        },
    );
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant("Waiting for the browser", waiting, cx))
        .child(variant("Failed", failed, cx))
        .into_any_element()
}

fn danger_zone(_: &mut Window, cx: &mut App) -> AnyElement {
    let zone = DangerZone::new("story-danger")
        .description(
            "Uninstall removes Claude from this computer. Its accounts stay until you remove them.",
        )
        .action(Button::new("story-uninstall", "Uninstall").variant(ButtonVariant::Danger));
    variant("Installed item", zone, cx)
}
