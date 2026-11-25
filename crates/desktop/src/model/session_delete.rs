//! The confirm of deleting a session (Delete session…, A97): what its dialog says at each stage, and how the service's
//! answers move it. The call is `agent.forget`, which never deletes the agent's own saved conversation.

use super::workspace_flow::{DialogCopy, DialogKind};
use crate::service::{ForgetSessionParams, Refusal};

/// Where the delete stands: the confirm, the question about a dirty worktree, or a plain refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionDeleteStage {
    Confirm,
    /// The worktree has uncommitted changes, so removing it asks to discard them.
    Dirty,
    /// A refusal nothing in the dialog can waive: the service's own sentence.
    Refused {
        message: String,
    },
}

/// Deleting one session. Its first call forgets the record; with the worktree option it also removes the worktree and
/// branch, and a dirty worktree is refused until the person discards its changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionDeleteFlow {
    pub session_id: String,
    pub title: String,
    /// Whether the dialog offers the worktree option. A plain folder has no worktree or branch to remove.
    offers_workspace: bool,
    remove_workspace: bool,
    force: bool,
    stage: SessionDeleteStage,
    busy: bool,
    error: Option<String>,
}

impl SessionDeleteFlow {
    pub fn new(
        session_id: impl Into<String>,
        title: impl Into<String>,
        offers_workspace: bool,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            title: title.into(),
            offers_workspace,
            remove_workspace: false,
            force: false,
            stage: SessionDeleteStage::Confirm,
            busy: false,
            error: None,
        }
    }

    pub fn stage(&self) -> &SessionDeleteStage {
        &self.stage
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn offers_workspace(&self) -> bool {
        self.offers_workspace
    }

    pub fn remove_workspace(&self) -> bool {
        self.remove_workspace
    }

    /// The worktree option. It can change only while the confirm is open and no call is out; false when it did not.
    pub fn set_remove_workspace(&mut self, on: bool) -> bool {
        if !self.offers_workspace || self.busy || self.stage != SessionDeleteStage::Confirm {
            return false;
        }
        self.remove_workspace = on;
        true
    }

    /// The params of the next call: the flags the flow has been given so far, and none it has not.
    pub fn params(&self) -> ForgetSessionParams {
        ForgetSessionParams {
            session: self.session_id.clone(),
            remove_workspace: self.remove_workspace,
            force: self.force,
        }
    }

    /// The primary action: gives the waiver this stage asks for and goes busy. False when the stage has nothing to
    /// run (a plain refusal) or a call is already out.
    pub fn confirm(&mut self) -> bool {
        if self.busy {
            return false;
        }
        match self.stage {
            SessionDeleteStage::Confirm => {}
            SessionDeleteStage::Dirty => self.force = true,
            SessionDeleteStage::Refused { .. } => return false,
        }
        self.busy = true;
        self.error = None;
        true
    }

    /// The service answered. Returns true when the session was deleted and the flow is over.
    pub fn answered(&mut self, result: Result<(), Refusal>) -> bool {
        self.busy = false;
        let refusal = match result {
            Ok(()) => return true,
            Err(refusal) => refusal,
        };
        let next = match &refusal {
            Refusal::Dirty if self.remove_workspace && !self.force => SessionDeleteStage::Dirty,
            Refusal::Dirty
            | Refusal::IgnoredFiles
            | Refusal::Orphan
            | Refusal::Locked { .. }
            | Refusal::MainWorktree { .. }
            | Refusal::Failed { .. } => SessionDeleteStage::Refused {
                message: refusal.message(),
            },
            // The call itself did not reach the service's answer: the dialog stays and shows why.
            Refusal::NameProblem { .. }
            | Refusal::PathProblem { .. }
            | Refusal::GitRefused { .. }
            | Refusal::Unavailable { .. } => {
                self.error = Some(refusal.message());
                return false;
            }
        };
        self.stage = next;
        false
    }

    /// What the current dialog says.
    pub fn copy(&self) -> DialogCopy {
        let title = &self.title;
        match &self.stage {
            SessionDeleteStage::Confirm if self.remove_workspace => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Delete session {title}?"),
                body: "This removes the session, its worktree and its branch from This PC. The agent's own saved \
                       conversation in its account home is never deleted."
                    .to_owned(),
                loss: vec!["The worktree folder and its branch are deleted.".to_owned()],
                action: DELETE_ACTION,
            },
            SessionDeleteStage::Confirm => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Delete session {title}?"),
                body: "This removes the session from This PC. Its worktree and branch stay where they are. The \
                       agent's own saved conversation in its account home is never deleted."
                    .to_owned(),
                loss: Vec::new(),
                action: DELETE_ACTION,
            },
            SessionDeleteStage::Dirty => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Delete session {title}?"),
                body: "The worktree has changes that are not committed, so nothing is removed yet.".to_owned(),
                loss: vec!["Uncommitted and untracked changes in the worktree are deleted.".to_owned()],
                action: DISCARD_ACTION,
            },
            SessionDeleteStage::Refused { message } => DialogCopy {
                kind: DialogKind::Error,
                title: format!("Cannot delete {title}"),
                body: message.clone(),
                loss: Vec::new(),
                action: "Close",
            },
        }
    }
}

const DELETE_ACTION: &str = "Delete session";
const DISCARD_ACTION: &str = "Discard changes and delete";

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(offers_workspace: bool) -> SessionDeleteFlow {
        SessionDeleteFlow::new("s1", "feat/billing", offers_workspace)
    }

    #[test]
    fn the_plain_confirm_names_the_session_and_keeps_the_worktree_and_the_conversation() {
        let copy = flow(true).copy();
        assert_eq!(copy.kind, DialogKind::Destructive);
        assert_eq!(copy.title, "Delete session feat/billing?");
        assert!(copy.body.contains("stay where they are"));
        assert!(copy.body.contains("never deleted"));
        assert!(copy.loss.is_empty());
        assert_eq!(copy.action, "Delete session");
    }

    #[test]
    fn the_worktree_option_says_the_worktree_and_branch_go() {
        let mut flow = flow(true);
        assert!(flow.set_remove_workspace(true));
        let copy = flow.copy();
        assert!(copy.body.contains("its worktree and its branch"));
        assert!(copy.body.contains("never deleted"));
        assert_eq!(
            copy.loss,
            ["The worktree folder and its branch are deleted."]
        );
    }

    #[test]
    fn a_plain_folder_offers_no_worktree_option() {
        let mut flow = flow(false);
        assert!(!flow.offers_workspace());
        assert!(!flow.set_remove_workspace(true));
        assert!(!flow.params().remove_workspace);
    }

    #[test]
    fn the_plain_call_sends_only_the_session() {
        let params = flow(true).params();
        assert_eq!(
            params,
            ForgetSessionParams {
                session: "s1".into(),
                remove_workspace: false,
                force: false,
            }
        );
    }

    #[test]
    fn a_dirty_worktree_asks_to_discard_and_then_retries_with_force_only() {
        let mut flow = flow(true);
        flow.set_remove_workspace(true);
        assert!(flow.confirm());
        assert!(!flow.answered(Err(Refusal::Dirty)));
        assert_eq!(flow.stage(), &SessionDeleteStage::Dirty);
        assert!(flow.confirm());
        assert!(flow.params().force, "the second call carries force");
        assert!(flow.answered(Ok(())));
    }

    #[test]
    fn a_refusal_shows_the_services_sentence_and_closes_with_close() {
        let mut flow = flow(false);
        assert!(flow.confirm());
        let refusal = Refusal::Failed {
            code: "agent.busy".into(),
            message: "Beta still uses this worktree.".into(),
        };
        assert!(!flow.answered(Err(refusal)));
        assert_eq!(
            flow.stage(),
            &SessionDeleteStage::Refused {
                message: "Beta still uses this worktree.".into(),
            }
        );
        assert_eq!(flow.copy().action, "Close");
        assert!(!flow.confirm(), "a plain refusal has nothing to run");
    }

    #[test]
    fn a_call_that_never_reached_the_service_keeps_the_confirm_with_an_error() {
        let mut flow = flow(true);
        assert!(flow.confirm());
        assert!(!flow.answered(Err(Refusal::Unavailable {
            message: "The service is not reachable.".into(),
        })));
        assert_eq!(flow.stage(), &SessionDeleteStage::Confirm);
        assert_eq!(flow.error(), Some("The service is not reachable."));
        assert!(!flow.busy());
    }
}
