//! `profile secure` on Windows (ADR 0073): the owner's explicit way to make an existing home private. It lists the DACL
//! entries of the home, `profiles` and the root that it would remove, and on request replaces each with the protected
//! owner-only DACL; Windows passes the change on to the inherited entries of everything below.

use std::fs;
use std::path::Path;

use crate::root_access::{FolderPlan, RootAccessError, profiles_dir};
use crate::windows_profile_root::{
    Disk, Links, RealDisk, check_link_home, owner_only_dir_dacl_sddl, set_dacl,
};
use crate::windows_sddl::Descriptor;
use crate::windows_trust::Trusted;

pub(crate) fn plan(home: &Path, dir: &Path) -> Result<Vec<FolderPlan>, RootAccessError> {
    plan_with(home, dir, &Trusted::current()?, &RealDisk)
}

pub(crate) fn apply(home: &Path, dir: &Path) -> Result<(), RootAccessError> {
    let trusted = Trusted::current()?;
    let dacl = owner_only_dir_dacl_sddl(trusted.user())?;
    // The home first: what `profiles` and the root inherit from it is already owner-only when they are set.
    for folder in plan_with(home, dir, &trusted, &RealDisk)? {
        if folder.changes() {
            set_dacl(&folder.path, &dacl, true)?;
        }
    }
    Ok(())
}

fn plan_with(
    home: &Path,
    dir: &Path,
    trusted: &Trusted,
    disk: &dyn Disk,
) -> Result<Vec<FolderPlan>, RootAccessError> {
    check_link_home(home, trusted, disk)?;
    let keep = vec![format!("(A;OICI;FA;;;{})", trusted.user())];
    let mut plans = Vec::new();
    for (path, links) in [
        (home, Links::Follow),
        (profiles_dir(home, dir), Links::Open),
        (dir, Links::Open),
    ] {
        let Ok(metadata) = fs::symlink_metadata(path) else {
            // Missing folders are created private by the start that follows.
            continue;
        };
        let refused = || -> RootAccessError {
            if path == dir {
                RootAccessError::ForeignEntries {
                    paths: vec![path.to_path_buf()],
                }
            } else {
                RootAccessError::UnsafeLocation {
                    path: path.to_path_buf(),
                }
            }
        };
        if links == Links::Open && metadata.file_type().is_symlink() {
            return Err(RootAccessError::UnsafeLocation {
                path: path.to_path_buf(),
            });
        }
        let descriptor = Descriptor::parse(&disk.read(path, links)?);
        // An owner can always change the access again, so a folder someone else owns cannot be made private here.
        if !descriptor
            .owner
            .as_ref()
            .is_some_and(|owner| trusted.owns(owner))
        {
            return Err(refused());
        }
        let remove = if descriptor.is_owner_only(trusted.user()) {
            Vec::new()
        } else {
            descriptor.entry_texts()
        };
        plans.push(FolderPlan {
            path: path.to_path_buf(),
            remove,
            keep: keep.clone(),
        });
    }
    Ok(plans)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{apply, plan};
    use crate::root_access::RootAccessError;
    use crate::windows_profile_root::{Links, read_sddl, secure_root, set_dacl};
    use crate::windows_sddl::Descriptor;
    use crate::windows_trust::current_user;

    /// A home made the ordinary way, the way an older build left it, with an extra grant others could use.
    fn shared_home() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let home = temp.path().join("home");
        let root = home.join("profiles").join("default");
        fs::create_dir_all(root.join("agents")).expect("an old profile");
        let user = current_user().expect("the user");
        set_dacl(
            &home,
            &format!("D:P(A;OICI;FA;;;{user})(A;OICI;0x1301bf;;;S-1-15-3-1)"),
            true,
        )
        .expect("a shared home");
        (temp, home, root)
    }

    #[test]
    fn the_plan_lists_every_entry_it_removes_and_the_one_it_keeps_and_changes_nothing() {
        let (_temp, home, root) = shared_home();
        let before = read_sddl(&home, Links::Open).expect("the descriptor");
        assert!(
            matches!(
                secure_root(&home, &root),
                Err(RootAccessError::UnsafeLocation { .. })
            ),
            "a start refuses the shared home"
        );

        let folders = plan(&home, &root).expect("a plan");

        assert_eq!(folders.len(), 3);
        assert_eq!(folders[0].path, home);
        assert!(
            folders[0]
                .remove
                .iter()
                .any(|entry| entry.contains("S-1-15-3-1")),
            "{:?}",
            folders[0].remove
        );
        let user = current_user().expect("the user");
        assert_eq!(folders[0].keep, vec![format!("(A;OICI;FA;;;{user})")]);
        assert_eq!(
            read_sddl(&home, Links::Open).expect("the descriptor"),
            before
        );
    }

    #[test]
    fn applying_makes_every_folder_owner_only_so_a_start_accepts_it() {
        let (_temp, home, root) = shared_home();

        apply(&home, &root).expect("applied");

        let user = current_user().expect("the user");
        for path in [&home, &home.join("profiles"), &root, &root.join("agents")] {
            let sddl = read_sddl(path, Links::Open).expect("the descriptor");
            assert!(!sddl.contains("S-1-15-3-1"), "{}: {sddl}", path.display());
        }
        for path in [&home, &home.join("profiles"), &root] {
            let descriptor = Descriptor::parse(&read_sddl(path, Links::Open).expect("descriptor"));
            assert!(descriptor.is_owner_only(&user), "{}", path.display());
        }
        assert!(
            plan(&home, &root)
                .expect("a plan")
                .iter()
                .all(|folder| !folder.changes())
        );
        secure_root(&home, &root).expect("a start accepts the home");
    }
}
