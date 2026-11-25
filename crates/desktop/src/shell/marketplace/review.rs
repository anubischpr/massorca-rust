//! The install review of a catalogue package (decision 0062). The review asks the service what installing does and
//! keeps the answer on screen. The install is sent only from the person's press of Install, with the consent of
//! the review they read; nothing is installed on an answer alone.

use super::InstallReview;
use crate::model::marketplace::{
    MarketplaceItemId, PackageAction, extension_refusal_text, package_refusal,
};
use crate::service::Refusal;
use crate::shell::Shell;
use gpui::Context;
use massorca_protocol::methods::extensions::{
    ExtensionInstallParams, ExtensionInstalled, ExtensionReview,
};

impl Shell {
    /// Opens the review of a catalogue package and asks the service what installing it would do.
    pub(super) fn open_extension_review(
        &mut self,
        item: MarketplaceItemId,
        cx: &mut Context<Self>,
    ) {
        if let Some(sentence) = self
            .model
            .marketplace()
            .item(item)
            .and_then(|found| package_refusal(found, PackageAction::Install))
        {
            self.marketplace_item_error = Some((item, sentence));
            cx.notify();
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(package) = self
            .model
            .marketplace()
            .item(item)
            .and_then(|found| found.extension.as_deref())
            .map(|entry| entry.id.clone())
        else {
            return;
        };
        let source = format!("catalogue:{package}");
        self.marketplace_review = Some(InstallReview {
            item: Some(item),
            source: source.clone(),
            review: None,
            run_installer: false,
            sending: false,
            error: None,
        });
        cx.notify();
        let host = self.paired_host_of(item);
        let picked = self.marketplace_pick.target().clone();
        cx.spawn(async move |this, cx| {
            let result = match host {
                Some(name) => service.inspect_host_extension(name, source).await,
                None => service.inspect_extension(source).await,
            };
            let _ = this.update(cx, |shell, cx| {
                // An answer for a host no longer picked is dropped.
                if shell.marketplace_pick.target() == &picked {
                    shell.finish_inspect(Some(item), result, cx);
                }
            });
        })
        .detach();
    }

    /// Opens the review of a package in a folder on This computer, which asks the service what installing it does.
    pub(super) fn open_folder_review(&mut self, source: String, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        self.marketplace_review = Some(InstallReview {
            item: None,
            source: source.clone(),
            review: None,
            run_installer: false,
            sending: false,
            error: None,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.inspect_extension(source).await;
            let _ = this.update(cx, |shell, cx| shell.finish_inspect(None, result, cx));
        })
        .detach();
    }

    fn finish_inspect(
        &mut self,
        item: Option<MarketplaceItemId>,
        result: Result<ExtensionReview, Refusal>,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self
            .marketplace_review
            .as_mut()
            .filter(|dialog| dialog.item == item && dialog.review.is_none())
        {
            match result {
                Ok(review) => dialog.review = Some(review),
                Err(refusal) => {
                    dialog.error =
                        Some(extension_refusal_text(refusal.code(), &refusal.sentence()));
                }
            }
        }
        cx.notify();
    }

    /// The tick for running the vendor installer the review lists. It is the person's own choice, never a default.
    pub(super) fn set_install_run_installer(&mut self, run: bool, cx: &mut Context<Self>) {
        if let Some(dialog) = self.marketplace_review.as_mut() {
            dialog.run_installer = run;
        }
        cx.notify();
    }

    /// Install, pressed on a review that was read. A review that runs a vendor installer needs its tick first.
    pub(super) fn confirm_extension_install(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(item) = self.marketplace_review.as_ref().map(|dialog| dialog.item) else {
            return;
        };
        let host = item.and_then(|id| self.paired_host_of(id));
        let request = self.marketplace_pick.request();
        let picked = self.marketplace_pick.target().clone();
        let Some(dialog) = self.marketplace_review.as_mut() else {
            return;
        };
        let Some(review) = dialog.review.as_ref() else {
            return;
        };
        if dialog.sending || (review.runs_installer && !dialog.run_installer) {
            return;
        }
        let params = ExtensionInstallParams {
            source: dialog.source.clone(),
            consent: Some(review.consent.clone()),
            run_installer: review.runs_installer,
            sha256: None,
            agent_token: None,
        };
        dialog.sending = true;
        dialog.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match host {
                Some(name) => service.install_host_extension(name, request, params).await,
                None => service.install_extension(params).await,
            };
            let _ = this.update(cx, |shell, cx| {
                // An answer for a host no longer picked is dropped.
                if shell.marketplace_pick.target() == &picked {
                    shell.finish_install(item, result, cx);
                }
            });
        })
        .detach();
    }

    /// An install that worked closes the review; the list follows the reads that the service sends after it.
    fn finish_install(
        &mut self,
        item: Option<MarketplaceItemId>,
        result: Result<ExtensionInstalled, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(_) => {
                if self
                    .marketplace_review
                    .as_ref()
                    .is_some_and(|dialog| dialog.item == item)
                {
                    self.marketplace_review = None;
                }
            }
            Err(refusal) => {
                if let Some(dialog) = self
                    .marketplace_review
                    .as_mut()
                    .filter(|dialog| dialog.item == item)
                {
                    dialog.sending = false;
                    dialog.error =
                        Some(extension_refusal_text(refusal.code(), &refusal.sentence()));
                }
            }
        }
        cx.notify();
    }

    /// Closes the review. An install already sent still finishes; its answer finds no review to show.
    pub(super) fn cancel_extension_review(&mut self, cx: &mut Context<Self>) {
        self.marketplace_review = None;
        cx.notify();
    }
}
