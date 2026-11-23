//! Reads one git configuration value through the git runner.

use std::path::Path;

use crate::error::GitError;
use crate::git::Git;

impl Git {
    /// The value `key` holds in `repo`'s configuration, or `None` when the key is unset. Only git's
    /// exit status 1 means unset; any other failure is an error.
    pub fn config_get(&self, repo: &Path, key: &str) -> Result<Option<String>, GitError> {
        let captured = self.run(Some(repo), ["config", "--get", key])?;
        match captured.exit_code() {
            Some(0) => {
                let text = captured.text()?;
                Ok(Some(text.strip_suffix('\n').unwrap_or(text).to_owned()))
            }
            Some(1) => Ok(None),
            _ => Err(captured.failure()),
        }
    }
}
