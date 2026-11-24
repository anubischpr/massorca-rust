//! The update checks of installed packages (decision 0062). A folder or archive package is inspected once per read of the
//! lists; a review whose sha256 differs from the installed one lists the package on Updates with that review's version,
//! and its Update button sends `extensions.update` with that review's consent. Built-in packages are not inspected: the
//! service refreshes them at start.

use std::collections::BTreeMap;

use gpui::Context;
use massorca_protocol::methods::extensions::{
    ExtensionInstalled, ExtensionItem, ExtensionReview, ExtensionUpdateParams, SourceKind,
};

use crate::model::marketplace::MarketplaceItemId;
use crate::model::navigation::MarketplacePage;
use crate::service::Refusal;
use crate::shell::Shell;

/// The check of the read that ran last, and the review each checked package returned. A package whose inspect was
/// refused has no review, so it is off Updates.
#[derive(Debug, Default)]
pub(in crate::shell) struct UpdateChecks {
    read: Option<u64>,
    reviews: BTreeMap<String, ExtensionReview>,
}

impl UpdateChecks {
    /// The packages a new read needs inspected, each with the source to inspect it at; None for a read already checked.
    /// A package no longer installed loses its review.
    pub(in crate::shell) fn begin(
        &mut self,
        read: u64,
        installed: &[ExtensionItem],
    ) -> Option<Vec<(String, String)>> {
        if self.read == Some(read) {
            return None;
        }
        self.read = Some(read);
        let checks: Vec<(String, String)> = installed
            .iter()
            .filter_map(|entry| Some((entry.id.clone(), inspect_source(entry)?)))
            .collect();
        self.reviews
            .retain(|package, _| checks.iter().any(|(id, _)| id == package));
        Some(checks)
    }

    fn held_reviews(&self) -> Vec<ExtensionReview> {
        self.reviews.values().cloned().collect()
    }
}

impl Shell {
    /// Starts the inspects of a new read in the background. Only a new read asks, so drawing the page never does.
    pub(in crate::shell) fn start_update_checks(
        &mut self,
        read: u64,
        checks: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        for (package, source) in checks {
            let service = service.clone();
            cx.spawn(async move |this, cx| {
                let result = service.inspect_extension(source).await;
                let _ = this.update(cx, |shell, cx| {
                    shell.finish_update_check(read, package, result, cx)
                });
            })
            .detach();
        }
    }

    /// Keeps a checked package's review, or drops it when its inspect was refused, and offers the updates again.
    fn finish_update_check(
        &mut self,
        read: u64,
        package: String,
        result: Result<ExtensionReview, Refusal>,
        cx: &mut Context<Self>,
    ) {
        if self.marketplace_updates.read != Some(read) {
            return;
        }
        match result {
            Ok(review) => {
                self.marketplace_updates.reviews.insert(package, review);
            }
            Err(_) => {
                self.marketplace_updates.reviews.remove(&package);
            }
        }
        self.marketplace
            .follow_update_reviews(&self.marketplace_updates.held_reviews());
        self.show_marketplace(cx);
        cx.notify();
    }

    /// The Update button: sends `extensions.update` with the consent of the review the check kept. A refusal opens the
    /// item page, where its sentence is shown; a change that worked arrives as a new read and is checked again.
    pub(super) fn update_package(&mut self, item: MarketplaceItemId, cx: &mut Context<Self>) {
        // Update checks run on This computer only, so a paired host's package is not updated from here yet.
        if self.paired_host_of(item).is_some() {
            return;
        }
        let Some(package) = self.package_of(item) else {
            return;
        };
        let Some(review) = self.marketplace_updates.reviews.get(&package).cloned() else {
            return;
        };
        let Some(service) = self.service.clone() else {
            return;
        };
        let archive = self
            .model
            .marketplace()
            .item(item)
            .and_then(|found| found.extension.as_deref())
            .and_then(|entry| entry.installed.as_ref())
            .is_some_and(|installed| installed.source.kind == SourceKind::File);
        let params = ExtensionUpdateParams {
            id: package,
            consent: Some(review.consent),
            sha256: archive.then_some(review.sha256),
            agent_token: None,
        };
        self.marketplace_item_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.update_extension(params).await;
            let _ = this.update(cx, |shell, cx| shell.finish_update(item, result, cx));
        })
        .detach();
    }

    fn finish_update(
        &mut self,
        item: MarketplaceItemId,
        result: Result<ExtensionInstalled, Refusal>,
        cx: &mut Context<Self>,
    ) {
        let refused = result.is_err();
        self.finish_package_change(item, result.map(|_| ()), cx);
        if refused {
            self.open_marketplace(MarketplacePage::Item(item), cx);
        }
    }
}

/// The source `extensions.inspect` reads a checked package at: its folder or archive. A built-in package refreshes at
/// service start, and a source this build does not know has no check, so both give None.
fn inspect_source(entry: &ExtensionItem) -> Option<String> {
    let source = &entry.installed.as_ref()?.source;
    match source.kind {
        SourceKind::Dir => Some(format!("dir:{}", source.reference)),
        SourceKind::File => Some(format!("file:{}", source.reference)),
        SourceKind::Catalogue | SourceKind::Unknown => None,
    }
}
