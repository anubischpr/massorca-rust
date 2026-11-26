//! The Marketplace pages: the catalogue and its views, and each item's page (D-EX-01, 7.2, 7.4).
//! Installs and agent accounts go through the source trait, and the snapshot always follows it.

mod accounts;
mod add_folder;
#[cfg(test)]
mod add_menu_tests;
#[cfg(test)]
mod banner_tests;
mod catalogue;
mod dialogs;
#[cfg(test)]
mod extension_tests;
mod extension_updates;
mod host_pick;
mod import;
#[cfg(test)]
mod import_tests;
mod installed;
mod item;
mod item_images;
#[cfg(test)]
mod item_images_tests;
mod log_view;
mod more_menu;
mod package;
mod program_update;
mod review;
mod rollback;
mod services;
#[cfg(test)]
mod services_tests;
mod sign_in;
mod skills;
#[cfg(test)]
mod tests;

use super::Shell;
use crate::model::marketplace::{
    AccountId, HostTarget, ItemKind, MarketplaceItemId, MarketplaceSnapshot, MarketplaceSource,
    RowOffers, SourceError,
};
use crate::model::navigation::{MarketplacePage, Page};
use crate::service::ServiceHandle;
pub(super) use extension_updates::UpdateChecks;
pub(super) use host_pick::{HostPick, host_picker_for};
pub(super) use log_view::RunLogView;
pub(super) use rollback::RollbackConfirm;
pub(super) use services::ServiceDialog;
pub(super) use skills::PlacementReview;
use std::collections::{BTreeMap, BTreeSet};

use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, SharedString, Styled, WeakEntity,
    Window, div, prelude::*, px,
};
use massorca_protocol::methods::extensions::{ExtensionReview, ProgramUpdateReview};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::search_field::SearchField;
use massorca_ui::components::select::SelectView;
use massorca_ui::tokens::{SPACE_4, SPACE_6};

use accounts::AccountWrite;
pub(super) use accounts::AccountsUi;
pub(super) use add_folder::AddFolderDialog;
pub(super) use dialogs::dialog;

/// The uninstall confirm on screen: the item, and the refusal to show in it, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UninstallConfirm {
    pub item: MarketplaceItemId,
    pub error: Option<String>,
    /// The removal is with the service; the confirm waits for its answer.
    pub sending: bool,
    /// The person's tick for removing the CLI the package installed too; only offered when the package installed one.
    pub remove_cli: bool,
    /// Whether Show details is open; it starts closed.
    pub details_open: bool,
}

/// The install review on screen for one package, from the catalogue or a folder. What the service reported is shown
/// as it came; nothing is sent to install until the person presses Install, and a vendor installer also needs its own
/// tick (decision 0062).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct InstallReview {
    /// The catalogue item the review is for; None for a package read from a folder on This computer.
    pub item: Option<MarketplaceItemId>,
    /// The `catalogue:<id>` or `dir:<path>` source the review and the install both name.
    pub source: String,
    /// What `extensions.inspect` reported; `None` while it runs or after it was refused.
    pub review: Option<ExtensionReview>,
    /// The person's tick for running the vendor installer the review lists.
    pub run_installer: bool,
    /// Whether Show details is open; it starts closed.
    pub details_open: bool,
    pub sending: bool,
    pub error: Option<String>,
    /// For an update of a host program MassOrca installed: the program, and its review once read (decision 0151).
    pub program: Option<String>,
    pub update: Option<ProgramUpdateReview>,
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
    /// The accounts the service refused as busy.
    busy: BTreeSet<AccountId>,
    /// The account whose sign-in terminal runs in the dialog.
    signing_in: Option<AccountId>,
    /// The accounts live in the connected service, so they can be imported.
    live_accounts: bool,
    /// Why a kind's accounts could not be listed, by kind.
    list_refusals: Vec<(String, String)>,
    /// The connected service, which serves the listed images; none offline.
    service: Option<ServiceHandle>,
    /// The note above a paired host's items while its answer is pending or refused.
    host_note: Option<String>,
    /// The host picker, which aims the Marketplace at This computer or a paired host.
    host_picker: Entity<SelectView>,
    /// The status word beside the picker, for the host it shows.
    host_status: &'static str,
    /// The Services section's rows of the picked host.
    services: crate::model::marketplace::HostedServicesView,
    /// A refused disconnect, with the service it concerns.
    service_error: Option<(String, String)>,
    /// The Skills section's host and what it shows for that host (SKL-01).
    skills: skills::SkillsSection,
    /// Your apps: the installed skills and connected services of the shown host, and what could not be read.
    apps: installed::YourApps,
    /// What each installed item's More menu offers.
    offers: BTreeMap<MarketplaceItemId, RowOffers>,
    /// Whether the item page shows Update: a host program MassOrca installed, and the service offers updates.
    updatable: bool,
    shell: WeakEntity<Shell>,
}

/// The frame of a Marketplace page, read from the shell.
pub(super) fn frame(
    shell: &Shell,
    page: MarketplacePage,
    weak: WeakEntity<Shell>,
    cx: &App,
) -> MarketplaceFrame {
    let skills = skills::section_of(shell, cx);
    let apps = installed::your_apps(shell, &skills, cx);
    let offers = installed::row_offers_of(shell, cx);
    MarketplaceFrame {
        snapshot: shell.model.marketplace().clone(),
        page,
        query: shell.model.marketplace_query().to_owned(),
        search: shell.marketplace_search.clone(),
        advanced_open: shell.model.marketplace_advanced_open(),
        error: shell.marketplace_error.clone(),
        item_error: shell.marketplace_item_error.clone(),
        busy: shell.marketplace_accounts.busy.clone(),
        signing_in: shell
            .marketplace_accounts
            .sign_in
            .as_ref()
            .and_then(|sign_in| sign_in.running_account()),
        live_accounts: shell.service.is_some(),
        list_refusals: accounts::list_refusals(shell.store.read(cx).state()),
        service: shell.service.clone(),
        host_note: shell.picked_note(cx),
        host_picker: shell.marketplace_host_select.clone(),
        host_status: shell.picker_status(cx),
        services: shell.marketplace_services.clone(),
        service_error: shell.marketplace_service_error.clone(),
        skills,
        apps,
        offers,
        updatable: match page {
            MarketplacePage::Item(id) => shell.updatable_program(id, cx).is_some(),
            _ => false,
        },
        shell: weak,
    }
}

/// The Marketplace page: its left navigation, and the view or item page beside it.
pub(super) fn page(frame: MarketplaceFrame, window: &mut Window, cx: &mut App) -> AnyElement {
    let nav = catalogue::nav_column(&frame, window, cx);
    let content = match frame.page {
        MarketplacePage::Item(id) => item::item_page(&frame, id, cx),
        page @ MarketplacePage::Category(ItemKind::Service) => div()
            .flex()
            .flex_col()
            .gap(px(SPACE_6))
            .child(catalogue::catalogue_view(&frame, page, window, cx))
            .child(services::section(&frame, cx))
            .into_any_element(),
        page => catalogue::catalogue_view(&frame, page, window, cx),
    };
    let content = match &frame.host_note {
        Some(note) => div()
            .flex()
            .flex_col()
            .gap(px(SPACE_4))
            .child(Banner::new(
                SharedString::from("marketplace-host-note"),
                BannerVariant::Info,
                note.clone(),
            ))
            .child(content)
            .into_any_element(),
        None => content,
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
        if matches!(
            page,
            MarketplacePage::Category(ItemKind::Skill) | MarketplacePage::Installed
        ) {
            let target = self.shown_target();
            self.read_skills(&target, false);
        }
        self.model.open_page(Page::Marketplace(page));
        if matches!(
            page,
            MarketplacePage::Category(ItemKind::Service) | MarketplacePage::Installed
        ) {
            self.read_hosted_services(cx);
        }
        cx.notify();
    }

    /// Applies one change to the source, then shows its snapshot again. The refusal is returned.
    fn change_marketplace(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut dyn MarketplaceSource) -> Result<(), SourceError>,
    ) -> Result<(), SourceError> {
        let result = change(self.marketplace.as_mut());
        self.show_marketplace(cx);
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

    /// Add account: on a connected service the sign-in runs in a service terminal inside the dialog.
    pub(super) fn add_marketplace_account(&mut self, agent_kind: &str, cx: &mut Context<Self>) {
        // Accounts are This computer's only; a paired host's accounts are not read yet.
        if matches!(self.marketplace_pick.target(), HostTarget::Paired(_)) {
            self.marketplace_error = Some((
                None,
                "Agent accounts are not available from the service yet.".to_owned(),
            ));
            cx.notify();
            return;
        }
        if self.service.is_some() {
            self.marketplace_error = None;
            self.start_service_sign_in(agent_kind, cx);
            return;
        }
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
        if !self.write_service_account(account, AccountWrite::Activate, cx) {
            self.report_account(account, cx, |source| source.activate_account(account));
        }
    }

    pub(super) fn check_marketplace_account(&mut self, account: AccountId, cx: &mut Context<Self>) {
        if !self.write_service_account(account, AccountWrite::Check, cx) {
            self.report_account(account, cx, |source| source.check_account(account));
        }
    }

    pub(super) fn remove_marketplace_account(
        &mut self,
        account: AccountId,
        cx: &mut Context<Self>,
    ) {
        if !self.write_service_account(account, AccountWrite::Remove, cx) {
            self.report_account(account, cx, |source| source.remove_account(account));
        }
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
        self.show_marketplace(cx);
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
            remove_cli: false,
            details_open: false,
        });
        cx.notify();
    }

    pub(super) fn set_uninstall_details(&mut self, open: bool, cx: &mut Context<Self>) {
        if let Some(confirm) = self.marketplace_uninstall.as_mut() {
            confirm.details_open = open;
        }
        cx.notify();
    }

    /// The CLI tick of the uninstall confirm. Nothing is sent until the confirm's own action.
    pub(super) fn set_marketplace_uninstall_cli(&mut self, remove: bool, cx: &mut Context<Self>) {
        if let Some(confirm) = self.marketplace_uninstall.as_mut() {
            confirm.remove_cli = remove;
        }
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
            self.remove_package(confirm.item, package, confirm.remove_cli, cx);
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
