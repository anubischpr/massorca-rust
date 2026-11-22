//! The profile root is created private and checked on start (ADR 0073).

use std::path::Path;

use massorca_profile::{Profile, RootAccess, create_root, ensure_dir, secure_root};

/// The home sits below the temporary folder, which other users may be able to change on a test machine.
fn profile_in(temp: &Path, name: &str) -> Profile {
    Profile::resolve_with(Some(temp.join("home")), Some(name.to_owned()), |_| None)
        .expect("the profile resolves")
}

#[test]
fn a_missing_root_is_created_and_then_found_private() {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = profile_in(temp.path(), "default");

    assert_eq!(secure_root(&profile).expect("created"), RootAccess::Created);
    assert!(profile.dir().is_dir());
    assert_eq!(secure_root(&profile).expect("checked"), RootAccess::Private);
}

#[test]
fn a_root_ensure_dir_or_create_root_made_needs_no_repair() {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let made_by_ensure = profile_in(temp.path(), "ensured");
    let made_by_create = profile_in(temp.path(), "created");
    ensure_dir(&made_by_ensure).expect("the profile directory");
    assert!(create_root(&made_by_create).expect("the profile directory"));

    assert_eq!(
        secure_root(&made_by_ensure).expect("checked"),
        RootAccess::Private
    );
    assert_eq!(
        secure_root(&made_by_create).expect("checked"),
        RootAccess::Private
    );
}

/// A root made the ordinary way inside a private `profiles` folder inherits only the owner, so it is repaired once.
#[test]
fn a_root_created_without_restriction_is_repaired_once() {
    let temp = tempfile::tempdir().expect("a temporary folder");
    secure_root(&profile_in(temp.path(), "seed")).expect("private ancestors");
    let profile = profile_in(temp.path(), "default");
    std::fs::create_dir(profile.dir()).expect("an ordinary directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(profile.dir(), std::fs::Permissions::from_mode(0o755))
            .expect("a directory others can read");
    }

    assert_eq!(
        secure_root(&profile).expect("repaired"),
        RootAccess::Repaired
    );
    assert_eq!(secure_root(&profile).expect("checked"), RootAccess::Private);
}

#[cfg(unix)]
#[test]
fn a_new_root_has_mode_0700() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = profile_in(temp.path(), "default");
    secure_root(&profile).expect("created");

    let mode = std::fs::metadata(profile.dir())
        .expect("its metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}
