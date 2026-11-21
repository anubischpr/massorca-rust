//! The create-workspace and remove-workspace dialogs, drawn from their flows (decision 0028).

use super::Shell;
use super::workspace_dialogs::CreateDialog;
use crate::model::workspace_flow::{DialogKind, ProjectChoice, RemoveFlow};
use gpui::{App, Focusable, WeakEntity, div, prelude::*, px};
use massorca_protocol::methods::projects::Project;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::checkbox::Checkbox;
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::tokens::{SPACE_1, SPACE_3, TYPE_BODY_SM, TYPE_CAPTION};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

/// "New workspace": project, name, and the branch options the service supports.
pub(super) fn create_dialog(
    dialog: &CreateDialog,
    projects: &[Project],
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let theme = *cx.theme();
    let form = &dialog.form;
    let struck = matches!(form.project, Some(ProjectChoice::NoProject));
    let project_button = {
        let shell = shell.clone();
        Button::new("create-workspace-project", form.project_label(projects))
            .variant(ButtonVariant::Secondary)
            .leading_icon(if struck {
                IconName::FolderX
            } else {
                IconName::Folder
            })
            .trailing_icon(IconName::ChevronDown)
            .disabled(form.busy)
            .on_click(move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.open_project_menu(window, cx));
            })
    };
    let existing = {
        let shell = shell.clone();
        Checkbox::new("create-workspace-existing", form.existing_branch)
            .label("Use an existing branch")
            .disabled(form.busy)
            .on_change(move |checked, _, cx| {
                let _ = shell.update(cx, |shell, cx| shell.set_existing_branch(checked, cx));
            })
    };
    let mut body = div().flex().flex_col().gap(px(SPACE_3)).child(
        div()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .child(
                div()
                    .text_style(TYPE_CAPTION)
                    .text_color(theme.palette.text.secondary)
                    .child("Project"),
            )
            .child(project_button),
    );
    if let Some(note) = form.no_project_note() {
        body = body.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(note),
        );
    }
    body = body
        .child(dialog.name.clone())
        .child(dialog.branch.clone())
        .child(existing)
        .child(dialog.base.clone());

    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let mut dialog_view = Dialog::new("create-workspace", "New workspace")
        .open(true)
        .child(body)
        .field(dialog.name.read(cx).focus_handle(cx))
        .action("Create workspace")
        .action_disabled(!form.can_submit())
        .busy(form.busy)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.submit_create(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_create(cx));
        });
    if let Some(error) = &form.error {
        dialog_view = dialog_view.error(error.clone());
    }
    dialog_view
}

/// The remove dialog for the flow's current stage: confirm, one waiver, or a plain refusal.
pub(super) fn remove_dialog(flow: &RemoveFlow, shell: &WeakEntity<Shell>, cx: &App) -> Dialog {
    let theme = *cx.theme();
    let copy = flow.copy();
    let variant = match copy.kind {
        DialogKind::Destructive => DialogVariant::Destructive,
        DialogKind::Confirm => DialogVariant::Confirm,
        DialogKind::Error => DialogVariant::Error,
    };
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let mut dialog = Dialog::new("remove-workspace", copy.title)
        .open(true)
        .variant(variant)
        .child(
            div()
                .text_color(theme.palette.text.secondary)
                .child(copy.body),
        )
        .loss(copy.loss)
        .action(copy.action)
        .busy(flow.busy())
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.confirm_remove(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_remove(cx));
        });
    if let Some(error) = flow.error() {
        dialog = dialog.error(error.to_owned());
    }
    dialog
}
