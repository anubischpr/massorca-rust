//! The Source control panel's Fetch, Pull and Push actions (GIT-19): their labels, and the note each
//! answer leaves under the panel. The calls and their refusals belong to the shell.

use massorca_protocol::methods::git::remote::{GitFetched, GitPulled, GitPushed, PushOutcome};

/// One remote action of the panel, in the order its buttons show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SyncAction {
    Fetch,
    Pull,
    Push,
}

impl SyncAction {
    pub(crate) const ALL: [Self; 3] = [Self::Fetch, Self::Pull, Self::Push];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Fetch => "Fetch",
            Self::Pull => "Pull",
            Self::Push => "Push",
        }
    }

    /// The id of the action's button, which the panel and its tests address.
    pub(crate) fn button_id(self) -> &'static str {
        match self {
            Self::Fetch => "source-control-fetch",
            Self::Pull => "source-control-pull",
            Self::Push => "source-control-push",
        }
    }
}

/// How the panel shows a note: a plain result, or one that leaves something for the user to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SyncTone {
    Info,
    Warning,
}

/// The note a finished action leaves under the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SyncNote {
    pub(crate) text: String,
    pub(crate) tone: SyncTone,
}

impl SyncNote {
    fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: SyncTone::Info,
        }
    }

    fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: SyncTone::Warning,
        }
    }
}

/// The note for a finished fetch: how many remote-tracking refs it moved.
pub(crate) fn fetched(answer: &GitFetched) -> SyncNote {
    match answer.updated.len() {
        0 => SyncNote::info("Nothing new on the remote."),
        1 => SyncNote::info("Fetched 1 changed ref."),
        count => SyncNote::info(format!("Fetched {count} changed refs.")),
    }
}

/// The note for a finished pull. A pull that left the branch as it was says what it left.
pub(crate) fn pulled(answer: &GitPulled) -> SyncNote {
    match answer {
        GitPulled::UpToDate => SyncNote::info("Already up to date."),
        GitPulled::FastForwarded { to, .. } => {
            SyncNote::info(format!("Fast-forwarded to {}.", short(to)))
        }
        GitPulled::Merged { commit } => SyncNote::info(format!("Merged as {}.", short(commit))),
        GitPulled::Rebased { head } => {
            SyncNote::info(format!("Rebased onto the upstream at {}.", short(head)))
        }
        GitPulled::Diverged { ahead, behind } => SyncNote::warning(format!(
            "Diverged: {ahead} ahead and {behind} behind. Nothing changed."
        )),
        GitPulled::Conflicts { paths } => SyncNote::warning(format!(
            "Conflicts in {}. Resolve them in your terminal, or abort the merge or rebase.",
            files(paths.len())
        )),
        GitPulled::Unknown => SyncNote::info("Pulled."),
    }
}

/// The note for a finished push, or the reason the remote refused it, which is an error.
pub(crate) fn pushed(answer: &GitPushed) -> Result<SyncNote, String> {
    let branch = answer.remote_branch.as_str();
    match &answer.outcome {
        PushOutcome::Created if answer.upstream_set => Ok(SyncNote::info(format!(
            "Pushed {branch} and set its upstream."
        ))),
        PushOutcome::Created | PushOutcome::FastForward | PushOutcome::Unknown => {
            Ok(SyncNote::info(format!("Pushed {branch}.")))
        }
        PushOutcome::Forced => Ok(SyncNote::info(format!(
            "Pushed {branch} with force-with-lease."
        ))),
        PushOutcome::UpToDate => Ok(SyncNote::info(format!(
            "{branch} is already up to date on the remote."
        ))),
        PushOutcome::Rejected { reason } => Err(format!("The remote rejected the push: {reason}.")),
    }
}

/// A commit id's first seven characters, as the panel shows them.
fn short(id: &str) -> String {
    id.chars().take(7).collect()
}

fn files(count: usize) -> String {
    if count == 1 {
        "1 file".to_owned()
    } else {
        format!("{count} files")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::git_pushed;

    #[test]
    fn the_three_actions_keep_their_labels_and_button_ids() {
        let labels: Vec<&str> = SyncAction::ALL
            .iter()
            .map(|action| action.label())
            .collect();
        assert_eq!(labels, ["Fetch", "Pull", "Push"]);
        assert_eq!(SyncAction::Push.button_id(), "source-control-push");
    }

    #[test]
    fn a_fetch_that_moved_nothing_says_so() {
        assert_eq!(
            fetched(&GitFetched::default()),
            SyncNote::info("Nothing new on the remote.")
        );
    }

    #[test]
    fn a_pull_that_diverged_names_both_counts_and_changes_nothing() {
        assert_eq!(
            pulled(&GitPulled::Diverged {
                ahead: 2,
                behind: 1
            }),
            SyncNote::warning("Diverged: 2 ahead and 1 behind. Nothing changed.")
        );
    }

    #[test]
    fn a_fast_forward_names_the_short_commit() {
        let answer = GitPulled::FastForwarded {
            from: "a".repeat(40),
            to: "b".repeat(40),
        };
        assert_eq!(pulled(&answer).text, "Fast-forwarded to bbbbbbb.");
    }

    #[test]
    fn a_conflicting_pull_counts_its_files_in_the_singular_and_plural() {
        let one = GitPulled::Conflicts {
            paths: vec!["a.rs".into()],
        };
        assert!(pulled(&one).text.starts_with("Conflicts in 1 file."));
        let two = GitPulled::Conflicts {
            paths: vec!["a.rs".into(), "b.rs".into()],
        };
        assert!(pulled(&two).text.starts_with("Conflicts in 2 files."));
    }

    #[test]
    fn a_rejected_push_is_an_error_with_the_remote_reason() {
        let answer = git_pushed(PushOutcome::Rejected {
            reason: "non-fast-forward".into(),
        });
        assert_eq!(
            pushed(&answer),
            Err("The remote rejected the push: non-fast-forward.".to_owned())
        );
    }

    #[test]
    fn a_push_that_sets_the_upstream_says_so() {
        let mut answer = git_pushed(PushOutcome::Created);
        answer.upstream_set = true;
        assert_eq!(
            pushed(&answer),
            Ok(SyncNote::info("Pushed feat/scm and set its upstream."))
        );
    }
}
