//! The git state of a workspace, read once: the branch, how far it is from its upstream, and the changed files
//! (decision 0080, screen M-DF-02).

use massorca_protocol::methods::git::status::{GitStatus, GitStatusParams, StatusEntry};

use crate::error::FfiError;
use crate::massorca::{Host, run_on};
use crate::wire::word;

/// The branch and the changed files of a workspace, as the host reports them now.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GitStatusSummary {
    /// The checked-out branch; absent when HEAD is detached.
    pub branch: Option<String>,
    /// Commits on the branch that its upstream lacks; absent without a reachable upstream.
    pub ahead: Option<u64>,
    /// Commits on the upstream that the branch lacks; absent without a reachable upstream.
    pub behind: Option<u64>,
    /// A merge, rebase or other operation that stopped midway, such as `merge`.
    pub operation: Option<String>,
    /// The changed files, sorted by path, as many as the host lists.
    pub files: Vec<GitFileStatus>,
    /// More changed files exist than `files` lists.
    pub truncated: bool,
}

/// One changed file. Each change reads as its word, such as `modified`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GitFileStatus {
    pub path: String,
    /// The path before a rename or copy.
    pub original_path: Option<String>,
    /// The change staged in the index.
    pub staged: Option<String>,
    /// The change in the working tree that is not staged.
    pub unstaged: Option<String>,
    /// The file is new and not tracked.
    pub untracked: bool,
    /// The kind of conflict (`both_modified`, ...), for a file git could not merge.
    pub conflict: Option<String>,
}

impl From<GitStatus> for GitStatusSummary {
    fn from(status: GitStatus) -> Self {
        Self {
            branch: status.branch.head,
            ahead: status.branch.ahead,
            behind: status.branch.behind,
            operation: status.operation.map(word),
            files: status
                .entries
                .into_iter()
                .map(GitFileStatus::from)
                .collect(),
            truncated: status.truncated,
        }
    }
}

impl From<StatusEntry> for GitFileStatus {
    fn from(entry: StatusEntry) -> Self {
        Self {
            path: entry.path,
            original_path: entry.original_path,
            staged: entry.staged.map(word),
            unstaged: entry.unstaged.map(word),
            untracked: entry.untracked,
            conflict: entry.conflict.map(word),
        }
    }
}

#[uniffi::export]
impl Host {
    /// The branch and the changed files of `workspace` (its id, name or path), read now.
    pub async fn git_status(&self, workspace: String) -> Result<GitStatusSummary, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            let status = client
                .git_status(&GitStatusParams {
                    workspace,
                    limit: None,
                })
                .await
                .map_err(|error| FfiError::from_client(error, &name))?;
            Ok(GitStatusSummary::from(status))
        })
        .await
    }
}
