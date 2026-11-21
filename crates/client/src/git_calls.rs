//! Typed helpers over the `git/1` methods (decision 0010).

use massorca_protocol::methods::git::diff::{
    self, GitDiff, GitDiffFile, GitDiffFileParams, GitDiffParams,
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

    /// `git.diff_file`: one file's patch, capped in bytes.
    pub async fn git_diff_file(
        &self,
        params: &GitDiffFileParams,
    ) -> Result<GitDiffFile, ClientError> {
        self.call(diff::DIFF_FILE, params).await
    }
}
