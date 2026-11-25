//! Import on a connected service (decision 0059): `agent.accounts.import` copies a kind's login files from a folder
//! into a new account. The folder is only read, and the answer never carries a secret.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, Subscription, WeakEntity, div, px};
use massorca_protocol::methods::agents::AccountsImportParams;
use massorca_ui::components::dialog::Dialog;
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{SPACE_1, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

use crate::model::marketplace::{
    FolderField, HostTarget, ImportCandidate, ImportCandidates, MANUAL_PLACEHOLDER, candidate_for,
    folder_field, import_outcome,
};
use crate::shell::Shell;
use crate::shell::path_field::{PickKind, path_field};

/// Said when Import is pressed with no folder.
const NO_FOLDER: &str = "Enter the folder that holds the login files.";

/// The import dialog of one kind, while it is open.
pub(in crate::shell) struct ImportDialog {
    kind: String,
    field: Entity<TextField>,
    /// The placeholder the field shows while empty: the folder the import expects, else the manual prompt.
    placeholder: String,
    /// What the service detected for the kind; `None` while the capability is absent or not yet answered.
    detection: Option<ImportCandidates>,
    error: Option<String>,
    /// An import is running, so Import waits for its answer.
    running: bool,
    /// Redraws the dialog as the folder changes, so its status line follows what is typed.
    _changes: Subscription,
}

impl ImportDialog {
    /// The candidate the service listed for this kind.
    fn candidate(&self) -> Option<&ImportCandidate> {
        self.detection
            .as_ref()
            .and_then(|found| candidate_for(found, &self.kind))
    }

    /// What the dialog shows for the folder `edited` holds: `None` when it opens, the typed text afterwards.
    fn folder(&self, edited: Option<&str>) -> FolderField {
        folder_field(self.detection.is_some(), self.candidate(), edited)
    }

    #[cfg(test)]
    pub(in crate::shell) fn field(&self) -> &Entity<TextField> {
        &self.field
    }

    #[cfg(test)]
    pub(in crate::shell) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[cfg(test)]
    pub(in crate::shell) fn placeholder(&self) -> &str {
        &self.placeholder
    }

    #[cfg(test)]
    pub(in crate::shell) fn detection(&self) -> Option<&ImportCandidates> {
        self.detection.as_ref()
    }
}

/// The folder field holding `text`, showing `placeholder` while empty, with the subscription that redraws the dialog.
fn folder_entity(
    cx: &mut Context<Shell>,
    placeholder: &str,
    text: &str,
) -> (Entity<TextField>, Subscription) {
    let field = cx.new(|cx| {
        TextField::new(cx)
            .label("Folder")
            .placeholder(placeholder.to_owned())
            .with_text(text)
            .mono()
    });
    let changes = cx.observe(&field, |_, _, cx| cx.notify());
    (field, changes)
}

impl Shell {
    /// Opens the import dialog of `kind`; its folder field fills in once the service answers what it detects.
    pub(super) fn open_account_import(&mut self, kind: &str, cx: &mut Context<Self>) {
        let (field, changes) = folder_entity(cx, MANUAL_PLACEHOLDER, "");
        self.marketplace_accounts.import = Some(ImportDialog {
            kind: kind.to_owned(),
            field,
            placeholder: MANUAL_PLACEHOLDER.to_owned(),
            detection: None,
            error: None,
            running: false,
            _changes: changes,
        });
        cx.notify();
        self.detect_account_import(kind, cx);
    }

    pub(super) fn close_account_import(&mut self, cx: &mut Context<Self>) {
        self.marketplace_accounts.import = None;
        cx.notify();
    }

    /// Asks the service what it detects for `kind`. Without the capability the answer is none and the field stays manual.
    fn detect_account_import(&self, kind: &str, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        if !self.store.read(cx).state().agent_import_detect {
            return;
        }
        let kind = kind.to_owned();
        cx.spawn(async move |this, cx| {
            let detection = service.accounts_import_candidates(&kind).await;
            let _ = this.update(cx, |shell, cx| {
                shell.finish_account_detection(&kind, detection, cx)
            });
        })
        .detach();
    }

    /// Prefills the folder from what the service detected, unless the person has typed into the field already.
    pub(in crate::shell) fn finish_account_detection(
        &mut self,
        kind: &str,
        detection: Option<ImportCandidates>,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.marketplace_accounts.import.as_mut() else {
            return;
        };
        if dialog.kind != kind || detection.is_none() {
            return;
        }
        dialog.detection = detection;
        let shown = dialog.folder(None);
        let typed = dialog.field.read(cx).text().to_owned();
        let text = if typed.is_empty() { shown.value } else { typed };
        let (field, changes) = folder_entity(cx, &shown.placeholder, &text);
        dialog.field = field;
        dialog._changes = changes;
        dialog.placeholder = shown.placeholder;
        cx.notify();
    }

    /// Imports from the folder the field holds, or from the detected folder while it is kept. The dialog stays open
    /// with the reason when nothing was kept, and the rows follow the read the answer triggers.
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
        let typed = dialog.field.read(cx).text().to_owned();
        let from = dialog.folder(Some(typed.as_str())).from;
        if from.as_deref().is_some_and(str::is_empty) {
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
    let typed = import.field.read(cx).text().to_owned();
    let folder = import.folder(Some(typed.as_str()));
    let (submit, dismiss) = (weak.clone(), weak.clone());
    let mut folder_section = div().flex().flex_col().gap(px(SPACE_1)).child(path_field(
        "account-import-browse",
        "account-import-field",
        weak.clone(),
        &import.field,
        PickKind::Folder,
        matches!(shell.marketplace_pick.target(), HostTarget::Local),
        !import.running,
    ));
    if let Some(status) = folder.status {
        folder_section = folder_section.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(status),
        );
    }
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
        .child(folder_section)
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
