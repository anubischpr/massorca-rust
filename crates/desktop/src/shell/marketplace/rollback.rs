//! Roll back from an installed row's More menu (decision 0100). The first call names no consent: the service answers with
//! the restored version and its consent, the confirm shows them, and the call is sent again with that consent.

use gpui::Context;
use massorca_protocol::codes;
use massorca_protocol::methods::extensions::ExtensionRollbackParams;

use crate::model::marketplace::{MarketplaceItemId, extension_refusal_text};
use crate::model::navigation::MarketplacePage;
use crate::service::{Refusal, RollbackAnswer};
use crate::shell::Shell;

/// The rollback confirm on screen: the package, the version it puts back, and the consent that accepts that version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::shell) struct RollbackConfirm {
    pub item: MarketplaceItemId,
    /// The package's name, for the confirm's words.
    pub name: String,
    /// The version the package is at now.
    pub from: String,
    /// The version the rollback puts back.
    pub version: String,
    /// The consent of the restored version's review, which the confirm sends.
    pub consent: String,
    /// The refusal to show in the confirm, if any.
    pub error: Option<String>,
    /// Whether the rollback is with the service; the confirm waits for its answer.
    pub sending: bool,
    /// Whether Show details is open; it starts closed.
    pub details_open: bool,
}

impl Shell {
    /// Asks the service what a rollback restores. Its answer opens the confirm, or says why nothing can be restored.
    pub(super) fn ask_marketplace_rollback(
        &mut self,
        item: MarketplaceItemId,
        cx: &mut Context<Self>,
    ) {
        let Some(package) = self.package_of(item) else {
            return;
        };
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(found) = self.model.marketplace().item(item) else {
            return;
        };
        let (name, from) = (found.name.clone(), found.version.clone());
        self.marketplace_item_error = None;
        let params = ExtensionRollbackParams {
            id: package,
            consent: None,
            agent_token: None,
        };
        cx.spawn(async move |this, cx| {
            let result = service.rollback_extension(params).await;
            let _ = this.update(cx, |shell, cx| {
                shell.finish_rollback_ask(item, name, from, result, cx)
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_rollback_ask(
        &mut self,
        item: MarketplaceItemId,
        name: String,
        from: String,
        result: Result<RollbackAnswer, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(RollbackAnswer::ConsentNeeded { version, consent }) => {
                self.marketplace_rollback = Some(RollbackConfirm {
                    item,
                    name,
                    from,
                    version,
                    consent,
                    error: None,
                    sending: false,
                    details_open: false,
                });
            }
            Ok(RollbackAnswer::Restored(_)) => {}
            Err(refusal) => {
                self.marketplace_item_error = Some((
                    item,
                    extension_refusal_text(refusal.code(), &refusal.sentence()),
                ));
                self.open_marketplace(MarketplacePage::Item(item), cx);
            }
        }
        cx.notify();
    }

    /// Sends the rollback with the consent the confirm shows. A refusal keeps the confirm open with its reason, except
    /// nothing kept, which closes it and says so on the item page.
    pub(super) fn confirm_marketplace_rollback(&mut self, cx: &mut Context<Self>) {
        let Some(confirm) = self.marketplace_rollback.clone() else {
            return;
        };
        if confirm.sending {
            return;
        }
        let (Some(service), Some(package)) = (self.service.clone(), self.package_of(confirm.item))
        else {
            return;
        };
        let item = confirm.item;
        let params = ExtensionRollbackParams {
            id: package,
            consent: Some(confirm.consent.clone()),
            agent_token: None,
        };
        self.marketplace_rollback = Some(RollbackConfirm {
            sending: true,
            error: None,
            ..confirm
        });
        cx.spawn(async move |this, cx| {
            let result = service.rollback_extension(params).await;
            let _ = this.update(cx, |shell, cx| shell.finish_rollback(item, result, cx));
        })
        .detach();
        cx.notify();
    }

    fn finish_rollback(
        &mut self,
        item: MarketplaceItemId,
        result: Result<RollbackAnswer, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(_) => self.marketplace_rollback = None,
            Err(refusal) if refusal.code() == Some(codes::EXTENSION_NO_PREVIOUS_VERSION) => {
                self.marketplace_rollback = None;
                self.marketplace_item_error = Some((
                    item,
                    extension_refusal_text(refusal.code(), &refusal.sentence()),
                ));
                self.open_marketplace(MarketplacePage::Item(item), cx);
            }
            Err(refusal) => {
                if let Some(confirm) = self.marketplace_rollback.as_mut() {
                    confirm.sending = false;
                    confirm.error =
                        Some(extension_refusal_text(refusal.code(), &refusal.sentence()));
                }
            }
        }
        cx.notify();
    }

    pub(super) fn set_rollback_details(&mut self, open: bool, cx: &mut Context<Self>) {
        if let Some(confirm) = self.marketplace_rollback.as_mut() {
            confirm.details_open = open;
        }
        cx.notify();
    }

    pub(super) fn cancel_marketplace_rollback(&mut self, cx: &mut Context<Self>) {
        self.marketplace_rollback = None;
        cx.notify();
    }
}
