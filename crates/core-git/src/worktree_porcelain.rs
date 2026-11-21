//! Parser for `git worktree list --porcelain`: one block of `key value` lines per
//! worktree, blocks separated by blank lines.

use crate::git::path_from_git;
use crate::worktree::WorktreeEntry;

/// Parses the output; an error names the first line that breaks the format.
pub(crate) fn parse_worktree_list(text: &str) -> Result<Vec<WorktreeEntry>, String> {
    let mut entries = Vec::new();
    let mut current: Option<WorktreeEntry> = None;
    for line in text.lines() {
        if line.is_empty() {
            entries.extend(current.take());
            continue;
        }
        // Paths may contain spaces, so only the first space separates key from value.
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        if key == "worktree" {
            entries.extend(current.take());
            if value.is_empty() {
                return Err(format!("`worktree` line without a path: {line:?}"));
            }
            current = Some(WorktreeEntry::new(path_from_git(value)));
            continue;
        }
        let Some(entry) = current.as_mut() else {
            return Err(format!("`{key}` line before any `worktree` line: {line:?}"));
        };
        match key {
            "HEAD" => entry.head = Some(value.to_owned()),
            "branch" => {
                entry.branch = Some(
                    value
                        .strip_prefix("refs/heads/")
                        .unwrap_or(value)
                        .to_owned(),
                );
            }
            "bare" => entry.bare = true,
            "detached" => entry.detached = true,
            "locked" => entry.locked = Some(value.to_owned()),
            "prunable" => entry.prunable = Some(value.to_owned()),
            // Keys added by newer git versions do not affect these fields.
            _ => {}
        }
    }
    entries.extend(current);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::parse_worktree_list;

    #[test]
    fn parses_the_main_and_a_linked_worktree() {
        let text = "worktree /repo\nHEAD 1111\nbranch refs/heads/main\n\n\
                    worktree /repo-wt\nHEAD 2222\nbranch refs/heads/feature/login\n\n";
        let entries = parse_worktree_list(text).expect("parse");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from("/repo"));
        assert_eq!(entries[0].head.as_deref(), Some("1111"));
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        assert_eq!(entries[1].branch.as_deref(), Some("feature/login"));
    }

    #[test]
    fn reads_detached_bare_locked_and_prunable_lines() {
        let text = "worktree C:/work/detached\nHEAD 3333\ndetached\n\n\
                    worktree C:/work/bare.git\nbare\n\n\
                    worktree C:/work/locked\nHEAD 4444\nbranch refs/heads/busy\nlocked agent is running\n\n\
                    worktree C:/work/gone\nHEAD 5555\ndetached\nlocked\nprunable gitdir file points to non-existent location\n";
        let entries = parse_worktree_list(text).expect("parse");
        assert_eq!(entries.len(), 4);
        assert!(entries[0].detached);
        assert_eq!(entries[0].branch, None);
        assert!(entries[1].bare);
        assert_eq!(entries[1].head, None);
        assert_eq!(entries[2].locked.as_deref(), Some("agent is running"));
        assert_eq!(entries[3].locked.as_deref(), Some(""));
        assert_eq!(
            entries[3].prunable.as_deref(),
            Some("gitdir file points to non-existent location")
        );
    }

    #[test]
    fn windows_drive_paths_keep_their_slashes_and_spaces() {
        let text = "worktree C:/Users/me/My Projects/repo\nHEAD 6\nbranch refs/heads/main\n\n";
        let entries = parse_worktree_list(text).expect("parse");
        assert_eq!(
            entries[0].path,
            PathBuf::from("C:/Users/me/My Projects/repo")
        );
    }

    #[test]
    fn the_last_block_is_kept_without_a_trailing_blank_line() {
        let text = "worktree /repo\nHEAD 1\nbranch refs/heads/main";
        assert_eq!(parse_worktree_list(text).expect("parse").len(), 1);
    }

    #[test]
    fn keys_from_newer_git_are_ignored() {
        let text = "worktree /repo\nHEAD 1\nbranch refs/heads/main\nfuture-key some value\n\n";
        let entries = parse_worktree_list(text).expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
    }

    #[test]
    fn empty_output_has_no_worktrees() {
        assert!(parse_worktree_list("").expect("parse").is_empty());
    }

    #[test]
    fn an_attribute_before_any_worktree_is_an_error() {
        assert!(parse_worktree_list("HEAD 1\n\n").is_err());
    }

    #[test]
    fn a_worktree_line_without_a_path_is_an_error() {
        assert!(parse_worktree_list("worktree\n").is_err());
        assert!(parse_worktree_list("worktree \n").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_prefixes_are_removed_from_paths() {
        let entries = parse_worktree_list("worktree \\\\?\\C:\\repo\nHEAD 1\n\n").expect("parse");
        assert_eq!(entries[0].path, PathBuf::from(r"C:\repo"));
    }
}
