//! `profile secure` on Windows (ADR 0073): the owner's explicit way to make an existing home private. It lists the DACL
//! entries of the home, `profiles` and the root that it would remove, and on request replaces each with the protected
//! owner-only DACL; Windows passes the change on to the inherited entries of everything below.

use std::fs;
use std::path::Path;

use crate::root_access::{FolderPlan, RootAccessError, profiles_dir};
use crate::windows_profile_root::{
    Disk, Links, RealDisk, check_link_home, owner_only_dir_dacl_sddl, pin_for_access_change,
    reset_inherited_below, set_dacl_through,
};
use crate::windows_sddl::Descriptor;
use crate::windows_trust::Trusted;

pub(crate) fn plan(home: &Path, dir: &Path) -> Result<Vec<FolderPlan>, RootAccessError> {
    plan_with(home, dir, &Trusted::current()?, &RealDisk)
}

/// Pins every folder of the plan first, without following links (the home may be a link the user chose), so none
/// can be renamed or swapped while its access changes, and sets each DACL through its handle, on exactly that folder,
/// home first. Once they are owner-only nobody else can rename them, so the pins are released and each folder is set
/// again by path, resetting the inherited entries of everything below. A failure part-way leaves the folders already done private and the
/// rest as they were; running the command again finishes the job.
pub(crate) fn apply(home: &Path, dir: &Path) -> Result<(), RootAccessError> {
    let trusted = Trusted::current()?;
    let dacl = owner_only_dir_dacl_sddl(trusted.user())?;
    let plan = plan_with(home, dir, &trusted, &RealDisk)?;
    let mut pinned = Vec::new();
    for folder in &plan {
        let links = if folder.path == home {
            Links::Follow
        } else {
            Links::Open
        };
        pinned.push((folder, pin_for_access_change(&folder.path, links)?));
    }
    for (folder, pin) in &pinned {
        if folder.changes() {
            set_dacl_through(pin, &dacl)?;
        }
    }
    drop(pinned);
    // Resetting below a folder also clears the protection of `profiles` and the root under it, so every folder of
    // the plan is set again, home first, once anything changed.
    if plan.iter().any(FolderPlan::changes) {
        for folder in &plan {
            reset_inherited_below(&folder.path, &dacl)?;
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

    fn junction(link: &std::path::Path, target: &std::path::Path) {
        let made = massorca_process::command("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .expect("mklink runs");
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
    }

    #[test]
    fn a_folder_pinned_for_an_access_change_is_never_a_junction_and_cannot_be_moved() {
        use crate::windows_profile_root::pin_for_access_change;

        let temp = tempfile::tempdir().expect("a temporary folder");
        let real = temp.path().join("real");
        fs::create_dir(&real).expect("a folder");
        let link = temp.path().join("link");
        junction(&link, &real);
        assert!(matches!(
            pin_for_access_change(&link, Links::Open),
            Err(RootAccessError::UnsafeLocation { .. })
        ));

        let pin = pin_for_access_change(&real, Links::Open).expect("pinned");
        assert!(
            fs::rename(&real, temp.path().join("moved")).is_err(),
            "a pinned folder stays"
        );
        drop(pin);
        fs::rename(&real, temp.path().join("moved")).expect("released");
    }

    #[test]
    fn open_private_dir_creates_an_owner_only_folder_and_refuses_a_junction() {
        use crate::private_dir::open_private_dir;

        let temp = tempfile::tempdir().expect("a temporary folder");
        let made = temp.path().join("made");
        let held = open_private_dir(&made).expect("created and opened");
        let user = current_user().expect("the user");
        let descriptor = Descriptor::parse(&read_sddl(&made, Links::Open).expect("descriptor"));
        assert!(descriptor.is_owner_only(&user));
        drop(held);

        let elsewhere = temp.path().join("elsewhere");
        fs::create_dir(&elsewhere).expect("a folder");
        let link = temp.path().join("planted");
        junction(&link, &elsewhere);
        assert!(matches!(
            open_private_dir(&link),
            Err(RootAccessError::UnsafeLocation { path }) if path == link
        ));
    }
}
