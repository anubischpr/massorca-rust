//! The "Add a project" form as plain state (decision 0028): the path typed, whether the call is out,
//! and where a refusal shows. The service picks the project's name and kind.

use massorca_protocol::methods::projects::ProjectAddParams;

use crate::service::Refusal;

/// The project menu's last row, which opens the dialog (A38).
pub const ADD_PROJECT_LABEL: &str = "Add a project\u{2026}";

/// The project menu's caption when the host has no project yet (A38).
pub const NO_PROJECTS_LABEL: &str = "No projects on This PC yet";

/// The form's state: the typed path, and where a refusal is shown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AddProjectForm {
    pub path: String,
    pub busy: bool,
    /// A refusal about the path, shown under the path field.
    pub path_error: Option<String>,
    /// Any other refusal, shown in the dialog.
    pub error: Option<String>,
}

impl AddProjectForm {
    pub fn can_submit(&self) -> bool {
        !self.busy && !self.path.trim().is_empty()
    }

    /// The `project.add` params for the typed path. The name and the kind are left to the service.
    pub fn params(&self) -> Option<ProjectAddParams> {
        self.can_submit().then(|| ProjectAddParams {
            icon: None,
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
}
