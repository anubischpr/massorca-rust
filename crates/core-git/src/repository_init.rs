//! The first repository of a folder: `git init` and the empty first commit a worktree branches from (decision 0115).

use std::path::Path;

use crate::error::GitError;
use crate::git::Git;

/// Options that give the first commit a fixed identity, used only when git has none configured (decision 0115).
const FALLBACK_IDENTITY: [&str; 4] = [
    "-c",
    "user.name=MassOrca",
    "-c",
    "user.email=massorca@localhost",
];

/// The message of the empty first commit.
const FIRST_COMMIT_MESSAGE: &str = "Initial commit";

/// Parts of git's messages for a missing author identity; the runner sets `LC_ALL=C`, so they are English.
const IDENTITY_MISSING: [&str; 5] = [
    "Please tell me who you are",
    "unable to auto-detect email address",
    "empty ident name",
    "user.name",
    "user.email",
];

impl Git {
    /// Runs `git init` in `dir`, then makes the empty first commit. A `.git` this call created is removed again when a
    /// step fails, so a refused init leaves the folder as it was.
    pub fn init_repository(&self, dir: &Path) -> Result<(), GitError> {
        let existed = dir.join(".git").symlink_metadata().is_ok();
        let result = self
            .run_mutation(Some(dir), ["init", "-q"])
            .and_then(|captured| captured.require_success())
            .and_then(|()| self.commit_first(dir));
        if result.is_err() && !existed {
            discard_new_repository(dir);
        }
        result
    }

    /// The empty first commit, with the configured identity or, when git has none, the fallback identity.
    fn commit_first(&self, dir: &Path) -> Result<(), GitError> {
        with_identity_fallback(|identity| {
            let mut args = identity.to_vec();
            args.extend(["commit", "--allow-empty", "-q", "-m", FIRST_COMMIT_MESSAGE]);
            self.run_mutation(Some(dir), args)?.require_success()
        })
    }
}

/// Removes the `.git` directory of `dir`, a repository this process made and a later step refused. Git writes some
/// object files read-only, which Windows will not delete, so their flag is cleared first.
pub fn discard_new_repository(dir: &Path) {
    let git_dir = dir.join(".git");
    clear_read_only(&git_dir);
    let _ = std::fs::remove_dir_all(&git_dir);
}

/// Runs `attempt` without extra options, then once more with the fallback identity when git reports no identity.
fn with_identity_fallback<F>(mut attempt: F) -> Result<(), GitError>
where
    F: FnMut(&[&str]) -> Result<(), GitError>,
{
    match attempt(&[]) {
        Err(error) if identity_missing(&error) => attempt(&FALLBACK_IDENTITY),
        other => other,
    }
}

/// Whether `error` is git refusing a commit for want of an author identity.
fn identity_missing(error: &GitError) -> bool {
    match error {
        GitError::Failed { stderr, .. } => {
            IDENTITY_MISSING.iter().any(|part| stderr.contains(*part))
        }
        _ => false,
    }
}

/// Clears the read-only flag of `path` and of what it holds, so that Windows can delete it.
#[cfg(windows)]
#[allow(
    clippy::permissions_set_readonly_false,
    reason = "Windows only: the flag is the only thing that blocks deleting git's object files; the Unix mode concern does not apply"
)]
fn clear_read_only(path: &Path) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.is_dir()
        && let Ok(entries) = std::fs::read_dir(path)
    {
        for entry in entries.flatten() {
            clear_read_only(&entry.path());
        }
    }
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

#[cfg(not(windows))]
fn clear_read_only(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::{
        FALLBACK_IDENTITY, discard_new_repository, identity_missing, with_identity_fallback,
    };
    use crate::error::GitError;

    const IDENTITY_ERROR: &str = "Author identity unknown\n\n*** Please tell me who you are.";

    fn failed(stderr: &str) -> GitError {
        GitError::Failed {
            args: vec!["commit".to_owned()],
            code: Some(128),
            stderr: stderr.to_owned(),
        }
    }

    fn owned(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| part.to_string()).collect()
    }

    #[test]
    fn a_commit_that_works_is_tried_once_without_the_fallback_identity() {
        let mut attempts: Vec<Vec<String>> = Vec::new();

        let result = with_identity_fallback(|identity| {
            attempts.push(owned(identity));
            Ok(())
        });

        assert!(result.is_ok());
        assert_eq!(attempts, vec![Vec::<String>::new()]);
    }

    #[test]
    fn a_missing_identity_is_retried_once_with_the_fallback_identity() {
        let mut attempts: Vec<Vec<String>> = Vec::new();

        let result = with_identity_fallback(|identity| {
            attempts.push(owned(identity));
            if attempts.len() == 1 {
                Err(failed(IDENTITY_ERROR))
            } else {
                Ok(())
            }
        });

        assert!(
            result.is_ok(),
            "the retry with the fallback identity succeeds"
        );
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[1], owned(&FALLBACK_IDENTITY));
    }

    #[test]
    fn a_failure_that_is_not_about_identity_is_returned_without_a_retry() {
        let mut attempts = 0;

        let result = with_identity_fallback(|_| {
            attempts += 1;
            Err(failed("cannot lock ref 'HEAD'"))
        });

        assert_eq!(attempts, 1);
        match result {
            Err(GitError::Failed { stderr, .. }) => assert_eq!(stderr, "cannot lock ref 'HEAD'"),
            other => panic!("expected the first failure, got {other:?}"),
        }
    }

    #[test]
    fn a_failing_retry_is_the_error_reported() {
        let mut attempts = 0;

        let result = with_identity_fallback(|_| {
            attempts += 1;
            Err(failed(IDENTITY_ERROR))
        });

        assert_eq!(attempts, 2);
        assert!(result.is_err());
    }

    #[test]
    fn identity_messages_from_git_are_recognised_and_other_messages_are_not() {
        for message in [
            "*** Please tell me who you are.",
            "fatal: unable to auto-detect email address (got 'user@host.(none)')",
            "fatal: empty ident name (for <>) not allowed",
            "Run  git config --global user.email \"you@example.com\"",
        ] {
            assert!(identity_missing(&failed(message)), "{message}");
        }
        assert!(!identity_missing(&failed("fatal: bad object HEAD")));
    }

    #[test]
    fn discarding_a_new_repository_removes_its_git_directory_and_nothing_else() {
        let dir = tempfile::tempdir().expect("temp dir");
        let objects = dir.path().join(".git").join("objects").join("ab");
        std::fs::create_dir_all(&objects).expect("create objects");
        let object = objects.join("cdef");
        std::fs::write(&object, "object").expect("write object");
        let mut permissions = std::fs::metadata(&object).expect("metadata").permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&object, permissions).expect("make the object read-only");
        std::fs::write(dir.path().join("notes.txt"), "keep me").expect("write notes");

        discard_new_repository(dir.path());

        assert!(!dir.path().join(".git").exists());
        assert!(dir.path().join("notes.txt").is_file());
    }
}
