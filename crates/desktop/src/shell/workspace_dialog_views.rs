//! The add-project and remove-workspace dialogs, drawn from their flows (decision 0028).

use super::Shell;
use super::path_field::{PickKind, path_field};
use super::project_mark::project_mark;
use super::workspace_dialogs::AddProjectDialog;
use crate::model::project_add::{AddProjectForm, plain_folder_copy};
use crate::model::project_icon::ProjectGlyph;
use crate::model::session_delete::{SessionDeleteFlow, SessionDeleteStage};
use crate::model::workspace_flow::{DialogCopy, DialogKind, KEEP_BRANCH_LATER, RemoveFlow};
use gpui::{AnyElement, App, Focusable, SharedString, WeakEntity, Window, div, prelude::*, px};
use massorca_protocol::methods::projects::ProjectIcon;
use massorca_ui::ActiveTheme;
use massorca_ui::components::button::Button;
use massorca_ui::components::checkbox::Checkbox;
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::components::project_icon_popover::ProjectIconControl;
use massorca_ui::tokens::SPACE_3;

/// "Add a project…": one path field. A refusal about the path shows under it, any other refusal in
/// the dialog (A38). A folder that is not a Git repository shows the warning instead (WS-31).
pub(super) fn add_project_dialog(
    dialog: &AddProjectDialog,
    shell: &WeakEntity<Shell>,
    init_git_offered: bool,
    cx: &App,
) -> Dialog {
    if dialog.form.plain_folder.is_some() {
        return plain_folder_dialog(dialog, shell, init_git_offered, cx);
    }
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
        .child(add_project_icon_row(form, shell))
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

/// The icon row of "Add a project…": the icon button and its grid, which choose the icon the project is added with.
fn add_project_icon_row(form: &AddProjectForm, shell: &WeakEntity<Shell>) -> impl IntoElement {
    let (on_open, on_choose) = (shell.clone(), shell.clone());
    let current = match &form.icon {
        Some(ProjectIcon::Lucide { name, .. }) => Some(name.clone()),
        _ => None,
    };
    ProjectIconControl::new(
        "add-project-icon",
        project_mark(&ProjectGlyph::of(form.icon.as_ref())),
        form.icon_open,
        move |open, _, cx| {
            let _ = on_open.update(cx, |shell, cx| shell.set_add_project_icon_open(open, cx));
        },
        move |choice, _, cx| {
            let _ = on_choose.update(cx, |shell, cx| shell.choose_add_project_icon(choice, cx));
        },
    )
    .current(current.as_deref())
    .refusal(form.icon_refusal.clone().map(SharedString::from))
    .reset_label("Detect from files")
}

/// The warning for a folder that is not a Git repository: what it loses, then Continue without Git or Cancel back
/// to the path (WS-31). Initialize Git sits under the copy when the service can run git init (`project-init-git/1`).
fn plain_folder_dialog(
    dialog: &AddProjectDialog,
    shell: &WeakEntity<Shell>,
    init_git_offered: bool,
    cx: &App,
) -> Dialog {
    let form = &dialog.form;
    let (on_continue, on_cancel) = (shell.clone(), shell.clone());
    let init_git = init_git_offered.then(|| {
        let on_init = shell.clone();
        Button::new("add-project-init-git", "Initialize Git")
            .disabled(form.busy)
            .on_click(move |_, cx| {
                let _ = on_init.update(cx, |shell, cx| shell.initialize_plain_folder_git(cx));
            })
            .into_any_element()
    });
    flow_dialog(
        "add-project-plain-folder",
        plain_folder_copy(),
        form.busy,
        form.error.as_deref().or(form.path_error.as_deref()),
        cx,
        (
            move |_: &mut Window, cx: &mut App| {
                let _ = on_continue.update(cx, |shell, cx| {
                    shell.continue_add_project_without_git(cx);
                });
            },
            move |_: &mut Window, cx: &mut App| {
                let _ = on_cancel.update(cx, |shell, cx| shell.cancel_plain_folder(cx));
            },
        ),
        init_git,
    )
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

/// The delete-session confirm for the flow's current stage. The worktree option sits under the copy while the
/// confirm is open, and only for a worktree (a plain folder has none).
pub(super) fn session_delete_dialog(
    flow: &SessionDeleteFlow,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let copy = flow.copy();
    let confirming = flow.stage() == &SessionDeleteStage::Confirm;
    let worktree = (flow.offers_workspace() && confirming).then(|| {
        let toggle = shell.clone();
        Checkbox::new("delete-session-worktree", flow.remove_workspace())
            .label("Also remove its worktree and branch")
            .disabled(flow.busy())
            .on_change(move |checked, _: &mut Window, cx: &mut App| {
                let _ = toggle.update(cx, |shell, cx| {
                    shell.set_session_delete_remove_workspace(checked, cx);
                });
            })
            .into_any_element()
    });
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    flow_dialog(
        "delete-session",
        copy,
        flow.busy(),
        flow.error(),
        cx,
        (
            move |_: &mut Window, cx: &mut App| {
                let _ = on_action.update(cx, |shell, cx| shell.confirm_session_delete(cx));
            },
            move |_: &mut Window, cx: &mut App| {
                let _ = on_dismiss.update(cx, |shell, cx| shell.close_session_delete(cx));
            },
        ),
        worktree,
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
