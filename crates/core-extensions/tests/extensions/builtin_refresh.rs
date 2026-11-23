//! A built-in install whose files predate the package this build embeds is brought up to date in place (decision 0063).
//! Every test runs on the fake host, so no vendor command runs.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use massorca_core_extensions::item::ItemState;
use massorca_core_extensions::source::Source;
use massorca_core_extensions::{PackageAgentKind, catalogue_agent_kinds, package_agent_kinds};
use serde_json::{Value, json};

use crate::fakes::FakeHost;
use crate::service::{Setup, setup};

const OLD_PATH: &str = "pkgs/massorca.gentle-shell/0.0.1-00000000";
const OLD_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn gentle_shell(s: &Setup) -> PackageAgentKind {
    package_agent_kinds(&s.profile)
        .expect("the kinds are listed")
        .into_iter()
        .find(|kind| kind.kind_id == "gentle-shell")
        .expect("gentle shell is listed")
}

fn write_lock(s: &Setup, lock: &Value) {
    fs::write(s.service.layout().lock_path(), lock.to_string()).expect("the lockfile is written");
}

/// Installs Gentle Shell, then rewrites its entry as an older build left it: another version, hash and folder, and
/// the state the person gave it. Returns the old folder.
fn outdated_gentle_shell(s: &Setup, edit: impl FnOnce(&mut Value)) -> PathBuf {
    s.install(
        &Source::Catalogue("massorca.gentle-shell".to_owned()),
        false,
    )
    .expect("installs");
    let mut lock = s.lock_json().expect("the lockfile");
    let entry = &mut lock["packages"][0];
    let current = s
        .service
        .layout()
        .resolve(entry["path"].as_str().expect("a path"));
    let old = s.service.layout().resolve(OLD_PATH);
    fs::create_dir_all(old.parent().expect("a parent")).expect("the package folder");
    fs::rename(&current, &old).expect("the files move to the old folder");
    entry["version"] = json!("0.0.1");
    entry["sha256"] = json!(OLD_HASH);
    entry["files_sha256"] = json!(OLD_HASH);
    entry["path"] = json!(OLD_PATH);
    edit(entry);
    write_lock(s, &lock);
    old
}

#[test]
fn an_outdated_built_in_is_refreshed_in_place_and_keeps_its_state() {
    let s = setup(FakeHost::new().with_program("gentle-shell"));
    let old = outdated_gentle_shell(&s, |entry| {
        entry["enabled"] = json!(false);
        entry["consent"] = json!("accepted-earlier");
        entry["installed_at_ms"] = json!(7);
        entry["cli"] = json!([{ "agent": "gentle-shell", "binary": "gentle-shell", "detected_before_install": true, "installed_by_package": false }]);
        entry["adopted"] = json!(true);
        entry["pinned"] = json!(true);
    });
    assert_eq!(
        gentle_shell(&s).state,
        ItemState::Available,
        "an outdated install is not the built-in"
    );

    let refreshed = s.service.refresh_builtins().expect("refreshes");
    assert_eq!(refreshed, ["massorca.gentle-shell"]);
    assert!(s.host.runs().is_empty(), "nothing ran: {:?}", s.host.runs());

    let lock = s.lock_json().expect("the lockfile");
    let entry = &lock["packages"][0];
    let built_in = catalogue_agent_kinds()
        .into_iter()
        .find(|kind| kind.kind_id == "gentle-shell")
        .expect("a built-in kind");
    assert_eq!(entry["version"], json!(built_in.package_version));
    assert_ne!(entry["sha256"], json!(OLD_HASH));
    assert_ne!(entry["path"], json!(OLD_PATH));
    assert_eq!(entry["enabled"], false);
    assert_eq!(entry["consent"], "accepted-earlier");
    assert_eq!(entry["installed_at_ms"], 7);
    assert_eq!(
        entry["cli"],
        json!([{ "agent": "gentle-shell", "binary": "gentle-shell", "detected_before_install": true, "installed_by_package": false }])
    );
    assert_eq!(entry["adopted"], true);
    assert_eq!(entry["pinned"], true);
    assert!(!old.exists(), "the outdated folder is removed");

    let kind = gentle_shell(&s);
    assert_eq!(kind.state, ItemState::Disabled);
    assert_eq!(
        kind.decl,
        Ok(String::from_utf8(built_in.declaration).expect("a declaration is text"))
    );
}

#[test]
fn a_current_built_in_is_not_rewritten() {
    let s = setup(FakeHost::new().with_program("gentle-shell"));
    s.install(
        &Source::Catalogue("massorca.gentle-shell".to_owned()),
        false,
    )
    .expect("installs");
    let path = s.service.layout().lock_path();
    let before = fs::read(&path).expect("read");

    assert!(s.service.refresh_builtins().expect("refreshes").is_empty());
    assert_eq!(fs::read(&path).expect("read"), before);
}

#[test]
fn a_first_party_entry_from_another_source_is_not_refreshed() {
    let s = setup(FakeHost::new());
    let path = s.service.layout().lock_path();
    fs::create_dir_all(path.parent().expect("a parent")).expect("the profile folder");
    write_lock(
        &s,
        &json!({ "version": 1, "packages": [{
            "id": "massorca.claude", "version": "1.0.0",
            "source": { "kind": "dir", "ref": "C:/Users/me/claude" },
            "sha256": "ab", "path": "pkgs/massorca.claude/1.0.0-ab", "enabled": true,
            "consent": "c", "installed_at_ms": 0, "agent_kinds": ["claude"]
        }] }),
    );

    assert!(s.service.refresh_builtins().expect("refreshes").is_empty());
    assert_eq!(
        s.lock_json().expect("the lockfile")["packages"][0]["sha256"],
        "ab"
    );
}

#[test]
fn adoption_after_an_upgrade_refreshes_an_outdated_built_in_instead_of_resetting_it() {
    let s = setup(FakeHost::new().with_program("gentle-shell"));
    outdated_gentle_shell(&s, |entry| entry["enabled"] = json!(false));

    let used: BTreeSet<String> = ["gentle-shell".to_owned()].into();
    let adopted = s.service.adopt_builtin_agents(&used).expect("adopts");
    assert!(adopted.is_empty(), "it was already installed: {adopted:?}");
    let lock = s.lock_json().expect("the lockfile");
    assert_eq!(lock["packages"].as_array().expect("packages").len(), 1);
    assert!(lock["packages"][0].get("adopted").is_none());
    assert_eq!(gentle_shell(&s).state, ItemState::Disabled);
}
