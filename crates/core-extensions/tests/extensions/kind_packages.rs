//! The agent kinds of the built-in catalogue and of installed packages by state (decision 0063). Every test runs on the
//! fake host, so no vendor command runs.

use std::fs;
use std::path::PathBuf;

use massorca_core_extensions::item::ItemState;
use massorca_core_extensions::source::Source;
use massorca_core_extensions::{
    PackageAgentKind, catalogue_agent_kinds, codes, package_agent_kinds,
};

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
    let s = setup(FakeHost::new());
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
    let s = setup(FakeHost::new().with_program("kit"));
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
    let s = setup(FakeHost::new());
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
    let s = setup(FakeHost::new());
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
