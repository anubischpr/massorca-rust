//! The agent kinds of the built-in catalogue and of installed packages by state (decision 0063). Every test runs on the
//! fake host, so no vendor command runs.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use massorca_core_extensions::item::ItemState;
use massorca_core_extensions::source::Source;
use massorca_core_extensions::{
    Extensions, PackageAgentKind, catalogue_agent_kinds, codes, package_agent_kinds,
};
use serde_json::json;

use crate::fakes::FakeHost;
use crate::fixture::agent_package;
use crate::service::{Setup, setup};

fn listed(s: &Setup) -> Vec<PackageAgentKind> {
    package_agent_kinds(&s.profile).expect("the kinds are listed")
}

fn listed_kind(s: &Setup, kind_id: &str) -> PackageAgentKind {
    listed(s)
        .into_iter()
        .find(|kind| kind.kind_id == kind_id)
        .expect("the kind is listed")
}

fn state_of(s: &Setup, kind_id: &str) -> ItemState {
    listed_kind(s, kind_id).state
}

/// The embedded declaration of a built-in kind, as text.
fn declaration(kind_id: &str) -> String {
    let kind = catalogue_agent_kinds()
        .into_iter()
        .find(|kind| kind.kind_id == kind_id)
        .expect("a built-in kind");
    String::from_utf8(kind.declaration).expect("a declaration is text")
}

/// The unpacked folder of the one installed package.
fn installed_folder(s: &Setup) -> PathBuf {
    let lock = s.lock_json().expect("the lockfile");
    let rel = lock["packages"][0]["path"].as_str().expect("a path");
    s.service.layout().resolve(rel)
}

/// A host where Gentle Shell is already on the machine, so installing it registers it without running its installer.
fn detected_gentle_shell() -> FakeHost {
    FakeHost::new().with_program("gentle-shell")
}

#[test]
fn an_empty_profile_lists_the_four_built_ins_as_available_with_their_declarations() {
    let s = setup(FakeHost::new());
    let kinds = listed(&s);
    let summary: Vec<_> = kinds
        .iter()
        .map(|kind| (kind.package_id.as_str(), kind.kind_id.as_str(), kind.state))
        .collect();
    assert_eq!(
        summary,
        [
            ("massorca.claude", "claude", ItemState::Available),
            ("massorca.codex", "codex", ItemState::Available),
            ("massorca.antigravity", "antigravity", ItemState::Available),
            (
                "massorca.gentle-shell",
                "gentle-shell",
                ItemState::Available
            ),
        ]
    );
    for (kind, built_in) in kinds.iter().zip(catalogue_agent_kinds()) {
        assert_eq!(kind.version, built_in.package_version);
        assert_eq!(kind.decl, Ok(declaration(&kind.kind_id)));
    }
}

#[test]
fn installing_disabling_and_removing_a_built_in_moves_it_through_its_states() {
    let s = setup(detected_gentle_shell());
    assert_eq!(state_of(&s, "gentle-shell"), ItemState::Available);

    s.install(
        &Source::Catalogue("massorca.gentle-shell".to_owned()),
        false,
    )
    .expect("installs");
    assert_eq!(state_of(&s, "gentle-shell"), ItemState::Enabled);
    assert_eq!(
        listed_kind(&s, "gentle-shell").decl,
        Ok(declaration("gentle-shell"))
    );

    s.service
        .set_enabled("massorca.gentle-shell", false)
        .expect("disables");
    assert_eq!(state_of(&s, "gentle-shell"), ItemState::Disabled);

    s.service
        .remove("massorca.gentle-shell", false, false, None)
        .expect("removes");
    assert_eq!(state_of(&s, "gentle-shell"), ItemState::Available);
    assert!(s.host.runs().is_empty(), "nothing ran: {:?}", s.host.runs());
}

#[test]
fn a_third_party_package_is_listed_after_the_built_ins_in_lockfile_order() {
    let s = setup(
        FakeHost::new()
            .with_program("kit")
            .with_program("gentle-shell"),
    );
    let kit = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&kit, false).expect("kit installs");
    s.install(
        &Source::Catalogue("massorca.gentle-shell".to_owned()),
        false,
    )
    .expect("gentle shell installs");

    let kinds = listed(&s);
    let summary: Vec<_> = kinds
        .iter()
        .map(|kind| (kind.package_id.as_str(), kind.kind_id.as_str(), kind.state))
        .collect();
    assert_eq!(
        summary,
        [
            ("massorca.claude", "claude", ItemState::Available),
            ("massorca.codex", "codex", ItemState::Available),
            ("massorca.antigravity", "antigravity", ItemState::Available),
            ("massorca.gentle-shell", "gentle-shell", ItemState::Enabled),
            ("acme.kit", "kit", ItemState::Enabled),
        ]
    );
}

#[test]
fn a_tampered_package_keeps_its_kinds_and_its_declaration_error_names_no_path() {
    let s = setup(detected_gentle_shell());
    s.install(
        &Source::Catalogue("massorca.gentle-shell".to_owned()),
        false,
    )
    .expect("installs");
    fs::write(
        installed_folder(&s).join("agents/gentle-shell.json"),
        br#"{"id":"changed"}"#,
    )
    .expect("the file is changed");

    let kind = listed_kind(&s, "gentle-shell");
    assert_eq!(kind.state, ItemState::Enabled);
    let problem = kind.decl.expect_err("a changed package is not trusted");
    assert!(problem.contains("massorca.gentle-shell"), "{problem}");
    assert!(problem.contains(codes::TAMPERED), "{problem}");
    assert!(
        !problem.contains('/') && !problem.contains('\\'),
        "{problem}"
    );
}

#[test]
fn a_package_whose_folder_is_gone_keeps_its_kinds_and_its_declaration_error_names_no_path() {
    let s = setup(detected_gentle_shell());
    s.install(
        &Source::Catalogue("massorca.gentle-shell".to_owned()),
        false,
    )
    .expect("installs");
    fs::remove_dir_all(installed_folder(&s)).expect("the folder is removed");

    let kind = listed_kind(&s, "gentle-shell");
    assert_eq!(kind.state, ItemState::Enabled);
    let problem = kind.decl.expect_err("an unreadable package is not used");
    assert!(problem.contains("massorca.gentle-shell"), "{problem}");
    assert!(problem.contains(codes::IO), "{problem}");
    assert!(
        !problem.contains('/') && !problem.contains('\\'),
        "{problem}"
    );
}

#[test]
fn a_damaged_lockfile_fails_the_listing() {
    let s = setup(FakeHost::new());
    let path = s.service.layout().lock_path();
    fs::create_dir_all(path.parent().expect("a parent")).expect("the profile folder");
    fs::write(&path, "{ not json").expect("the lockfile is damaged");

    let error = package_agent_kinds(&s.profile).expect_err("a damaged lockfile");
    assert_eq!(error.code(), codes::INVALID_LOCKFILE);
}

/// The built-in kinds a profile used, by kind id.
fn used(kind_ids: &[&str]) -> BTreeSet<String> {
    kind_ids.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn adoption_registers_exactly_the_used_built_ins_and_runs_no_command() {
    let s = setup(FakeHost::new());
    let adopted = s
        .service
        .adopt_builtin_agents(&used(&["claude", "codex"]))
        .expect("adopts");
    assert_eq!(adopted, ["massorca.claude", "massorca.codex"]);
    assert!(
        s.host.runs().is_empty(),
        "no vendor command ran: {:?}",
        s.host.runs()
    );
    assert_eq!(state_of(&s, "claude"), ItemState::Enabled);
    assert_eq!(state_of(&s, "codex"), ItemState::Enabled);
    assert_eq!(state_of(&s, "antigravity"), ItemState::Available);
    assert_eq!(state_of(&s, "gentle-shell"), ItemState::Available);

    let lock = s.lock_json().expect("the lockfile");
    assert_eq!(lock["migrations"], json!(["builtin-agents"]));
    let packages = lock["packages"].as_array().expect("packages");
    assert_eq!(packages.len(), 2);
    for entry in packages {
        assert_eq!(entry["adopted"], true);
        assert_eq!(entry["enabled"], true);
        assert_eq!(entry["source"]["kind"], "catalogue");
        assert_eq!(entry["cli"], json!([]));
    }
}

#[test]
fn adoption_runs_once_across_a_restart_so_a_removed_built_in_is_not_adopted_again() {
    let s = setup(FakeHost::new());
    let first = s
        .service
        .adopt_builtin_agents(&used(&["claude"]))
        .expect("adopts");
    assert_eq!(first, ["massorca.claude"]);
    s.service
        .remove("massorca.claude", false, false, None)
        .expect("removes");

    // A service started again on the same profile reads the marker from the lockfile.
    let restarted = Extensions::new(&s.profile, s.host.clone(), s.host.clone());
    let second = restarted
        .adopt_builtin_agents(&used(&["claude"]))
        .expect("a second run");
    assert!(second.is_empty());
    assert_eq!(state_of(&s, "claude"), ItemState::Available);
    let lock = s.lock_json().expect("the lockfile");
    assert_eq!(lock["migrations"], json!(["builtin-agents"]));
    assert_eq!(lock["packages"], json!([]));
}

#[test]
fn unknown_and_third_party_kind_ids_are_ignored_but_the_run_is_recorded() {
    let s = setup(
        FakeHost::new()
            .with_program("kit")
            .with_program("gentle-shell"),
    );
    let kit = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&kit, false).expect("kit installs");

    let adopted = s
        .service
        .adopt_builtin_agents(&used(&["kit", "ghost", "massorca.claude"]))
        .expect("adopts nothing");
    assert!(adopted.is_empty());
    let lock = s.lock_json().expect("the lockfile");
    assert_eq!(lock["migrations"], json!(["builtin-agents"]));
    let ids: Vec<&str> = lock["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    assert_eq!(ids, ["acme.kit"]);
}

#[test]
fn an_empty_set_of_used_kinds_still_records_the_run() {
    let s = setup(FakeHost::new());
    let adopted = s
        .service
        .adopt_builtin_agents(&BTreeSet::new())
        .expect("records the run");
    assert!(adopted.is_empty());
    let lock = s.lock_json().expect("the lockfile");
    assert_eq!(lock["migrations"], json!(["builtin-agents"]));
    assert_eq!(lock["packages"], json!([]));
}

#[test]
fn adoption_leaves_an_installed_built_in_as_it_is_and_adopts_the_rest() {
    let s = setup(FakeHost::new().with_program("claude"));
    s.install(&Source::Catalogue("massorca.claude".to_owned()), false)
        .expect("claude installs");
    let before = s.lock_json().expect("the lockfile")["packages"][0].clone();

    let adopted = s
        .service
        .adopt_builtin_agents(&used(&["claude", "codex"]))
        .expect("adopts");
    assert_eq!(adopted, ["massorca.codex"]);
    let lock = s.lock_json().expect("the lockfile");
    assert_eq!(lock["packages"][0], before, "the install is untouched");
    assert!(lock["packages"][0].get("adopted").is_none());
    assert_eq!(lock["packages"][1]["id"], "massorca.codex");
    assert_eq!(lock["packages"][1]["adopted"], true);
}

#[test]
fn a_damaged_lockfile_fails_adoption_and_is_left_as_it_was() {
    let s = setup(FakeHost::new());
    let path = s.service.layout().lock_path();
    fs::create_dir_all(path.parent().expect("a parent")).expect("the profile folder");
    fs::write(&path, "{ not json").expect("the lockfile is damaged");

    let error = s
        .service
        .adopt_builtin_agents(&used(&["claude"]))
        .expect_err("a damaged lockfile");
    assert_eq!(error.code(), codes::INVALID_LOCKFILE);
    assert_eq!(fs::read_to_string(&path).expect("read"), "{ not json");
}

#[test]
fn adoption_replaces_a_first_party_entry_the_catalogue_does_not_vouch_for() {
    let s = setup(FakeHost::new());
    let path = s.service.layout().lock_path();
    fs::create_dir_all(path.parent().expect("a parent")).expect("the profile folder");
    let forged = json!({ "version": 1, "packages": [{
        "id": "massorca.claude", "version": "1.0.0",
        "source": { "kind": "dir", "ref": "C:/evil" },
        "sha256": "ab", "path": "pkgs/massorca.claude/1.0.0-ab", "enabled": true,
        "consent": "c", "installed_at_ms": 0, "agent_kinds": ["claude"]
    }] });
    fs::write(&path, forged.to_string()).expect("the forged lockfile");

    let adopted = s
        .service
        .adopt_builtin_agents(&used(&["claude"]))
        .expect("adopts");
    assert_eq!(adopted, ["massorca.claude"]);
    let lock = s.lock_json().expect("the lockfile");
    let packages = lock["packages"].as_array().expect("packages");
    assert_eq!(packages.len(), 1, "{lock}");
    assert_eq!(packages[0]["source"]["kind"], "catalogue");
    assert_eq!(state_of(&s, "claude"), ItemState::Enabled);
}
