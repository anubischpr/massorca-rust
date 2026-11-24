//! Puts the stable command on disk and takes it off again: a symbolic link on Unix, a command file on Windows.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::layout::{self, Layout, Platform};
use super::plan::Observed;

/// Command files are a few hundred bytes; anything much larger is not one of ours.
const MAX_COMMAND_FILE_BYTES: u64 = 4096;

/// What is at the command path now.
pub fn observe(layout: &Layout) -> io::Result<Observed> {
    let metadata = match fs::symlink_metadata(&layout.command_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Observed::Missing),
        Err(error) => return Err(error),
    };
    let points_to = match layout.platform {
        Platform::Unix if metadata.file_type().is_symlink() => {
            Some(fs::read_link(&layout.command_path)?)
        }
        Platform::Windows if metadata.is_file() && metadata.len() <= MAX_COMMAND_FILE_BYTES => {
            read_command_file(&layout.command_path)
        }
        _ => None,
    };
    Ok(match points_to {
        Some(points_to) if layout::is_massorca_binary(&points_to) => Observed::Ours { points_to },
        _ => Observed::Foreign,
    })
}

fn read_command_file(path: &Path) -> Option<PathBuf> {
    layout::command_file_target(&fs::read_to_string(path).ok()?)
}

/// Makes the command run `target`, replacing the command this build made before.
pub fn place(layout: &Layout, target: &Path) -> io::Result<()> {
    fs::create_dir_all(&layout.bin_dir)?;
    let staging = layout.bin_dir.join(format!(
        ".{}.{}.partial",
        layout::COMMAND_NAME,
        std::process::id()
    ));
    let _ = fs::remove_file(&staging);
    let written = match layout.platform {
        Platform::Unix => symlink(target, &staging),
        Platform::Windows => {
            let text = layout::command_file_text(target).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the target cannot be written into a command file",
                )
            })?;
            fs::write(&staging, text)
        }
    };
    // Why: rename replaces the old command in one step, so a failed write never leaves the command missing.
    written
        .and_then(|()| fs::rename(&staging, &layout.command_path))
        .inspect_err(|_| {
            let _ = fs::remove_file(&staging);
        })
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink(_target: &Path, _link: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "a link to the binary is made on Unix only",
    ))
}

/// Removes the command file or link. The caller has checked that massorca made it.
pub fn remove(layout: &Layout) -> io::Result<()> {
    fs::remove_file(&layout.command_path)
}

/// Removes the command folder when nothing else is in it; returns whether the folder is gone afterwards.
pub fn remove_bin_dir_if_empty(layout: &Layout) -> io::Result<bool> {
    match fs::remove_dir(&layout.bin_dir) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn a_windows_command_file_is_placed_observed_and_replaced_in_place() {
        let root = tempdir().expect("temp dir");
        let layout = Layout::windows(root.path());
        assert_eq!(observe(&layout).expect("observe"), Observed::Missing);
        place(&layout, Path::new("opt/massorca/massorca.exe")).expect("place");
        assert_eq!(
            observe(&layout).expect("observe"),
            Observed::Ours {
                points_to: PathBuf::from("opt/massorca/massorca.exe")
            }
        );
        place(&layout, Path::new("opt/new/massorca.exe")).expect("replace");
        assert_eq!(
            observe(&layout).expect("observe"),
            Observed::Ours {
                points_to: PathBuf::from("opt/new/massorca.exe")
            }
        );
        let names: Vec<OsString> = fs::read_dir(&layout.bin_dir)
            .expect("folder")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(names, vec![OsString::from("massorca.cmd")]);
    }

    #[test]
    fn a_batch_file_massorca_did_not_write_is_foreign() {
        let root = tempdir().expect("temp dir");
        let layout = Layout::windows(root.path());
        fs::create_dir_all(&layout.bin_dir).expect("folder");
        fs::write(&layout.command_path, "@echo off\r\ncall tool.bat %*\r\n").expect("write");
        assert_eq!(observe(&layout).expect("observe"), Observed::Foreign);
    }

    #[test]
    fn remove_bin_dir_keeps_a_folder_that_holds_other_files() {
        let root = tempdir().expect("temp dir");
        let layout = Layout::windows(root.path());
        place(&layout, Path::new("opt/massorca/massorca.exe")).expect("place");
        fs::write(layout.bin_dir.join("other.cmd"), "").expect("other file");
        remove(&layout).expect("remove");
        assert_eq!(observe(&layout).expect("observe"), Observed::Missing);
        assert!(!remove_bin_dir_if_empty(&layout).expect("keep folder"));
        assert!(layout.bin_dir.exists());
        fs::remove_file(layout.bin_dir.join("other.cmd")).expect("clear folder");
        assert!(remove_bin_dir_if_empty(&layout).expect("remove folder"));
        assert!(!layout.bin_dir.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_unix_command_is_a_link_that_observe_reads_place_repoints_and_remove_deletes() {
        let root = tempdir().expect("temp dir");
        let layout = Layout::unix(root.path());
        place(&layout, Path::new("/opt/massorca/massorca")).expect("place");
        assert_eq!(
            observe(&layout).expect("observe"),
            Observed::Ours {
                points_to: PathBuf::from("/opt/massorca/massorca")
            }
        );
        place(&layout, Path::new("/opt/new/massorca")).expect("repoint");
        assert_eq!(
            observe(&layout).expect("observe"),
            Observed::Ours {
                points_to: PathBuf::from("/opt/new/massorca")
            }
        );
        remove(&layout).expect("remove");
        assert_eq!(observe(&layout).expect("observe"), Observed::Missing);
    }

    #[cfg(unix)]
    #[test]
    fn a_regular_file_or_another_programs_link_at_the_command_path_is_foreign() {
        let root = tempdir().expect("temp dir");
        let layout = Layout::unix(root.path());
        fs::create_dir_all(&layout.bin_dir).expect("folder");
        fs::write(&layout.command_path, "#!/bin/sh\n").expect("file");
        assert_eq!(observe(&layout).expect("observe"), Observed::Foreign);
        fs::remove_file(&layout.command_path).expect("clear");
        std::os::unix::fs::symlink("/usr/bin/orca", &layout.command_path).expect("link");
        assert_eq!(observe(&layout).expect("observe"), Observed::Foreign);
    }
}
