//! The Source control flows as plain data: the discard flow, which asks for each waiver its refusal
//! names (decision 0013), and the commit box, which shows the service's refusals inline (screen
//! D-DF-02). Free of GPUI, so the rules test without a window.

use massorca_protocol::codes;
use massorca_protocol::methods::git::commit::GitCommitParams;
use massorca_protocol::methods::git::index::GitDiscardParams;

use super::workspace_flow::{DialogCopy, DialogKind};
use crate::service::Refusal;

/// The most files a dialog names one by one; the rest are counted.
const LISTED_FILES: usize = 5;

/// Where the discard flow stands: which dialog it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscardStage {
    /// Asks before anything is discarded, so no waiver is given yet.
    Confirm,
    /// `git.discard_untracked`: these untracked files would be deleted.
    Untracked { paths: Vec<String> },
    /// `git.discard_staged`: these files also have staged changes that would be lost.
    Staged { paths: Vec<String> },
    /// Nothing can be waived: a conflicted file, or a refusal that came back after its waiver.
    Refused { message: String },
}

/// The waivers given so far. Each covers one refusal and nothing else (decision 0013).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiscardWaivers {
    pub delete_untracked: bool,
    pub include_staged: bool,
}

/// One discard of the files the user picked, from the confirmation to the answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardFlow {
    workspace_id: String,
    paths: Vec<String>,
    stage: DiscardStage,
    waivers: DiscardWaivers,
    busy: bool,
    error: Option<String>,
}

impl DiscardFlow {
    pub fn new(workspace_id: impl Into<String>, paths: Vec<String>) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            paths,
            stage: DiscardStage::Confirm,
            waivers: DiscardWaivers::default(),
            busy: false,
            error: None,
        }
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    pub fn stage(&self) -> &DiscardStage {
        &self.stage
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The call as it stands: the files and the waivers given so far. Never `all`, never a dry run.
    pub fn params(&self) -> GitDiscardParams {
        GitDiscardParams {
            workspace: self.workspace_id.clone(),
            paths: self.paths.clone(),
            all: false,
            delete_untracked: self.waivers.delete_untracked,
            include_staged: self.waivers.include_staged,
            dry_run: false,
        }
    }

    /// The primary action: gives the waiver this stage asks for and goes busy. False when there is
    /// nothing to run (a plain refusal), or a call is out already.
    pub fn confirm(&mut self) -> bool {
        if self.busy {
            return false;
        }
        match self.stage {
            DiscardStage::Confirm => {}
            DiscardStage::Untracked { .. } => self.waivers.delete_untracked = true,
            DiscardStage::Staged { .. } => self.waivers.include_staged = true,
            DiscardStage::Refused { .. } => return false,
        }
        self.busy = true;
        self.error = None;
        true
    }

    /// The service answered. True when the discard is over: it ran.
    pub fn answered(&mut self, result: Result<(), Refusal>) -> bool {
        self.busy = false;
        match result {
            Ok(()) => return true,
            Err(Refusal::GitRefused {
                code,
                paths,
                message,
            }) => {
                self.stage = match code.as_str() {
                    codes::GIT_DISCARD_UNTRACKED if !self.waivers.delete_untracked => {
                        DiscardStage::Untracked { paths }
                    }
                    codes::GIT_DISCARD_STAGED if !self.waivers.include_staged => {
                        DiscardStage::Staged { paths }
                    }
                    _ => DiscardStage::Refused { message },
                };
            }
            // The link is down: the dialog stays, says why, and the same call can be tried again.
            Err(refusal @ Refusal::Unavailable { .. }) => self.error = Some(refusal.message()),
            Err(other) => {
                self.stage = DiscardStage::Refused {
                    message: other.message(),
                };
            }
        }
        false
    }

    /// What the dialog says at the current stage.
    pub fn copy(&self) -> DialogCopy {
        match &self.stage {
            DiscardStage::Confirm => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Discard changes in {}?", files_title(&self.paths)),
                body: "Edits that are not staged are lost. Staged changes stay.".to_owned(),
                loss: listed(&self.paths),
                action: "Discard changes",
            },
            DiscardStage::Untracked { paths } => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Delete untracked {}?", files_title(paths)),
                body: "Git does not track these files, so they cannot be brought back.".to_owned(),
                loss: listed(paths),
                action: "Delete untracked files",
            },
            DiscardStage::Staged { paths } => DialogCopy {
                kind: DialogKind::Destructive,
                title: "Discard staged changes too?".to_owned(),
                body: "Discarding restores these files from HEAD, so their staged changes are lost too."
                    .to_owned(),
                loss: listed(paths),
                action: "Discard staged changes too",
            },
            DiscardStage::Refused { message } => DialogCopy {
                kind: DialogKind::Error,
                title: "Changes not discarded".to_owned(),
                body: message.clone(),
                loss: Vec::new(),
                action: "Close",
            },
        }
    }
}

/// "a.txt" for one file, "3 files" for more.
fn files_title(paths: &[String]) -> String {
    match paths {
        [only] => only.rsplit('/').next().unwrap_or(only).to_owned(),
        many => format!("{} files", many.len()),
    }
}

/// The paths a dialog lists, with the rest counted.
fn listed(paths: &[String]) -> Vec<String> {
    let mut rows: Vec<String> = paths.iter().take(LISTED_FILES).cloned().collect();
    if paths.len() > LISTED_FILES {
        rows.push(format!("and {} more", paths.len() - LISTED_FILES));
    }
    rows
}

/// The message field and what the service said about the last attempt to commit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitForm {
    pub message: String,
    busy: bool,
    /// Under the message field: the service refused an empty message.
    pub message_error: Option<String>,
    /// Under the box: any other refusal, in the service's words.
    pub error: Option<String>,
}

impl CommitForm {
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// The commit for the message as typed, or `None` while one is out.
    pub fn submit(&mut self, workspace_id: &str) -> Option<GitCommitParams> {
        if self.busy {
            return None;
        }
        self.busy = true;
        self.message_error = None;
        self.error = None;
        Some(GitCommitParams {
            workspace: workspace_id.to_owned(),
            message: Some(self.message.trim().to_owned()),
            amend: false,
            allow_published: false,
        })
    }

    /// The service committed: the box empties and is ready for the next change.
    pub fn committed(&mut self) {
        *self = Self::default();
    }

    /// The service refused: an empty message is said under the field, anything else under the box.
    pub fn refused(&mut self, refusal: &Refusal) {
        self.busy = false;
        match refusal.code() {
            Some(codes::GIT_COMMIT_MESSAGE_EMPTY) => {
                self.message_error = Some("Write a commit message.".to_owned());
            }
            _ => self.error = Some(refusal.message()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn refused(code: &str, names: &[&str]) -> Refusal {
        Refusal::GitRefused {
            code: code.to_owned(),
            message: "the service says no".to_owned(),
            paths: paths(names),
        }
    }

    #[test]
    fn a_discard_asks_first_and_gives_no_waiver_until_one_is_asked_for() {
        let mut flow = DiscardFlow::new("w1", paths(&["src/a.rs"]));
        assert_eq!(flow.stage(), &DiscardStage::Confirm);
        assert!(flow.confirm());
        assert!(flow.busy());
        let params = flow.params();
        assert_eq!(params.workspace, "w1");
        assert_eq!(params.paths, paths(&["src/a.rs"]));
        assert!(
            !params.delete_untracked && !params.include_staged && !params.all && !params.dry_run
        );
    }

    #[test]
    fn an_untracked_refusal_asks_for_delete_untracked_only() {
        let mut flow = DiscardFlow::new("w1", paths(&["a.txt", "b.txt"]));
        flow.confirm();
        assert!(!flow.answered(Err(refused(codes::GIT_DISCARD_UNTRACKED, &["b.txt"]))));
        assert_eq!(
            flow.stage(),
            &DiscardStage::Untracked {
                paths: paths(&["b.txt"])
            }
        );
        assert!(flow.confirm());
        let params = flow.params();
        assert!(params.delete_untracked && !params.include_staged);
        assert!(flow.answered(Ok(())));
    }

    #[test]
    fn a_staged_refusal_has_its_own_waiver_after_the_untracked_one() {
        let mut flow = DiscardFlow::new("w1", paths(&["a.txt"]));
        flow.confirm();
        flow.answered(Err(refused(codes::GIT_DISCARD_UNTRACKED, &["a.txt"])));
        flow.confirm();
        flow.answered(Err(refused(codes::GIT_DISCARD_STAGED, &["a.txt"])));
        assert_eq!(
            flow.stage(),
            &DiscardStage::Staged {
                paths: paths(&["a.txt"])
            }
        );
        flow.confirm();
        let params = flow.params();
        assert!(params.delete_untracked && params.include_staged);
    }

    #[test]
    fn a_refusal_that_comes_back_after_its_waiver_does_not_loop() {
        let mut flow = DiscardFlow::new("w1", paths(&["a.txt"]));
        flow.confirm();
        flow.answered(Err(refused(codes::GIT_DISCARD_UNTRACKED, &["a.txt"])));
        flow.confirm();
        flow.answered(Err(refused(codes::GIT_DISCARD_UNTRACKED, &["a.txt"])));
        assert!(matches!(flow.stage(), DiscardStage::Refused { .. }));
        assert!(!flow.confirm(), "a plain refusal runs nothing");
        assert_eq!(flow.copy().kind, DialogKind::Error);
    }

    #[test]
    fn a_conflicted_file_is_a_plain_refusal_with_the_service_text() {
        let mut flow = DiscardFlow::new("w1", paths(&["Cargo.lock"]));
        flow.confirm();
        flow.answered(Err(refused(codes::GIT_PATH_CONFLICTED, &["Cargo.lock"])));
        assert_eq!(
            flow.stage(),
            &DiscardStage::Refused {
                message: "the service says no".into()
            }
        );
    }

    #[test]
    fn a_link_that_is_down_keeps_the_dialog_and_the_call_can_be_tried_again() {
        let mut flow = DiscardFlow::new("w1", paths(&["a.txt"]));
        flow.confirm();
        flow.answered(Err(Refusal::unavailable("The service is not connected.")));
        assert_eq!(flow.stage(), &DiscardStage::Confirm);
        assert!(!flow.busy());
        assert_eq!(flow.error(), Some("The service is not connected."));
        assert!(flow.confirm(), "the same call goes out again");
        assert_eq!(flow.error(), None);
    }

    #[test]
    fn a_second_confirm_while_a_call_is_out_does_nothing() {
        let mut flow = DiscardFlow::new("w1", paths(&["a.txt"]));
        assert!(flow.confirm());
        assert!(!flow.confirm());
    }

    #[test]
    fn each_dialog_names_its_loss_and_counts_the_files_past_the_list() {
        let many: Vec<String> = (1..=7).map(|n| format!("dir/f{n}.txt")).collect();
        let mut flow = DiscardFlow::new("w1", many);
        let copy = flow.copy();
        assert_eq!(copy.title, "Discard changes in 7 files?");
        assert_eq!(copy.kind, DialogKind::Destructive);
        assert_eq!(copy.loss.len(), LISTED_FILES + 1);
        assert_eq!(copy.loss.last().map(String::as_str), Some("and 2 more"));
        flow.confirm();
        flow.answered(Err(refused(codes::GIT_DISCARD_UNTRACKED, &["dir/f1.txt"])));
        assert_eq!(flow.copy().title, "Delete untracked f1.txt?");
    }

    #[test]
    fn a_single_file_is_named_by_its_name_alone() {
        let flow = DiscardFlow::new("w1", paths(&["src/deep/a.rs"]));
        assert_eq!(flow.copy().title, "Discard changes in a.rs?");
    }

    #[test]
    fn a_blank_message_is_refused_under_the_field_and_other_refusals_under_the_box() {
        let mut form = CommitForm::default();
        assert!(form.submit("w1").is_some());
        assert!(form.busy());
        assert!(form.submit("w1").is_none(), "one commit at a time");
        form.refused(&Refusal::Failed {
            code: codes::GIT_COMMIT_MESSAGE_EMPTY.into(),
            message: "empty".into(),
        });
        assert_eq!(
            form.message_error.as_deref(),
            Some("Write a commit message.")
        );
        assert!(!form.busy());
        form.submit("w1");
        form.refused(&Refusal::Failed {
            code: codes::GIT_NOTHING_TO_COMMIT.into(),
            message: "Nothing is staged.".into(),
        });
        assert_eq!(form.error.as_deref(), Some("Nothing is staged."));
        assert_eq!(form.message_error, None);
    }

    #[test]
    fn a_commit_sends_the_trimmed_message_and_empties_the_box() {
        let mut form = CommitForm {
            message: "  fix the parser \n".into(),
            ..CommitForm::default()
        };
        let params = form.submit("w1").expect("the commit goes out");
        assert_eq!(params.message.as_deref(), Some("fix the parser"));
        assert!(!params.amend && !params.allow_published);
        form.committed();
        assert_eq!(form, CommitForm::default());
    }
}
