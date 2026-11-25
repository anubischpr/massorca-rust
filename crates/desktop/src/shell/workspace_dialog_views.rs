//! The add-project and remove-workspace dialogs, drawn from their flows (decision 0028).

use super::Shell;
use super::path_field::{PickKind, path_field};
use super::workspace_dialogs::AddProjectDialog;
use crate::model::workspace_flow::{DialogCopy, DialogKind, KEEP_BRANCH_LATER, RemoveFlow};
use gpui::{AnyElement, App, Focusable, WeakEntity, Window, div, prelude::*, px};
use massorca_ui::ActiveTheme;
use massorca_ui::components::checkbox::Checkbox;
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::tokens::SPACE_3;

/// "Add a project…": one path field. A refusal about the path shows under it, any other refusal in
/// the dialog (A38).
pub(super) fn add_project_dialog(
    dialog: &AddProjectDialog,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let form = &dialog.form;
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let mut view = Dialog::new("add-project", "Add a project")
        .open(true)
        .variant(DialogVariant::OneField)
        .child(path_field(
            "add-project-browse",
            "add-project-field",
            shell.clone(),
            &dialog.path,
            PickKind::Folder,
            true,
            true,
        ))
        .field(dialog.path.read(cx).focus_handle(cx))
        .action("Add project")
        .action_disabled(!form.can_submit())
        .busy(form.busy)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.submit_add_project(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_add_project(cx));
        });
    if let Some(error) = &form.error {
        view = view.error(error.clone());
    }
    view
}

/// The remove dialog for the flow's current stage: confirm, one waiver, or a plain refusal.
/// A destructive stage also shows the keep-branch choice, which stays off for now.
pub(super) fn remove_dialog(flow: &RemoveFlow, shell: &WeakEntity<Shell>, cx: &App) -> Dialog {
    let copy = flow.copy();
    let keep_branch = (copy.kind == DialogKind::Destructive).then(|| {
        // Shown off: the remove call has no keep flag yet (D-WS-16).
        Checkbox::new("remove-keep-branch", false)
            .label("Keep the branch")
            .helper(KEEP_BRANCH_LATER)
            .disabled(true)
            .into_any_element()
    });
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    flow_dialog(
        "remove-workspace",
        copy,
        flow.busy(),
        flow.error(),
        cx,
        (
            move |_: &mut Window, cx: &mut App| {
                let _ = on_action.update(cx, |shell, cx| shell.confirm_remove(cx));
            },
            move |_: &mut Window, cx: &mut App| {
                let _ = on_dismiss.update(cx, |shell, cx| shell.close_remove(cx));
            },
        ),
        keep_branch,
    )
}

/// A flow's dialog: its kind picks the style, its copy names what would be lost and gives the action's
/// words, and its error, when it has one, shows under the body. `extra` sits under the copy.
pub(super) fn flow_dialog(
    id: &'static str,
    copy: DialogCopy,
    busy: bool,
    error: Option<&str>,
    cx: &App,
    actions: (
        impl Fn(&mut Window, &mut App) + 'static,
        impl Fn(&mut Window, &mut App) + 'static,
    ),
    extra: Option<AnyElement>,
) -> Dialog {
    let theme = *cx.theme();
    let variant = match copy.kind {
        DialogKind::Destructive => DialogVariant::Destructive,
        DialogKind::Confirm => DialogVariant::Confirm,
        DialogKind::Error => DialogVariant::Error,
    };
    let (on_action, on_dismiss) = actions;
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(
            div()
                .text_color(theme.palette.text.secondary)
                .child(copy.body),
        )
        .children(extra);
    let mut dialog = Dialog::new(id, copy.title)
        .open(true)
        .variant(variant)
        .child(body)
        .loss(copy.loss)
        .action(copy.action)
        .busy(busy)
        .on_action(on_action)
        .on_dismiss(on_dismiss);
    if let Some(error) = error {
        dialog = dialog.error(error.to_owned());
    }
    dialog
}
