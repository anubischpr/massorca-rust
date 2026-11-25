//! The choices of a project's icon popover, which set, reset or upload the project's icon through `project.update`
//! (decision 0091). The upload is a PNG of up to 256 KiB, which the protocol checks on the service.

use massorca_protocol::methods::projects::{ProjectIcon, ProjectIconSource, ProjectUpdateParams};
use massorca_ui::components::project_icon_popover::ProjectIconChoice;

use super::Shell;
use super::path_field::{PickKind, system_picker};
use crate::model::project_icon::update_params;
use gpui::Context;

impl Shell {
    /// Sets the project's icon to the choice, or resets it to the folder glyph; an upload asks for a PNG first.
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
                self.send_project_icon(update_params(&project, Some(icon)), cx);
            }
            ProjectIconChoice::Reset => self.send_project_icon(update_params(&project, None), cx),
            ProjectIconChoice::Upload => self.upload_project_icon(project, cx),
        }
    }

    fn send_project_icon(&self, params: ProjectUpdateParams, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |_, _| {
            if let Err(refusal) = service.update_project(params).await {
                tracing::warn!(?refusal, "the project icon was not set");
            }
        })
        .detach();
    }

    fn upload_project_icon(&self, project: String, cx: &mut Context<Self>) {
        let picked = system_picker().pick(PickKind::File, cx);
        let service = self.service.clone();
        cx.spawn(async move |_, _| {
            let Some(path) = picked.await else {
                return;
            };
            let Some(service) = service else {
                return;
            };
            let label = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            let icon = std::fs::read(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    ProjectIcon::image_from_bytes(ProjectIconSource::Upload, &bytes, label)
                });
            match icon {
                Ok(icon) => {
                    if let Err(refusal) = service
                        .update_project(update_params(&project, Some(icon)))
                        .await
                    {
                        tracing::warn!(?refusal, "the uploaded project icon was not set");
                    }
                }
                Err(reason) => tracing::warn!(%reason, "the project icon was not read"),
            }
        })
        .detach();
    }
}
