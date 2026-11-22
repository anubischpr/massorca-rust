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

/// One file a discard covers. An untracked file has no content to restore, so discarding it deletes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardTarget {
    pub path: String,
    pub untracked: bool,
}

/// What an answer did to the dialog it reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The answer belongs to another call, so the dialog did not change.
    Ignored,
    /// The discard ran, so the dialog is over.
    Finished,
    /// The dialog stays open: a waiver is asked for, or the call failed and can be tried again.
    Continues,
}

/// One discard of the files the user picked, from the confirmation to the answer. The dialog has an id,
/// and each call it makes has one: an answer counts only for the call the dialog is waiting for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardFlow {
    id: u64,
    workspace_id: String,
    targets: Vec<DiscardTarget>,
    stage: DiscardStage,
    waivers: DiscardWaivers,
    /// The call out for this dialog, while one is.
    pending: Option<u64>,
    error: Option<String>,
}

impl DiscardFlow {
    /// The first dialog names the deletion when a target is untracked, so the first call carries that
    /// waiver: the user confirms once, for exactly what the dialog says.
    pub fn new(id: u64, workspace_id: impl Into<String>, targets: Vec<DiscardTarget>) -> Self {
        let delete_untracked = targets.iter().any(|target| target.untracked);
        Self {
            id,
            workspace_id: workspace_id.into(),
            targets,
            stage: DiscardStage::Confirm,
            waivers: DiscardWaivers {
                delete_untracked,
                include_staged: false,
            },
            pending: None,
            error: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn paths(&self) -> Vec<String> {
        self.targets
            .iter()
            .map(|target| target.path.clone())
            .collect()
    }

    pub fn stage(&self) -> &DiscardStage {
        &self.stage
    }

    /// A call is out for this dialog.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The call as it stands: the files and the waivers given so far. Never `all`, never a dry run.
    pub fn params(&self) -> GitDiscardParams {
        GitDiscardParams {
            workspace: self.workspace_id.clone(),
            paths: self.paths(),
            all: false,
            delete_untracked: self.waivers.delete_untracked,
            include_staged: self.waivers.include_staged,
            dry_run: false,
        }
    }

    /// The primary action: gives the waiver this stage asks for and sends the call under `call`. None when
    /// nothing can run (a plain refusal, or a call is out already).
    pub fn confirm(&mut self, call: u64) -> Option<GitDiscardParams> {
        if self.pending.is_some() {
            return None;
        }
        match self.stage {
            DiscardStage::Confirm => {}
            DiscardStage::Untracked { .. } => self.waivers.delete_untracked = true,
            DiscardStage::Staged { .. } => self.waivers.include_staged = true,
            DiscardStage::Refused { .. } => return None,
        }
        self.pending = Some(call);
        self.error = None;
        Some(self.params())
    }

    /// The service answered `call`. Only the call this dialog waits for counts, and any other answer
    /// changes nothing. A refusal can offer a waiver only for files this dialog covers.
    pub fn answered(&mut self, call: u64, result: Result<(), Refusal>) -> Answer {
        if self.pending != Some(call) {
            return Answer::Ignored;
        }
        self.pending = None;
        match result {
            Ok(()) => return Answer::Finished,
            Err(Refusal::GitRefused {
                code,
                paths,
                message,
            }) => {
                let covered = paths
                    .iter()
                    .all(|path| self.targets.iter().any(|target| target.path == *path));
                self.stage = match code.as_str() {
                    codes::GIT_DISCARD_UNTRACKED if covered && !self.waivers.delete_untracked => {
                        DiscardStage::Untracked { paths }
                    }
                    codes::GIT_DISCARD_STAGED if covered && !self.waivers.include_staged => {
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
        Answer::Continues
    }

    /// What the dialog says at the current stage.
    pub fn copy(&self) -> DialogCopy {
        match &self.stage {
            DiscardStage::Confirm => self.confirm_copy(),
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

    /// The first dialog. It names exactly what the call will do, so the user confirms once.
    fn confirm_copy(&self) -> DialogCopy {
        let paths = self.paths();
        let untracked = self
            .targets
            .iter()
            .filter(|target| target.untracked)
            .count();
        let tracked = self.targets.len() - untracked;
        match (tracked, untracked) {
            (0, 1) => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Delete {}?", files_title(&paths)),
                body: "Git does not track this file, so it cannot be brought back.".to_owned(),
                loss: listed(&paths),
                action: "Delete file",
            },
            (0, _) => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Delete {untracked} untracked files?"),
                body: "Git does not track these files, so they cannot be brought back.".to_owned(),
                loss: listed(&paths),
                action: "Delete files",
            },
            (_, 0) => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Discard changes in {}?", files_title(&paths)),
                body: "Edits that are not staged are lost. Staged changes stay.".to_owned(),
                loss: listed(&paths),
                action: "Discard changes",
            },
            _ => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Discard changes in {}?", files_title(&paths)),
                body: "Edits that are not staged are lost, and the untracked files are deleted. Staged changes stay."
                    .to_owned(),
                loss: listed(&paths),
                action: "Discard changes",
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
    const CALL: u64 = 1;

    fn paths(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn target(path: &str, untracked: bool) -> DiscardTarget {
        DiscardTarget {
            path: path.to_owned(),
            untracked,
        }
    }

    fn tracked(names: &[&str]) -> Vec<DiscardTarget> {
        names.iter().map(|name| target(name, false)).collect()
    }

    fn tracked_owned(names: Vec<String>) -> Vec<DiscardTarget> {
        names
            .into_iter()
            .map(|name| DiscardTarget {
                path: name,
                untracked: false,
            })
            .collect()
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
        let mut flow = DiscardFlow::new(1, "w1", tracked(&["src/a.rs"]));
        assert_eq!(flow.stage(), &DiscardStage::Confirm);
        assert!(flow.confirm(CALL).is_some());
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
        let mut flow = DiscardFlow::new(1, "w1", tracked(&["a.txt", "b.txt"]));
        flow.confirm(CALL);
        assert_eq!(
            flow.answered(CALL, Err(refused(codes::GIT_DISCARD_UNTRACKED, &["b.txt"]))),
            Answer::Continues
        );
        assert_eq!(
            flow.stage(),
            &DiscardStage::Untracked {
                paths: paths(&["b.txt"])
            }
        );
        assert!(flow.confirm(CALL).is_some());
        let params = flow.params();
        assert!(params.delete_untracked && !params.include_staged);
        assert_eq!(flow.answered(CALL, Ok(())), Answer::Finished);
    }

    #[test]
    fn a_staged_refusal_has_its_own_waiver_after_the_untracked_one() {
        let mut flow = DiscardFlow::new(1, "w1", tracked(&["a.txt"]));
        flow.confirm(CALL);
        flow.answered(CALL, Err(refused(codes::GIT_DISCARD_UNTRACKED, &["a.txt"])));
        flow.confirm(CALL);
        flow.answered(CALL, Err(refused(codes::GIT_DISCARD_STAGED, &["a.txt"])));
        assert_eq!(
            flow.stage(),
            &DiscardStage::Staged {
                paths: paths(&["a.txt"])
            }
        );
        flow.confirm(CALL);
        let params = flow.params();
        assert!(params.delete_untracked && params.include_staged);
    }

    #[test]
    fn a_refusal_that_comes_back_after_its_waiver_does_not_loop() {
        let mut flow = DiscardFlow::new(1, "w1", tracked(&["a.txt"]));
        flow.confirm(CALL);
        flow.answered(CALL, Err(refused(codes::GIT_DISCARD_UNTRACKED, &["a.txt"])));
        flow.confirm(CALL);
        flow.answered(CALL, Err(refused(codes::GIT_DISCARD_UNTRACKED, &["a.txt"])));
        assert!(matches!(flow.stage(), DiscardStage::Refused { .. }));
        assert!(flow.confirm(CALL).is_none(), "a plain refusal runs nothing");
        assert_eq!(flow.copy().kind, DialogKind::Error);
    }

    #[test]
    fn a_conflicted_file_is_a_plain_refusal_with_the_service_text() {
        let mut flow = DiscardFlow::new(1, "w1", tracked(&["Cargo.lock"]));
        flow.confirm(CALL);
        flow.answered(
            CALL,
            Err(refused(codes::GIT_PATH_CONFLICTED, &["Cargo.lock"])),
        );
        assert_eq!(
            flow.stage(),
            &DiscardStage::Refused {
                message: "the service says no".into()
            }
        );
    }

    #[test]
    fn a_link_that_is_down_keeps_the_dialog_and_the_call_can_be_tried_again() {
        let mut flow = DiscardFlow::new(1, "w1", tracked(&["a.txt"]));
        flow.confirm(CALL);
        flow.answered(
            CALL,
            Err(Refusal::unavailable("The service is not connected.")),
        );
        assert_eq!(flow.stage(), &DiscardStage::Confirm);
        assert!(!flow.busy());
        assert_eq!(flow.error(), Some("The service is not connected."));
        assert!(flow.confirm(CALL).is_some(), "the same call goes out again");
        assert_eq!(flow.error(), None);
    }

    #[test]
    fn a_second_confirm_while_a_call_is_out_does_nothing() {
        let mut flow = DiscardFlow::new(1, "w1", tracked(&["a.txt"]));
        assert!(flow.confirm(CALL).is_some());
        assert!(flow.confirm(CALL).is_none());
    }

    #[test]
    fn each_dialog_names_its_loss_and_counts_the_files_past_the_list() {
        let many: Vec<String> = (1..=7).map(|n| format!("dir/f{n}.txt")).collect();
        let mut flow = DiscardFlow::new(1, "w1", tracked_owned(many));
        let copy = flow.copy();
        assert_eq!(copy.title, "Discard changes in 7 files?");
        assert_eq!(copy.kind, DialogKind::Destructive);
        assert_eq!(copy.loss.len(), LISTED_FILES + 1);
        assert_eq!(copy.loss.last().map(String::as_str), Some("and 2 more"));
        flow.confirm(CALL);
        flow.answered(
            CALL,
            Err(refused(codes::GIT_DISCARD_UNTRACKED, &["dir/f1.txt"])),
        );
        assert_eq!(flow.copy().title, "Delete untracked f1.txt?");
    }

    #[test]
    fn a_single_file_is_named_by_its_name_alone() {
        let flow = DiscardFlow::new(1, "w1", tracked(&["src/deep/a.rs"]));
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

    #[test]
    fn an_untracked_only_selection_is_a_deletion_from_its_first_dialog() {
        let mut flow = DiscardFlow::new(1, "w1", vec![target("notes.md", true)]);
        let copy = flow.copy();
        assert_eq!(copy.title, "Delete notes.md?");
        assert_eq!(copy.kind, DialogKind::Destructive);
        assert_eq!(copy.action, "Delete file");
        assert!(copy.body.contains("cannot be brought back"));
        assert_eq!(copy.loss, vec!["notes.md".to_owned()]);
        assert!(
            flow.params().delete_untracked,
            "the first call carries the waiver the dialog names"
        );
        assert!(flow.confirm(CALL).is_some());
        assert_eq!(
            flow.answered(CALL, Ok(())),
            Answer::Finished,
            "one confirmation is enough"
        );
    }

    #[test]
    fn several_untracked_files_are_counted_in_the_first_dialog() {
        let flow = DiscardFlow::new(1, "w1", vec![target("a.md", true), target("b.md", true)]);
        assert_eq!(flow.copy().title, "Delete 2 untracked files?");
        assert_eq!(flow.copy().action, "Delete files");
        assert!(flow.params().delete_untracked);
    }

    #[test]
    fn a_mixed_selection_names_both_losses_in_the_first_dialog() {
        let flow = DiscardFlow::new(
            1,
            "w1",
            vec![target("src/a.rs", false), target("b.txt", true)],
        );
        let copy = flow.copy();
        assert_eq!(copy.title, "Discard changes in 2 files?");
        assert!(copy.body.contains("untracked files are deleted"));
        assert_eq!(copy.action, "Discard changes");
        assert!(flow.params().delete_untracked);
    }

    #[test]
    fn a_tracked_selection_names_only_the_edits_and_sends_no_deletion_waiver() {
        let flow = DiscardFlow::new(1, "w1", vec![target("src/a.rs", false)]);
        let copy = flow.copy();
        assert_eq!(copy.title, "Discard changes in a.rs?");
        assert!(!copy.body.contains("deleted"));
        assert!(!flow.params().delete_untracked);
    }

    #[test]
    fn an_answer_for_another_call_changes_nothing() {
        let mut flow = DiscardFlow::new(1, "w1", tracked(&["a.txt"]));
        assert!(flow.confirm(CALL).is_some());
        assert_eq!(
            flow.answered(
                CALL + 1,
                Err(refused(codes::GIT_DISCARD_STAGED, &["a.txt"]))
            ),
            Answer::Ignored
        );
        assert_eq!(flow.stage(), &DiscardStage::Confirm);
        assert!(flow.busy(), "the call the dialog waits for is still out");
        assert_eq!(flow.answered(CALL, Ok(())), Answer::Finished);
    }

    #[test]
    fn a_refusal_offers_a_waiver_only_for_files_the_dialog_covers() {
        let mut flow = DiscardFlow::new(1, "w1", tracked(&["a.txt"]));
        flow.confirm(CALL);
        flow.answered(CALL, Err(refused(codes::GIT_DISCARD_STAGED, &["b.txt"])));
        assert_eq!(
            flow.stage(),
            &DiscardStage::Refused {
                message: "the service says no".into()
            },
            "no waiver for a file this dialog does not cover"
        );
    }

    #[test]
    fn a_new_dialog_has_no_call_out_so_an_old_answer_cannot_reach_it() {
        let mut flow = DiscardFlow::new(2, "w1", tracked(&["b.txt"]));
        assert_eq!(
            flow.answered(CALL, Err(refused(codes::GIT_DISCARD_STAGED, &["b.txt"]))),
            Answer::Ignored
        );
        assert_eq!(flow.stage(), &DiscardStage::Confirm);
        assert_eq!(flow.id(), 2);
    }
}
