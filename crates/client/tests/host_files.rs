//! A paired host's device key and folders are owner-only, and a profile that hands wide access to its children is
//! refused before anything is written into it (ADR 0073).
#![cfg(windows)]

use std::path::Path;

use massorca_client::{HostRecord, Hosts};
use massorca_net::TlsIdentity;
use massorca_profile::Profile;

fn assert_owner_only(path: &Path) {
    let owner_only = massorca_profile::has_owner_only_dacl(path).expect("read the DACL");
    let dacl = massorca_profile::dacl_sddl(path).unwrap_or_default();
    assert!(owner_only, "{}: {dacl}", path.display());
}

fn record() -> HostRecord {
    HostRecord {
        name: "laptop".to_owned(),
        server: "server".to_owned(),
        addresses: vec!["127.0.0.1:1".to_owned()],
        pin: "sha256:none".to_owned(),
        device_id: "device".to_owned(),
        device_name: "this-device".to_owned(),
        scopes: Vec::new(),
        paired_at_ms: 0,
    }
}

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
fn private_profile(temp: &tempfile::TempDir) -> Profile {
    let home = temp.path().join("home");
    massorca_profile::create_home(&home).expect("a private home");
    Profile::resolve_with(Some(home), Some("default".to_owned()), |_| None)
        .expect("the profile resolves")
}

#[test]
fn device_key_and_host_directories_are_owner_only() {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = private_profile(&temp);
    let identity = TlsIdentity::generate("this-device").expect("an identity");
    Hosts::of(&profile)
        .add(&record(), &identity)
        .expect("add the host");

    let hosts = profile.dir().join("hosts");
    assert_owner_only(&hosts);
    assert_owner_only(&hosts.join("laptop"));
    assert_owner_only(&hosts.join("laptop").join("device.key"));
}

#[test]
fn a_profile_that_hands_wide_access_to_its_children_is_refused_before_the_key_is_written() {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = private_profile(&temp);
    std::fs::create_dir_all(profile.dir()).expect("the profile directory");
    massorca_profile::set_dacl_sddl(profile.dir(), "D:(A;OICI;GA;;;WD)")
        .expect("widen the profile");

    let identity = TlsIdentity::generate("this-device").expect("an identity");
    let refused = Hosts::of(&profile)
        .add(&record(), &identity)
        .expect_err("a profile others can change is refused");
    assert!(
        refused.to_string().contains("profile."),
        "a typed profile refusal: {refused}"
    );
    assert!(
        !profile
            .dir()
            .join("hosts")
            .join("laptop")
            .join("device.key")
            .exists(),
        "nothing is written into a refused profile"
    );
}
