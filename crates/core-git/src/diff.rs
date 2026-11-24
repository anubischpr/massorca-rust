//! `git.diff` and `git.diff_file` (decision 0011). Plumbing only (`diff-files`, `diff-index`, `diff-tree`):
//! porcelain `git diff` rewrites the index, which would wake every watcher.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use massorca_protocol::methods::git::diff::{
    DiffEntry, DiffTarget, DiffTotals, GitDiff, GitDiffFile,
};
use massorca_protocol::methods::git::{ChangeKind, LineCounts};

use crate::error::GitError;
use crate::git::{Captured, Git};
use crate::status::EMPTY_TREE;
use crate::submodule_pointer::GITLINK_MODE;

/// Options that keep a user's diff drivers, colours and pagers out of every diff.
const SAFE_OPTIONS: [&str; 3] = ["--no-ext-diff", "--no-textconv", "--no-color"];
/// Git's own binary test: a NUL byte in the first 8000 bytes.
const BINARY_PROBE: usize = 8000;
pub(crate) const ZERO_OID: &str = "0000000000000000000000000000000000000000";

impl Git {
    /// The files `target` changes, sorted by path, at most `limit` of them.
    pub fn diff(&self, dir: &Path, target: &DiffTarget, limit: usize) -> Result<GitDiff, GitError> {
        let command = self.diff_command(dir, target)?;
        let raw = self.run(Some(dir), command.args(&["--raw", "-z"], &[]))?;
        raw.require_success()?;
        let numstat = self.run(Some(dir), command.args(&["--numstat", "-z"], &[]))?;
        numstat.require_success()?;
        let counts = parse_numstat(&numstat)?;
        let mut files: Vec<DiffEntry> = parse_raw(&raw)?
            .into_iter()
            .filter(|record| !command.stat_only(record, &counts))
            .map(|record| record.entry(&counts))
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let totals = DiffTotals {
            files: files.len() as u64,
            added: files.iter().map(|file| file.lines.added).sum(),
            removed: files.iter().map(|file| file.lines.removed).sum(),
        };
        let truncated = files.len() > limit;
        files.truncate(limit);
        Ok(GitDiff {
            files,
            totals,
            truncated,
        })
    }

    /// The patch of `path` under `target`, cut at the last line break within `max_bytes`.
    /// An untracked file is shown as wholly added.
    pub fn diff_file(
        &self,
        dir: &Path,
        target: &DiffTarget,
        path: &str,
        original_path: Option<&str>,
        max_bytes: usize,
    ) -> Result<GitDiffFile, GitError> {
        let command = self.diff_command(dir, target)?;
        let paths: Vec<&str> = std::iter::once(path).chain(original_path).collect();
        let raw = self.run(Some(dir), command.args(&["--raw", "-z"], &paths))?;
        raw.require_success()?;
        let record = parse_raw(&raw)?
            .into_iter()
            .find(|record| record.path == path);
        let Some(record) = record else {
            if command.reads_worktree() && self.is_untracked(dir, path)? {
                return untracked_patch(dir, path, max_bytes);
            }
            return Ok(GitDiffFile {
                path: path.to_owned(),
                ..empty_patch()
            });
        };
        if record.old_mode == GITLINK_MODE || record.new_mode == GITLINK_MODE {
            let old = (record.old_mode == GITLINK_MODE).then_some(record.old_oid.as_str());
            let new = (record.new_mode == GITLINK_MODE).then_some(record.new_oid.as_str());
            let pointer = self.submodule_pointer(dir, path, old, new, command.reads_worktree())?;
            return Ok(GitDiffFile {
                path: path.to_owned(),
                submodule: Some(pointer),
                ..empty_patch()
            });
        }
        let numstat = self.run(Some(dir), command.args(&["--numstat", "-z"], &paths))?;
        numstat.require_success()?;
        let lines = parse_numstat(&numstat)?.get(path).copied();
        if lines.is_some_and(|lines| lines.binary) {
            return Ok(GitDiffFile {
                path: path.to_owned(),
                binary: true,
                old_size: self.side_size(dir, &record.old_mode, &record.old_oid, None)?,
                new_size: self.side_size(dir, &record.new_mode, &record.new_oid, Some(path))?,
                ..empty_patch()
            });
        }
        let patch = self.run_capped(Some(dir), command.args(&["-p"], &paths), max_bytes)?;
        if !patch.truncated() {
            patch.require_success()?;
        }
        let (text, truncated) = cut_at_line(patch.bytes(), patch.truncated());
        Ok(GitDiffFile {
            path: path.to_owned(),
            patch: (!text.is_empty()).then_some(text),
            truncated,
            ..empty_patch()
        })
    }

    fn diff_command(&self, dir: &Path, target: &DiffTarget) -> Result<DiffCommand, GitError> {
        let head = || -> Result<String, GitError> {
            Ok(if self.ref_exists(dir, "HEAD")? {
                "HEAD".to_owned()
            } else {
                EMPTY_TREE.to_owned()
            })
        };
        Ok(match target {
            DiffTarget::Worktree => DiffCommand {
                base: vec!["diff-files".into()],
                revisions: vec![],
                worktree: true,
                renames: false,
            },
            DiffTarget::Staged => DiffCommand {
                base: vec!["diff-index".into(), "--cached".into()],
                revisions: vec![head()?],
                worktree: false,
                renames: true,
            },
            DiffTarget::Head => DiffCommand {
                base: vec!["diff-index".into()],
                revisions: vec![head()?],
                worktree: true,
                renames: true,
            },
            DiffTarget::Range { from, to } => DiffCommand {
                base: vec!["diff-tree".into(), "-r".into()],
                revisions: vec![
                    from.clone(),
                    to.clone().unwrap_or_else(|| "HEAD".to_owned()),
                ],
                worktree: false,
                renames: true,
            },
        })
    }

    fn is_untracked(&self, dir: &Path, path: &str) -> Result<bool, GitError> {
        let listed = self.run(
            Some(dir),
            [
                "--literal-pathspecs",
                "ls-files",
                "-z",
                "--others",
                "--exclude-standard",
                "--",
                path,
            ],
        )?;
        listed.require_success()?;
        Ok(!listed.bytes().is_empty())
    }

    /// One side's size: a blob's size, the working file's size, or `None` where the side does not exist.
    fn side_size(
        &self,
        dir: &Path,
        mode: &str,
        oid: &str,
        worktree_path: Option<&str>,
    ) -> Result<Option<u64>, GitError> {
        if mode == "000000" {
            return Ok(None);
        }
        if oid == ZERO_OID {
            // A zero id on the new side means the working tree, whose content git has not hashed.
            return Ok(worktree_path
                .and_then(|path| std::fs::symlink_metadata(dir.join(path)).ok())
                .map(|metadata| metadata.len()));
        }
        let captured = self.run(Some(dir), ["cat-file", "-s", oid])?;
        captured.require_success()?;
        Ok(captured.text()?.trim().parse().ok())
    }
}

/// The diff subcommand and its revisions, with options placed before the revisions as plumbing requires.
#[derive(Debug)]
struct DiffCommand {
    base: Vec<String>,
    revisions: Vec<String>,
    /// One side is the working tree, where a touched but unchanged file can show up.
    worktree: bool,
    renames: bool,
}

impl DiffCommand {
    fn args(&self, extra: &[&str], paths: &[&str]) -> Vec<String> {
        let mut args = vec!["--literal-pathspecs".to_owned()];
        args.extend(self.base.iter().cloned());
        args.extend(SAFE_OPTIONS.iter().map(|option| (*option).to_owned()));
        args.push(if self.renames { "-M" } else { "--no-renames" }.to_owned());
        args.extend(extra.iter().map(|option| (*option).to_owned()));
        args.extend(self.revisions.iter().cloned());
        args.push("--".to_owned());
        args.extend(paths.iter().map(|path| (*path).to_owned()));
        args
    }

    fn reads_worktree(&self) -> bool {
        self.worktree
    }

    /// A file whose stat data changed but whose content did not: same mode, no lines, not binary.
    fn stat_only(&self, record: &RawRecord, counts: &HashMap<String, LineCounts>) -> bool {
        self.worktree
            && record.status == 'M'
            && record.old_mode == record.new_mode
            // Numstat omits a file whose content did not change at all.
            && counts
                .get(&record.path)
                .is_none_or(|lines| !lines.binary && lines.added == 0 && lines.removed == 0)
    }
}

#[derive(Debug)]
struct RawRecord {
    old_mode: String,
    new_mode: String,
    old_oid: String,
    new_oid: String,
    status: char,
    path: String,
    original_path: Option<String>,
    path_lossy: bool,
}

impl RawRecord {
    fn entry(self, counts: &HashMap<String, LineCounts>) -> DiffEntry {
        let change = match self.status {
            'A' => ChangeKind::Added,
            'M' => ChangeKind::Modified,
            'D' => ChangeKind::Deleted,
            'R' => ChangeKind::Renamed,
            'C' => ChangeKind::Copied,
            'T' => ChangeKind::TypeChanged,
            _ => ChangeKind::Unknown,
        };
        let lines = counts.get(&self.path).copied().unwrap_or(LineCounts {
            added: 0,
            removed: 0,
            binary: false,
        });
        DiffEntry {
            lines,
            change,
            original_path: self.original_path,
            path_lossy: self.path_lossy,
            path: self.path,
        }
    }
}

/// `:<old mode> <new mode> <old id> <new id> <status>[score]\0<path>\0[<new path>\0]`.
fn parse_raw(captured: &Captured) -> Result<Vec<RawRecord>, GitError> {
    let mut records = Vec::new();
    let mut fields = captured
        .bytes()
        .split(|&b| b == 0)
        .filter(|field| !field.is_empty());
    while let Some(header) = fields.next() {
        let header = String::from_utf8_lossy(header);
        let parts: Vec<&str> = header.trim_start_matches(':').split(' ').collect();
        let [old_mode, new_mode, old_oid, new_oid, status] = parts[..] else {
            return Err(captured.invalid_output(format!("unexpected raw diff record `{header}`")));
        };
        let status = status.chars().next().unwrap_or('?');
        let first = fields
            .next()
            .ok_or_else(|| captured.invalid_output("a raw record has no path"))?;
        let (path, original) = if matches!(status, 'R' | 'C') {
            let second = fields
                .next()
                .ok_or_else(|| captured.invalid_output("a rename has no new path"))?;
            (second, Some(first))
        } else {
            (first, None)
        };
        let (path, lossy) = text(path);
        let original = original.map(text);
        records.push(RawRecord {
            old_mode: old_mode.to_owned(),
            new_mode: new_mode.to_owned(),
            old_oid: old_oid.to_owned(),
            new_oid: new_oid.to_owned(),
            status,
            path,
            path_lossy: lossy || original.as_ref().is_some_and(|(_, lossy)| *lossy),
            original_path: original.map(|(text, _)| text),
        });
    }
    Ok(records)
}

/// `added\tremoved\tpath\0`, or for a rename `added\tremoved\t\0old\0new\0`; keyed by the new path.
fn parse_numstat(captured: &Captured) -> Result<HashMap<String, LineCounts>, GitError> {
    let mut counts = HashMap::new();
    let mut fields = captured
        .bytes()
        .split(|&b| b == 0)
        .filter(|field| !field.is_empty());
    while let Some(record) = fields.next() {
        let mut parts = record.splitn(3, |&b| b == b'\t');
        let (Some(added), Some(removed)) = (parts.next(), parts.next()) else {
            return Err(captured.invalid_output("a numstat record has too few fields"));
        };
        let path = match parts.next() {
            Some(path) if !path.is_empty() => path,
            // A rename: the old path, then the new one, follow as their own fields.
            _ => {
                fields.next();
                fields
                    .next()
                    .ok_or_else(|| captured.invalid_output("a numstat rename has no new path"))?
            }
        };
        let lines = if added == b"-" && removed == b"-" {
            LineCounts {
                added: 0,
                removed: 0,
                binary: true,
            }
        } else {
            let number = |field: &[u8]| {
                std::str::from_utf8(field)
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| captured.invalid_output("a numstat count is not a number"))
            };
            LineCounts {
                added: number(added)?,
                removed: number(removed)?,
                binary: false,
            }
        };
        counts.insert(text(path).0, lines);
    }
    Ok(counts)
}

fn text(bytes: &[u8]) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_owned(), false),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), true),
    }
}

/// The patch text, cut after its last complete line when the output was truncated.
fn cut_at_line(bytes: &[u8], truncated: bool) -> (String, bool) {
    let kept = if truncated {
        bytes
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(&bytes[..0], |end| &bytes[..=end])
    } else {
        bytes
    };
    (String::from_utf8_lossy(kept).into_owned(), truncated)
}

fn empty_patch() -> GitDiffFile {
    GitDiffFile {
        path: String::new(),
        patch: None,
        binary: false,
        truncated: false,
        old_size: None,
        new_size: None,
        submodule: None,
    }
}

/// An untracked file as an addition. A symlink shows its target, as git shows a link; it is never followed.
fn untracked_patch(dir: &Path, path: &str, max_bytes: usize) -> Result<GitDiffFile, GitError> {
    let full = dir.join(path);
    let unreadable = |source| GitError::File {
        path: full.clone(),
        source,
    };
    let metadata = std::fs::symlink_metadata(&full).map_err(unreadable)?;
    let content = if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(&full).map_err(unreadable)?;
        target.to_string_lossy().into_owned().into_bytes()
    } else {
        let mut bytes = Vec::new();
        let file = std::fs::File::open(&full).map_err(unreadable)?;
        file.take(
            u64::try_from(max_bytes)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
        bytes
    };
    if content[..content.len().min(BINARY_PROBE)].contains(&0) {
        return Ok(GitDiffFile {
            path: path.to_owned(),
            binary: true,
            new_size: Some(metadata.len()),
            ..empty_patch()
        });
    }
    let mut patch = format!("--- /dev/null\n+++ b/{path}\n");
    let body_budget = max_bytes.saturating_sub(patch.len());
    let truncated = content.len() > body_budget;
    let (body, _) = cut_at_line(&content[..content.len().min(body_budget)], truncated);
    let lines: Vec<&str> = body.lines().collect();
    if !lines.is_empty() {
        patch.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len()));
        for line in lines {
            patch.push('+');
            patch.push_str(line);
            patch.push('\n');
        }
    }
    Ok(GitDiffFile {
        path: path.to_owned(),
        patch: Some(patch),
        truncated,
        new_size: Some(metadata.len()),
        ..empty_patch()
    })
}
