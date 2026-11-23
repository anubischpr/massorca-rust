//! The confirm before an owned secret is deleted (D-VT-02, merged into X-DLG-01). It is destructive: it names the
//! scopes that lose the secret, Cancel has the focus, and nothing is sent until Delete is pressed. The dialog closes
//! only when the service has removed the secret.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, WeakEntity, div, px};
use massorca_protocol::methods::secrets::SecretRemoved;
use massorca_ui::components::dialog::{Dialog, DialogVariant, DialogWidth};
use massorca_ui::tokens::{SPACE_1, SPACE_2, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, Icon, IconName, IconSize, TextStyled};

use super::Shell;
use crate::model::secret_form::{FormStatus, RemoveForm, delete_label, removal_refusal_sentence};
use crate::model::secret_vault::scope_title;
use crate::service::Refusal;

/// The delete confirm on screen: its form, and the title of each scope that links to the secret.
#[derive(Debug)]
pub(super) struct RemoveDialog {
    pub(super) form: RemoveForm,
    pub(super) losses: Vec<String>,
}

impl Shell {
    /// Opens the confirm for the owned secret `name`, with the scopes that link to it, which lose it too.
    pub(super) fn open_remove_secret(
        &mut self,
        scope: String,
        name: String,
        linked_scopes: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let losses = {
            let state = self.store.read(cx).state();
            linked_scopes
                .iter()
                .map(|linked| scope_title(linked, &state.projects))
                .collect()
        };
        self.vault_remove = Some(RemoveDialog {
            form: RemoveForm::new(scope, name, linked_scopes),
            losses,
        });
        cx.notify();
    }

    /// Delete, once confirmed: asks the vault to remove the secret. The dialog waits for the answer.
    pub(super) fn confirm_remove_secret(&mut self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let Some(dialog) = self.vault_remove.as_mut() else {
            return;
        };
        let name = dialog.form.name.clone();
        let Some(service) = service else {
            let refusal = Refusal::unavailable("The service is not connected.");
            dialog
                .form
                .refused(removal_refusal_sentence(&refusal, &name));
            cx.notify();
            return;
        };
        let params = dialog.form.request();
        cx.spawn(async move |this, cx| {
            let result = service.secret_remove(params).await;
            let _ = this.update(cx, |shell, cx| shell.remove_answered(&name, result, cx));
        })
        .detach();
        cx.notify();
    }

    /// The service's answer to a deletion: removed closes the dialog; a refusal keeps it open and says why.
    fn remove_answered(
        &mut self,
        name: &str,
        result: Result<SecretRemoved, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(_) => self.vault_remove = None,
            Err(refusal) => {
                if let Some(dialog) = self.vault_remove.as_mut() {
                    dialog
                        .form
                        .refused(removal_refusal_sentence(&refusal, name));
                }
            }
        }
        cx.notify();
    }

    /// Cancel, Escape or the scrim: nothing is deleted.
    pub(super) fn close_remove_secret(&mut self, cx: &mut Context<Self>) {
        self.vault_remove = None;
        cx.notify();
    }
}

/// The delete confirm on screen. `connected` says whether the service can take the deletion.
pub(super) fn remove_dialog_view(
    dialog: &RemoveDialog,
    shell: &WeakEntity<Shell>,
    connected: bool,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let form = &dialog.form;
    let saving = form.status == FormStatus::Saving;
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child("It is removed from the scopes that use it. Agents there lose it at their next run."),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_1))
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.status.danger_fg)
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size(IconSize::XSmall)
                        .color(theme.palette.status.danger_fg),
                )
                .child("This cannot be undone."),
        );
    let (confirm, dismiss) = (shell.clone(), shell.clone());
    let mut view = Dialog::new(
        "settings-vault-remove",
        format!("Delete secret {}?", form.name),
    )
    .variant(DialogVariant::Destructive)
    .open(true)
    .width(DialogWidth::Medium)
    .child(body)
    .loss(dialog.losses.iter().map(|title| format!("{title} linked")))
    .action(delete_label(&form.linked_scopes))
    .action_disabled(!connected || saving)
    .busy(saving)
    .on_action(move |_, cx| {
        let _ = confirm.update(cx, |shell, cx| shell.confirm_remove_secret(cx));
    })
    .on_dismiss(move |_, cx| {
        let _ = dismiss.update(cx, |shell, cx| shell.close_remove_secret(cx));
    });
    if let FormStatus::Refused(text) = &form.status {
        view = view.error(text.clone());
    }
    view.into_any_element()
}
