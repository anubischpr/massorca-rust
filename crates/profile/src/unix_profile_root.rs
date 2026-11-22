//! Unix access of the profile root (ADR 0073): created `0700`, its location and content checked on every start.

use std::fs::{self, Metadata};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;

use crate::root_access::{Findings, RootAccess, RootAccessError, profiles_dir};
use crate::unix_user::current_uid;

const ROOT_UID: u32 = 0;
const GROUP_OR_OTHER_WRITE: u32 = 0o022;
const OTHER_WRITE: u32 = 0o002;
const STICKY: u32 = 0o1000;

/// Reads the owner of an entry; tests pass another to report owners no unprivileged test can set.
type Owner<'a> = &'a dyn Fn(&Path, &Metadata) -> u32;

/// Creates `dir` with mode `0700` in one step; false when it already existed.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<bool> {
    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn secure_root(home: &Path, dir: &Path) -> Result<RootAccess, RootAccessError> {
    secure_root_with(home, dir, current_uid(), &|_, metadata| metadata.uid())
}

fn secure_root_with(
    home: &Path,
    dir: &Path,
    uid: u32,
    owner: Owner,
) -> Result<RootAccess, RootAccessError> {
    if let Some(parent) = home.parent() {
        fs::create_dir_all(parent)?;
    }
    // The home may itself be a link the user chose; `profiles` and the root may not.
    let profiles = profiles_dir(home, dir);
    if !create_private_dir(home)? {
        check_ancestor(home, &fs::metadata(home)?, uid, owner)?;
    }
    if !create_private_dir(profiles)? {
        check_ancestor(profiles, &fs::symlink_metadata(profiles)?, uid, owner)?;
    }
    if create_private_dir(dir)? {
        return Ok(RootAccess::Created);
    }
    let metadata = fs::symlink_metadata(dir)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RootAccessError::UnsafeLocation {
            path: dir.to_path_buf(),
        });
    }
    if owner(dir, &metadata) != uid {
        return Err(RootAccessError::ForeignEntries {
            paths: vec![dir.to_path_buf()],
        });
    }
    let mode = metadata.mode();
    // Others could have rewritten files the user owns; making the root private would make the content trusted.
    if mode & GROUP_OR_OTHER_WRITE != 0 {
        return Err(RootAccessError::WritableByOthers {
            paths: vec![dir.to_path_buf()],
        });
    }
    let repaired = mode & 0o777 != 0o700;
    if repaired {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    scan_entries(dir, uid, owner)?.into_result()?;
    Ok(if repaired {
        RootAccess::Repaired
    } else {
        RootAccess::Private
    })
}

/// A folder between the home and the root must be a real folder owned by the user or root, and not writable by group
/// or others unless it is sticky.
fn check_ancestor(
    path: &Path,
    metadata: &Metadata,
    uid: u32,
    owner: Owner,
) -> Result<(), RootAccessError> {
    let owned = [uid, ROOT_UID].contains(&owner(path, metadata));
    let mode = metadata.mode();
    let shared = mode & GROUP_OR_OTHER_WRITE != 0 && mode & STICKY == 0;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || !owned || shared {
        return Err(RootAccessError::UnsafeLocation {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

/// Every entry below `root`, links never followed: owned by the user or root, and not writable by others. Group write
/// is allowed: entries sat below a `0700` root since ADR 0004, and a `002` umask sets it on everything.
fn scan_entries(root: &Path, uid: u32, owner: Owner) -> io::Result<Findings> {
    let mut findings = Findings::default();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if dir == root => return Err(error),
            // A folder that cannot be listed may hide planted content.
            Err(_) => {
                findings.add_foreign(dir);
                continue;
            }
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if ![uid, ROOT_UID].contains(&owner(&path, &metadata)) {
                findings.add_foreign(path);
            } else if !metadata.file_type().is_symlink() && metadata.mode() & OTHER_WRITE != 0 {
                findings.add_writable(path);
            } else if metadata.is_dir() {
                pending.push(path);
            }
            if findings.is_full() {
                return Ok(findings);
            }
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use super::{secure_root, secure_root_with};
    use crate::root_access::{RootAccess, RootAccessError};
    use crate::unix_user::current_uid;

    fn layout() -> (TempDir, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let home = temp.path().join("home");
        let root = home.join("profiles").join("default");
        (temp, home, root)
    }

    fn mode(path: &Path) -> u32 {
        fs::symlink_metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o7777
    }

    fn chmod(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod");
    }

    #[test]
    fn a_new_root_and_its_ancestors_have_mode_0700() {
        let (_temp, home, root) = layout();

        assert_eq!(
            secure_root(&home, &root).expect("created"),
            RootAccess::Created
        );

        for path in [&home, &home.join("profiles"), &root] {
            assert_eq!(mode(path), 0o700, "{}", path.display());
        }
    }

    #[test]
    fn a_root_only_its_owner_can_write_is_repaired_to_0700() {
        let (_temp, home, root) = layout();
        secure_root(&home, &root).expect("created");
        chmod(&root, 0o755);

        assert_eq!(
            secure_root(&home, &root).expect("repaired"),
            RootAccess::Repaired
        );
        assert_eq!(mode(&root), 0o700);
    }

    #[test]
    fn a_root_others_could_write_is_refused_and_left_as_it_is() {
        let (_temp, home, root) = layout();
        secure_root(&home, &root).expect("created");
        chmod(&root, 0o777);

        let refused = secure_root(&home, &root);

        assert!(
            matches!(refused, Err(RootAccessError::WritableByOthers { .. })),
            "{refused:?}"
        );
        assert_eq!(mode(&root), 0o777);
    }

    #[test]
    fn a_symlinked_root_is_refused() {
        let (_temp, home, root) = layout();
        secure_root(&home, &home.join("profiles").join("seed")).expect("safe ancestors");
        std::os::unix::fs::symlink(home.join("profiles").join("seed"), &root).expect("a link");

        let refused = secure_root(&home, &root);

        assert!(
            matches!(&refused, Err(RootAccessError::UnsafeLocation { path }) if path == &root),
            "{refused:?}"
        );
    }

    #[test]
    fn a_home_group_members_can_write_is_refused_unless_sticky() {
        let (_temp, home, root) = layout();
        secure_root(&home, &root).expect("created");
        chmod(&home, 0o770);
        let refused = secure_root(&home, &root);
        assert!(
            matches!(&refused, Err(RootAccessError::UnsafeLocation { path }) if path == &home),
            "{refused:?}"
        );

        chmod(&home, 0o1777);
        assert_eq!(
            secure_root(&home, &root).expect("sticky"),
            RootAccess::Private
        );
    }

    #[test]
    fn a_root_owned_by_another_uid_is_refused_as_foreign() {
        let (_temp, home, root) = layout();
        secure_root(&home, &root).expect("created");
        let root_for_owner = root.clone();
        let owner = move |path: &Path, metadata: &fs::Metadata| {
            if path == root_for_owner {
                4242
            } else {
                metadata.uid()
            }
        };

        let refused = secure_root_with(&home, &root, current_uid(), &owner);

        assert!(
            matches!(&refused, Err(RootAccessError::ForeignEntries { paths }) if paths == &vec![root.clone()]),
            "{refused:?}"
        );
    }

    #[test]
    fn a_deep_entry_owned_by_another_uid_is_refused() {
        let (_temp, home, root) = layout();
        secure_root(&home, &root).expect("created");
        let deep = root.join("agents/homes/claude/.claude/hooks");
        fs::create_dir_all(&deep).expect("deep folders");
        let planted = deep.join("planted.json");
        fs::write(&planted, b"{}").expect("a planted file");
        let planted_for_owner = planted.clone();
        let owner = move |path: &Path, metadata: &fs::Metadata| {
            if path == planted_for_owner {
                4242
            } else {
                metadata.uid()
            }
        };

        let refused = secure_root_with(&home, &root, current_uid(), &owner);

        assert!(
            matches!(&refused, Err(RootAccessError::ForeignEntries { paths }) if paths == &vec![planted.clone()]),
            "{refused:?}"
        );
    }
}
