//! Typed helpers over the `git/1` methods (decision 0010).

use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::branch::{
    self, GitBranch, GitBranchCreateParams, GitBranchDeleteParams, GitBranchDeleted,
    GitBranchRenameParams, GitBranchSwitchParams, GitBranchSwitched, GitBranches,
};
use massorca_protocol::methods::git::commit::{self, GitCommitParams, GitCommitted};
use massorca_protocol::methods::git::diff::{
    self, GitDiff, GitDiffFile, GitDiffFileParams, GitDiffParams,
};
use massorca_protocol::methods::git::index::{
    self, GitDiscardParams, GitDiscarded, GitPathsParams,
};
use massorca_protocol::methods::git::log::{self, GitLog, GitLogParams};
use massorca_protocol::methods::git::remote::{
    self, GitAborted, GitFetchParams, GitFetched, GitPullParams, GitPulled, GitPushParams,
    GitPushed,
};
use massorca_protocol::methods::git::status::{self, GitStatus, GitStatusParams};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `git.status`: the changed files of a workspace and its branch state.
    pub async fn git_status(&self, params: &GitStatusParams) -> Result<GitStatus, ClientError> {
        self.call(status::STATUS, params).await
    }

    /// `git.diff`: the files a comparison changes, with line counts and no patch text.
    pub async fn git_diff(&self, params: &GitDiffParams) -> Result<GitDiff, ClientError> {
        self.call(diff::DIFF, params).await
    }

    /// `git.stage`: stages the named files, or every change.
    pub async fn git_stage(&self, params: &GitPathsParams) -> Result<(), ClientError> {
        self.call(index::STAGE, params).await
    }

    /// `git.unstage`: unstages the named files, or everything.
    pub async fn git_unstage(&self, params: &GitPathsParams) -> Result<(), ClientError> {
        self.call(index::UNSTAGE, params).await
    }

    /// `git.discard`: discards changes under the waivers it names; refusals list the files in `data.paths`.
    pub async fn git_discard(
        &self,
        params: &GitDiscardParams,
    ) -> Result<GitDiscarded, ClientError> {
        self.call(index::DISCARD, params).await
    }

    /// `git.commit`: commits the staged changes, or amends HEAD.
    pub async fn git_commit(&self, params: &GitCommitParams) -> Result<GitCommitted, ClientError> {
        self.call(commit::COMMIT, params).await
    }

    /// `git.fetch`: fetches with prune; reports the remote-tracking refs that changed.
    pub async fn git_fetch(&self, params: &GitFetchParams) -> Result<GitFetched, ClientError> {
        self.call(remote::FETCH, params).await
    }

    /// `git.pull`: brings the branch up to date under an explicit mode.
    pub async fn git_pull(&self, params: &GitPullParams) -> Result<GitPulled, ClientError> {
        self.call(remote::PULL, params).await
    }

    /// `git.push`: pushes the branch; a rejection is a result, not an error.
    pub async fn git_push(&self, params: &GitPushParams) -> Result<GitPushed, ClientError> {
        self.call(remote::PUSH, params).await
    }

    /// `git.abort`: aborts the merge, rebase, cherry-pick or revert in progress.
    pub async fn git_abort(&self, params: &GitWorkspaceRef) -> Result<GitAborted, ClientError> {
        self.call(remote::ABORT, params).await
    }

    /// `git.log`: one page of history; pass `next_cursor` back for the next page.
    pub async fn git_log(&self, params: &GitLogParams) -> Result<GitLog, ClientError> {
        self.call(log::LOG, params).await
    }

    /// `git.diff_file`: one file's patch, capped in bytes.
    pub async fn git_diff_file(
        &self,
        params: &GitDiffFileParams,
    ) -> Result<GitDiffFile, ClientError> {
        self.call(diff::DIFF_FILE, params).await
    }

    /// `git.branches`: the local branches, the current one first.
    pub async fn git_branches(&self, params: &GitWorkspaceRef) -> Result<GitBranches, ClientError> {
        self.call(branch::BRANCHES, params).await
    }

    /// `git.branch_create`: creates a branch with no upstream, and checks it out with `switch`.
    pub async fn git_branch_create(
        &self,
        params: &GitBranchCreateParams,
    ) -> Result<GitBranch, ClientError> {
        self.call(branch::BRANCH_CREATE, params).await
    }

    /// `git.branch_switch`: checks out a local branch; a conflicting change lists its files in `data.paths`.
    pub async fn git_branch_switch(
        &self,
        params: &GitBranchSwitchParams,
    ) -> Result<GitBranchSwitched, ClientError> {
        self.call(branch::BRANCH_SWITCH, params).await
    }

    /// `git.branch_rename`: renames a branch without overwriting another.
    pub async fn git_branch_rename(
        &self,
        params: &GitBranchRenameParams,
    ) -> Result<GitBranch, ClientError> {
        self.call(branch::BRANCH_RENAME, params).await
    }

    /// `git.branch_delete`: deletes a branch and returns the commit it pointed at.
    pub async fn git_branch_delete(
        &self,
        params: &GitBranchDeleteParams,
    ) -> Result<GitBranchDeleted, ClientError> {
        self.call(branch::BRANCH_DELETE, params).await
    }
}
