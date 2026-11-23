//! The Marketplace pages: the catalogue and its views, and each item's page (D-EX-01, 7.2, 7.4).
//! Installs and agent accounts go through the source trait, and the snapshot always follows it.

mod catalogue;
mod dialogs;
#[cfg(test)]
mod extension_tests;
mod item;
mod package;
mod review;
#[cfg(test)]
mod tests;

use super::Shell;
use crate::model::marketplace::{
    AccountId, MarketplaceItemId, MarketplaceSnapshot, MarketplaceSource, SourceError,
};
use crate::model::navigation::{MarketplacePage, Page};
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, WeakEntity, Window, div,
    prelude::*, px,
};
use massorca_protocol::methods::extensions::ExtensionReview;
use massorca_ui::components::search_field::SearchField;
use massorca_ui::tokens::SPACE_6;

pub(super) use dialogs::dialog;

/// The uninstall confirm on screen: the item, and the refusal to show in it, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UninstallConfirm {
    pub item: MarketplaceItemId,
    pub error: Option<String>,
    /// The removal is with the service; the confirm waits for its answer.
    pub sending: bool,
}

/// The install review on screen for one catalogue package. What the service reported is shown as it came; nothing
/// is sent to install until the person presses Install, and a vendor installer also needs its own tick (decision
/// 0062).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct InstallReview {
    pub item: MarketplaceItemId,
    /// The `catalogue:<id>` source the review and the install both name.
    pub source: String,
    /// What `extensions.inspect` reported; `None` while it runs or after it was refused.
    pub review: Option<ExtensionReview>,
    /// The person's tick for running the vendor installer the review lists.
    pub run_installer: bool,
    pub sending: bool,
    pub error: Option<String>,
}

/// What the Marketplace pages draw for one frame, gathered by the shell.
pub(super) struct MarketplaceFrame {
    snapshot: MarketplaceSnapshot,
    page: MarketplacePage,
    query: String,
    search: Entity<SearchField>,
    advanced_open: bool,
    /// A refused change, with the account it concerns (None for the whole Connection part).
    error: Option<(Option<AccountId>, String)>,
    /// A refused change to an installed package, with the item it concerns.
    item_error: Option<(MarketplaceItemId, String)>,
    shell: WeakEntity<Shell>,
}

/// The frame of a Marketplace page, read from the shell.
pub(super) fn frame(
    shell: &Shell,
    page: MarketplacePage,
    weak: WeakEntity<Shell>,
) -> MarketplaceFrame {
    MarketplaceFrame {
        snapshot: shell.model.marketplace().clone(),
        page,
        query: shell.model.marketplace_query().to_owned(),
        search: shell.marketplace_search.clone(),
        advanced_open: shell.model.marketplace_advanced_open(),
        error: shell.marketplace_error.clone(),
        item_error: shell.marketplace_item_error.clone(),
        shell: weak,
    }
}

/// The Marketplace page: its left navigation, and the view or item page beside it.
pub(super) fn page(frame: MarketplaceFrame, window: &mut Window, cx: &mut App) -> AnyElement {
    let nav = catalogue::nav_column(&frame, window, cx);
    let content = match frame.page {
        MarketplacePage::Item(id) => item::item_page(&frame, id, cx),
        page => catalogue::catalogue_view(&frame, page, cx),
    };
    div()
        .size_full()
        .flex()
        .flex_row()
        .min_h_0()
        .child(nav)
        .child(
            div()
                .id("marketplace-content")
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(SPACE_6))
                .child(content),
        )
        .into_any_element()
}

impl Shell {
    /// Opens a Marketplace view or an item's page from anywhere in the shell.
    pub(super) fn open_marketplace(&mut self, page: MarketplacePage, cx: &mut Context<Self>) {
        self.marketplace_error = None;
        self.model.open_page(Page::Marketplace(page));
        cx.notify();
    }

    /// Applies one change to the source, then shows its snapshot again. The refusal is returned.
    fn change_marketplace(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut dyn MarketplaceSource) -> Result<(), SourceError>,
    ) -> Result<(), SourceError> {
        let result = change(self.marketplace.as_mut());
        self.model.set_marketplace(self.marketplace.snapshot());
        cx.notify();
        result
    }

    /// Records a refusal under the account it concerns, or clears it after a change that worked.
    fn report_account(
        &mut self,
        account: AccountId,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut dyn MarketplaceSource) -> Result<(), SourceError>,
    ) {
        self.marketplace_error = match self.change_marketplace(cx, change) {
            Ok(()) => None,
            Err(error) => Some((Some(account), error.message())),
        };
        cx.notify();
    }

    pub(super) fn set_marketplace_query(&mut self, query: &str, cx: &mut Context<Self>) {
        self.model.set_marketplace_query(query);
        cx.notify();
    }

    pub(super) fn toggle_marketplace_advanced(&mut self, cx: &mut Context<Self>) {
        let open = self.model.marketplace_advanced_open();
        self.model.set_marketplace_advanced_open(!open);
        cx.notify();
    }

    /// The Switch of an installed item. A service package is changed through the service; the sample changes itself.
    pub(super) fn set_marketplace_enabled(
        &mut self,
        item: MarketplaceItemId,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(package) = self.package_of(item) {
            self.set_package_enabled(item, package, enabled, cx);
            return;
        }
        let _ = self.change_marketplace(cx, |source| source.set_enabled(item, enabled));
    }

    pub(super) fn add_marketplace_account(&mut self, agent_kind: &str, cx: &mut Context<Self>) {
        self.marketplace_error =
            match self.change_marketplace(cx, |source| source.add_account(agent_kind)) {
                Ok(()) => None,
                Err(error) => Some((None, error.message())),
            };
        cx.notify();
    }

    pub(super) fn make_marketplace_account_active(
        &mut self,
        account: AccountId,
        cx: &mut Context<Self>,
    ) {
        self.report_account(account, cx, |source| source.activate_account(account));
    }

    pub(super) fn check_marketplace_account(&mut self, account: AccountId, cx: &mut Context<Self>) {
        self.report_account(account, cx, |source| source.check_account(account));
    }

    pub(super) fn remove_marketplace_account(
        &mut self,
        account: AccountId,
        cx: &mut Context<Self>,
    ) {
        self.report_account(account, cx, |source| source.remove_account(account));
    }

    pub(super) fn sign_in_open_page(&mut self, cx: &mut Context<Self>) {
        let _ = self.change_marketplace(cx, |source| source.open_sign_in_page());
    }

    /// Sends the code the field holds. The field is cleared once the code is accepted.
    pub(super) fn sign_in_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let code = self.marketplace_code.read(cx).text().to_owned();
        if self
            .change_marketplace(cx, |source| source.submit_sign_in_code(&code))
            .is_ok()
        {
            self.marketplace_code
                .update(cx, |field, cx| field.set_text("", window, cx));
        }
    }

    pub(super) fn sign_in_retry(&mut self, cx: &mut Context<Self>) {
        let _ = self.change_marketplace(cx, |source| source.retry_sign_in());
    }

    /// Done and Cancel both end the sign-in; a finished account is already in the snapshot.
    pub(super) fn end_sign_in(&mut self, cx: &mut Context<Self>) {
        self.marketplace.cancel_sign_in();
        self.model.set_marketplace(self.marketplace.snapshot());
        cx.notify();
    }

    pub(super) fn ask_marketplace_uninstall(
        &mut self,
        item: MarketplaceItemId,
        cx: &mut Context<Self>,
    ) {
        self.marketplace_uninstall = Some(UninstallConfirm {
            item,
            error: None,
            sending: false,
        });
        cx.notify();
    }

    pub(super) fn cancel_marketplace_uninstall(&mut self, cx: &mut Context<Self>) {
        self.marketplace_uninstall = None;
        cx.notify();
    }

    /// Uninstalls the item in the confirm. A refusal keeps the confirm open with the reason.
    pub(super) fn confirm_marketplace_uninstall(&mut self, cx: &mut Context<Self>) {
        let Some(confirm) = self.marketplace_uninstall.clone() else {
            return;
        };
        if confirm.sending {
            return;
        }
        if let Some(package) = self.package_of(confirm.item) {
            self.remove_package(confirm.item, package, cx);
            return;
        }
        match self.change_marketplace(cx, |source| source.uninstall(confirm.item)) {
            Ok(()) => self.marketplace_uninstall = None,
            Err(error) => {
                self.marketplace_uninstall = Some(UninstallConfirm {
                    error: Some(error.message()),
                    ..confirm
                });
            }
        }
        cx.notify();
    }
}
