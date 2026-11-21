//! Typed helpers over the `git/1` methods (decision 0010).

use massorca_protocol::methods::git::status::{self, GitStatus, GitStatusParams};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `git.status`: the changed files of a workspace and its branch state.
    pub async fn git_status(&self, params: &GitStatusParams) -> Result<GitStatus, ClientError> {
        self.call(status::STATUS, params).await
    }
}
