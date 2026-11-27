//! A project's base branch (WS-04): the branch new workspaces start from. The dialog's text is the only
//! input; blank text clears the base, so the detected default applies again.

use massorca_protocol::methods::projects::ProjectSetBaseParams;

use super::workspace_flow::{DialogCopy, DialogKind};
use crate::service::Refusal;

/// One project's base-branch edit: whether its call is out, and what the last one refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectBaseFlow {
    pub project_id: String,
    pub project_name: String,
    busy: bool,
    error: Option<String>,
}

impl ProjectBaseFlow {
    pub fn new(project_id: impl Into<String>, project_name: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            project_name: project_name.into(),
            busy: false,
            error: None,
        }
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The call for the text the field holds. Surrounding spaces are dropped; blank text clears the base.
    pub fn params(&self, text: &str) -> ProjectSetBaseParams {
        let base = text.trim();
        ProjectSetBaseParams {
            project: self.project_id.clone(),
            base: (!base.is_empty()).then(|| base.to_owned()),
        }
    }

    /// The primary action: goes busy and returns true, unless a call is out already.
    pub fn start(&mut self) -> bool {
        if self.busy {
            return false;
        }
        self.busy = true;
        self.error = None;
        true
    }

    /// The service answered. Returns true when the base was set and the dialog is over.
    pub fn answered(&mut self, result: Result<(), Refusal>) -> bool {
        self.busy = false;
        match result {
            Ok(()) => true,
            Err(refusal) => {
                self.error = Some(refusal.message());
                false
            }
        }
    }

    pub fn copy(&self) -> DialogCopy {
        DialogCopy {
            kind: DialogKind::Confirm,
            title: format!("Base branch of {}", self.project_name),
            body: "New workspaces start from this branch. Leave the field empty to use the detected default."
                .to_owned(),
            loss: Vec::new(),
            action: "Save",
        }
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::codes;

    use super::*;

    #[test]
    fn the_typed_base_is_trimmed_and_a_blank_one_clears_it() {
        let flow = ProjectBaseFlow::new("p1", "alpha");
        assert_eq!(flow.params(" develop ").base, Some("develop".to_owned()));
        assert_eq!(flow.params("   ").base, None);
    }

    #[test]
    fn a_second_save_while_the_call_is_out_sends_nothing() {
        let mut flow = ProjectBaseFlow::new("p1", "alpha");
        assert!(flow.start());
        assert!(!flow.start());
    }

    #[test]
    fn a_refused_base_keeps_the_dialog_open_with_the_reason() {
        let mut flow = ProjectBaseFlow::new("p1", "alpha");
        assert!(flow.start());
        let refused = Refusal::Failed {
            code: codes::GIT_REVISION_NOT_FOUND.into(),
            message: "There is no branch named nope.".into(),
        };
        assert!(!flow.answered(Err(refused)));
        assert!(!flow.busy());
        assert_eq!(flow.error(), Some("There is no branch named nope."));
    }
}
