//! Keeping a project's in-project worktrees folder out of git (decision 0183): one line in
//! `<common git dir>/info/exclude`, local to the clone and never committed.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::GitError;
use crate::git::{Git, path_from_git};

/// The folder inside a project that holds its worktrees in the `in_project` layout.
pub const WORKTREES_FOLDER: &str = ".worktrees";

/// The exclude pattern: the folder at the top of the work tree only.
const EXCLUDE_LINE: &str = "/.worktrees/";

impl Git {
    /// The git directory that every worktree of the repository containing `dir` shares.
    pub fn common_dir(&self, dir: &Path) -> Result<PathBuf, GitError> {
        let captured = self.run(Some(dir), ["rev-parse", "--git-common-dir"])?;
        captured.require_success()?;
        let line = captured.text()?.trim_end_matches(['\r', '\n']).to_owned();
        Ok(dunce::simplified(&dir.join(path_from_git(&line))).to_path_buf())
    }
}

/// Adds `/.worktrees/` to `<common_dir>/info/exclude` unless a line already excludes the folder, creating `info/`
/// when it is missing. Swap this one function to keep the folder out with `.gitignore` instead.
pub fn exclude_worktrees_folder(common_dir: &Path) -> Result<(), GitError> {
    let info = common_dir.join("info");
    let file = info.join("exclude");
    let failed = |source: io::Error| GitError::File {
        path: file.clone(),
        source,
    };
    let current = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(failed(error)),
    };
    let present = current.lines().map(str::trim).any(|line| {
        matches!(
            line,
            "/.worktrees/" | "/.worktrees" | ".worktrees/" | ".worktrees"
        )
    });
    if present {
        return Ok(());
    }
    fs::create_dir_all(&info).map_err(failed)?;
    let separator = if current.is_empty() || current.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .and_then(|mut out| out.write_all(format!("{separator}{EXCLUDE_LINE}\n").as_bytes()))
        .map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::exclude_worktrees_folder;

    #[test]
    fn the_line_is_added_once_and_info_is_created_when_missing() {
        let dir = tempfile::tempdir().expect("temp dir");
        exclude_worktrees_folder(dir.path()).expect("first");
        exclude_worktrees_folder(dir.path()).expect("second");
        let text = std::fs::read_to_string(dir.path().join("info").join("exclude")).expect("read");
        assert_eq!(text, "/.worktrees/\n");
    }

    #[test]
    fn existing_lines_are_kept_and_a_missing_final_newline_is_supplied() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("info")).expect("info");
        let file = dir.path().join("info").join("exclude");
        std::fs::write(&file, "# local\n*.log").expect("write");
        exclude_worktrees_folder(dir.path()).expect("exclude");
        assert_eq!(
            std::fs::read_to_string(&file).expect("read"),
            "# local\n*.log\n/.worktrees/\n"
        );
    }

    #[test]
    fn a_line_the_person_wrote_for_the_folder_is_enough() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("info")).expect("info");
        let file = dir.path().join("info").join("exclude");
        std::fs::write(&file, ".worktrees\n").expect("write");
        exclude_worktrees_folder(dir.path()).expect("exclude");
        assert_eq!(
            std::fs::read_to_string(&file).expect("read"),
            ".worktrees\n"
        );
    }
}
