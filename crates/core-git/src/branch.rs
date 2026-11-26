//! Local branches: list, create, switch, rename and delete (decisions 0010, 0013 and 0014). Branch names reach
//! git after `--`, as an option value, or as `refs/heads/<name>`, so a leading dash is never read as an option.

use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::git::branch::GitBranch;

use crate::classify::{failure_code, overwritten_paths};
use crate::error::GitError;
use crate::git::Git;

/// Why a branch command did not run, or a git failure while it ran.
#[derive(Debug, thiserror::Error)]
pub enum BranchError {
    /// A refusal: `code` names it.
    #[error("{message}")]
    Refused { code: &'static str, message: String },
    /// Git refused a checkout that would overwrite local changes or untracked files.
    #[error("{message}")]
    WouldOverwrite { message: String, paths: Vec<String> },
    #[error(transparent)]
    Git(#[from] GitError),
}

fn refused(code: &'static str, message: String) -> BranchError {
    BranchError::Refused { code, message }
}

impl Git {
    /// Every local branch, the one checked out in `dir` first, then by name.
    pub fn branches(&self, dir: &Path) -> Result<Vec<GitBranch>, GitError> {
        let captured = self.run(
            Some(dir),
            [
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00%(upstream:short)%00%(upstream:track)",
                "refs/heads",
            ],
        )?;
        captured.require_success()?;
        let current = self.current_branch(dir)?;
        let elsewhere: Vec<String> = self
            .worktree_list(dir)?
            .into_iter()
            .filter_map(|entry| entry.branch)
            .collect();
        let mut branches = Vec::new();
        for line in captured.text()?.lines() {
            let fields: Vec<&str> = line.split('\0').collect();
            let [refname, commit, upstream, track] = fields[..] else {
                return Err(captured.invalid_output(format!("unexpected branch line `{line}`")));
            };
            let Some(name) = refname.strip_prefix("refs/heads/") else {
                continue;
            };
            let current = current.as_deref() == Some(name);
            let mut branch = GitBranch {
                name: name.to_owned(),
                commit: commit.to_owned(),
                upstream: (!upstream.is_empty()).then(|| upstream.to_owned()),
                ahead: None,
                behind: None,
                upstream_gone: false,
                current,
                checked_out_elsewhere: !current && elsewhere.iter().any(|other| other == name),
            };
            if branch.upstream.is_some() {
                parse_track(track, &mut branch).ok_or_else(|| {
                    captured.invalid_output(format!("unexpected track `{track}`"))
                })?;
            }
            branches.push(branch);
        }
        branches.sort_by(|a, b| b.current.cmp(&a.current).then_with(|| a.name.cmp(&b.name)));
        Ok(branches)
    }

    /// Creates `name` at `start` (HEAD when `None`) with no upstream, and checks it out with `switch`.
    pub fn branch_create(
        &self,
        dir: &Path,
        name: &str,
        start: Option<&str>,
        switch: bool,
    ) -> Result<GitBranch, BranchError> {
        self.require_new_name(dir, name)?;
        let start = start.unwrap_or("HEAD");
        let Some(commit) = self.commit_of(dir, start)? else {
            return Err(refused(
                codes::GIT_REVISION_NOT_FOUND,
                format!("`{start}` does not name a commit"),
            ));
        };
        let captured = if switch {
            // `checkout -b` creates the branch only once the checkout succeeds.
            self.run_mutation(
                Some(dir),
                [
                    "checkout",
                    "-q",
                    "--no-track",
                    "-b",
                    name,
                    commit.as_str(),
                    "--",
                ],
            )?
        } else {
            self.run_mutation(
                Some(dir),
                ["branch", "--no-track", "--", name, commit.as_str()],
            )?
        };
        checkout_result(captured.require_success())?;
        self.branch_entry(dir, name)
    }

    /// Checks out the local branch `branch` in `dir`; git keeps local changes that do not conflict.
    pub fn branch_switch(&self, dir: &Path, branch: &str) -> Result<(), BranchError> {
        self.require_branch(dir, branch)?;
        let current = self.current_branch(dir)?;
        if current.as_deref() != Some(branch)
            && self
                .worktree_list(dir)?
                .iter()
                .any(|entry| entry.branch.as_deref() == Some(branch))
        {
            return Err(refused(
                codes::BRANCH_CHECKED_OUT,
                format!("branch `{branch}` is checked out in another worktree"),
            ));
        }
        let captured = self.run_mutation(Some(dir), ["checkout", "-q", branch, "--"])?;
        checkout_result(captured.require_success())
    }

    /// Renames `from` to `to`; never overwrites an existing branch.
    pub fn branch_rename(
        &self,
        dir: &Path,
        from: &str,
        to: &str,
    ) -> Result<GitBranch, BranchError> {
        self.require_branch(dir, from)?;
        self.require_new_name(dir, to)?;
        self.run_mutation(Some(dir), ["branch", "-m", "--", from, to])?
            .require_success()?;
        self.branch_entry(dir, to)
    }

    /// Deletes `branch` and returns the commit it pointed at. A branch checked out in any worktree, or named
    /// in `in_use`, is always refused; an unmerged one needs `force`.
    pub fn branch_delete(
        &self,
        dir: &Path,
        branch: &str,
        force: bool,
        in_use: &[String],
    ) -> Result<String, BranchError> {
        let reference = format!("refs/heads/{branch}");
        let Some(commit) = self.commit_of(dir, &reference)? else {
            return Err(refused(
                codes::BRANCH_NOT_FOUND,
                format!("branch `{branch}` does not exist"),
            ));
        };
        if self
            .worktree_list(dir)?
            .iter()
            .any(|entry| entry.branch.as_deref() == Some(branch))
        {
            return Err(refused(
                codes::BRANCH_CHECKED_OUT,
                format!("branch `{branch}` is checked out in a worktree"),
            ));
        }
        if in_use.iter().any(|held| held == branch) {
            return Err(refused(
                codes::BRANCH_IN_USE,
                format!(
                    "a workspace was created on branch `{branch}`; remove the workspace instead"
                ),
            ));
        }
        if force {
            self.delete_branch_at(dir, branch, &commit)?;
        } else if !self.delete_branch_if_merged(dir, branch)? {
            return Err(refused(
                codes::BRANCH_UNMERGED,
                format!("branch `{branch}` is not merged; pass force to delete it"),
            ));
        }
        Ok(commit)
    }

    /// Deletes `branch` only while it still points at `commit`, so a branch moved meanwhile is kept.
    pub fn delete_branch_at(&self, dir: &Path, branch: &str, commit: &str) -> Result<(), GitError> {
        let reference = format!("refs/heads/{branch}");
        self.run_mutation(Some(dir), ["update-ref", "-d", reference.as_str(), commit])?
            .require_success()
    }

    /// The short name of the branch checked out in `dir`; `None` when HEAD is detached.
    pub fn current_branch(&self, dir: &Path) -> Result<Option<String>, GitError> {
        let captured = self.run(Some(dir), ["symbolic-ref", "-q", "HEAD"])?;
        match captured.exit_code() {
            Some(0) => Ok(captured
                .text()?
                .trim_end()
                .strip_prefix("refs/heads/")
                .map(str::to_owned)),
            Some(1) => Ok(None),
            _ => Err(captured.failure()),
        }
    }

    /// The commit `rev` resolves to, or `None`; a leading dash resolves to nothing.
    fn commit_of(&self, dir: &Path, rev: &str) -> Result<Option<String>, GitError> {
        if rev.starts_with('-') {
            return Ok(None);
        }
        let commit = format!("{rev}^{{commit}}");
        let captured = self.run(
            Some(dir),
            ["rev-parse", "--verify", "--quiet", commit.as_str()],
        )?;
        match captured.exit_code() {
            Some(0) => Ok(Some(captured.text()?.trim_end().to_owned())),
            Some(1) => Ok(None),
            _ => Err(captured.failure()),
        }
    }

    fn require_branch(&self, dir: &Path, branch: &str) -> Result<(), BranchError> {
        if self.branch_exists(dir, branch)? {
            Ok(())
        } else {
            Err(refused(
                codes::BRANCH_NOT_FOUND,
                format!("branch `{branch}` does not exist"),
            ))
        }
    }

    fn require_new_name(&self, dir: &Path, name: &str) -> Result<(), BranchError> {
        if !self.check_branch_name(dir, name)? {
            return Err(refused(
                codes::BRANCH_NAME_INVALID,
                format!("`{name}` is not a valid branch name"),
            ));
        }
        if self.branch_exists(dir, name)? {
            return Err(refused(
                codes::BRANCH_EXISTS,
                format!("branch `{name}` already exists"),
            ));
        }
        Ok(())
    }

    fn branch_entry(&self, dir: &Path, name: &str) -> Result<GitBranch, BranchError> {
        self.branches(dir)?
            .into_iter()
            .find(|branch| branch.name == name)
            .ok_or_else(|| {
                refused(
                    codes::BRANCH_NOT_FOUND,
                    format!("branch `{name}` does not exist"),
                )
            })
    }
}

/// Maps a failed checkout to `WouldOverwrite` when git refused to overwrite local files.
fn checkout_result(result: Result<(), GitError>) -> Result<(), BranchError> {
    match result {
        Err(error) if failure_code(&error) == Some(codes::GIT_CHECKOUT_WOULD_OVERWRITE) => {
            let paths = overwritten_paths(&error);
            Err(BranchError::WouldOverwrite {
                message: format!(
                    "the checkout would overwrite local changes: {}",
                    paths.join(", ")
                ),
                paths,
            })
        }
        other => Ok(other?),
    }
}

/// Fills ahead, behind and gone from `%(upstream:track)`: empty, `[gone]`, or `[ahead N, behind M]` in part.
fn parse_track(track: &str, branch: &mut GitBranch) -> Option<()> {
    if track == "[gone]" {
        branch.upstream_gone = true;
        return Some(());
    }
    let (mut ahead, mut behind) = (0, 0);
    if !track.is_empty() {
        let inner = track.strip_prefix('[')?.strip_suffix(']')?;
        for part in inner.split(", ") {
            let (word, count) = part.split_once(' ')?;
            let count = count.parse().ok()?;
            match word {
                "ahead" => ahead = count,
                "behind" => behind = count,
                _ => return None,
            }
        }
    }
    branch.ahead = Some(ahead);
    branch.behind = Some(behind);
    Some(())
}
