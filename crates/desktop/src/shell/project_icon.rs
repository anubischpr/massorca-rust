//! The choices of a project's icon popover, which set, reset or upload the project's icon through `project.update`
//! (decision 0091). The popover closes once the service accepts the change, and keeps the reason when it refuses it.

use gpui::Context;
use massorca_protocol::methods::projects::ProjectIcon;
use massorca_ui::components::project_icon_popover::ProjectIconChoice;

use super::Shell;
use super::path_field::{PickKind, system_picker};
use crate::model::project_icon::{update_params, upload_icon};

const NOT_CONNECTED: &str = "The icon was not set: the service is not connected.";

impl Shell {
    /// Sets the project's icon to the choice, or resets it; an upload asks for a file first.
    pub(super) fn choose_project_icon(
        &mut self,
        project: String,
        choice: ProjectIconChoice,
        cx: &mut Context<Self>,
    ) {
        match choice {
            ProjectIconChoice::Lucide(name) => {
                let icon = ProjectIcon::Lucide {
                    name: name.to_owned(),
                };
                self.send_project_icon(project, Some(icon), cx);
            }
            ProjectIconChoice::Reset => self.send_project_icon(project, None, cx),
            ProjectIconChoice::Upload => self.upload_project_icon(project, cx),
        }
    }

    /// Sends the change through the service, and answers with what the service says.
    fn send_project_icon(
        &mut self,
        project: String,
        icon: Option<ProjectIcon>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            self.answer_project_icon(&project, Err(NOT_CONNECTED.to_owned()), cx);
            return;
        };
        let params = update_params(&project, icon);
        cx.spawn(async move |this, cx| {
            let answer = service
                .update_project(params)
                .await
                .map(|_| ())
                .map_err(|refusal| refusal.sentence());
            let _ = this.update(cx, |shell, cx| {
                shell.answer_project_icon(&project, answer, cx)
            });
        })
        .detach();
    }

    /// Reads the file the person picks, checks it as a PNG of at most 256 KiB, and sends it.
    fn upload_project_icon(&self, project: String, cx: &mut Context<Self>) {
        let picked = system_picker().pick(PickKind::File, cx);
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let Some(path) = picked.await else {
                return;
            };
            let label = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            let icon = std::fs::read(&path)
                .map_err(|error| format!("The file could not be read: {error}."))
                .and_then(|bytes| {
                    upload_icon(&bytes, label)
                        .map_err(|reason| format!("The icon was not set: {reason}."))
                });
            let answer = match (icon, service) {
                (Err(reason), _) => Err(reason),
                (Ok(_), None) => Err(NOT_CONNECTED.to_owned()),
                (Ok(icon), Some(service)) => service
                    .update_project(update_params(&project, Some(icon)))
                    .await
                    .map(|_| ())
                    .map_err(|refusal| refusal.sentence()),
            };
            let _ = this.update(cx, |shell, cx| {
                shell.answer_project_icon(&project, answer, cx)
            });
        })
        .detach();
    }

    /// Closes the popover once the change went through, and keeps it open with the reason when it did not.
    fn answer_project_icon(
        &mut self,
        project: &str,
        answer: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .project_icon
            .as_ref()
            .is_some_and(|popover| popover.project == project)
        {
            return;
        }
        match answer {
            Ok(()) => self.project_icon = None,
            Err(reason) => {
                if let Some(popover) = self.project_icon.as_mut() {
                    popover.refusal = Some(reason);
                }
            }
        }
        cx.notify();
    }
}
