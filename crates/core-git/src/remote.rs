//! `git.fetch`, `git.pull`, `git.push` and `git.abort` (decisions 0013 and 0014). Nothing here prompts: the runner
//! empties every askpass, and SSH runs in batch mode unless the user configured their own command.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use massorca_protocol::codes;
use massorca_protocol::methods::git::remote::{
    GitAborted, GitFetched, GitPulled, GitPushed, PullMode, PushOutcome, RefChange,
};
use massorca_protocol::methods::git::status::GitOperation;

use crate::error::GitError;
use crate::git::Git;

const BATCH_SSH: &str = "ssh -o BatchMode=yes";

/// A refusal before any network or repository change, or a git failure.
#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("{message}")]
    Refused { code: &'static str, message: String },
    #[error(transparent)]
    Git(#[from] GitError),
}

#[derive(Debug, Clone, Copy)]
pub enum FetchTarget<'a> {
    /// The current branch's remote, else `origin`.
    Default,
    Remote(&'a str),
    All,
}

impl Git {
    /// Fetches with prune and reports the remote-tracking refs that changed.
    pub fn fetch(
        &self,
        dir: &Path,
        target: FetchTarget<'_>,
        timeout: Duration,
    ) -> Result<GitFetched, RemoteError> {
        let remote = match target {
            FetchTarget::Remote(name) => Some(self.known_remote(dir, name)?),
            FetchTarget::Default => Some(self.default_remote(dir)?),
            FetchTarget::All => None,
        };
        let before = self.remote_refs(dir)?;
        let mut args = vec!["fetch", "--prune", "--no-recurse-submodules"];
        match remote.as_deref() {
            Some(name) => args.extend(["--", name]),
            None => args.push("--all"),
        }
        self.network(dir, &args, timeout)?.require_success()?;
        let after = self.remote_refs(dir)?;
        Ok(GitFetched {
            updated: changes(&before, &after),
        })
    }

    /// Fetches the branch a remote-tracking base names (`refs/remotes/<remote>/<branch>`) and reports the ref it moved.
    /// `None`, with no network run, when `base` is not one. Nothing is pruned.
    pub fn fetch_tracking_base(
        &self,
        dir: &Path,
        base: &str,
        timeout: Duration,
    ) -> Result<Option<GitFetched>, RemoteError> {
        let Some((remote, branch)) = self.tracking_branch_of(dir, base)? else {
            return Ok(None);
        };
        let before = self.remote_refs(dir)?;
        let refspec = format!("+refs/heads/{branch}:refs/remotes/{remote}/{branch}");
        let args = [
            "fetch",
            "--no-tags",
            "--no-recurse-submodules",
            "--",
            remote.as_str(),
            refspec.as_str(),
        ];
        let result = self
            .network(dir, &args, timeout)
            .and_then(|captured| captured.require_success());
        if let Err(error) = &result {
            tracing::warn!(%base, %error, "could not refresh the base from its remote");
        }
        result?;
        let after = self.remote_refs(dir)?;
        Ok(Some(GitFetched {
            updated: changes(&before, &after),
        }))
    }

    /// Brings the current branch up to date with its upstream under `mode`; `ff_only` never merges.
    pub fn pull(
        &self,
        dir: &Path,
        mode: PullMode,
        timeout: Duration,
    ) -> Result<GitPulled, RemoteError> {
        let branch = self.branch_name(dir)?;
        let Some((remote, upstream)) = self.upstream_of(dir, &branch)? else {
            return refuse(
                codes::GIT_NO_UPSTREAM,
                format!("`{branch}` has no upstream to pull from"),
            );
        };
        self.network(
            dir,
            &["fetch", "--no-recurse-submodules", "--", &remote],
            timeout,
        )?
        .require_success()?;
        let head = self.rev(dir, "HEAD")?;
        let theirs = self.rev(dir, &upstream)?;
        let (ahead, behind) = self.ahead_behind(dir, &head, &theirs)?;
        if behind == 0 {
            return Ok(GitPulled::UpToDate);
        }
        if ahead == 0 {
            self.run_mutation(Some(dir), ["merge", "--ff-only", "-q", theirs.as_str()])?
                .require_success()?;
            return Ok(GitPulled::FastForwarded {
                from: head,
                to: theirs,
            });
        }
        let outcome = match mode {
            PullMode::Merge => {
                self.run_mutation(Some(dir), ["merge", "--no-edit", "-q", theirs.as_str()])?
            }
            PullMode::Rebase => self.run_mutation(Some(dir), ["rebase", "-q", theirs.as_str()])?,
            PullMode::FfOnly | PullMode::Unknown => {
                return Ok(GitPulled::Diverged { ahead, behind });
            }
        };
        if !outcome.succeeded() {
            let status = self.status(dir, usize::MAX)?;
            if status.operation.is_some() {
                let paths = status
                    .entries
                    .into_iter()
                    .filter(|e| e.conflict.is_some())
                    .map(|e| e.path)
                    .collect();
                return Ok(GitPulled::Conflicts { paths });
            }
            return Err(outcome.failure().into());
        }
        let now = self.rev(dir, "HEAD")?;
        Ok(match mode {
            PullMode::Rebase => GitPulled::Rebased { head: now },
            _ => GitPulled::Merged { commit: now },
        })
    }

    /// Pushes the current branch to its upstream, or creates one on `remote` (default `origin`). Never forces;
    /// `force_with_lease` overwrites only a remote branch that still points at that commit.
    pub fn push(
        &self,
        dir: &Path,
        remote: Option<&str>,
        force_with_lease: Option<&str>,
        timeout: Duration,
    ) -> Result<GitPushed, RemoteError> {
        let branch = self.branch_name(dir)?;
        let (remote, remote_branch, set_upstream) = match self.upstream_of(dir, &branch)? {
            Some((remote, upstream)) => {
                let name = upstream
                    .strip_prefix(&format!("{remote}/"))
                    .unwrap_or(&branch)
                    .to_owned();
                (remote, name, false)
            }
            None => (
                self.known_remote(dir, remote.unwrap_or("origin"))?,
                branch.clone(),
                true,
            ),
        };
        let refspec = format!("refs/heads/{branch}:refs/heads/{remote_branch}");
        let mut args = vec!["push".to_owned(), "--porcelain".to_owned()];
        if let Some(expected) = force_with_lease {
            if expected.starts_with('-') || expected.is_empty() {
                return refuse(
                    codes::GIT_REVISION_NOT_FOUND,
                    "the lease must name a commit",
                );
            }
            args.push(format!(
                "--force-with-lease=refs/heads/{remote_branch}:{expected}"
            ));
        }
        if set_upstream {
            args.push("--set-upstream".to_owned());
        }
        args.extend(["--".to_owned(), remote.clone(), refspec]);
        let captured = self.network(dir, &args, timeout)?;
        let outcome = parse_push(captured.text()?);
        match outcome {
            Some(outcome) => Ok(GitPushed {
                remote_branch: format!("{remote}/{remote_branch}"),
                upstream_set: set_upstream && captured.succeeded(),
                outcome,
            }),
            None => Err(captured.failure().into()),
        }
    }

    /// Aborts the merge, rebase, cherry-pick or revert in progress.
    pub fn abort(&self, dir: &Path) -> Result<GitAborted, RemoteError> {
        let status = self.status(dir, 0)?;
        let (command, operation) = match status.operation {
            Some(GitOperation::Merge) => ("merge", GitOperation::Merge),
            Some(GitOperation::Rebase) => ("rebase", GitOperation::Rebase),
            Some(GitOperation::CherryPick) => ("cherry-pick", GitOperation::CherryPick),
            Some(GitOperation::Revert) => ("revert", GitOperation::Revert),
            Some(GitOperation::ApplyMailbox) => ("am", GitOperation::ApplyMailbox),
            _ => return refuse(codes::GIT_NO_OPERATION, "nothing is in progress"),
        };
        self.run_mutation(Some(dir), [command, "--abort"])?
            .require_success()?;
        Ok(GitAborted { operation })
    }

    /// Runs a network command with SSH in batch mode, unless the user set an SSH command of their own.
    fn network<S: AsRef<str>>(
        &self,
        dir: &Path,
        args: &[S],
        timeout: Duration,
    ) -> Result<crate::git::Captured, GitError> {
        let user_ssh = std::env::var_os("GIT_SSH_COMMAND").is_some()
            || std::env::var_os("GIT_SSH").is_some()
            || self.config_value(dir, "core.sshCommand")?.is_some();
        let env: &[(&str, &str)] = if user_ssh {
            &[]
        } else {
            &[("GIT_SSH_COMMAND", BATCH_SSH)]
        };
        self.run_network(Some(dir), args.iter().map(AsRef::as_ref), timeout, env)
    }

    fn config_value(&self, dir: &Path, key: &str) -> Result<Option<String>, GitError> {
        let captured = self.run(Some(dir), ["config", "--get", key])?;
        captured
            .succeeded()
            .then(|| captured.text().map(|text| text.trim().to_owned()))
            .transpose()
    }

    fn branch_name(&self, dir: &Path) -> Result<String, RemoteError> {
        let captured = self.run(Some(dir), ["symbolic-ref", "--short", "-q", "HEAD"])?;
        if !captured.succeeded() {
            return refuse(codes::GIT_DETACHED_HEAD, "HEAD is not on a branch");
        }
        Ok(captured.text()?.trim().to_owned())
    }

    /// The upstream's remote and short name (`origin`, `origin/main`), when the branch has one that resolves.
    fn upstream_of(&self, dir: &Path, branch: &str) -> Result<Option<(String, String)>, GitError> {
        let remote = self.config_value(dir, &format!("branch.{branch}.remote"))?;
        let upstream = format!("{branch}@{{upstream}}");
        let captured = self.run(
            Some(dir),
            [
                "rev-parse",
                "--abbrev-ref",
                "--verify",
                "-q",
                upstream.as_str(),
            ],
        )?;
        if !captured.succeeded() {
            return Ok(None);
        }
        let short = captured.text()?.trim().to_owned();
        Ok(remote.map(|remote| (remote, short)))
    }

    /// The remote and branch of a `refs/remotes/<remote>/<branch>` revision; remote names may hold `/`, so the
    /// longest configured name that prefixes it wins.
    fn tracking_branch_of(
        &self,
        dir: &Path,
        revision: &str,
    ) -> Result<Option<(String, String)>, GitError> {
        let Some(tail) = revision.strip_prefix("refs/remotes/") else {
            return Ok(None);
        };
        let captured = self.run(Some(dir), ["remote"])?;
        captured.require_success()?;
        let remote = captured
            .text()?
            .lines()
            .filter(|name| {
                tail.strip_prefix(*name)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .is_some_and(|branch| !branch.is_empty() && branch != "HEAD")
            })
            .max_by_key(|name| name.len());
        Ok(remote.map(|name| (name.to_owned(), tail[name.len() + 1..].to_owned())))
    }

    fn known_remote(&self, dir: &Path, name: &str) -> Result<String, RemoteError> {
        let captured = self.run(Some(dir), ["remote"])?;
        captured.require_success()?;
        if captured.text()?.lines().any(|line| line == name) {
            Ok(name.to_owned())
        } else {
            refuse(
                codes::GIT_REMOTE_NOT_FOUND,
                format!("no remote named `{name}`"),
            )
        }
    }

    fn default_remote(&self, dir: &Path) -> Result<String, RemoteError> {
        if let Ok(branch) = self.branch_name(dir)
            && let Some(remote) = self.config_value(dir, &format!("branch.{branch}.remote"))?
        {
            return Ok(remote);
        }
        self.known_remote(dir, "origin")
    }

    fn rev(&self, dir: &Path, revision: &str) -> Result<String, GitError> {
        let commit = format!("{revision}^{{commit}}");
        let captured = self.run(Some(dir), ["rev-parse", "--verify", commit.as_str()])?;
        captured.require_success()?;
        Ok(captured.text()?.trim().to_owned())
    }

    fn ahead_behind(&self, dir: &Path, ours: &str, theirs: &str) -> Result<(u64, u64), GitError> {
        let range = format!("{ours}...{theirs}");
        let captured = self.run(
            Some(dir),
            ["rev-list", "--left-right", "--count", range.as_str()],
        )?;
        captured.require_success()?;
        let text = captured.text()?;
        let mut counts = text.split_whitespace().map(str::parse::<u64>);
        match (counts.next(), counts.next()) {
            (Some(Ok(ahead)), Some(Ok(behind))) => Ok((ahead, behind)),
            _ => Err(captured.invalid_output("rev-list printed no counts")),
        }
    }

    fn remote_refs(&self, dir: &Path) -> Result<BTreeMap<String, String>, GitError> {
        let captured = self.run(
            Some(dir),
            [
                "for-each-ref",
                "--format=%(objectname) %(refname)",
                "refs/remotes",
            ],
        )?;
        captured.require_success()?;
        // `refs/remotes/<remote>/HEAD` is a pointer, not a branch; its short name would be just `<remote>`.
        Ok(captured
            .text()?
            .lines()
            .filter_map(|line| line.split_once(' '))
            .filter_map(|(id, name)| Some((name.strip_prefix("refs/remotes/")?, id)))
            .filter(|(name, _)| !name.ends_with("/HEAD"))
            .map(|(name, id)| (name.to_owned(), id.to_owned()))
            .collect())
    }
}

fn changes(before: &BTreeMap<String, String>, after: &BTreeMap<String, String>) -> Vec<RefChange> {
    let mut names: Vec<&String> = before.keys().chain(after.keys()).collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .filter(|name| before.get(*name) != after.get(*name))
        .map(|name| RefChange {
            name: name.clone(),
            old: before.get(name).cloned(),
            new: after.get(name).cloned(),
        })
        .collect()
}

/// The ref line of `push --porcelain`: `<flag>\t<from>:<to>\t<summary>`.
fn parse_push(stdout: &str) -> Option<PushOutcome> {
    let line = stdout.lines().find(|line| line.split('\t').count() >= 3)?;
    let mut fields = line.split('\t');
    let flag = fields.next()?.chars().next()?;
    let summary = fields.nth(1).unwrap_or_default();
    Some(match flag {
        '*' => PushOutcome::Created,
        ' ' => PushOutcome::FastForward,
        '+' => PushOutcome::Forced,
        '=' => PushOutcome::UpToDate,
        '!' => {
            let reason = summary
                .split_once('(')
                .and_then(|(_, rest)| rest.split_once(')'))
                .map_or(summary, |(reason, _)| reason);
            PushOutcome::Rejected {
                reason: reason.to_owned(),
            }
        }
        _ => PushOutcome::Unknown,
    })
}

fn refuse<T>(code: &'static str, message: impl Into<String>) -> Result<T, RemoteError> {
    Err(RemoteError::Refused {
        code,
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::git::remote::PushOutcome;

    use super::parse_push;

    #[test]
    fn porcelain_push_lines_map_to_outcomes() {
        let created = "To /tmp/r.git\n*\trefs/heads/f:refs/heads/f\t[new branch]\nDone\n";
        assert_eq!(parse_push(created), Some(PushOutcome::Created));
        let rejected =
            "To /tmp/r.git\n!\trefs/heads/f:refs/heads/f\t[rejected] (non-fast-forward)\nDone\n";
        assert_eq!(
            parse_push(rejected),
            Some(PushOutcome::Rejected {
                reason: "non-fast-forward".to_owned()
            })
        );
        let lease = "To x\n!\trefs/heads/f:refs/heads/f\t[rejected] (stale info)\nDone\n";
        assert_eq!(
            parse_push(lease),
            Some(PushOutcome::Rejected {
                reason: "stale info".to_owned()
            })
        );
        assert_eq!(
            parse_push("To x\n=\trefs/heads/f:refs/heads/f\t[up to date]\nDone\n"),
            Some(PushOutcome::UpToDate)
        );
        assert_eq!(parse_push("fatal: nothing\n"), None);
    }
}
