//! The dialog that adds or rotates a secret from Settings (X-VT-01, decision 0071). The value comes from the
//! clipboard and is never drawn: the dialog says only that one is ready. Save sends it once and drops it, and the
//! dialog closes only when the service has stored it.

use std::fmt;

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, Subscription, WeakEntity, div, px};
use massorca_protocol::methods::secrets::SecretEntry;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::{Dialog, DialogWidth};
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{SPACE_2, SPACE_3, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled, Theme};

use super::Shell;
use super::settings::value_box;
use crate::model::secret_form::{
    FormMode, FormStatus, SecretForm, refusal_sentence, scope_choices,
};
use crate::model::secret_vault::scope_title;
use crate::service::Refusal;

/// The add or rotate dialog on screen: its form, the name field of an add, and the scopes it can store to, each
/// with the title its scope shows.
pub(super) struct VaultDialog {
    pub(super) form: SecretForm,
    /// The name an add types. A rotation shows its name as text and has no field.
    pub(super) name_field: Option<Entity<TextField>>,
    /// Redraws the dialog as the name changes, so Save follows what is typed.
    _name_changes: Option<Subscription>,
    pub(super) scopes: Vec<(String, String)>,
}

impl fmt::Debug for VaultDialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultDialog")
            .field("form", &self.form)
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

impl Shell {
    /// Opens Add with the first project preselected, as the design has it.
    pub(super) fn open_add_secret(&mut self, cx: &mut Context<Self>) {
        let scopes: Vec<(String, String)> = {
            let state = self.store.read(cx).state();
            scope_choices(&state.projects)
                .into_iter()
                .map(|scope| {
                    let title = scope_title(&scope, &state.projects);
                    (scope, title)
                })
                .collect()
        };
        let Some((first, _)) = scopes.first().cloned() else {
            return;
        };
        let name_field = cx.new(|cx| TextField::new(cx).placeholder("OPENAI_API_KEY").mono());
        let name_changes = cx.observe(&name_field, |_, _, cx| cx.notify());
        self.vault_dialog = Some(VaultDialog {
            form: SecretForm::add(first),
            name_field: Some(name_field),
            _name_changes: Some(name_changes),
            scopes,
        });
        cx.notify();
    }

    /// Opens Rotate for the owned secret `name` in `scope`. Its name and scope stay as they are.
    pub(super) fn open_rotate_secret(
        &mut self,
        scope: String,
        name: String,
        cx: &mut Context<Self>,
    ) {
        let title = scope_title(&scope, &self.store.read(cx).state().projects);
        self.vault_dialog = Some(VaultDialog {
            form: SecretForm::rotate(scope.clone(), name),
            name_field: None,
            _name_changes: None,
            scopes: vec![(scope, title)],
        });
        cx.notify();
    }

    /// Chooses the scope an add stores to, by its segment.
    pub(super) fn choose_secret_scope(&mut self, index: usize, cx: &mut Context<Self>) {
        let scope = self
            .vault_dialog
            .as_ref()
            .and_then(|dialog| dialog.scopes.get(index))
            .map(|(scope, _)| scope.clone());
        if let (Some(dialog), Some(scope)) = (self.vault_dialog.as_mut(), scope) {
            dialog.form.scope = scope;
        }
        cx.notify();
    }

    /// Takes the clipboard text as the value to send. The text is never drawn.
    pub(super) fn paste_secret_value(&mut self, cx: &mut Context<Self>) {
        let text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        if let Some(dialog) = self.vault_dialog.as_mut() {
            dialog.form.paste(&text);
        }
        cx.notify();
    }

    /// Save: checks the name, takes the value and sends it. The dialog stays until the service stores the secret.
    pub(super) fn save_secret(&mut self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let Some(dialog) = self.vault_dialog.as_mut() else {
            return;
        };
        let name = match &dialog.name_field {
            Some(field) => field.read(cx).text().to_owned(),
            None => dialog.form.name.clone(),
        };
        let Some(service) = service else {
            let refusal = Refusal::unavailable("The service is not connected.");
            dialog.form.refused(refusal_sentence(&refusal, &name));
            cx.notify();
            return;
        };
        match dialog.form.request(&name) {
            Ok(params) => {
                cx.spawn(async move |this, cx| {
                    let result = service.secret_set(params).await;
                    let _ = this.update(cx, |shell, cx| shell.secret_answered(&name, result, cx));
                })
                .detach();
            }
            Err(problem) => dialog.form.refused(problem.to_owned()),
        }
        cx.notify();
    }

    /// The service's answer to a save: stored closes the dialog, and a refusal keeps it open and says why.
    fn secret_answered(
        &mut self,
        name: &str,
        result: Result<SecretEntry, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(_) => self.vault_dialog = None,
            Err(refusal) => {
                if let Some(dialog) = self.vault_dialog.as_mut() {
                    dialog.form.refused(refusal_sentence(&refusal, name));
                }
            }
        }
        cx.notify();
    }

    /// Cancel, Escape or the scrim: the dialog goes, and the value it held with it.
    pub(super) fn close_secret_dialog(&mut self, cx: &mut Context<Self>) {
        self.vault_dialog = None;
        cx.notify();
    }
}

/// The dialog on screen. `connected` says whether the service can take a save.
pub(super) fn secret_dialog(
    dialog: &VaultDialog,
    shell: &WeakEntity<Shell>,
    connected: bool,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let form = &dialog.form;
    let (title, action) = match form.mode {
        FormMode::Add => ("Add secret", "Save secret"),
        FormMode::Rotate => ("Rotate secret", "Rotate"),
    };
    let name = match &dialog.name_field {
        Some(field) => field.read(cx).text().to_owned(),
        None => form.name.clone(),
    };
    let saving = form.status == FormStatus::Saving;
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(
                    "Agents get it as an environment variable when they run. It is stored on This PC and never goes into the chat.",
                ),
        )
        .child(name_field(dialog, &theme))
        .child(scope_field(dialog, shell, &theme))
        .child(value_field(form, shell, saving, &theme));
    let (save, dismiss) = (shell.clone(), shell.clone());
    let mut view = Dialog::new("settings-vault-dialog", title)
        .open(true)
        .width(DialogWidth::Medium)
        .child(body)
        .action(action)
        .action_disabled(!form.can_save(&name, connected))
        .busy(saving)
        .on_action(move |_, cx| {
            let _ = save.update(cx, |shell, cx| shell.save_secret(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.close_secret_dialog(cx));
        });
    if let FormStatus::Refused(text) = &form.status {
        view = view.error(text.clone());
    }
    view.into_any_element()
}

/// The name an add types, or the name a rotation replaces.
fn name_field(dialog: &VaultDialog, theme: &Theme) -> AnyElement {
    match &dialog.name_field {
        Some(field) => field.clone().into_any_element(),
        None => value_box(dialog.form.name.clone(), theme),
    }
}

/// The scope an add stores to, one segment per scope; a rotation shows the scope it keeps.
fn scope_field(dialog: &VaultDialog, shell: &WeakEntity<Shell>, theme: &Theme) -> AnyElement {
    match dialog.form.mode {
        FormMode::Add => {
            let segments: Vec<Segment> = dialog
                .scopes
                .iter()
                .map(|(_, title)| Segment::text(title.clone()))
                .collect();
            let selected = dialog
                .scopes
                .iter()
                .position(|(scope, _)| *scope == dialog.form.scope)
                .unwrap_or(0);
            let shell = shell.clone();
            SegmentedControl::new("settings-vault-scope", "Scope", segments)
                .selected(selected)
                .on_change(move |index, _, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.choose_secret_scope(index, cx));
                })
                .into_any_element()
        }
        FormMode::Rotate => value_box(
            dialog
                .scopes
                .first()
                .map(|(_, title)| title.clone())
                .unwrap_or_default(),
            theme,
        ),
    }
}

/// The value as a state only: it is pasted from the clipboard and never drawn.
fn value_field(
    form: &SecretForm,
    shell: &WeakEntity<Shell>,
    saving: bool,
    theme: &Theme,
) -> AnyElement {
    let state = if form.has_value() {
        "A value is ready. It is sent on save and never shown."
    } else {
        "No value yet. Copy the value, then paste it here."
    };
    let shell = shell.clone();
    let paste = Button::new("settings-vault-paste", "Paste value")
        .variant(ButtonVariant::Secondary)
        .disabled(saving)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.paste_secret_value(cx));
        });
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(state),
        )
        .child(paste)
        .into_any_element()
}
