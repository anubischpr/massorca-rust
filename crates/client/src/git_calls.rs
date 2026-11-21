//! Typed helpers over the `git/1` methods (decision 0010).

use massorca_protocol::methods::git::diff::{
    self, GitDiff, GitDiffFile, GitDiffFileParams, GitDiffParams,
};
use massorca_protocol::methods::git::index::{
    self, GitDiscardParams, GitDiscarded, GitPathsParams,
};
use massorca_protocol::methods::git::log::{self, GitLog, GitLogParams};
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
}
