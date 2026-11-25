//! `git.diff` lists the changed files of a comparison; `git.diff_file` returns one file's patch (decision 0011);
//! `git.file_content` returns one file's bytes on one side of a comparison (decision 0099).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use super::{ChangeKind, LineCounts, is_false};

/// Params: [`GitDiffParams`]. Result: [`GitDiff`].
pub const DIFF: &str = "git.diff";
/// Params: [`GitDiffFileParams`]. Result: [`GitDiffFile`].
pub const DIFF_FILE: &str = "git.diff_file";
/// Params: [`GitFileContentParams`]. Result: [`GitFileContent`].
pub const FILE_CONTENT: &str = "git.file_content";

/// Files listed when the caller sets no limit.
pub const DEFAULT_DIFF_LIMIT: u32 = 1_000;
pub const MAX_DIFF_LIMIT: u32 = 10_000;
/// Patch bytes returned when the caller sets no cap.
pub const DEFAULT_PATCH_BYTES: u32 = 1 << 20;
pub const MAX_PATCH_BYTES: u32 = 4 << 20;
/// File bytes returned when the caller sets no cap; the most a caller may ask for.
pub const DEFAULT_CONTENT_BYTES: u32 = 4 << 20;
pub const MAX_CONTENT_BYTES: u32 = 4 << 20;

/// What a diff compares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum DiffTarget {
    /// The index against the working tree: unstaged changes.
    Worktree,
    /// HEAD against the index: staged changes.
    Staged,
    /// HEAD against the working tree: everything not committed.
    Head,
    /// One revision against another, such as a branch against its base.
    Range {
        from: String,
        /// Defaults to `HEAD`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffParams {
    pub workspace: String,
    #[serde(flatten)]
    pub target: DiffTarget,
    /// Clamped to [`MAX_DIFF_LIMIT`]; `None` means [`DEFAULT_DIFF_LIMIT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiff {
    /// Sorted by path; at most the requested limit.
    pub files: Vec<DiffEntry>,
    /// Counts over every changed file, including those past the limit.
    pub totals: DiffTotals,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffEntry {
    pub path: String,
    /// The path before a rename or copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    pub change: ChangeKind,
    pub lines: LineCounts,
    #[serde(default, skip_serializing_if = "is_false")]
    pub path_lossy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DiffTotals {
    pub files: u64,
    pub added: u64,
    pub removed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffFileParams {
    pub workspace: String,
    #[serde(flatten)]
    pub target: DiffTarget,
    pub path: String,
    /// For a rename, the old path from `git.diff`, so the patch shows the rename rather than an addition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    /// Clamped to [`MAX_PATCH_BYTES`]; `None` means [`DEFAULT_PATCH_BYTES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffFile {
    pub path: String,
    /// The unified patch, git's own format; absent for a binary file and when nothing changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub binary: bool,
    /// The patch was cut at the last line break before the byte cap.
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    /// Sizes of the two sides in bytes, for a binary file; `None` where that side does not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_size: Option<u64>,
    /// Set for a submodule (gitlink) change in place of a text patch: the commits the parent records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submodule: Option<SubmodulePointer>,
}

/// A submodule's change, which git records as the commit a parent points at, not as file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmodulePointer {
    /// The commit the old side records; absent where that side has no submodule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_commit: Option<String>,
    /// The commit the new side records: the submodule's checked-out HEAD for a working-tree side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_commit: Option<String>,
    /// Whether the submodule's working tree has changes; absent when the new side is not a checked-out working tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
    /// Git's one-line form, `Subproject commit <old>..<new>`, with one side when only one is known.
    pub summary: String,
}

/// Which side of a comparison a file is read from (decision 0099).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "side", rename_all = "snake_case")]
pub enum FileSide {
    /// The file on disk under the workspace root.
    Worktree,
    /// The file as the index holds it, the content a commit would record.
    Index,
    /// The file as a revision holds it, such as `HEAD` or a branch.
    Revision { rev: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitFileContentParams {
    pub workspace: String,
    #[serde(flatten)]
    pub side: FileSide,
    /// Workspace-relative, with `/` separators.
    pub path: String,
    /// Clamped to [`MAX_CONTENT_BYTES`]; `None` means [`DEFAULT_CONTENT_BYTES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u32>,
}

/// One side's file. A side with no file at `path` has no `size`; a file over the cap has `too_large` and no bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitFileContent {
    pub path: String,
    /// Bytes of the file on that side; absent when the side has no file at `path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// The file is larger than the cap: `size` is known, and no bytes are sent.
    #[serde(default, skip_serializing_if = "is_false")]
    pub too_large: bool,
    /// The file's bytes, base64; absent when the side has no file or the file is too large.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
    /// The image type of a previewable image, from its extension (see [`preview_mime`]); absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
}

impl GitFileContent {
    /// The bytes `content_base64` carries; `None` when no bytes were sent.
    pub fn decoded(&self) -> Option<Vec<u8>> {
        STANDARD.decode(self.content_base64.as_deref()?).ok()
    }
}

/// The image type of a file that a client can preview, from its extension; `None` for any other file.
pub fn preview_mime(path: &str) -> Option<&'static str> {
    let extension = std::path::Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}
