//! `git.log`: history in pages pinned to the commit the first page resolved (decision 0011).

use serde::{Deserialize, Serialize};

use super::is_false;

/// Params: [`GitLogParams`]. Result: [`GitLog`].
pub const LOG: &str = "git.log";

pub const DEFAULT_LOG_LIMIT: u32 = 50;
pub const MAX_LOG_LIMIT: u32 = 500;
/// Message bytes kept per commit; a page of the largest size stays far below a frame.
pub const MAX_MESSAGE_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitLogParams {
    pub workspace: String,
    /// Where history starts; `None` means HEAD. Ignored when `cursor` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Commits per page; `None` means [`DEFAULT_LOG_LIMIT`]. Clamped to [`MAX_LOG_LIMIT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// `next_cursor` of the previous page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitLog {
    /// Newest first, in git's default order.
    pub commits: Vec<Commit>,
    /// Pass it back to read the next page; absent on the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub id: String,
    pub parents: Vec<String>,
    pub author: Signature,
    pub committer: Signature,
    /// The first line of the message.
    pub subject: String,
    /// The whole message, cut at [`MAX_MESSAGE_BYTES`].
    pub message: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub message_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub name: String,
    pub email: String,
    /// Unix milliseconds.
    pub time_ms: i64,
}
