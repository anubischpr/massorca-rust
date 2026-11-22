//! The agents API: installed kinds, enabled and disabled, and the catalogue's kinds.

use massorca_core_extensions::source::Source;
use massorca_core_extensions::{catalogue_agent_kinds, installed_agent_kinds};

use crate::fakes::FakeHost;
use crate::fixture::agent_package;
use crate::service::setup;

#[test]
fn an_empty_profile_has_no_installed_kinds() {
    let s = setup(FakeHost::new());
    assert!(installed_agent_kinds(&s.profile).expect("reads").is_empty());
}

#[test]
fn installed_kinds_list_enabled_and_disabled_packages_in_lockfile_order() {
    let s = setup(FakeHost::new().with_program("codex"));
    let kit = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&kit, true)
        .expect("kit installs with its installer");
    s.install(&Source::Catalogue("massorca.codex".to_owned()), false)
        .expect("codex installs");
    s.service.set_enabled("acme.kit", false).expect("disables");

    let kinds = installed_agent_kinds(&s.profile).expect("reads");
    let summary: Vec<_> = kinds
        .iter()
        .map(|kind| {
            (
                kind.package_id.as_str(),
                kind.kind_id.as_str(),
                kind.enabled,
                kind.cli_installed_by_package,
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("acme.kit", "kit", false, true),
            ("massorca.codex", "codex", true, false)
        ]
    );
    assert_eq!(kinds[0].package_version, "1.2.0");
    for kind in &kinds {
        let path = kind.path.as_ref().expect("a trusted package has a path");
        assert!(path.is_absolute(), "{}", path.display());
        assert!(path.ends_with(format!("agents/{}.json", kind.kind_id)));
        assert!(path.is_file());
        assert_eq!(kind.error, None);
    }
}

#[test]
fn the_catalogue_offers_the_four_first_party_kinds_with_their_cli() {
    let kinds = catalogue_agent_kinds();
    let ids: Vec<_> = kinds.iter().map(|kind| kind.kind_id.as_str()).collect();
    assert_eq!(ids, ["claude", "codex", "antigravity", "gentle-shell"]);
    for kind in &kinds {
        assert_eq!(kind.package_id, format!("massorca.{}", kind.kind_id));
        let declaration: serde_json::Value =
            serde_json::from_slice(&kind.declaration).expect("declaration is JSON");
        assert_eq!(declaration["id"], kind.kind_id.as_str());
        assert_eq!(
            kind.cli.verify.as_ref().expect("a verify command")[0],
            kind.cli.binary
        );
    }
}
