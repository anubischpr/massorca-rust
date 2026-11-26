//! Writes an executable file that a test runs right after writing it.

use std::path::Path;

/// Writes `contents` to `path` as an executable. On Unix the bytes reach `path` through `cp`: a file this process
/// writes stays open for writing in any child another test thread forks until that child execs, and running the file
/// in that window fails with ETXTBSY. Only the `cp` child ever holds `path` open.
pub fn write_executable(path: &Path, contents: &str) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let source = path.with_extension("src");
        std::fs::write(&source, contents).expect("write the executable's source");
        let copied = crate::command("cp")
            .arg(&source)
            .arg(path)
            .status()
            .expect("run cp");
        assert!(copied.success(), "cp {} failed", source.display());
        std::fs::remove_file(&source).expect("remove the executable's source");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");
    }
    #[cfg(not(unix))]
    std::fs::write(path, contents).expect("write the executable");
}

/// Copies the executable `from` to `to`, with `cp` writing the copy on Unix for the reason `write_executable` gives.
pub fn copy_executable(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let (from, to): (&Path, &Path) = (from.as_ref(), to.as_ref());
        let copied = crate::command("cp").arg(from).arg(to).status()?;
        if copied.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "cp {} failed",
                from.display()
            )))
        }
    }
    #[cfg(not(unix))]
    std::fs::copy(from, to).map(|_| ())
}
