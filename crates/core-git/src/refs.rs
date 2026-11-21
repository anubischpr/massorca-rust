//! Branch and ref queries. Yes/no answers come from git's exit status. The one
//! place that reads git's message text is `delete_branch_if_merged`.

use std::path::Path;

use crate::error::GitError;
use crate::git::{Captured, Git};

impl Git {
    /// Whether the local branch `branch` exists in `repo`.
    pub fn branch_exists(&self, repo: &Path, branch: &str) -> Result<bool, GitError> {
        let reference = format!("refs/heads/{branch}");
        let captured = self.run(
            Some(repo),
            ["show-ref", "--verify", "--quiet", reference.as_str()],
        )?;
        exit_code_answer(&captured)
    }

    /// Whether any remote-tracking ref `refs/remotes/<remote>/<branch>` exists.
    pub fn remote_branch_exists(&self, repo: &Path, branch: &str) -> Result<bool, GitError> {
        let captured = self.run(
            Some(repo),
            ["for-each-ref", "--format=%(refname)", "refs/remotes/"],
        )?;
        captured.require_success()?;
        Ok(captured
            .text()?
            .lines()
            .any(|reference| remote_branch_of(reference) == Some(branch)))
    }

    /// Whether `rev` resolves to a commit in `repo`.
    pub fn ref_exists(&self, repo: &Path, rev: &str) -> Result<bool, GitError> {
        // No ref starts with a dash, and a leading one would be read as an option.
        if rev.starts_with('-') {
            return Ok(false);
        }
        let commit = format!("{rev}^{{commit}}");
        let captured = self.run(
            Some(repo),
            ["rev-parse", "--verify", "--quiet", commit.as_str()],
        )?;
        exit_code_answer(&captured)
    }

    /// Deletes `branch` with `git branch -d`. Returns `false` when git refuses because
    /// the branch is not fully merged; any other refusal is an error.
    pub fn delete_branch_if_merged(&self, repo: &Path, branch: &str) -> Result<bool, GitError> {
        let captured = self.run_mutation(Some(repo), ["branch", "-d", "--", branch])?;
        if captured.succeeded() {
            return Ok(true);
        }
        // Git exits 1 for this refusal and for other failures alike; only the
        // message tells them apart, and LC_ALL=C keeps it in English.
        if captured.stderr_contains("is not fully merged") {
            return Ok(false);
        }
        Err(captured.failure())
    }

    /// Whether git accepts `name` as a branch name.
    pub fn check_branch_name(&self, repo: &Path, name: &str) -> Result<bool, GitError> {
        // These never pass as branch names. Checking them here keeps git from reading a
        // dash as an option and from expanding `@{-1}` to the previous branch.
        if name.is_empty() || name.starts_with('-') || name.contains("@{") {
            return Ok(false);
        }
        let captured = self.run(Some(repo), ["check-ref-format", "--branch", name])?;
        match captured.exit_code() {
            Some(0) => Ok(true),
            // Under `--branch`, git exits 128 for an invalid name; 1 is the plain-refname answer.
            Some(1 | 128) => Ok(false),
            _ => Err(captured.failure()),
        }
    }
}

/// The branch part of `refs/remotes/<remote>/<branch>`. The remote is the first
/// path segment, which matches how git names remotes in practice.
fn remote_branch_of(reference: &str) -> Option<&str> {
    let rest = reference.strip_prefix("refs/remotes/")?;
    let (remote, branch) = rest.split_once('/')?;
    (!remote.is_empty()).then_some(branch)
}

/// Maps a yes/no query's exit status: 0 is yes, 1 is no, anything else is a failure.
fn exit_code_answer(captured: &Captured) -> Result<bool, GitError> {
    match captured.exit_code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(captured.failure()),
    }
}

#[cfg(test)]
mod tests {
    use super::remote_branch_of;

    #[test]
    fn the_branch_is_everything_after_the_remote_segment() {
        assert_eq!(
            remote_branch_of("refs/remotes/origin/feature/x"),
            Some("feature/x")
        );
        assert_eq!(remote_branch_of("refs/remotes/origin/main"), Some("main"));
        assert_eq!(remote_branch_of("refs/remotes/origin"), None);
        assert_eq!(remote_branch_of("refs/heads/main"), None);
        assert_eq!(remote_branch_of("refs/remotes//x"), None);
    }
}
