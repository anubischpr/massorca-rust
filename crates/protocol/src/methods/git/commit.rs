//! `git.commit`: commit the staged set, or amend HEAD (decisions 0010 and 0013).

use serde::{Deserialize, Serialize};

use super::is_false;

/// Params: [`GitCommitParams`]. Result: [`GitCommitted`].
pub const COMMIT: &str = "git.commit";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCommitParams {
    pub workspace: String,
    /// The commit message. Required unless amending, where `None` keeps HEAD's message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Replace HEAD with a commit of HEAD's changes plus the staged ones.
    #[serde(default, skip_serializing_if = "is_false")]
    pub amend: bool,
    /// Waives the refusal to amend a commit some remote-tracking branch already contains.
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_published: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCommitted {
    pub id: String,
    /// The branch the commit is on; `None` when HEAD is detached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub subject: String,
    /// The commit the amend replaced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced: Option<String>,
}
