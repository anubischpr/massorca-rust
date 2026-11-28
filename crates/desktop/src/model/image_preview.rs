//! The two sides of an image diff (GIT-06): which sides a diff compares, and what each side holds. Free of GPUI: the
//! shell turns the bytes into an image it draws.

use massorca_protocol::methods::git::diff::{DiffTarget, FileSide, GitFileContent, preview_mime};

/// The base side and the new side a diff compares, as `git.file_content` reads them.
pub fn sides(target: &DiffTarget) -> (FileSide, FileSide) {
    match target {
        DiffTarget::Worktree => (FileSide::Index, FileSide::Worktree),
        DiffTarget::Staged => (revision("HEAD"), FileSide::Index),
        DiffTarget::Head => (revision("HEAD"), FileSide::Worktree),
        DiffTarget::Range { from, to } => {
            (revision(from), revision(to.as_deref().unwrap_or("HEAD")))
        }
    }
}

fn revision(rev: &str) -> FileSide {
    FileSide::Revision {
        rev: rev.to_owned(),
    }
}

/// What one side of an image diff holds, as its answer says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageSide {
    /// No file at the path on this side, such as the side of an added or a deleted image.
    Missing,
    /// The file is over the cap, so only its size is known.
    TooLarge { size: u64 },
    /// The file's bytes, as the side holds them.
    Loaded { bytes: Vec<u8> },
    /// The side has the file, but the answer carried no bytes for it.
    Unreadable,
}

impl ImageSide {
    /// Reads one side's answer.
    pub fn from_content(content: &GitFileContent) -> Self {
        match (content.size, content.too_large) {
            (None, _) => Self::Missing,
            (Some(size), true) => Self::TooLarge { size },
            (Some(_), false) => content
                .decoded()
                .map_or(Self::Unreadable, |bytes| Self::Loaded { bytes }),
        }
    }
}

/// Both sides of an image diff, each `None` until its answer arrives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImagePreview {
    /// The image type the path names, from its extension.
    pub mime: &'static str,
    pub before: Option<ImageSide>,
    pub after: Option<ImageSide>,
}

impl ImagePreview {
    /// The preview of `path` while both sides are on their way; `None` when the path names no image.
    pub fn loading(path: &str) -> Option<Self> {
        Some(Self {
            mime: preview_mime(path)?,
            before: None,
            after: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    fn answer(size: Option<u64>, too_large: bool, bytes: Option<&[u8]>) -> GitFileContent {
        GitFileContent {
            path: "pic.png".to_owned(),
            size,
            too_large,
            content_base64: bytes.map(|bytes| STANDARD.encode(bytes)),
            mime: Some("image/png".to_owned()),
        }
    }

    #[test]
    fn a_worktree_diff_compares_the_index_with_the_worktree_and_the_others_with_head() {
        assert_eq!(
            sides(&DiffTarget::Worktree),
            (FileSide::Index, FileSide::Worktree)
        );
        assert_eq!(
            sides(&DiffTarget::Staged),
            (revision("HEAD"), FileSide::Index)
        );
        assert_eq!(
            sides(&DiffTarget::Head),
            (revision("HEAD"), FileSide::Worktree)
        );
    }

    #[test]
    fn a_range_compares_its_two_revisions_and_the_new_one_defaults_to_head() {
        let open = DiffTarget::Range {
            from: "main".to_owned(),
            to: None,
        };
        assert_eq!(sides(&open), (revision("main"), revision("HEAD")));
        let closed = DiffTarget::Range {
            from: "main".to_owned(),
            to: Some("feat".to_owned()),
        };
        assert_eq!(sides(&closed), (revision("main"), revision("feat")));
    }

    #[test]
    fn a_side_without_a_file_is_missing_a_capped_one_keeps_its_size_and_a_read_one_has_its_bytes() {
        assert_eq!(
            ImageSide::from_content(&answer(None, false, None)),
            ImageSide::Missing
        );
        assert_eq!(
            ImageSide::from_content(&answer(Some(5), true, None)),
            ImageSide::TooLarge { size: 5 }
        );
        assert_eq!(
            ImageSide::from_content(&answer(Some(3), false, Some(b"png"))),
            ImageSide::Loaded {
                bytes: b"png".to_vec()
            }
        );
        assert_eq!(
            ImageSide::from_content(&answer(Some(3), false, None)),
            ImageSide::Unreadable
        );
    }

    #[test]
    fn only_a_path_that_names_an_image_starts_a_preview() {
        let svg = ImagePreview::loading("logo.svg").expect("an svg is previewed");
        assert_eq!(svg.mime, "image/svg+xml");
        assert_eq!((svg.before, svg.after), (None, None));
        assert!(ImagePreview::loading("README.md").is_none());
    }
}
