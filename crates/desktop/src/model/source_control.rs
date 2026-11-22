//! The Source control panel as plain data (screen D-DF-02, decision 0010): the branch, the changes in
//! four groups (conflicted, staged, changed, untracked) and the history. Free of GPUI, so the rules
//! test without a window.

use massorca_protocol::codes;
use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::log::{Commit, GitLog};
use massorca_protocol::methods::git::status::{GitOperation, GitStatus, StatusEntry};

use crate::service::{Refusal, WorkspaceGit};

/// The letter and word a change shows: the file chip of DiffView.md and the git letter of FileTree.md.
pub type Label = (&'static str, &'static str);

const UNTRACKED: Label = ("?", "Untracked");
const CONFLICTED: Label = ("U", "Conflicted");

/// What the panel shows for one workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelView {
    /// The service has not answered for this workspace yet.
    Loading,
    /// The folder is not a Git repository, so Source control has nothing to show (D-WS-14).
    NotRepository,
    /// The status could not be read; `message` is the service's text.
    Failed {
        message: String,
    },
    Ready(Box<Panel>),
}

/// A repository's state the panel lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Panel {
    pub branch: BranchLine,
    /// A merge, rebase, cherry-pick, revert or mailbox apply that stopped midway, by its name.
    pub operation: Option<&'static str>,
    pub conflicted: Vec<ChangeRow>,
    pub staged: Vec<ChangeRow>,
    pub changes: Vec<ChangeRow>,
    pub untracked: Vec<ChangeRow>,
    /// The service cut the list at its limit; the rows shown are its first entries.
    pub truncated: bool,
    /// Why working-tree edits are not observed, while they are not (decision 0012).
    pub degraded: Option<String>,
    pub history: History,
}

impl Panel {
    /// Nothing is changed, staged, conflicted or untracked ("Nothing to commit.").
    pub fn is_clean(&self) -> bool {
        self.conflicted.is_empty()
            && self.staged.is_empty()
            && self.changes.is_empty()
            && self.untracked.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchLine {
    /// The checked-out branch, or "Detached HEAD".
    pub name: String,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    /// The upstream is configured but gone from the remote.
    pub upstream_gone: bool,
    /// HEAD has no commit yet.
    pub unborn: bool,
}

/// One changed file, as a row of a group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeRow {
    /// The workspace-relative path, which the actions name.
    pub path: String,
    pub name: String,
    /// The directory part, empty at the root.
    pub directory: String,
    /// The path before a rename.
    pub original_path: Option<String>,
    pub letter: &'static str,
    pub word: &'static str,
    /// Added and removed lines on this side; `None` where git counts none.
    pub lines: Option<(u64, u64)>,
    pub binary: bool,
}

/// The first page of history, as the panel lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum History {
    Loading,
    Failed { message: String },
    Loaded { commits: Vec<CommitRow>, more: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitRow {
    pub id: String,
    /// The first seven characters of the id, as the rows show it.
    pub short_id: String,
    pub subject: String,
    pub author: String,
}

/// The panel for one workspace, derived from what the store holds (decision 0028).
pub fn panel_view(git: Option<&WorkspaceGit>) -> PanelView {
    let Some(git) = git else {
        return PanelView::Loading;
    };
    match &git.status {
        None => PanelView::Loading,
        Some(Err(refusal)) if refusal.code() == Some(codes::GIT_NOT_A_REPOSITORY) => {
            PanelView::NotRepository
        }
        Some(Err(refusal)) => PanelView::Failed {
            message: refusal.message(),
        },
        Some(Ok(status)) => PanelView::Ready(Box::new(panel(status, git))),
    }
}

fn panel(status: &GitStatus, git: &WorkspaceGit) -> Panel {
    let mut panel = Panel {
        branch: branch_line(status),
        operation: status.operation.map(operation_name),
        conflicted: Vec::new(),
        staged: Vec::new(),
        changes: Vec::new(),
        untracked: Vec::new(),
        truncated: status.truncated,
        degraded: git.degraded.clone(),
        history: history(git.log.as_ref()),
    };
    for entry in &status.entries {
        if entry.conflict.is_some() {
            panel.conflicted.push(row(entry, CONFLICTED, None, false));
            continue;
        }
        if let Some(kind) = entry.staged {
            let counts = entry.staged_lines;
            panel.staged.push(row(
                entry,
                change_label(kind),
                counts.map(|c| (c.added, c.removed)),
                counts.is_some_and(|c| c.binary),
            ));
        }
        if let Some(kind) = entry.unstaged {
            let counts = entry.unstaged_lines;
            panel.changes.push(row(
                entry,
                change_label(kind),
                counts.map(|c| (c.added, c.removed)),
                counts.is_some_and(|c| c.binary),
            ));
        }
        if entry.untracked {
            panel.untracked.push(row(entry, UNTRACKED, None, false));
        }
    }
    panel
}

/// The letter and word of a change kind. A kind this build does not know reads as a plain change.
pub fn change_label(kind: ChangeKind) -> Label {
    match kind {
        ChangeKind::Added => ("A", "Added"),
        ChangeKind::Modified => ("M", "Modified"),
        ChangeKind::Deleted => ("D", "Deleted"),
        ChangeKind::Renamed => ("R", "Renamed"),
        ChangeKind::Copied => ("C", "Copied"),
        ChangeKind::TypeChanged => ("T", "Type changed"),
        ChangeKind::Unknown => ("?", "Changed"),
    }
}

fn row(entry: &StatusEntry, label: Label, lines: Option<(u64, u64)>, binary: bool) -> ChangeRow {
    let (directory, name) = match entry.path.rsplit_once('/') {
        Some((directory, name)) => (directory.to_owned(), name.to_owned()),
        None => (String::new(), entry.path.clone()),
    };
    ChangeRow {
        path: entry.path.clone(),
        name,
        directory,
        original_path: entry.original_path.clone(),
        letter: label.0,
        word: label.1,
        lines,
        binary,
    }
}

fn branch_line(status: &GitStatus) -> BranchLine {
    let branch = &status.branch;
    let (name, detached) = match &branch.head {
        Some(name) => (name.clone(), false),
        None => ("Detached HEAD".to_owned(), true),
    };
    BranchLine {
        name,
        detached,
        upstream: branch.upstream.clone(),
        ahead: branch.ahead.unwrap_or(0),
        behind: branch.behind.unwrap_or(0),
        upstream_gone: branch.upstream_gone,
        unborn: branch.oid.is_none(),
    }
}

fn operation_name(operation: GitOperation) -> &'static str {
    match operation {
        GitOperation::Merge => "Merge in progress",
        GitOperation::Rebase => "Rebase in progress",
        GitOperation::CherryPick => "Cherry-pick in progress",
        GitOperation::Revert => "Revert in progress",
        GitOperation::ApplyMailbox => "Patch apply in progress",
        GitOperation::Unknown => "An operation is in progress",
    }
}

fn history(log: Option<&Result<GitLog, Refusal>>) -> History {
    match log {
        None => History::Loading,
        Some(Err(refusal)) => History::Failed {
            message: refusal.message(),
        },
        Some(Ok(log)) => History::Loaded {
            commits: log.commits.iter().map(commit_row).collect(),
            more: log.next_cursor.is_some(),
        },
    }
}

fn commit_row(commit: &Commit) -> CommitRow {
    CommitRow {
        id: commit.id.clone(),
        short_id: commit.id.chars().take(7).collect(),
        subject: commit.subject.clone(),
        author: commit.author.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::git::LineCounts;
    use massorca_protocol::methods::git::log::Signature;
    use massorca_protocol::methods::git::status::{
        BranchState, ConflictKind, GitOperation, StatusTotals,
    };

    use super::*;
    use crate::service::Refusal;

    fn entry(path: &str) -> StatusEntry {
        StatusEntry {
            path: path.to_owned(),
            original_path: None,
            staged: None,
            unstaged: None,
            untracked: false,
            conflict: None,
            staged_lines: None,
            unstaged_lines: None,
            submodule: false,
            path_lossy: false,
        }
    }

    fn status(entries: Vec<StatusEntry>) -> GitStatus {
        GitStatus {
            branch: BranchState {
                head: Some("feat/scm".into()),
                oid: Some("a".repeat(40)),
                ..BranchState::default()
            },
            operation: None,
            entries,
            totals: StatusTotals::default(),
            truncated: false,
        }
    }

    fn with_status(status: GitStatus) -> WorkspaceGit {
        WorkspaceGit {
            status: Some(Ok(status)),
            log: None,
            degraded: None,
        }
    }

    fn ready(git: &WorkspaceGit) -> Panel {
        match panel_view(Some(git)) {
            PanelView::Ready(panel) => *panel,
            other => panic!("expected a ready panel, got {other:?}"),
        }
    }

    #[test]
    fn nothing_answered_yet_is_loading_and_a_folder_is_not_a_repository() {
        assert_eq!(panel_view(None), PanelView::Loading);
        let loading = WorkspaceGit::default();
        assert_eq!(panel_view(Some(&loading)), PanelView::Loading);
        let folder = WorkspaceGit {
            status: Some(Err(Refusal::Failed {
                code: codes::GIT_NOT_A_REPOSITORY.into(),
                message: "not a repository".into(),
            })),
            ..WorkspaceGit::default()
        };
        assert_eq!(panel_view(Some(&folder)), PanelView::NotRepository);
    }

    #[test]
    fn other_status_failures_keep_the_service_text() {
        let failed = WorkspaceGit {
            status: Some(Err(Refusal::Failed {
                code: codes::GIT_FAILED.into(),
                message: "git could not read the index".into(),
            })),
            ..WorkspaceGit::default()
        };
        assert_eq!(
            panel_view(Some(&failed)),
            PanelView::Failed {
                message: "git could not read the index".into()
            }
        );
    }

    #[test]
    fn a_partly_staged_file_is_listed_as_staged_and_as_changed() {
        let mut both = entry("src/lib/main.rs");
        both.staged = Some(ChangeKind::Modified);
        both.staged_lines = Some(LineCounts {
            added: 3,
            removed: 1,
            binary: false,
        });
        both.unstaged = Some(ChangeKind::Modified);
        both.unstaged_lines = Some(LineCounts {
            added: 2,
            removed: 0,
            binary: false,
        });
        let panel = ready(&with_status(status(vec![both])));
        assert_eq!(panel.staged.len(), 1);
        assert_eq!(panel.changes.len(), 1);
        assert_eq!(panel.staged[0].lines, Some((3, 1)));
        assert_eq!(panel.changes[0].lines, Some((2, 0)));
        assert_eq!(panel.staged[0].directory, "src/lib");
        assert_eq!(panel.staged[0].name, "main.rs");
        assert!(panel.untracked.is_empty() && panel.conflicted.is_empty());
    }

    #[test]
    fn untracked_and_conflicted_entries_have_their_own_groups() {
        let mut untracked = entry("notes.md");
        untracked.untracked = true;
        let mut conflicted = entry("Cargo.lock");
        conflicted.conflict = Some(ConflictKind::BothModified);
        let panel = ready(&with_status(status(vec![untracked, conflicted])));
        assert_eq!(panel.untracked[0].word, "Untracked");
        assert_eq!(panel.untracked[0].directory, "");
        assert_eq!(panel.conflicted[0].letter, "U");
        assert!(panel.staged.is_empty() && panel.changes.is_empty());
        assert!(!panel.is_clean());
    }

    #[test]
    fn a_status_without_entries_is_clean_and_says_so() {
        let panel = ready(&with_status(status(Vec::new())));
        assert!(panel.is_clean());
    }

    #[test]
    fn a_rename_keeps_its_old_path_and_a_binary_side_counts_no_lines() {
        let mut renamed = entry("docs/new.png");
        renamed.staged = Some(ChangeKind::Renamed);
        renamed.original_path = Some("docs/old.png".into());
        renamed.staged_lines = Some(LineCounts {
            added: 0,
            removed: 0,
            binary: true,
        });
        let panel = ready(&with_status(status(vec![renamed])));
        let row = &panel.staged[0];
        assert_eq!(row.original_path.as_deref(), Some("docs/old.png"));
        assert_eq!((row.letter, row.word), ("R", "Renamed"));
        assert!(row.binary);
    }

    #[test]
    fn a_detached_head_and_the_upstream_counts_are_on_the_branch_line() {
        let mut detached = status(Vec::new());
        detached.branch = BranchState {
            head: None,
            oid: Some("b".repeat(40)),
            upstream: Some("origin/main".into()),
            ahead: Some(2),
            behind: Some(5),
            upstream_gone: false,
        };
        let panel = ready(&with_status(detached));
        assert!(panel.branch.detached);
        assert_eq!(panel.branch.name, "Detached HEAD");
        assert_eq!((panel.branch.ahead, panel.branch.behind), (2, 5));
        assert_eq!(panel.branch.upstream.as_deref(), Some("origin/main"));
        assert!(!panel.branch.unborn);
    }

    #[test]
    fn an_operation_in_progress_is_named_and_an_unborn_branch_says_so() {
        let mut merging = status(Vec::new());
        merging.operation = Some(GitOperation::Merge);
        merging.branch.oid = None;
        let panel = ready(&with_status(merging));
        assert_eq!(panel.operation, Some("Merge in progress"));
        assert!(panel.branch.unborn);
    }

    #[test]
    fn a_truncated_status_and_a_degraded_watch_are_carried_over() {
        let mut long = status(Vec::new());
        long.truncated = true;
        let git = WorkspaceGit {
            status: Some(Ok(long)),
            log: None,
            degraded: Some("The file system cannot be watched.".into()),
        };
        let panel = ready(&git);
        assert!(panel.truncated);
        assert_eq!(
            panel.degraded.as_deref(),
            Some("The file system cannot be watched.")
        );
    }

    #[test]
    fn history_waits_for_its_answer_then_lists_commits_with_more_pages_flagged() {
        let commit = Commit {
            id: "0123456789abcdef".into(),
            parents: Vec::new(),
            author: Signature {
                name: "Ada".into(),
                email: "ada@example.invalid".into(),
                time_ms: 1,
            },
            committer: Signature {
                name: "Ada".into(),
                email: "ada@example.invalid".into(),
                time_ms: 1,
            },
            subject: "first".into(),
            message: "first\n".into(),
            message_truncated: false,
        };
        let mut git = with_status(status(Vec::new()));
        assert_eq!(ready(&git).history, History::Loading);
        git.log = Some(Ok(GitLog {
            commits: vec![commit],
            next_cursor: Some("next".into()),
        }));
        assert_eq!(
            ready(&git).history,
            History::Loaded {
                commits: vec![CommitRow {
                    id: "0123456789abcdef".into(),
                    short_id: "0123456".into(),
                    subject: "first".into(),
                    author: "Ada".into(),
                }],
                more: true,
            }
        );
        git.log = Some(Err(Refusal::unavailable("no history")));
        assert_eq!(
            ready(&git).history,
            History::Failed {
                message: "no history".into()
            }
        );
    }

    #[test]
    fn every_change_kind_has_a_letter_and_a_word() {
        assert_eq!(change_label(ChangeKind::Added), ("A", "Added"));
        assert_eq!(change_label(ChangeKind::Deleted), ("D", "Deleted"));
        assert_eq!(change_label(ChangeKind::TypeChanged), ("T", "Type changed"));
        assert_eq!(change_label(ChangeKind::Unknown), ("?", "Changed"));
    }
}
