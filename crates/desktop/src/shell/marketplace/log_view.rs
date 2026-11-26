//! The run log of an installed package, from its More menu: the runs the service lists, each on one line, with its output
//! under Show details when it wrote any (decision 0100).

use std::collections::BTreeSet;

use gpui::Context;
use massorca_protocol::methods::extensions::{ExtensionLog, ExtensionRun};

use crate::model::marketplace::MarketplaceItemId;
use crate::service::Refusal;
use crate::shell::Shell;

/// The run log on screen: the package, its runs once the service answers, and which runs have their output open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::shell) struct RunLogView {
    pub item: MarketplaceItemId,
    /// The package's name, for the dialog's title.
    pub name: String,
    /// The runs in the service's order, or why they could not be listed; None while the service is asked.
    pub runs: Option<Result<Vec<ExtensionRun>, String>>,
    /// The runs whose output is open, by their place in the list.
    pub open_output: BTreeSet<usize>,
}

impl Shell {
    /// Opens the run log of an installed package and asks the service for its runs.
    pub(super) fn ask_marketplace_run_log(
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
        let Some(name) = self
            .model
            .marketplace()
            .item(item)
            .map(|found| found.name.clone())
        else {
            return;
        };
        self.marketplace_run_log = Some(RunLogView {
            item,
            name,
            runs: None,
            open_output: BTreeSet::new(),
        });
        cx.spawn(async move |this, cx| {
            let result = service.extension_log(package).await;
            let _ = this.update(cx, |shell, cx| shell.finish_run_log(item, result, cx));
        })
        .detach();
        cx.notify();
    }

    fn finish_run_log(
        &mut self,
        item: MarketplaceItemId,
        result: Result<ExtensionLog, Refusal>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self
            .marketplace_run_log
            .as_mut()
            .filter(|view| view.item == item)
        {
            view.runs = Some(match result {
                Ok(log) => Ok(log.runs),
                Err(refusal) => Err(refusal.sentence()),
            });
        }
        cx.notify();
    }

    pub(super) fn set_run_log_output(&mut self, index: usize, open: bool, cx: &mut Context<Self>) {
        if let Some(view) = self.marketplace_run_log.as_mut() {
            if open {
                view.open_output.insert(index);
            } else {
                view.open_output.remove(&index);
            }
        }
        cx.notify();
    }

    pub(super) fn close_run_log(&mut self, cx: &mut Context<Self>) {
        self.marketplace_run_log = None;
        cx.notify();
    }
}
