//! `git.commit`: commit the staged set or amend HEAD, with the refusals of decision 0013. Hooks always run.

use std::path::Path;
use std::time::Duration;

use massorca_protocol::codes;
use massorca_protocol::methods::git::commit::GitCommitted;

use crate::error::GitError;
use crate::git::{Git, path_from_git};

/// Hooks and signing run inside a commit; this bounds one that waits on something that never answers.
const COMMIT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// The message travels in a file in the git directory: an argument could exceed the Windows command-line limit.
const MESSAGE_FILE: &str = "MASSORCA_COMMIT_MSG";

#[derive(Debug, Clone, Copy)]
pub struct CommitRequest<'a> {
    pub message: Option<&'a str>,
    pub amend: bool,
    pub allow_published: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error("{message}")]
    Refused { code: &'static str, message: String },
    #[error(transparent)]
    Git(#[from] GitError),
}

impl Git {
    /// Commits what is staged, or amends HEAD, refusing what decision 0013 refuses.
    pub fn commit(
        &self,
        dir: &Path,
        request: CommitRequest<'_>,
    ) -> Result<GitCommitted, CommitError> {
        let status = self.status(dir, 0)?;
        if status.totals.conflicted > 0 {
            return refuse(
                codes::GIT_PATH_CONFLICTED,
                "resolve the unmerged files and stage them first",
            );
        }
        let message = request.message.filter(|text| !text.trim().is_empty());
        if request.message.is_some() && message.is_none() || message.is_none() && !request.amend {
            return refuse(
                codes::GIT_COMMIT_MESSAGE_EMPTY,
                "the commit message is empty",
            );
        }
        let replaced = if request.amend {
            let Some(head) = status.branch.oid.clone() else {
                return refuse(codes::GIT_NO_COMMITS, "there is no commit to amend yet");
            };
            if !request.allow_published {
                let remotes = self.remote_refs_containing(dir, &head)?;
                if !remotes.is_empty() {
                    return refuse(
                        codes::GIT_COMMIT_PUBLISHED,
                        format!(
                            "HEAD is already on {}; amending rewrites published history",
                            remotes.join(", ")
                        ),
                    );
                }
            }
            Some(head)
        } else {
            if status.totals.staged == 0 {
                return refuse(codes::GIT_NOTHING_TO_COMMIT, "nothing is staged");
            }
            None
        };
        let git_dir = self.absolute_git_dir(dir)?;
        let mut args: Vec<String> = vec!["commit".into(), "-q".into()];
        if request.amend {
            args.push("--amend".into());
        }
        let message_file = git_dir.join(MESSAGE_FILE);
        match message {
            Some(text) => {
                std::fs::write(&message_file, text).map_err(|source| GitError::File {
                    path: message_file.clone(),
                    source,
                })?;
                args.push("-F".into());
                args.push(message_file.to_string_lossy().into_owned());
            }
            None => args.push("--no-edit".into()),
        }
        let outcome = self.run_mutation_within(Some(dir), &args, COMMIT_TIMEOUT);
        let _ = std::fs::remove_file(&message_file);
        outcome?.require_success()?;
        Ok(GitCommitted {
            id: self.head_commit(dir)?,
            branch: self.current_branch(dir)?,
            subject: self.head_subject(dir)?,
            replaced,
        })
    }

    /// Remote-tracking refs that contain `commit`.
    fn remote_refs_containing(&self, dir: &Path, commit: &str) -> Result<Vec<String>, GitError> {
        let contains = format!("--contains={commit}");
        let captured = self.run(
            Some(dir),
            [
                "for-each-ref",
                "--format=%(refname:short)",
                contains.as_str(),
                "refs/remotes",
            ],
        )?;
        captured.require_success()?;
        Ok(captured.text()?.lines().map(str::to_owned).collect())
    }

    fn absolute_git_dir(&self, dir: &Path) -> Result<std::path::PathBuf, GitError> {
        let captured = self.run(Some(dir), ["rev-parse", "--absolute-git-dir"])?;
        captured.require_success()?;
        Ok(path_from_git(
            captured.text()?.trim_end_matches(['\r', '\n']),
        ))
    }

    fn head_commit(&self, dir: &Path) -> Result<String, GitError> {
        let captured = self.run(Some(dir), ["rev-parse", "--verify", "HEAD"])?;
        captured.require_success()?;
        Ok(captured.text()?.trim().to_owned())
    }

    /// The checked-out branch, or `None` when HEAD is detached.
    fn current_branch(&self, dir: &Path) -> Result<Option<String>, GitError> {
        let captured = self.run(Some(dir), ["symbolic-ref", "--short", "-q", "HEAD"])?;
        captured
            .succeeded()
            .then(|| captured.text().map(|text| text.trim().to_owned()))
            .transpose()
    }

    fn head_subject(&self, dir: &Path) -> Result<String, GitError> {
        let captured = self.run(
            Some(dir),
            [
                "log",
                "-1",
                "--no-show-signature",
                "--format=%s",
                "HEAD",
                "--",
            ],
        )?;
        captured.require_success()?;
        Ok(captured.text()?.trim_end().to_owned())
    }
}

fn refuse<T>(code: &'static str, message: impl Into<String>) -> Result<T, CommitError> {
    Err(CommitError::Refused {
        code,
        message: message.into(),
    })
}
