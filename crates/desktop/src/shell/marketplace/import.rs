//! Import on a connected service (decision 0059): `agent.accounts.import` copies a kind's login files from a folder
//! into a new account. The folder is only read, and the answer never carries a secret.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, WeakEntity, div};
use massorca_protocol::methods::agents::AccountsImportParams;
use massorca_ui::components::dialog::Dialog;
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{TYPE_BODY, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

use crate::model::marketplace::{HostTarget, import_outcome, import_update_sentence};
use crate::shell::Shell;
use crate::shell::path_field::{PickKind, path_field};

/// Said when Import is pressed with no folder.
const NO_FOLDER: &str = "Enter the folder that holds the login files.";

/// The import dialog of one kind, while it is open.
pub(in crate::shell) struct ImportDialog {
    kind: String,
    field: Entity<TextField>,
    error: Option<String>,
    /// An import is running, so Import waits for its answer.
    running: bool,
    /// The sentence of an import that updated an existing account, shown until Done.
    done: Option<String>,
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
            done: None,
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
            from: Some(from),
            label: None,
        };
        let agent = agent_name(self, &params.kind);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.accounts_import(params).await;
            let outcome = match result {
                Ok(imported) => {
                    import_outcome(&imported).map(|()| import_update_sentence(&imported, &agent))
                }
                Err(refusal) => Err(refusal.sentence()),
            };
            let _ = this.update(cx, |shell, cx| shell.finish_account_import(outcome, cx));
        })
        .detach();
    }

    fn finish_account_import(
        &mut self,
        outcome: Result<Option<String>, String>,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            Ok(None) => self.marketplace_accounts.import = None,
            Ok(Some(sentence)) => {
                if let Some(dialog) = self.marketplace_accounts.import.as_mut() {
                    dialog.running = false;
                    dialog.done = Some(sentence);
                }
            }
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
    let agent = agent_name(shell, &import.kind);
    if let Some(sentence) = &import.done {
        return Some(updated_dialog(&agent, sentence, weak, cx));
    }
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
        .child(path_field(
            "account-import-browse",
            "account-import-field",
            weak.clone(),
            &import.field,
            PickKind::Folder,
            matches!(shell.marketplace_pick.target(), HostTarget::Local),
            !import.running,
        ))
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

/// The name of the agent of `kind` as the Marketplace lists it, or the kind itself when no listed item offers it.
fn agent_name(shell: &Shell, kind: &str) -> String {
    shell
        .model()
        .marketplace()
        .items
        .iter()
        .find(|item| item.agent_kind.as_deref() == Some(kind))
        .map_or_else(|| kind.to_owned(), |item| item.name.clone())
}

/// The dialog after an import that updated an existing account: its sentence, and Done, which closes it.
fn updated_dialog(agent: &str, sentence: &str, weak: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let (done, dismiss) = (weak.clone(), weak.clone());
    Dialog::new("marketplace-import", format!("Import account to {agent}"))
        .open(true)
        .child(
            div()
                .text_style(TYPE_BODY)
                .text_color(theme.palette.text.primary)
                .child(sentence.to_owned()),
        )
        .action("Done")
        .cancel_label("Close")
        .on_action(move |_, cx| {
            let _ = done.update(cx, |shell, cx| shell.close_account_import(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.close_account_import(cx));
        })
        .into_any_element()
}
