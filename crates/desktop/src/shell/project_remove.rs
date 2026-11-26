//! The Remove project action (WS-17): its confirm dialog, the `project.remove` call behind it, and the
//! waiver the service asks for when the project still has workspace records.

use gpui::{App, Context, WeakEntity, Window};
use massorca_ui::components::dialog::Dialog;

use super::Shell;
use super::workspace_dialog_views::flow_dialog;
use crate::model::project_remove::ProjectRemoveFlow;
use crate::service::Refusal;

impl Shell {
    /// Opens the confirm for `project_id`. Nothing opens for a project the store does not hold.
    pub(super) fn open_project_remove(&mut self, project_id: &str, cx: &mut Context<Self>) {
        let Some(project) = self.store.read(cx).state().project(project_id) else {
            return;
        };
        self.project_remove = Some(ProjectRemoveFlow::new(
            project.id.clone(),
            project.name.clone(),
        ));
        cx.notify();
    }

    /// The dialog's primary action: sends the call its stage asks for, once.
    pub(super) fn confirm_project_remove(&mut self, cx: &mut Context<Self>) {
        let (Some(flow), Some(handle)) = (self.project_remove.as_mut(), self.service.clone())
        else {
            return;
        };
        if !flow.confirm() {
            return;
        }
        let (project, params) = (flow.project_id.clone(), flow.params());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.remove_project(params).await.map(|_| ());
            this.update(cx, |shell, cx| {
                shell.finish_project_remove(&project, result, cx)
            })
            .ok();
        })
        .detach();
    }

    /// The call's answer. It counts only while the dialog still shows that project.
    fn finish_project_remove(
        &mut self,
        project: &str,
        result: Result<(), Refusal>,
        cx: &mut Context<Self>,
    ) {
        let removed = self
            .project_remove
            .as_mut()
            .is_some_and(|flow| flow.project_id == project && flow.answered(result));
        if removed {
            self.project_remove = None;
        }
        cx.notify();
    }

    /// Dismisses the dialog. A call that is out keeps it open until its answer comes.
    pub(super) fn close_project_remove(&mut self, cx: &mut Context<Self>) {
        if self
            .project_remove
            .as_ref()
            .is_some_and(|flow| !flow.busy())
        {
            self.project_remove = None;
            cx.notify();
        }
    }
}

/// The confirm dialog for the flow's stage. Its action sends the stage's call; dismissing it removes nothing.
pub(super) fn project_remove_dialog(
    flow: &ProjectRemoveFlow,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    flow_dialog(
        "project-remove",
        flow.copy(),
        flow.busy(),
        flow.error(),
        cx,
        (
            move |_: &mut Window, cx: &mut App| {
                let _ = on_action.update(cx, |shell, cx| shell.confirm_project_remove(cx));
            },
            move |_: &mut Window, cx: &mut App| {
                let _ = on_dismiss.update(cx, |shell, cx| shell.close_project_remove(cx));
            },
        ),
        None,
    )
}
