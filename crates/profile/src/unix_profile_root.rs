//! Unix access of the profile root (ADR 0073): created `0700`, its location and content checked on every start.

use std::fs::{self, Metadata};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;

use crate::root_access::{Findings, FolderPlan, RootAccess, RootAccessError, profiles_dir};
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
    check_link_home(home, uid, owner)?;
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
        // Before the scan, as on Windows; a refusal by the scan leaves the root `0700`, which trusts nothing.
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    scan_entries(dir, uid, owner)?.into_result()?;
    Ok(if repaired {
        RootAccess::Repaired
    } else {
        RootAccess::Private
    })
}

/// Opens `dir` with `O_NOFOLLOW | O_DIRECTORY`, so a symlink or a file is refused, checks that the user owns it, and
/// sets it `0700` through the descriptor, never through a link. A link or file fails with `InvalidInput`, a folder
/// another user owns with `PermissionDenied`.
pub(crate) fn open_private_dir(dir: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(dir)
        .map_err(|error| match error.raw_os_error() {
            Some(libc::ELOOP | libc::ENOTDIR) => io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is a link or not a folder", dir.display()),
            ),
            _ => error,
        })?;
    if file.metadata()?.uid() != current_uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} belongs to another user", dir.display()),
        ));
    }
    file.set_permissions(fs::Permissions::from_mode(0o700))?;
    Ok(file)
}

/// A home that is itself a symlink is replaceable by whoever can write its parent, so the parent must be as safe as an
/// ancestor.
fn check_link_home(home: &Path, uid: u32, owner: Owner) -> Result<(), RootAccessError> {
    let is_link =
        fs::symlink_metadata(home).is_ok_and(|metadata| metadata.file_type().is_symlink());
    let Some(parent) = home.parent().filter(|_| is_link) else {
        return Ok(());
    };
    // `lstat`: a parent that is itself a symlink is refused, as on Windows.
    match check_ancestor(parent, &fs::symlink_metadata(parent)?, uid, owner) {
        Err(RootAccessError::UnsafeLocation { .. }) => Err(RootAccessError::UnsafeLocation {
            path: home.to_path_buf(),
        }),
        other => other,
    }
}

/// What `profile secure` changes: the mode of each existing folder of the home, `profiles` and the root that is not
/// `0700`. A folder another user owns, or a symlinked `profiles` or root, cannot be made private here.
pub(crate) fn plan_secure(home: &Path, dir: &Path) -> Result<Vec<FolderPlan>, RootAccessError> {
    let uid = current_uid();
    check_link_home(home, uid, &|_, metadata| metadata.uid())?;
    let mut plans = Vec::new();
    for (path, follow) in [(home, true), (profiles_dir(home, dir), false), (dir, false)] {
        let metadata = match if follow {
            fs::metadata(path)
        } else {
            fs::symlink_metadata(path)
        } {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(RootAccessError::UnsafeLocation {
                path: path.to_path_buf(),
            });
        }
        if metadata.uid() != uid {
            return Err(RootAccessError::UnsafeLocation {
                path: path.to_path_buf(),
            });
        }
        let mode = metadata.mode() & 0o7777;
        plans.push(FolderPlan {
            path: path.to_path_buf(),
            remove: if mode == 0o700 {
                Vec::new()
            } else {
                vec![format!("mode {mode:04o}")]
            },
            keep: vec!["mode 0700".to_owned()],
        });
    }
    Ok(plans)
}

/// Sets each planned folder `0700` through a descriptor: `profiles` and the root are opened with `O_NOFOLLOW`, so a
/// symlink swapped in after the plan is refused, never followed; the home may be a link the user chose. A failure
/// part-way leaves the folders already done private; running the command again finishes the job.
pub(crate) fn apply_secure(home: &Path, dir: &Path) -> Result<(), RootAccessError> {
    use std::os::unix::fs::OpenOptionsExt;

    for folder in plan_secure(home, dir)? {
        if !folder.changes() {
            continue;
        }
        if folder.path == home {
            fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY)
                .open(home)?
                .set_permissions(fs::Permissions::from_mode(0o700))?;
        } else {
            open_private_dir(&folder.path).map_err(|_| RootAccessError::UnsafeLocation {
                path: folder.path.clone(),
            })?;
        }
    }
    Ok(())
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
            // Removed since its parent was listed, for example by a service starting at the same time: nothing is hidden.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            // A folder that cannot be listed may hide planted content.
            Err(_) => {
                findings.add_foreign(dir);
                continue;
            }
        };
        for entry in entries {
            let path = entry?.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                // Removed since it was listed, as a concurrent start does with the store's journal.
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
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
    fn profile_secure_plans_then_makes_a_shared_home_0700() {
        let (_temp, home, root) = layout();
        fs::create_dir_all(&root).expect("an old profile");
        chmod(&home, 0o777);
        assert!(matches!(
            secure_root(&home, &root),
            Err(RootAccessError::UnsafeLocation { .. })
        ));

        let plan = super::plan_secure(&home, &root).expect("a plan");
        assert_eq!(plan[0].remove, vec!["mode 0777".to_owned()]);
        assert_eq!(mode(&home), 0o777, "planning changes nothing");

        super::apply_secure(&home, &root).expect("applied");
        assert_eq!(mode(&home), 0o700);
        secure_root(&home, &root).expect("a start accepts it");
    }

    #[test]
    fn a_symlinked_home_in_a_shared_parent_is_refused() {
        let (temp, home, root) = layout();
        let real = temp.path().join("real-home");
        fs::create_dir(&real).expect("a home");
        chmod(&real, 0o700);
        std::os::unix::fs::symlink(&real, &home).expect("a link home");
        chmod(temp.path(), 0o777);

        let refused = secure_root(&home, &root);

        chmod(temp.path(), 0o700);
        assert!(
            matches!(&refused, Err(RootAccessError::UnsafeLocation { path }) if path == &home),
            "{refused:?}"
        );
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

    /// A service starting at the same time creates and removes files in the root, such as the store's journal, while
    /// this scan lists it. The listing is read before the first entry is checked, so the removal happens in between.
    #[test]
    fn entries_removed_while_the_scan_lists_them_are_skipped() {
        let (_temp, home, root) = layout();
        secure_root(&home, &root).expect("created");
        for name in ["a", "b", "c"] {
            fs::write(root.join(name), b"x").expect("a file");
        }
        fs::create_dir(root.join("d")).expect("a folder");
        let removed = std::cell::Cell::new(false);
        let root_for_owner = root.clone();
        let owner = move |path: &Path, metadata: &fs::Metadata| {
            if path.parent() == Some(root_for_owner.as_path()) && !removed.replace(true) {
                for name in ["a", "b", "c", "d"] {
                    let other = root_for_owner.join(name);
                    if other != path {
                        let _ = fs::remove_file(&other).or_else(|_| fs::remove_dir(&other));
                    }
                }
            }
            metadata.uid()
        };

        let scanned = secure_root_with(&home, &root, current_uid(), &owner);

        assert!(matches!(scanned, Ok(RootAccess::Private)), "{scanned:?}");
    }

    #[test]
    fn a_folder_removed_before_the_scan_lists_it_is_skipped_not_refused() {
        let (_temp, home, root) = layout();
        secure_root(&home, &root).expect("created");
        let gone = root.join("gone");
        fs::create_dir_all(gone.join("inner")).expect("folders");
        let gone_for_owner = gone.clone();
        let owner = move |path: &Path, metadata: &fs::Metadata| {
            if path == gone_for_owner {
                fs::remove_dir_all(path).expect("removed");
            }
            metadata.uid()
        };

        let scanned = secure_root_with(&home, &root, current_uid(), &owner);

        assert!(matches!(scanned, Ok(RootAccess::Private)), "{scanned:?}");
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
