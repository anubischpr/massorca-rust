//! The dialog that adds or rotates a secret from Settings (X-VT-01, decision 0071). The value is typed into a masked
//! field or pasted there: the field paints bullets, and its Show button reveals only the value being typed. Save sends
//! the value once and locks the field until the service answers. A stored value clears the field and closes the
//! dialog; a refusal keeps the value in the field so the save can be retried.

use std::fmt;

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, EntityId, Subscription, WeakEntity, Window, div, px};
use massorca_protocol::methods::secrets::SecretEntry;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::{Dialog, DialogWidth};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled, Theme};

use super::Shell;
use super::settings::value_box;
use crate::model::secret_form::{
    FormMode, FormStatus, SecretForm, refusal_sentence, scope_caption, scope_choices, scope_segment,
};
use crate::model::secret_requests::RequestRow;
use crate::model::secret_vault::scope_title;
use crate::service::Refusal;

/// The add or rotate dialog on screen: its form, the name field of an add, the masked value field, and the scopes it
/// can store to, each with the title its scope shows.
pub(super) struct VaultDialog {
    pub(super) form: SecretForm,
    /// The name an add types. A rotation shows its name as text and has no field.
    pub(super) name_field: Option<Entity<TextField>>,
    /// The value, typed or pasted. Save reads it; it stays here until the service stores it or the dialog closes.
    pub(super) value_field: Entity<TextField>,
    /// Redraws the dialog as the name changes, so Save follows what is typed.
    _name_changes: Option<Subscription>,
    /// Redraws the dialog as the value changes, so Save and its wording follow it.
    _value_changes: Subscription,
    pub(super) scopes: Vec<(String, String)>,
    /// The project select of an add: every project, with the one the form stores to chosen. Only a project shows it.
    pub(super) project_select: Option<Entity<SelectView>>,
    /// The project scopes in the select's order, so a choice maps to the scope it names.
    pub(super) project_scopes: Vec<String>,
    /// Whether the scope is fixed: a rotation's is, and so is a request's, so no scope control can change it.
    pub(super) scope_locked: bool,
    /// The request this add answers, by id, when it was opened for one: the transcript card shows it as entering.
    pub(super) request: Option<String>,
    /// Keeps the select's choices reaching the dialog.
    _project_events: Option<Subscription>,
}

impl fmt::Debug for VaultDialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultDialog")
            .field("form", &self.form)
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

impl VaultDialog {
    /// Locks or unlocks the fields while a save is in flight: a value being sent is not edited under the request.
    fn lock_fields(&self, locked: bool, cx: &mut App) {
        if let Some(field) = &self.name_field {
            field.update(cx, |field, cx| field.set_disabled(locked, cx));
        }
        self.value_field
            .update(cx, |field, cx| field.set_disabled(locked, cx));
    }
}

/// The masked field a value is typed or pasted into, with the subscription that redraws the dialog as it changes.
fn value_entry(cx: &mut Context<Shell>) -> (Entity<TextField>, Subscription) {
    let field = cx.new(|cx| {
        TextField::new(cx)
            .label("Value")
            .masked()
            .revealable()
            .mono()
    });
    let changes = cx.observe(&field, |_, _, cx| cx.notify());
    (field, changes)
}

/// The name a project's scope title carries: "Project oppm" names "oppm".
fn project_name(title: &str) -> &str {
    title.strip_prefix("Project ").unwrap_or(title)
}

impl Shell {
    /// Opens Add with the first project preselected, as the design has it.
    pub(super) fn open_add_secret(&mut self, cx: &mut Context<Self>) {
        self.open_add(None, cx);
    }

    /// Opens Add for a request: its name and scope are set and locked. Nothing opens for a scope Add cannot store to.
    pub(super) fn open_add_for_request(&mut self, request: &RequestRow, cx: &mut Context<Self>) {
        self.open_add(Some(request), cx);
    }

    fn open_add(&mut self, request: Option<&RequestRow>, cx: &mut Context<Self>) {
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
        let first = match request {
            Some(request) => request.scope.clone(),
            None => match scopes.first() {
                Some((scope, _)) => scope.clone(),
                None => return,
            },
        };
        if !scopes.iter().any(|(scope, _)| *scope == first) {
            return;
        }
        let name_field = cx.new(|cx| {
            let field = TextField::new(cx)
                .label("Variable name")
                .helper("Letters, digits and underscores. Cannot start with a digit.")
                .placeholder("OPENAI_API_KEY")
                .mono();
            match request {
                Some(request) => field.with_text(&request.name),
                None => field,
            }
        });
        if request.is_some() {
            name_field.update(cx, |field, cx| field.set_disabled(true, cx));
        }
        let name_changes = cx.observe(&name_field, |_, _, cx| cx.notify());
        let (value_field, value_changes) = value_entry(cx);
        let projects: Vec<(String, String)> = scopes
            .iter()
            .filter(|(scope, _)| scope.starts_with("project:"))
            .cloned()
            .collect();
        let project_scopes: Vec<String> = projects.iter().map(|(scope, _)| scope.clone()).collect();
        let (project_select, project_events) = if projects.is_empty() {
            (None, None)
        } else {
            let names = projects
                .iter()
                .map(|(_, title)| project_name(title).to_owned());
            let select = cx.new(|cx| {
                SelectView::new(names, cx)
                    .label("Project")
                    .selected(Some(
                        project_scopes
                            .iter()
                            .position(|scope| *scope == first)
                            .unwrap_or(0),
                    ))
                    .disabled(request.is_some())
                    .layer(OverlayLayer::Dialog)
            });
            let events = cx.subscribe(&select, |shell, _, event: &SelectEvent, cx| match event {
                SelectEvent::Changed(index) => shell.choose_secret_project(*index, cx),
            });
            (Some(select), Some(events))
        };
        self.vault_dialog = Some(VaultDialog {
            form: SecretForm::add(first),
            name_field: Some(name_field),
            value_field,
            _name_changes: Some(name_changes),
            _value_changes: value_changes,
            scopes,
            project_select,
            project_scopes,
            scope_locked: request.is_some(),
            request: request.map(|request| request.id.clone()),
            _project_events: project_events,
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
        let (value_field, value_changes) = value_entry(cx);
        self.vault_dialog = Some(VaultDialog {
            form: SecretForm::rotate(scope.clone(), name),
            name_field: None,
            value_field,
            _name_changes: None,
            _value_changes: value_changes,
            scopes: vec![(scope, title)],
            project_select: None,
            project_scopes: Vec::new(),
            scope_locked: true,
            request: None,
            _project_events: None,
        });
        cx.notify();
    }

    /// Chooses the project an add stores to, by the row the project select chose.
    pub(super) fn choose_secret_project(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(dialog) = self
            .vault_dialog
            .as_mut()
            .filter(|dialog| !dialog.scope_locked)
            && let Some(scope) = dialog.project_scopes.get(index).cloned()
        {
            dialog.form.scope = scope;
        }
        cx.notify();
    }

    /// Chooses a segment of the scope control. Project keeps a project already chosen and takes the first one
    /// otherwise; Global stores to Global. The project select then shows the scope the form holds.
    pub(super) fn choose_secret_segment(&mut self, segment: usize, cx: &mut Context<Self>) {
        let Some(dialog) = self
            .vault_dialog
            .as_mut()
            .filter(|dialog| !dialog.scope_locked)
        else {
            return;
        };
        match segment {
            0 if scope_segment(&dialog.form.scope) != 0 => {
                if let Some(first) = dialog.project_scopes.first().cloned() {
                    dialog.form.scope = first;
                }
            }
            2 => dialog.form.scope = "global".to_owned(),
            _ => {}
        }
        let index = dialog
            .project_scopes
            .iter()
            .position(|scope| *scope == dialog.form.scope);
        if let Some(select) = dialog.project_select.clone() {
            select.update(cx, |select, cx| select.set_selected(index, cx));
        }
        cx.notify();
    }

    /// Paste: the clipboard text replaces the value in the field. Empty text is refused, and says why.
    pub(super) fn paste_secret_value(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        if let Some(dialog) = self.vault_dialog.as_mut()
            && dialog.form.accepts_paste(&text)
        {
            dialog.value_field.update(cx, |field, cx| {
                field.clear_text(window, cx);
                field.insert_pasted(&text, window, cx);
            });
        }
        cx.notify();
    }

    /// Save: checks the name, then sends the value the field holds once. The field is locked while the service
    /// answers and keeps the value, so a refused save can be retried as it is.
    pub(super) fn save_secret(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let Some(dialog) = self.vault_dialog.as_mut() else {
            return;
        };
        if dialog.form.status == FormStatus::Saving {
            return;
        }
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
        match dialog
            .form
            .request(&name, dialog.value_field.read(cx).text())
        {
            Ok(params) => {
                dialog.lock_fields(true, cx);
                let dialog_id = dialog.value_field.entity_id();
                cx.spawn_in(window, async move |this, cx| {
                    let result = service.secret_set(params).await;
                    let _ = this.update_in(cx, |shell, window, cx| {
                        shell.secret_answered(dialog_id, &name, result, window, cx)
                    });
                })
                .detach();
            }
            Err(problem) => dialog.form.refused(problem.to_owned()),
        }
        cx.notify();
    }

    /// The service's answer to a save: stored clears the value and closes the dialog; a refusal keeps the value,
    /// unlocks the fields and says why. An answer for a dialog that is no longer open is dropped, so it never
    /// acts on a newer dialog.
    fn secret_answered(
        &mut self,
        dialog_id: EntityId,
        name: &str,
        result: Result<SecretEntry, Refusal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let answers_open_dialog = self
            .vault_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.value_field.entity_id() == dialog_id);
        if !answers_open_dialog {
            return;
        }
        match result {
            Ok(_) => self.close_secret_dialog(window, cx),
            Err(refusal) => {
                if let Some(dialog) = self.vault_dialog.as_mut() {
                    dialog.form.refused(refusal_sentence(&refusal, name));
                    dialog.lock_fields(false, cx);
                }
                cx.notify();
            }
        }
    }

    /// Cancel, Escape or the scrim: the value is cleared from the field, then the dialog goes.
    pub(super) fn close_secret_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dialog) = self.vault_dialog.take() {
            dialog
                .value_field
                .update(cx, |field, cx| field.clear_text(window, cx));
        }
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
    let has_value = !dialog.value_field.read(cx).text().is_empty();
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
        .children(add_link(dialog))
        .child(value_field(dialog, shell, has_value, saving, &theme));
    let (save, dismiss) = (shell.clone(), shell.clone());
    let mut view = Dialog::new("settings-vault-dialog", title)
        .open(true)
        .closable()
        .width(DialogWidth::Medium)
        .child(body)
        .action(action)
        .action_disabled(!form.can_save(&name, has_value, connected))
        .busy(saving)
        .on_action(move |window, cx| {
            let _ = save.update(cx, |shell, cx| shell.save_secret(window, cx));
        })
        .on_dismiss(move |window, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.close_secret_dialog(window, cx));
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
            let has_projects = !dialog.project_scopes.is_empty();
            let name = dialog
                .scopes
                .iter()
                .find(|(scope, _)| *scope == dialog.form.scope)
                .map(|(_, title)| project_name(title).to_owned())
                .unwrap_or_default();
            let caption = scope_caption(&dialog.form.scope, &name);
            let segments = vec![
                Segment::text("Project").disabled(!has_projects),
                Segment::text("Team").disabled(true),
                Segment::text("Global"),
            ];
            let shell = shell.clone();
            let control = SegmentedControl::new("settings-vault-scope", "Scope", segments)
                .selected(scope_segment(&dialog.form.scope))
                .disabled(dialog.scope_locked)
                .on_change(move |index, _, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.choose_secret_segment(index, cx));
                });
            let mut column = div().flex().flex_col().gap(px(SPACE_1)).child(control);
            if scope_segment(&dialog.form.scope) == 0
                && let Some(select) = &dialog.project_select
            {
                column = column.child(select.clone());
            }
            column
                .child(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(theme.palette.text.secondary)
                        .child(caption),
                )
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

/// "Link an existing secret instead", under the scope of an add. Linking is not built yet, so the control is off.
fn add_link(dialog: &VaultDialog) -> Option<AnyElement> {
    (dialog.form.mode == FormMode::Add).then(|| {
        Button::new("settings-vault-link", "Link an existing secret instead")
            .variant(ButtonVariant::Ghost)
            .disabled(true)
            .into_any_element()
    })
}

/// The value: the masked field the user types into, with Paste beside it. The field paints bullets, and its Show
/// button reveals the value only while the field has focus.
fn value_field(
    dialog: &VaultDialog,
    shell: &WeakEntity<Shell>,
    has_value: bool,
    saving: bool,
    theme: &Theme,
) -> AnyElement {
    let state = if has_value {
        "A value is ready. It is sent on save and cleared once the vault stores it."
    } else {
        "Type the value, or copy it and choose Paste value."
    };
    let shell = shell.clone();
    let paste = Button::new("settings-vault-paste", "Paste value")
        .variant(ButtonVariant::Secondary)
        .disabled(saving)
        .on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.paste_secret_value(window, cx));
        });
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(dialog.value_field.clone())
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(state),
        )
        .child(paste)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use gpui::{ClipboardItem, TestAppContext, VisualTestContext};

    use crate::model::secret_requests::RequestRow;
    use crate::service::fixtures::{project, workspace};
    use crate::service::{Command, SecretCommand, StoreUpdate};
    use crate::shell::settings_tests::{Rig, settings_rig};

    fn scope_of_dialog(rig: &Rig, cx: &mut VisualTestContext) -> String {
        rig.shell.read_with(cx, |shell, _| {
            shell
                .vault_dialog
                .as_ref()
                .map(|dialog| dialog.form.scope.clone())
                .expect("the dialog is open")
        })
    }

    #[gpui::test]
    fn a_dialog_opened_from_a_request_keeps_the_scope_the_request_names(cx: &mut TestAppContext) {
        let (mut rig, cx) = settings_rig(cx);
        rig._updates
            .send(StoreUpdate::Resync {
                projects: vec![project("p1", "alpha"), project("p2", "beta")],
                workspaces: vec![workspace("w1", "p1", "fix-login", 1)],
            })
            .expect("the store listens");
        cx.run_until_parked();
        let request = RequestRow {
            id: "r1".to_owned(),
            name: "OPENAI_API_KEY".to_owned(),
            scope: "project:p1".to_owned(),
            scope_title: "Project alpha".to_owned(),
            asker: "a command run on this computer".to_owned(),
            reason: String::new(),
            can_store: true,
            links: Vec::new(),
        };
        rig.shell
            .update(cx, |shell, cx| shell.open_add_for_request(&request, cx));
        rig.shell
            .update(cx, |shell, cx| shell.choose_secret_project(1, cx));
        rig.shell
            .update(cx, |shell, cx| shell.choose_secret_segment(2, cx));
        assert_eq!(
            scope_of_dialog(&rig, cx),
            "project:p1",
            "the scope controls cannot move the scope a request names"
        );
        cx.update(|_, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string("request-value-1".to_owned()))
        });
        rig.shell
            .update_in(cx, |shell, window, cx| shell.paste_secret_value(window, cx));
        rig.shell
            .update_in(cx, |shell, window, cx| shell.save_secret(window, cx));
        cx.run_until_parked();
        let Some(Command::Secret(SecretCommand::Set {
            params,
            reply: _reply,
        })) = rig.script.next()
        else {
            panic!("a secret.set was expected");
        };
        assert_eq!(params.scope, "project:p1", "the request's scope is sent");
        assert_eq!(params.name, "OPENAI_API_KEY");
    }
}
