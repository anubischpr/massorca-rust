//! Removing a project from MassOrca (WS-17): one confirm, then a waiver stage when the service refuses
//! because the project still has workspace records. The service removes the registration and its records;
//! no folder, git repository, worktree or branch is touched.

use massorca_protocol::codes;
use massorca_protocol::methods::projects::ProjectRemoveParams;

use super::workspace_flow::{DialogCopy, DialogKind};
use crate::service::Refusal;

/// Where the removal stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectRemoveStage {
    /// The first call, with no waiver.
    Confirm,
    /// The project still has workspace records. Forgetting them is the waiver the next call sends.
    HasWorkspaces { message: String },
    /// The call failed for a reason no waiver covers; the dialog says why.
    Refused { message: String },
}

/// One project's removal: its confirm, its waiver and the call in flight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRemoveFlow {
    pub project_id: String,
    pub project_name: String,
    stage: ProjectRemoveStage,
    forget_workspaces: bool,
    busy: bool,
    error: Option<String>,
}

impl ProjectRemoveFlow {
    pub fn new(project_id: impl Into<String>, project_name: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            project_name: project_name.into(),
            stage: ProjectRemoveStage::Confirm,
            forget_workspaces: false,
            busy: false,
            error: None,
        }
    }

    pub fn stage(&self) -> &ProjectRemoveStage {
        &self.stage
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The call as it stands: the project, and whether its workspace records are forgotten too.
    pub fn params(&self) -> ProjectRemoveParams {
        ProjectRemoveParams {
            project: self.project_id.clone(),
            forget_workspaces: self.forget_workspaces,
        }
    }

    /// The primary action: gives the waiver the stage asks for and goes busy. False when a call is out or
    /// the stage has nothing to run.
    pub fn confirm(&mut self) -> bool {
        if self.busy {
            return false;
        }
        match self.stage {
            ProjectRemoveStage::Confirm => {}
            ProjectRemoveStage::HasWorkspaces { .. } => self.forget_workspaces = true,
            ProjectRemoveStage::Refused { .. } => return false,
        }
        self.busy = true;
        self.error = None;
        true
    }

    /// The service answered. Returns true when the project was removed and the flow is over.
    pub fn answered(&mut self, result: Result<(), Refusal>) -> bool {
        self.busy = false;
        let Err(refusal) = result else {
            return true;
        };
        if refusal.code() == Some(codes::PROJECT_HAS_WORKSPACES) && !self.forget_workspaces {
            self.stage = ProjectRemoveStage::HasWorkspaces {
                message: refusal.message(),
            };
        } else {
            self.error = Some(refusal.message());
        }
        false
    }

    /// The words of the dialog for the stage it is in.
    pub fn copy(&self) -> DialogCopy {
        match &self.stage {
            ProjectRemoveStage::Confirm => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Remove project {}?", self.project_name),
                body: "MassOrca forgets this project. Its folder, git repository and every worktree stay on disk."
                    .to_owned(),
                loss: Vec::new(),
                action: "Remove project",
            },
            ProjectRemoveStage::HasWorkspaces { message } => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Remove {} with its workspaces?", self.project_name),
                body: format!(
                    "{message} Removing the project also forgets the records of its workspaces. No folder, worktree or branch is deleted."
                ),
                loss: Vec::new(),
                action: "Forget workspaces and remove",
            },
            ProjectRemoveStage::Refused { message } => DialogCopy {
                kind: DialogKind::Error,
                title: "The project was not removed".to_owned(),
                body: message.clone(),
                loss: Vec::new(),
                action: "Close",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::codes;

    use super::*;

    fn has_workspaces() -> Refusal {
        Refusal::Failed {
            code: codes::PROJECT_HAS_WORKSPACES.into(),
            message: "The project still has workspaces.".into(),
        }
    }

    #[test]
    fn the_first_call_carries_no_waiver() {
        let flow = ProjectRemoveFlow::new("p1", "alpha");
        assert_eq!(flow.params().project, "p1");
        assert!(!flow.params().forget_workspaces);
    }

    #[test]
    fn a_project_with_workspaces_asks_before_forgetting_them() {
        let mut flow = ProjectRemoveFlow::new("p1", "alpha");
        assert!(flow.confirm());
        assert!(!flow.answered(Err(has_workspaces())));
        assert_eq!(
            flow.stage(),
            &ProjectRemoveStage::HasWorkspaces {
                message: "The project still has workspaces.".into()
            }
        );
        assert!(flow.confirm());
        assert!(flow.params().forget_workspaces);
    }

    #[test]
    fn a_removed_project_ends_the_flow() {
        let mut flow = ProjectRemoveFlow::new("p1", "alpha");
        assert!(flow.confirm());
        assert!(flow.answered(Ok(())));
    }

    #[test]
    fn a_second_press_while_the_call_is_out_sends_nothing() {
        let mut flow = ProjectRemoveFlow::new("p1", "alpha");
        assert!(flow.confirm());
        assert!(!flow.confirm());
    }
}
