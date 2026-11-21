//! The one table that reads git's message text (decision 0010). Each pattern is checked against real git
//! output under `LC_ALL=C` by a test; text that matches none stays `git.failed`.

use massorca_protocol::codes;

use crate::error::GitError;

/// `(code, phrase)`: the first phrase found in stderr names the failure.
const PATTERNS: &[(&str, &str)] = &[
    (codes::GIT_INDEX_LOCKED, "index.lock': File exists"),
    // Authentication first: git adds "Could not read from remote repository" to these as well.
    (codes::GIT_AUTH_REQUIRED, "terminal prompts disabled"),
    (codes::GIT_AUTH_REQUIRED, "could not read Username"),
    (codes::GIT_AUTH_REQUIRED, "could not read Password"),
    (codes::GIT_AUTH_REQUIRED, "Authentication failed"),
    (codes::GIT_AUTH_REQUIRED, "Permission denied (publickey"), // untested: needs a real remote
    (codes::GIT_HOST_KEY_UNKNOWN, "Host key verification failed"), // untested: needs a real remote
    (
        codes::GIT_REMOTE_NOT_FOUND,
        "does not appear to be a git repository",
    ),
    (codes::GIT_REMOTE_NOT_FOUND, "Repository not found"), // untested
    (codes::GIT_REMOTE_UNREACHABLE, "Could not resolve host"), // untested: needs a real remote
    (codes::GIT_REMOTE_UNREACHABLE, "Failed to connect to"),
    (codes::GIT_REMOTE_UNREACHABLE, "Connection refused"),
    (codes::GIT_REMOTE_UNREACHABLE, "Connection timed out"), // untested: needs a real remote
    (codes::GIT_REMOTE_UNREACHABLE, "Network is unreachable"), // untested: needs a real remote
    (codes::GIT_LOCAL_CHANGES, "would be overwritten by merge"), // untested
    (codes::GIT_LOCAL_CHANGES, "You have unstaged changes"), // untested
    (
        codes::GIT_LOCAL_CHANGES,
        "Your index contains uncommitted changes",
    ), // untested
    (codes::GIT_IDENTITY_UNKNOWN, "Please tell me who you are"),
    (codes::GIT_IDENTITY_UNKNOWN, "empty ident name"),
    // Both the local-changes and the untracked-files refusals end their heading this way.
    (
        codes::GIT_CHECKOUT_WOULD_OVERWRITE,
        "would be overwritten by checkout:",
    ),
];

/// The files a refused checkout names: the tab-indented lines under each "would be overwritten" heading, sorted.
pub(crate) fn overwritten_paths(error: &GitError) -> Vec<String> {
    let GitError::Failed { stderr, .. } = error else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    let mut listing = false;
    for line in stderr.lines() {
        if line.contains("would be overwritten by checkout:") {
            listing = true;
        } else if let Some(path) = line.strip_prefix('\t').filter(|_| listing) {
            paths.push(path.trim_end().to_owned());
        } else {
            listing = false;
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

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
