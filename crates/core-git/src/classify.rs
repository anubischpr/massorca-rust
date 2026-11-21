//! The one table that reads git's message text (decision 0010). Each pattern is checked against real git
//! output under `LC_ALL=C` by a test; text that matches none stays `git.failed`.

use massorca_protocol::codes;

use crate::error::GitError;

/// `(code, phrase)`: the first phrase found in stderr names the failure.
const PATTERNS: &[(&str, &str)] = &[
    (codes::GIT_INDEX_LOCKED, "index.lock': File exists"),
    (codes::GIT_IDENTITY_UNKNOWN, "Please tell me who you are"),
    (codes::GIT_IDENTITY_UNKNOWN, "empty ident name"),
];

/// The protocol code a failed git command maps to, when its message is one the table knows.
pub fn failure_code(error: &GitError) -> Option<&'static str> {
    let GitError::Failed { stderr, .. } = error else {
        return None;
    };
    PATTERNS
        .iter()
        .find(|(_, phrase)| stderr.contains(phrase))
        .map(|(code, _)| *code)
}
