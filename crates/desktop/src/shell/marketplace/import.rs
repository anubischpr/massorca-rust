//! Import on a connected service (decision 0059): `agent.accounts.import` copies a kind's login files from a folder
//! into a new account. The folder is only read, and the answer never carries a secret.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, WeakEntity, div};
use massorca_protocol::methods::agents::AccountsImportParams;
use massorca_ui::components::dialog::Dialog;
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::TYPE_BODY_SM;
use massorca_ui::{ActiveTheme, TextStyled};

use crate::model::marketplace::import_outcome;
use crate::shell::Shell;

/// Said when Import is pressed with no folder.
const NO_FOLDER: &str = "Enter the folder that holds the login files.";

/// The import dialog of one kind, while it is open.
pub(in crate::shell) struct ImportDialog {
    kind: String,
    field: Entity<TextField>,
    error: Option<String>,
    /// An import is running, so Import waits for its answer.
    running: bool,
}

impl ImportDialog {
    #[cfg(test)]
    pub(in crate::shell) fn field(&self) -> &Entity<TextField> {
        &self.field
    }

    #[cfg(test)]
    pub(in crate::shell) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl Shell {
    /// Opens the import dialog of `kind`, with an empty folder field.
    pub(super) fn open_account_import(&mut self, kind: &str, cx: &mut Context<Self>) {
        let field = cx.new(|cx| {
            TextField::new(cx)
                .label("Folder")
                .placeholder("Folder that holds the login files")
                .mono()
        });
        self.marketplace_accounts.import = Some(ImportDialog {
            kind: kind.to_owned(),
            field,
            error: None,
            running: false,
        });
        cx.notify();
    }

    pub(super) fn close_account_import(&mut self, cx: &mut Context<Self>) {
        self.marketplace_accounts.import = None;
        cx.notify();
    }

    /// Imports from the folder the field holds. The dialog stays open with the reason when nothing was kept, and the
    /// rows follow the read the answer triggers.
    pub(super) fn submit_account_import(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(dialog) = self.marketplace_accounts.import.as_mut() else {
            return;
        };
        if dialog.running {
            return;
        }
        let from = dialog.field.read(cx).text().trim().to_owned();
        if from.is_empty() {
            dialog.error = Some(NO_FOLDER.to_owned());
            cx.notify();
            return;
        }
        dialog.running = true;
        dialog.error = None;
        let params = AccountsImportParams {
            kind: dialog.kind.clone(),
            from,
            label: None,
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.accounts_import(params).await;
            let outcome = match result {
                Ok(imported) => import_outcome(&imported),
                Err(refusal) => Err(refusal.sentence()),
            };
            let _ = this.update(cx, |shell, cx| shell.finish_account_import(outcome, cx));
        })
        .detach();
    }

    fn finish_account_import(&mut self, outcome: Result<(), String>, cx: &mut Context<Self>) {
        match outcome {
            Ok(()) => self.marketplace_accounts.import = None,
            Err(reason) => {
                if let Some(dialog) = self.marketplace_accounts.import.as_mut() {
                    dialog.running = false;
                    dialog.error = Some(reason);
                }
            }
        }
        cx.notify();
    }
}

/// The import dialog, if one is open.
pub(super) fn dialog(shell: &Shell, weak: &WeakEntity<Shell>, cx: &App) -> Option<AnyElement> {
    let import = shell.marketplace_accounts.import.as_ref()?;
    let agent = shell
        .model()
        .marketplace()
        .items
        .iter()
        .find(|item| item.agent_kind.as_deref() == Some(import.kind.as_str()))
        .map_or_else(|| import.kind.clone(), |item| item.name.clone());
    let theme = *cx.theme();
    let (submit, dismiss) = (weak.clone(), weak.clone());
    let mut dialog = Dialog::new("marketplace-import", format!("Import account to {agent}"))
        .open(true)
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(
                    "Copies the agent's login files from a folder where the service runs into a new account. The folder is only read.",
                ),
        )
        .child(import.field.clone())
        .action(if import.running { "Importing..." } else { "Import" })
        .action_disabled(import.running)
        .cancel_label("Cancel")
        .on_action(move |_, cx| {
            let _ = submit.update(cx, |shell, cx| shell.submit_account_import(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.close_account_import(cx));
        });
    if let Some(error) = &import.error {
        dialog = dialog.error(error.clone());
    }
    Some(dialog.into_any_element())
}
