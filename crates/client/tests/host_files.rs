//! A paired host's device key stays owner-only even when the profile directory hands wide access to its children.
#![cfg(windows)]

use std::path::Path;

use massorca_client::{HostRecord, Hosts};
use massorca_net::TlsIdentity;
use massorca_profile::Profile;

fn assert_owner_only(path: &Path) {
    let owner_only = massorca_profile::is_owner_only(path).expect("read the DACL");
    let dacl = massorca_profile::dacl_sddl(path).unwrap_or_default();
    assert!(owner_only, "{}: {dacl}", path.display());
}

#[test]
fn device_key_and_host_directories_are_owner_only_under_a_wide_inheritable_profile() {
    let home = tempfile::tempdir().expect("a temporary home");
    let profile = Profile::resolve_with(
        Some(home.path().to_path_buf()),
        Some("default".to_owned()),
        |_| None,
    )
    .expect("the profile resolves");
    std::fs::create_dir_all(profile.dir()).expect("the profile directory");
    massorca_profile::set_dacl_sddl(profile.dir(), "D:(A;OICI;GA;;;WD)")
        .expect("widen the profile");

    let record = HostRecord {
        name: "laptop".to_owned(),
        server: "server".to_owned(),
        addresses: vec!["127.0.0.1:1".to_owned()],
        pin: "sha256:none".to_owned(),
        device_id: "device".to_owned(),
        device_name: "this-device".to_owned(),
        scopes: Vec::new(),
        paired_at_ms: 0,
    };
    let identity = TlsIdentity::generate("this-device").expect("an identity");
    Hosts::of(&profile)
        .add(&record, &identity)
        .expect("add the host");

    let hosts = profile.dir().join("hosts");
    assert_owner_only(&hosts);
    assert_owner_only(&hosts.join("laptop"));
    assert_owner_only(&hosts.join("laptop").join("device.key"));
}
