//! The profile root is created private and checked on start (ADR 0073).

use std::path::Path;

use massorca_profile::{Profile, RootAccess, ensure_dir, secure_root};

fn profile_in(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

#[test]
fn a_missing_root_is_created_and_then_found_private() {
    let home = tempfile::tempdir().expect("a home");
    let profile = profile_in(home.path());

    assert_eq!(secure_root(&profile).expect("created"), RootAccess::Created);
    assert!(profile.dir().is_dir());
    assert_eq!(secure_root(&profile).expect("checked"), RootAccess::Private);
}

#[test]
fn a_root_ensure_dir_created_needs_no_repair() {
    let home = tempfile::tempdir().expect("a home");
    let profile = profile_in(home.path());
    ensure_dir(&profile).expect("the profile directory");

    assert_eq!(secure_root(&profile).expect("checked"), RootAccess::Private);
}

/// A directory made the ordinary way takes its access from its parent, so the check repairs it once.
#[test]
fn a_root_created_without_restriction_is_repaired_once() {
    let home = tempfile::tempdir().expect("a home");
    let profile = profile_in(home.path());
    std::fs::create_dir_all(profile.dir()).expect("an ordinary directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(profile.dir(), std::fs::Permissions::from_mode(0o755))
            .expect("a permissive directory");
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

    let home = tempfile::tempdir().expect("a home");
    let profile = profile_in(home.path());
    secure_root(&profile).expect("created");

    let mode = std::fs::metadata(profile.dir())
        .expect("its metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}
