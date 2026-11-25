//! Links from an agent folder to the canonical copy, as pv96 places them: a relative symlink on Unix, a junction with an
//! absolute target on Windows. The standard library cannot make a junction and the workspace forbids `unsafe`, so the
//! junction comes from `mklink /J` through the process helper. Removal deletes the link itself, never what it points to.

use std::fs;
use std::io;
use std::path::Path;
#[cfg(unix)]
use std::path::{Component, PathBuf};

/// Makes `link` point at `canonical`. `link` must not exist, so an unowned folder is never replaced.
pub(super) fn create_link(canonical: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let parent = link.parent().unwrap_or(Path::new("."));
        std::os::unix::fs::symlink(relative_target(parent, canonical), link)
    }
    #[cfg(windows)]
    {
        let status = massorca_process::command("cmd")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(link)
            .arg(canonical)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other("mklink /J was refused"))
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (canonical, link);
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// Whether `link` is a link to `canonical`, by the target the link itself records.
pub(super) fn link_points_to(link: &Path, canonical: &Path) -> bool {
    let Ok(target) = fs::read_link(link) else {
        return false;
    };
    #[cfg(unix)]
    {
        let parent = link.parent().unwrap_or(Path::new("."));
        target == relative_target(parent, canonical)
    }
    #[cfg(not(unix))]
    {
        normal_windows(&target) == normal_windows(canonical)
    }
}

/// Removes the link, leaving its target alone.
pub(super) fn remove_link(link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::remove_file(link)
    }
    #[cfg(not(unix))]
    {
        // Why: on Windows RemoveDirectory on a junction removes the junction and never its target's contents.
        fs::remove_dir(link)
    }
}

/// The path from `from` to `to`, both absolute, as a symlink target: `..` for each folder left, then the rest.
#[cfg(unix)]
fn relative_target(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<Component> = from.components().collect();
    let to: Vec<Component> = to.components().collect();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut target = PathBuf::new();
    for _ in common..from.len() {
        target.push("..");
    }
    for part in &to[common..] {
        target.push(part.as_os_str());
    }
    target
}

/// A Windows path without its verbatim prefix, folded to lower case, so a junction's target compares equal to ours.
#[cfg(not(unix))]
fn normal_windows(path: &Path) -> String {
    let text = path.to_string_lossy().to_ascii_lowercase();
    text.trim_start_matches(r"\\?\")
        .trim_start_matches(r"\??\")
        .trim_end_matches(['\\', '/'])
        .to_owned()
}
