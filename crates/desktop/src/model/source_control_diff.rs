//! A file's diff from the service as the DiffView draws it (decision 0011, DiffView.md): git's unified
//! patch parsed into hunks, and the state the view shows instead of the diff. Free of GPUI.

use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::diff::GitDiffFile;
use massorca_ui::components::diff_view::{DiffLine, DiffStatus, FileDiff, FileStatus, Hunk};

/// The file status the diff header shows, from the change kind the status listed.
pub fn file_status(kind: ChangeKind) -> FileStatus {
    match kind {
        ChangeKind::Added => FileStatus::Added,
        ChangeKind::Deleted => FileStatus::Deleted,
        ChangeKind::Renamed | ChangeKind::Copied => FileStatus::Renamed,
        ChangeKind::Modified | ChangeKind::TypeChanged | ChangeKind::Unknown => {
            FileStatus::Modified
        }
    }
}

/// What the view shows in place of the diff: a submodule's pointer, a binary change with its sizes, an empty change,
/// or the diff itself. A patch the service cut is still drawn; the page says so above it.
pub fn diff_status(file: &GitDiffFile) -> DiffStatus {
    if let Some(pointer) = &file.submodule {
        return DiffStatus::Submodule {
            summary: pointer.summary.clone().into(),
            dirty: pointer.dirty,
        };
    }
    if file.binary {
        return DiffStatus::Binary {
            before: size_text(file.old_size).into(),
            after: size_text(file.new_size).into(),
        };
    }
    match file.patch {
        Some(_) => DiffStatus::Ready,
        None => DiffStatus::Empty,
    }
}

/// A byte count as the binary note writes it: "2.1 MiB", or "none" for a side that does not exist.
pub fn size_text(bytes: Option<u64>) -> String {
    const KIB: f64 = 1024.0;
    match bytes {
        None => "none".to_owned(),
        Some(bytes) if bytes < 1024 => format!("{bytes} B"),
        Some(bytes) if (bytes as f64) < KIB * KIB => format!("{:.1} KiB", bytes as f64 / KIB),
        Some(bytes) => format!("{:.1} MiB", bytes as f64 / (KIB * KIB)),
    }
}

/// Parses the unified patch of one file into the hunks the view draws. Text before the first hunk
/// (the file header) is skipped, and so is git's "\ No newline at end of file" marker. Line numbers
/// come from each hunk header.
pub fn parse_patch(path: &str, status: FileStatus, patch: &str) -> FileDiff {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut numbers = (0, 0);
    for line in patch.lines() {
        if let Some(hunk) = parse_header(line) {
            numbers = (hunk.old_start, hunk.new_start);
            hunks.push(hunk);
            continue;
        }
        let Some(hunk) = hunks.last_mut() else {
            continue;
        };
        let (old_no, new_no) = &mut numbers;
        let parsed = match line.chars().next() {
            Some('+') => {
                let added = DiffLine::added(*new_no, &line[1..]);
                *new_no += 1;
                added
            }
            Some('-') => {
                let removed = DiffLine::removed(*old_no, &line[1..]);
                *old_no += 1;
                removed
            }
            Some(' ') | None => {
                let context = DiffLine::context(*old_no, *new_no, line.get(1..).unwrap_or(""));
                *old_no += 1;
                *new_no += 1;
                context
            }
            // The "\ No newline" marker and any other line that is not part of the hunk.
            Some(_) => continue,
        };
        hunk.lines.push(parsed);
    }
    FileDiff {
        path: path.to_owned().into(),
        status,
        hunks,
        hidden_after: Vec::new(),
    }
}

/// `@@ -old_start[,old_len] +new_start[,new_len] @@ heading`, or `None` for any other line.
fn parse_header(line: &str) -> Option<Hunk> {
    let rest = line.strip_prefix("@@ ")?;
    let (ranges, heading) = rest.split_once(" @@")?;
    let (old, new) = ranges.split_once(' ')?;
    let (old_start, old_len) = range(old.strip_prefix('-')?)?;
    let (new_start, new_len) = range(new.strip_prefix('+')?)?;
    let heading = heading.trim_start();
    Some(Hunk {
        heading: (!heading.is_empty()).then(|| heading.to_owned().into()),
        old_start,
        old_len,
        new_start,
        new_len,
        hidden_before: Vec::new(),
        lines: Vec::new(),
    })
}

/// A hunk range: `start,len`, or `start` alone for one line.
fn range(text: &str) -> Option<(u32, u32)> {
    match text.split_once(',') {
        Some((start, len)) => Some((start.parse().ok()?, len.parse().ok()?)),
        None => Some((text.parse().ok()?, 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -2,4 +2,5 @@ fn first() {
 keep
-old line
+new line
+extra
 tail
\\ No newline at end of file
@@ -20 +21 @@
-was
+is
";

    #[test]
    fn the_hunks_carry_their_ranges_and_heading() {
        let diff = parse_patch("src/lib.rs", FileStatus::Modified, PATCH);
        assert_eq!(diff.hunks.len(), 2);
        let first = &diff.hunks[0];
        assert_eq!(
            (
                first.old_start,
                first.old_len,
                first.new_start,
                first.new_len
            ),
            (2, 4, 2, 5)
        );
        assert_eq!(first.heading.as_deref(), Some("fn first() {"));
        assert!(diff.hunks[1].heading.is_none());
        assert_eq!((diff.hunks[1].old_len, diff.hunks[1].new_len), (1, 1));
    }

    #[test]
    fn lines_are_numbered_per_side_and_the_marker_is_skipped() {
        let diff = parse_patch("src/lib.rs", FileStatus::Modified, PATCH);
        let lines = &diff.hunks[0].lines;
        assert_eq!(lines.len(), 5, "the no-newline marker is not a line");
        assert_eq!(lines[0], DiffLine::context(2, 2, "keep"));
        assert_eq!(lines[1], DiffLine::removed(3, "old line"));
        assert_eq!(lines[2], DiffLine::added(3, "new line"));
        assert_eq!(lines[3], DiffLine::added(4, "extra"));
        assert_eq!(lines[4], DiffLine::context(4, 5, "tail"));
        assert_eq!(diff.hunks[1].lines[0], DiffLine::removed(20, "was"));
        assert_eq!(diff.hunks[1].lines[1], DiffLine::added(21, "is"));
    }

    #[test]
    fn the_file_header_before_the_first_hunk_is_not_a_line() {
        let diff = parse_patch(
            "a",
            FileStatus::Added,
            "--- /dev/null\n+++ b/a\n@@ -0,0 +1 @@\n+one\n",
        );
        assert_eq!(diff.hunks.len(), 1);
        assert_eq!(diff.hunks[0].lines, vec![DiffLine::added(1, "one")]);
        assert_eq!(diff.status, FileStatus::Added);
        assert_eq!(&*diff.path, "a");
    }

    #[test]
    fn a_patch_without_hunks_has_no_lines() {
        let diff = parse_patch("a", FileStatus::Modified, "diff --git a/a b/a\n");
        assert!(diff.hunks.is_empty());
    }

    #[test]
    fn a_binary_change_shows_its_sizes_and_an_unchanged_one_is_empty() {
        let binary = GitDiffFile {
            old_size: Some(2 * 1024 * 1024 + 1024 * 1024 / 10),
            ..crate::service::fixtures::base_git_diff_file()
        };
        assert_eq!(
            diff_status(&binary),
            DiffStatus::Binary {
                before: "2.1 MiB".into(),
                after: "none".into(),
            }
        );
        let unchanged = GitDiffFile {
            binary: false,
            old_size: None,
            ..binary
        };
        assert_eq!(diff_status(&unchanged), DiffStatus::Empty);
    }

    #[test]
    fn a_submodule_pointer_change_is_drawn_as_its_pointer_and_not_as_an_empty_diff() {
        let pointer = GitDiffFile {
            submodule: Some(crate::service::fixtures::base_submodule_pointer()),
            ..crate::service::fixtures::base_git_diff_file()
        };
        assert_eq!(
            diff_status(&pointer),
            DiffStatus::Submodule {
                summary: "Subproject commit 1111111aaaaaaa..2222222bbbbbbb".into(),
                dirty: Some(false),
            }
        );
    }

    #[test]
    fn sizes_read_in_bytes_kibibytes_and_mebibytes() {
        assert_eq!(size_text(Some(512)), "512 B");
        assert_eq!(size_text(Some(1536)), "1.5 KiB");
        assert_eq!(size_text(Some(3 * 1024 * 1024)), "3.0 MiB");
        assert_eq!(size_text(None), "none");
    }

    #[test]
    fn every_change_kind_maps_to_a_file_status() {
        assert_eq!(file_status(ChangeKind::Added), FileStatus::Added);
        assert_eq!(file_status(ChangeKind::Deleted), FileStatus::Deleted);
        assert_eq!(file_status(ChangeKind::Copied), FileStatus::Renamed);
        assert_eq!(file_status(ChangeKind::TypeChanged), FileStatus::Modified);
    }
}
