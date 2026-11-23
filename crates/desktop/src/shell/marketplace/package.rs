//! The actions on an installed extension package (decision 0062): turning it on or off, and removing it. Each goes to
//! the service, and the page changes only through the reads that follow the answer, never from the answer itself.

use crate::model::marketplace::{MarketplaceItemId, extension_refusal_text};
use crate::service::Refusal;
use crate::shell::Shell;
use gpui::Context;
use massorca_protocol::methods::extensions::ExtensionSetEnabledParams;

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
