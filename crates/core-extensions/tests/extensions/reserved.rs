//! The first party's identity comes only from the built-in catalogue (0062, second review): a package from any other
//! source that claims publisher `massorca` or a `massorca.*` id is refused, and a lock entry the catalogue does not
//! vouch for never shows as a built-in nor blocks one.

use std::fs;

use massorca_core_extensions::codes;
use massorca_core_extensions::installed_agent_kinds;
use massorca_core_extensions::source::{Source, sha256_hex};
use serde_json::{Value, json};

use crate::fakes::FakeHost;
use crate::fixture::{agent_package, package_files, zip};
use crate::service::{Setup, setup};

const WRONG: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn write_lock(s: &Setup, packages: Value) {
    let path = s.service.layout().lock_path();
    fs::create_dir_all(path.parent().expect("parent")).expect("the profile folder");
    fs::write(
        path,
        json!({ "version": 1, "packages": packages }).to_string(),
    )
    .expect("the lockfile");
}

/// A lockfile entry written by hand, as an old or damaged lockfile might hold it.
fn forged(id: &str, kind: &str, reference: &str, agent_kinds: &[&str]) -> Value {
    json!({
        "id": id, "version": "1.0.0", "source": { "kind": kind, "ref": reference },
        "sha256": "ab", "path": format!("pkgs/{id}/1.0.0-ab"), "enabled": true,
        "consent": "c", "installed_at_ms": 0, "agent_kinds": agent_kinds
    })
}

#[test]
fn a_dir_package_claiming_the_first_party_is_refused_at_review_and_install() {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("first", &agent_package("massorca", "kit", "kit"));

    let review = s.service.inspect(&source);
    assert_eq!(
        review.expect_err("refused").code(),
        codes::RESERVED_PUBLISHER
    );
    let install = s.service.install(&source, Some(WRONG), false, None);
    assert_eq!(
        install.expect_err("refused").code(),
        codes::RESERVED_PUBLISHER
    );
    assert!(s.lock_json().is_none(), "nothing is registered");
}

#[test]
fn a_file_package_claiming_the_first_party_is_refused_at_review_and_install() {
    let s = setup(FakeHost::new().with_program("kit"));
    let bytes = zip(&package_files(&agent_package("massorca", "kit", "kit")));
    let path = s.home.path().join("first.mxt");
    fs::write(&path, &bytes).expect("archive");
    let source = Source::File(path);
    let pin = sha256_hex(&bytes);

    let review = s.service.inspect(&source);
    assert_eq!(
        review.expect_err("refused").code(),
        codes::RESERVED_PUBLISHER
    );
    let install = s.service.install(&source, Some(WRONG), false, Some(&pin));
    assert_eq!(
        install.expect_err("refused").code(),
        codes::RESERVED_PUBLISHER
    );
    assert!(s.lock_json().is_none(), "nothing is registered");
}

#[test]
fn a_hand_written_first_party_entry_never_shows_as_installed_or_contributes_a_kind() {
    let s = setup(FakeHost::new());
    write_lock(
        &s,
        json!([forged("massorca.claude", "dir", "C:/evil", &["claude"])]),
    );

    let catalogue = s.service.catalogue().expect("the catalogue");
    let claude = catalogue
        .iter()
        .find(|item| item.id == "massorca.claude")
        .expect("the built-in is listed");
    assert!(claude.installed.is_none(), "{claude:?}");

    let item = s.service.get("massorca.claude").expect("the item");
    assert_eq!(
        item.problem.as_ref().map(|problem| problem.code.as_str()),
        Some(codes::RESERVED_PUBLISHER)
    );
    let kinds = installed_agent_kinds(&s.profile).expect("the kinds");
    assert!(
        kinds
            .iter()
            .all(|kind| kind.package_id != "massorca.claude")
    );
}

#[test]
fn a_third_party_entry_never_blocks_the_built_in_install() {
    let s = setup(FakeHost::new().with_program("claude"));
    write_lock(
        &s,
        json!([forged("acme.kit", "dir", "C:/evil", &["claude"])]),
    );

    s.install(&Source::Catalogue("massorca.claude".to_owned()), false)
        .expect("the built-in installs");

    let lock = s.lock_json().expect("the lockfile");
    let ids: Vec<&str> = lock["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    assert_eq!(ids, ["acme.kit", "massorca.claude"]);
    assert_eq!(lock["packages"][1]["source"]["kind"], "catalogue");
}

#[test]
fn the_built_in_install_replaces_a_first_party_entry_the_catalogue_does_not_vouch_for() {
    let s = setup(FakeHost::new().with_program("claude"));
    write_lock(
        &s,
        json!([forged("massorca.claude", "dir", "C:/evil", &["claude"])]),
    );

    s.install(&Source::Catalogue("massorca.claude".to_owned()), false)
        .expect("the built-in installs");

    let lock = s.lock_json().expect("the lockfile");
    let packages = lock["packages"].as_array().expect("packages");
    assert_eq!(packages.len(), 1, "{lock}");
    assert_eq!(packages[0]["source"]["kind"], "catalogue");
    assert_eq!(packages[0]["source"]["ref"], "massorca.claude");
}
