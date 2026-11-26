//! Update of a host program MassOrca installed (decision 0151), and the More menu of an item card. The update reuses
//! the install review dialog: the service reports the command, and Update sends exactly that review's consent.

use super::InstallReview;
use crate::model::marketplace::{
    ItemMenuId, MarketplaceItemId, extension_refusal_text, item_menu, updatable_program,
};
use crate::model::navigation::MarketplacePage;
use crate::service::Refusal;
use crate::shell::Shell;
use crate::shell::workspace_dialogs::MenuTarget;
use gpui::{App, Context, Window};
use massorca_protocol::methods::extensions::{
    ProgramUpdateParams, ProgramUpdateReview, ProgramUpdated,
};
use massorca_ui::components::menu::{MenuEntry, MenuItem};
use massorca_ui::components::popover::OverlayLayer;

impl Shell {
    /// The host program Update acts on for `item`, when the service offers it and MassOrca installed the program.
    pub(super) fn updatable_program(&self, item: MarketplaceItemId, cx: &App) -> Option<String> {
        let offered = self.store.read(cx).state().program_update;
        let found = self.model.marketplace().item(item)?;
        updatable_program(found, offered, std::env::consts::OS)
    }

    /// Opens the update review and asks the service what the update would run. Nothing runs until Update.
    pub(super) fn open_program_update(&mut self, item: MarketplaceItemId, cx: &mut Context<Self>) {
        let Some(program) = self.updatable_program(item, cx) else {
            return;
        };
        let (Some(service), Some(package)) = (
            self.service.clone(),
            self.model
                .marketplace()
                .item(item)
                .and_then(|found| found.extension.as_deref())
                .map(|entry| entry.id.clone()),
        ) else {
            return;
        };
        self.marketplace_review = Some(InstallReview {
            item: Some(item),
            source: format!("catalogue:{package}"),
            review: None,
            run_installer: false,
            details_open: false,
            sending: false,
            error: None,
            program: Some(program.clone()),
            update: None,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.review_program_update(package, program).await;
            let _ = this.update(cx, |shell, cx| shell.finish_update_review(item, result, cx));
        })
        .detach();
    }

    fn finish_update_review(
        &mut self,
        item: MarketplaceItemId,
        result: Result<ProgramUpdateReview, Refusal>,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self
            .marketplace_review
            .as_mut()
            .filter(|dialog| dialog.item == Some(item) && dialog.program.is_some())
        {
            match result {
                Ok(review) => dialog.update = Some(review),
                Err(refusal) => {
                    dialog.error =
                        Some(extension_refusal_text(refusal.code(), &refusal.sentence()));
                }
            }
        }
        cx.notify();
    }

    /// Update, pressed on a review that was read: sends that review's consent.
    pub(super) fn confirm_program_update(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(dialog) = self.marketplace_review.as_mut() else {
            return;
        };
        let (Some(item), Some(review)) = (dialog.item, dialog.update.as_ref()) else {
            return;
        };
        if dialog.sending {
            return;
        }
        let params = ProgramUpdateParams {
            id: review.id.clone(),
            program: review.program.clone(),
            consent: Some(review.consent.clone()),
            agent_token: None,
        };
        dialog.sending = true;
        dialog.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.update_program(params).await;
            let _ = this.update(cx, |shell, cx| {
                shell.finish_program_update(item, result, cx)
            });
        })
        .detach();
    }

    fn finish_program_update(
        &mut self,
        item: MarketplaceItemId,
        result: Result<ProgramUpdated, Refusal>,
        cx: &mut Context<Self>,
    ) {
        let open = |shell: &Shell| {
            shell
                .marketplace_review
                .as_ref()
                .is_some_and(|dialog| dialog.item == Some(item) && dialog.program.is_some())
        };
        match result {
            Ok(_) if open(self) => self.marketplace_review = None,
            Ok(_) => {}
            Err(refusal) => {
                if let Some(dialog) = self
                    .marketplace_review
                    .as_mut()
                    .filter(|dialog| dialog.item == Some(item) && dialog.program.is_some())
                {
                    dialog.sending = false;
                    dialog.error =
                        Some(extension_refusal_text(refusal.code(), &refusal.sentence()));
                }
            }
        }
        cx.notify();
    }

    /// The More menu of an item card: only the actions that work for it.
    pub(super) fn open_item_menu(
        &mut self,
        item: MarketplaceItemId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let updatable = self.updatable_program(item, cx).is_some();
        let Some(found) = self.model.marketplace().item(item) else {
            return;
        };
        let rows = item_menu(found, updatable);
        let entries = rows
            .iter()
            .map(|row| {
                let item = match row {
                    ItemMenuId::Open => MenuItem::new("Open"),
                    ItemMenuId::Update => MenuItem::new("Update"),
                    ItemMenuId::Uninstall => MenuItem::new("Uninstall").destructive(),
                };
                MenuEntry::Item(item)
            })
            .collect();
        self.open_menu(
            entries,
            MenuTarget::MarketplaceItem { item, rows },
            OverlayLayer::Page,
            window.mouse_position(),
            window,
            cx,
        );
    }

    /// Runs a chosen row of an item's More menu.
    pub(in crate::shell) fn run_item_menu(
        &mut self,
        item: MarketplaceItemId,
        row: ItemMenuId,
        cx: &mut Context<Self>,
    ) {
        match row {
            ItemMenuId::Open => self.open_marketplace(MarketplacePage::Item(item), cx),
            ItemMenuId::Update => self.open_program_update(item, cx),
            ItemMenuId::Uninstall => self.ask_marketplace_uninstall(item, cx),
        }
    }
}
