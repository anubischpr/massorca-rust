//! The actions on an installed extension package (decision 0062): turning it on or off, and removing it. Each goes to
//! the service, and the page changes only through the reads that follow the answer, never from the answer itself.

use crate::model::marketplace::{
    MarketplaceItemId, PackageAction, extension_refusal_text, package_refusal,
};
use crate::service::Refusal;
use crate::shell::Shell;
use gpui::Context;
use massorca_protocol::methods::extensions::{
    CliRemoval, CliRemovalAction, ExtensionRemoveParams, ExtensionRemoved,
    ExtensionSetEnabledParams,
};

/// Why a CLI the person asked to remove is still on this computer, as a sentence; none when its removal ran.
fn cli_not_removed(removal: &CliRemoval) -> Option<String> {
    match &removal.action {
        CliRemovalAction::Failed { message, .. } => {
            Some(format!("The CLI was not removed: {message}"))
        }
        CliRemovalAction::NoCommand => {
            Some("The CLI was not removed: this system has no uninstall command for it.".to_owned())
        }
        _ => None,
    }
}

impl Shell {
    /// The package an item stands for, when the service lists it as an extension package.
    pub(super) fn package_of(&self, item: MarketplaceItemId) -> Option<String> {
        self.model
            .marketplace()
            .item(item)
            .and_then(|found| found.extension.as_deref())
            .map(|entry| entry.id.clone())
    }

    /// Turns an installed package on or off. The switch keeps the state the service reports, so it does not move
    /// before the answer.
    pub(super) fn set_package_enabled(
        &mut self,
        item: MarketplaceItemId,
        package: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(sentence) = self
            .model
            .marketplace()
            .item(item)
            .and_then(|found| package_refusal(found, PackageAction::Enable))
        {
            self.marketplace_item_error = Some((item, sentence));
            cx.notify();
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.marketplace_item_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service
                .set_extension_enabled(ExtensionSetEnabledParams {
                    id: package,
                    enabled,
                })
                .await;
            let _ = this.update(cx, |shell, cx| {
                shell.finish_package_change(item, result.map(|_| ()), cx);
            });
        })
        .detach();
    }

    /// Removes an installed package the person confirmed. The confirm waits for the answer; a refusal keeps it open
    /// with its sentence. The CLI the package installed is removed only when the person ticked it (`uninstall_cli`).
    pub(super) fn remove_package(
        &mut self,
        item: MarketplaceItemId,
        package: String,
        uninstall_cli: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        if let Some(confirm) = self.marketplace_uninstall.as_mut() {
            confirm.sending = true;
            confirm.error = None;
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service
                .remove_extension(ExtensionRemoveParams {
                    id: package,
                    delete_agent_homes: false,
                    uninstall_cli,
                    agent_token: None,
                    uninstall_consent: None,
                })
                .await;
            let _ = this.update(cx, |shell, cx| {
                shell.finish_package_removal(item, result, uninstall_cli, cx)
            });
        })
        .detach();
    }

    /// A removal that worked closes the confirm. A step that did not finish, or a CLI the person asked to remove that is
    /// still there, is shown under the item, since the package is gone and the item page is the place to read why.
    fn finish_package_removal(
        &mut self,
        item: MarketplaceItemId,
        result: Result<ExtensionRemoved, Refusal>,
        uninstall_cli: bool,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(removed) => {
                if self
                    .marketplace_uninstall
                    .as_ref()
                    .is_some_and(|confirm| confirm.item == item)
                {
                    self.marketplace_uninstall = None;
                }
                let mut steps: Vec<String> = removed
                    .failures
                    .iter()
                    .map(|failure| failure.message.clone())
                    .collect();
                if uninstall_cli {
                    steps.extend(removed.cli.iter().filter_map(cli_not_removed));
                }
                if !steps.is_empty() {
                    self.marketplace_item_error = Some((
                        item,
                        format!("Removed, but not every step finished. {}", steps.join(" ")),
                    ));
                }
            }
            Err(refusal) => {
                if let Some(confirm) = self
                    .marketplace_uninstall
                    .as_mut()
                    .filter(|confirm| confirm.item == item)
                {
                    confirm.sending = false;
                    confirm.error =
                        Some(extension_refusal_text(refusal.code(), &refusal.sentence()));
                }
            }
        }
        cx.notify();
    }

    /// A refused change shows its sentence under the item; a change that worked needs no words here.
    fn finish_package_change(
        &mut self,
        item: MarketplaceItemId,
        result: Result<(), Refusal>,
        cx: &mut Context<Self>,
    ) {
        if let Err(refusal) = result {
            self.marketplace_item_error = Some((
                item,
                extension_refusal_text(refusal.code(), &refusal.sentence()),
            ));
        }
        cx.notify();
    }
}
