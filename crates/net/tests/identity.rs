//! Pins and identity files.

use massorca_net::{NetError, Pin, TlsIdentity};

#[test]
fn pin_round_trips_through_display() {
    let identity = TlsIdentity::generate("test").expect("generate");
    let pin = identity.pin();
    let text = pin.to_string();
    assert!(text.starts_with("sha256:"));
    assert_eq!(text.parse::<Pin>().expect("parse"), pin);
    assert_eq!(
        Pin::of_certificate(identity.certificate_der()).expect("pin"),
        pin
    );
}

#[test]
fn pin_refuses_wrong_prefix_and_length() {
    let text = TlsIdentity::generate("test")
        .expect("generate")
        .pin()
        .to_string();
    let encoded = text.trim_start_matches("sha256:");
    assert!(matches!(
        format!("sha1:{encoded}").parse::<Pin>(),
        Err(NetError::InvalidPin(_))
    ));
    assert!(matches!(
        encoded.parse::<Pin>(),
        Err(NetError::InvalidPin(_))
    ));
    assert!(matches!(
        format!("sha256:{}", &encoded[..20]).parse::<Pin>(),
        Err(NetError::InvalidPin(_))
    ));
    assert!(matches!(
        format!("{text}AAAA").parse::<Pin>(),
        Err(NetError::InvalidPin(_))
    ));
}

#[test]
fn generated_identities_have_different_pins() {
    let a = TlsIdentity::generate("a").expect("generate");
    let b = TlsIdentity::generate("b").expect("generate");
    assert_ne!(a.pin(), b.pin());
}

#[test]
fn debug_never_prints_the_key() {
    let identity = TlsIdentity::generate("test").expect("generate");
    let debug = format!("{identity:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("PRIVATE"));
}

#[test]
fn load_or_create_creates_then_loads_the_same_identity() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = dir.path().join("network").join("identity.key");
    let cert = dir.path().join("network").join("identity.crt");
    let created = TlsIdentity::load_or_create(&key, &cert, "server").expect("create");
    assert!(key.is_file() && cert.is_file());
    assert!(!dir.path().join("network").join("identity.key.tmp").exists());
    let loaded = TlsIdentity::load_or_create(&key, &cert, "server").expect("load");
    assert_eq!(loaded.pin(), created.pin());
    assert_eq!(
        TlsIdentity::load(&key, &cert).expect("load").pin(),
        created.pin()
    );
}

#[test]
fn load_or_create_refuses_a_half_present_pair() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = dir.path().join("identity.key");
    let cert = dir.path().join("identity.crt");
    TlsIdentity::load_or_create(&key, &cert, "server").expect("create");
    let key_bytes = std::fs::read(&key).expect("read key");
    std::fs::remove_file(&cert).expect("remove cert");
    assert!(matches!(
        TlsIdentity::load_or_create(&key, &cert, "server"),
        Err(NetError::Certificate(_))
    ));
    assert_eq!(
        std::fs::read(&key).expect("read key"),
        key_bytes,
        "the key is never overwritten"
    );
}

#[test]
fn load_refuses_a_certificate_of_another_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (key_a, cert_a) = (dir.path().join("a.key"), dir.path().join("a.crt"));
    let (key_b, cert_b) = (dir.path().join("b.key"), dir.path().join("b.crt"));
    TlsIdentity::generate("a")
        .expect("a")
        .save(&key_a, &cert_a)
        .expect("save a");
    TlsIdentity::generate("b")
        .expect("b")
        .save(&key_b, &cert_b)
        .expect("save b");
    assert!(matches!(
        TlsIdentity::load(&key_a, &cert_b),
        Err(NetError::Certificate(_))
    ));
}

#[cfg(unix)]
#[test]
fn key_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let key = dir.path().join("network").join("identity.key");
    let cert = dir.path().join("network").join("identity.crt");
    TlsIdentity::load_or_create(&key, &cert, "server").expect("create");
    let mode = std::fs::metadata(&key)
        .expect("metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let dir_mode = std::fs::metadata(dir.path().join("network"))
        .expect("metadata")
        .permissions()
        .mode();
    assert_eq!(dir_mode & 0o777, 0o700);
}

/// Asserts the DACL is protected and holds a single allow entry, for the current user.
#[cfg(windows)]
fn assert_owner_only(path: &std::path::Path) {
    let owner_only = massorca_profile::has_owner_only_dacl(path).expect("read the DACL");
    let dacl = massorca_profile::dacl_sddl(path).unwrap_or_default();
    assert!(owner_only, "{}: {dacl}", path.display());
}

#[cfg(windows)]
#[test]
fn key_file_is_owner_only_under_a_wide_inheritable_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    massorca_profile::set_dacl_sddl(dir.path(), "D:(A;OICI;GA;;;WD)").expect("widen the directory");
    let key = dir.path().join("identity.key");
    let cert = dir.path().join("identity.crt");
    TlsIdentity::load_or_create(&key, &cert, "server").expect("create");
    assert_owner_only(&key);
    assert!(
        massorca_profile::dacl_sddl(&cert)
            .expect("the certificate DACL")
            .contains(";WD)"),
        "the public certificate keeps the inherited entries"
    );
}

#[cfg(windows)]
#[test]
fn a_created_key_directory_is_owner_only_under_a_wide_inheritable_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    massorca_profile::set_dacl_sddl(dir.path(), "D:(A;OICI;GA;;;WD)").expect("widen the directory");
    let network = dir.path().join("network");
    let key = network.join("identity.key");
    TlsIdentity::load_or_create(&key, &network.join("identity.crt"), "server").expect("create");
    assert_owner_only(&network);
    assert_owner_only(&key);
}
