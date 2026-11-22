//! `git.status`: porcelain v2 records, numstat line counts, and the operation in progress (decision 0010).

use std::collections::HashMap;
use std::path::Path;

use massorca_protocol::methods::git::status::{
    BranchState, ConflictKind, GitOperation, GitStatus, StatusEntry, StatusTotals,
};
use massorca_protocol::methods::git::{ChangeKind, LineCounts};

use crate::error::GitError;
use crate::git::{Captured, Git, path_from_git};

impl Git {
    /// The status of the work tree at `dir`, listing at most `limit` entries.
    pub fn status(&self, dir: &Path, limit: usize) -> Result<GitStatus, GitError> {
        let captured = self.run(
            Some(dir),
            [
                "status",
                "--porcelain=v2",
                "-z",
                "--branch",
                "--untracked-files=all",
            ],
        )?;
        captured.require_success()?;
        let parsed = parse_porcelain_v2(captured.bytes())
            .map_err(|reason| captured.invalid_output(reason))?;
        let unstaged = self.numstat(dir, Side::Worktree)?;
        let staged = if parsed.branch.oid.is_some() {
            self.numstat(dir, Side::Index)?
        } else {
            self.numstat(dir, Side::IndexOnUnbornHead)?
        };
        let operation = self.operation_in_progress(dir)?;
        Ok(assemble(parsed, &staged, &unstaged, operation, limit))
    }

    /// Line counts per path of the index against the work tree, or of HEAD against the index.
    fn numstat(&self, dir: &Path, side: Side) -> Result<HashMap<String, LineCounts>, GitError> {
        // Plumbing: porcelain `git diff` refreshes and rewrites the index even under `GIT_OPTIONAL_LOCKS=0`.
        let options = ["--numstat", "-z", "--no-renames", "--no-textconv"];
        let args: Vec<&str> = match side {
            Side::Worktree => [&["diff-files"][..], &options].concat(),
            Side::Index => [&["diff-index"][..], &options, &["--cached", "HEAD"]].concat(),
            Side::IndexOnUnbornHead => {
                [&["diff-index"][..], &options, &["--cached", EMPTY_TREE]].concat()
            }
        };
        let captured = self.run(Some(dir), args)?;
        captured.require_success()?;
        parse_numstat(&captured)
    }

    /// The operation git stopped in the middle of in the work tree at `dir`, read from git's state files in that work
    /// tree's own git directory, so a linked worktree reports its own. A lone `REBASE_HEAD` can linger after a rebase
    /// ends, so the directories decide.
    pub fn operation_in_progress(&self, dir: &Path) -> Result<Option<GitOperation>, GitError> {
        let captured = self.run(Some(dir), ["rev-parse", "--absolute-git-dir"])?;
        captured.require_success()?;
        let git_dir = path_from_git(captured.text()?.trim_end_matches(['\r', '\n']));
        let operation = if git_dir.join("rebase-merge").is_dir() {
            Some(GitOperation::Rebase)
        } else if git_dir.join("rebase-apply").is_dir() {
            if git_dir.join("rebase-apply").join("applying").exists() {
                Some(GitOperation::ApplyMailbox)
            } else {
                Some(GitOperation::Rebase)
            }
        } else if git_dir.join("MERGE_HEAD").is_file() {
            Some(GitOperation::Merge)
        } else if git_dir.join("CHERRY_PICK_HEAD").is_file() {
            Some(GitOperation::CherryPick)
        } else if git_dir.join("REVERT_HEAD").is_file() {
            Some(GitOperation::Revert)
        } else {
            None
        };
        Ok(operation)
    }
}

/// The tree with no entries; every git version knows its id without storing it.
pub(crate) const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

#[derive(Debug, Clone, Copy)]
enum Side {
    Worktree,
    Index,
    /// Before the first commit there is no HEAD to compare the index with.
    IndexOnUnbornHead,
}

#[derive(Debug, Default)]
struct Parsed {
    branch: BranchState,
    entries: Vec<StatusEntry>,
}

fn parse_porcelain_v2(bytes: &[u8]) -> Result<Parsed, String> {
    let mut parsed = Parsed::default();
    let mut fields = bytes.split(|&b| b == 0).filter(|field| !field.is_empty());
    while let Some(record) = fields.next() {
        let (kind, rest) = record.split_at(1);
        match kind {
            b"#" => parse_header(&mut parsed.branch, &String::from_utf8_lossy(rest)),
            b"1" => parsed.entries.push(ordinary(rest, 7, None)?),
            b"2" => {
                let original = fields
                    .next()
                    .ok_or("a rename record has no original path")?;
                parsed.entries.push(ordinary(rest, 8, Some(original))?);
            }
            b"u" => parsed.entries.push(unmerged(rest)?),
            b"?" => parsed.entries.push(untracked(rest)),
            // Ignored files are not requested; anything else comes from a newer git.
            _ => {}
        }
    }
    Ok(parsed)
}

fn parse_header(branch: &mut BranchState, line: &str) {
    let line = line.trim_start();
    if let Some(oid) = line.strip_prefix("branch.oid ") {
        branch.oid = (oid != "(initial)").then(|| oid.to_owned());
    } else if let Some(head) = line.strip_prefix("branch.head ") {
        branch.head = (head != "(detached)").then(|| head.to_owned());
    } else if let Some(upstream) = line.strip_prefix("branch.upstream ") {
        branch.upstream = Some(upstream.to_owned());
        // Git prints `branch.ab` only when the upstream resolves; until then it is gone.
        branch.upstream_gone = true;
    } else if let Some(counts) = line.strip_prefix("branch.ab ") {
        let mut parts = counts.split(' ');
        branch.ahead = parts
            .next()
            .and_then(|p| p.strip_prefix('+'))
            .and_then(|n| n.parse().ok());
        branch.behind = parts
            .next()
            .and_then(|p| p.strip_prefix('-'))
            .and_then(|n| n.parse().ok());
        branch.upstream_gone = false;
    }
}

/// `1 XY sub mH mI mW hH hI path` and `2 XY sub mH mI mW hH hI Xscore path`: `fixed` fields precede the path.
fn ordinary(rest: &[u8], fixed: usize, original: Option<&[u8]>) -> Result<StatusEntry, String> {
    let (fields, path) = split_fields(rest, fixed)?;
    let xy = fields[0].as_bytes();
    let (path, lossy) = path_text(path);
    let original = original.map(path_text);
    Ok(StatusEntry {
        path,
        original_path: original.as_ref().map(|(text, _)| text.clone()),
        staged: change_kind(xy[0]),
        unstaged: change_kind(xy[1]),
        untracked: false,
        conflict: None,
        staged_lines: None,
        unstaged_lines: None,
        submodule: fields[1].starts_with('S'),
        path_lossy: lossy || original.is_some_and(|(_, lossy)| lossy),
    })
}

/// `u XY sub m1 m2 m3 mW h1 h2 h3 path`.
fn unmerged(rest: &[u8]) -> Result<StatusEntry, String> {
    let (fields, path) = split_fields(rest, 9)?;
    let (path, path_lossy) = path_text(path);
    let conflict = match fields[0].as_str() {
        "UU" => ConflictKind::BothModified,
        "AA" => ConflictKind::BothAdded,
        "DD" => ConflictKind::BothDeleted,
        "AU" => ConflictKind::AddedByUs,
        "UA" => ConflictKind::AddedByThem,
        "DU" => ConflictKind::DeletedByUs,
        "UD" => ConflictKind::DeletedByThem,
        _ => ConflictKind::Unknown,
    };
    Ok(StatusEntry {
        conflict: Some(conflict),
        submodule: fields[1].starts_with('S'),
        path_lossy,
        ..empty_entry(path)
    })
}

fn untracked(rest: &[u8]) -> StatusEntry {
    let (path, path_lossy) = path_text(rest.strip_prefix(b" ").unwrap_or(rest));
    StatusEntry {
        untracked: true,
        path_lossy,
        ..empty_entry(path)
    }
}

fn empty_entry(path: String) -> StatusEntry {
    StatusEntry {
        path,
        original_path: None,
        staged: None,
        unstaged: None,
        untracked: false,
        conflict: None,
        staged_lines: None,
        unstaged_lines: None,
        submodule: false,
        path_lossy: false,
    }
}

/// Splits ` f1 f2 ... fN path`: the path is everything after the `count`-th field, spaces included.
fn split_fields(rest: &[u8], count: usize) -> Result<(Vec<String>, &[u8]), String> {
    let rest = rest
        .strip_prefix(b" ")
        .ok_or("a status record has no fields")?;
    let mut fields = Vec::with_capacity(count);
    let mut remaining = rest;
    for _ in 0..count {
        let space = remaining
            .iter()
            .position(|&b| b == b' ')
            .ok_or("a status record has too few fields")?;
        fields.push(String::from_utf8_lossy(&remaining[..space]).into_owned());
        remaining = &remaining[space + 1..];
    }
    if fields[0].len() != 2 {
        return Err(format!("unexpected status code `{}`", fields[0]));
    }
    Ok((fields, remaining))
}

fn change_kind(code: u8) -> Option<ChangeKind> {
    match code {
        b'.' => None,
        b'A' => Some(ChangeKind::Added),
        b'M' => Some(ChangeKind::Modified),
        b'D' => Some(ChangeKind::Deleted),
        b'R' => Some(ChangeKind::Renamed),
        b'C' => Some(ChangeKind::Copied),
        b'T' => Some(ChangeKind::TypeChanged),
        _ => Some(ChangeKind::Unknown),
    }
}

/// The path as text, and whether it had to be decoded lossily.
fn path_text(bytes: &[u8]) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_owned(), false),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), true),
    }
}

/// `added\tremoved\tpath\0`; `-\t-\t` marks a binary file. Renames are off, so each record names one path.
fn parse_numstat(captured: &Captured) -> Result<HashMap<String, LineCounts>, GitError> {
    let mut counts = HashMap::new();
    for record in captured
        .bytes()
        .split(|&b| b == 0)
        .filter(|r| !r.is_empty())
    {
        let mut parts = record.splitn(3, |&b| b == b'\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(captured.invalid_output("a numstat record has too few fields"));
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
                    .and_then(|text| text.parse().ok())
                    .ok_or_else(|| captured.invalid_output("a numstat count is not a number"))
            };
            LineCounts {
                added: number(added)?,
                removed: number(removed)?,
                binary: false,
            }
        };
        counts.insert(path_text(path).0, lines);
    }
    Ok(counts)
}

fn assemble(
    parsed: Parsed,
    staged: &HashMap<String, LineCounts>,
    unstaged: &HashMap<String, LineCounts>,
    operation: Option<GitOperation>,
    limit: usize,
) -> GitStatus {
    let mut totals = StatusTotals::default();
    let mut entries = parsed.entries;
    for entry in &mut entries {
        totals.staged += u64::from(entry.staged.is_some());
        totals.unstaged += u64::from(entry.unstaged.is_some());
        totals.untracked += u64::from(entry.untracked);
        totals.conflicted += u64::from(entry.conflict.is_some());
        if entry.staged.is_some() {
            entry.staged_lines = staged.get(&entry.path).copied();
        }
        if entry.unstaged.is_some() {
            entry.unstaged_lines = unstaged.get(&entry.path).copied();
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let truncated = entries.len() > limit;
    entries.truncate(limit);
    GitStatus {
        branch: parsed.branch,
        operation,
        entries,
        totals,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_parse_with_spaces_renames_and_conflicts() {
        let bytes = b"# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\0\
1 .M N... 100644 100644 100644 h1 h2 a file.txt\0\
2 R. N... 100644 100644 100644 h1 h2 R100 new name\0old name\0\
u UU N... 100644 100644 100644 100644 h1 h2 h3 both.txt\0\
? dir/new file\0";
        let parsed = parse_porcelain_v2(bytes).expect("parse");
        assert_eq!(parsed.branch.head.as_deref(), Some("main"));
        assert_eq!(
            (parsed.branch.ahead, parsed.branch.behind),
            (Some(2), Some(1))
        );
        assert!(!parsed.branch.upstream_gone);
        let paths: Vec<_> = parsed.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(
            paths,
            ["a file.txt", "new name", "both.txt", "dir/new file"]
        );
        assert_eq!(parsed.entries[1].original_path.as_deref(), Some("old name"));
        assert_eq!(parsed.entries[1].staged, Some(ChangeKind::Renamed));
        assert_eq!(parsed.entries[2].conflict, Some(ConflictKind::BothModified));
        assert!(parsed.entries[3].untracked);
    }

    #[test]
    fn an_upstream_without_counts_is_gone_and_an_unborn_head_has_no_oid() {
        let bytes =
            b"# branch.oid (initial)\0# branch.head (detached)\0# branch.upstream origin/x\0";
        let parsed = parse_porcelain_v2(bytes).expect("parse");
        assert!(parsed.branch.upstream_gone);
        assert_eq!(parsed.branch.oid, None);
        assert_eq!(parsed.branch.head, None);
    }

    #[test]
    fn a_non_utf8_path_is_marked_lossy() {
        let parsed = parse_porcelain_v2(b"? caf\xe9\0").expect("parse");
        assert!(parsed.entries[0].path_lossy);
    }

    #[test]
    fn the_limit_cuts_entries_but_not_totals() {
        let parsed = parse_porcelain_v2(b"? b\0? a\0? c\0").expect("parse");
        let status = assemble(parsed, &HashMap::new(), &HashMap::new(), None, 2);
        assert!(status.truncated);
        assert_eq!(status.totals.untracked, 3);
        let paths: Vec<_> = status.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a", "b"]);
    }
}
