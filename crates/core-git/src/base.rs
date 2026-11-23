//! Which commit a new workspace's branch starts from: the repository's default branch when none is named, and the
//! namespace a short name belongs to, so a tag of the same name never wins.

use std::path::Path;

use crate::error::GitError;
use crate::git::Git;

/// The default bases, in order, after `origin/HEAD`: the ref probed and the name shown for it.
const DEFAULT_PROBES: [(&str, &str); 4] = [
    ("refs/remotes/origin/main", "origin/main"),
    ("refs/remotes/origin/master", "origin/master"),
    ("refs/heads/main", "main"),
    ("refs/heads/master", "master"),
];

const ORIGIN_HEAD: &str = "refs/remotes/origin/HEAD";

/// A base as shown to the person and as passed to git.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    /// What the workspace records, such as `origin/main`.
    pub name: String,
    /// What git receives, such as `refs/remotes/origin/main`.
    pub revision: String,
}

impl Git {
    /// The repository's default base: what `origin/HEAD` points at, else `origin/main`, `origin/master`, `main`,
    /// `master`; `None` when none of them exists.
    pub fn default_base(&self, repo: &Path) -> Result<Option<Base>, GitError> {
        // A character class matches the ref exactly, never a ref below it such as `main/topic`.
        let patterns: Vec<String> = std::iter::once(ORIGIN_HEAD)
            .chain(DEFAULT_PROBES.iter().map(|(reference, _)| *reference))
            .map(|reference| {
                let (head, last) = reference.split_at(reference.len() - 1);
                format!("{head}[{last}]")
            })
            .collect();
        let mut args = vec!["for-each-ref", "--format=%(refname)%00%(symref)"];
        args.extend(patterns.iter().map(String::as_str));
        let captured = self.run(Some(repo), args)?;
        captured.require_success()?;
        let mut present = Vec::new();
        for line in captured.text()?.lines() {
            let Some((reference, target)) = line.split_once('\0') else {
                continue;
            };
            if reference == ORIGIN_HEAD && !target.is_empty() {
                let name = target.strip_prefix("refs/remotes/").unwrap_or(target);
                return Ok(Some(Base {
                    name: name.to_owned(),
                    revision: target.to_owned(),
                }));
            }
            present.push(reference.to_owned());
        }
        Ok(DEFAULT_PROBES
            .iter()
            .find(|(reference, _)| present.iter().any(|found| found == reference))
            .map(|(reference, name)| Base {
                name: (*name).to_owned(),
                revision: (*reference).to_owned(),
            }))
    }

    /// The ref a named base means: a name with a `/` is a remote-tracking branch first, then a local branch; a plain
    /// name is a local branch. A full `refs/` name, a commit id or anything else is passed through unchanged.
    pub fn qualify_base(&self, repo: &Path, name: &str) -> Result<Base, GitError> {
        let candidates: Vec<String> = if name.starts_with("refs/") || name.starts_with('-') {
            Vec::new()
        } else if name.contains('/') {
            vec![format!("refs/remotes/{name}"), format!("refs/heads/{name}")]
        } else {
            vec![format!("refs/heads/{name}")]
        };
        for candidate in candidates {
            if self.ref_exists(repo, &candidate)? {
                return Ok(Base {
                    name: name.to_owned(),
                    revision: candidate,
                });
            }
        }
        Ok(Base {
            name: name.to_owned(),
            revision: name.to_owned(),
        })
    }
}
