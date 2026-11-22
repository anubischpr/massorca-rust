//! The item page data and its listed images.

use massorca_core_extensions::codes;
use massorca_core_extensions::item::ItemState;
use massorca_core_extensions::source::Source;

use crate::fakes::FakeHost;
use crate::fixture::{agent_package, package_files, write_dir};
use crate::service::setup;

#[test]
fn a_catalogue_item_is_available_until_installed() {
    let s = setup(FakeHost::new().with_program("claude"));
    let catalogue = s.service.catalogue().expect("catalogue");
    assert_eq!(catalogue.len(), 4);
    let claude = &catalogue[0];
    assert_eq!(claude.id, "massorca.claude");
    assert_eq!(claude.state, ItemState::Available);
    assert_eq!(claude.detected, Some(true));
    assert_eq!(claude.listing.category, "agents");
    assert_eq!(claude.listing.mention.as_deref(), Some("claude"));
    assert_eq!(claude.uninstall.offered_agent_homes, ["claude"]);
    assert_eq!(
        catalogue[1].detected,
        Some(false),
        "codex is not on this host"
    );

    s.install(&Source::Catalogue("massorca.claude".to_owned()), false)
        .expect("installs");
    s.service
        .set_enabled("massorca.claude", false)
        .expect("disables");
    let item = s.service.get("massorca.claude").expect("item");
    assert_eq!(item.state, ItemState::Disabled);
    assert!(item.installed.is_some());
    assert_eq!(s.service.list().expect("list").len(), 1);
    let error = s.service.get("acme.none").expect_err("unknown");
    assert_eq!(error.code(), codes::NOT_FOUND);
}

#[test]
fn an_installed_item_shows_its_danger_zone_and_serializes() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, true).expect("installs");
    let item = s.service.get("acme.kit").expect("item");
    assert_eq!(item.state, ItemState::Enabled);
    assert!(item.agents[0].cli_installed_by_package);
    assert_eq!(item.uninstall.cli_binaries, ["kit"]);
    assert_eq!(item.uninstall.connections, ["account"]);
    let value = serde_json::to_value(&item).expect("serializes");
    assert_eq!(value["state"], "enabled");
    assert_eq!(value["listing"]["banner"]["examples"][0]["text"], "@kit");
    assert_eq!(value["risk"]["level"], "high");
}

#[test]
fn an_asset_is_served_only_when_the_listing_names_it() {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, false).expect("installs");
    assert_eq!(
        s.service.asset("acme.kit", "media/logo.svg").expect("logo"),
        b"<svg/>"
    );
    assert!(s.service.asset("acme.kit", "media/banner.png").is_ok());
    for unlisted in ["notes/private.txt", "extension.json", "../lock.json"] {
        let error = s.service.asset("acme.kit", unlisted).expect_err(unlisted);
        assert_eq!(error.code(), codes::ASSET_NOT_LISTED);
    }
}

#[test]
fn an_oversized_asset_is_refused() {
    let s = setup(FakeHost::new().with_program("kit"));
    let mut files = package_files(&agent_package("acme", "kit", "kit"));
    files.insert("media/logo.svg".to_owned(), vec![b' '; 1024 * 1024 + 1]);
    let dir = s.home.path().join("src").join("big");
    write_dir(&dir, &files);
    s.install(&Source::Dir(dir), false).expect("installs");
    let error = s
        .service
        .asset("acme.kit", "media/logo.svg")
        .expect_err("too large");
    assert_eq!(error.code(), codes::ASSET_TOO_LARGE);
}
