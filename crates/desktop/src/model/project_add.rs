//! The "Add a project" form as plain state (decision 0028): the path typed, whether the call is out,
//! where a refusal shows, and the warning for a folder that is not a Git repository (WS-31). The service
//! picks the project's name.

use massorca_protocol::methods::projects::{ProjectAddParams, ProjectIcon, ProjectKind};

use crate::model::workspace_flow::{DialogCopy, DialogKind};
use crate::service::Refusal;

/// The project menu's last row, which opens the dialog (A38).
pub const ADD_PROJECT_LABEL: &str = "Add a project\u{2026}";

/// The project menu's caption when the host has no project yet (A38).
pub const NO_PROJECTS_LABEL: &str = "No projects on This PC yet";

/// The warning before a folder that is not a Git repository is added (WS-31): what it loses, and the
/// action that adds it without Git. "Initialize Git" is left out until the service can run git init.
pub fn plain_folder_copy() -> DialogCopy {
    DialogCopy {
        kind: DialogKind::Confirm,
        title: "This folder is not a Git repository".to_owned(),
        body: "Add it as a plain folder and work in it as it is.".to_owned(),
        loss: vec![
            "Worktrees, branches and Source control need Git.".to_owned(),
            "Child sessions run in this folder, not in their own worktrees.".to_owned(),
        ],
        action: "Continue without Git",
    }
}

/// The form's state: the typed path, where a refusal is shown, and the folder the warning is about.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AddProjectForm {
    pub path: String,
    pub busy: bool,
    /// A refusal about the path, shown under the path field.
    pub path_error: Option<String>,
    /// Any other refusal, shown in the dialog.
    pub error: Option<String>,
    /// A folder that is not a Git repository, held until the person continues without Git or cancels.
    pub plain_folder: Option<String>,
    /// The icon the project is added with; `None` leaves it to the service, which detects one from the folder's files.
    pub icon: Option<ProjectIcon>,
    /// Whether the icon grid is open.
    pub icon_open: bool,
    /// Why the last uploaded icon was refused, shown under the grid until the next choice.
    pub icon_refusal: Option<String>,
}

impl AddProjectForm {
    pub fn can_submit(&self) -> bool {
        !self.busy && !self.path.trim().is_empty()
    }

    /// The `project.add` params for the typed path and the chosen icon. The name and the kind are left to the service.
    pub fn params(&self) -> Option<ProjectAddParams> {
        self.can_submit().then(|| ProjectAddParams {
            icon: self.icon.clone(),
            path: self.path.trim().to_owned(),
            name: None,
            kind: None,
            workspaces_root: None,
        })
    }

    /// Sends the form: busy until the service answers, with no earlier refusal shown.
    pub fn begin(&mut self) {
        self.busy = true;
        self.path_error = None;
        self.error = None;
    }

    /// Frees the form and shows the refusal where it belongs.
    pub fn refused(&mut self, refusal: &Refusal) {
        self.busy = false;
        match refusal {
            Refusal::PathProblem { message } => self.path_error = Some(message.clone()),
            other => self.error = Some(other.message()),
        }
    }

    /// The service found `path` is not a Git repository: nothing is added until the person decides (WS-31).
    pub fn hold_plain_folder(&mut self, path: String) {
        self.busy = false;
        self.plain_folder = Some(path);
    }

    /// Cancel on the warning: nothing is added, and the path stays in the field.
    pub fn cancel_plain_folder(&mut self) {
        self.plain_folder = None;
    }

    /// Chooses the icon the project is added with, or `None` to leave it to the service, and closes the grid.
    pub fn choose_icon(&mut self, icon: Option<ProjectIcon>) {
        self.icon = icon;
        self.icon_open = false;
        self.icon_refusal = None;
    }

    /// Opens or closes the icon grid.
    pub fn set_icon_open(&mut self, open: bool) {
        self.icon_open = open;
    }

    /// An uploaded icon was refused: the grid stays open with the reason under it.
    pub fn refuse_icon(&mut self, reason: String) {
        self.icon_refusal = Some(reason);
    }

    /// Continue without Git: the `project.add` params that register the held folder as a plain folder.
    pub fn continue_without_git(&mut self) -> Option<ProjectAddParams> {
        let path = self.plain_folder.take()?;
        self.begin();
        Some(ProjectAddParams {
            icon: self.icon.clone(),
            path,
            name: None,
            kind: Some(ProjectKind::Folder),
            workspaces_root: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_or_busy_form_cannot_be_sent() {
        let mut form = AddProjectForm::default();
        assert!(!form.can_submit());
        form.path = "   ".into();
        assert!(!form.can_submit());
        assert!(form.params().is_none());
        form.path = "/repo".into();
        assert!(form.can_submit());
        form.begin();
        assert!(!form.can_submit(), "a call is out");
    }

    #[test]
    fn the_params_send_the_trimmed_path_and_nothing_the_service_picks() {
        let form = AddProjectForm {
            path: "  /repo  ".into(),
            ..AddProjectForm::default()
        };
        let params = form.params().expect("the form can be sent");
        assert_eq!(params.path, "/repo");
        assert_eq!(params.name, None);
        assert_eq!(params.kind, None);
        assert_eq!(params.workspaces_root, None);
    }

    #[test]
    fn a_path_refusal_stays_under_the_field_and_frees_the_form() {
        let mut form = AddProjectForm {
            path: "/repo".into(),
            ..AddProjectForm::default()
        };
        form.begin();
        form.refused(&Refusal::PathProblem {
            message: "Not a Git repository.".into(),
        });
        assert!(!form.busy);
        assert_eq!(form.path_error.as_deref(), Some("Not a Git repository."));
        assert_eq!(form.error, None);
    }

    #[test]
    fn other_refusals_show_in_the_dialog() {
        let mut form = AddProjectForm {
            path: "/repo".into(),
            ..AddProjectForm::default()
        };
        form.begin();
        form.refused(&Refusal::unavailable(
            "The service cannot be reached right now.",
        ));
        assert_eq!(
            form.error.as_deref(),
            Some("The service cannot be reached right now.")
        );
        assert_eq!(form.path_error, None);
    }

    #[test]
    fn a_plain_folder_is_added_as_a_folder_only_on_continue() {
        let mut form = AddProjectForm {
            path: "/notes".into(),
            ..AddProjectForm::default()
        };
        form.begin();
        form.hold_plain_folder("/notes".into());
        assert!(!form.busy, "the add waits for the warning");
        assert_eq!(form.plain_folder.as_deref(), Some("/notes"));
        let params = form
            .continue_without_git()
            .expect("the held folder is added");
        assert_eq!(params.path, "/notes");
        assert_eq!(params.kind, Some(ProjectKind::Folder));
        assert!(form.busy, "the call is out");
        assert_eq!(form.plain_folder, None);
    }

    #[test]
    fn cancelling_the_warning_adds_nothing_and_keeps_the_path() {
        let mut form = AddProjectForm {
            path: "/notes".into(),
            ..AddProjectForm::default()
        };
        form.begin();
        form.hold_plain_folder("/notes".into());
        form.cancel_plain_folder();
        assert_eq!(form.plain_folder, None);
        assert_eq!(form.path, "/notes");
        assert!(form.params().is_some(), "the form can be sent again");
    }

    #[test]
    fn the_warning_names_what_a_folder_loses_and_what_continue_does() {
        let copy = plain_folder_copy();
        assert_eq!(copy.title, "This folder is not a Git repository");
        assert_eq!(copy.action, "Continue without Git");
        assert_eq!(copy.loss.len(), 2, "two losses are named");
    }

    fn lucide(name: &str) -> ProjectIcon {
        ProjectIcon::Lucide {
            name: name.to_owned(),
            color: None,
        }
    }

    #[test]
    fn the_params_send_the_icon_the_owner_chose_and_none_until_then() {
        let mut form = AddProjectForm {
            path: "/repo".into(),
            ..AddProjectForm::default()
        };
        let params = form.params().expect("the form can be sent");
        assert_eq!(params.icon, None, "the service detects an icon");
        form.choose_icon(Some(lucide("rocket")));
        let params = form.params().expect("the form can be sent");
        assert_eq!(params.icon, Some(lucide("rocket")));
    }

    #[test]
    fn choosing_an_icon_closes_the_grid_and_reset_leaves_the_icon_to_the_service() {
        let mut form = AddProjectForm {
            path: "/repo".into(),
            icon_open: true,
            icon_refusal: Some("The icon was not set.".into()),
            ..AddProjectForm::default()
        };
        form.choose_icon(Some(lucide("rocket")));
        assert!(!form.icon_open);
        assert_eq!(form.icon_refusal, None);
        form.choose_icon(None);
        assert_eq!(form.icon, None);
    }

    #[test]
    fn a_refused_upload_keeps_the_grid_open_with_its_reason() {
        let mut form = AddProjectForm {
            icon_open: true,
            ..AddProjectForm::default()
        };
        form.refuse_icon("The icon was not set: the file is not a PNG.".into());
        assert!(form.icon_open);
        assert_eq!(form.icon, None);
        assert_eq!(
            form.icon_refusal.as_deref(),
            Some("The icon was not set: the file is not a PNG.")
        );
    }

    #[test]
    fn a_plain_folder_is_added_with_the_icon_chosen_before_the_warning() {
        let mut form = AddProjectForm {
            path: "/notes".into(),
            ..AddProjectForm::default()
        };
        form.choose_icon(Some(lucide("notebook")));
        form.begin();
        form.hold_plain_folder("/notes".into());
        let params = form
            .continue_without_git()
            .expect("the held folder is added");
        assert_eq!(params.icon, Some(lucide("notebook")));
        assert_eq!(params.kind, Some(ProjectKind::Folder));
    }
}
