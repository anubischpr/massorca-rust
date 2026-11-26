//! The base-branch dialog of the Project sidebar (WS-04): a text field holds the branch new workspaces
//! start from, and Save sends `project.set_base`. Blank text clears the base.

use gpui::{App, AppContext, Context, Entity, IntoElement, WeakEntity, Window};
use massorca_ui::components::dialog::Dialog;
use massorca_ui::components::text_field::TextField;

use super::Shell;
use super::workspace_dialog_views::flow_dialog;
use crate::model::project_base::ProjectBaseFlow;
use crate::service::Refusal;

/// The open dialog: its flow and the field the branch is typed into.
#[derive(Debug)]
pub(super) struct ProjectBase {
    pub(super) flow: ProjectBaseFlow,
    pub(super) field: Entity<TextField>,
}

impl Shell {
    /// Opens the dialog for `project_id`, with its current base in the field. Nothing opens for an unknown project.
    pub(super) fn open_project_base(&mut self, project_id: &str, cx: &mut Context<Self>) {
        let Some((id, name, current)) =
            self.store
                .read(cx)
                .state()
                .project(project_id)
                .map(|project| {
                    (
                        project.id.clone(),
                        project.name.clone(),
                        project.base_ref.clone().unwrap_or_default(),
                    )
                })
        else {
            return;
        };
        let field = cx.new(|cx| {
            TextField::new(cx)
                .label("Base branch")
                .placeholder("The detected default")
                .mono()
                .with_text(&current)
        });
        self.project_base = Some(ProjectBase {
            flow: ProjectBaseFlow::new(id, name),
            field,
        });
        cx.notify();
    }

    /// The dialog's Save: sends the call for the field's text, once.
    pub(super) fn confirm_project_base(&mut self, cx: &mut Context<Self>) {
        let (Some(base), Some(handle)) = (self.project_base.as_mut(), self.service.clone()) else {
            return;
        };
        if !base.flow.start() {
            return;
        }
        let text = base.field.read(cx).text().to_owned();
        let params = base.flow.params(&text);
        let project = base.flow.project_id.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.set_project_base(params).await.map(|_| ());
            this.update(cx, |shell, cx| {
                shell.finish_project_base(&project, result, cx)
            })
            .ok();
        })
        .detach();
    }

    /// The call's answer. It counts only while the dialog still shows that project.
    fn finish_project_base(
        &mut self,
        project: &str,
        result: Result<(), Refusal>,
        cx: &mut Context<Self>,
    ) {
        let saved = self
            .project_base
            .as_mut()
            .is_some_and(|base| base.flow.project_id == project && base.flow.answered(result));
        if saved {
            self.project_base = None;
        }
        cx.notify();
    }

    /// Dismisses the dialog. A call that is out keeps it open until its answer comes.
    pub(super) fn close_project_base(&mut self, cx: &mut Context<Self>) {
        if self
            .project_base
            .as_ref()
            .is_some_and(|base| !base.flow.busy())
        {
            self.project_base = None;
            cx.notify();
        }
    }
}

/// The base-branch dialog. Its field is the dialog's extra element; Save sends the call.
pub(super) fn project_base_dialog(
    base: &ProjectBase,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    flow_dialog(
        "project-base",
        base.flow.copy(),
        base.flow.busy(),
        base.flow.error(),
        cx,
        (
            move |_: &mut Window, cx: &mut App| {
                let _ = on_action.update(cx, |shell, cx| shell.confirm_project_base(cx));
            },
            move |_: &mut Window, cx: &mut App| {
                let _ = on_dismiss.update(cx, |shell, cx| shell.close_project_base(cx));
            },
        ),
        Some(base.field.clone().into_any_element()),
    )
}
